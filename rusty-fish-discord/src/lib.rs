//! Rusty Fish, die Discord-Tür.
//!
//! Die Tür ist dünn. Was ein Raum ist und wer was in welcher Sprache bekommt,
//! steht in `rusty-fish-room`. Das Profil, was der Fisch sich merkt, seine
//! festen Texte und der Ablauf einer Nachricht stehen in `rusty-fish-house`.
//! Hier steht nur, was Discord eigen ist: die Verbindung, Nachrichten
//! hereinreichen, Zustellungen unter dem Namen des Schreibenden absetzen,
//! Knöpfe, die Erwähnung des Bots, Verpasstes nachholen.

pub mod door;
pub mod mention;
pub mod render;
