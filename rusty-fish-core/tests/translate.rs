//! Tests des Kerns, ganz ohne Netz und ohne Schlüssel. Die KI ist eine Attrappe.

use rusty_fish_core::{
    FishError, Line, Model, ModelError, Outcome, Prompt, Reply, Request, build_prompt,
    read_answer, translate,
};
use std::future::Future;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

/// Attrappe: merkt sich, was sie gefragt wurde, und antwortet, was man ihr gab.
struct Fake {
    answer: Result<Reply, ModelError>,
    asked: Mutex<Vec<Prompt>>,
}

impl Fake {
    fn says(text: &str) -> Self {
        Fake {
            answer: Ok(Reply { text: text.to_owned(), tokens: Some(7) }),
            asked: Mutex::new(Vec::new()),
        }
    }

    fn fails(why: &str) -> Self {
        Fake { answer: Err(ModelError(why.to_owned())), asked: Mutex::new(Vec::new()) }
    }

    fn calls(&self) -> usize {
        self.asked.lock().unwrap().len()
    }

    fn last(&self) -> Prompt {
        self.asked.lock().unwrap().last().cloned().expect("model was never asked")
    }
}

impl Model for Fake {
    async fn complete(&self, prompt: &Prompt) -> Result<Reply, ModelError> {
        self.asked.lock().unwrap().push(prompt.clone());
        self.answer.clone()
    }
}

/// Die Attrappe antwortet sofort, also reicht ein einziger Poll.
fn now<F: Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = Context::from_waker(Waker::noop());
    match f.as_mut().poll(&mut cx) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("the fake model must answer at once"),
    }
}

fn request<'a>(text: &'a str) -> Request<'a> {
    Request {
        speaker: "zebra#1",
        text,
        target_lang: "Russian",
        pass_through: &["English"],
        context: &[],
        writer_gender: None,
        reader_gender: None,
        informal: false,
        reader_formality: None,
    }
}

#[test]
fn translates_into_the_readers_language() {
    let fake = Fake::says("Привет, как дела?");
    let out = now(translate(&fake, &request("Hallo, wie geht es dir?")));
    assert_eq!(out, Ok(Outcome::Translated("Привет, как дела?".to_owned())));
    let p = fake.last();
    assert!(p.system.contains("Russian"));
    assert!(p.user.contains("Hallo, wie geht es dir?"));
}

#[test]
fn skip_means_nothing_to_translate() {
    let fake = Fake::says("  =SKIP=\n");
    assert_eq!(now(translate(&fake, &request("See you tomorrow"))), Ok(Outcome::Skip));
}

#[test]
fn pass_through_languages_are_named_to_the_model() {
    let p = build_prompt(&request("See you tomorrow"));
    assert!(p.system.contains("=English="));
}

#[test]
fn the_model_is_asked_to_name_the_language_first() {
    let mut r = request("Привет");
    r.target_lang = "English";
    r.pass_through = &["Russian", "German"];
    let p = build_prompt(&r);
    assert!(p.system.contains("write =English= or =Russian= or =German= if"), "got: {}", p.system);
    assert!(p.system.contains("=OTHER= for every other language"), "got: {}", p.system);
    assert!(p.system.contains("into English"), "got: {}", p.system);

    r.pass_through = &[];
    let p = build_prompt(&r);
    assert!(p.system.contains("write =English= if"), "got: {}", p.system);
    assert!(!p.system.contains("Russian"), "got: {}", p.system);
}

#[test]
fn the_code_decides_by_the_named_language_not_the_model() {
    // Gemessen am 22.11.2026: gpt-6-luna übersetzte Russisch ins Englische,
    // obwohl Russisch unübersetzt bleiben sollte. Nennt die KI die Sprache,
    // bleibt der Text stehen, auch wenn sie trotzdem eine Übersetzung anhängt.
    let mut r = request("Какая сегодня погода?");
    r.target_lang = "English";
    r.pass_through = &["Russian"];
    for answer in ["=Russian=", "=Russian=\nWhat's the weather like today?", " =russian= \n", "=English="] {
        let fake = Fake::says(answer);
        assert_eq!(now(translate(&fake, &r)), Ok(Outcome::Skip), "answer {answer:?}");
    }
}

#[test]
fn after_other_comes_the_translation() {
    let r = request("Hallo, wie geht es dir?");
    for answer in ["=OTHER=\nПривет, как дела?", "=OTHER= Привет, как дела?", "=German=\nПривет, как дела?"] {
        let fake = Fake::says(answer);
        assert_eq!(
            now(translate(&fake, &r)),
            Ok(Outcome::Translated("Привет, как дела?".to_owned())),
            "answer {answer:?}"
        );
    }
}

#[test]
fn a_named_language_without_a_translation_fails_loudly() {
    let r = request("Hallo");
    for answer in ["=OTHER=", "=OTHER=\n  ", "=German="] {
        let fake = Fake::says(answer);
        assert_eq!(now(translate(&fake, &r)), Err(FishError::EmptyAnswer), "answer {answer:?}");
    }
    let fake = Fake::says("=OTHER=\n=SKIP= (no words)");
    assert!(matches!(now(translate(&fake, &r)), Err(FishError::UnclearAnswer(_))));
}

#[test]
fn a_language_name_of_the_host_in_another_script_is_matched_too() {
    let mut r = request("Привет");
    r.target_lang = "Русский";
    r.pass_through = &[];
    let fake = Fake::says("=Русский=");
    assert_eq!(now(translate(&fake, &r)), Ok(Outcome::Skip));
}

#[test]
fn target_language_is_not_listed_twice_as_pass_through() {
    let mut r = request("Привет");
    r.pass_through = &["russian", "English", " "];
    let p = build_prompt(&r);
    assert_eq!(p.system.matches("ussian").count(), p.system.matches("Russian").count());
    assert!(p.system.contains("English"));
}

#[test]
fn context_goes_along_but_names_stay_home() {
    let lines = [
        Line { speaker: "quokka#2", text: "Ты придёшь завтра?" },
        Line { speaker: "zebra#1", text: "Ich weiß es noch nicht." },
    ];
    let mut r = request("Doch, ich komme.");
    r.context = &lines;
    let p = build_prompt(&r);
    assert!(p.user.contains("Ты придёшь завтра?"));
    assert!(p.user.contains("Ich weiß es noch nicht."));
    let all = format!("{}\n{}", p.system, p.user);
    assert!(!all.contains("quokka"), "speaker ids must never reach the model");
    assert!(!all.contains("zebra"), "speaker ids must never reach the model");
    // Erste Stimme im Kontext ist A, die zweite B, und B schreibt auch den Text.
    assert!(p.user.contains("A: Ты придёшь завтра?"));
    assert!(p.user.contains("B: Ich weiß es noch nicht."));
    assert!(p.user.contains("<translate speaker=\"B\">"));
}

#[test]
fn without_context_there_is_no_context_block() {
    let p = build_prompt(&request("Hallo"));
    assert!(!p.user.contains("<context>"));
    assert!(p.user.contains("<translate speaker=\"A\">"));
}

#[test]
fn empty_answer_fails_loudly() {
    let fake = Fake::says("   ");
    assert_eq!(now(translate(&fake, &request("Hallo"))), Err(FishError::EmptyAnswer));
}

#[test]
fn skip_mixed_with_text_fails_loudly() {
    let got = read_answer("=SKIP= (already English)");
    assert!(matches!(got, Err(FishError::UnclearAnswer(_))), "got {got:?}");
}

#[test]
fn model_failure_fails_loudly() {
    let fake = Fake::fails("quota exceeded");
    assert_eq!(
        now(translate(&fake, &request("Hallo"))),
        Err(FishError::Model(ModelError("quota exceeded".to_owned())))
    );
}

#[test]
fn text_without_words_costs_nothing() {
    let fake = Fake::says("should never be asked");
    for text in ["", "   ", "👍", "12:30", "https://", "?!"] {
        // "https://" enthält Buchstaben und muss deshalb zur KI.
        let expect_call = text.chars().any(char::is_alphabetic);
        let before = fake.calls();
        let out = now(translate(&fake, &request(text)));
        if expect_call {
            assert_eq!(fake.calls(), before + 1, "text {text:?}");
        } else {
            assert_eq!(out, Ok(Outcome::Skip), "text {text:?}");
            assert_eq!(fake.calls(), before, "text {text:?} must not reach the model");
        }
    }
}

#[test]
fn missing_target_language_fails_before_the_model_is_asked() {
    let fake = Fake::says("x");
    let mut r = request("Hallo");
    r.target_lang = "  ";
    assert_eq!(now(translate(&fake, &r)), Err(FishError::NoTargetLanguage));
    assert_eq!(fake.calls(), 0);
}

#[test]
fn the_bite_can_cross_threads() {
    // Die Discord-Tür läuft auf mehreren Threads, also muss der Biss Send sein.
    fn assert_send<T: Send>(_: T) {}
    let fake = Fake::says("x");
    let r = request("Hallo");
    assert_send(translate(&fake, &r));
}

#[test]
fn the_informal_address_is_asked_for_only_when_set() {
    let plain = build_prompt(&request("Hello! Press OK to agree."));
    assert!(!plain.system.contains("informally"));

    let mut r = request("Hello! Press OK to agree.");
    r.informal = true;
    let friendly = build_prompt(&r);
    assert!(friendly.system.contains("Address the reader informally"));
    // Sonst ändert sich am Auftrag nichts.
    assert!(friendly.system.starts_with(&plain.system));
}
