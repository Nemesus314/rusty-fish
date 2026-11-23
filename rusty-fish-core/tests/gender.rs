//! Tests der freiwilligen Angabe zum grammatischen Geschlecht. Ohne Netz.

use rusty_fish_core::{Ask, Formality, Gender, Request, build_ask_prompt, build_prompt};

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
