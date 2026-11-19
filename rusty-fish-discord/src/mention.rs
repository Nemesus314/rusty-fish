//! Die echte Discord-Erwähnung als Ansprache. Rein, ohne Netz.
//!
//! Wer in Discord `@` tippt und den Bot aus der Liste wählt, schreibt im Text
//! der Nachricht `<@ID>` (in alten Fassungen `<@!ID>`), mit der Benutzer-ID
//! des Bots. Das geht mit jeder Tastatur, auch mit einer kyrillischen, und
//! zählt deshalb wie der Rufname am Anfang (entschieden am 19.11.2026).
//!
//! Die Raum-Logik kennt keine Discord-Kürzel. Die Tür übersetzt die Erwähnung
//! deshalb in das, was die Raum-Logik versteht: den Rufnamen am Anfang. So
//! gelten Kontingent, Zustimmung und alles andere unverändert, und weder die
//! KI noch die anderen Plätze bekommen das Kürzel zu sehen.

/// Beginnt der Text mit der Erwähnung genau dieses Bots? Wenn ja, kommt der
/// Rest dahinter zurück, die Frage.
///
/// Eine Erwähnung mitten im Satz, die Erwähnung eines anderen Benutzers oder
/// einer Rolle (`<@&ID>`) und eine Erwähnung ohne Text dahinter sprechen
/// nicht an.
pub fn mention_question(text: &str, bot_id: u64) -> Option<&str> {
    // Solange der Bot seine eigene ID nicht kennt, spricht ihn nichts an.
    if bot_id == 0 {
        return None;
    }
    let rest = text.trim_start().strip_prefix("<@")?;
    let rest = rest.strip_prefix('!').unwrap_or(rest);
    // Die ID muss ganz stimmen, bis zur schließenden Klammer: `<@420>` ist
    // nicht `<@42>`, und `<@&42>` ist eine Rolle.
    let rest = rest.strip_prefix(bot_id.to_string().as_str())?.strip_prefix('>')?;
    // Nach dem Kürzel darf ein Komma oder Doppelpunkt stehen, wie nach dem
    // Rufnamen. Nötig ist es nicht: das Kürzel endet von selbst.
    let question =
        rest.trim_start_matches(|c: char| c.is_whitespace() || c == ',' || c == ':').trim_end();
    if question.is_empty() { None } else { Some(question) }
}

/// Der Text, wie ihn die Raum-Logik bekommt: aus „<@ID> wie spät?" wird
/// „Fish, wie spät?". Ohne Erwähnung am Anfang kommt nichts zurück, und der
/// Text bleibt, wie er ist.
pub fn mention_as_call(text: &str, bot_id: u64, name: &str) -> Option<String> {
    let question = mention_question(text, bot_id)?;
    Some(format!("{}, {question}", name.trim()))
}
