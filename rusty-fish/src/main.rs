//! Rusty Fish, das Programm: Schlüssel verwalten, einen Text zur Probe
//! übersetzen, Profile verwalten, die Discord-Tür öffnen. Weitere Türen kommen
//! als Befehle dazu.
//!
//! Eine Oberfläche spricht nur über diese Befehle mit dem Programm. Wo sie
//! etwas weiterverarbeitet, gibt es `--json`; Fehler stehen als Text auf der
//! Fehlerausgabe, und das Programm endet dann mit Code 1.

mod keys;
mod lock;
mod profiles;
mod secrets;

use clap::{Parser, Subcommand, ValueEnum};
use profiles::Folder;
use rusty_fish_core::{Gender, Outcome, Request, Secret, translate};
use rusty_fish_discord::config::Config;
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
        /// The text to translate
        text: String,
    },
    /// Manage saved profiles: settings you can keep, change and start
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Open the Discord door and translate between the configured channels
    Run {
        /// Settings file; see rusty-fish.example.toml [default: rusty-fish.toml]
        #[arg(long, conflicts_with = "profile")]
        config: Option<PathBuf>,
        /// Name of a saved profile; see `rusty-fish profile list`
        #[arg(long)]
        profile: Option<String>,
    },
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
    /// Take an existing settings file, and the state next to it, as a new profile
    Import {
        /// The settings file to copy
        path: PathBuf,
        /// Name of the new profile
        #[arg(long)]
        name: String,
    },
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
            println!("{} stored in this system's credential store.", which.label());
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
            };
            match translate(&fish, &request).await.map_err(|e| e.to_string())? {
                Outcome::Translated(translation) => println!("{translation}"),
                Outcome::Skip => println!("(nothing to translate)"),
            }
            Ok(())
        }
        Command::Profile { action } => profile(action),
        Command::Run { config, profile } => {
            // Wo die Einstellungen und ihr Zustand liegen: im Profilordner oder,
            // wie von Anfang an, in einer frei gewählten Datei mit dem Zustand
            // daneben.
            let folder = folder();
            let (path, state_path) = match &profile {
                Some(name) => {
                    profiles::valid_name(name)?;
                    let folder = folder.clone()?;
                    (folder.config_path(name), folder.state_path(name))
                }
                None => {
                    let path = config.unwrap_or_else(|| PathBuf::from("rusty-fish.toml"));
                    let state_path = path.with_extension("state.json");
                    (path, state_path)
                }
            };
            let text = match &profile {
                Some(name) => folder.clone()?.read_config(name)?,
                None => std::fs::read_to_string(&path)
                    .map_err(|e| format!("cannot read {}: {e}", path.display()))?,
            };
            let config = Config::parse(&text)?;
            // Erst alles prüfen, dann verbinden: fehlt etwas, scheitert der
            // Fisch, bevor er sich bei Discord zeigt. Dienst, Adresse und
            // Schlüssel kommen aus dem Profil.
            let key = key_for(config.provider)?;
            let token = keys::load(profiles::BOT)?.ok_or_else(|| {
                format!(
                    "{} not stored; run `rusty-fish key set {}` first",
                    secrets::label(profiles::BOT),
                    profiles::BOT
                )
            })?;
            let base_url = config.service_address();
            let fish = connect(config.provider, Some(&base_url), &config.model, key)?;

            // Ein Bot, ein Fisch: die Sperre gilt bis zum Ende dieses Laufs,
            // auch für einen Start mit --config, denn der Bot ist derselbe.
            let who = lock::Running { profile, pid: std::process::id(), started: now() };
            let _held = match &folder {
                Ok(folder) => Some(lock::acquire(folder.dir(), profiles::BOT, &who)?),
                // Nur bei --config erreichbar; mit --profile ist der Ordner oben
                // schon Pflicht. Lieber ohne Sperre laufen als gar nicht.
                Err(why) => {
                    eprintln!("rusty-fish: warning: {why}");
                    eprintln!("rusty-fish: warning: running without the one-fish-per-bot lock");
                    None
                }
            };

            // Der Name des Dienstes kommt aus der Adresse, die wirklich benutzt
            // wird, damit die Bitte um Zustimmung nichts anderes nennt.
            let service = rusty_fish_discord::render::service_name(&base_url);
            rusty_fish_discord::door::run(token.expose(), config, &state_path, fish, &service).await
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
        ProfileAction::Import { path, name } => {
            folder.import(&path, &name)?;
            println!("Profile \"{name}\" created from {}", path.display());
            Ok(())
        }
    }
}

/// Der Profilordner dieses Rechners.
fn folder() -> Result<Folder, String> {
    let home = profiles::home(&|key| std::env::var(key).ok(), profiles::this_os())?;
    Ok(Folder::at(home.join("profiles")))
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
}
