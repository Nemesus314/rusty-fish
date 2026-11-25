//! Rusty Fish, das Programm: Schlüssel verwalten, einen Text zur Probe
//! übersetzen, Profile verwalten, den Fisch an Matrix anmelden, die Türen
//! öffnen (Discord und Matrix, siehe `doors`).
//!
//! Eine Oberfläche spricht nur über diese Befehle mit dem Programm. Wo sie
//! etwas weiterverarbeitet, gibt es `--json`; Fehler stehen als Text auf der
//! Fehlerausgabe, und das Programm endet dann mit Code 1.

mod awake;
mod botlink;
mod doors;
mod envelope;
mod keys;
mod lock;
mod profiles;
mod secrets;
mod stop;

use clap::{Parser, Subcommand, ValueEnum};
use profiles::Folder;
use rusty_fish_core::{
    Formality, Gender, Model, Outcome, Request, Secret, build_prompt, read_reply, translate,
};
use rusty_fish_house::config::Config;
use rusty_fish_house::served::Profile as DoorProfile;
use rusty_fish_providers::{AnyModel, Provider, catalog, connect};
use serde_json::json;
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser)]
#[command(name = "rusty-fish", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage the secrets in this system's credential store
    Key {
        #[command(subcommand)]
        action: KeyAction,
    },
    /// List the AI services the fish can talk to
    Providers {
        /// Print the list as JSON
        #[arg(long)]
        json: bool,
    },
    /// List the models your key can use
    Models {
        /// The AI service; see `rusty-fish providers`
        #[arg(long, default_value = "openai")]
        provider: String,
        /// An address of your own, in place of the address of the service
        #[arg(long)]
        base_url: Option<String>,
        /// Print the list as JSON
        #[arg(long)]
        json: bool,
    },
    /// Translate one text, to try the fish without any chat
    Translate {
        /// Language of the reader, for example Russian
        #[arg(long)]
        to: String,
        /// Language that stays untranslated; may be given several times
        #[arg(long)]
        keep: Vec<String>,
        /// Model that translates; see `rusty-fish models`
        #[arg(long)]
        model: String,
        /// The AI service; see `rusty-fish providers`
        #[arg(long, default_value = "openai")]
        provider: String,
        /// An address of your own, in place of the address of the service
        #[arg(long)]
        base_url: Option<String>,
        /// Grammatical gender of the writer, for languages that mark it
        #[arg(long, value_enum)]
        writer: Option<Who>,
        /// Grammatical gender of the reader, for languages that mark it
        #[arg(long, value_enum)]
        reader: Option<Who>,
        /// Address the reader informally, the way friends do
        #[arg(long)]
        informal: bool,
        /// How the reader is addressed, for languages that tell friends from
        /// elders; without it the form of address is unknown
        #[arg(long, value_enum)]
        formality: Option<HowFormal>,
        /// For measuring only: one more sentence for the AI, after the fish's
        /// own instructions. So a hint can be tried with a real model before
        /// it is built into a profile.
        #[arg(long, hide = true)]
        hint: Option<String>,
        /// For measuring only: say on the error output what the call used.
        #[arg(long, hide = true)]
        tokens: bool,
        /// The text to translate
        text: String,
    },
    /// List the languages the fish suggests when a profile is filled in
    Languages {
        /// Print the list as JSON
        #[arg(long)]
        json: bool,
    },
    /// Manage saved profiles: settings you can keep, change and start
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Manage the fish's own Matrix account
    Matrix {
        #[command(subcommand)]
        action: MatrixAction,
    },
    /// Move or back up everything at once: all profiles with the bot token
    /// and the API keys in one file, locked with a password
    Envelope {
        #[command(subcommand)]
        action: EnvelopeAction,
    },
    /// Open the doors (Discord, Matrix) and translate between the configured channels
    Run {
        /// Settings file; see rusty-fish.example.toml [default: rusty-fish.toml]
        #[arg(long, conflicts_with = "profile")]
        config: Option<PathBuf>,
        /// Name of a saved profile; see `rusty-fish profile list`. Give it
        /// several times to serve several profiles at once. While the fish
        /// runs, the lines `start NAME`, `stop NAME` and `stop` on its
        /// standard input add a profile, take one away, or stop it.
        #[arg(long)]
        profile: Vec<String>,
        /// Serve every saved profile. A profile that cannot be started says
        /// why and is left out; the others run. The line `restart NAME` on
        /// the standard input loads a profile again after it was changed.
        #[arg(long, conflicts_with_all = ["profile", "config"])]
        all: bool,
    },
    /// Print a description of this program for an AI assistant to explain the setup
    Guide,
    /// Print the link that brings your bot onto a Discord server. Send it to
    /// whoever owns the server; it contains the id of the bot and no secret
    Invite,
}

/// Die Beschreibung des Programms, wie sie im Repository steht.
///
/// Nemesus am 20.11.2026: „Wir sollten noch einen Text mit einer genauen
/// Beschreibung der Anwendung hinterlegen. [...] Auf Wunsch kann dann [ein
/// Gast] ihre KI ihres Vertrauens damit füttern". Der Text ist ins Programm
/// eingebaut, damit er zu genau diesem Stand gehört und nicht zu einem anderen.
const GUIDE: &str = include_str!("../../GUIDE.md");

fn guide() -> String {
    format!("{}\n\nThis text describes rusty-fish {}.\n", GUIDE.trim_end(), env!("CARGO_PKG_VERSION"))
}

#[derive(Subcommand)]
enum KeyAction {
    /// Store a secret; asks for it with hidden input unless --stdin is given
    Set {
        #[arg(value_enum, default_value_t = Which::Openai)]
        which: Which,
        /// Read the secret as one line from standard input instead of asking
        #[arg(long)]
        stdin: bool,
    },
    /// Tell whether a secret is stored, without showing it
    Status {
        #[arg(value_enum, default_value_t = Which::Openai)]
        which: Which,
        /// Print the status of every secret as JSON
        #[arg(long)]
        json: bool,
    },
    /// Delete the stored secret
    Forget {
        #[arg(value_enum, default_value_t = Which::Openai)]
        which: Which,
    },
    /// Tell where the secrets are kept, or change it. A Linux server without
    /// a desktop has no credential store; `file` keeps them in files in the
    /// fish's own folder instead, readable only by this account, not encrypted
    Store {
        #[arg(value_enum)]
        place: Option<Kept>,
    },
}

/// Der Umschlag; siehe `envelope`. Die Namen des Befehls und seiner zwei
/// Schritte hat Nemesus am 25.11.2026 gewählt.
#[derive(Subcommand)]
enum EnvelopeAction {
    /// Write all profiles, the bot token and the API keys into one file,
    /// locked with a password of at least twelve characters. Asks for the
    /// password twice with hidden input unless --stdin is given. The fish's
    /// Matrix account is never part of it. A forgotten password is final
    Write {
        /// The file to write; an existing file is never replaced
        path: PathBuf,
        /// Read the password as two equal lines from standard input instead
        /// of asking
        #[arg(long)]
        stdin: bool,
        /// Print what was written as JSON
        #[arg(long)]
        json: bool,
    },
    /// Open a file written by `envelope write` and take what it holds.
    /// Nothing that is here already is replaced: a profile or a secret of
    /// the same name is refused and named. Asks for the password with hidden
    /// input unless --stdin is given
    Read {
        /// The file to read
        path: PathBuf,
        /// Read the password as one line from standard input instead of asking
        #[arg(long)]
        stdin: bool,
        /// Print what was taken and what was refused as JSON
        #[arg(long)]
        json: bool,
    },
}

/// Wo die Geheimnisse liegen sollen; siehe `keys`.
#[derive(Clone, Copy, ValueEnum)]
enum Kept {
    /// In files in the fish's own folder; for a Linux server
    File,
    /// In this system's credential store
    System,
}

#[derive(Subcommand)]
enum MatrixAction {
    /// Log the fish in to its Matrix account; asks for the password with
    /// hidden input unless --stdin is given. The password is used once and
    /// not kept
    Login {
        /// The user id of the fish's account, like @name:server
        #[arg(long)]
        user: String,
        /// The address of the Matrix server, if it cannot be found from the user id
        #[arg(long)]
        homeserver: Option<String>,
        /// Read the password as one line from standard input instead of asking;
        /// a second line, if there is one, is the recovery key
        #[arg(long)]
        stdin: bool,
        /// Also ask for the recovery key of the account, to verify the fish's
        /// device. For an account that a person has also used with a Matrix app
        #[arg(long)]
        recovery_key: bool,
    },
    /// Tell whether the fish is logged in, and as whom
    Status {
        /// Print the status as JSON
        #[arg(long)]
        json: bool,
        /// Also ask the Matrix server whether the fish's device is verified
        /// by its account, and whether the stored access still holds
        #[arg(long)]
        ask: bool,
    },
    /// Log the fish out and empty its device store
    Logout,
}

#[derive(Subcommand)]
enum ProfileAction {
    /// Print the folder that holds the profiles
    Dir,
    /// List all profiles
    List {
        /// Print the list as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show one profile
    Show {
        name: String,
        /// Print the profile as an editable JSON document
        #[arg(long)]
        json: bool,
    },
    /// Check a profile given as a JSON document on standard input, then save it
    Save { name: String },
    /// Move a profile and its state to the trash folder; nothing is erased
    Delete { name: String },
    /// List the deleted profiles in the trash folder
    Trash {
        /// Print the list as JSON
        #[arg(long)]
        json: bool,
    },
    /// Put a deleted profile and its state back, out of the trash folder
    Restore {
        /// The entry as `profile trash` lists it, for example tea-20261120-070102
        id: String,
        /// Put it back under this name instead of its old one
        #[arg(long)]
        name: Option<String>,
    },
    /// Erase a deleted profile and its state for good; this cannot be undone
    Erase {
        /// The entry as `profile trash` lists it
        id: String,
    },
    /// Write all profiles, each with its state, into one file: to move them
    /// to another machine, or to keep a copy. The file holds no secrets
    Export {
        /// The file to write; an existing file is never replaced
        path: PathBuf,
    },
    /// Take the profiles of a file written by `profile export`. With --name:
    /// take one existing settings file, and the state next to it, as a new profile
    Import {
        /// The file to read
        path: PathBuf,
        /// Name of the new profile, for a single settings file
        #[arg(long)]
        name: Option<String>,
        /// Print what was taken and what was refused as JSON
        #[arg(long, conflicts_with = "name")]
        json: bool,
    },
}

/// Die Anrede für den Probe-Befehl, damit sich der Hinweis an die KI mit
/// einem echten Modell messen lässt.
#[derive(Clone, Copy, ValueEnum)]
enum HowFormal {
    Familiar,
    Respectful,
}

impl HowFormal {
    fn formality(self) -> Formality {
        match self {
            HowFormal::Familiar => Formality::Familiar,
            HowFormal::Respectful => Formality::Respectful,
        }
    }
}

/// Grammatisches Geschlecht für den Probe-Befehl, damit sich der Hinweis an
/// die KI mit einem echten Modell messen lässt.
#[derive(Clone, Copy, ValueEnum)]
enum Who {
    Female,
    Male,
}

impl Who {
    fn gender(self) -> Gender {
        match self {
            Who::Female => Gender::Female,
            Who::Male => Gender::Male,
        }
    }
}

/// Die Geheimnisse, die der Fisch kennt. Jedes liegt für sich in der Ablage:
/// eines je KI-Dienst und der Token des Bots. Welche es gibt, sagt
/// [`secrets::entries`]; ein Test hält diese Liste und die Befehlszeile gleich.
#[derive(Clone, Copy, ValueEnum)]
enum Which {
    /// API key for ChatGPT (OpenAI)
    Openai,
    /// API key for Grok (xAI)
    Xai,
    /// API key for Gemini (Google)
    Google,
    /// API key for Claude (Anthropic)
    Anthropic,
    /// API key for your own model, if it asks for one
    Custom,
    /// Token of your Discord bot
    Discord,
}

impl Which {
    /// Name des Eintrags in der Ablage.
    fn entry(self) -> &'static str {
        match self {
            Which::Openai => Provider::OpenAi.id(),
            Which::Xai => Provider::Xai.id(),
            Which::Google => Provider::Google.id(),
            Which::Anthropic => Provider::Anthropic.id(),
            Which::Custom => Provider::Custom.id(),
            Which::Discord => profiles::BOT,
        }
    }

    fn label(self) -> String {
        secrets::label(self.entry())
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse().command).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("rusty-fish: {why}");
            ExitCode::FAILURE
        }
    }
}

async fn run(command: Command) -> Result<(), String> {
    match command {
        Command::Key { action: KeyAction::Set { which, stdin } } => {
            // Das Geheimnis geht direkt in die Ablage des Systems. Es steht in
            // keinem Argument, keiner Datei und keiner Ausgabe. Mit --stdin
            // reicht eine Oberfläche es durch, ohne dass es je auf einer
            // Befehlszeile erscheint, wo andere Programme es lesen könnten.
            let typed = if stdin {
                let mut line = String::new();
                std::io::stdin()
                    .read_line(&mut line)
                    .map_err(|e| format!("cannot read from standard input: {e}"))?;
                // Ein vorangestelltes BOM gehört nicht zum Geheimnis.
                line.trim_start_matches('\u{feff}').to_owned()
            } else {
                rpassword::prompt_password(format!("{} (input stays hidden): ", which.label()))
                    .map_err(|e| format!("cannot read from the terminal: {e}"))?
            };
            let typed = typed.trim();
            if typed.is_empty() {
                return Err("nothing entered, nothing stored".to_owned());
            }
            keys::store(which.entry(), typed)?;
            println!("{} stored in {}.", which.label(), keys::described(&keys::place()));
            Ok(())
        }
        Command::Key { action: KeyAction::Store { place } } => {
            let home = home()?;
            match place {
                // Nie von selbst: die Dateien gelten erst, wenn jemand sie
                // verlangt hat, und die Meldung sagt, was sie nicht leisten.
                Some(Kept::File) => {
                    let dir = keys::keep_in_files(&home)?;
                    println!("From now on the secrets are kept in files in {}.", dir.display());
                    println!(
                        "Only this account can read them. They are not encrypted: whoever is root \
                         on this system, or has a backup of that folder or the disk, reads them."
                    );
                    println!(
                        "Nothing was moved there. Store each secret with `rusty-fish key set`, and \
                         log the fish in to Matrix with `rusty-fish matrix login`."
                    );
                }
                Some(Kept::System) => {
                    keys::keep_in_system(&home)?;
                    println!("From now on the secrets are kept in this system's credential store.");
                }
                None => match keys::place_in(&home) {
                    keys::Place::System => {
                        println!("The secrets are kept in this system's credential store.")
                    }
                    #[cfg(unix)]
                    keys::Place::Files(dir) => println!(
                        "The secrets are kept in files in {}, readable only by this account, not \
                         encrypted.",
                        dir.display()
                    ),
                },
            }
            Ok(())
        }
        Command::Key { action: KeyAction::Status { which, json } } => {
            if json {
                let status = secrets::status(&|entry| keys::load(entry).map(|s| s.is_some()))?;
                println!("{status}");
                return Ok(());
            }
            match keys::load(which.entry())? {
                Some(_) => println!("{}: stored.", which.label()),
                None => println!("{}: not stored.", which.label()),
            }
            Ok(())
        }
        Command::Key { action: KeyAction::Forget { which } } => {
            if keys::forget(which.entry())? {
                println!("{} deleted.", which.label());
            } else {
                println!("{}: nothing was stored.", which.label());
            }
            Ok(())
        }
        Command::Providers { json } => {
            let list = catalog();
            if json {
                println!("{}", pretty(&list));
                return Ok(());
            }
            for entry in list.as_array().into_iter().flatten() {
                let id = entry["id"].as_str().unwrap_or("?");
                let label = entry["label"].as_str().unwrap_or("?");
                let address = entry["base_url"].as_str().unwrap_or("(the address in the profile)");
                println!("{id:<10}  {label:<28}  {address}");
            }
            Ok(())
        }
        Command::Models { provider, base_url, json } => {
            let fish = service(&provider, base_url.as_deref(), "")?;
            let models = fish.models().await.map_err(|e| e.to_string())?;
            if json {
                println!("{}", json!(models));
            } else {
                for id in models {
                    println!("{id}");
                }
            }
            Ok(())
        }
        Command::Translate {
            to,
            keep,
            model,
            provider,
            base_url,
            writer,
            reader,
            informal,
            formality,
            hint,
            tokens,
            text,
        } => {
            let fish = service(&provider, base_url.as_deref(), &model)?;
            let keep: Vec<&str> = keep.iter().map(String::as_str).collect();
            let request = Request {
                speaker: "me",
                text: &text,
                target_lang: &to,
                pass_through: &keep,
                context: &[],
                writer_gender: writer.map(Who::gender),
                reader_gender: reader.map(Who::gender),
                informal,
                reader_formality: formality.map(HowFormal::formality),
            };
            // Zum Messen: derselbe Weg wie im Kern, Schritt für Schritt, mit
            // einem Satz mehr im Auftrag. Seit dem 23.11.2026, für die Frage,
            // ob ein Hinweis zur Anrede eine Übersetzung verlässlich ändert.
            let outcome = if hint.is_some() || tokens {
                let mut prompt = build_prompt(&request);
                if let Some(hint) = hint.as_deref().map(str::trim).filter(|h| !h.is_empty()) {
                    prompt.system.push(' ');
                    prompt.system.push_str(hint);
                }
                let reply = fish.complete(&prompt).await.map_err(|e| e.0)?;
                if tokens {
                    let used = reply.tokens.map_or_else(|| "?".to_owned(), |n| n.to_string());
                    eprintln!("tokens: {used}");
                }
                read_reply(&reply.text, &request).map_err(|e| e.to_string())?
            } else {
                translate(&fish, &request).await.map_err(|e| e.to_string())?
            };
            match outcome {
                Outcome::Translated(translation) => println!("{translation}"),
                Outcome::Skip => println!("(nothing to translate)"),
            }
            Ok(())
        }
        Command::Languages { json } => {
            let languages = rusty_fish_core::languages::LANGUAGES;
            if json {
                // Für die Maske: der Name, der ins Profil kommt, andere Namen
                // zum Finden, und ob die Sprache eine Höflichkeitsstufe
                // verlangt.
                let list: Vec<serde_json::Value> = languages
                    .iter()
                    .map(|language| {
                        json!({
                            "name": language.name,
                            "also": language.also,
                            "level": language.asks_for_a_level(),
                        })
                    })
                    .collect();
                println!("{}", pretty(&json!({ "languages": list })));
            } else {
                for language in languages {
                    println!("{}", language.name);
                }
            }
            Ok(())
        }
        Command::Profile { action } => profile(action),
        Command::Matrix { action } => matrix(action).await,
        Command::Envelope { action } => envelope_action(action),
        Command::Guide => {
            print!("{}", guide());
            Ok(())
        }
        Command::Invite => {
            let token = keys::load(profiles::BOT)?.ok_or_else(|| {
                format!(
                    "{} not stored; run `rusty-fish key set {}` first",
                    secrets::label(profiles::BOT),
                    profiles::BOT
                )
            })?;
            let id = botlink::bot_id(token.expose()).ok_or_else(|| {
                "cannot read the id of the bot from the stored token; take the Application ID \
                 from the Discord Developer Portal instead"
                    .to_owned()
            })?;
            println!("{}", botlink::link(&id));
            Ok(())
        }
        Command::Run { config, profile, all } => {
            // Wo die Einstellungen und ihr Zustand liegen: im Profilordner oder,
            // wie von Anfang an, in einer frei gewählten Datei mit dem Zustand
            // daneben.
            let folder = folder();
            // Erst alles prüfen, dann verbinden: fehlt etwas, scheitert der
            // Fisch, bevor er sich bei Discord zeigt.
            let mut first = Vec::new();
            // Welche Profile dieser Fisch von Anfang an bedient, mit Namen.
            let mut profile = profile;
            if all {
                // Alle gespeicherten Profile. Eines, das sich nicht laden
                // lässt, sagt warum und bleibt weg; die anderen laufen.
                //
                // Nemesus am 21.11.2026: „Stattdessen bauen wir nur einen
                // Button. An und Aus. Schaltet alle Profile ein oder aus."
                let folder = folder.clone()?;
                for name in folder.names()? {
                    match load_profile(&folder, &name) {
                        Ok(loaded) => {
                            first.push(loaded);
                            profile.push(name);
                        }
                        Err(why) => eprintln!("rusty-fish: [{name}] cannot start: {why}"),
                    }
                }
                if first.is_empty() {
                    return Err("there is no profile that can be started".to_owned());
                }
            } else if profile.is_empty() {
                // Wie von Anfang an: eine frei gewählte Datei mit dem Zustand
                // daneben.
                let path = config.unwrap_or_else(|| PathBuf::from("rusty-fish.toml"));
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
                first.push(serve(None, &text, path.with_extension("state.json"))?);
            } else {
                let folder = folder.clone()?;
                for name in &profile {
                    first.push(load_profile(&folder, name).map_err(|why| {
                        // Bei einem einzelnen Profil bleibt die Meldung, wie sie war.
                        if profile.len() == 1 { why } else { format!("profile \"{name}\": {why}") }
                    })?);
                }
            }
            // Ein Fisch je Rechner: die Sperre gilt bis zum Ende dieses Laufs,
            // auch für einen Start mit --config, denn Bot und Matrix-Konto sind
            // dieselben. Seit dem 21.11.2026 bedient dieser eine Fisch mehrere
            // Profile; welche, steht in der Notiz zur Sperre, und die Maske
            // liest sie. Die Sperre heißt weiter nach dem Bot-Eintrag, damit
            // ein älteres Programm sie achtet.
            let who =
                lock::Running { profiles: profile.clone(), pid: std::process::id(), started: now() };
            let held = match &folder {
                Ok(folder) => Some(lock::acquire(folder.dir(), profiles::BOT, &who)?),
                // Nur bei --config erreichbar; mit --profile ist der Ordner oben
                // schon Pflicht. Lieber ohne Sperre laufen als gar nicht.
                Err(why) => {
                    eprintln!("rusty-fish: warning: {why}");
                    eprintln!("rusty-fish: warning: running without the one-fish lock");
                    None
                }
            };
            // Solange der Fisch läuft, soll sich der Rechner nicht von selbst
            // schlafen legen: ein schlafender PC übersetzt nicht. Verstellt
            // wird nichts, und mit dem Ende des Programms gilt es nicht mehr.
            if awake::keep() {
                eprintln!("rusty-fish: keeping this PC from going to sleep by itself while the fish is on");
            }

            // Ein Profil, das im Lauf dazukommt oder sich ändert, wird so
            // nachgeladen. Welche Tür es bedient, sagen seine Tische.
            let load = move |name: &str| folder.clone().and_then(|f| load_profile(&f, name));
            doors::run(first, all, load, held, home()?).await
        }
    }
}

/// Der Ordner, in dem der Fisch seine Dinge ablegt.
fn home() -> Result<PathBuf, String> {
    profiles::home(&|key| std::env::var(key).ok(), profiles::this_os())
}

/// Die Befehle rund um das Matrix-Konto des Fischs.
async fn matrix(action: MatrixAction) -> Result<(), String> {
    let home = home()?;
    let store = doors::matrix_store(&home);
    match action {
        MatrixAction::Login { user, homeserver, stdin, recovery_key } => {
            // Die Ablage der Geräteschlüssel wird neu angelegt; ein laufender
            // Fisch hielte sie offen.
            if doors::fish_is_running(&home) {
                return Err("the fish is running; switch it off before logging in to Matrix".to_owned());
            }
            // Erst die Kennung prüfen, dann nach dem Passwort fragen.
            let user = rusty_fish_matrix::session::user_id(&user)?;
            // Das Passwort steht in keinem Argument, keiner Datei und keiner
            // Ausgabe, wie jedes Geheimnis hier. Es wird einmal gebraucht.
            let typed = if stdin {
                let mut line = String::new();
                std::io::stdin()
                    .read_line(&mut line)
                    .map_err(|e| format!("cannot read from standard input: {e}"))?;
                line.trim_start_matches('\u{feff}').to_owned()
            } else {
                rpassword::prompt_password(format!("Password of {user} (input stays hidden): "))
                    .map_err(|e| format!("cannot read from the terminal: {e}"))?
            };
            let password = typed.trim_end_matches(['\r', '\n']);
            if password.is_empty() {
                return Err("nothing entered, nothing done".to_owned());
            }
            // Der Wiederherstellungsschlüssel ist ein Geheimnis wie das
            // Passwort: derselbe Weg hinein, einmal gebraucht, nie aufbewahrt.
            let recovery = if stdin {
                let mut line = String::new();
                std::io::stdin()
                    .read_line(&mut line)
                    .map_err(|e| format!("cannot read from standard input: {e}"))?;
                line
            } else if recovery_key {
                rpassword::prompt_password(format!(
                    "Recovery key of {user} (input stays hidden, empty to skip): "
                ))
                .map_err(|e| format!("cannot read from the terminal: {e}"))?
            } else {
                String::new()
            };
            let logged_in = rusty_fish_matrix::session::login(
                &store,
                user.as_str(),
                password,
                homeserver.as_deref(),
                Some(recovery.trim()),
            )
            .await?;
            keys::store(secrets::MATRIX, &logged_in.access.to_secret())?;
            println!(
                "Logged in to Matrix as {} at {}.",
                logged_in.access.user_id(),
                logged_in.access.homeserver()
            );
            println!(
                "The access is in {}, the device keys are in {}.",
                keys::described(&keys::place()),
                store.display()
            );
            for note in logged_in.notes {
                println!("Note: {note}.");
            }
            Ok(())
        }
        MatrixAction::Status { json, ask } => {
            use rusty_fish_matrix::session::Device;
            let access = match keys::load(secrets::MATRIX)? {
                Some(secret) => Some(rusty_fish_matrix::session::Access::parse(secret.expose())?),
                None => None,
            };
            // Ohne --ask bleibt der Befehl ohne Netz: er sagt, was hier
            // gespeichert ist. Mit --ask sagt er dazu, was der Server zeigt.
            let device = match (&access, ask) {
                (Some(access), true) => Some(rusty_fish_matrix::session::device(access).await),
                _ => None,
            };
            if json {
                println!(
                    "{}",
                    pretty(&json!({
                        "logged_in": access.is_some(),
                        "user": access.as_ref().map(|a| a.user_id().to_string()),
                        "homeserver": access.as_ref().map(|a| a.homeserver().to_owned()),
                        "device": device.as_ref().map(|device| match device {
                            Device::Verified => "verified",
                            Device::Unverified => "unverified",
                            Device::Refused => "refused",
                            Device::Unknown(_) => "unknown",
                        }),
                        "device_note": match &device {
                            Some(Device::Unknown(why)) => Some(why.as_str()),
                            _ => None,
                        },
                    }))
                );
                return Ok(());
            }
            match access {
                Some(access) => {
                    println!("Matrix: logged in as {} at {}.", access.user_id(), access.homeserver())
                }
                None => println!("Matrix: not logged in."),
            }
            match device {
                Some(Device::Verified) => println!("Device: verified by its account."),
                Some(Device::Unverified) => println!(
                    "Device: not verified; some apps show a warning at the fish's messages."
                ),
                Some(Device::Refused) => println!(
                    "Device: the Matrix server no longer accepts the stored access; log out and \
                     log in again."
                ),
                Some(Device::Unknown(why)) => println!("Device: not known ({why})."),
                None => {}
            }
            Ok(())
        }
        MatrixAction::Logout => {
            if doors::fish_is_running(&home) {
                return Err("the fish is running; switch it off before logging out of Matrix".to_owned());
            }
            let Some(secret) = keys::load(secrets::MATRIX)? else {
                println!("Matrix: nothing to log out of.");
                return Ok(());
            };
            // Ein Zugang, der sich nicht lesen lässt, wird trotzdem vergessen.
            let told = match rusty_fish_matrix::session::Access::parse(secret.expose()) {
                Ok(access) => rusty_fish_matrix::session::logout(&access, &store).await,
                Err(_) => rusty_fish_matrix::session::clear(&store).map(|()| None),
            };
            // Der Zugang wird in jedem Fall vergessen. Bliebe er nach einem
            // Fehler stehen, hielte sich der Fisch weiter für angemeldet und
            // böte kein neues Anmelden an (so geschehen am 22.11.2026).
            keys::forget(secrets::MATRIX)?;
            match told {
                Ok(told) => {
                    println!("Logged out of Matrix. The device store is empty.");
                    if let Some(note) = told {
                        println!("Note: {note}.");
                    }
                }
                // Die Ablage ist ohne den Zugang wertlos: ihr Kennwort stand
                // darin. Die nächste Anmeldung leert sie.
                Err(why) => {
                    println!("Logged out of Matrix.");
                    println!(
                        "Note: the device store could not be emptied ({why}); the next login \
                         empties it."
                    );
                }
            }
            Ok(())
        }
    }
}

/// Die Befehle rund um gespeicherte Profile. Kein Netz, keine Geheimnisse.
fn profile(action: ProfileAction) -> Result<(), String> {
    let folder = folder()?;
    match action {
        ProfileAction::Dir => {
            println!("{}", folder.dir().display());
            Ok(())
        }
        ProfileAction::List { json } => {
            let list = folder.list()?;
            if json {
                println!("{}", pretty(&list));
                return Ok(());
            }
            let profiles = list["profiles"].as_array().cloned().unwrap_or_default();
            if profiles.is_empty() {
                println!("No profiles yet in {}", folder.dir().display());
            }
            for p in profiles {
                let name = p["name"].as_str().unwrap_or("?");
                let status = match (p["ok"].as_bool(), p["running"].as_bool()) {
                    (Some(false), _) => format!("broken: {}", p["error"].as_str().unwrap_or("?")),
                    (_, Some(true)) => "running".to_owned(),
                    _ => "ready".to_owned(),
                };
                println!("{name:<40}  {status}");
            }
            Ok(())
        }
        ProfileAction::Show { name, json } => {
            if json {
                let document = folder.show(&name)?;
                let value = serde_json::to_value(&document)
                    .map_err(|e| format!("cannot print the profile: {e}"))?;
                println!("{}", pretty(&value));
            } else {
                profiles::valid_name(&name)?;
                print!("{}", folder.read_config(&name)?);
            }
            Ok(())
        }
        ProfileAction::Save { name } => {
            let mut document = String::new();
            std::io::stdin()
                .read_to_string(&mut document)
                .map_err(|e| format!("cannot read the profile from standard input: {e}"))?;
            // Ein vorangestelltes BOM (etwa aus einer PowerShell-Pipe) ist kein
            // Teil des Dokuments. Gemessen am 20.11.2026: ohne das Abstreifen
            // lehnte das Programm so eine Eingabe als kein JSON ab.
            folder.save(&name, document.trim_start_matches('\u{feff}'))?;
            println!("Profile \"{name}\" saved.");
            Ok(())
        }
        ProfileAction::Delete { name } => {
            let moved = folder.delete(&name, &profiles::stamp(now()))?;
            println!("Profile \"{name}\" moved to {}", moved.display());
            Ok(())
        }
        ProfileAction::Trash { json } => {
            let trash = folder.trash()?;
            if json {
                println!("{}", pretty(&trash));
                return Ok(());
            }
            let deleted = trash["deleted"].as_array().cloned().unwrap_or_default();
            if deleted.is_empty() {
                println!("The trash is empty.");
            }
            for entry in deleted {
                println!(
                    "{:<60}  deleted {}",
                    entry["id"].as_str().unwrap_or("?"),
                    entry["deleted"].as_str().unwrap_or("?")
                );
            }
            Ok(())
        }
        ProfileAction::Restore { id, name } => {
            let name = folder.restore(&id, name.as_deref())?;
            println!("Profile \"{name}\" is back.");
            Ok(())
        }
        ProfileAction::Erase { id } => {
            folder.erase(&id)?;
            println!("\"{id}\" is erased.");
            Ok(())
        }
        ProfileAction::Export { path } => {
            let (document, count) = folder.export(now())?;
            profiles::write_new(&path, document.as_bytes())?;
            let word = if count == 1 { "profile" } else { "profiles" };
            println!("{count} {word} written to {}", path.display());
            println!(
                "Note: the file holds the names and ids of the people in them, readable by \
                 whoever has it. It holds no secrets and nothing of the Matrix account."
            );
            // Was der laufende Fisch von jetzt an lernt, steht nicht mehr in
            // der Datei: wer umzieht, schaltet ihn vorher aus.
            if lock::running(folder.dir(), profiles::BOT).is_some() {
                println!(
                    "Note: the fish is running here. What it learns from now on (who agreed, \
                     what was read) is not in the file; to move, switch it off first and export \
                     again."
                );
            }
            Ok(())
        }
        ProfileAction::Import { path, name: Some(name), .. } => {
            // Eine Datei mit mehreren Profilen würde so als ein kaputtes
            // Profil übernommen; lieber sagen, wie es gemeint ist.
            if std::fs::read_to_string(&path).is_ok_and(|text| profiles::is_export(&text)) {
                return Err(format!(
                    "{} holds several profiles, each under its own name; leave out `--name`",
                    path.display()
                ));
            }
            folder.import(&path, &name)?;
            println!("Profile \"{name}\" created from {}", path.display());
            Ok(())
        }
        ProfileAction::Import { path, name: None, json } => {
            let cannot = |e: std::io::Error| format!("cannot read {}: {e}", path.display());
            let size = std::fs::metadata(&path).map_err(cannot)?.len();
            if size > profiles::EXPORT_LIMIT {
                return Err(format!(
                    "{} is larger than a file of profiles can be; nothing was changed",
                    path.display()
                ));
            }
            let document = std::fs::read_to_string(&path).map_err(cannot)?;
            let imported = folder.import_all(&document).map_err(|why| {
                // Eine einzelne Einstellungsdatei ist kein Fehler der Datei:
                // ihr fehlt nur der Name.
                if why.starts_with("this is not a file") {
                    format!("{why}. For a single settings file, give `--name NAME`")
                } else {
                    why
                }
            })?;
            if json {
                println!("{}", pretty(&imported.json()));
                return Ok(());
            }
            for name in &imported.created {
                println!("Profile \"{name}\" taken.");
            }
            for (name, why) in &imported.refused {
                println!("Profile \"{name}\" refused: {why}.");
            }
            println!("{} taken, {} refused.", imported.created.len(), imported.refused.len());
            if !imported.created.is_empty() {
                println!(
                    "Note: secrets are not part of the file. Store them here with `rusty-fish \
                     key set`, and log the fish in to Matrix with `rusty-fish matrix login`."
                );
                println!(
                    "Note: one bot runs in one place only. Switch the fish off where the file \
                     came from before you switch it on here, or everything is translated twice."
                );
            }
            Ok(())
        }
    }
}

/// Eine Zeile von der Standardeingabe, ohne ihr Zeilenende und ohne ein
/// vorangestelltes BOM. Leerzeichen bleiben: sie können zu einem Kennwort
/// gehören.
fn line_from_stdin() -> Result<String, String> {
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| format!("cannot read from standard input: {e}"))?;
    Ok(line.trim_start_matches('\u{feff}').trim_end_matches(['\r', '\n']).to_owned())
}

/// Die Befehle rund um den Umschlag.
fn envelope_action(action: EnvelopeAction) -> Result<(), String> {
    use zeroize::Zeroize;
    let hidden = |prompt: &str| {
        rpassword::prompt_password(prompt).map_err(|e| format!("cannot read from the terminal: {e}"))
    };
    match action {
        EnvelopeAction::Write { path, stdin, json } => {
            // Vor der Frage nach dem Kennwort: wer es zweimal getippt hat,
            // soll nicht danach erfahren, dass der Name vergeben ist.
            if path.exists() {
                return Err(format!(
                    "{} already exists; choose another name, nothing is replaced",
                    path.display()
                ));
            }
            let folder = folder()?;
            let names = folder.names()?;
            let mut secrets = Vec::new();
            // Was man einfügt, reist mit. Der Zugang zu Matrix nie: er ist
            // ein Gerät, und dasselbe Gerät darf nicht an zwei Orten laufen.
            for entry in secrets::entries() {
                if let Some(secret) = keys::load(entry)? {
                    secrets.push((entry.to_owned(), secret.expose().to_owned()));
                }
            }
            if names.is_empty() && secrets.is_empty() {
                return Err("there is nothing to put into an envelope: no profile and no stored \
                            secret"
                    .to_owned());
            }
            // Das Kennwort steht in keinem Argument, keiner Datei und keiner
            // Ausgabe. Zweimal, auch von einer Oberfläche: ein Tippfehler in
            // dem Kennwort, das die einzige Kopie schützt, ist nicht zu
            // bemerken und nicht zu heilen.
            let (mut password, mut again) = if stdin {
                (line_from_stdin()?, line_from_stdin()?)
            } else {
                (
                    hidden("Password for the envelope, at least 12 characters (input stays hidden): ")?,
                    hidden("The same password once more: ")?,
                )
            };
            let equal = password == again;
            again.zeroize();
            if !equal {
                password.zeroize();
                return Err("the two passwords differ; nothing was written".to_owned());
            }
            let sealed = envelope::check_password(&password).and_then(|()| {
                let document = if names.is_empty() { None } else { Some(folder.export(now())?.0) };
                let body = envelope::body(document.as_deref(), &secrets);
                envelope::seal(body.as_bytes(), &password, now())
            });
            password.zeroize();
            profiles::write_new(&path, sealed?.as_bytes())?;

            let matrix_left_out = keys::load(secrets::MATRIX)?.is_some();
            let running = lock::running(folder.dir(), profiles::BOT).is_some();
            if json {
                let entries: Vec<&str> = secrets.iter().map(|(entry, _)| entry.as_str()).collect();
                println!(
                    "{}",
                    pretty(&json!({
                        "profiles": names.len(),
                        "secrets": entries,
                        "matrix_left_out": matrix_left_out,
                        "running": running,
                    }))
                );
                return Ok(());
            }
            let profiles = if names.len() == 1 { "profile" } else { "profiles" };
            let word = if secrets.len() == 1 { "secret" } else { "secrets" };
            println!(
                "{} {profiles} and {} {word} written to {}",
                names.len(),
                secrets.len(),
                path.display()
            );
            for (entry, _) in &secrets {
                println!("In it: {}.", secrets::label(entry));
            }
            println!(
                "Note: only the password opens this file, and nothing brings a forgotten one back."
            );
            if matrix_left_out {
                println!(
                    "Note: the fish's Matrix account is not in it. Log the fish in again where you \
                     read the file, with `rusty-fish matrix login`."
                );
            }
            if running {
                println!(
                    "Note: the fish is running here. What it learns from now on (who agreed, \
                     what was read) is not in the file; to move, switch it off first and write \
                     the envelope again."
                );
            }
            Ok(())
        }
        EnvelopeAction::Read { path, stdin, json } => {
            let cannot = |e: std::io::Error| format!("cannot read {}: {e}", path.display());
            let size = std::fs::metadata(&path).map_err(cannot)?.len();
            if size > envelope::LIMIT {
                return Err(format!(
                    "{} is larger than an envelope can be; nothing was changed",
                    path.display()
                ));
            }
            let text = std::fs::read_to_string(&path).map_err(cannot)?;
            // Die Datei ohne Geheimnisse ist kein Umschlag; lieber sagen,
            // womit man sie liest, als nach einem Kennwort fragen.
            if profiles::is_export(&text) {
                return Err(format!(
                    "{} is written by `rusty-fish profile export` and has no password; read it \
                     with `rusty-fish profile import`",
                    path.display()
                ));
            }
            let mut password = if stdin {
                line_from_stdin()?
            } else {
                hidden("Password of the envelope (input stays hidden): ")?
            };
            let opened = envelope::open(&text, &password);
            password.zeroize();
            let contents = envelope::contents(&opened?)?;

            let folder = folder()?;
            let imported = match &contents.profiles {
                Some(document) => folder.import_all(document)?,
                None => profiles::Imported { created: Vec::new(), refused: Vec::new() },
            };
            let taken = envelope::take_secrets(
                &contents.secrets,
                &secrets::entries(),
                &|entry| Ok(keys::load(entry)?.map(|secret| secret.expose().to_owned())),
                &keys::store,
            )
            .map_err(|why| {
                // Die Profile sind dann schon da; die Meldung sagt es, damit
                // niemand glaubt, es sei nichts geschehen.
                if imported.created.is_empty() {
                    why
                } else {
                    format!("{why}; profiles taken before that: {}", imported.created.join(", "))
                }
            })?;

            if json {
                let refused: Vec<serde_json::Value> = taken
                    .refused
                    .iter()
                    .map(|(name, why)| json!({ "name": name, "why": why }))
                    .collect();
                let mut all = imported.json();
                all["secrets"] =
                    json!({ "stored": taken.stored, "same": taken.same, "refused": refused });
                println!("{}", pretty(&all));
                return Ok(());
            }
            for name in &imported.created {
                println!("Profile \"{name}\" taken.");
            }
            for (name, why) in &imported.refused {
                println!("Profile \"{name}\" refused: {why}.");
            }
            let place = keys::described(&keys::place());
            for entry in &taken.stored {
                println!("{} stored in {place}.", secrets::label(entry));
            }
            for entry in &taken.same {
                println!("{}: the same one is stored here already.", secrets::label(entry));
            }
            for (entry, why) in &taken.refused {
                println!("{} refused: {why}.", secrets::label(entry));
            }
            println!(
                "Profiles: {} taken, {} refused. Secrets: {} stored, {} here already, {} refused.",
                imported.created.len(),
                imported.refused.len(),
                taken.stored.len(),
                taken.same.len(),
                taken.refused.len()
            );
            println!(
                "Note: the fish's Matrix account is never part of an envelope. If a profile has a \
                 table at Matrix, log the fish in here with `rusty-fish matrix login`."
            );
            println!(
                "Note: one bot runs in one place only. Switch the fish off where the file came \
                 from before you switch it on here, or everything is translated twice."
            );
            Ok(())
        }
    }
}

/// Der Profilordner dieses Rechners.
fn folder() -> Result<Folder, String> {
    Ok(Folder::at(home()?.join("profiles")))
}

/// Lädt ein gespeichertes Profil so weit, dass die Tür es bedienen kann.
fn load_profile(folder: &Folder, name: &str) -> Result<DoorProfile<AnyModel>, String> {
    profiles::valid_name(name)?;
    let text = folder.read_config(name)?;
    serve(Some(name.to_owned()), &text, folder.state_path(name))
}

/// Aus dem Text eines Profils wird, was die Tür braucht: die geprüften
/// Einstellungen und die KI, die für dieses Profil übersetzt. Dienst, Adresse
/// und Schlüssel kommen aus dem Profil.
fn serve(
    name: Option<String>,
    text: &str,
    state_path: PathBuf,
) -> Result<DoorProfile<AnyModel>, String> {
    let config = Config::parse(text)?;
    let key = key_for(config.provider)?;
    let base_url = config.service_address();
    let model = connect(config.provider, Some(&base_url), &config.model, key)?;
    // Der Name des Dienstes kommt aus der Adresse, die wirklich benutzt wird,
    // damit die Bitte um Zustimmung nichts anderes nennt.
    let service = rusty_fish_house::texts::service_name(&base_url);
    Ok(DoorProfile { name, config, state_path, model, service, shared: None })
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn pretty(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

/// Der Schlüssel für einen Dienst aus der Ablage. Fehlt er, obwohl der Dienst
/// einen braucht, sagt die Meldung, wie man ihn ablegt. Ein eigenes Modell
/// darf ohne Schlüssel laufen.
fn key_for(provider: Provider) -> Result<Option<Secret>, String> {
    match keys::load(provider.id())? {
        Some(key) => Ok(Some(key)),
        None if provider.needs_key() => Err(secrets::missing(provider)),
        None => Ok(None),
    }
}

/// Der Anschluss für die Befehle `models` und `translate`.
fn service(provider: &str, base_url: Option<&str>, model: &str) -> Result<AnyModel, String> {
    let provider = Provider::parse(provider)?;
    // Auf der Befehlszeile heißt die Adresse `--base-url`; die Meldung aus den
    // Einstellungen würde hier ein Feld nennen, das es an dieser Stelle nicht gibt.
    let own = base_url.map(str::trim).filter(|url| !url.is_empty());
    if own.is_none() && provider.base_url().is_none() {
        return Err(format!(
            "{} needs `--base-url`, the address of the service",
            provider.label()
        ));
    }
    connect(provider, own, model, key_for(provider)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_line_knows_exactly_the_entries_of_the_store() {
        let mut from_cli: Vec<&str> = Which::value_variants().iter().map(|w| w.entry()).collect();
        let mut from_list = secrets::entries();
        from_cli.sort();
        from_list.sort();
        assert_eq!(from_cli, from_list);
    }

    #[test]
    fn the_guide_names_every_ai_service_the_fish_knows() {
        let text = guide();
        for entry in catalog().as_array().into_iter().flatten() {
            let label = entry["label"].as_str().expect("a service has a label");
            assert!(text.contains(label), "the guide does not name {label}");
        }
    }

    /// Kennungen von Discord sind lange Ziffernfolgen. Die Beschreibung geht an
    /// fremde KIs; eine echte Kennung hat darin nichts verloren.
    #[test]
    fn the_guide_contains_no_discord_id() {
        let longest = guide()
            .split(|c: char| !c.is_ascii_digit())
            .map(str::len)
            .max()
            .unwrap_or(0);
        assert!(longest < 10, "the guide contains a number of {longest} digits");
    }

    /// Nemesus am 22.11.2026 zum Auftrag der Matrix-Tür: ehrlich schreiben,
    /// dass die Verschlüsselung von Matrix nur bis zum Fisch reicht und der
    /// Satz danach an den KI-Dienst des Gastgebers geht.
    #[test]
    fn the_guide_says_how_far_the_encryption_of_matrix_reaches() {
        // Der Leitfaden ist umbrochen; verglichen wird ohne die Umbrüche.
        let text = guide().split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(text.contains("That encryption reaches only as far as the fish."));
        assert!(text.contains("the host's AI service reads every sentence the fish translates"));
        assert!(text.contains("rusty-fish matrix login --user @name:server"));
    }

    /// Nemesus am 24.11.2026: er hatte einmal eine KI zum Teilen angeboten,
    /// und die erste Frage eines Gasts konnte sie nicht beantworten: „Wer ist
    /// Nemesus." Der Leitfaden sagt es jetzt, und er sagt der KI, dass sie
    /// nichts dazuerfindet.
    #[test]
    fn the_guide_says_who_made_the_fish_and_asks_for_nothing_invented() {
        let text = guide().split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(text.contains("## Who made this Rusty Fish is made by Nemesus."));
        assert!(text.contains("https://www.youtube.com/@NemesusEN"));
        assert!(text.contains("Do not add anything about him that is not written here."));
    }

    /// Entschieden von Nemesus am 24.11.2026: die AGPL 3, „Version 3 oder
    /// später", mit beiden Namen im Vermerk. Der Wortlaut liegt unverändert im
    /// Repository, und der Leitfaden nennt Lizenz und Urheber.
    #[test]
    fn the_license_is_the_agpl_3_and_the_guide_says_so() {
        let license = include_str!("../../LICENSE");
        assert!(license.trim_start().starts_with("GNU AFFERO GENERAL PUBLIC LICENSE"));
        assert!(license.contains("Version 3, 19 November 2007"));
        assert!(license.contains("13. Remote Network Interaction"));
        assert!(license.contains("END OF TERMS AND CONDITIONS"));
        assert_eq!(env!("CARGO_PKG_LICENSE"), "AGPL-3.0-or-later");

        let text = guide().split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(text.contains(
            "GNU Affero General Public License, version 3 or (at your option) any later version"
        ));
        assert!(text.contains("Copyright (C) 2026 Christian Ermisch (Nemesus)."));
    }

    /// Die Datei für systemd liegt dem Linux-Paket bei, und der Leitfaden
    /// zeigt dieselbe zum Abschreiben. Zwei Fassungen, die auseinanderlaufen,
    /// wären schlimmer als keine.
    #[test]
    fn the_file_for_systemd_is_the_one_the_guide_shows() {
        let text = guide();
        let shown: Vec<&str> = text.lines().map(str::trim).collect();
        for line in include_str!("../../packaging/rusty-fish.service").lines() {
            if !line.trim().is_empty() {
                assert!(shown.contains(&line.trim()), "the guide does not show the line {line:?}");
            }
        }
    }

    #[test]
    fn the_guide_ends_with_the_version_it_describes() {
        let text = guide();
        assert!(text.trim_end().ends_with(&format!("rusty-fish {}.", env!("CARGO_PKG_VERSION"))));
    }
}
