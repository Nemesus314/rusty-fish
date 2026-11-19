//! Einstellungen der Tür, als TOML-Datei. Nichts ist auf einen bestimmten
//! Betreiber zugeschnitten: andere sollen den Fisch bei sich laufen lassen.

use rusty_fish_room::{Limits, Room, Seat};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Modell, das übersetzt. Siehe `rusty-fish models`.
    pub model: String,
    /// Adresse eines Dienstes im OpenAI-Format. Fehlt sie, gilt OpenAI selbst.
    pub base_url: Option<String>,
    /// Discord-Kennung des Gastgebers. Wer den Fisch startet, hat zugestimmt.
    pub host: String,
    /// Der Name, unter dem die KI ansprechbar ist. Fehlt er, übersetzt sie nur.
    /// Nemesus am 18.11.2026: „Den können wir frei wählen."
    pub ai_name: Option<String>,
    /// Was der Gastgeber pro Tag erlaubt.
    pub limits: Limits,
    pub rooms: Vec<Room>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    model: String,
    base_url: Option<String>,
    host: String,
    ai_name: Option<String>,
    #[serde(default)]
    limits: RawLimits,
    #[serde(rename = "room", default)]
    rooms: Vec<RawRoom>,
}

/// Jede Grenze darf einzeln fehlen; dann gilt die vorsichtige Vorgabe.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawLimits {
    translation_tokens_per_day: Option<u64>,
    questions_per_guest_per_day: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRoom {
    #[serde(default)]
    pass_through: Vec<String>,
    #[serde(default = "default_context_lines")]
    context_lines: usize,
    #[serde(rename = "seat", default)]
    seats: Vec<RawSeat>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSeat {
    channel: String,
    lang: String,
}

fn default_context_lines() -> usize {
    10
}

impl Config {
    /// Liest und prüft die Einstellungen. Jeder Fehler ist laut und nennt die Stelle.
    pub fn parse(text: &str) -> Result<Config, String> {
        let raw: RawConfig = toml::from_str(text).map_err(|e| format!("configuration: {e}"))?;

        let model = raw.model.trim().to_owned();
        if model.is_empty() {
            return Err("configuration: `model` is empty".to_owned());
        }
        let host = discord_id(&raw.host, "host")?;
        if raw.rooms.is_empty() {
            return Err("configuration: no `[[room]]` is defined".to_owned());
        }

        let mut taken: Vec<String> = Vec::new();
        let mut rooms = Vec::new();
        for (n, room) in raw.rooms.into_iter().enumerate() {
            if room.seats.len() < 2 {
                return Err(format!("configuration: room {} needs at least two seats", n + 1));
            }
            let mut seats = Vec::new();
            for seat in room.seats {
                let channel = discord_id(&seat.channel, "channel")?;
                let lang = seat.lang.trim().to_owned();
                if lang.is_empty() {
                    return Err(format!("configuration: `lang` is empty for channel {channel}"));
                }
                // Ein Kanal an zwei Plätzen würde Nachrichten im Kreis schicken.
                if taken.contains(&channel) {
                    return Err(format!("configuration: channel {channel} is used twice"));
                }
                taken.push(channel.clone());
                seats.push(Seat { channel, lang });
            }
            rooms.push(Room {
                seats,
                pass_through: room.pass_through,
                context_lines: room.context_lines,
            });
        }

        let base_url = raw.base_url.map(|u| u.trim().to_owned()).filter(|u| !u.is_empty());
        // Ein leerer Name heißt: die KI ist nicht ansprechbar.
        let ai_name = raw.ai_name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
        let defaults = Limits::default();
        let limits = Limits {
            translation_tokens_per_day: raw
                .limits
                .translation_tokens_per_day
                .unwrap_or(defaults.translation_tokens_per_day),
            questions_per_guest_per_day: raw
                .limits
                .questions_per_guest_per_day
                .unwrap_or(defaults.questions_per_guest_per_day),
        };
        Ok(Config { model, base_url, host, ai_name, limits, rooms })
    }

    /// In welchem Raum liegt dieser Kanal?
    pub fn room_index(&self, channel: &str) -> Option<usize> {
        self.rooms.iter().position(|r| r.seats.iter().any(|s| s.channel == channel))
    }

    /// Welche Sprache hat der Platz an diesem Kanal?
    pub fn lang_of(&self, channel: &str) -> Option<&str> {
        self.rooms
            .iter()
            .flat_map(|r| r.seats.iter())
            .find(|s| s.channel == channel)
            .map(|s| s.lang.as_str())
    }
}

/// Discord-Kennungen sind Zahlen. Ein Kanalname wie "#general" ist keine.
fn discord_id(value: &str, field: &str) -> Result<String, String> {
    let value = value.trim();
    match value.parse::<u64>() {
        Ok(n) if n > 0 => Ok(value.to_owned()),
        _ => Err(format!(
            "configuration: `{field}` must be a Discord id (digits only, not zero), got {value:?}"
        )),
    }
}
