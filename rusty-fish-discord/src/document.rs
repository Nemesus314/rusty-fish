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

use crate::config::Config;
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
    pub channel: String,
    pub lang: String,
    /// `null`, "female" oder "male".
    #[serde(default)]
    pub gender: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocPerson {
    pub id: String,
    pub gender: String,
}

fn default_context_lines() -> usize {
    10
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
            model: config.model.clone(),
            base_url: config.base_url.clone(),
            host: config.host.clone(),
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
                        .map(|seat| DocSeat {
                            channel: seat.channel.clone(),
                            lang: seat.lang.clone(),
                            gender: seat.gender.map(|g| gender_word(g).to_owned()),
                        })
                        .collect(),
                })
                .collect(),
            people: config
                .people
                .iter()
                .map(|(id, gender)| DocPerson {
                    id: id.clone(),
                    gender: gender_word(*gender).to_owned(),
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
                out.push_str(&format!("  channel = {}\n", quote(&seat.channel)));
                out.push_str(&format!("  lang = {}\n", quote(&seat.lang)));
                if let Some(gender) = &seat.gender {
                    out.push_str(&format!("  gender = {}\n", quote(gender)));
                }
            }
        }

        for person in &self.people {
            out.push_str("\n[[person]]\n");
            out.push_str(&format!("id = {}\n", quote(&person.id)));
            out.push_str(&format!("gender = {}\n", quote(&person.gender)));
        }
        out
    }

    /// Prüft das Dokument mit derselben Prüfung wie jede Einstellungsdatei und
    /// liefert den Text, der gespeichert werden darf, samt den gelesenen
    /// Einstellungen.
    pub fn check(&self) -> Result<(String, Config), String> {
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
