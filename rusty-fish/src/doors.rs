//! Die Türen eines laufenden Fischs.
//!
//! Seit dem 22.11.2026 hat der Fisch zwei Türen, Discord und Matrix. Ein
//! Profil steht an genau einer; welche, sagen seine Tische. Der Fisch öffnet
//! eine Tür, sobald er ein Profil an ihr bedient, und beide laufen
//! nebeneinander in einem Programm. Hier steht, wer von beiden was zu hören
//! bekommt: ein Profil geht an seine Tür, „stop" an alle.
//!
//! Die Türen wissen voneinander nichts. Ein Raum mit Tischen an beiden ist
//! nicht gebaut (siehe `rusty_fish_house::config`).
//!
//! Benannte Grenze: schließt eine Tür gerade, weil ihr letztes Profil
//! angehalten wurde, kann ein Profil, das im selben Augenblick für sie
//! gestartet wird, verloren gehen. Die Programmausgabe nennt es dann nicht
//! als bedient; ein zweiter Start nimmt es auf.

use crate::keys;
use crate::lock::Held;
use crate::profiles::{self, BOT};
use crate::secrets;
use crate::stop::{self, Input};
use rusty_fish_house::config::Door;
use rusty_fish_house::served::{Command, OnChange, Profile};
use rusty_fish_matrix::session::Access;
use rusty_fish_providers::AnyModel;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

/// Ein Profil, fertig geladen für seine Tür.
pub type Loaded = Profile<AnyModel>;

const DOORS: [Door; 2] = [Door::Discord, Door::Matrix];

/// Was dem laufenden Fisch zustößt.
enum Event {
    /// Ihm wird etwas gesagt, von der Maske oder im Terminal.
    Said(Input),
    /// Eine Tür meldet, welche Profile sie jetzt bedient.
    Serving(Door, Vec<String>),
    /// Eine Tür ist zu.
    Ended(Door, Result<(), String>),
}

/// Eine offene Tür: die Leitung zu ihr, und was sie gerade bedient.
struct Open {
    door: Door,
    to: UnboundedSender<Command<AnyModel>>,
    serving: Vec<String>,
}

/// Wo die Geräteschlüssel des Fischs für Matrix liegen: in seinem eigenen
/// Ordner, neben den Profilen und nicht darin.
pub fn matrix_store(home: &Path) -> PathBuf {
    home.join("matrix")
}

/// Der Zugang zum Matrix-Konto aus der Schlüsselablage.
pub fn matrix_access() -> Result<Access, String> {
    let secret = keys::load(secrets::MATRIX)?.ok_or_else(|| {
        "the fish is not logged in to Matrix; run `rusty-fish matrix login --user @name:server` \
         first"
            .to_owned()
    })?;
    Access::parse(secret.expose())
}

/// Öffnet eine Tür mit ihren ersten Profilen. Scheitert schon hier, wenn ihr
/// Zugang fehlt; alles Weitere meldet die Tür selbst als Ereignis.
fn open(
    door: Door,
    first: Vec<Loaded>,
    lenient: bool,
    events: &UnboundedSender<Event>,
    home: &Path,
) -> Result<Open, String> {
    let (to, commands) = unbounded_channel();
    let on_change: OnChange = {
        let events = events.clone();
        Box::new(move |names: &[String]| {
            let _ = events.send(Event::Serving(door, names.to_vec()));
        })
    };
    let events = events.clone();
    match door {
        Door::Discord => {
            let token = keys::load(BOT)?.ok_or_else(|| {
                format!("{} not stored; run `rusty-fish key set {BOT}` first", secrets::label(BOT))
            })?;
            tokio::spawn(async move {
                let ended =
                    rusty_fish_discord::door::run(token.expose(), first, lenient, commands, on_change)
                        .await;
                let _ = events.send(Event::Ended(door, ended));
            });
        }
        Door::Matrix => {
            let access = matrix_access()?;
            let store = matrix_store(home);
            tokio::spawn(async move {
                let ended =
                    rusty_fish_matrix::door::run(&access, &store, first, lenient, commands, on_change)
                        .await;
                let _ = events.send(Event::Ended(door, ended));
            });
        }
    }
    Ok(Open { door, to, serving: Vec::new() })
}

/// Lässt den Fisch laufen, bis er angehalten wird oder seine letzte Tür zu ist.
///
/// - `first`: die Profile, mit denen er beginnt, jedes für seine Tür.
/// - `lenient`: wie bei „alle Profile": was sich nicht starten lässt, sagt
///   warum und bleibt weg, und der Rest läuft. Streng (`false`) hält der
///   Fisch an, sobald eine Tür scheitert: der Aufrufer hat genau diese
///   Profile verlangt.
/// - `load`: lädt ein Profil nach, wenn es im Lauf dazukommt oder sich ändert.
/// - `held`: die Sperre dieses Laufs; in ihrer Notiz steht, welche Profile
///   bedient werden, und die Maske liest sie.
pub async fn run(
    first: Vec<Loaded>,
    lenient: bool,
    load: impl Fn(&str) -> Result<Loaded, String>,
    held: Option<Held>,
    home: PathBuf,
) -> Result<(), String> {
    let (events, mut happened) = unbounded_channel::<Event>();

    // Erst alles prüfen, dann verbinden: fehlt ein Zugang, scheitert der
    // Fisch, bevor er sich irgendwo zeigt.
    let mut doors: Vec<Open> = Vec::new();
    let mut waiting = first;
    for door in DOORS {
        let (mine, rest): (Vec<Loaded>, Vec<Loaded>) =
            waiting.into_iter().partition(|profile| profile.config.door == door);
        waiting = rest;
        if mine.is_empty() {
            continue;
        }
        let names: Vec<Option<String>> = mine.iter().map(|profile| profile.name.clone()).collect();
        match open(door, mine, lenient, &events, &home) {
            Ok(opened) => doors.push(opened),
            Err(why) if lenient => {
                for name in names {
                    let label = rusty_fish_house::served::label_of(name.as_deref());
                    eprintln!("rusty-fish: {label}cannot start: {why}");
                }
            }
            Err(why) => return Err(why),
        }
    }
    if doors.is_empty() {
        return Err("there is no profile that can be started".to_owned());
    }

    // Was dem Fisch gesagt wird, kommt in dieselbe Reihe wie das, was seine
    // Türen melden.
    let mut said = stop::listen();
    tokio::spawn({
        let events = events.clone();
        async move {
            while let Some(input) = said.recv().await {
                if events.send(Event::Said(input)).is_err() {
                    break;
                }
            }
        }
    });

    let mut stopping = false;
    let mut failed: Option<String> = None;
    while let Some(event) = happened.recv().await {
        match event {
            Event::Serving(door, names) => {
                if let Some(open) = doors.iter_mut().find(|open| open.door == door) {
                    open.serving = names;
                }
                note(&held, &doors);
            }
            Event::Ended(door, ended) => {
                doors.retain(|open| open.door != door);
                note(&held, &doors);
                if let Err(why) = ended {
                    if !doors.is_empty() {
                        eprintln!("rusty-fish: the {} door is closed: {why}", door.label());
                    }
                    failed.get_or_insert(why);
                    // Streng: eine gescheiterte Tür hält den ganzen Fisch an.
                    if !lenient && !stopping {
                        stopping = true;
                        tell_all(&doors, || Command::StopAll);
                    }
                }
                if doors.is_empty() {
                    break;
                }
            }
            Event::Said(Input::StopAll) => {
                stopping = true;
                eprintln!("rusty-fish: stopping");
                tell_all(&doors, || Command::StopAll);
            }
            Event::Said(Input::Stop(name)) => {
                // An die Tür, die es bedient. Weiß es keine, sollen es alle
                // hören: jede sagt dann, dass es bei ihr nicht lief.
                let serving = doors.iter().any(|open| open.serving.contains(&name));
                for open in &doors {
                    if !serving || open.serving.contains(&name) {
                        let _ = open.to.send(Command::Stop(name.clone()));
                    }
                }
            }
            Event::Said(Input::Start(name)) => match load(&name) {
                Ok(profile) => {
                    let door = profile.config.door;
                    match doors.iter().find(|open| open.door == door) {
                        Some(open) => {
                            let _ = open.to.send(Command::Start(Box::new(profile)));
                        }
                        None => match open(door, vec![profile], false, &events, &home) {
                            Ok(opened) => doors.push(opened),
                            Err(why) => eprintln!("rusty-fish: [{name}] cannot start: {why}"),
                        },
                    }
                }
                Err(why) => eprintln!("rusty-fish: [{name}] cannot start: {why}"),
            },
            // Lässt sich das geänderte Profil nicht laden, läuft das
            // bisherige weiter: lieber der alte Stand als keiner.
            Event::Said(Input::Restart(name)) => match load(&name) {
                Ok(profile) => {
                    let door = profile.config.door;
                    // Steht das Profil nach der Änderung an der anderen Tür,
                    // geht es dort.
                    for open in doors.iter().filter(|open| open.door != door) {
                        if open.serving.contains(&name) {
                            let _ = open.to.send(Command::Stop(name.clone()));
                        }
                    }
                    match doors.iter().find(|open| open.door == door) {
                        Some(open) => {
                            let _ = open.to.send(Command::Restart(Box::new(profile)));
                        }
                        None => match open(door, vec![profile], false, &events, &home) {
                            Ok(opened) => doors.push(opened),
                            Err(why) => eprintln!("rusty-fish: [{name}] cannot start: {why}"),
                        },
                    }
                }
                Err(why) => eprintln!("rusty-fish: [{name}] cannot restart: {why}"),
            },
        }
    }

    match failed {
        // Nach einem gewollten Anhalten ist das Ende einer Tür kein Fehler.
        Some(why) if !stopping || !lenient => Err(why),
        _ => Ok(()),
    }
}

/// Sagt allen offenen Türen dasselbe.
fn tell_all(doors: &[Open], command: impl Fn() -> Command<AnyModel>) {
    for open in doors {
        let _ = open.to.send(command());
    }
}

/// Schreibt in die Notiz zur Sperre, welche Profile der Fisch jetzt bedient,
/// über alle Türen hinweg.
fn note(held: &Option<Held>, doors: &[Open]) {
    let names: Vec<String> = doors.iter().flat_map(|open| open.serving.iter().cloned()).collect();
    if let Some(held) = held
        && let Err(why) = held.set_profiles(&names)
    {
        eprintln!("rusty-fish: warning: {why}");
    }
}

/// Ob gerade ein Fisch läuft. Die Ablage der Geräteschlüssel darf dann
/// niemand anderes anfassen.
pub fn fish_is_running(home: &Path) -> bool {
    crate::lock::running(&home.join("profiles"), profiles::BOT).is_some()
}
