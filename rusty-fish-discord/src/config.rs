//! Einstellungen der Tür, als TOML-Datei. Nichts ist auf einen bestimmten
//! Betreiber zugeschnitten: andere sollen den Fisch bei sich laufen lassen.

use rusty_fish_providers::Provider;
use rusty_fish_room::{Gender, Limits, Room, Seat};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Der KI-Dienst, der übersetzt. Fehlt die Angabe, gilt OpenAI, damit
    /// Profile von vor dem 20.11.2026 unverändert laufen.
    ///
    /// Entschieden von Nemesus am 20.11.2026: „Die Möglichkeit, die KIs im
    /// Profil zu wechseln sollte jedoch erhalten bleiben."
    pub provider: Provider,
    /// Modell, das übersetzt. Siehe `rusty-fish models`.
    pub model: String,
    /// Eine eigene Adresse statt der des Dienstes. Bei einem selbst gehosteten
    /// Modell („custom") ist sie Pflicht, sonst eine Überschreibung.
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
    /// Was der Gastgeber über einzelne Menschen eingetragen hat: Discord-
    /// Kennung und grammatisches Geschlecht. Freiwillig.
    ///
    /// Entschieden am 19.11.2026: beide Kanäle bleiben für beide offen.
    /// Nemesus: „Sie soll ruhig auch den Kanal #tee sehen können und sich
    /// reinschalten." Seine Frage, weil das Geschlecht bis dahin am Platz
    /// hing: „Wie können wir das ändern?"
    pub people: BTreeMap<String, Gender>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    provider: Option<String>,
    model: String,
    base_url: Option<String>,
    host: String,
    ai_name: Option<RawNames>,
    #[serde(default)]
    limits: RawLimits,
    #[serde(rename = "room", default)]
    rooms: Vec<RawRoom>,
    #[serde(rename = "person", default)]
    people: Vec<RawPerson>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPerson {
    id: String,
    gender: String,
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
                let gender = read_gender(seat.gender.as_deref(), &format!("channel {channel}"))?;
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
        let mut people = BTreeMap::new();
        for person in raw.people {
            let id = discord_id(&person.id, "id")?;
            // Ein Eintrag ohne brauchbare Angabe sagt nichts; deshalb ist das
            // Geschlecht hier Pflicht und wird wie beim Platz streng geprüft.
            let Some(gender) = read_gender(Some(&person.gender), &format!("person {id}"))? else {
                continue;
            };
            // Zwei Einträge zur selben Person wären ein Versehen oder ein Widerspruch.
            if people.insert(id.clone(), gender).is_some() {
                return Err(format!("configuration: person {id} is listed twice"));
            }
        }
        // Fehlt der Dienst oder ist er leer, gilt OpenAI: so liefen alle
        // Profile, bevor es die Wahl gab. Ein unbekannter wird laut abgelehnt.
        let provider = match raw.provider.as_deref().map(str::trim) {
            None | Some("") => Provider::OpenAi,
            Some(id) => {
                Provider::parse(id).map_err(|why| format!("configuration: `provider`: {why}"))?
            }
        };
        // Die eine Stelle, die über die Adresse entscheidet, prüft auch, ob es
        // eine gibt. Bei einem eigenen Modell muss sie im Profil stehen.
        rusty_fish_providers::address(provider, base_url.as_deref())
            .map_err(|why| format!("configuration: {why}"))?;
        Ok(Config { provider, model, base_url, host, ai_names, limits, rooms, people })
    }

    /// Die Adresse des Dienstes, die wirklich benutzt wird: die eigene aus dem
    /// Profil, sonst die des Dienstes. Die Bitte um Zustimmung nennt den Dienst
    /// nach dieser Adresse.
    pub fn service_address(&self) -> String {
        // Dass es eine gibt, hat `parse` geprüft.
        rusty_fish_providers::address(self.provider, self.base_url.as_deref())
            .unwrap_or_default()
    }

    /// Das grammatische Geschlecht, das der Gastgeber für diese Person
    /// eingetragen hat.
    pub fn gender_of(&self, user: &str) -> Option<Gender> {
        self.people.get(user).copied()
    }

    /// Das grammatische Geschlecht für jemanden, der an diesem Kanal schreibt
    /// oder dort angesprochen wird: zuerst die Angabe zur Person, sonst die
    /// des Platzes, sonst keine.
    pub fn gender_for(&self, user: &str, channel: &str) -> Option<Gender> {
        self.gender_of(user).or_else(|| self.seat_of(channel).and_then(|s| s.gender))
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

/// Liest die Angabe zum grammatischen Geschlecht eines Platzes oder einer
/// Person. Erlaubt sind "female" und "male"; alles andere wird laut abgelehnt,
/// damit ein Tippfehler nicht still zu „keine Angabe" wird. `whose` nennt in
/// der Fehlermeldung die Stelle, etwa "channel 222" oder "person 777".
fn read_gender(value: Option<&str>, whose: &str) -> Result<Option<Gender>, String> {
    let Some(value) = value else { return Ok(None) };
    match value.trim().to_ascii_lowercase().as_str() {
        "female" => Ok(Some(Gender::Female)),
        "male" => Ok(Some(Gender::Male)),
        _ => Err(format!(
            "configuration: `gender` for {whose} must be \"female\" or \"male\", got {value:?}"
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
