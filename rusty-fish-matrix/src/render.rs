//! Wie eine Zustellung in Matrix aussieht. Rein, ohne Netz.
//!
//! Der Wortlaut der festen Texte steht in `rusty-fish-house`. Hier steht nur,
//! was Matrix eigen ist.
//!
//! - Bei Matrix kann der Fisch nicht unter fremdem Namen schreiben; so etwas
//!   wie die Webhooks von Discord gibt es dort nicht. Jede Zustellung ist eine
//!   Nachricht des Fischs, und der Name des Schreibenden steht davor.
//! - Eine Nachricht hat zwei Fassungen: reinen Text und HTML. Die Programme
//!   zeigen das HTML, wenn sie es können.
//! - Der Fisch sendet als „Notiz" (`m.notice`). Matrix kennt kein Kennzeichen
//!   für Bots am Konto; die Regel dort heißt: Bots senden Notizen und
//!   antworten nie auf eine. Daran erkennt ein zweiter Fisch im selben Raum,
//!   was er zu überlesen hat.

use rusty_fish_house::texts::{chunks, untranslated_note};
use rusty_fish_room::Delivery;
use rusty_fish_room::tongue::Tongue;

/// So viele Zeichen Text je Nachricht. Matrix nimmt ein Ereignis bis 65536
/// Bytes an, und der Text steht zweimal darin, als reiner Text und als HTML.
pub const MESSAGE_LIMIT: usize = 4000;

/// Eine Nachricht, wie sie hinausgeht: reiner Text und dasselbe als HTML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub body: String,
    pub html: String,
}

/// Eine Zustellung als eine oder, wenn sie sehr lang ist, mehrere Nachrichten.
/// Vor jeder steht der Name dessen, von dem sie stammt; die ehrliche
/// Kennzeichnung einer unübersetzten Zustellung steht unter der letzten, in
/// der Sprache des Tisches, an dem sie erscheint.
pub fn render_in(delivery: &Delivery, tongue: Tongue, name: &str) -> Vec<Rendered> {
    let name = shown_name(name);
    let note = untranslated_note(&delivery.mark, tongue);
    let mut parts = chunks(&delivery.text, MESSAGE_LIMIT);
    // Auch ein leerer Text ist eine Zustellung, etwa wenn nur die
    // Kennzeichnung etwas sagt.
    if parts.is_empty() {
        parts.push(String::new());
    }
    let last = parts.len() - 1;
    parts
        .iter()
        .enumerate()
        .map(|(n, part)| {
            let mut body = format!("{name}: {part}");
            let mut html = format!("<strong>{}</strong>: {}", escape(&name), escape(part));
            if let (true, Some(note)) = (n == last, &note) {
                body.push_str(&format!("\n({note})"));
                html.push_str(&format!("<br><em>{}</em>", escape(note)));
            }
            Rendered { body, html }
        })
        .collect()
}

/// Ein fester Text des Fischs, etwa die Bitte um Zustimmung: derselbe Text in
/// beiden Fassungen.
pub fn plain(text: &str) -> Rendered {
    Rendered { body: text.to_owned(), html: escape(text) }
}

/// Der Name, wie er vor einer Zustellung stehen darf: in einer Zeile und
/// nicht beliebig lang. Ein Anzeigename ist frei gewählt; ein Zeilenumbruch
/// darin würde die Zeile zerreißen. Ein leerer wird zu „Guest".
pub fn shown_name(name: &str) -> String {
    let one_line = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.is_empty() { "Guest".to_owned() } else { one_line.chars().take(80).collect() }
}

/// Text als HTML: die Zeichen, die in HTML etwas bedeuten, werden maskiert,
/// und ein Zeilenende wird ein Umbruch. So kann kein Text eines Menschen als
/// Auszeichnung gelesen werden.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("<br>"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out
}

/// Der Text einer Antwort ohne das vorangestellte Zitat.
///
/// Ältere Programme stellen einer Antwort den Satz voran, auf den sie
/// antwortet, jede Zeile mit `> ` davor, dann eine Leerzeile. Das ist ein
/// Satz eines anderen Menschen und würde sonst als Wort des Antwortenden
/// übersetzt. Neuere Programme senden das Zitat nicht mehr; dann bleibt der
/// Text, wie er ist. Nur aufzurufen, wenn die Nachricht eine Antwort ist: ein
/// Mensch kann auch selbst mit `> ` beginnen.
pub fn without_quote(body: &str) -> &str {
    let mut rest = body;
    let mut quoted = false;
    while let Some(line_end) = rest.find('\n') {
        let line = &rest[..line_end];
        if line.starts_with("> ") || line == ">" {
            quoted = true;
            rest = &rest[line_end + 1..];
        } else if quoted && line.trim().is_empty() {
            // Die Leerzeile hinter dem Zitat gehört noch dazu.
            return &rest[line_end + 1..];
        } else {
            break;
        }
    }
    // Ein Zitat ohne Leerzeile und Text dahinter war keines: nichts abschneiden.
    body
}
