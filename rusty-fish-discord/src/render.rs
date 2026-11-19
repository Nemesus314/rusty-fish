//! Wie eine Zustellung in Discord aussieht. Rein, ohne Netz.

use rusty_fish_room::{Delivery, Mark, Reason};

/// Discord nimmt höchstens 2000 Zeichen je Nachricht. Etwas Abstand, falls
/// Discord anders zählt als wir.
pub const MESSAGE_LIMIT: usize = 1900;

/// Text einer Zustellung samt ehrlicher Kennzeichnung. `-# ` ist in Discord
/// eine kleine graue Zeile.
pub fn render(delivery: &Delivery) -> String {
    match &delivery.mark {
        Mark::Translated | Mark::Original => delivery.text.clone(),
        Mark::Untranslated(Reason::NoConsent) => format!(
            "{}\n-# not translated: the writer has not agreed to AI translation yet",
            delivery.text
        ),
        Mark::Untranslated(Reason::Failed(why)) => {
            // Der Grund wird gekürzt, die Nachricht nie.
            let why: String = why.chars().take(120).map(|c| if c == '\n' { ' ' } else { c }).collect();
            format!("{}\n-# not translated: {why}", delivery.text)
        }
    }
}

/// Teilt einen Text in Stücke, die Discord annimmt. Nichts geht verloren.
pub fn chunks(text: &str, limit: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut parts = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let rest = chars.len() - start;
        let mut len = rest.min(limit);
        if rest > limit {
            // Lieber an einem Zeilenende oder Leerzeichen trennen als mitten im
            // Wort, solange das Stück dadurch nicht winzig wird.
            let window = &chars[start..start + limit];
            let cut = window
                .iter()
                .rposition(|c| *c == '\n')
                .or_else(|| window.iter().rposition(|c| *c == ' '));
            if let Some(i) = cut {
                if i + 1 >= limit / 2 {
                    len = i + 1;
                }
            }
        }
        parts.push(chars[start..start + len].iter().collect());
        start += len;
    }
    parts
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
