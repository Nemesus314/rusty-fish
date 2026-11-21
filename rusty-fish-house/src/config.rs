//! Das Profil eines Fischs, als TOML-Datei. Nichts ist auf einen bestimmten
//! Betreiber zugeschnitten: andere sollen den Fisch bei sich laufen lassen.
//!
//! Benannte Grenze: Menschen und Kanäle stehen hier mit ihrer Discord-Kennung,
//! einer Zahl. Das Profil gehört damit zu einer Tür.

use rusty_fish_providers::Provider;
use rusty_fish_room::tongue::{Tongue, tongue_of};
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
    /// Die Räume, so wie die Raum-Logik sie braucht: an jedem Tisch steht
    /// schon, was dort gilt (siehe [`Written`] für das, was eingetragen wurde).
    pub rooms: Vec<Room>,
    /// Was der Gastgeber über einzelne Menschen eingetragen hat, nach
    /// Discord-Kennung. Freiwillig.
    ///
    /// Entschieden am 19.11.2026: beide Kanäle bleiben für beide offen.
    /// Nemesus: „Sie soll ruhig auch den Kanal #tee sehen können und sich
    /// reinschalten." Seine Frage, weil das Geschlecht bis dahin am Platz
    /// hing: „Wie können wir das ändern?"
    pub people: BTreeMap<String, Person>,
    /// Die Kennungen der Menschen in der Reihenfolge, in der sie eingetragen
    /// sind. Die Maske zeigt sie so wieder und findet nach dem Speichern vor,
    /// was sie geschickt hat; nach Kennung sortiert stünde "9" hinter "42".
    pub people_order: Vec<String>,
    /// Was zu jedem Platz in der Datei steht, nach Kanal. Die Maske zeigt und
    /// speichert das Eingetragene, nicht das daraus Abgeleitete.
    pub written: BTreeMap<String, Written>,
}

/// Was der Gastgeber über einen Menschen eingetragen hat.
///
/// Nemesus am 20.11.2026: „ob es nicht sinnvoller wäre, pro Discord-Nummer die
/// Muttersprache und die Neutralsprache festzulegen. Dann wäre die Aufgabe
/// [...]: Übersetze alles für eine Russin, halte dich von Englisch fern."
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Person {
    /// Grammatisches Geschlecht, als Hinweis für die Grammatik.
    pub gender: Option<Gender>,
    /// Die Muttersprache: in sie wird an seinem Tisch übersetzt.
    pub lang: Option<String>,
    /// Sprachen, die für diesen Menschen unübersetzt bleiben. `None` heißt: es
    /// gilt die Angabe des Raums. Eine leere Liste heißt: alles übersetzen.
    pub neutral: Option<Vec<String>>,
    /// Hat der Gastgeber diesen Menschen eingeladen? Dann bedient sein Fisch
    /// auch dessen Tisch, auf seine Rechnung.
    ///
    /// Nemesus am 20.11.2026: „Es sollte auch die Funktion ‚Einladen' geben.
    /// Und dann ein Haken, wer eingeladen wird."
    pub invited: bool,
}

/// Was zu einem Platz in der Datei steht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub lang: String,
    pub gender: Option<Gender>,
    /// Discord-Kennung des Menschen, dessen Tisch das ist. Er muss dafür nicht
    /// unter `[[person]]` stehen.
    pub owner: Option<String>,
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
    gender: Option<String>,
    lang: Option<String>,
    neutral: Option<Vec<String>>,
    #[serde(default)]
    invited: bool,
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
    owner: Option<String>,
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

        // Die Menschen zuerst: was an einem Tisch gilt, hängt an seinem Besitzer.
        let mut people = BTreeMap::new();
        let mut people_order = Vec::new();
        for person in raw.people {
            let id = discord_id(&person.id, "id")?;
            let entry = Person {
                // Bis zum 20.11.2026 war das Geschlecht hier Pflicht, weil ein
                // Eintrag sonst nichts aussagte. Seit ein Mensch auch Sprache
                // und Einladung trägt, ist es freiwillig wie die anderen.
                gender: read_gender(person.gender.as_deref(), &format!("person {id}"))?,
                lang: person.lang.map(|l| l.trim().to_owned()).filter(|l| !l.is_empty()),
                neutral: person.neutral.map(clean_langs),
                invited: person.invited,
            };
            // Zwei Einträge zur selben Person wären ein Versehen oder ein Widerspruch.
            if people.insert(id.clone(), entry).is_some() {
                return Err(format!("configuration: person {id} is listed twice"));
            }
            people_order.push(id);
        }

        let mut written = BTreeMap::new();
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
                if written.contains_key(&channel) {
                    return Err(format!("configuration: channel {channel} is used twice"));
                }
                let gender = read_gender(seat.gender.as_deref(), &format!("channel {channel}"))?;
                // Ein leerer Besitzer heißt wie ein fehlender: der Tisch gehört niemandem.
                let owner = match seat.owner.as_deref().map(str::trim) {
                    None | Some("") => None,
                    Some(id) => Some(discord_id(id, "owner")?),
                };
                seats.push(at_the_table(&channel, &lang, gender, owner.as_deref(), &host, &people));
                written.insert(channel, Written { lang, gender, owner });
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
        Ok(Config {
            provider,
            model,
            base_url,
            host,
            ai_names,
            limits,
            rooms,
            people,
            people_order,
            written,
        })
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
        self.people.get(user).and_then(|p| p.gender)
    }

    /// Das grammatische Geschlecht für jemanden, der an diesem Kanal schreibt
    /// oder dort als Schreibender angesprochen wird: zuerst die Angabe zur
    /// Person, sonst die des Platzes, sonst keine. Die Anrede des Besitzers
    /// gilt hier nicht; sie gilt für das, was an seinem Tisch gelesen wird
    /// (siehe [`Seat::reader`]).
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

    /// Der Platz an diesem Kanal, mit dem, was dort gilt.
    pub fn seat_of(&self, channel: &str) -> Option<&Seat> {
        self.rooms.iter().flat_map(|r| r.seats.iter()).find(|s| s.channel == channel)
    }

    /// In welche Sprache wird an diesem Kanal übersetzt? Die Muttersprache
    /// des Besitzers, sonst die Sprache des Platzes.
    pub fn lang_of(&self, channel: &str) -> Option<&str> {
        self.seat_of(channel).map(|s| s.lang.as_str())
    }

    /// Die Sprache der festen Texte an diesem Tisch: Deutsch, Englisch oder
    /// Russisch, je nach Sprache des Tisches; für jede andere Englisch.
    pub fn tongue_at(&self, channel: &str) -> Tongue {
        tongue_of(self.lang_of(channel).unwrap_or(""))
    }

    /// Schreibt dieser Fisch an diesen Kanal? Ein Kanal außerhalb jedes
    /// Raums wird nicht bedient.
    pub fn serves(&self, channel: &str) -> bool {
        self.seat_of(channel).is_some_and(|s| s.served)
    }

    /// Wessen Tisch ist dieser Kanal?
    pub fn owner_of(&self, channel: &str) -> Option<&str> {
        self.written.get(channel).and_then(|w| w.owner.as_deref())
    }

    /// Gäbe dieser Fisch an die KI, was an diesem Kanal geschrieben wird?
    ///
    /// Nur dann bittet er dort um Zustimmung. Das ist der Fall, wenn er im
    /// selben Raum einen anderen Tisch bedient (für den übersetzt er es) oder
    /// wenn er diesen Tisch bedient und die KI ansprechbar ist (dann kann hier
    /// eine Frage an sie gehen).
    ///
    /// Warum nicht einfach an jedem Tisch: bringt jeder seinen eigenen Fisch
    /// mit, bedient jeder Fisch nur den Tisch seines Gastgebers. Wer dort als
    /// Gast schreibt, bekäme sonst von diesem Fisch eine Bitte um Zustimmung
    /// für eine KI, die von ihm nie ein Wort liest.
    pub fn sends_to_ai(&self, channel: &str) -> bool {
        let Some(room) = self.room_index(channel).map(|n| &self.rooms[n]) else { return false };
        room.seats.iter().any(|seat| {
            seat.served && (seat.channel != channel || !self.ai_names.is_empty())
        })
    }
}

/// Was an einem Tisch gilt. Hat er einen Besitzer, gehen dessen Angaben vor:
/// seine Muttersprache vor der Sprache des Platzes, seine unübersetzten
/// Sprachen vor denen des Raums, seine Anrede vor der des Platzes.
///
/// Bedient wird ein Tisch,
/// - wenn er keinen Besitzer hat (so laufen Profile von vor dem 20.11.2026
///   unverändert weiter),
/// - wenn er dem Gastgeber gehört,
/// - oder wenn sein Besitzer eingeladen ist.
///
/// Für alle anderen Tische übersetzt der Fisch ihres Besitzers.
fn at_the_table(
    channel: &str,
    lang: &str,
    gender: Option<Gender>,
    owner: Option<&str>,
    host: &str,
    people: &BTreeMap<String, Person>,
) -> Seat {
    // Der Besitzer muss nicht unter `[[person]]` stehen. Dann ist über ihn
    // nichts bekannt: es bleibt bei Platz und Raum, und eingeladen ist er nicht.
    let person = owner.and_then(|id| people.get(id));
    let served = match owner {
        None => true,
        Some(id) => id == host || person.is_some_and(|p| p.invited),
    };
    Seat {
        channel: channel.to_owned(),
        lang: person.and_then(|p| p.lang.clone()).unwrap_or_else(|| lang.to_owned()),
        // Die Angabe des Platzes bleibt stehen: sie gilt weiter für den, der
        // hier schreibt und selbst keinen Eintrag hat (Person vor Platz).
        gender,
        // Gelesen wird am Tisch vom Besitzer. Kennt die Tür seine Anrede
        // nicht, fällt die Raum-Logik auf die des Platzes zurück.
        reader_gender: person.and_then(|p| p.gender),
        neutral: person.and_then(|p| p.neutral.clone()),
        served,
    }
}

/// Eine Liste von Sprachen, wie ein Mensch sie eintippt: Leerraum weg, leere
/// Einträge weg. Die Reihenfolge bleibt.
fn clean_langs(langs: Vec<String>) -> Vec<String> {
    langs.into_iter().map(|l| l.trim().to_owned()).filter(|l| !l.is_empty()).collect()
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
