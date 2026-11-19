//! Tests der ansprechbaren KI im Kern. Ohne Netz, die KI ist eine Attrappe.

use rusty_fish_core::{
    Ask, FishError, Line, Model, ModelError, Prompt, Reply, addressed, addressed_any, ask,
    build_ask_prompt,
};
use std::future::Future;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

struct Fake {
    answer: Result<String, String>,
    asked: Mutex<Vec<Prompt>>,
}

impl Fake {
    fn says(text: &str) -> Self {
        Fake { answer: Ok(text.to_owned()), asked: Mutex::new(Vec::new()) }
    }

    fn fails(why: &str) -> Self {
        Fake { answer: Err(why.to_owned()), asked: Mutex::new(Vec::new()) }
    }

    fn calls(&self) -> usize {
        self.asked.lock().unwrap().len()
    }
}

impl Model for Fake {
    async fn complete(&self, prompt: &Prompt) -> Result<Reply, ModelError> {
        self.asked.lock().unwrap().push(prompt.clone());
        match &self.answer {
            Ok(text) => Ok(Reply { text: text.clone(), tokens: None }),
            Err(why) => Err(ModelError(why.clone())),
        }
    }
}

fn now<F: Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = Context::from_waker(Waker::noop());
    match f.as_mut().poll(&mut cx) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("the fake model must answer at once"),
    }
}

#[test]
fn the_name_at_the_start_addresses_the_ai() {
    assert_eq!(addressed("Fisch, wie spät ist es?", "Fisch"), Some("wie spät ist es?"));
    assert_eq!(addressed("fisch: hallo", "Fisch"), Some("hallo"));
    assert_eq!(addressed("FISCH hallo", "Fisch"), Some("hallo"));
    assert_eq!(addressed("@Fisch hallo", "Fisch"), Some("hallo"));
    assert_eq!(addressed("   Fisch,   hallo  ", "Fisch"), Some("hallo"));
    assert_eq!(addressed("Fisch, hallo", "  Fisch "), Some("hallo"));
}

#[test]
fn the_name_elsewhere_does_not() {
    assert_eq!(addressed("Ich frage Fisch, ob er das weiß", "Fisch"), None);
    assert_eq!(addressed("Fischlein, hallo", "Fisch"), None);
    assert_eq!(addressed("Fisch", "Fisch"), None);
    assert_eq!(addressed("Fisch,", "Fisch"), None);
    assert_eq!(addressed("Fisch:   ", "Fisch"), None);
    assert_eq!(addressed("Fisch.hallo", "Fisch"), None);
    assert_eq!(addressed("", "Fisch"), None);
    assert_eq!(addressed("Fi", "Fisch"), None);
    assert_eq!(addressed("hallo", ""), None);
    assert_eq!(addressed("hallo", "  "), None);
}

#[test]
fn other_scripts_work_in_the_question_and_in_the_name() {
    assert_eq!(addressed("Fisch, как дела?", "Fisch"), Some("как дела?"));
    assert_eq!(addressed("рыбка, привет", "Рыбка"), Some("привет"));
    assert_eq!(addressed("@РЫБКА: привет", "Рыбка"), Some("привет"));
    assert_eq!(addressed("Rusty Fish, hello", "rusty fish"), Some("hello"));
}

// --- mehrere Rufnamen --------------------------------------------------------

const CALL_NAMES: [&str; 2] = ["Fish", "Фиш"];

#[test]
fn a_cyrillic_call_name_works_with_cyrillic_text() {
    assert_eq!(addressed_any("Фиш, как дела?", &CALL_NAMES), Some("как дела?"));
    assert_eq!(addressed_any("фиш: привет", &CALL_NAMES), Some("привет"));
    assert_eq!(addressed_any("@ФИШ что это значит", &CALL_NAMES), Some("что это значит"));
}

#[test]
fn a_latin_call_name_works_with_cyrillic_text() {
    assert_eq!(addressed_any("Fish, как дела?", &CALL_NAMES), Some("как дела?"));
    assert_eq!(addressed_any("fish wie spät ist es?", &CALL_NAMES), Some("wie spät ist es?"));
}

#[test]
fn the_longest_call_name_wins_whatever_the_order() {
    // „Fish" ist der Anfang von „Fish Bot" und darf nicht zu früh greifen.
    assert_eq!(addressed_any("Fish Bot, hello", &["Fish", "Fish Bot"]), Some("hello"));
    assert_eq!(addressed_any("Fish Bot, hello", &["Fish Bot", "Fish"]), Some("hello"));
    // Der kürzere Name gilt weiter für sich allein.
    assert_eq!(addressed_any("Fish, hello", &["Fish", "Fish Bot"]), Some("hello"));
    assert_eq!(addressed_any("Фишка, привет", &["Фиш", "Фишка"]), Some("привет"));
}

#[test]
fn a_call_name_in_the_middle_of_a_sentence_does_not_address() {
    assert_eq!(addressed_any("Спроси у Фиш, что это", &CALL_NAMES), None);
    assert_eq!(addressed_any("I asked Fish, what is it", &CALL_NAMES), None);
    assert_eq!(addressed_any("Fisherman, hello", &CALL_NAMES), None);
    assert_eq!(addressed_any("Фишка, привет", &CALL_NAMES), None);
}

#[test]
fn no_call_names_means_nobody_is_addressed() {
    assert_eq!(addressed_any("Fish, hello", &[]), None);
    // Leere Einträge zählen nicht und sprechen niemanden an.
    assert_eq!(addressed_any("Fish, hello", &["", "  "]), None);
    assert_eq!(addressed_any(", hello", &["", "Fish"]), None);
    assert_eq!(addressed_any("Fish, hello", &["  ", " Fish "]), Some("hello"));
}

fn question<'a>(context: &'a [Line<'a>]) -> Ask<'a> {
    Ask {
        name: "Fisch",
        own_speaker: "ai#0",
        speaker: "quokka#2",
        text: "Fisch, что он имел в виду?",
        answer_lang: "Russian",
        context,
        asker_gender: None,
    }
}

#[test]
fn the_question_names_the_ai_and_the_language_but_no_person() {
    let lines = [
        Line { speaker: "zebra#1", text: "Das passt schon." },
        Line { speaker: "ai#0", text: "Gern geschehen." },
        Line { speaker: "quokka#2", text: "Хорошо." },
    ];
    let p = build_ask_prompt(&question(&lines));
    assert!(p.system.contains("Fisch"));
    assert!(p.system.contains("Russian"));
    let all = format!("{}\n{}", p.system, p.user);
    for id in ["zebra", "quokka", "ai#0"] {
        assert!(!all.contains(id), "speaker id {id:?} must never reach the model");
    }
    // Menschen stehen unter Buchstaben, die KI unter ihrem Namen.
    assert!(p.user.contains("A: Das passt schon."));
    assert!(p.user.contains("Fisch: Gern geschehen."));
    assert!(p.user.contains("B: Хорошо."));
    assert!(p.user.contains("<message speaker=\"B\">"));
    assert!(p.user.contains("Fisch, что он имел в виду?"));
}

#[test]
fn a_question_without_context_has_no_context_block() {
    let p = build_ask_prompt(&question(&[]));
    assert!(!p.user.contains("<context>"));
    assert!(p.user.contains("<message speaker=\"A\">"));
}

#[test]
fn the_answer_comes_back_trimmed() {
    let fake = Fake::says("  Он имел в виду, что всё в порядке.\n");
    assert_eq!(
        now(ask(&fake, &question(&[]))),
        Ok("Он имел в виду, что всё в порядке.".to_owned())
    );
    assert_eq!(fake.calls(), 1);
}

#[test]
fn asking_fails_loudly() {
    assert_eq!(now(ask(&Fake::says("  "), &question(&[]))), Err(FishError::EmptyAnswer));
    assert_eq!(
        now(ask(&Fake::fails("quota exceeded"), &question(&[]))),
        Err(FishError::Model(ModelError("quota exceeded".to_owned())))
    );
    let fake = Fake::says("x");
    let mut q = question(&[]);
    q.answer_lang = " ";
    assert_eq!(now(ask(&fake, &q)), Err(FishError::NoTargetLanguage));
    assert_eq!(fake.calls(), 0);
}

#[test]
fn asking_can_cross_threads() {
    fn assert_send<T: Send>(_: T) {}
    let fake = Fake::says("x");
    let q = question(&[]);
    assert_send(ask(&fake, &q));
}
