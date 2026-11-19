//! Wie eine Zustellung in Discord aussieht. Rein, ohne Netz.

use rusty_fish_room::{Delivery, Mark, Reason};

/// Discord nimmt höchstens 2000 Zeichen je Nachricht. Etwas Abstand, falls
/// Discord anders zählt als wir.
pub const MESSAGE_LIMIT: usize = 1900;

/// Text einer Zustellung samt ehrlicher Kennzeichnung. `-# ` ist in Discord
/// eine kleine graue Zeile.
pub fn render(delivery: &Delivery) -> String {
    match &delivery.mark {
        Mark::Translated | Mark::Original | Mark::Notice => delivery.text.clone(),
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

// ---------------------------------------------------------------------------
// Der Hinweis an den Gastgeber, wenn das Übersetzungskontingent leer ist
//
// Nemesus am 19.11.2026: „Meine Idee wäre eine Systemnachricht, der nur an den
// Gastgeber geht. Gleich mit Abfage, ob er es wieder um den selben Betrag
// erhöhen soll, mit Beitragnennung".
// ---------------------------------------------------------------------------

/// Eine Zahl mit Tausendertrennern, wie im Englischen üblich: 100,000.
pub fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// Die Direktnachricht an den Gastgeber. `allowed` ist, was heute insgesamt
/// erlaubt war, `step` der eingestellte Tagesbetrag, um den erhöht würde.
/// Genannt werden Tokens, kein Geld: der Fisch kennt die Preise der Anbieter nicht.
pub fn budget_notice(allowed: u64, step: u64) -> String {
    format!(
        "Rusty Fish: today's translation budget of {} tokens is used up. Messages still \
         arrive, untranslated and marked. Do you want to raise today's budget by another {} \
         tokens?",
        grouped(allowed),
        grouped(step)
    )
}

/// Beschriftung des Knopfs zum Erhöhen, mit Betrag.
pub fn raise_label(step: u64) -> String {
    format!("Raise by {}", grouped(step))
}

/// Beschriftung des Knopfs, mit dem der Gastgeber es dabei belässt.
pub const KEEP_LABEL: &str = "Leave it";

/// Bestätigung nach dem Erhöhen, mit dem neuen Tagesstand.
pub fn raised_notice(allowed: u64, left: u64) -> String {
    format!(
        "Raised. Today's translation budget is now {} tokens, {} of them are left. \
         Translation continues with the next message.",
        grouped(allowed),
        grouped(left)
    )
}

/// Antwort, wenn es nichts zu erhöhen gibt: ein zweiter Klick, oder ein neuer
/// Tag hat inzwischen begonnen.
pub fn nothing_to_raise(allowed: u64, left: u64) -> String {
    format!(
        "Nothing to raise: {} of today's {} tokens are still left.",
        grouped(left),
        grouped(allowed)
    )
}

/// Antwort, wenn der Gastgeber es dabei belässt.
pub const KEPT_NOTICE: &str = "Left as it is. Messages keep arriving, untranslated and marked, \
    until the new day begins (UTC).";

/// Antwort an jemanden, der nicht der Gastgeber ist und trotzdem drückt.
pub const NOT_THE_HOST: &str = "Only the host can decide this.";

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

/// Der Name des KI-Dienstes, so wie ein Mensch ihn wiedererkennt: der
/// Rechnername aus der Adresse, ohne Protokoll und Pfad.
pub fn service_name(base_url: &str) -> String {
    let url = base_url.trim();
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).unwrap_or(url);
    rest.split('/').next().unwrap_or(rest).to_owned()
}

/// Der Text, mit dem der Fisch um Zustimmung bittet. Er wird in die Sprache
/// des Platzes übersetzt; er enthält kein Wort eines Menschen, deshalb darf
/// er vor der Zustimmung zur KI.
///
/// Entschieden von Nemesus am 19.11.2026 („Ja, ergänzen"): die Bitte sagt,
/// dass der Gastgeber den KI-Dienst gewählt hat, und nennt ihn. Wer zustimmt,
/// soll wissen, wohin seine Sätze gehen.
pub fn consent_text(service: &str) -> String {
    // Bis zum 19.11.2026 hieß es nur „an AI translates in between. To do
    // that, the AI reads what you write here", ohne zu sagen, wessen KI.
    format!(
        "Hello! I am Rusty Fish, a translation bot. In this channel everyone writes in their \
         own language, and an AI translates in between. The AI is an outside service chosen by \
         the host of this server ({service}). To translate, it reads what you write here. \
         Nothing you write is sent to it before you agree. Press OK to agree. Until then your \
         messages are passed on untranslated."
    )
}
