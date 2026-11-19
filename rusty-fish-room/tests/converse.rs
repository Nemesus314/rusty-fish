//! Tests des Kontingents und der ansprechbaren KI. Keine Tür, kein Netz.

use rusty_fish_core::{Model, ModelError, Prompt, Reply};
use rusty_fish_room::{
    ASSISTANT, BUDGET_USED_UP, Delivery, House, Incoming, Limits, Mark, Metered, NO_ANSWER,
    NO_QUESTIONS_LEFT, Reason, Room, RoomState, Seat, Sender, Tally, converse,
};
use std::future::Future;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

/// Attrappe mit zwei Stimmen: eine für Fragen, eine fürs Übersetzen.
struct Fake {
    translation: Result<String, String>,
    answer: Result<String, String>,
    tokens: Option<u64>,
    asked: Mutex<Vec<Prompt>>,
}

impl Fake {
    fn new() -> Self {
        Fake {
            translation: Ok("TRANSLATED".to_owned()),
            answer: Ok("ANSWER".to_owned()),
            tokens: Some(50),
            asked: Mutex::new(Vec::new()),
        }
    }

    fn is_question(prompt: &Prompt) -> bool {
        prompt.user.contains("<message ")
    }

    fn questions(&self) -> Vec<Prompt> {
        self.asked.lock().unwrap().iter().filter(|p| Fake::is_question(p)).cloned().collect()
    }

    fn translations(&self) -> Vec<Prompt> {
        self.asked.lock().unwrap().iter().filter(|p| !Fake::is_question(p)).cloned().collect()
    }

    fn calls(&self) -> usize {
        self.asked.lock().unwrap().len()
    }
}

impl Model for Fake {
    async fn complete(&self, prompt: &Prompt) -> Result<Reply, ModelError> {
        self.asked.lock().unwrap().push(prompt.clone());
        let voice = if Fake::is_question(prompt) { &self.answer } else { &self.translation };
        match voice {
            Ok(text) => Ok(Reply { text: text.clone(), tokens: self.tokens }),
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

fn room() -> Room {
    Room {
        seats: vec![
            Seat { channel: "de".to_owned(), lang: "German".to_owned(), gender: None },
            Seat { channel: "ru".to_owned(), lang: "Russian".to_owned(), gender: None },
        ],
        pass_through: vec![],
        context_lines: 5,
    }
}

fn from<'a>(channel: &'a str, author: &'a str, text: &'a str) -> Incoming<'a> {
    Incoming { channel, author, text, consented: true }
}

/// Ein Haus mit Namen für die KI und den gegebenen Grenzen.
struct Setup {
    limits: Limits,
    state: RoomState,
    tally: Tally,
}

impl Setup {
    fn new(tokens: u64, questions: u32) -> Self {
        Setup {
            limits: Limits {
                translation_tokens_per_day: tokens,
                questions_per_guest_per_day: questions,
            },
            state: RoomState::default(),
            tally: Tally::default(),
        }
    }

    fn say(&mut self, fake: &Fake, day: &str, msg: &Incoming<'_>, is_host: bool) -> Vec<Delivery> {
        self.say_with(fake, day, Some("Fisch"), msg, is_host)
    }

    fn say_with(
        &mut self,
        fake: &Fake,
        day: &str,
        ai_name: Option<&str>,
        msg: &Incoming<'_>,
        is_host: bool,
    ) -> Vec<Delivery> {
        let names: Vec<&str> = ai_name.into_iter().collect();
        self.say_names(fake, day, &names, msg, is_host)
    }

    fn say_names(
        &mut self,
        fake: &Fake,
        day: &str,
        ai_names: &[&str],
        msg: &Incoming<'_>,
        is_host: bool,
    ) -> Vec<Delivery> {
        let house = House { limits: &self.limits, ai_names, today: day };
        now(converse(fake, &room(), &mut self.state, &mut self.tally, &house, msg, is_host))
    }
}

const DAY: &str = "2026-11-19";

// --- die ansprechbare KI ---------------------------------------------------

#[test]
fn an_ordinary_message_is_only_relayed() {
    let fake = Fake::new();
    let mut s = Setup::new(1000, 3);
    let out = s.say(&fake, DAY, &from("de", "guest", "Hallo zusammen"), false);
    assert_eq!(
        out,
        [Delivery {
            channel: "ru".to_owned(),
            text: "TRANSLATED".to_owned(),
            mark: Mark::Translated,
            from: Sender::Writer,
        }]
    );
    assert!(fake.questions().is_empty());
}

#[test]
fn addressed_by_name_the_ai_answers_once_at_every_seat() {
    let fake = Fake::new();
    let mut s = Setup::new(1000, 3);
    let out = s.say(&fake, DAY, &from("de", "guest", "Fisch, wie spät ist es?"), false);
    assert_eq!(
        out,
        [
            // Zuerst die Frage selbst, wie jede Nachricht.
            Delivery {
                channel: "ru".to_owned(),
                text: "TRANSLATED".to_owned(),
                mark: Mark::Translated,
                from: Sender::Writer,
            },
            // Dann die Antwort: am Platz des Fragenden unübersetzt ...
            Delivery {
                channel: "de".to_owned(),
                text: "ANSWER".to_owned(),
                mark: Mark::Original,
                from: Sender::Assistant,
            },
            // ... an den anderen in deren Sprache.
            Delivery {
                channel: "ru".to_owned(),
                text: "TRANSLATED".to_owned(),
                mark: Mark::Translated,
                from: Sender::Assistant,
            },
        ]
    );
    let questions = fake.questions();
    assert_eq!(questions.len(), 1, "the AI answers exactly once");
    assert!(questions[0].system.contains("Fisch"));
    assert!(questions[0].system.contains("German"), "the answer is in the asker's language");
    // Die Frage steht im Auftrag, nicht noch einmal im Kontext.
    assert_eq!(questions[0].user.matches("wie spät ist es?").count(), 1);
}

#[test]
fn the_name_in_the_middle_of_a_sentence_is_no_command() {
    let fake = Fake::new();
    let mut s = Setup::new(1000, 3);
    let out = s.say(&fake, DAY, &from("de", "guest", "Ich mag den Fisch, wirklich"), false);
    assert_eq!(out.len(), 1);
    assert!(fake.questions().is_empty());
}

#[test]
fn without_a_name_the_ai_cannot_be_addressed() {
    let fake = Fake::new();
    let mut s = Setup::new(1000, 3);
    let out = s.say_with(&fake, DAY, None, &from("de", "guest", "Fisch, hallo?"), false);
    assert_eq!(out.len(), 1);
    assert!(fake.questions().is_empty());
}

#[test]
fn every_call_name_addresses_the_ai_and_the_first_is_who_she_is() {
    let fake = Fake::new();
    let mut s = Setup::new(1000, 3);
    // Am russischen Platz mit dem kyrillischen Rufnamen gefragt.
    let msg = from("ru", "guest", "Фиш, который час?");
    let out = s.say_names(&fake, DAY, &["Fish", "Фиш"], &msg, false);
    assert_eq!(out.len(), 3, "the question, the answer, and the answer at the other seat");
    let questions = fake.questions();
    assert_eq!(questions.len(), 1, "the second call name must address the AI too");
    assert!(questions[0].system.contains("You are Fish,"), "got: {}", questions[0].system);
    assert!(questions[0].system.contains("Russian"), "the answer is in the asker's language");

    // Der erste Rufname gilt genauso.
    let out = s.say_names(&fake, DAY, &["Fish", "Фиш"], &from("de", "guest", "Fish, danke"), false);
    assert_eq!(out.len(), 3);
    assert_eq!(fake.questions().len(), 2);
}

#[test]
fn an_empty_list_of_call_names_means_off() {
    let fake = Fake::new();
    let mut s = Setup::new(1000, 3);
    let out = s.say_names(&fake, DAY, &[], &from("de", "guest", "Fish, hallo?"), false);
    assert_eq!(out.len(), 1);
    assert!(fake.questions().is_empty());
}

#[test]
fn without_consent_a_question_reaches_no_model() {
    let fake = Fake::new();
    let mut s = Setup::new(1000, 3);
    let mut msg = from("ru", "guest", "Fisch, это личное?");
    msg.consented = false;
    let out = s.say(&fake, DAY, &msg, false);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].mark, Mark::Untranslated(Reason::NoConsent));
    assert_eq!(fake.calls(), 0);
    assert!(s.tally.questions.is_empty(), "an unasked question must not be counted");
}

#[test]
fn the_answer_becomes_context_like_any_message() {
    let fake = Fake { answer: Ok("Es ist drei Uhr.".to_owned()), ..Fake::new() };
    let mut s = Setup::new(10_000, 3);
    let _ = s.say(&fake, DAY, &from("de", "guest", "Fisch, wie spät ist es?"), false);
    let _ = s.say(&fake, DAY, &from("ru", "other", "Спасибо"), false);
    let last = fake.translations().pop().unwrap();
    assert!(last.user.contains("Es ist drei Uhr."), "the answer is missing from the context");
    assert!(!last.user.contains(ASSISTANT), "the AI's id must not reach the model");
}

#[test]
fn a_failed_answer_is_said_honestly_at_the_askers_seat_only() {
    let fake = Fake { answer: Err("provider down".to_owned()), ..Fake::new() };
    let mut s = Setup::new(1000, 3);
    let out = s.say(&fake, DAY, &from("de", "guest", "Fisch, bist du da?"), false);
    assert_eq!(out.len(), 2);
    assert_eq!(out[1].channel, "de");
    assert_eq!(out[1].mark, Mark::Notice);
    assert_eq!(out[1].from, Sender::Assistant);
    assert!(out[1].text.starts_with(NO_ANSWER), "got {:?}", out[1].text);
    assert!(out[1].text.contains("provider down"));
    // Der Versuch kann gekostet haben, also zählt er.
    assert_eq!(s.tally.questions.get("guest"), Some(&1));
}

// --- Fragen je Gast --------------------------------------------------------

#[test]
fn a_guest_runs_out_of_questions_and_is_told_so() {
    let fake = Fake::new();
    let mut s = Setup::new(100_000, 2);
    for _ in 0..2 {
        let out = s.say(&fake, DAY, &from("ru", "guest", "Fisch, ещё вопрос"), false);
        assert_eq!(out.len(), 3);
    }
    let out = s.say(&fake, DAY, &from("ru", "guest", "Fisch, и ещё один"), false);
    assert_eq!(fake.questions().len(), 2, "the third question must not reach the model");
    // Die Nachricht selbst kommt weiter an, die Absage nur beim Fragenden.
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].from, Sender::Writer);
    assert_eq!(
        out[1],
        Delivery {
            channel: "ru".to_owned(),
            text: NO_QUESTIONS_LEFT.to_owned(),
            mark: Mark::Notice,
            from: Sender::Assistant,
        }
    );
}

#[test]
fn every_guest_has_their_own_questions() {
    let fake = Fake::new();
    let mut s = Setup::new(100_000, 1);
    let _ = s.say(&fake, DAY, &from("ru", "guest", "Fisch, раз"), false);
    let _ = s.say(&fake, DAY, &from("ru", "guest", "Fisch, два"), false);
    let out = s.say(&fake, DAY, &from("de", "other", "Fisch, und ich?"), false);
    assert_eq!(out.len(), 3, "another guest must still be answered");
    assert_eq!(fake.questions().len(), 2);
}

#[test]
fn the_host_has_no_question_limit() {
    let fake = Fake::new();
    let mut s = Setup::new(100_000, 0);
    for _ in 0..5 {
        let out = s.say(&fake, DAY, &from("de", "host", "Fisch, noch eine Frage"), true);
        assert_eq!(out.len(), 3);
    }
    assert_eq!(fake.questions().len(), 5);
    assert!(s.tally.questions.is_empty(), "the host's questions are not counted");
}

#[test]
fn a_new_day_starts_at_zero() {
    let fake = Fake::new();
    let mut s = Setup::new(100_000, 1);
    let _ = s.say(&fake, "2026-11-19", &from("ru", "guest", "Fisch, раз"), false);
    assert!(s.tally.translation_tokens > 0);
    let refused = s.say(&fake, "2026-11-19", &from("ru", "guest", "Fisch, два"), false);
    assert_eq!(refused[1].mark, Mark::Notice);

    let out = s.say(&fake, "2026-11-20", &from("ru", "guest", "Fisch, три"), false);
    assert_eq!(out.len(), 3, "on a new day the guest may ask again");
    assert_eq!(s.tally.day, "2026-11-20");
    assert_eq!(s.tally.questions.get("guest"), Some(&1));
    assert_eq!(s.tally.translation_tokens, 50, "only today's translation is counted");
}

// --- der Übersetzungstopf --------------------------------------------------

#[test]
fn translations_are_counted_as_the_provider_reports_them() {
    let fake = Fake::new(); // meldet 50 Tokens je Aufruf
    let mut s = Setup::new(1000, 3);
    let _ = s.say(&fake, DAY, &from("de", "guest", "Hallo"), false);
    assert_eq!(s.tally.translation_tokens, 50);
    let _ = s.say(&fake, DAY, &from("ru", "other", "Привет"), false);
    assert_eq!(s.tally.translation_tokens, 100);
    assert_eq!(s.tally.tokens_left(&s.limits), 900);
}

#[test]
fn an_empty_pot_means_no_call_and_an_honest_original() {
    let fake = Fake::new();
    let mut s = Setup::new(100, 3);
    let _ = s.say(&fake, DAY, &from("de", "guest", "eins"), false); // 50
    let _ = s.say(&fake, DAY, &from("de", "guest", "zwei"), false); // 100: Topf leer
    let before = fake.calls();
    let out = s.say(&fake, DAY, &from("de", "guest", "drei"), false);
    assert_eq!(fake.calls(), before, "an empty pot must not cause a call");
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].text, "drei");
    match &out[0].mark {
        Mark::Untranslated(Reason::Failed(why)) => assert!(why.contains(BUDGET_USED_UP)),
        other => panic!("expected a marked original, got {other:?}"),
    }
    assert_eq!(s.tally.translation_tokens, 100, "nothing was spent, nothing is counted");
}

#[test]
fn questions_never_drain_the_translation_pot() {
    let fake = Fake::new();
    let mut s = Setup::new(1000, 3);
    let out = s.say(&fake, DAY, &from("de", "guest", "Fisch, wie spät ist es?"), false);
    assert_eq!(out.len(), 3);
    // Drei Aufrufe fanden statt: die Frage übersetzen, antworten, die Antwort
    // übersetzen. Gegen den Übersetzungstopf zählt nur der erste.
    assert_eq!(fake.calls(), 3);
    assert_eq!(s.tally.translation_tokens, 50);
}

#[test]
fn with_an_empty_pot_a_question_is_still_answered() {
    let fake = Fake::new();
    let mut s = Setup::new(0, 3);
    let out = s.say(&fake, DAY, &from("de", "guest", "Fisch, bist du da?"), false);
    assert_eq!(out.len(), 3);
    assert!(matches!(out[0].mark, Mark::Untranslated(Reason::Failed(_))));
    assert_eq!(out[1].text, "ANSWER");
    assert_eq!(out[2].mark, Mark::Translated);
    assert_eq!(s.tally.translation_tokens, 0);
}

#[test]
fn without_a_usage_report_the_meter_estimates() {
    let fake = Fake { tokens: None, ..Fake::new() };
    let meter = Metered::new(&fake, 1_000_000);
    let prompt = Prompt { system: "s".repeat(300), user: "u".repeat(300) };
    let reply = now(meter.complete(&prompt)).unwrap();
    assert_eq!(reply.text, "TRANSLATED");
    // 600 Zeichen Auftrag und 10 Zeichen Antwort, drei Zeichen je Token.
    assert_eq!(meter.used(), 200 + 4);
}

#[test]
fn a_failed_call_is_counted_with_the_estimate_of_its_prompt() {
    let fake = Fake { translation: Err("timeout".to_owned()), ..Fake::new() };
    let meter = Metered::new(&fake, 1_000_000);
    let prompt = Prompt { system: "s".repeat(30), user: "u".repeat(30) };
    assert!(now(meter.complete(&prompt)).is_err());
    assert_eq!(meter.used(), 20);
}

#[test]
fn the_meter_refuses_without_calling_once_nothing_is_left() {
    let fake = Fake::new();
    let prompt = Prompt { system: "s".to_owned(), user: "u".to_owned() };

    let empty = Metered::new(&fake, 0);
    assert_eq!(now(empty.complete(&prompt)), Err(ModelError(BUDGET_USED_UP.to_owned())));
    assert_eq!(fake.calls(), 0);
    assert_eq!(empty.used(), 0);

    // Die Grenze ist weich: der letzte Aufruf darf überziehen, der nächste nicht.
    let small = Metered::new(&fake, 10);
    assert!(now(small.complete(&prompt)).is_ok());
    assert_eq!(small.used(), 50);
    assert!(now(small.complete(&prompt)).is_err());
    assert_eq!(fake.calls(), 1);
}

#[test]
fn the_tally_keeps_the_two_pots_apart() {
    let limits = Limits { translation_tokens_per_day: 100, questions_per_guest_per_day: 1 };
    let mut tally = Tally::default();
    tally.start_day(DAY);
    assert!(tally.may_ask("guest", &limits));
    tally.asked("guest");
    assert!(!tally.may_ask("guest", &limits));
    assert!(tally.may_ask("other", &limits));
    assert_eq!(tally.tokens_left(&limits), 100);
    tally.spend(250);
    assert_eq!(tally.tokens_left(&limits), 0);
    tally.start_day(DAY);
    assert_eq!(tally.translation_tokens, 250, "the same day must not reset the count");
    tally.start_day("2026-11-20");
    assert_eq!(tally, Tally { day: "2026-11-20".to_owned(), ..Tally::default() });
}

#[test]
fn the_defaults_are_cautious_but_usable() {
    let limits = Limits::default();
    assert!(limits.translation_tokens_per_day > 0);
    assert!(limits.questions_per_guest_per_day > 0);
}

#[test]
fn conversing_can_cross_threads() {
    fn assert_send<T: Send>(_: T) {}
    let fake = Fake::new();
    let room = room();
    let limits = Limits::default();
    let house = House { limits: &limits, ai_names: &["Fisch"], today: DAY };
    let mut state = RoomState::default();
    let mut tally = Tally::default();
    let msg = from("de", "guest", "Hallo");
    assert_send(converse(&fake, &room, &mut state, &mut tally, &house, &msg, false));
}
