//! Tests der freiwilligen Angabe zum grammatischen Geschlecht. Ohne Netz.

use rusty_fish_core::{
    Ask, Formality, Gender, Request, asks_for_a_level, build_ask_prompt, build_prompt,
};

fn request(writer: Option<Gender>, reader: Option<Gender>) -> Request<'static> {
    Request {
        speaker: "zebra#1",
        text: "Ich war müde. Hast du gut geschlafen?",
        target_lang: "Russian",
        pass_through: &[],
        context: &[],
        writer_gender: writer,
        reader_gender: reader,
        informal: false,
        reader_formality: None,
    }
}

fn question(asker: Option<Gender>) -> Ask<'static> {
    Ask {
        name: "Fisch",
        own_speaker: "ai#0",
        speaker: "quokka#2",
        text: "Fisch, что это значит?",
        answer_lang: "Russian",
        context: &[],
        asker_gender: asker,
    }
}

#[test]
fn without_the_hint_the_prompt_says_nothing_about_gender() {
    let p = build_prompt(&request(None, None));
    for word in ["woman", "is a man", "gender", "grammatical"] {
        assert!(!p.system.contains(word), "unexpected {word:?} in: {}", p.system);
    }
    let q = build_ask_prompt(&question(None));
    for word in ["woman", "is a man", "gender", "grammatical"] {
        assert!(!q.system.contains(word), "unexpected {word:?} in: {}", q.system);
    }
}

#[test]
fn the_writers_gender_is_named_for_grammar_only() {
    let p = build_prompt(&request(Some(Gender::Female), None));
    assert!(p.system.contains("The writer of the text is a woman."), "got: {}", p.system);
    assert!(!p.system.contains("The reader of your translation"), "got: {}", p.system);
    assert!(p.system.contains("only for grammatical forms"), "got: {}", p.system);
    assert!(p.system.contains("never mention it"), "got: {}", p.system);

    let p = build_prompt(&request(Some(Gender::Male), None));
    assert!(p.system.contains("The writer of the text is a man."), "got: {}", p.system);
}

#[test]
fn the_readers_gender_is_named_for_grammar_only() {
    let p = build_prompt(&request(None, Some(Gender::Female)));
    assert!(p.system.contains("The reader of your translation is a woman"), "got: {}", p.system);
    assert!(!p.system.contains("The writer of the text is"), "got: {}", p.system);
    assert!(p.system.contains("only for grammatical forms"), "got: {}", p.system);
}

#[test]
fn both_hints_can_stand_together_and_the_reason_is_said_once() {
    let p = build_prompt(&request(Some(Gender::Male), Some(Gender::Female)));
    assert!(p.system.contains("The writer of the text is a man."));
    assert!(p.system.contains("The reader of your translation is a woman"));
    assert_eq!(p.system.matches("only for grammatical forms").count(), 1);
}

#[test]
fn the_hint_stays_out_of_the_material_to_translate() {
    let p = build_prompt(&request(Some(Gender::Male), Some(Gender::Female)));
    assert!(!p.user.contains("woman") && !p.user.contains("a man"), "got: {}", p.user);
}

#[test]
fn the_askers_gender_is_named_to_the_addressable_ai() {
    let q = build_ask_prompt(&question(Some(Gender::Female)));
    assert!(q.system.contains("The person who addressed you is a woman."), "got: {}", q.system);
    assert!(q.system.contains("only for grammatical forms"), "got: {}", q.system);
    assert!(q.system.contains("never mention it"), "got: {}", q.system);

    let q = build_ask_prompt(&question(Some(Gender::Male)));
    assert!(q.system.contains("The person who addressed you is a man."), "got: {}", q.system);
}

// --- die Anrede, seit dem 23.11.2026 ------------------------------------------

#[test]
fn without_the_entry_the_prompt_says_nothing_about_how_to_address_the_reader() {
    let p = build_prompt(&request(Some(Gender::Female), Some(Gender::Male)));
    for word in ["respectfully", "familiar way", "honorific", "forms of address and never"] {
        assert!(!p.system.contains(word), "unexpected {word:?} in: {}", p.system);
    }
}

#[test]
fn the_readers_formality_is_named_for_forms_of_address_only() {
    let plain = build_prompt(&request(None, None));

    let respectful = build_prompt(&Request {
        reader_formality: Some(Formality::Respectful),
        ..request(None, None)
    });
    assert!(
        respectful.system.contains("the reader is to be addressed respectfully"),
        "got: {}",
        respectful.system
    );
    assert!(respectful.system.contains("polite or honorific forms"), "got: {}", respectful.system);
    assert!(
        respectful.system.contains("Use this only for forms of address and never mention it."),
        "got: {}",
        respectful.system
    );
    assert!(!respectful.system.contains("familiar way"), "got: {}", respectful.system);

    let familiar = build_prompt(&Request {
        reader_formality: Some(Formality::Familiar),
        ..request(None, None)
    });
    assert!(familiar.system.contains("addressed in the familiar way"), "got: {}", familiar.system);
    assert!(familiar.system.contains("plain, familiar forms"), "got: {}", familiar.system);
    assert!(!familiar.system.contains("respectfully"), "got: {}", familiar.system);

    // Der Hinweis steht hinter dem Auftrag, wie er war: davor ändert sich
    // kein Wort, und der Text, der übersetzt wird, bleibt derselbe.
    assert!(respectful.system.starts_with(&plain.system), "the prompt before it is unchanged");
    assert!(familiar.system.starts_with(&plain.system));
    assert_eq!(respectful.user, plain.user);
}

#[test]
fn the_formality_stands_after_the_hints_about_gender() {
    let p = build_prompt(&Request {
        reader_formality: Some(Formality::Respectful),
        ..request(Some(Gender::Female), Some(Gender::Male))
    });
    let gender = p.system.find("The reader of your translation is a man").unwrap();
    let formality = p.system.find("addressed respectfully").unwrap();
    assert!(gender < formality, "got: {}", p.system);
    // Jeder Hinweis nennt, wofür er gilt.
    assert!(p.system.contains("only for grammatical forms that depend on gender"));
    assert!(p.system.contains("only for forms of address"));
}

// --- unbekannte Anrede in Sprachen, die eine Stufe verlangen -------------------

/// Der Satz, an dem die Regel im Auftrag zu erkennen ist.
const SAFE: &str = "use its ordinary polite everyday forms";

fn into(lang: &'static str) -> Request<'static> {
    Request { target_lang: lang, ..request(None, None) }
}

#[test]
fn the_languages_of_the_table_ask_for_a_level() {
    // Entschieden von Nemesus am 23.11.2026: die Liste nach der Tabelle.
    for lang in [
        "Korean", "Japanese", "Javanese", "Sundanese", "Balinese", "Tibetan", "Vietnamese",
        "Thai", "Lao", "Burmese", "Khmer", "Tagalog", "Hindi", "Urdu", "Bengali", "Nepali",
        "Marathi", "Gujarati", "Punjabi", "Tamil", "Telugu", "Kannada", "Malayalam", "Sinhala",
    ] {
        assert!(asks_for_a_level(lang), "{lang}");
    }
    // Wie ein Mensch sie eintippt, und unter ihren anderen Namen.
    for lang in ["korean", " KOREAN ", "Korean (Seoul)", "Laotian", "Filipino", "Bangla", "Sinhalese"] {
        assert!(asks_for_a_level(lang), "{lang}");
    }
}

#[test]
fn every_other_language_does_not() {
    for lang in [
        "German", "English", "Russian", "French", "Spanish", "Italian", "Brazilian Portuguese",
        "Polish", "Turkish", "Persian", "Arabic", "Indonesian", "Malay", "Simplified Chinese",
        "Traditional Chinese (Taiwan)", "",
    ] {
        assert!(!asks_for_a_level(lang), "{lang}");
    }
    // Ein ganzes Wort, kein Teil eines anderen: Malaiisch ist nicht Malayalam.
    assert!(!asks_for_a_level("Malay"));
    assert!(asks_for_a_level("Malayalam"));
    // Benannte Grenze: der Name in der Sprache selbst wird nicht erkannt.
    assert!(!asks_for_a_level("한국어"));
}

#[test]
fn with_the_address_unknown_such_a_language_gets_the_polite_everyday_forms() {
    for lang in ["Korean", "Japanese", "Hindi", "Vietnamese"] {
        let p = build_prompt(&into(lang));
        assert!(p.system.contains(SAFE), "{lang}: {}", p.system);
        assert!(p.system.contains("not the intimate forms"), "{lang}: {}", p.system);
        assert!(p.system.contains("only for forms of address and never mention it"), "{lang}");
    }
}

#[test]
fn into_every_other_language_the_prompt_is_word_for_word_as_before() {
    for lang in ["Russian", "German", "French", "Brazilian Portuguese"] {
        let p = build_prompt(&into(lang));
        assert!(!p.system.contains(SAFE), "{lang}: {}", p.system);
        assert!(!p.system.contains("politely"), "{lang}: {}", p.system);
    }
}

#[test]
fn an_entry_for_the_reader_goes_before_the_rule() {
    let respectful =
        build_prompt(&Request { reader_formality: Some(Formality::Respectful), ..into("Korean") });
    assert!(respectful.system.contains("addressed respectfully"), "got: {}", respectful.system);
    assert!(!respectful.system.contains(SAFE), "got: {}", respectful.system);

    let familiar =
        build_prompt(&Request { reader_formality: Some(Formality::Familiar), ..into("Korean") });
    assert!(familiar.system.contains("addressed in the familiar way"), "got: {}", familiar.system);
    assert!(!familiar.system.contains(SAFE), "got: {}", familiar.system);
}

#[test]
fn the_greeting_stays_informal_except_where_the_language_asks_for_a_level() {
    // Entschieden am 19.11.2026: die Begrüßung duzt. Das galt Russisch und
    // Deutsch; in einer Sprache mit Stufen wäre die vertraute Form gegenüber
    // einem Gast, den der Fisch nicht kennt, grob.
    let russian = build_prompt(&Request { informal: true, ..into("Russian") });
    assert!(russian.system.contains("Address the reader informally"), "got: {}", russian.system);

    let korean = build_prompt(&Request { informal: true, ..into("Korean") });
    assert!(!korean.system.contains("Address the reader informally"), "got: {}", korean.system);
    assert!(korean.system.contains(SAFE), "got: {}", korean.system);
}
