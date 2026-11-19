//! Was die Tür über Neustarts hinweg wissen muss: wer zugestimmt hat und bis
//! wohin jeder Kanal schon verarbeitet ist. Eine kleine JSON-Datei.
//!
//! Keine Nachrichtentexte: der Fisch speichert kein Gespräch.

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

    pub fn has_consented(&self, user: &str) -> bool {
        self.consented.contains(user)
    }

    pub fn consent(&mut self, user: &str) {
        self.consented.insert(user.to_owned());
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
