//! Rusty Fish, das Programm: Schlüssel verwalten, einen Text zur Probe
//! übersetzen, die Discord-Tür öffnen. Weitere Türen kommen als Befehle dazu.

mod keys;

use clap::{Parser, Subcommand, ValueEnum};
use rusty_fish_core::{Gender, Outcome, Request, translate};
use rusty_fish_discord::config::Config;
use rusty_fish_openai::{OPENAI_BASE_URL, OpenAi, Secret};
use std::path::PathBuf;
use std::process::ExitCode;

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
    /// List the models your key can use
    Models {
        /// Address of a service that speaks the OpenAI chat format
        #[arg(long, default_value = OPENAI_BASE_URL)]
        base_url: String,
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
        /// Address of a service that speaks the OpenAI chat format
        #[arg(long, default_value = OPENAI_BASE_URL)]
        base_url: String,
        /// Grammatical gender of the writer, for languages that mark it
        #[arg(long, value_enum)]
        writer: Option<Who>,
        /// Grammatical gender of the reader, for languages that mark it
        #[arg(long, value_enum)]
        reader: Option<Who>,
        /// The text to translate
        text: String,
    },
    /// Open the Discord door and translate between the configured channels
    Run {
        /// Settings file; see rusty-fish.example.toml
        #[arg(long, default_value = "rusty-fish.toml")]
        config: PathBuf,
    },
}

#[derive(Subcommand)]
enum KeyAction {
    /// Ask for the secret (input stays hidden) and store it
    Set {
        #[arg(value_enum, default_value_t = Which::Openai)]
        which: Which,
    },
    /// Tell whether a secret is stored, without showing it
    Status {
        #[arg(value_enum, default_value_t = Which::Openai)]
        which: Which,
    },
    /// Delete the stored secret
    Forget {
        #[arg(value_enum, default_value_t = Which::Openai)]
        which: Which,
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

/// Die Geheimnisse, die der Fisch kennt. Jedes liegt für sich in der Ablage.
#[derive(Clone, Copy, ValueEnum)]
enum Which {
    /// API key of the translating AI
    Openai,
    /// Token of your Discord bot
    Discord,
}

impl Which {
    /// Name des Eintrags in der Ablage.
    fn entry(self) -> &'static str {
        match self {
            Which::Openai => "openai",
            Which::Discord => "discord",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Which::Openai => "API key",
            Which::Discord => "Discord bot token",
        }
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
        Command::Key { action: KeyAction::Set { which } } => {
            // Das Geheimnis geht vom Terminal direkt in die Ablage des Systems.
            // Es steht in keinem Argument, keiner Datei und keiner Ausgabe.
            let typed = rpassword::prompt_password(format!("{} (input stays hidden): ", which.label()))
                .map_err(|e| format!("cannot read from the terminal: {e}"))?;
            let typed = typed.trim();
            if typed.is_empty() {
                return Err("nothing entered, nothing stored".to_owned());
            }
            keys::store(which.entry(), typed)?;
            println!("{} stored in this system's credential store.", which.label());
            Ok(())
        }
        Command::Key { action: KeyAction::Status { which } } => {
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
        Command::Models { base_url } => {
            let fish = OpenAi::new(&base_url, "", stored(Which::Openai)?).map_err(|e| e.to_string())?;
            for id in fish.models().await.map_err(|e| e.to_string())? {
                println!("{id}");
            }
            Ok(())
        }
        Command::Translate { to, keep, model, base_url, writer, reader, text } => {
            let fish =
                OpenAi::new(&base_url, &model, stored(Which::Openai)?).map_err(|e| e.to_string())?;
            let keep: Vec<&str> = keep.iter().map(String::as_str).collect();
            let request = Request {
                speaker: "me",
                text: &text,
                target_lang: &to,
                pass_through: &keep,
                context: &[],
                writer_gender: writer.map(Who::gender),
                reader_gender: reader.map(Who::gender),
            };
            match translate(&fish, &request).await.map_err(|e| e.to_string())? {
                Outcome::Translated(translation) => println!("{translation}"),
                Outcome::Skip => println!("(nothing to translate)"),
            }
            Ok(())
        }
        Command::Run { config: path } => {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            let config = Config::parse(&text)?;
            // Erst alles prüfen, dann verbinden: fehlt etwas, scheitert der
            // Fisch, bevor er sich bei Discord zeigt.
            let key = stored(Which::Openai)?;
            let token = stored(Which::Discord)?;
            let base_url = config.base_url.clone().unwrap_or_else(|| OPENAI_BASE_URL.to_owned());
            let fish = OpenAi::new(&base_url, &config.model, key).map_err(|e| e.to_string())?;
            // Der Zustand liegt neben den Einstellungen: rusty-fish.state.json
            let state_path = path.with_extension("state.json");
            rusty_fish_discord::door::run(token.expose(), config, &state_path, fish).await
        }
    }
}

fn stored(which: Which) -> Result<Secret, String> {
    keys::load(which.entry())?.ok_or_else(|| {
        format!(
            "{} not stored; run `rusty-fish key set {}` first",
            which.label(),
            which.entry()
        )
    })
}
