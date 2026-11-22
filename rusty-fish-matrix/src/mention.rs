//! Die Erwähnung des Fischs als Ansprache. Rein, ohne Netz.
//!
//! Wer in einem Matrix-Programm `@` tippt und den Fisch aus der Liste wählt,
//! schreibt in den Text der Nachricht dessen Anzeigenamen (in manchen
//! Programmen seine Kennung) und in ein eigenes Feld der Nachricht, wen er
//! meint. Das geht mit jeder Tastatur und zählt deshalb wie der Rufname am
//! Anfang, so wie die Erwähnung bei Discord (entschieden am 19.11.2026).
//!
//! Die Raum-Logik kennt keine Erwähnungen. Die Tür übersetzt sie in das, was
//! die Raum-Logik versteht: den Rufnamen am Anfang.

/// Beginnt der Text mit einer Erwähnung des Fischs? Wenn ja, kommt der Rest
/// dahinter zurück, die Frage.
///
/// - `mentioned`: die Nachricht nennt den Fisch in ihrem Feld für Erwähnungen.
/// - `own`: die Namen, unter denen der Fisch im Text stehen kann: sein
///   Anzeigename in diesem Raum, der vordere Teil seiner Kennung und die
///   ganze Kennung (`@name:server`).
///
/// Die ganze Kennung am Anfang zählt auch ohne das Feld: sie kann niemand
/// anderen meinen, und ältere Programme setzen das Feld nicht. Ein bloßer
/// Name zählt nur mit dem Feld; sonst spräche jeder Satz, der zufällig mit
/// dem Anzeigenamen des Fischs beginnt, ihn an.
///
/// Eine Erwähnung mitten im Satz und eine ohne Text dahinter sprechen nicht an.
pub fn mention_question<'a>(text: &'a str, mentioned: bool, own: &[&str]) -> Option<&'a str> {
    let start = text.trim_start();
    // Der längste Name zuerst: „Rusty Fish" vor „Rusty".
    let mut names: Vec<&str> = own.iter().map(|n| n.trim()).filter(|n| !n.is_empty()).collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.chars().count()));
    for name in names {
        let is_id = name.starts_with('@') && name.contains(':');
        if !mentioned && !is_id {
            continue;
        }
        // Vor einem bloßen Namen darf ein `@` stehen, wie man es tippt.
        let candidates = [Some(start), start.strip_prefix('@').filter(|_| !is_id)];
        for candidate in candidates.into_iter().flatten() {
            let Some(rest) = strip_prefix_ignoring_case(candidate, name) else { continue };
            // Der Name muss zu Ende sein: „Fishing" ist nicht „Fish".
            if !rest.is_empty() && !rest.starts_with(|c: char| c.is_whitespace() || c == ',' || c == ':')
            {
                continue;
            }
            let question = rest
                .trim_start_matches(|c: char| c.is_whitespace() || c == ',' || c == ':')
                .trim_end();
            if !question.is_empty() {
                return Some(question);
            }
        }
    }
    None
}

/// Der Text, wie ihn die Raum-Logik bekommt: aus „Rusty Fish: wie spät?" wird
/// „Fish, wie spät?". Ohne Erwähnung am Anfang kommt nichts zurück, und der
/// Text bleibt, wie er ist.
pub fn mention_as_call(text: &str, mentioned: bool, own: &[&str], name: &str) -> Option<String> {
    let question = mention_question(text, mentioned, own)?;
    Some(format!("{}, {question}", name.trim()))
}

/// Wie `str::strip_prefix`, aber ohne auf Groß- und Kleinschreibung zu achten.
fn strip_prefix_ignoring_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let mut rest = text.char_indices();
    for wanted in prefix.chars() {
        let (_, got) = rest.next()?;
        if !got.to_lowercase().eq(wanted.to_lowercase()) {
            return None;
        }
    }
    Some(match rest.next() {
        Some((at, _)) => &text[at..],
        None => "",
    })
}
