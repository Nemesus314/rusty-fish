//! Wie eine Zustellung in Discord aussieht. Rein, ohne Netz.
//!
//! Der Wortlaut der festen Texte steht in `rusty-fish-house`. Hier steht nur,
//! was Discord eigen ist: die graue Zeile, die Länge einer Nachricht und
//! welche Namen ein Webhook tragen darf.

use rusty_fish_house::texts::untranslated_note;

// Das Teilen eines langen Textes braucht jede Tür; es steht seit dem
// 22.11.2026 im Haus und bleibt unter diesem Namen hier erreichbar.
pub use rusty_fish_house::texts::chunks;
use rusty_fish_room::Delivery;
use rusty_fish_room::tongue::Tongue;

/// Discord nimmt höchstens 2000 Zeichen je Nachricht. Etwas Abstand, falls
/// Discord anders zählt als wir.
pub const MESSAGE_LIMIT: usize = 1900;

/// Text einer Zustellung samt ehrlicher Kennzeichnung, auf Englisch. Die Tür
/// nimmt [`render_in`], damit die Zeile in der Sprache des Tisches steht; das
/// hier ist dessen englische Fassung, so wie sie bis zum 20.11.2026 an jedem
/// Tisch stand.
pub fn render(delivery: &Delivery) -> String {
    render_in(delivery, Tongue::English)
}

/// Text einer Zustellung samt ehrlicher Kennzeichnung, in der Sprache des
/// Tisches, an dem sie erscheint. `-# ` ist in Discord eine kleine graue Zeile.
pub fn render_in(delivery: &Delivery, tongue: Tongue) -> String {
    match untranslated_note(&delivery.mark, tongue) {
        None => delivery.text.clone(),
        Some(note) => format!("{}\n-# {note}", delivery.text),
    }
}

/// Name, unter dem eine Zustellung erscheint. Discord lehnt manche Namen ab.
pub fn hook_name(name: &str) -> String {
    let mut out: String = name.trim().chars().take(80).collect();
    // Discord verbietet diese Wörter in Namen von Webhooks.
    for banned in ["discord", "clyde"] {
        while let Some(at) = out.to_ascii_lowercase().find(banned) {
            // Der zweite Buchstabe wird ersetzt; beide Wörter sind reines ASCII.
            out.replace_range(at + 1..at + 2, "-");
        }
    }
    if out.trim().is_empty() { "Guest".to_owned() } else { out }
}
