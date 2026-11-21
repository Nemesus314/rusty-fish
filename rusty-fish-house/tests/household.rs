//! Tests des Ablaufs einer Nachricht: von der Ankunft bis zu dem, was eine
//! Tür abzusetzen hat. Keine Tür, kein Netz.
//!
//! Bis zum 21.11.2026 stand dieser Ablauf in der Discord-Tür und war nur von
//! Hand in Discord gemessen.

use rusty_fish_core::{Model, ModelError, Prompt, Reply};
use rusty_fish_house::config::Config;
use rusty_fish_house::household::{Arrived, Household, Steps, Terms};
use rusty_fish_house::invite::Due;
use rusty_fish_house::texts::{
    KEEP_LABEL, budget_notice, consent_text, consent_text_in, nothing_to_raise, raise_label,
    raised_notice,
};
use rusty_fish_room::tongue::Tongue;
use rusty_fish_room::{BUDGET_USED_UP, Mark, Reason, Sender};
use std::future::Future;
use std::path::PathBuf;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

const SERVICE: &str = "ai.example";
const TODAY: &str = "2026-11-21";

/// Attrappe einer KI: antwortet sofort, immer dasselbe, und merkt sich,
/// was sie gefragt wurde.
struct Fake {
    reply: Result<String, String>,
    tokens: Option<u64>,
    asked: Mutex<Vec<Prompt>>,
}

impl Fake {
    fn new() -> Fake {
        Fake { reply: Ok("TRANSLATED".to_owned()), tokens: Some(10), asked: Mutex::new(Vec::new()) }
    }

    fn failing() -> Fake {
        Fake { reply: Err("HTTP 500".to_owned()), ..Fake::new() }
    }

    fn calls(&self) -> usize {
        self.asked.lock().unwrap().len()
    }

    /// Alles, was die KI je zu lesen bekam, in einem Stück.
    fn everything_read(&self) -> String {
        self.asked.lock().unwrap().iter().map(|p| format!("{}\n{}\n", p.system, p.user)).collect()
    }
}

impl Model for Fake {
    async fn complete(&self, prompt: &Prompt) -> Result<Reply, ModelError> {
        self.asked.lock().unwrap().push(prompt.clone());
        match &self.reply {
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

/// Zwei Tische: der Gastgeber (100) liest Deutsch an 111, sein Gast (7)
/// liest an 222 in `guest_lang`.
fn settings(guest_lang: &str, limit: u64) -> Config {
    Config::parse(&format!(
        r#"
model = "some-model"
host = "100"

[limits]
translation_tokens_per_day = {limit}

[[room]]

  [[room.seat]]
  channel = "111"
  lang = "German"
  owner = "100"

  [[room.seat]]
  channel = "222"
  lang = "{guest_lang}"
  owner = "7"

[[person]]
id = "7"
invited = true
"#
    ))
    .unwrap()
}

/// Ein eigener Ordner je Test, damit sie einander nicht in die Datei schreiben.
fn state_file(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rusty-fish-household-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("state.json")
}

fn terms(config: &Config) -> Terms<'_> {
    Terms { config, service: SERVICE, today: TODAY }
}

fn from_host(text: &'static str, mark: u64) -> Arrived<'static> {
    Arrived { channel: "111", author: "100", text, mark }
}

fn from_guest(text: &'static str, mark: u64) -> Arrived<'static> {
    Arrived { channel: "222", author: "7", text, mark }
}

fn handle(house: &mut Household, fake: &Fake, config: &Config, msg: &Arrived<'_>) -> Option<Steps> {
    now(house.message(fake, &terms(config), msg))
}

// --- Zustellen und die Lesemarke -----------------------------------------------

#[test]
fn a_message_goes_to_the_other_table_and_moves_the_reading_mark() {
    let config = settings("Russian", 100_000);
    let mut house = Household::open(&config, &state_file("relay")).unwrap();
    let fake = Fake::new();

    let steps = handle(&mut house, &fake, &config, &from_host("Guten Morgen", 5)).unwrap();
    assert_eq!(steps.deliveries.len(), 1);
    let delivery = &steps.deliveries[0];
    assert_eq!(delivery.channel, "222");
    assert_eq!(delivery.text, "TRANSLATED");
    assert_eq!(delivery.mark, Mark::Translated);
    assert_eq!(delivery.from, Sender::Writer);
    // Der Gastgeber hat mit dem Start zugestimmt, und das Kontingent reicht.
    assert_eq!(steps.ask_consent, None);
    assert_eq!(steps.tell_host, None);
    assert_eq!(house.seen("111"), Some(5));
    assert_eq!(house.seen("222"), None);
}

#[test]
fn a_message_that_was_handled_is_not_handled_again() {
    let config = settings("Russian", 100_000);
    let mut house = Household::open(&config, &state_file("twice")).unwrap();
    let fake = Fake::new();

    assert!(handle(&mut house, &fake, &config, &from_host("eins", 5)).is_some());
    // Nachholen und Live-Meldung überschneiden sich: dieselbe und eine ältere.
    assert_eq!(handle(&mut house, &fake, &config, &from_host("eins", 5)), None);
    assert_eq!(handle(&mut house, &fake, &config, &from_host("null", 4)), None);
    assert_eq!(fake.calls(), 1, "a second handling would be paid for a second time");
    assert_eq!(house.seen("111"), Some(5));
    // Die Lesemarke gilt je Kanal.
    house.consent("7", SERVICE).unwrap();
    assert!(handle(&mut house, &fake, &config, &from_guest("раз", 3)).is_some());
}

#[test]
fn a_channel_outside_every_room_is_none_of_the_fishs_business() {
    let config = settings("Russian", 100_000);
    let mut house = Household::open(&config, &state_file("outside")).unwrap();
    let fake = Fake::new();

    let stray = Arrived { channel: "999", author: "100", text: "hallo", mark: 9 };
    assert_eq!(handle(&mut house, &fake, &config, &stray), None);
    assert_eq!(fake.calls(), 0);
    assert_eq!(house.seen("999"), None);
}

#[test]
fn nothing_counts_as_read_until_the_door_says_save() {
    let config = settings("Russian", 100_000);
    let path = state_file("save");
    let fake = Fake::new();

    // Die Tür stürzt ab, bevor sie zugestellt und gespeichert hat.
    let mut house = Household::open(&config, &path).unwrap();
    assert!(handle(&mut house, &fake, &config, &from_host("eins", 5)).is_some());
    drop(house);

    // Der nächste Start weiß von nichts und verarbeitet die Nachricht noch einmal.
    let mut house = Household::open(&config, &path).unwrap();
    assert_eq!(house.seen("111"), None);
    assert!(handle(&mut house, &fake, &config, &from_host("eins", 5)).is_some());
    house.save().unwrap();
    drop(house);

    // Nach dem Speichern gilt sie als gelesen.
    let mut house = Household::open(&config, &path).unwrap();
    assert_eq!(house.seen("111"), Some(5));
    assert_eq!(handle(&mut house, &fake, &config, &from_host("eins", 5)), None);
}

// --- Die Zustimmung ------------------------------------------------------------

#[test]
fn a_guest_is_asked_once_per_run_and_until_then_nothing_reaches_the_ai() {
    let config = settings("Russian", 100_000);
    let mut house = Household::open(&config, &state_file("ask")).unwrap();
    let fake = Fake::new();

    let first = handle(&mut house, &fake, &config, &from_guest("Привет", 1)).unwrap();
    assert_eq!(first.deliveries.len(), 1);
    assert_eq!(first.deliveries[0].channel, "111");
    assert_eq!(first.deliveries[0].text, "Привет", "delivered all the same, in the original");
    assert_eq!(first.deliveries[0].mark, Mark::Untranslated(Reason::NoConsent));
    // Die Bitte steht fest im Programm, in der Sprache des Tisches, und nennt den Dienst.
    assert_eq!(first.ask_consent, Some(consent_text_in(Tongue::Russian, SERVICE)));

    let second = handle(&mut house, &fake, &config, &from_guest("Ты тут?", 2)).unwrap();
    assert_eq!(second.deliveries[0].mark, Mark::Untranslated(Reason::NoConsent));
    assert_eq!(second.ask_consent, None, "asked once per run, not under every message");

    assert_eq!(fake.calls(), 0, "no word of the guest may reach the AI before consent");
}

#[test]
fn consent_is_stored_at_once_and_holds_after_a_restart() {
    let config = settings("Russian", 100_000);
    let path = state_file("consent");
    let fake = Fake::new();

    let mut house = Household::open(&config, &path).unwrap();
    house.consent("7", SERVICE).unwrap();
    let steps = handle(&mut house, &fake, &config, &from_guest("Привет", 1)).unwrap();
    assert_eq!(steps.deliveries[0].mark, Mark::Translated);
    assert_eq!(steps.ask_consent, None);
    drop(house);

    // Ohne ein `save` der Tür: die Zustimmung hat sich selbst gespeichert.
    let mut house = Household::open(&config, &path).unwrap();
    let steps = handle(&mut house, &fake, &config, &from_guest("Привет", 1)).unwrap();
    assert_eq!(steps.deliveries[0].mark, Mark::Translated);
    assert_eq!(steps.ask_consent, None);
}

#[test]
fn at_a_table_in_a_fourth_language_the_ai_translates_the_request() {
    let config = settings("French", 100_000);
    let mut house = Household::open(&config, &state_file("french")).unwrap();
    let fake = Fake::new();

    let steps = handle(&mut house, &fake, &config, &from_guest("Bonjour", 1)).unwrap();
    assert_eq!(steps.ask_consent.as_deref(), Some("TRANSLATED"));
    // Übersetzt wurde die Bitte, nicht die Nachricht des Gasts.
    assert_eq!(fake.calls(), 1);
    let read = fake.everything_read();
    assert!(read.contains(SERVICE), "the request names the service");
    assert!(!read.contains("Bonjour"), "the guest has not agreed yet");
}

#[test]
fn if_the_ai_fails_the_request_comes_in_english_rather_than_not_at_all() {
    let config = settings("French", 100_000);
    let mut house = Household::open(&config, &state_file("french-down")).unwrap();
    let fake = Fake::failing();

    let steps = handle(&mut house, &fake, &config, &from_guest("Bonjour", 1)).unwrap();
    assert_eq!(steps.ask_consent, Some(consent_text(SERVICE)));
}

// --- Das Kontingent und der Hinweis an den Gastgeber ------------------------------

#[test]
fn the_host_hears_once_that_the_budget_is_used_up_and_can_raise_it() {
    // Zehn Tokens am Tag, und jede Übersetzung kostet zehn.
    let config = settings("Russian", 10);
    let mut house = Household::open(&config, &state_file("budget")).unwrap();
    let fake = Fake::new();

    let first = handle(&mut house, &fake, &config, &from_host("eins", 1)).unwrap();
    assert_eq!(first.deliveries[0].mark, Mark::Translated);
    assert_eq!(first.tell_host, None);

    // Das Gespräch geht weiter, unübersetzt und ehrlich markiert.
    let second = handle(&mut house, &fake, &config, &from_host("zwei", 2)).unwrap();
    assert_eq!(second.deliveries[0].text, "zwei");
    match &second.deliveries[0].mark {
        Mark::Untranslated(Reason::Failed(why)) => assert!(why.contains(BUDGET_USED_UP)),
        other => panic!("expected an untranslated delivery, got {other:?}"),
    }
    let notice = second.tell_host.expect("the host has to hear about it");
    assert_eq!(notice.text, budget_notice(10, 10));
    assert_eq!(notice.raise_label, raise_label(10));
    assert_eq!(notice.keep_label, KEEP_LABEL);

    let third = handle(&mut house, &fake, &config, &from_host("drei", 3)).unwrap();
    assert_eq!(third.tell_host, None, "once per exhaustion, not under every message");
    assert_eq!(fake.calls(), 1, "an empty budget means no call at all");

    // Der Gastgeber erhöht. Ein zweiter Klick erhöht nicht noch einmal.
    let (text, saved) = house.raise(&terms(&config));
    assert_eq!((text, saved), (raised_notice(20, 10), Ok(())));
    let (text, saved) = house.raise(&terms(&config));
    assert_eq!((text, saved), (nothing_to_raise(20, 10), Ok(())));

    let fourth = handle(&mut house, &fake, &config, &from_host("vier", 4)).unwrap();
    assert_eq!(fourth.deliveries[0].mark, Mark::Translated);

    // Wieder leer: nach einer Erhöhung wird die neue Erschöpfung wieder gemeldet.
    let fifth = handle(&mut house, &fake, &config, &from_host("fünf", 5)).unwrap();
    assert_eq!(fifth.tell_host.map(|n| n.text), Some(budget_notice(20, 10)));
}

#[test]
fn a_click_on_yesterdays_notice_finds_a_fresh_day_and_raises_nothing() {
    let config = settings("Russian", 10);
    let mut house = Household::open(&config, &state_file("new-day")).unwrap();
    let fake = Fake::new();
    assert!(handle(&mut house, &fake, &config, &from_host("eins", 1)).is_some());
    assert!(handle(&mut house, &fake, &config, &from_host("zwei", 2)).unwrap().tell_host.is_some());

    let tomorrow = Terms { config: &config, service: SERVICE, today: "2026-11-22" };
    let (text, saved) = house.raise(&tomorrow);
    assert_eq!((text, saved), (nothing_to_raise(10, 10), Ok(())));
}

// --- Die Einladung ---------------------------------------------------------------

#[test]
fn an_invitation_stays_due_until_it_has_reached_the_guest() {
    let config = settings("Russian", 100_000);
    let path = state_file("invite");
    let mut house = Household::open(&config, &path).unwrap();
    let due = [Due { person: "7".to_owned(), channel: "222".to_owned() }];

    // Die Tür konnte nicht zustellen: der nächste Versuch kommt wieder.
    let pending = house.invitations(&config).expect("the guest is invited");
    assert_eq!(pending.due, due);
    house.invitations_told(pending, &[]).unwrap();
    let pending = house.invitations(&config).expect("not reached, so still due");
    assert_eq!(pending.due, due);

    // Zugestellt: danach nie wieder, auch nicht nach einem Neustart.
    house.invitations_told(pending, &["7".to_owned()]).unwrap();
    assert_eq!(house.invitations(&config), None);
    drop(house);
    let house = Household::open(&config, &path).unwrap();
    assert_eq!(house.invitations(&config), None);
}

// --- Die Zustimmung gilt einem KI-Dienst ---------------------------------------

#[test]
fn consent_is_given_to_one_ai_service_and_a_new_one_asks_again() {
    let config = settings("Russian", 100_000);
    let mut house = Household::open(&config, &state_file("service")).unwrap();
    let fake = Fake::new();
    house.consent("7", SERVICE).unwrap();
    assert!(handle(&mut house, &fake, &config, &from_guest("раз", 1)).unwrap().ask_consent.is_none());
    assert_eq!(fake.calls(), 1);

    // Der Gastgeber hat den Dienst gewechselt. Dem neuen hat der Gast nie zugestimmt.
    let other = Terms { config: &config, service: "other.example", today: TODAY };
    let steps = now(house.message(&fake, &other, &from_guest("два", 2))).unwrap();
    assert_eq!(steps.deliveries[0].mark, Mark::Untranslated(Reason::NoConsent));
    assert_eq!(steps.ask_consent, Some(consent_text_in(Tongue::Russian, "other.example")));
    assert_eq!(fake.calls(), 1, "no word may reach a service the guest did not agree to");

    // Nach der Zustimmung zum neuen Dienst geht es weiter.
    house.consent("7", "other.example").unwrap();
    let steps = now(house.message(&fake, &other, &from_guest("три", 3))).unwrap();
    assert_eq!(steps.deliveries[0].mark, Mark::Translated);
}

#[test]
fn a_consent_from_before_services_were_noted_counts_for_the_service_then_in_use() {
    let config = settings("Russian", 100_000);
    let path = state_file("old-consent");
    // So sah die Datei bis zum 21.11.2026 aus: wer, aber nicht wem.
    std::fs::write(&path, r#"{"consented": ["7"], "last_seen": {}}"#).unwrap();
    let fake = Fake::new();

    let mut house = Household::open(&config, &path).unwrap();
    let steps = handle(&mut house, &fake, &config, &from_guest("раз", 1)).unwrap();
    assert_eq!(steps.deliveries[0].mark, Mark::Translated, "nobody is asked a second time");
    house.save().unwrap();
    drop(house);

    // Von da an ist der Dienst gemerkt, und ein Wechsel fragt neu.
    let mut house = Household::open(&config, &path).unwrap();
    let other = Terms { config: &config, service: "other.example", today: TODAY };
    let steps = now(house.message(&fake, &other, &from_guest("два", 2))).unwrap();
    assert_eq!(steps.deliveries[0].mark, Mark::Untranslated(Reason::NoConsent));
    assert!(steps.ask_consent.is_some());
}
