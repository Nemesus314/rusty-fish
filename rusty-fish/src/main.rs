//! Rusty Fish, das Programm. Noch ohne Tür: es verwaltet den Schlüssel und
//! übersetzt einen einzelnen Text zur Probe. Die Türen (Discord, später
//! Matrix) kommen als weitere Befehle dazu.

mod keys;

use clap::{Parser, Subcommand};
use rusty_fish_core::{Outcome, Request, translate};
use rusty_fish_openai::{OPENAI_BASE_URL, OpenAi, Secret};
use std::process::ExitCode;

const PROVIDER: &str = "openai";

#[derive(Parser)]
#[command(name = "rusty-fish", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage the API key in this system's credential store
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
        /// The text to translate
        text: String,
    },
}

#[derive(Subcommand)]
enum KeyAction {
    /// Ask for the key (input stays hidden) and store it
    Set,
    /// Tell whether a key is stored, without showing it
    Status,
    /// Delete the stored key
    Forget,
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
        Command::Key { action: KeyAction::Set } => {
            // Der Schlüssel geht vom Terminal direkt in die Ablage des Systems.
            // Er steht in keinem Argument, keiner Datei und keiner Ausgabe.
            let typed = rpassword::prompt_password("API key (input stays hidden): ")
                .map_err(|e| format!("cannot read from the terminal: {e}"))?;
            let typed = typed.trim();
            if typed.is_empty() {
                return Err("no key entered, nothing stored".to_owned());
            }
            keys::store(PROVIDER, typed)?;
            println!("Key stored in this system's credential store.");
            Ok(())
        }
        Command::Key { action: KeyAction::Status } => {
            match keys::load(PROVIDER)? {
                Some(_) => println!("A key is stored."),
                None => println!("No key is stored."),
            }
            Ok(())
        }
        Command::Key { action: KeyAction::Forget } => {
            if keys::forget(PROVIDER)? {
                println!("Key deleted.");
            } else {
                println!("No key was stored.");
            }
            Ok(())
        }
        Command::Models { base_url } => {
            let fish = OpenAi::new(&base_url, "", stored_key()?).map_err(|e| e.to_string())?;
            for id in fish.models().await.map_err(|e| e.to_string())? {
                println!("{id}");
            }
            Ok(())
        }
        Command::Translate { to, keep, model, base_url, text } => {
            let fish = OpenAi::new(&base_url, &model, stored_key()?).map_err(|e| e.to_string())?;
            let keep: Vec<&str> = keep.iter().map(String::as_str).collect();
            let request = Request {
                speaker: "me",
                text: &text,
                target_lang: &to,
                pass_through: &keep,
                context: &[],
            };
            match translate(&fish, &request).await.map_err(|e| e.to_string())? {
                Outcome::Translated(translation) => println!("{translation}"),
                Outcome::Skip => println!("(nothing to translate)"),
            }
            Ok(())
        }
    }
}

fn stored_key() -> Result<Secret, String> {
    keys::load(PROVIDER)?.ok_or_else(|| "no key stored; run `rusty-fish key set` first".to_owned())
}
