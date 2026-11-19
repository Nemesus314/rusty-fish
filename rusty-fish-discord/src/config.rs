//! Einstellungen der Tür, als TOML-Datei. Nichts ist auf einen bestimmten
//! Betreiber zugeschnitten: andere sollen den Fisch bei sich laufen lassen.

use rusty_fish_room::{Gender, Limits, Room, Seat};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Modell, das übersetzt. Siehe `rusty-fish models`.
    pub model: String,
    /// Adresse eines Dienstes im OpenAI-Format. Fehlt sie, gilt OpenAI selbst.
    pub base_url: Option<String>,
    /// Discord-Kennung des Gastgebers. Wer den Fisch startet, hat zugestimmt.
    pub host: String,
    /// Die Rufnamen der KI. Der erste ist der Name, unter dem sie erscheint;
    /// jeder gilt als Ansprache. Ist die Liste leer, übersetzt sie nur.
    /// Nemesus am 18.11.2026: „Den können wir frei wählen."
    ///
    /// Mehrere, weil ein Gast mit kyrillischer Tastatur für einen lateinischen
    /// Namen jedes Mal umschalten müsste (entschieden am 19.11.2026).
    pub ai_names: Vec<String>,
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
    ai_name: Option<RawNames>,
    #[serde(default)]
    limits: RawLimits,
    #[serde(rename = "room", default)]
    rooms: Vec<RawRoom>,
}

/// `ai_name` darf ein einzelner Text sein, wie von Anfang an, oder eine Liste.
#[derive(Deserialize)]
#[serde(untagged)]
enum RawNames {
    One(String),
    Many(Vec<String>),
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
    gender: Option<String>,
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
                let gender = read_gender(seat.gender.as_deref(), &channel)?;
                seats.push(Seat { channel, lang, gender });
            }
            rooms.push(Room {
                seats,
                pass_through: room.pass_through,
                context_lines: room.context_lines,
            });
        }

        let base_url = raw.base_url.map(|u| u.trim().to_owned()).filter(|u| !u.is_empty());
        let ai_names = read_names(raw.ai_name)?;
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
        Ok(Config { model, base_url, host, ai_names, limits, rooms })
    }

    /// Der Name, unter dem die KI erscheint: ihr erster Rufname.
    pub fn ai_name(&self) -> Option<&str> {
        self.ai_names.first().map(String::as_str)
    }

    /// In welchem Raum liegt dieser Kanal?
    pub fn room_index(&self, channel: &str) -> Option<usize> {
        self.rooms.iter().position(|r| r.seats.iter().any(|s| s.channel == channel))
    }

    /// Der Platz an diesem Kanal.
    pub fn seat_of(&self, channel: &str) -> Option<&Seat> {
        self.rooms.iter().flat_map(|r| r.seats.iter()).find(|s| s.channel == channel)
    }

    /// Welche Sprache hat der Platz an diesem Kanal?
    pub fn lang_of(&self, channel: &str) -> Option<&str> {
        self.seat_of(channel).map(|s| s.lang.as_str())
    }
}

/// Liest die Rufnamen der KI.
///
/// - Fehlt der Wert, ist die Liste leer oder steht dort ein einzelner leerer
///   Text, dann ist die KI nicht ansprechbar. So war es von Anfang an.
/// - Ein leerer Eintrag in einer Liste wird laut abgelehnt: wer eine Liste
///   schreibt, meint jeden Eintrag, und ein leerer ist ein Versehen.
fn read_names(raw: Option<RawNames>) -> Result<Vec<String>, String> {
    match raw {
        None => Ok(Vec::new()),
        Some(RawNames::One(name)) => {
            let name = name.trim();
            Ok(if name.is_empty() { Vec::new() } else { vec![name.to_owned()] })
        }
        Some(RawNames::Many(names)) => {
            let mut out = Vec::new();
            for (n, name) in names.iter().enumerate() {
                let name = name.trim();
                if name.is_empty() {
                    return Err(format!("configuration: entry {} of `ai_name` is empty", n + 1));
                }
                out.push(name.to_owned());
            }
            Ok(out)
        }
    }
}

/// Liest die freiwillige Angabe zum grammatischen Geschlecht eines Platzes.
/// Erlaubt sind "female" und "male"; alles andere wird laut abgelehnt, damit
/// ein Tippfehler nicht still zu „keine Angabe" wird.
fn read_gender(value: Option<&str>, channel: &str) -> Result<Option<Gender>, String> {
    let Some(value) = value else { return Ok(None) };
    match value.trim().to_ascii_lowercase().as_str() {
        "female" => Ok(Some(Gender::Female)),
        "male" => Ok(Some(Gender::Male)),
        _ => Err(format!(
            "configuration: `gender` for channel {channel} must be \"female\" or \"male\" \
             (or be left out), got {value:?}"
        )),
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
