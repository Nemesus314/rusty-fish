//! Was der Aufrufer einer Tür übergibt und was er ihr im Lauf sagen kann.
//!
//! Bis zum 22.11.2026 standen diese Typen in der Discord-Tür. Mit der zweiten
//! Tür gehören sie hierher: beide Türen nehmen dasselbe Profil entgegen und
//! verstehen dieselben Worte, und das Programm muss nicht wissen, an welche
//! Tür es gerade spricht.

use crate::between::Shared;
use crate::config::Config;
use std::path::PathBuf;

/// Ein Profil, wie der Aufrufer es einer Tür übergibt: fertig geprüft, mit der
/// KI, die für es übersetzt.
pub struct Profile<M> {
    /// Der Name des Profils. `None`, wenn der Fisch mit einer einzelnen
    /// Einstellungsdatei gestartet wurde.
    pub name: Option<String>,
    pub config: Config,
    pub state_path: PathBuf,
    pub model: M,
    /// Der Name des KI-Dienstes, zu dem `model` spricht. Er kommt vom
    /// Aufrufer, weil nur der die Adresse kennt, die wirklich benutzt wird.
    pub service: String,
    /// Steht das Profil an mehr als einer Tür: was es dort mit den anderen
    /// teilt. `None` heißt: es steht an einer Tür, und die führt seinen
    /// Haushalt allein, wie von Anfang an.
    pub shared: Option<Shared>,
}

/// Was der Aufrufer einer laufenden Tür sagen kann.
pub enum Command<M> {
    /// Ein weiteres Profil bedienen, ohne die laufenden zu stören.
    Start(Box<Profile<M>>),
    /// Ein Profil neu aufnehmen, etwa nachdem es geändert wurde: das laufende
    /// gleichen Namens geht, dieses kommt. Die Tür hält dabei nie an, auch
    /// wenn es ihr einziges Profil ist.
    Restart(Box<Profile<M>>),
    /// Ein Profil nicht mehr bedienen. War es das letzte, hält die Tür an.
    Stop(String),
    /// Anhalten.
    StopAll,
}

/// Wird gerufen, wenn sich ändert, welche Profile eine Tür bedient, mit deren
/// Namen. So kann der Aufrufer es anderen sagen, etwa der Maske.
pub type OnChange = Box<dyn Fn(&[String]) + Send + Sync>;

/// Wie ein Profil in der Programmausgabe heißt: `[tea-time] ` vor der Zeile,
/// oder nichts, wenn es keinen Namen hat.
pub fn label_of(name: Option<&str>) -> String {
    name.map(|name| format!("[{name}] ")).unwrap_or_default()
}

/// Wie ein Profil in einer Fehlermeldung heißt.
pub fn title_of(name: Option<&str>) -> String {
    match name {
        Some(name) => format!("profile \"{name}\""),
        None => "the running settings file".to_owned(),
    }
}
