//! Tests der Teile der Tür, die ohne Discord auskommen.

use rusty_fish_discord::config::Config;
use rusty_fish_discord::render::{chunks, hook_name, render};
use rusty_fish_discord::state::Store;
use rusty_fish_room::{Delivery, Mark, Reason};

const GOOD: &str = r#"
model = "some-model"
host = "100"

[[room]]
pass_through = ["English"]
context_lines = 4

  [[room.seat]]
  channel = "111"
  lang = "German"

  [[room.seat]]
  channel = "222"
  lang = "Russian"
"#;

#[test]
fn reads_a_room_with_two_seats() {
    let c = Config::parse(GOOD).unwrap();
    assert_eq!(c.model, "some-model");
    assert_eq!(c.base_url, None);
    assert_eq!(c.host, "100");
    assert_eq!(c.rooms.len(), 1);
    assert_eq!(c.rooms[0].context_lines, 4);
    assert_eq!(c.rooms[0].pass_through, ["English"]);
    assert_eq!(c.room_index("222"), Some(0));
    assert_eq!(c.room_index("333"), None);
    assert_eq!(c.lang_of("111"), Some("German"));
    assert_eq!(c.lang_of("333"), None);
}

#[test]
fn context_lines_have_a_default() {
    let text = GOOD.replace("context_lines = 4", "");
    assert_eq!(Config::parse(&text).unwrap().rooms[0].context_lines, 10);
}

fn refused(text: &str, expect: &str) {
    let err = Config::parse(text).expect_err("this configuration must be refused");
    assert!(err.contains(expect), "expected {expect:?} in: {err}");
}

#[test]
fn refuses_what_cannot_work() {
    refused(&GOOD.replace("some-model", " "), "model");
    refused(&GOOD.replace("host = \"100\"", "host = \"me\""), "host");
    refused(&GOOD.replace("\"222\"", "\"111\""), "111");
    refused(&GOOD.replace("\"222\"", "\"#general\""), "#general");
    refused(&GOOD.replace("lang = \"Russian\"", "lang = \"\""), "lang");
    refused("model = \"m\"\nhost = \"1\"\n", "room");
    let one_seat = GOOD.split("  [[room.seat]]\n  channel = \"222\"").next().unwrap().to_owned();
    refused(&one_seat, "two seats");
    refused(&GOOD.replace("host =", "hots ="), "hots");
}

/// Das mitgelieferte Beispiel und der Leser dürfen nicht auseinanderlaufen.
#[test]
fn the_shipped_example_is_refused_until_filled_in_and_valid_after() {
    let example = include_str!("../../rusty-fish.example.toml");
    refused(example, "host");
    let filled = example.replace("host = \"000000000000000000\"", "host = \"42\"");
    let c = Config::parse(&filled).unwrap();
    assert_eq!(c.rooms[0].seats.len(), 2);
    assert_eq!(c.rooms[0].pass_through, ["English"]);
}

fn delivery(text: &str, mark: Mark) -> Delivery {
    Delivery { channel: "222".to_owned(), text: text.to_owned(), mark }
}

#[test]
fn a_translation_and_a_fitting_original_carry_no_note() {
    assert_eq!(render(&delivery("Привет", Mark::Translated)), "Привет");
    assert_eq!(render(&delivery("See you", Mark::Original)), "See you");
}

#[test]
fn an_untranslated_message_says_why() {
    let no_consent = render(&delivery("Привет", Mark::Untranslated(Reason::NoConsent)));
    assert!(no_consent.starts_with("Привет\n-# "), "got {no_consent:?}");
    assert!(no_consent.contains("not translated"));

    let failed =
        render(&delivery("Hallo", Mark::Untranslated(Reason::Failed("HTTP 429: quota".into()))));
    assert!(failed.starts_with("Hallo\n-# "), "got {failed:?}");
    assert!(failed.contains("HTTP 429: quota"));
}

#[test]
fn a_long_reason_is_cut_but_the_message_never_is() {
    let long = "x".repeat(500);
    let out = render(&delivery("Hallo", Mark::Untranslated(Reason::Failed(long))));
    assert!(out.starts_with("Hallo\n"));
    assert!(out.chars().count() < 250, "the note must stay short: {}", out.chars().count());
}

#[test]
fn chunks_lose_nothing_and_respect_the_limit() {
    let text = "я".repeat(25) + "\n" + &"b".repeat(30);
    let parts = chunks(&text, 10);
    assert!(parts.iter().all(|p| p.chars().count() <= 10));
    assert_eq!(parts.concat(), text);
    assert_eq!(chunks("short", 10), ["short"]);
    assert!(chunks("", 10).is_empty());
}

#[test]
fn hook_names_are_ones_discord_accepts() {
    assert_eq!(hook_name("Robin"), "Robin");
    assert_eq!(hook_name("  "), "Guest");
    assert!(!hook_name("Discord Fan").to_lowercase().contains("discord"));
    assert!(!hook_name("clyde").to_lowercase().contains("clyde"));
    assert_eq!(hook_name(&"ä".repeat(200)).chars().count(), 80);
}

fn temp_file(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rusty-fish-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("state.json")
}

#[test]
fn the_store_survives_a_restart() {
    let path = temp_file("restart");
    let mut store = Store::open(&path).unwrap();
    assert!(!store.has_consented("7"));
    assert_eq!(store.seen("111"), None);
    store.consent("7");
    store.saw("111", 50);
    store.saw("111", 40); // nie rückwärts
    store.save().unwrap();

    let again = Store::open(&path).unwrap();
    assert!(again.has_consented("7"));
    assert!(!again.has_consented("8"));
    assert_eq!(again.seen("111"), Some(50));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn a_damaged_store_fails_loudly_instead_of_forgetting() {
    let path = temp_file("damaged");
    std::fs::write(&path, "{ not json").unwrap();
    assert!(Store::open(&path).is_err());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn the_store_holds_no_message_text() {
    let path = temp_file("notext");
    let mut store = Store::open(&path).unwrap();
    store.consent("7");
    store.saw("111", 50);
    store.save().unwrap();
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(raw.contains("consented") && raw.contains("last_seen"));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
