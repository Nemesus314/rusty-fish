//! Tests der Raum-Logik. Keine Tür, kein Netz, die KI ist eine Attrappe.

use rusty_fish_core::{Model, ModelError, Prompt, Reply};
use rusty_fish_room::{Delivery, Incoming, Mark, Reason, Room, RoomState, Seat, Sender, relay};
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

    fn prompts(&self) -> Vec<Prompt> {
        self.asked.lock().unwrap().clone()
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

fn seat(channel: &str, lang: &str) -> Seat {
    Seat::new(channel, lang)
}

fn two_seats() -> Room {
    Room {
        seats: vec![seat("de", "German"), seat("ru", "Russian")],
        pass_through: vec!["English".to_owned()],
        context_lines: 3,
    }
}

fn from<'a>(channel: &'a str, author: &'a str, text: &'a str) -> Incoming<'a> {
    Incoming { channel, author, text, consented: true, author_gender: None }
}

#[test]
fn a_message_appears_translated_at_the_other_seat() {
    let fake = Fake::says("Привет!");
    let mut state = RoomState::default();
    let out = now(relay(&fake, &two_seats(), &mut state, &from("de", "n", "Hallo!")));
    assert_eq!(
        out,
        [Delivery {
            channel: "ru".to_owned(),
            text: "Привет!".to_owned(),
            mark: Mark::Translated,
            from: Sender::Writer,
        }]
    );
    let asked = fake.prompts();
    assert_eq!(asked.len(), 1);
    assert!(asked[0].system.contains("into Russian"));
}

#[test]
fn nothing_comes_back_to_the_writers_own_seat() {
    let fake = Fake::says("Привет!");
    let mut state = RoomState::default();
    let out = now(relay(&fake, &two_seats(), &mut state, &from("de", "n", "Hallo!")));
    assert!(out.iter().all(|d| d.channel != "de"));
}

#[test]
fn a_skip_delivers_the_original_and_says_that_is_fine() {
    let fake = Fake::says("=SKIP=");
    let mut state = RoomState::default();
    let out = now(relay(&fake, &two_seats(), &mut state, &from("de", "n", "See you tomorrow")));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].text, "See you tomorrow");
    assert_eq!(out[0].mark, Mark::Original);
}

#[test]
fn the_fish_may_cough_but_never_swallows_the_message() {
    let fake = Fake::fails("quota exceeded");
    let mut state = RoomState::default();
    let out = now(relay(&fake, &two_seats(), &mut state, &from("de", "n", "Hallo!")));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].text, "Hallo!");
    match &out[0].mark {
        Mark::Untranslated(Reason::Failed(why)) => assert!(why.contains("quota exceeded")),
        other => panic!("expected a marked failure, got {other:?}"),
    }
}

#[test]
fn without_consent_no_word_reaches_the_model() {
    let fake = Fake::says("should never be asked");
    let mut state = RoomState::default();
    let mut msg = from("ru", "a", "Привет, это личное");
    msg.consented = false;
    let out = now(relay(&fake, &two_seats(), &mut state, &msg));
    assert_eq!(
        out,
        [Delivery {
            channel: "de".to_owned(),
            text: "Привет, это личное".to_owned(),
            mark: Mark::Untranslated(Reason::NoConsent),
            from: Sender::Writer,
        }]
    );
    assert!(fake.prompts().is_empty());

    // Auch später, als Kontext einer fremden Nachricht, darf der Satz nicht
    // zur KI gelangen.
    let _ = now(relay(&fake, &two_seats(), &mut state, &from("de", "n", "Wie bitte?")));
    for p in fake.prompts() {
        assert!(!p.user.contains("это личное"), "unconsented text leaked as context");
    }
}

#[test]
fn seats_sharing_a_language_share_one_translation() {
    let fake = Fake::says("Привет!");
    let room = Room {
        seats: vec![seat("de", "German"), seat("ru1", "Russian"), seat("ru2", "russian ")],
        pass_through: vec![],
        context_lines: 0,
    };
    let mut state = RoomState::default();
    let out = now(relay(&fake, &room, &mut state, &from("de", "n", "Hallo!")));
    assert_eq!(out.len(), 2);
    assert!(out.iter().all(|d| d.text == "Привет!" && d.mark == Mark::Translated));
    assert_eq!(fake.prompts().len(), 1, "one language, one paid translation");
}

#[test]
fn every_other_language_gets_its_own_translation() {
    let fake = Fake::says("x");
    let room = Room {
        seats: vec![seat("de", "German"), seat("ru", "Russian"), seat("ko", "Korean")],
        pass_through: vec![],
        context_lines: 0,
    };
    let mut state = RoomState::default();
    let out = now(relay(&fake, &room, &mut state, &from("ru", "a", "Привет")));
    let channels: Vec<&str> = out.iter().map(|d| d.channel.as_str()).collect();
    assert_eq!(channels, ["de", "ko"]);
    let targets: Vec<bool> =
        fake.prompts().iter().map(|p| p.system.contains("into German")).collect();
    assert_eq!(targets, [true, false]);
}

#[test]
fn a_channel_outside_the_room_is_ignored() {
    let fake = Fake::says("x");
    let mut state = RoomState::default();
    let out = now(relay(&fake, &two_seats(), &mut state, &from("elsewhere", "n", "Hallo!")));
    assert!(out.is_empty());
    assert!(fake.prompts().is_empty());
}

#[test]
fn earlier_messages_go_along_as_context_up_to_the_limit() {
    let fake = Fake::says("x");
    let room = two_seats(); // context_lines: 3
    let mut state = RoomState::default();
    for text in ["eins", "zwei", "drei", "vier"] {
        let _ = now(relay(&fake, &room, &mut state, &from("de", "n", text)));
    }
    let _ = now(relay(&fake, &room, &mut state, &from("ru", "a", "пять")));
    let last = fake.prompts().pop().unwrap();
    assert!(!last.user.contains("eins"), "only the last three lines may go along");
    for text in ["zwei", "drei", "vier"] {
        assert!(last.user.contains(text), "context is missing {text:?}");
    }
    // Die eigene Nachricht steht im Auftrag, nicht noch einmal im Kontext.
    assert_eq!(last.user.matches("пять").count(), 1);
}

#[test]
fn relaying_can_cross_threads() {
    fn assert_send<T: Send>(_: T) {}
    let fake = Fake::says("x");
    let room = two_seats();
    let mut state = RoomState::default();
    let msg = from("de", "n", "Hallo!");
    assert_send(relay(&fake, &room, &mut state, &msg));
}
