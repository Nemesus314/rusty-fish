//! Die Türen eines laufenden Fischs.
//!
//! Seit dem 22.11.2026 hat der Fisch zwei Türen, Discord und Matrix. An
//! welcher ein Profil steht, sagen seine Tische. Der Fisch öffnet eine Tür,
//! sobald er ein Profil an ihr bedient, und beide laufen nebeneinander in
//! einem Programm. Hier steht, wer von beiden was zu hören bekommt: ein Profil
//! geht an seine Türen, „stop" an alle.
//!
//! Seit dem 23.11.2026 kann ein Profil Tische an beiden Türen haben: der
//! gemischte Raum. Die Türen wissen weiter nichts voneinander. Das Programm
//! öffnet für so ein Profil einen Haushalt, gibt ihn beiden Türen mit und
//! legt die Leitung zwischen ihnen (siehe `rusty_fish_house::between`).
//! Starten, Anhalten und Neuladen gehen an beide.
//!
//! Ein Profil an zwei Türen läuft an beiden oder gar nicht. Ein halber Raum,
//! in dem die eine Seite schreibt und an der anderen nichts ankommt, wäre
//! schlimmer als keiner:
//! - Fehlt einer Tür der Zugang, startet das Profil nicht und sagt, was fehlt.
//! - Lehnt eine Tür das Profil ab oder schließt sie, während die andere es
//!   bedient, hält das Programm es auch dort an und sagt es.
//!
//! Benannte Grenzen:
//! - Wird ein Profil an zwei Türen neu geladen, nimmt jede Tür es für sich neu
//!   auf, die eine einen Augenblick vor der anderen. Eine Nachricht, die genau
//!   dazwischen ankommt, kann noch nach den alten Einstellungen laufen.
//! - Ein Profil, das an einer Tür nicht aufgenommen werden konnte, wird an
//!   der anderen angehalten, auch wenn es dort für sich laufen könnte.
//! - Schließt eine Tür gerade, weil ihr letztes Profil angehalten wurde, kann
//!   ein Profil, das im selben Augenblick für sie gestartet wird, verloren
//!   gehen. Die Programmausgabe nennt es dann nicht als bedient; ein zweiter
//!   Start nimmt es auf. Stand es an zwei Türen, wird es an der anderen
//!   angehalten.

use crate::keys;
use crate::lock::Held;
use crate::profiles::{self, BOT};
use crate::secrets;
use crate::stop::{self, Input};
use rusty_fish_core::Secret;
use rusty_fish_house::between::{Handed, Lines, Refused, Shared};
use rusty_fish_house::config::Door;
use rusty_fish_house::household::Household;
use rusty_fish_house::served::{Command, OnChange, Profile, label_of};
use rusty_fish_matrix::session::Access;
use rusty_fish_providers::AnyModel;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

/// Ein Profil, fertig geladen für seine Türen.
pub type Loaded = Profile<AnyModel>;

/// Was eine laufende Tür am Ende meldet.
type Running = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;

/// Wie das Programm eine Tür öffnet. Im Programm sind es die echten Türen
/// mit ihrem Zugang aus der Schlüsselablage; in den Tests Attrappen.
trait Entrance<M> {
    /// Kann diese Tür geöffnet werden? Wenn nicht, sagt die Meldung, was fehlt.
    fn ready(&self, door: Door) -> Result<(), String>;

    /// Lässt die Tür laufen, bis sie zu ist.
    fn run(
        &self,
        door: Door,
        first: Vec<Profile<M>>,
        lenient: bool,
        commands: UnboundedReceiver<Command<M>>,
        handed: UnboundedReceiver<Handed>,
        on_change: OnChange,
    ) -> Running;
}

/// Die echten Türen.
struct Real {
    home: PathBuf,
}

impl Entrance<AnyModel> for Real {
    fn ready(&self, door: Door) -> Result<(), String> {
        match door {
            Door::Discord => discord_token().map(|_| ()),
            Door::Matrix => matrix_access().map(|_| ()),
        }
    }

    fn run(
        &self,
        door: Door,
        first: Vec<Loaded>,
        lenient: bool,
        commands: UnboundedReceiver<Command<AnyModel>>,
        handed: UnboundedReceiver<Handed>,
        on_change: OnChange,
    ) -> Running {
        match door {
            Door::Discord => Box::pin(async move {
                let token = discord_token()?;
                rusty_fish_discord::door::run(
                    token.expose(),
                    first,
                    lenient,
                    commands,
                    handed,
                    on_change,
                )
                .await
            }),
            Door::Matrix => {
                let store = matrix_store(&self.home);
                Box::pin(async move {
                    let access = matrix_access()?;
                    rusty_fish_matrix::door::run(
                        &access, &store, first, lenient, commands, handed, on_change,
                    )
                    .await
                })
            }
        }
    }
}

/// Der Bot-Token für Discord aus der Schlüsselablage.
fn discord_token() -> Result<Secret, String> {
    keys::load(BOT)?.ok_or_else(|| {
        format!("{} not stored; run `rusty-fish key set {BOT}` first", secrets::label(BOT))
    })
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

/// Was dem laufenden Fisch zustößt.
enum Event {
    /// Ihm wird etwas gesagt, von der Maske oder im Terminal.
    Said(Input),
    /// Eine Tür meldet, welche Profile sie jetzt bedient.
    Serving(Door, Vec<String>),
    /// Eine Tür ist zu.
    Ended(Door, Result<(), String>),
    /// Eine Tür hat ein Profil abgelehnt, das auch an einer anderen steht.
    Refused(Refused),
}

/// Eine offene Tür: die Leitung zu ihr, und was sie gerade bedient.
struct Open<M> {
    door: Door,
    to: UnboundedSender<Command<M>>,
    serving: Vec<String>,
}

/// Der laufende Fisch: seine offenen Türen und was zwischen ihnen liegt.
struct Fish<M, E> {
    entrance: E,
    events: UnboundedSender<Event>,
    doors: Vec<Open<M>>,
    /// Die Leitungen von Tür zu Tür, für Profile an mehr als einer.
    lines: Arc<Lines>,
    /// Die laufenden Profile, die an mehr als einer Tür stehen, mit ihren
    /// Türen. Schließt eine davon, gehen sie auch an den anderen.
    mixed: Vec<(Option<String>, Vec<Door>)>,
}

/// Ob ein Profil dazukommt oder ein laufendes ersetzt.
#[derive(Clone, Copy, PartialEq, Eq)]
enum How {
    Start,
    Restart,
}

/// Lässt den Fisch laufen, bis er angehalten wird oder seine letzte Tür zu ist.
///
/// - `first`: die Profile, mit denen er beginnt, jedes für seine Türen.
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
    let note = move |names: &[String]| {
        if let Some(held) = &held
            && let Err(why) = held.set_profiles(names)
        {
            eprintln!("rusty-fish: warning: {why}");
        }
    };
    serve(Real { home }, first, lenient, load, note, stop::listen()).await
}

/// Der Lauf selbst, mit den Türen, die `entrance` öffnet.
async fn serve<M, E>(
    entrance: E,
    first: Vec<Profile<M>>,
    lenient: bool,
    load: impl Fn(&str) -> Result<Profile<M>, String>,
    note: impl Fn(&[String]),
    mut said: UnboundedReceiver<Input>,
) -> Result<(), String>
where
    M: Clone + Send + 'static,
    E: Entrance<M>,
{
    let (events, mut happened) = unbounded_channel::<Event>();
    let lines = Lines::new();
    let mut refused = lines.listen_for_refusals();
    let mut fish =
        Fish { entrance, events: events.clone(), doors: Vec::new(), lines, mixed: Vec::new() };

    // Erst alles prüfen, dann verbinden: fehlt ein Zugang, scheitert der
    // Fisch, bevor er sich irgendwo zeigt. Ein Profil an zwei Türen braucht
    // beide Zugänge.
    let mut waiting: Vec<(Door, Vec<Profile<M>>)> = Vec::new();
    for profile in first {
        let label = label_of(profile.name.as_deref());
        match fish.split(profile) {
            Ok(parts) => {
                for (door, part) in parts {
                    match waiting.iter_mut().find(|(d, _)| *d == door) {
                        Some((_, list)) => list.push(part),
                        None => waiting.push((door, vec![part])),
                    }
                }
            }
            Err(why) if lenient => eprintln!("rusty-fish: {label}cannot start: {why}"),
            Err(why) => return Err(why),
        }
    }
    for (door, list) in waiting {
        fish.open(door, list, lenient);
    }
    if fish.doors.is_empty() {
        return Err("there is no profile that can be started".to_owned());
    }

    // Was dem Fisch gesagt wird und was seine Türen ablehnen, kommt in
    // dieselbe Reihe wie das, was sie sonst melden.
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
    tokio::spawn({
        let events = events.clone();
        async move {
            while let Some(refusal) = refused.recv().await {
                if events.send(Event::Refused(refusal)).is_err() {
                    break;
                }
            }
        }
    });
    drop(events);

    let mut stopping = false;
    let mut failed: Option<String> = None;
    while let Some(event) = happened.recv().await {
        match event {
            Event::Serving(door, names) => {
                if let Some(open) = fish.doors.iter_mut().find(|open| open.door == door) {
                    open.serving = names;
                }
                note(&fish.serving());
            }
            Event::Ended(door, ended) => {
                fish.doors.retain(|open| open.door != door);
                fish.lines.close(door);
                if let Err(why) = &ended {
                    if !fish.doors.is_empty() {
                        eprintln!("rusty-fish: the {} door is closed: {why}", door.label());
                    }
                    failed.get_or_insert(why.clone());
                    // Streng: eine gescheiterte Tür hält den ganzen Fisch an.
                    if !lenient && !stopping {
                        stopping = true;
                        fish.tell_all(|| Command::StopAll);
                    }
                }
                // Was auch an dieser Tür stand, geht an den anderen mit.
                if !stopping {
                    fish.stop_orphans(door, ended.as_ref().err());
                }
                note(&fish.serving());
                if fish.doors.is_empty() {
                    break;
                }
            }
            Event::Refused(refusal) => {
                // Die Tür hat ihren Grund schon gesagt.
                let name = Some(refusal.profile.clone());
                if let Some(doors) = fish.forget(&name) {
                    let why = format!(
                        "the {} door cannot serve it ({}), and a profile with tables at two \
                         doors runs at both or not at all",
                        refusal.door.label(),
                        refusal.why
                    );
                    fish.halt(&refusal.profile, &doors, refusal.door, &why);
                }
            }
            Event::Said(Input::StopAll) => {
                stopping = true;
                eprintln!("rusty-fish: stopping");
                fish.tell_all(|| Command::StopAll);
            }
            Event::Said(Input::Stop(name)) => {
                fish.forget(&Some(name.clone()));
                // An die Türen, die es bedienen. Weiß es keine, sollen es
                // alle hören: jede sagt dann, dass es bei ihr nicht lief.
                if !fish.tell_serving(&name) {
                    fish.tell_all(|| Command::Stop(name.clone()));
                }
            }
            Event::Said(Input::Start(name)) => {
                let started = load(&name).and_then(|profile| fish.place(profile, How::Start));
                if let Err(why) = started {
                    eprintln!("rusty-fish: [{name}] cannot start: {why}");
                }
            }
            // Lässt sich das geänderte Profil nicht laden, oder fehlt einer
            // seiner Türen der Zugang, läuft das bisherige weiter: lieber der
            // alte Stand als keiner.
            Event::Said(Input::Restart(name)) => {
                let restarted = load(&name).and_then(|profile| fish.place(profile, How::Restart));
                if let Err(why) = restarted {
                    eprintln!("rusty-fish: [{name}] cannot restart: {why}");
                }
            }
        }
    }

    match failed {
        // Nach einem gewollten Anhalten ist das Ende einer Tür kein Fehler.
        Some(why) if !stopping || !lenient => Err(why),
        _ => Ok(()),
    }
}

impl<M, E> Fish<M, E>
where
    M: Clone + Send + 'static,
    E: Entrance<M>,
{
    /// Macht aus einem Profil, was jede seiner Türen bekommt. Steht es an
    /// einer, ist es das Profil selbst. Steht es an mehreren, bekommt jede
    /// dasselbe Profil mit demselben Haushalt.
    ///
    /// Scheitert, bevor irgendeine Tür etwas hört, wenn eine der Türen nicht
    /// geöffnet werden kann: die Meldung sagt, was fehlt.
    fn split(&mut self, profile: Profile<M>) -> Result<Vec<(Door, Profile<M>)>, String> {
        let doors = profile.config.doors.clone();
        for door in &doors {
            if self.doors.iter().any(|open| open.door == *door) {
                continue;
            }
            self.entrance.ready(*door).map_err(|why| {
                if doors.len() > 1 {
                    format!(
                        "{why} (this profile has tables at {}; it runs at all of them or not at \
                         all)",
                        doors.iter().map(|d| d.label()).collect::<Vec<_>>().join(" and ")
                    )
                } else {
                    why
                }
            })?;
        }
        // Ob es bisher an mehreren Türen lief, gilt mit diesem Stand neu.
        if doors.len() < 2 {
            self.forget(&profile.name);
            return Ok(vec![(profile.config.door, profile)]);
        }
        let house = Household::open(&profile.config, &profile.state_path)?;
        let shared = Shared { house: Arc::new(Mutex::new(house)), lines: self.lines.clone() };
        self.forget(&profile.name);
        self.mixed.push((profile.name.clone(), doors.clone()));
        Ok(doors
            .into_iter()
            .map(|door| {
                let part = Profile {
                    name: profile.name.clone(),
                    config: profile.config.clone(),
                    state_path: profile.state_path.clone(),
                    model: profile.model.clone(),
                    service: profile.service.clone(),
                    shared: Some(shared.clone()),
                };
                (door, part)
            })
            .collect())
    }

    /// Öffnet eine Tür mit ihren ersten Profilen. Alles Weitere meldet die
    /// Tür selbst als Ereignis.
    fn open(&mut self, door: Door, first: Vec<Profile<M>>, lenient: bool) {
        let (to, commands) = unbounded_channel();
        let handed = self.lines.open(door);
        let on_change: OnChange = {
            let events = self.events.clone();
            Box::new(move |names: &[String]| {
                let _ = events.send(Event::Serving(door, names.to_vec()));
            })
        };
        let running = self.entrance.run(door, first, lenient, commands, handed, on_change);
        let events = self.events.clone();
        tokio::spawn(async move {
            let ended = running.await;
            let _ = events.send(Event::Ended(door, ended));
        });
        self.doors.push(Open { door, to, serving: Vec::new() });
    }

    /// Gibt ein Profil im Lauf an seine Türen: neu dazu, oder an Stelle des
    /// laufenden gleichen Namens.
    fn place(&mut self, profile: Profile<M>, how: How) -> Result<(), String> {
        let name = profile.name.clone();
        let parts = self.split(profile)?;
        // Steht das Profil nach der Änderung nicht mehr an einer Tür, geht es
        // dort.
        if let (How::Restart, Some(name)) = (how, name.as_deref()) {
            for open in &self.doors {
                let stays = parts.iter().any(|(door, _)| *door == open.door);
                if !stays && open.serving.iter().any(|served| served == name) {
                    let _ = open.to.send(Command::Stop(name.to_owned()));
                }
            }
        }
        for (door, part) in parts {
            match self.doors.iter().find(|open| open.door == door) {
                Some(open) => {
                    let part = Box::new(part);
                    let _ = open.to.send(match how {
                        How::Start => Command::Start(part),
                        How::Restart => Command::Restart(part),
                    });
                }
                None => self.open(door, vec![part], false),
            }
        }
        Ok(())
    }

    /// Dieses Profil gilt nicht mehr als eines, das an mehreren Türen läuft.
    /// Liefert die Türen, an denen es stand, wenn es eines war.
    fn forget(&mut self, name: &Option<String>) -> Option<Vec<Door>> {
        let at = self.mixed.iter().position(|(mixed, _)| mixed == name)?;
        Some(self.mixed.remove(at).1)
    }

    /// Eine Tür ist zu. Was auch an ihr stand, wird an den anderen angehalten.
    fn stop_orphans(&mut self, closed: Door, failed: Option<&String>) {
        let why = format!(
            "the {} door is closed{}, and a profile with tables at two doors runs at both or not \
             at all",
            closed.label(),
            failed.map(|why| format!(" ({why})")).unwrap_or_default()
        );
        let orphans: Vec<Option<String>> = self
            .mixed
            .iter()
            .filter(|(_, doors)| doors.contains(&closed))
            .map(|(name, _)| name.clone())
            .collect();
        for name in orphans {
            let doors = self.forget(&name).unwrap_or_default();
            match &name {
                Some(name) => self.halt(name, &doors, closed, &why),
                // Ohne Namen lässt sich ein einzelnes Profil nicht anhalten;
                // es ist dann das einzige, und der Fisch hält an.
                None => {
                    self.tell_all(|| Command::StopAll);
                    if !self.doors.is_empty() {
                        eprintln!("rusty-fish: stopped: {why}");
                    }
                }
            }
        }
    }

    /// Sagt „stop NAME" den Türen, die dieses Profil bedienen. Liefert, ob
    /// es eine gehört hat.
    fn tell_serving(&self, name: &str) -> bool {
        let mut told = false;
        for open in &self.doors {
            if open.serving.iter().any(|served| served == name) {
                let _ = open.to.send(Command::Stop(name.to_owned()));
                told = true;
            }
        }
        told
    }

    /// Hält ein Profil an den Türen an, an denen es stand, außer der einen,
    /// die es nicht mehr bedient. Nicht danach, was die Türen als bedient
    /// gemeldet haben: eine Meldung kann noch unterwegs sein. Den Grund sagt
    /// die Tür selbst, als letzte Zeile zu diesem Profil; so steht er in der
    /// Maske an seiner Karte und nicht vor einem „serving", das ihn wegwischt.
    fn halt(&self, name: &str, doors: &[Door], except: Door, why: &str) {
        for open in &self.doors {
            if open.door != except && doors.contains(&open.door) {
                let _ = open.to.send(Command::Halt(name.to_owned(), why.to_owned()));
            }
        }
    }

    /// Sagt allen offenen Türen dasselbe.
    fn tell_all(&self, command: impl Fn() -> Command<M>) {
        for open in &self.doors {
            let _ = open.to.send(command());
        }
    }

    /// Welche Profile der Fisch jetzt bedient, über alle Türen hinweg. Eines
    /// an zwei Türen steht einmal da.
    fn serving(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for name in self.doors.iter().flat_map(|open| open.serving.iter()) {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        names
    }
}

/// Ob gerade ein Fisch läuft. Die Ablage der Geräteschlüssel darf dann
/// niemand anderes anfassen.
pub fn fish_is_running(home: &Path) -> bool {
    crate::lock::running(&home.join("profiles"), profiles::BOT).is_some()
}

#[cfg(test)]
mod tests {
    //! Der Lauf mit Attrappen an Stelle der Türen: kein Netz, kein Konto,
    //! keine Schlüsselablage. Geprüft wird, was das Programm den Türen sagt.

    use super::*;
    use rusty_fish_house::config::Config;
    use std::collections::HashMap;
    use std::sync::Mutex as Lock;
    use std::time::Duration;
    use tokio::sync::Notify;

    /// Ein Gespräch an zwei Türen.
    const TEA: &str = r#"
model = "m"
host = "nemo"

[[room]]
  [[room.seat]]
  discord = "111"
  lang = "German"
  owner = "nemo"

  [[room.seat]]
  matrix = "!english:localhost"
  lang = "English"

[[person]]
name = "nemo"
discord = "100"
"#;

    /// Dasselbe Profil, nachdem der Gastgeber den Matrix-Tisch durch einen
    /// Discord-Kanal ersetzt hat.
    const TEA_AT_DISCORD: &str = r#"
model = "m"
host = "100"

[[room]]
  [[room.seat]]
  discord = "111"
  lang = "German"

  [[room.seat]]
  discord = "112"
  lang = "English"
"#;

    const AT_DISCORD: &str = r#"
model = "m"
host = "100"

[[room]]
  [[room.seat]]
  discord = "211"
  lang = "German"

  [[room.seat]]
  discord = "212"
  lang = "English"
"#;

    const AT_MATRIX: &str = r#"
model = "m"
host = "@nemo:localhost"

[[room]]
  [[room.seat]]
  matrix = "!a:localhost"
  lang = "German"

  [[room.seat]]
  matrix = "!b:localhost"
  lang = "English"
"#;

    /// Attrappen an Stelle der Türen. Sie merken sich, was ihnen gesagt wird.
    #[derive(Clone, Default)]
    struct Fakes {
        log: Arc<Lock<Vec<String>>>,
        /// Türen ohne Zugang.
        missing: Arc<Lock<Vec<Door>>>,
        /// Profile, die eine Tür ablehnt.
        refusing: Arc<Lock<Vec<(Door, String)>>>,
        /// Die Haushalte, die den Türen mitgegeben wurden.
        houses: Arc<Lock<Vec<(Door, String, Arc<Mutex<Household>>)>>>,
        /// Lässt eine Tür scheitern, als wäre ihre Verbindung abgerissen.
        breaks: Arc<Lock<HashMap<Door, Arc<Notify>>>>,
    }

    impl Fakes {
        fn said(&self, line: String) {
            self.log.lock().unwrap().push(line);
        }

        fn log(&self) -> Vec<String> {
            self.log.lock().unwrap().clone()
        }

        fn breaker(&self, door: Door) -> Arc<Notify> {
            self.breaks.lock().unwrap().entry(door).or_default().clone()
        }

        /// Nimmt ein Profil auf oder lehnt es ab, wie eine echte Tür.
        fn take(&self, door: Door, how: &str, profile: Profile<()>, serving: &mut Vec<String>) {
            let name = profile.name.clone().unwrap_or_default();
            if self.refusing.lock().unwrap().contains(&(door, name.clone())) {
                self.said(format!("{} refuses {name}", door.label()));
                if let Some(shared) = &profile.shared {
                    shared.lines.refuse(door, &name, "refused by the test");
                }
                return;
            }
            let shared = match profile.shared {
                Some(shared) => {
                    self.houses.lock().unwrap().push((door, name.clone(), shared.house));
                    "shared"
                }
                None => "alone",
            };
            self.said(format!("{} {how} {name} {shared}", door.label()));
            serving.retain(|served| *served != name);
            serving.push(name);
        }
    }

    impl Entrance<()> for Fakes {
        fn ready(&self, door: Door) -> Result<(), String> {
            if self.missing.lock().unwrap().contains(&door) {
                Err(format!("no access to {}", door.label()))
            } else {
                Ok(())
            }
        }

        fn run(
            &self,
            door: Door,
            first: Vec<Profile<()>>,
            _lenient: bool,
            mut commands: UnboundedReceiver<Command<()>>,
            _handed: UnboundedReceiver<Handed>,
            on_change: OnChange,
        ) -> Running {
            let me = self.clone();
            let broken = self.breaker(door);
            Box::pin(async move {
                let mut serving = Vec::new();
                for profile in first {
                    me.take(door, "starts", profile, &mut serving);
                }
                on_change(&serving);
                if serving.is_empty() {
                    return Err("no profile could be started".to_owned());
                }
                loop {
                    let command = tokio::select! {
                        command = commands.recv() => command,
                        () = broken.notified() => {
                            me.said(format!("{} breaks", door.label()));
                            return Err(format!("the {} connection ended", door.label()));
                        }
                    };
                    match command {
                        Some(Command::Start(profile)) => {
                            me.take(door, "starts", *profile, &mut serving);
                        }
                        Some(Command::Restart(profile)) => {
                            me.take(door, "restarts", *profile, &mut serving);
                        }
                        Some(Command::Halt(name, why)) => {
                            me.said(format!("{} halts {name}: {why}", door.label()));
                            me.said(format!("{} stops {name}", door.label()));
                            serving.retain(|served| *served != name);
                            if serving.is_empty() {
                                break;
                            }
                        }
                        Some(Command::Stop(name)) => {
                            me.said(format!("{} stops {name}", door.label()));
                            serving.retain(|served| *served != name);
                            if serving.is_empty() {
                                break;
                            }
                        }
                        Some(Command::StopAll) | None => break,
                    }
                    on_change(&serving);
                }
                me.said(format!("{} closes", door.label()));
                Ok(())
            })
        }
    }

    /// Ein laufender Fisch im Test: was ihm gesagt wird, was er als bedient
    /// notiert, und wie er endet.
    struct Run {
        fakes: Fakes,
        say: UnboundedSender<Input>,
        noted: Arc<Lock<Vec<String>>>,
        files: Arc<Lock<HashMap<String, String>>>,
        ended: tokio::task::JoinHandle<Result<(), String>>,
    }

    fn folder(test: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rusty-fish-doors-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn profile(dir: &Path, name: &str, text: &str) -> Result<Profile<()>, String> {
        Ok(Profile {
            name: Some(name.to_owned()),
            config: Config::parse(text)?,
            state_path: dir.join(format!("{name}.state.json")),
            model: (),
            service: "ai.example".to_owned(),
            shared: None,
        })
    }

    fn start(test: &str, fakes: Fakes, first: &[(&str, &str)], lenient: bool) -> Run {
        let dir = folder(test);
        let files: Arc<Lock<HashMap<String, String>>> = Arc::default();
        for (name, text) in first {
            files.lock().unwrap().insert((*name).to_owned(), (*text).to_owned());
        }
        let first: Vec<Profile<()>> =
            first.iter().map(|(name, text)| profile(&dir, name, text).unwrap()).collect();
        let (say, said) = unbounded_channel();
        let noted: Arc<Lock<Vec<String>>> = Arc::default();
        let ended = tokio::spawn({
            let (fakes, files, noted) = (fakes.clone(), files.clone(), noted.clone());
            async move {
                let load = move |name: &str| {
                    let text = files.lock().unwrap().get(name).cloned();
                    profile(&dir, name, &text.ok_or("there is no such profile")?)
                };
                let note = move |names: &[String]| *noted.lock().unwrap() = names.to_vec();
                serve(fakes, first, lenient, load, note, said).await
            }
        });
        Run { fakes, say, noted, files, ended }
    }

    impl Run {
        /// Wartet, bis die Türen dies alles gehört haben, in beliebiger Folge.
        async fn until(&self, lines: &[&str]) {
            for _ in 0..500 {
                let log = self.fakes.log();
                if lines.iter().all(|line| log.iter().any(|said| said == line)) {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("expected {lines:?} among {:#?}", self.fakes.log());
        }

        async fn until_noted(&self, names: &[&str]) {
            for _ in 0..500 {
                if *self.noted.lock().unwrap() == names {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("expected {names:?} to be noted, got {:?}", self.noted.lock().unwrap());
        }

        async fn end(self) -> Result<(), String> {
            tokio::time::timeout(Duration::from_secs(5), self.ended)
                .await
                .expect("the fish ends when told")
                .unwrap()
        }
    }

    #[tokio::test]
    async fn a_profile_at_one_door_goes_to_that_door_alone_as_before() {
        let run = start("one", Fakes::default(), &[("plain", AT_DISCORD)], false);
        run.until(&["Discord starts plain alone"]).await;
        run.until_noted(&["plain"]).await;
        // Ein zweites kommt im Lauf dazu und öffnet seine eigene Tür.
        run.files.lock().unwrap().insert("rooms".to_owned(), AT_MATRIX.to_owned());
        run.say.send(Input::Start("rooms".to_owned())).unwrap();
        run.until(&["Matrix starts rooms alone"]).await;
        run.until_noted(&["plain", "rooms"]).await;
        assert!(run.fakes.houses.lock().unwrap().is_empty(), "no door was handed a household");
        run.say.send(Input::Stop("rooms".to_owned())).unwrap();
        run.until(&["Matrix stops rooms", "Matrix closes"]).await;
        assert!(!run.fakes.log().contains(&"Discord stops rooms".to_owned()));
        run.say.send(Input::StopAll).unwrap();
        assert_eq!(run.end().await, Ok(()));
    }

    #[tokio::test]
    async fn a_profile_at_two_doors_goes_to_both_with_one_household() {
        let run = start("both", Fakes::default(), &[("tea", TEA)], false);
        run.until(&["Discord starts tea shared", "Matrix starts tea shared"]).await;
        {
            let houses = run.fakes.houses.lock().unwrap();
            assert_eq!(houses.len(), 2);
            assert!(Arc::ptr_eq(&houses[0].2, &houses[1].2), "both doors hold the same household");
        }
        // In der Notiz für die Maske steht es einmal.
        run.until_noted(&["tea"]).await;

        // Neu laden kommt an beiden an, mit einem frischen gemeinsamen Haushalt.
        run.say.send(Input::Restart("tea".to_owned())).unwrap();
        run.until(&["Discord restarts tea shared", "Matrix restarts tea shared"]).await;
        {
            let houses = run.fakes.houses.lock().unwrap();
            assert_eq!(houses.len(), 4);
            assert!(Arc::ptr_eq(&houses[2].2, &houses[3].2));
            assert!(!Arc::ptr_eq(&houses[0].2, &houses[2].2), "reloading opens the state anew");
        }

        // Anhalten auch.
        run.say.send(Input::Stop("tea".to_owned())).unwrap();
        run.until(&["Discord stops tea", "Matrix stops tea"]).await;
        assert_eq!(run.end().await, Ok(()));
    }

    #[tokio::test]
    async fn without_the_access_to_one_door_the_profile_does_not_start_at_the_other() {
        let fakes = Fakes::default();
        fakes.missing.lock().unwrap().push(Door::Matrix);

        // Streng: der Fisch startet nicht und sagt, was fehlt.
        let run = start("access-strict", fakes.clone(), &[("tea", TEA)], false);
        let why = run.end().await.unwrap_err();
        assert!(why.contains("no access to Matrix"), "got {why}");
        assert!(why.contains("tables at Discord and Matrix"), "got {why}");
        assert!(fakes.log().is_empty(), "no door heard of it: {:?}", fakes.log());

        // Nachsichtig: das Profil bleibt ganz weg, die anderen laufen.
        let run = start("access-lenient", fakes.clone(), &[("tea", TEA), ("plain", AT_DISCORD)], true);
        run.until(&["Discord starts plain alone"]).await;
        run.until_noted(&["plain"]).await;
        // Auch im Lauf: kein halber Start.
        run.say.send(Input::Start("tea".to_owned())).unwrap();
        run.say.send(Input::StopAll).unwrap();
        assert_eq!(run.end().await, Ok(()));
        assert!(!fakes.log().iter().any(|line| line.contains("tea")), "got {:?}", fakes.log());
    }

    #[tokio::test]
    async fn refused_at_one_door_the_profile_is_stopped_at_the_other() {
        let fakes = Fakes::default();
        fakes.refusing.lock().unwrap().push((Door::Matrix, "tea".to_owned()));
        let first = [("plain", AT_DISCORD), ("rooms", AT_MATRIX)];
        let run = start("refused", fakes, &first, true);
        run.until(&["Discord starts plain alone", "Matrix starts rooms alone"]).await;
        run.files.lock().unwrap().insert("tea".to_owned(), TEA.to_owned());
        run.say.send(Input::Start("tea".to_owned())).unwrap();
        run.until(&["Discord starts tea shared", "Matrix refuses tea", "Discord stops tea"]).await;
        // Den Grund sagt die Tür, die es anhält: so steht er an der Karte.
        run.until(&[concat!(
            "Discord halts tea: the Matrix door cannot serve it (refused by the test), ",
            "and a profile with tables at two doors runs at both or not at all"
        )])
            .await;
        // Die anderen Profile laufen weiter.
        run.until_noted(&["plain", "rooms"]).await;
        run.say.send(Input::StopAll).unwrap();
        assert_eq!(run.end().await, Ok(()));
    }

    #[tokio::test]
    async fn when_one_door_breaks_the_profile_is_stopped_at_the_other() {
        let fakes = Fakes::default();
        let run = start("breaks", fakes.clone(), &[("tea", TEA), ("plain", AT_DISCORD)], true);
        run.until(&["Discord starts tea shared", "Matrix starts tea shared"]).await;
        run.until_noted(&["tea", "plain"]).await;
        fakes.breaker(Door::Matrix).notify_one();
        run.until(&["Matrix breaks", "Discord stops tea"]).await;
        run.until(&[concat!(
            "Discord halts tea: the Matrix door is closed (the Matrix connection ended), ",
            "and a profile with tables at two doors runs at both or not at all"
        )])
            .await;
        // Was nur an der anderen Tür steht, läuft weiter.
        run.until_noted(&["plain"]).await;
        assert!(!fakes.log().contains(&"Discord stops plain".to_owned()));
        run.say.send(Input::StopAll).unwrap();
        // Nach einem gewollten Anhalten ist die gescheiterte Tür kein Fehler.
        assert_eq!(run.end().await, Ok(()));
    }

    #[tokio::test]
    async fn a_reloaded_profile_can_leave_a_door() {
        let first = [("tea", TEA), ("rooms", AT_MATRIX)];
        let run = start("leaves", Fakes::default(), &first, true);
        run.until(&["Discord starts tea shared", "Matrix starts tea shared"]).await;
        run.until_noted(&["tea", "rooms"]).await;
        // Der Gastgeber ersetzt den Matrix-Tisch durch einen Discord-Kanal.
        run.files.lock().unwrap().insert("tea".to_owned(), TEA_AT_DISCORD.to_owned());
        run.say.send(Input::Restart("tea".to_owned())).unwrap();
        run.until(&["Matrix stops tea", "Discord restarts tea alone"]).await;
        // Bricht danach die Matrix-Tür, betrifft es dieses Profil nicht mehr.
        run.fakes.breaker(Door::Matrix).notify_one();
        run.until(&["Matrix breaks"]).await;
        run.until_noted(&["tea"]).await;
        assert!(!run.fakes.log().contains(&"Discord stops tea".to_owned()));
        run.say.send(Input::StopAll).unwrap();
        assert_eq!(run.end().await, Ok(()));
    }
}
