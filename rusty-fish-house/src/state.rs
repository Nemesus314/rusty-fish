//! Was die Tür über Neustarts hinweg wissen muss: wer zugestimmt hat und bis
//! wohin jeder Kanal schon verarbeitet ist. Eine kleine JSON-Datei.
//!
//! Keine Nachrichtentexte: der Fisch speichert kein Gespräch.

use rusty_fish_room::Tally;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Store {
    #[serde(skip)]
    path: PathBuf,
    #[serde(default)]
    consented: BTreeSet<String>,
    #[serde(default)]
    last_seen: BTreeMap<String, u64>,
    /// Das Kontingent des laufenden Tages: Datum, verbrauchte Tokens, Fragen
    /// je Gast. Kennungen und Zahlen, kein Text.
    #[serde(default)]
    day: String,
    #[serde(default)]
    translation_tokens: u64,
    #[serde(default)]
    questions: BTreeMap<String, u32>,
    /// Was der Gastgeber heute nachgelegt hat, und ob er über die laufende
    /// Erschöpfung schon benachrichtigt ist.
    #[serde(default)]
    raised_tokens: u64,
    #[serde(default)]
    host_notified: bool,
    /// Wem der Fisch schon mitgeteilt hat, dass er eingeladen ist. Nur die
    /// Kennungen, damit die Mitteilung nach einem Neustart nicht erneut kommt.
    ///
    /// Leer wird das Feld nicht geschrieben: solange niemand eingeladen
    /// wurde, sieht die Datei aus wie vor dem 20.11.2026, und ein älteres
    /// Programm, das unbekannte Felder ablehnt, liest sie weiter.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    invited_told: BTreeSet<String>,
    /// Welchem KI-Dienst jemand zugestimmt hat, nach Kennung: der Name des
    /// Dienstes, so wie die Bitte um Zustimmung ihn nannte.
    ///
    /// Seit dem 21.11.2026. Die Bitte sagt, wohin die Sätze gehen; wechselt
    /// der Gastgeber den Dienst, gilt die Zustimmung dem neuen nicht.
    ///
    /// Wer in `consented` steht, aber nicht hier, hat zugestimmt, bevor der
    /// Dienst gemerkt wurde (siehe [`Store::settle_service`]). Leer wird das
    /// Feld nicht geschrieben, aus demselben Grund wie `invited_told`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    consented_for: BTreeMap<String, String>,
}

impl Store {
    /// Öffnet die Datei. Fehlt sie, beginnt der Fisch leer. Ist sie
    /// beschädigt, scheitert er laut, statt Zustimmungen still zu vergessen.
    pub fn open(path: &Path) -> Result<Store, String> {
        let mut store = if path.exists() {
            let raw = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            serde_json::from_str::<Store>(&raw)
                .map_err(|e| format!("{} is damaged: {e}", path.display()))?
        } else {
            Store::default()
        };
        store.path = path.to_owned();
        Ok(store)
    }

    /// Hat dieser Mensch zugestimmt, dass dieser KI-Dienst seine Sätze liest?
    ///
    /// Eine Zustimmung, zu der kein Dienst gemerkt ist, gilt: sie stammt von
    /// vor dem 21.11.2026.
    pub fn has_consented(&self, user: &str, service: &str) -> bool {
        self.consented.contains(user)
            && self.consented_for.get(user).is_none_or(|agreed| agreed == service)
    }

    /// Dieser Mensch stimmt diesem KI-Dienst zu. Eine frühere Zustimmung zu
    /// einem anderen Dienst ist damit ersetzt: gemerkt wird ein Dienst je Mensch.
    pub fn consent(&mut self, user: &str, service: &str) {
        self.consented.insert(user.to_owned());
        self.consented_for.insert(user.to_owned(), service.to_owned());
    }

    /// Trägt zu einer Zustimmung ohne gemerkten Dienst den Dienst nach, der
    /// jetzt läuft. Wer nicht zugestimmt hat oder schon einen Dienst trägt,
    /// bleibt, wie er ist.
    ///
    /// Benannte Grenze: welchem Dienst so jemand damals wirklich zugestimmt
    /// hat, weiß der Fisch nicht. Er nimmt den ersten, mit dem er diesen
    /// Menschen nach dem 21.11.2026 schreiben sieht.
    pub fn settle_service(&mut self, user: &str, service: &str) {
        if self.consented.contains(user) && !self.consented_for.contains_key(user) {
            self.consented_for.insert(user.to_owned(), service.to_owned());
        }
    }

    /// Letzte verarbeitete Nachricht dieses Kanals.
    pub fn seen(&self, channel: &str) -> Option<u64> {
        self.last_seen.get(channel).copied()
    }

    /// Merkt sich eine verarbeitete Nachricht. Geht nie rückwärts.
    pub fn saw(&mut self, channel: &str, message: u64) {
        let seen = self.last_seen.entry(channel.to_owned()).or_insert(0);
        *seen = (*seen).max(message);
    }

    /// Wem die Einladung schon mitgeteilt wurde.
    pub fn told(&self) -> &BTreeSet<String> {
        &self.invited_told
    }

    /// Ersetzt die Liste derer, denen die Einladung mitgeteilt wurde.
    pub fn set_told(&mut self, told: BTreeSet<String>) {
        self.invited_told = told;
    }

    /// Der Tageszähler, so wie er zuletzt gespeichert wurde.
    pub fn tally(&self) -> Tally {
        Tally {
            day: self.day.clone(),
            translation_tokens: self.translation_tokens,
            questions: self.questions.clone(),
            raised_tokens: self.raised_tokens,
            host_notified: self.host_notified,
        }
    }

    /// Übernimmt den Tageszähler, damit er einen Neustart überlebt.
    pub fn set_tally(&mut self, tally: &Tally) {
        self.day = tally.day.clone();
        self.translation_tokens = tally.translation_tokens;
        self.questions = tally.questions.clone();
        self.raised_tokens = tally.raised_tokens;
        self.host_notified = tally.host_notified;
    }

    /// Schreibt die Datei so, dass ein Absturz mittendrin sie nicht zerstört:
    /// erst daneben schreiben, dann umbenennen.
    pub fn save(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("cannot write the state: {e}"))?;
        let mut tmp = self.path.clone().into_os_string();
        tmp.push(".tmp");
        std::fs::write(&tmp, json)
            .and_then(|()| std::fs::rename(&tmp, &self.path))
            .map_err(|e| format!("cannot write {}: {e}", self.path.display()))
    }
}
