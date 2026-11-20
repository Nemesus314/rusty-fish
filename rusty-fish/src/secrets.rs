//! Welche Geheimnisse der Fisch kennt und wie sie heißen.
//!
//! Je KI-Dienst ein Eintrag in der Ablage, benannt nach der Kennung des
//! Dienstes, dazu der Token des Discord-Bots. Die Namen kommen aus der Liste
//! der Dienste und werden hier nicht noch einmal abgetippt.
//!
//! Entschieden von Nemesus am 20.11.2026: „Die Möglichkeit, die KIs im Profil
//! zu wechseln sollte jedoch erhalten bleiben." Mit nur einem Eintrag für den
//! KI-Schlüssel hätte jeder Wechsel den Schlüssel des vorigen Dienstes ersetzt.

use crate::profiles::BOT;
use rusty_fish_providers::Provider;
use serde_json::Value;

/// Alle Einträge: erst der Bot, dann die Dienste in der Reihenfolge der Liste.
pub fn entries() -> Vec<&'static str> {
    std::iter::once(BOT).chain(Provider::ALL.iter().map(|p| p.id())).collect()
}

/// Wie ein Eintrag in Meldungen heißt.
pub fn label(entry: &str) -> String {
    if entry == BOT {
        return "Discord bot token".to_owned();
    }
    match Provider::parse(entry) {
        Ok(provider) => format!("{} API key", provider.label()),
        // Kommt nicht vor, solange die Befehlszeile nur bekannte Einträge
        // annimmt; lieber den Namen zeigen als nichts.
        Err(_) => format!("{entry} secret"),
    }
}

/// Was `key status --json` ausgibt: zu jedem Eintrag, ob etwas liegt. `stored`
/// fragt die Ablage; als Parameter, damit die Tests sie nicht anfassen.
pub fn status(stored: &dyn Fn(&str) -> Result<bool, String>) -> Result<Value, String> {
    let mut status = serde_json::Map::new();
    for entry in entries() {
        status.insert(entry.to_owned(), Value::Bool(stored(entry)?));
    }
    Ok(Value::Object(status))
}

/// Die Meldung, wenn der Schlüssel eines Dienstes fehlt. Sie nennt den Befehl,
/// mit dem man ihn ablegt.
pub fn missing(provider: Provider) -> String {
    format!(
        "{} not stored; run `rusty-fish key set {}` first",
        label(provider.id()),
        provider.id()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_is_one_entry_for_the_bot_and_one_per_service() {
        assert_eq!(entries(), ["discord", "openai", "xai", "google", "anthropic", "custom"]);
    }

    #[test]
    fn the_old_entry_names_are_unchanged() {
        // Unter diesen beiden Namen liegen schon Geheimnisse in der Ablage.
        assert!(entries().contains(&"openai"));
        assert!(entries().contains(&"discord"));
    }

    #[test]
    fn labels_say_what_the_secret_is_for() {
        assert_eq!(label("discord"), "Discord bot token");
        assert_eq!(label("openai"), "ChatGPT (OpenAI) API key");
        assert_eq!(label("anthropic"), "Claude (Anthropic) API key");
        assert_eq!(label("custom"), "Own model or other service API key");
    }

    #[test]
    fn the_status_names_every_entry() {
        let status = status(&|entry| Ok(entry == "openai" || entry == "discord")).unwrap();
        let object = status.as_object().unwrap();
        assert_eq!(object.len(), 6);
        assert_eq!(status["openai"], true);
        assert_eq!(status["discord"], true);
        for empty in ["xai", "google", "anthropic", "custom"] {
            assert_eq!(status[empty], false, "{empty}");
        }
    }

    #[test]
    fn a_store_that_cannot_be_read_fails_the_status() {
        assert!(status(&|_| Err("locked".to_owned())).is_err());
    }

    #[test]
    fn a_missing_key_names_the_command_that_stores_it() {
        let message = missing(Provider::Anthropic);
        assert!(message.contains("Claude (Anthropic)"), "got {message}");
        assert!(message.contains("rusty-fish key set anthropic"), "got {message}");
    }
}
