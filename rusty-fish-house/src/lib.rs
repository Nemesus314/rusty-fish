//! Rusty Fish, der Haushalt.
//!
//! Zwischen der Raum-Logik und einer Tür liegt, was jeder Fisch braucht, egal
//! durch welche Tür er spricht: das Profil des Gastgebers, was der Fisch sich
//! über Neustarts merkt, seine festen Texte in drei Sprachen und der Ablauf
//! einer Nachricht von der Ankunft bis zu dem, was abzusetzen ist.
//!
//! Bis zum 21.11.2026 stand das alles in der Discord-Tür. Nemesus an dem Tag:
//! „Aber genau deshalb halte ich es für wichtig, den Fisch so zu bauen, dass
//! er schon jetzt eines Basis für Rusty Fish Server und das spätere CLU573R$
//! ist." Eine zweite Tür hätte es sonst abschreiben müssen.
//!
//! Diese Schicht kennt kein Netz. Eine Tür reicht herein, was ankam, und
//! setzt ab, was hier herauskommt (siehe [`household`]).
//!
//! Benannte Grenzen, die noch von der ersten Tür stammen:
//! - Menschen und Kanäle sind im Profil Discord-Kennungen, also Zahlen
//!   (siehe `config`). Für eine zweite Tür braucht die Person eine eigene
//!   Identität.
//! - Die Lesemarke je Kanal ist eine Zahl, die wächst (siehe `state`). Das
//!   stimmt bei Discord, nicht bei jeder Tür.

pub mod clock;
pub mod config;
pub mod document;
pub mod household;
pub mod invite;
pub mod place;
pub mod state;
pub mod texts;
