//! Das bearbeitbare Dokument eines Profils: dieselben Einstellungen wie in der
//! TOML-Datei, aber als JSON, damit eine Oberfläche sie anzeigen und ändern kann.
//!
//! Nemesus am 20.11.2026: „Du öffnest das Win-Programm und bekommst eine Maske
//! mit allen Profilen. Jedes kannst du editieren und danach speichern."
//!
//! Eine Wahrheit: geprüft wird ein Dokument nie hier, sondern immer von
//! [`Config::parse`]. Das Dokument wird dafür erst zu TOML gemacht und dann
//! gelesen wie jede von Hand geschriebene Datei. So kann die Maske nichts
//! speichern, was der Fisch beim Start ablehnen würde, und es gibt keine
//! zweite Prüfung, die von der ersten abweichen könnte.

use crate::config::{Config, DOORS};
use rusty_fish_room::{Gender, Limits};
use serde::{Deserialize, Serialize};

/// Die Einstellungen eines Profils in der Form, die über JSON hinausgeht.
/// Kennungen und Geschlecht sind hier Texte, so wie ein Mensch sie eintippt;
/// ob sie stimmen, entscheidet erst [`Document::check`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    /// Der Name des Profils. Er steht nicht in der Datei, sondern ist ihr
    /// Dateiname; beim Speichern gilt der Name aus dem Befehl, nicht dieser.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Die Kennung des KI-Dienstes, etwa "openai" oder "anthropic". Fehlt sie
    /// in einem Dokument, gilt "openai", wie in einer Datei.
    #[serde(default = "default_provider")]
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub base_url: Option<String>,
    pub host: String,
    #[serde(default)]
    pub ai_names: Vec<String>,
    #[serde(default)]
    pub limits: DocLimits,
    pub rooms: Vec<DocRoom>,
    #[serde(default)]
    pub people: Vec<DocPerson>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocLimits {
    pub translation_tokens_per_day: u64,
    pub questions_per_guest_per_day: u32,
}

impl Default for DocLimits {
    /// Dieselben Vorgaben wie in der Raum-Logik, nicht noch einmal abgetippt.
    fn default() -> Self {
        let limits = Limits::default();
        DocLimits {
            translation_tokens_per_day: limits.translation_tokens_per_day,
            questions_per_guest_per_day: limits.questions_per_guest_per_day,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocRoom {
    #[serde(default)]
    pub pass_through: Vec<String>,
    #[serde(default = "default_context_lines")]
    pub context_lines: usize,
    pub seats: Vec<DocSeat>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocSeat {
    /// Die Tür, zu der dieser Tisch gehört. Bisher gibt es nur "discord";
    /// fehlt die Angabe, gilt sie.
    #[serde(default = "default_door")]
    pub door: String,
    /// Die Kennung des Kanals an dieser Tür.
    pub channel: String,
    pub lang: String,
    /// `null`, "female" oder "male".
    #[serde(default)]
    pub gender: Option<String>,
    /// Der Mensch, dessen Tisch das ist: sein Name, wie unter `people`, oder
    /// seine Discord-Kennung. `null`, wenn der Tisch niemandem gehört.
    #[serde(default)]
    pub owner: Option<String>,
    /// Schreibt dieser Fisch an diesen Tisch? Berechnet, nicht eingestellt:
    /// es steht nur in der Liste der Profile. Beim Speichern wird es
    /// überlesen, damit eine Maske einen Eintrag der Liste zurückschicken kann.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub served: Option<bool>,
}

/// Was der Gastgeber über einen Menschen einträgt. Pflicht ist ein Name oder
/// eine Kennung.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocPerson {
    /// Der Name, den der Gastgeber diesem Menschen gibt, oder `null`.
    #[serde(default)]
    pub name: Option<String>,
    /// Die Kennung an der Discord-Tür, oder `null`. Bis zum 21.11.2026 hieß
    /// das Feld `id`; so wird es weiter angenommen.
    #[serde(default, alias = "id")]
    pub discord: Option<String>,
    /// `null`, "female" oder "male".
    #[serde(default)]
    pub gender: Option<String>,
    /// Die Muttersprache, oder `null`.
    #[serde(default)]
    pub lang: Option<String>,
    /// Sprachen, die für diesen Menschen unübersetzt bleiben. `null` heißt:
    /// es gilt die Angabe des Raums. Eine leere Liste heißt: alles übersetzen.
    #[serde(default)]
    pub neutral: Option<Vec<String>>,
    /// Vom Gastgeber eingeladen: sein Fisch bedient auch diesen Tisch.
    #[serde(default)]
    pub invited: bool,
}

impl DocPerson {
    /// Ein Mensch, über den nur die Discord-Kennung eingetragen ist.
    pub fn new(id: &str) -> DocPerson {
        DocPerson {
            name: None,
            discord: Some(id.to_owned()),
            gender: None,
            lang: None,
            neutral: None,
            invited: false,
        }
    }

    /// Ein Mensch, über den nur der Name eingetragen ist.
    pub fn named(name: &str) -> DocPerson {
        DocPerson { name: Some(name.to_owned()), discord: None, ..DocPerson::new("") }
    }
}

impl DocSeat {
    /// Ein Platz ohne Besitzer und ohne Anrede.
    pub fn new(channel: &str, lang: &str) -> DocSeat {
        DocSeat {
            door: default_door(),
            channel: channel.to_owned(),
            lang: lang.to_owned(),
            gender: None,
            owner: None,
            served: None,
        }
    }
}

fn default_context_lines() -> usize {
    10
}

fn default_door() -> String {
    "discord".to_owned()
}

fn default_provider() -> String {
    "openai".to_owned()
}

/// Wie ein Geschlecht im Dokument und in der TOML-Datei heißt.
fn gender_word(gender: Gender) -> &'static str {
    match gender {
        Gender::Female => "female",
        Gender::Male => "male",
    }
}

impl From<&Config> for Document {
    /// Aus gelesenen und geprüften Einstellungen wird das Dokument für die Maske.
    fn from(config: &Config) -> Self {
        Document {
            name: None,
            provider: config.provider.id().to_owned(),
            model: config.model.clone(),
            base_url: config.base_url.clone(),
            // Gezeigt wird, was eingetragen wurde: der Name, wenn dort einer steht.
            host: config.host_written.clone(),
            ai_names: config.ai_names.clone(),
            limits: DocLimits {
                translation_tokens_per_day: config.limits.translation_tokens_per_day,
                questions_per_guest_per_day: config.limits.questions_per_guest_per_day,
            },
            rooms: config
                .rooms
                .iter()
                .map(|room| DocRoom {
                    pass_through: room.pass_through.clone(),
                    context_lines: room.context_lines,
                    seats: room
                        .seats
                        .iter()
                        .map(|seat| {
                            // Gezeigt wird, was eingetragen wurde, nicht das
                            // daraus Abgeleitete: wer am Tisch eines Russen
                            // "German" stehen hat, soll das wiederfinden.
                            let written = config.written.get(&seat.channel);
                            DocSeat {
                                door: default_door(),
                                channel: seat.channel.clone(),
                                lang: written.map_or_else(|| seat.lang.clone(), |w| w.lang.clone()),
                                gender: written
                                    .map_or(seat.gender, |w| w.gender)
                                    .map(|g| gender_word(g).to_owned()),
                                owner: written.and_then(|w| w.owner_written.clone()),
                                served: None,
                            }
                        })
                        .collect(),
                })
                .collect(),
            // In der Reihenfolge, in der sie eingetragen sind, nicht nach
            // Kennung sortiert: die Maske soll wiederfinden, was sie schickte.
            people: config
                .people_order
                .iter()
                .filter_map(|key| config.people.get(key))
                .map(|person| DocPerson {
                    name: person.name.clone(),
                    discord: person.discord.clone(),
                    gender: person.gender.map(|g| gender_word(g).to_owned()),
                    lang: person.lang.clone(),
                    neutral: person.neutral.clone(),
                    invited: person.invited,
                })
                .collect(),
        }
    }
}

impl Document {
    /// Liest ein Dokument, wie die Oberfläche es schickt.
    pub fn from_json(json: &str) -> Result<Document, String> {
        serde_json::from_str(json).map_err(|e| format!("the profile is not a valid JSON document: {e}"))
    }

    /// Schreibt das Dokument als TOML-Datei im Format, das der Fisch liest.
    /// Ungeprüft: was hier herauskommt, muss erst durch [`Config::parse`].
    ///
    /// Benannte Grenze: Kommentare, die jemand von Hand in die Datei
    /// geschrieben hat, überleben das Speichern aus der Maske nicht.
    pub fn to_toml(&self) -> String {
        let mut out = String::from(
            "# Rusty Fish profile. Saved by the program; change it there or by hand.\n",
        );
        // Der Dienst steht immer da, auch wenn es der Vorgabe-Dienst ist: wer
        // die Datei liest, soll nicht wissen müssen, was ohne Angabe gilt.
        out.push_str(&format!("provider = {}\n", quote(&self.provider)));
        out.push_str(&format!("model = {}\n", quote(&self.model)));
        // Eine leere Adresse heißt wie beim Lesen: keine, also der Vorgabe-Dienst.
        if let Some(url) = self.base_url.as_deref().filter(|u| !u.trim().is_empty()) {
            out.push_str(&format!("base_url = {}\n", quote(url)));
        }
        out.push_str(&format!("host = {}\n", quote(&self.host)));
        if !self.ai_names.is_empty() {
            out.push_str(&format!("ai_name = {}\n", list(&self.ai_names)));
        }

        out.push_str("\n[limits]\n");
        out.push_str(&format!(
            "translation_tokens_per_day = {}\n",
            self.limits.translation_tokens_per_day
        ));
        out.push_str(&format!(
            "questions_per_guest_per_day = {}\n",
            self.limits.questions_per_guest_per_day
        ));

        for room in &self.rooms {
            out.push_str("\n[[room]]\n");
            out.push_str(&format!("pass_through = {}\n", list(&room.pass_through)));
            out.push_str(&format!("context_lines = {}\n", room.context_lines));
            for seat in &room.seats {
                out.push_str("\n  [[room.seat]]\n");
                // Der Tisch nennt seine Tür: der Kanal steht unter ihrem Namen.
                // Eine Tür, die es nicht gibt, lehnt `check` vorher ab.
                out.push_str(&format!("  {} = {}\n", seat.door.trim(), quote(&seat.channel)));
                out.push_str(&format!("  lang = {}\n", quote(&seat.lang)));
                if let Some(gender) = &seat.gender {
                    out.push_str(&format!("  gender = {}\n", quote(gender)));
                }
                // Ein leerer Besitzer heißt wie beim Lesen: keiner. `served`
                // wird nie geschrieben: es ist berechnet, nicht eingestellt.
                if let Some(owner) = seat.owner.as_deref().filter(|o| !o.trim().is_empty()) {
                    out.push_str(&format!("  owner = {}\n", quote(owner)));
                }
            }
        }

        for person in &self.people {
            out.push_str("\n[[person]]\n");
            if let Some(name) = person.name.as_deref().filter(|n| !n.trim().is_empty()) {
                out.push_str(&format!("name = {}\n", quote(name)));
            }
            if let Some(id) = person.discord.as_deref().filter(|d| !d.trim().is_empty()) {
                out.push_str(&format!("discord = {}\n", quote(id)));
            }
            if let Some(gender) = &person.gender {
                out.push_str(&format!("gender = {}\n", quote(gender)));
            }
            if let Some(lang) = person.lang.as_deref().filter(|l| !l.trim().is_empty()) {
                out.push_str(&format!("lang = {}\n", quote(lang)));
            }
            // Eine leere Liste wird geschrieben: sie heißt „alles übersetzen"
            // und ist etwas anderes als keine Angabe.
            if let Some(neutral) = &person.neutral {
                out.push_str(&format!("neutral = {}\n", list(neutral)));
            }
            // Nur wenn gesetzt, damit ein Profil ohne Einladungen so in der
            // Datei steht wie vor dem 20.11.2026.
            if person.invited {
                out.push_str("invited = true\n");
            }
        }
        out
    }

    /// Dasselbe Dokument, ergänzt um die Angabe je Platz, ob dieser Fisch ihn
    /// bedient. So steht es in der Liste der Profile.
    pub fn with_served(mut self, config: &Config) -> Document {
        // Die Regel steht an einer Stelle, in `Config::parse`. Hier wird nur
        // abgelesen, was sie ergeben hat.
        for seat in self.rooms.iter_mut().flat_map(|room| room.seats.iter_mut()) {
            seat.served = Some(config.serves(&seat.channel));
        }
        self
    }

    /// Prüft das Dokument mit derselben Prüfung wie jede Einstellungsdatei und
    /// liefert den Text, der gespeichert werden darf, samt den gelesenen
    /// Einstellungen.
    pub fn check(&self) -> Result<(String, Config), String> {
        for seat in self.rooms.iter().flat_map(|room| room.seats.iter()) {
            let door = seat.door.trim();
            if !DOORS.contains(&door) {
                return Err(format!(
                    "configuration: the door {door:?} is not built; this fish knows: {}",
                    DOORS.join(", ")
                ));
            }
        }
        let toml = self.to_toml();
        let config = Config::parse(&toml)?;
        Ok((toml, config))
    }
}

/// Ein Text als TOML-Zeichenkette in doppelten Anführungszeichen. Von Hand,
/// weil es wenige Regeln sind: Anführungszeichen, Rückstrich und
/// Steuerzeichen werden maskiert, alles andere bleibt, wie es ist.
fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn list(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|item| quote(item)).collect();
    format!("[{}]", quoted.join(", "))
}
