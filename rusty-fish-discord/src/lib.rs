//! Rusty Fish, die Discord-Tür.
//!
//! Die Tür ist dünn. Was ein Raum ist und wer was in welcher Sprache bekommt,
//! steht in `rusty-fish-room`. Hier steht nur, was Discord eigen ist:
//! Einstellungen lesen, Nachrichten hereinreichen, Zustellungen unter dem
//! Namen des Schreibenden absetzen, die Zustimmung einholen, Verpasstes nachholen.

pub mod clock;
pub mod config;
pub mod door;
pub mod render;
pub mod state;
