//! Rusty Fish, die Matrix-Tür.
//!
//! Die Tür ist dünn, wie die zu Discord. Was ein Raum ist und wer was in
//! welcher Sprache bekommt, steht in `rusty-fish-room`. Das Profil, was der
//! Fisch sich merkt, seine festen Texte und der Ablauf einer Nachricht stehen
//! in `rusty-fish-house`. Hier steht nur, was Matrix eigen ist: das Konto und
//! seine Sitzung, Nachrichten hereinreichen, Zustellungen mit dem Namen des
//! Schreibenden davor absetzen, die Zeichen unter einer Nachricht, die bei
//! Matrix die Knöpfe ersetzen, und die Erwähnung des Fischs.

pub mod door;
pub mod mention;
pub mod render;
pub mod session;
pub mod signs;
