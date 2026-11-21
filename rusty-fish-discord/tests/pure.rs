//! Tests der Teile der Tür, die ohne Verbindung zu Discord auskommen: die
//! Erwähnung des Bots, die graue Zeile, die Länge einer Nachricht und die
//! Namen, die ein Webhook tragen darf.

use rusty_fish_discord::mention::{mention_as_call, mention_question};
use rusty_fish_discord::render::{MESSAGE_LIMIT, chunks, hook_name, render, render_in};
use rusty_fish_house::texts::{consent_text_in, invitation};
use rusty_fish_room::tongue::Tongue;
use rusty_fish_room::{BUDGET_USED_UP, Delivery, Mark, Reason, Sender};

const ALL: [Tongue; 3] = [Tongue::German, Tongue::English, Tongue::Russian];

fn delivery(text: &str, mark: Mark) -> Delivery {
    Delivery { channel: "222".to_owned(), text: text.to_owned(), mark, from: Sender::Writer }
}

// --- die echte Erwähnung -----------------------------------------------------

#[test]
fn a_mention_of_the_bot_at_the_start_is_a_question() {
    assert_eq!(mention_question("<@42> wie spät ist es?", 42), Some("wie spät ist es?"));
    // Die alte Schreibweise mit Ausrufezeichen.
    assert_eq!(mention_question("<@!42> как дела?", 42), Some("как дела?"));
    assert_eq!(mention_question("   <@42>   hallo  ", 42), Some("hallo"));
    assert_eq!(mention_question("<@42>, hallo", 42), Some("hallo"));
    assert_eq!(mention_question("<@42>: hallo", 42), Some("hallo"));
    // Das Kürzel endet von selbst; ein Leerzeichen danach ist nicht nötig.
    assert_eq!(mention_question("<@42>hallo", 42), Some("hallo"));
}

#[test]
fn other_mentions_are_no_question() {
    assert_eq!(mention_question("hallo <@42>, wie geht es?", 42), None, "middle of a sentence");
    assert_eq!(mention_question("<@43> hallo", 42), None, "another user");
    assert_eq!(mention_question("<@420> hallo", 42), None, "an id that only starts the same");
    assert_eq!(mention_question("<@4> hallo", 42), None);
    assert_eq!(mention_question("<@&42> hallo", 42), None, "a role");
    assert_eq!(mention_question("<#42> hallo", 42), None, "a channel");
    assert_eq!(mention_question("<@42>", 42), None, "nothing asked");
    assert_eq!(mention_question("<@42> , : ", 42), None, "nothing asked");
    assert_eq!(mention_question("<@42 hallo", 42), None, "not closed");
    assert_eq!(mention_question("hallo", 42), None);
    assert_eq!(mention_question("", 42), None);
    // Solange der Bot seine eigene ID nicht kennt, spricht ihn nichts an.
    assert_eq!(mention_question("<@0> hallo", 0), None);
}

#[test]
fn the_room_gets_the_call_name_instead_of_the_discord_code() {
    assert_eq!(
        mention_as_call("<@42> wie spät ist es?", 42, "Fish").as_deref(),
        Some("Fish, wie spät ist es?")
    );
    assert_eq!(mention_as_call("<@!42>: как дела?", 42, "Fish").as_deref(), Some("Fish, как дела?"));
    assert_eq!(mention_as_call("hallo <@42>", 42, "Fish"), None);
    assert_eq!(mention_as_call("<@43> hallo", 42, "Fish"), None);
    // Was dabei herauskommt, versteht die Ansprechregel des Kerns.
    let handed = mention_as_call("<@42> hallo?", 42, "Fish").unwrap();
    assert_eq!(rusty_fish_core::addressed_any(&handed, &["Fish", "Фиш"]), Some("hallo?"));
}

// --- wie eine Zustellung aussieht ---------------------------------------------

#[test]
fn a_notice_is_shown_as_it_is() {
    assert_eq!(render(&delivery("No questions left.", Mark::Notice)), "No questions left.");
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

// --- die graue Zeile in drei Sprachen ----------------------------------------

#[test]
fn a_translation_carries_no_note_in_any_language() {
    for tongue in ALL {
        assert_eq!(render_in(&delivery("Привет", Mark::Translated), tongue), "Привет");
        assert_eq!(render_in(&delivery("See you", Mark::Original), tongue), "See you");
        assert_eq!(render_in(&delivery("A notice.", Mark::Notice), tongue), "A notice.");
    }
}

#[test]
fn the_grey_line_speaks_the_language_of_the_table() {
    let no_consent = delivery("Привет", Mark::Untranslated(Reason::NoConsent));
    let en = render_in(&no_consent, Tongue::English);
    assert_eq!(en, render(&no_consent), "English is what it always said");
    let de = render_in(&no_consent, Tongue::German);
    assert!(de.starts_with("Привет\n-# nicht übersetzt: "), "got {de:?}");
    assert!(de.contains("zugestimmt"));
    let ru = render_in(&no_consent, Tongue::Russian);
    assert!(ru.starts_with("Привет\n-# не переведено: "), "got {ru:?}");
    assert!(ru.contains("соглас"));
}

#[test]
fn a_used_up_budget_is_said_in_the_language_of_the_table() {
    let out = delivery("Hallo", Mark::Untranslated(Reason::Failed(BUDGET_USED_UP.to_owned())));
    assert_eq!(render_in(&out, Tongue::English), render(&out));
    let de = render_in(&out, Tongue::German);
    assert!(de.starts_with("Hallo\n-# nicht übersetzt: "), "got {de:?}");
    assert!(de.contains("Kontingent") && !de.contains("budget"), "got {de:?}");
    let ru = render_in(&out, Tongue::Russian);
    assert!(ru.starts_with("Hallo\n-# не переведено: "), "got {ru:?}");
    assert!(ru.contains("лимит") && !ru.contains("budget"), "got {ru:?}");
}

#[test]
fn any_other_reason_stays_as_the_service_gave_it() {
    let out = delivery("Hallo", Mark::Untranslated(Reason::Failed("HTTP 429: quota".into())));
    assert_eq!(render_in(&out, Tongue::German), "Hallo\n-# nicht übersetzt: HTTP 429: quota");
    assert_eq!(render_in(&out, Tongue::Russian), "Hallo\n-# не переведено: HTTP 429: quota");
    assert_eq!(render_in(&out, Tongue::English), "Hallo\n-# not translated: HTTP 429: quota");
    // Der Grund wird gekürzt, die Nachricht nie.
    let long = delivery("Hallo", Mark::Untranslated(Reason::Failed("x".repeat(500))));
    for tongue in ALL {
        let text = render_in(&long, tongue);
        assert!(text.starts_with("Hallo\n") && text.chars().count() < 250);
    }
}

#[test]
fn every_fixed_text_fits_into_one_discord_message() {
    let service = "a-rather-long-host-name.of-some-ai-service.example";
    for tongue in ALL {
        assert!(consent_text_in(tongue, service).chars().count() < MESSAGE_LIMIT);
        assert!(invitation(tongue, Some(&"z".repeat(5000))).chars().count() < MESSAGE_LIMIT);
    }
}
