//! Was der Fisch selbst sagt: seine festen Texte. Rein, ohne Netz.
//!
//! Wie ein Text an einer Tür aussieht (eine graue Zeile, ein Knopf, die
//! Länge einer Nachricht), entscheidet die Tür. Hier steht nur der Wortlaut.

use crate::config::Door;
use rusty_fish_room::tongue::Tongue;
use rusty_fish_room::{BUDGET_USED_UP, Mark, Reason};

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

/// Der Name des KI-Dienstes, so wie ein Mensch ihn wiedererkennt: der
/// Rechnername aus der Adresse, ohne Protokoll und Pfad.
pub fn service_name(base_url: &str) -> String {
    let url = base_url.trim();
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).unwrap_or(url);
    rest.split('/').next().unwrap_or(rest).to_owned()
}

/// Der Text, mit dem der Fisch um Zustimmung bittet, auf Englisch. Auf Deutsch
/// und Russisch gibt es ihn fest ([`consent_text_in`]). Für jede andere Sprache
/// wird diese Fassung von der KI übersetzt; sie enthält kein Wort eines
/// Menschen, deshalb darf sie vor der Zustimmung zur KI.
///
/// Entschieden von Nemesus am 19.11.2026 („Ja, ergänzen"): die Bitte sagt,
/// dass der Gastgeber den KI-Dienst gewählt hat, und nennt ihn. Wer zustimmt,
/// soll wissen, wohin seine Sätze gehen.
pub fn consent_text(service: &str) -> String {
    consent_text_at(Door::Discord, Tongue::English, service)
}

/// Die Wörter, in denen sich die festen Texte je Tür unterscheiden.
///
/// Seit dem 22.11.2026, mit der Matrix-Tür. Bei Discord steht ein Tisch in
/// einem Kanal auf dem Server des Gastgebers. Bei Matrix ist er ein Raum, und
/// der Server gehört meist jemand anderem: „der Gastgeber dieses Servers"
/// würde dort auf den Falschen zeigen. Der Knopf „OK" ist bei Matrix ein
/// Zeichen unter der Nachricht, das man antippt.
struct Words {
    /// „In diesem Kanal", am Satzanfang.
    here: &'static str,
    /// Wer den KI-Dienst gewählt hat, eingepasst in den Satz der Sprache.
    chosen_by: &'static str,
    /// „Drücke OK".
    press: &'static str,
    /// „dieses Kanals", als Genitiv hinter „Sprache".
    of_here: &'static str,
}

fn words(door: Door, tongue: Tongue) -> Words {
    match (door, tongue) {
        (Door::Discord, Tongue::English) => Words {
            here: "In this channel",
            chosen_by: "the host of this server",
            press: "Press OK",
            of_here: "of this channel",
        },
        (Door::Matrix, Tongue::English) => Words {
            here: "In this room",
            chosen_by: "the host who runs this bot",
            press: "Press OK below this message",
            of_here: "of this room",
        },
        (Door::Discord, Tongue::German) => Words {
            here: "In diesem Kanal",
            chosen_by: "den der Gastgeber dieses Servers gewählt hat",
            press: "Drücke OK",
            of_here: "dieses Kanals",
        },
        (Door::Matrix, Tongue::German) => Words {
            here: "In diesem Raum",
            chosen_by: "den der Gastgeber gewählt hat, der diesen Bot betreibt",
            press: "Drücke OK unter dieser Nachricht",
            of_here: "dieses Raums",
        },
        (Door::Discord, Tongue::Russian) => Words {
            here: "В этом канале",
            chosen_by: "хозяин этого сервера",
            press: "Нажми OK",
            of_here: "этого канала",
        },
        (Door::Matrix, Tongue::Russian) => Words {
            here: "В этой комнате",
            chosen_by: "хозяин этого бота",
            press: "Нажми OK под этим сообщением",
            of_here: "этой комнаты",
        },
    }
}

// ---------------------------------------------------------------------------
// Die festen Texte des Fischs im Chat, in drei Sprachen
//
// Entschieden von Nemesus am 20.11.2026: „Daraus würde dann folgen, dass wir
// das Ding dreisprachig aufziehen. [...] Deutsch, Englisch(Standard) und
// Russisch." Gewählt wird nach der Sprache des Tisches, an dem der Text
// erscheint; jede andere Sprache bekommt Englisch.
//
// Ein fester Text braucht keine KI: er kommt auch an, wenn die KI ausfällt
// oder das Kontingent leer ist, und er kostet nichts.
//
// Benannte Grenze: das Russische hat noch keine Muttersprachlerin geprüft.
// ---------------------------------------------------------------------------

/// Die ehrliche Kennzeichnung einer Zustellung, die unübersetzt hinausgeht,
/// in der Sprache des Tisches, an dem sie erscheint: etwa „nicht übersetzt:
/// das heutige Kontingent für Übersetzungen ist verbraucht". Für alles, was
/// keine Kennzeichnung braucht, `None`.
///
/// Der technische Grund eines Fehlers bleibt, wie der Dienst ihn meldet;
/// übersetzt ist nur der feste Teil davor. Wie die Zeile aussieht, etwa
/// klein und grau, entscheidet die Tür.
pub fn untranslated_note(mark: &Mark, tongue: Tongue) -> Option<String> {
    let why: String = match mark {
        Mark::Translated | Mark::Original | Mark::Notice => return None,
        Mark::Untranslated(Reason::NoConsent) => no_consent(tongue).to_owned(),
        Mark::Untranslated(Reason::Failed(why)) => match budget_used_up(tongue) {
            // Erkannt am Grund, den der Zähler selbst setzt, wie in
            // `rusty_fish_room::budget_ran_out`.
            Some(fixed) if why.contains(BUDGET_USED_UP) => fixed.to_owned(),
            // Der Grund wird gekürzt, die Nachricht nie.
            _ => why.chars().take(120).map(|c| if c == '\n' { ' ' } else { c }).collect(),
        },
    };
    Some(format!("{}: {why}", not_translated(tongue)))
}

/// Der Anfang der grauen Zeile.
fn not_translated(tongue: Tongue) -> &'static str {
    match tongue {
        Tongue::German => "nicht übersetzt",
        Tongue::English => "not translated",
        Tongue::Russian => "не переведено",
    }
}

/// Der Grund, wenn der Schreibende der KI noch nicht zugestimmt hat. Ohne
/// „er" oder „sie": der Fisch weiß nicht, wer geschrieben hat.
fn no_consent(tongue: Tongue) -> &'static str {
    match tongue {
        Tongue::German => "wer das geschrieben hat, hat der KI-Übersetzung noch nicht zugestimmt",
        Tongue::English => "the writer has not agreed to AI translation yet",
        Tongue::Russian => "пока нет согласия автора на перевод с помощью ИИ",
    }
}

/// Der Grund, wenn das Kontingent des Tages verbraucht ist. Für Englisch
/// `None`: dort bleibt der Grund, wie der Zähler ihn setzt, er ist schon
/// englisch ([`BUDGET_USED_UP`]).
fn budget_used_up(tongue: Tongue) -> Option<&'static str> {
    match tongue {
        Tongue::German => Some("das heutige Kontingent für Übersetzungen ist verbraucht"),
        Tongue::English => None,
        Tongue::Russian => Some("лимит переводов на сегодня исчерпан"),
    }
}

/// Die Bitte um Zustimmung als fester Text, per du, mit dem Namen des
/// KI-Dienstes. Für jede andere Sprache übersetzt die KI die englische Fassung.
///
/// Die drei Fassungen sagen dasselbe wie [`consent_text`]: wer der Fisch ist,
/// dass eine KI übersetzt, wessen KI es ist, dass sie mitliest, dass vor der
/// Zustimmung nichts an sie geht, was bis dahin geschieht, und seit dem
/// 21.11.2026: dass die letzten Nachrichten nach der Zustimmung nachträglich
/// übersetzt werden. Wer zustimmt, stimmt damit auch dem zu. Der Knopf heißt
/// in jeder Sprache „OK".
///
/// Im Russischen ohne Vergangenheitsform des Schreibens: die hätte ein
/// Geschlecht, und der Fisch weiß nicht, wer liest.
pub fn consent_text_in(tongue: Tongue, service: &str) -> String {
    consent_text_at(Door::Discord, tongue, service)
}

/// Die Bitte um Zustimmung, wie sie an dieser Tür lautet. Für Discord ist es
/// Wort für Wort [`consent_text_in`]; die Unterschiede stehen in [`Words`].
pub fn consent_text_at(door: Door, tongue: Tongue, service: &str) -> String {
    let Words { here, chosen_by, press, .. } = words(door, tongue);
    match tongue {
        // Bis zum 19.11.2026 hieß es nur „an AI translates in between. To do
        // that, the AI reads what you write here", ohne zu sagen, wessen KI.
        Tongue::English => format!(
            "Hello! I am Rusty Fish, a translation bot. {here} everyone writes in their \
             own language, and an AI translates in between. The AI is an outside service chosen \
             by {chosen_by} ({service}). To translate, it reads what you write here. \
             Nothing you write is sent to it before you agree. {press} to agree. Until then \
             your messages are passed on untranslated; once you agree, the last of them are \
             translated after all."
        ),
        Tongue::German => format!(
            "Hallo! Ich bin Rusty Fish, ein Übersetzungsbot. {here} schreibt jeder in \
             seiner eigenen Sprache, und eine KI übersetzt dazwischen. Die KI ist ein fremder \
             Dienst, {chosen_by} ({service}). Zum Übersetzen \
             liest sie, was du hier schreibst. Nichts von dem, was du schreibst, geht an sie, \
             bevor du zustimmst. {press}, um zuzustimmen. Bis dahin werden deine Nachrichten \
             unübersetzt weitergegeben; nach deiner Zustimmung werden die letzten nachträglich \
             übersetzt."
        ),
        Tongue::Russian => format!(
            "Привет! Я Rusty Fish, бот-переводчик. {here} каждый пишет на своём языке, а \
             ИИ переводит. ИИ — это внешний сервис, который выбрал {chosen_by} \
             ({service}). Чтобы переводить, он читает то, что ты здесь пишешь. Ничего из того, \
             что ты пишешь, не отправляется ему, пока ты не согласишься. {press}, чтобы \
             согласиться. До тех пор твои сообщения передаются без перевода; после твоего \
             согласия последние из них будут переведены."
        ),
    }
}

/// Die Mitteilung an einen eingeladenen Menschen, an seinem Tisch.
///
/// Nemesus am 20.11.2026: „Es sollte auch die Funktion ‚Einladen' geben. Und
/// dann ein Haken, wer eingeladen wird. Das könnte dann per Systemnachricht
/// mitgeteilt werden."
///
/// `host` ist der Anzeigename des Gastgebers, soweit der Fisch ihn kennt.
/// Kennt er ihn nicht, steht dort niemand mit Namen. Die Mitteilung sagt, wer
/// einlädt, dass hier erscheint, was die anderen schreiben, in der Sprache
/// dieses Tisches, und wer die Übersetzung bezahlt.
///
/// Ohne „er" oder „sie" für den Gastgeber: der Fisch kennt nur dessen Namen.
/// Im Russischen steht deshalb mit Namen die Gegenwart („приглашает"), ohne
/// Namen die unpersönliche Form („тебя пригласили").
pub fn invitation(tongue: Tongue, host: Option<&str>) -> String {
    invitation_at(Door::Discord, tongue, host)
}

/// Die Mitteilung an einen Eingeladenen, wie sie an dieser Tür lautet. Für
/// Discord ist es Wort für Wort [`invitation`].
pub fn invitation_at(door: Door, tongue: Tongue, host: Option<&str>) -> String {
    // Für den, der es liest, ist sein Tisch ein Kanal oder ein Raum; so nennt
    // ihn auch die Bitte um Zustimmung.
    let of_here = words(door, tongue).of_here;
    match (tongue, shown_name(host)) {
        (Tongue::German, Some(host)) => format!(
            "Hallo! Ich bin Rusty Fish, ein Übersetzungsbot. {host} hat dich eingeladen: Ab jetzt \
             erscheint hier, was die anderen schreiben, in der Sprache {of_here}. Die Kosten \
             der Übersetzung trägt {host}."
        ),
        (Tongue::German, None) => format!(
            "Hallo! Ich bin Rusty Fish, ein Übersetzungsbot. Du bist \
             eingeladen: Ab jetzt erscheint hier, was die anderen schreiben, in der Sprache \
             {of_here}. Die Kosten der Übersetzung trägt, wer dich eingeladen hat."
        ),
        (Tongue::English, Some(host)) => format!(
            "Hello! I am Rusty Fish, a translation bot. {host} has invited you: from now on, what \
             the others write appears here in the language {of_here}. The translation is \
             paid for by {host}."
        ),
        (Tongue::English, None) => format!(
            "Hello! I am Rusty Fish, a translation bot. You have been \
             invited: from now on, what the others write appears here in the language \
             {of_here}. The translation is paid for by the person who invited you."
        ),
        (Tongue::Russian, Some(host)) => format!(
            "Привет! Я Rusty Fish, бот-переводчик. {host} приглашает тебя: теперь всё, что пишут \
             остальные, появляется здесь на языке {of_here}. Перевод оплачивает {host}."
        ),
        (Tongue::Russian, None) => format!(
            "Привет! Я Rusty Fish, бот-переводчик. Тебя пригласили: теперь \
             всё, что пишут остальные, появляется здесь на языке {of_here}. Перевод оплачивает \
             тот, кто тебя пригласил."
        ),
    }
}

/// Der Anzeigename, wie er in einer Mitteilung stehen darf: in einer Zeile
/// und nicht beliebig lang. Ein leerer Name ist kein Name.
///
/// Ein Anzeigename ist frei gewählt. Ein Zeilenumbruch darin würde die
/// Mitteilung zerreißen, ein sehr langer Name sie über die Grenze von Discord
/// schieben.
fn shown_name(name: Option<&str>) -> Option<String> {
    let one_line = name?.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.is_empty() {
        return None;
    }
    Some(one_line.chars().take(80).collect())
}

/// Teilt einen Text in Stücke von höchstens `limit` Zeichen, so lang, wie
/// eine Tür sie annimmt. Nichts geht verloren.
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
