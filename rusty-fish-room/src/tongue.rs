//! Die drei Sprachen, in denen der Fisch selbst spricht.
//!
//! Was Menschen einander schreiben, übersetzt die KI in jede Sprache. Was der
//! Fisch selbst sagt (eine Absage, eine graue Zeile unter einer unübersetzten
//! Nachricht, die Bitte um Zustimmung), steht dagegen fest im Programm, auf
//! Deutsch, Englisch und Russisch. Ein fester Text braucht keine KI und kommt
//! deshalb auch an, wenn die KI ausfällt oder das Kontingent leer ist.
//!
//! Entschieden von Nemesus am 20.11.2026: „Daraus würde dann folgen, dass wir
//! das Ding dreisprachig aufziehen. (Bei Bismarck waren das Deutsch und
//! Englisch, bei Rusty Fish wären das Deutsch, Englisch(Standard) und
//! Russisch."
//!
//! Benannte Grenze: das Russische hat noch keine Muttersprachlerin geprüft.

/// Eine der drei Sprachen der festen Texte. Englisch ist der Rückfall für
/// jede andere Sprache eines Tisches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tongue {
    German,
    English,
    Russian,
}

/// Ordnet die Sprache eines Tisches einer der drei Sprachen zu.
///
/// Im Profil steht die Sprache als freier Text, so wie ein Mensch sie
/// eintippt: "German", "Deutsch", "de", "Русский". Erkannt werden die
/// üblichen Schreibweisen; alles andere bekommt Englisch.
pub fn tongue_of(lang: &str) -> Tongue {
    known_tongue(lang).unwrap_or(Tongue::English)
}

/// Wie [`tongue_of`], aber ohne Rückfall: `None`, wenn die Sprache keine der
/// drei ist. Gebraucht dort, wo für eine andere Sprache nicht einfach
/// Englisch stehen soll, sondern eine Übersetzung durch die KI.
pub fn known_tongue(lang: &str) -> Option<Tongue> {
    let word = lang.trim().to_lowercase();
    // Ein Regionszusatz wie "de-AT" oder "ru_RU" ändert die Sprache nicht.
    let head = word.split(['-', '_']).next().unwrap_or("");
    match head {
        "german" | "deutsch" | "de" | "deu" | "ger" | "немецкий" => Some(Tongue::German),
        "english" | "englisch" | "en" | "eng" | "английский" => Some(Tongue::English),
        "russian" | "russisch" | "ru" | "rus" | "русский" => Some(Tongue::Russian),
        _ => None,
    }
}

/// Die Absage, wenn ein Gast für heute keine Fragen mehr frei hat.
pub fn no_questions_left(tongue: Tongue) -> &'static str {
    match tongue {
        Tongue::German => {
            "Ich kann gerade nicht antworten: Deine Fragen an mich sind für heute aufgebraucht."
        }
        Tongue::English => crate::NO_QUESTIONS_LEFT,
        Tongue::Russian => {
            "Сейчас я не могу ответить: твои вопросы ко мне на сегодня закончились."
        }
    }
}

/// Der Anfang der Mitteilung, wenn die KI nicht antworten konnte. Dahinter
/// folgt ein Doppelpunkt und der Grund, wie der Dienst ihn meldet.
pub fn no_answer(tongue: Tongue) -> &'static str {
    match tongue {
        Tongue::German => "Ich konnte nicht antworten",
        Tongue::English => crate::NO_ANSWER,
        Tongue::Russian => "Мне не удалось ответить",
    }
}
