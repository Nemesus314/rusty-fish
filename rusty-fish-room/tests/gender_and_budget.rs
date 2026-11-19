//! Tests zum grammatischen Geschlecht je Platz und zum Hinweis an den
//! Gastgeber, wenn das Übersetzungskontingent leer ist. Keine Tür, kein Netz.

use rusty_fish_core::{Model, ModelError, Prompt, Reply};
use rusty_fish_room::{
    Delivery, Gender, House, Incoming, Limits, Mark, Reason, Room, RoomState, Seat, Sender, Tally,
    budget_ran_out, converse, relay,
};
use std::future::Future;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

/// Attrappe mit zwei Stimmen: eine für Fragen, eine fürs Übersetzen.
struct Fake {
    fails: Option<String>,
    asked: Mutex<Vec<Prompt>>,
}

impl Fake {
    fn new() -> Self {
        Fake { fails: None, asked: Mutex::new(Vec::new()) }
    }

    fn failing(why: &str) -> Self {
        Fake { fails: Some(why.to_owned()), asked: Mutex::new(Vec::new()) }
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
}

impl Model for Fake {
    async fn complete(&self, prompt: &Prompt) -> Result<Reply, ModelError> {
        self.asked.lock().unwrap().push(prompt.clone());
        if let Some(why) = &self.fails {
            return Err(ModelError(why.clone()));
        }
        let text = if Fake::is_question(prompt) { "ANSWER" } else { "TRANSLATED" };
        Ok(Reply { text: text.to_owned(), tokens: Some(50) })
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

fn seat(channel: &str, lang: &str, gender: Option<Gender>) -> Seat {
    Seat { channel: channel.to_owned(), lang: lang.to_owned(), gender }
}

fn room(seats: Vec<Seat>) -> Room {
    Room { seats, pass_through: vec![], context_lines: 5 }
}

fn from<'a>(channel: &'a str, author: &'a str, text: &'a str) -> Incoming<'a> {
    Incoming { channel, author, text, consented: true }
}

const WRITER_IS: &str = "The writer of the text is";
const READER_IS: &str = "The reader of your translation is";
const ASKER_IS: &str = "The person who addressed you is";
const DAY: &str = "2026-11-19";

// --- das grammatische Geschlecht je Platz ------------------------------------

#[test]
fn the_writers_and_the_readers_gender_reach_the_translator() {
    let room = room(vec![
        seat("de", "German", Some(Gender::Male)),
        seat("ru", "Russian", Some(Gender::Female)),
    ]);

    let fake = Fake::new();
    let mut state = RoomState::default();
    let _ = now(relay(&fake, &room, &mut state, &from("de", "zebra#1", "Ich war müde.")));
    let p = fake.translations().pop().unwrap();
    assert!(p.system.contains("The writer of the text is a man."), "got: {}", p.system);
    assert!(p.system.contains("The reader of your translation is a woman"), "got: {}", p.system);

    let fake = Fake::new();
    let _ = now(relay(&fake, &room, &mut state, &from("ru", "quokka#2", "Я устала.")));
    let p = fake.translations().pop().unwrap();
    assert!(p.system.contains("The writer of the text is a woman."), "got: {}", p.system);
    assert!(p.system.contains("The reader of your translation is a man"), "got: {}", p.system);
}

#[test]
fn seats_without_the_setting_send_no_hint() {
    let room = room(vec![seat("de", "German", None), seat("ru", "Russian", None)]);
    let fake = Fake::new();
    let mut state = RoomState::default();
    let _ = now(relay(&fake, &room, &mut state, &from("de", "zebra#1", "Ich war müde.")));
    let p = fake.translations().pop().unwrap();
    assert!(!p.system.contains("grammatical"), "got: {}", p.system);
}

#[test]
fn one_seat_with_the_setting_is_enough_for_its_own_hint() {
    let room = room(vec![seat("de", "German", None), seat("ru", "Russian", Some(Gender::Female))]);
    let fake = Fake::new();
    let mut state = RoomState::default();
    let _ = now(relay(&fake, &room, &mut state, &from("de", "zebra#1", "Hast du geschlafen?")));
    let p = fake.translations().pop().unwrap();
    assert!(!p.system.contains(WRITER_IS), "got: {}", p.system);
    assert!(p.system.contains("The reader of your translation is a woman"), "got: {}", p.system);
}

#[test]
fn one_translation_is_shared_only_by_seats_of_the_same_language_and_gender() {
    let room = room(vec![
        seat("de", "German", None),
        seat("ru1", "Russian", Some(Gender::Female)),
        seat("ru2", "Russian", Some(Gender::Male)),
        seat("ru3", "russian ", Some(Gender::Female)),
        seat("ru4", "Russian", None),
    ]);
    let fake = Fake::new();
    let mut state = RoomState::default();
    let out = now(relay(&fake, &room, &mut state, &from("de", "zebra#1", "Hast du geschlafen?")));
    assert_eq!(out.len(), 4);
    let prompts = fake.translations();
    // Weiblich (ru1 und ru3 teilen), männlich (ru2), ohne Angabe (ru4).
    assert_eq!(prompts.len(), 3, "one paid translation per language and gender");
    assert_eq!(prompts.iter().filter(|p| p.system.contains("is a woman")).count(), 1);
    assert_eq!(prompts.iter().filter(|p| p.system.contains("is a man")).count(), 1);
    assert_eq!(prompts.iter().filter(|p| !p.system.contains(READER_IS)).count(), 1);
}

fn house<'a>(limits: &'a Limits, today: &'a str) -> House<'a> {
    House { limits, ai_name: Some("Fisch"), today }
}

#[test]
fn the_askers_gender_reaches_the_addressable_ai() {
    let room = room(vec![seat("de", "German", None), seat("ru", "Russian", Some(Gender::Female))]);
    let limits = Limits { translation_tokens_per_day: 10_000, questions_per_guest_per_day: 5 };
    let fake = Fake::new();
    let mut state = RoomState::default();
    let mut tally = Tally::default();
    let msg = from("ru", "quokka#2", "Fisch, что это значит?");
    let _ = now(converse(&fake, &room, &mut state, &mut tally, &house(&limits, DAY), &msg, false));

    let q = fake.questions().pop().unwrap();
    assert!(q.system.contains("The person who addressed you is a woman."), "got: {}", q.system);
}

#[test]
fn an_asker_without_the_setting_gets_no_hint_and_the_ai_has_no_gender_as_a_writer() {
    let room = room(vec![seat("de", "German", None), seat("ru", "Russian", Some(Gender::Female))]);
    let limits = Limits { translation_tokens_per_day: 10_000, questions_per_guest_per_day: 5 };
    let fake = Fake::new();
    let mut state = RoomState::default();
    let mut tally = Tally::default();
    let msg = from("de", "zebra#1", "Fisch, was heißt das?");
    let _ = now(converse(&fake, &room, &mut state, &mut tally, &house(&limits, DAY), &msg, true));

    let q = fake.questions().pop().unwrap();
    assert!(!q.system.contains(ASKER_IS), "got: {}", q.system);

    // Die Antwort der KI geht an den russischen Platz: die Leserin ist
    // bekannt, über die KI als Schreibende wird nichts gesagt.
    let answer_translation = fake.translations().pop().unwrap();
    assert!(answer_translation.user.contains("ANSWER"));
    assert!(answer_translation.system.contains("The reader of your translation is a woman"));
    assert!(!answer_translation.system.contains(WRITER_IS), "got: {}", answer_translation.system);
}

// --- der Hinweis an den Gastgeber --------------------------------------------

fn limits(tokens: u64) -> Limits {
    Limits { translation_tokens_per_day: tokens, questions_per_guest_per_day: 3 }
}

fn used_up(tokens: u64, day: &str) -> Tally {
    Tally { day: day.to_owned(), translation_tokens: tokens, ..Tally::default() }
}

#[test]
fn todays_allowance_is_the_daily_amount_plus_what_the_host_added() {
    let l = limits(100);
    let mut tally = Tally::default();
    assert_eq!(tally.allowed_today(&l), 100);
    tally.raised_tokens = 200;
    assert_eq!(tally.allowed_today(&l), 300);
    tally.spend(120);
    assert_eq!(tally.tokens_left(&l), 180);
}

#[test]
fn the_notice_is_due_exactly_once_per_exhaustion() {
    let l = limits(100);
    let mut tally = used_up(40, DAY);
    assert!(!tally.notice_due(&l), "nothing is used up yet");
    assert!(!tally.host_notified);

    tally.spend(70);
    assert!(tally.notice_due(&l));
    assert!(tally.host_notified);
    assert!(!tally.notice_due(&l), "the host was told already");
    assert!(!tally.notice_due(&l));
}

#[test]
fn raising_adds_exactly_one_daily_amount_and_translation_resumes() {
    let l = limits(100);
    let mut tally = used_up(120, DAY);
    assert!(tally.notice_due(&l));

    assert!(tally.raise(&l));
    assert_eq!(tally.raised_tokens, 100);
    assert_eq!(tally.allowed_today(&l), 200);
    assert_eq!(tally.tokens_left(&l), 80);
    assert!(!tally.host_notified, "a new exhaustion must be announced again");
    assert_eq!(tally.translation_tokens, 120, "what was spent stays spent");

    // Erneut verbraucht: der Hinweis ist wieder genau einmal fällig.
    assert!(!tally.notice_due(&l));
    tally.spend(80);
    assert!(tally.notice_due(&l));
    assert!(!tally.notice_due(&l));
    assert!(tally.raise(&l));
    assert_eq!(tally.allowed_today(&l), 300);
}

#[test]
fn raising_does_nothing_while_tokens_are_left() {
    // Ein doppelter Klick darf nicht doppelt erhöhen.
    let l = limits(100);
    let mut tally = used_up(100, DAY);
    assert!(tally.raise(&l));
    assert!(!tally.raise(&l));
    assert_eq!(tally.allowed_today(&l), 200);

    let mut fresh = used_up(10, DAY);
    assert!(!fresh.raise(&l));
    assert_eq!(fresh.raised_tokens, 0);
}

#[test]
fn a_new_day_starts_without_the_raise_and_without_the_mark() {
    let l = limits(100);
    let mut tally = used_up(100, DAY);
    assert!(tally.notice_due(&l));
    assert!(tally.raise(&l));
    tally.spend(100);
    assert!(tally.notice_due(&l));

    tally.start_day("2026-11-20");
    assert_eq!(tally.raised_tokens, 0);
    assert!(!tally.host_notified);
    assert_eq!(tally.allowed_today(&l), 100);
    assert_eq!(tally.tokens_left(&l), 100);
}

#[test]
fn a_daily_amount_of_zero_never_asks_the_host() {
    // Wer null einstellt, will keine Übersetzung. „Um weitere 0 erhöhen" wäre Unsinn.
    let l = limits(0);
    let mut tally = Tally::default();
    assert!(!tally.notice_due(&l));
    assert!(!tally.raise(&l));
    assert_eq!(tally.allowed_today(&l), 0);
}

fn delivery(mark: Mark) -> Delivery {
    Delivery { channel: "ru".to_owned(), text: "x".to_owned(), mark, from: Sender::Writer }
}

#[test]
fn only_an_empty_budget_counts_as_run_out() {
    assert!(!budget_ran_out(&[]));
    assert!(!budget_ran_out(&[delivery(Mark::Translated), delivery(Mark::Original)]));
    assert!(!budget_ran_out(&[delivery(Mark::Untranslated(Reason::NoConsent))]));
    assert!(!budget_ran_out(&[delivery(Mark::Notice)]));

    // Ein Ausfall des Anbieters ist kein leeres Kontingent, auch wenn der
    // Anbieter selbst von „quota" spricht.
    let fake = Fake::failing("HTTP 429: You exceeded your current quota.");
    let room = room(vec![seat("de", "German", None), seat("ru", "Russian", None)]);
    let mut state = RoomState::default();
    let out = now(relay(&fake, &room, &mut state, &from("de", "zebra#1", "Hallo")));
    assert!(matches!(out[0].mark, Mark::Untranslated(Reason::Failed(_))));
    assert!(!budget_ran_out(&out));
}

#[test]
fn in_a_conversation_the_host_is_due_a_notice_once_and_a_raise_brings_translation_back() {
    let room = room(vec![seat("de", "German", None), seat("ru", "Russian", None)]);
    let l = limits(60); // jede Übersetzung der Attrappe kostet 50
    let fake = Fake::new();
    let mut state = RoomState::default();
    let mut tally = Tally::default();
    let mut say = |tally: &mut Tally, day: &str, text: &str| {
        let msg = from("de", "zebra#1", text);
        now(converse(&fake, &room, &mut state, tally, &house(&l, day), &msg, true))
    };

    // 60 erlaubt: die erste kostet 50, die zweite überzieht auf 100.
    assert_eq!(say(&mut tally, DAY, "eins")[0].mark, Mark::Translated);
    let out = say(&mut tally, DAY, "zwei");
    assert_eq!(out[0].mark, Mark::Translated);
    assert!(!budget_ran_out(&out));

    // Leer: die Nachricht kommt an, unübersetzt, und der Hinweis ist fällig.
    let out = say(&mut tally, DAY, "drei");
    assert_eq!(out[0].text, "drei");
    assert!(budget_ran_out(&out));
    assert!(tally.notice_due(&l));

    // Weiter leer: kein zweiter Hinweis.
    let out = say(&mut tally, DAY, "vier");
    assert!(budget_ran_out(&out));
    assert!(!tally.notice_due(&l));

    // Der Gastgeber erhöht: 120 erlaubt, 100 verbraucht, es wird wieder übersetzt.
    assert!(tally.raise(&l));
    let out = say(&mut tally, DAY, "fünf");
    assert_eq!(out[0].mark, Mark::Translated);
    assert!(!budget_ran_out(&out));

    // Wieder leer: der Hinweis ist ein weiteres Mal fällig.
    let out = say(&mut tally, DAY, "sechs");
    assert!(budget_ran_out(&out));
    assert!(tally.notice_due(&l));

    // Am nächsten Tag ist alles zurückgesetzt.
    let out = say(&mut tally, "2026-11-20", "sieben");
    assert_eq!(out[0].mark, Mark::Translated);
    assert_eq!(tally.raised_tokens, 0);
    assert!(!tally.host_notified);
}
