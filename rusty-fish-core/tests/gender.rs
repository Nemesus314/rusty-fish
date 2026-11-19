//! Tests der freiwilligen Angabe zum grammatischen Geschlecht. Ohne Netz.

use rusty_fish_core::{Ask, Gender, Request, build_ask_prompt, build_prompt};

fn request(writer: Option<Gender>, reader: Option<Gender>) -> Request<'static> {
    Request {
        speaker: "zebra#1",
        text: "Ich war müde. Hast du gut geschlafen?",
        target_lang: "Russian",
        pass_through: &[],
        context: &[],
        writer_gender: writer,
        reader_gender: reader,
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
