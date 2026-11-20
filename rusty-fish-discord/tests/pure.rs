//! Tests der Teile der Tür, die ohne Discord auskommen.

use rusty_fish_discord::clock::utc_date;
use rusty_fish_discord::config::Config;
use rusty_fish_discord::mention::{mention_as_call, mention_question};
use rusty_fish_discord::render::{
    KEEP_LABEL, budget_notice, chunks, grouped, hook_name, nothing_to_raise, raise_label,
    raised_notice, render,
};
use rusty_fish_discord::state::Store;
use rusty_fish_room::{Delivery, Gender, Limits, Mark, Reason, Sender, Tally};

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
    Delivery { channel: "222".to_owned(), text: text.to_owned(), mark, from: Sender::Writer }
}

// --- Kontingent und Name der KI in den Einstellungen -------------------------

#[test]
fn without_a_limits_section_the_cautious_defaults_apply() {
    let c = Config::parse(GOOD).unwrap();
    assert_eq!(c.limits, Limits::default());
    assert!(c.ai_names.is_empty());
    assert_eq!(c.ai_name(), None);
}

#[test]
fn limits_and_the_ai_name_are_read() {
    let text = GOOD.replace(
        "host = \"100\"",
        "host = \"100\"\nai_name = \"  Fisch \"\n\n[limits]\ntranslation_tokens_per_day = 5000\nquestions_per_guest_per_day = 3",
    );
    let c = Config::parse(&text).unwrap();
    assert_eq!(c.ai_names, ["Fisch"]);
    assert_eq!(c.ai_name(), Some("Fisch"));
    assert_eq!(
        c.limits,
        Limits { translation_tokens_per_day: 5000, questions_per_guest_per_day: 3 }
    );
    assert_eq!(c.rooms.len(), 1, "the room after the limits section must still be read");
}

#[test]
fn a_single_limit_may_be_set_and_the_other_keeps_its_default() {
    let text =
        GOOD.replace("host = \"100\"", "host = \"100\"\n\n[limits]\nquestions_per_guest_per_day = 0");
    let c = Config::parse(&text).unwrap();
    assert_eq!(c.limits.questions_per_guest_per_day, 0);
    assert_eq!(
        c.limits.translation_tokens_per_day,
        Limits::default().translation_tokens_per_day
    );
}

#[test]
fn an_empty_ai_name_means_off_and_a_misspelt_limit_is_refused() {
    let off = GOOD.replace("host = \"100\"", "host = \"100\"\nai_name = \"  \"");
    assert!(Config::parse(&off).unwrap().ai_names.is_empty());
    refused(
        &GOOD.replace("host = \"100\"", "host = \"100\"\n\n[limits]\nquestions_per_day = 3"),
        "questions_per_day",
    );
}

#[test]
fn the_shipped_example_shows_the_defaults_and_leaves_the_ai_name_off() {
    let example = include_str!("../../rusty-fish.example.toml");
    assert!(example.contains("[limits]"));
    assert!(example.contains("ai_name"));
    let filled = example.replace("host = \"000000000000000000\"", "host = \"42\"");
    let c = Config::parse(&filled).unwrap();
    assert_eq!(c.limits, Limits::default(), "the example must show the real defaults");
    assert!(c.ai_names.is_empty());
}

// --- mehrere Rufnamen und die echte Erwähnung --------------------------------

fn with_names(value: &str) -> String {
    GOOD.replace("host = \"100\"", &format!("host = \"100\"\nai_name = {value}"))
}

#[test]
fn the_ai_may_have_several_call_names_and_the_first_is_her_name() {
    let c = Config::parse(&with_names("[\"Fish\", \"  Фиш \"]")).unwrap();
    assert_eq!(c.ai_names, ["Fish", "Фиш"]);
    assert_eq!(c.ai_name(), Some("Fish"));
    assert_eq!(c.rooms.len(), 1, "the room after the list must still be read");
}

#[test]
fn an_empty_list_means_off_but_an_empty_entry_is_refused() {
    assert!(Config::parse(&with_names("[]")).unwrap().ai_names.is_empty());
    refused(&with_names("[\"Fish\", \"  \"]"), "ai_name");
    refused(&with_names("[\"\"]"), "ai_name");
    // Weder Zahl noch Tabelle: nur Text oder Liste von Texten.
    refused(&with_names("7"), "ai_name");
}

#[test]
fn the_shipped_example_shows_the_list_form() {
    let example = include_str!("../../rusty-fish.example.toml");
    assert!(example.contains("# ai_name = [\"Fish\", \"Фиш\"]"), "the list form must be shown");
    // Einkommentiert muss sie auch gelesen werden können.
    let filled = example
        .replace("host = \"000000000000000000\"", "host = \"42\"")
        .replace("# ai_name = [\"Fish\", \"Фиш\"]", "ai_name = [\"Fish\", \"Фиш\"]");
    assert_eq!(Config::parse(&filled).unwrap().ai_names, ["Fish", "Фиш"]);
}

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

// --- das Datum ---------------------------------------------------------------

#[test]
fn the_utc_date_is_computed_without_a_calendar_library() {
    assert_eq!(utc_date(0), "1970-01-01");
    assert_eq!(utc_date(86_399), "1970-01-01");
    assert_eq!(utc_date(86_400), "1970-01-02");
    assert_eq!(utc_date(951_782_400), "2000-02-29");
    assert_eq!(utc_date(1_709_164_800), "2024-02-29");
    assert_eq!(utc_date(1_709_251_200), "2024-03-01");
    assert_eq!(utc_date(1_795_046_400), "2026-11-19");
    assert_eq!(utc_date(1_795_046_400 + 86_399), "2026-11-19");
    assert_eq!(utc_date(4_102_444_800), "2100-01-01");
}

// --- der Tageszähler in der Zustandsdatei --------------------------------------

#[test]
fn the_tally_survives_a_restart() {
    let path = temp_file("tally");
    let mut store = Store::open(&path).unwrap();
    assert_eq!(store.tally(), Tally::default());
    let mut tally = Tally { day: "2026-11-19".to_owned(), translation_tokens: 1234, ..Tally::default() };
    tally.questions.insert("7".to_owned(), 2);
    store.set_tally(&tally);
    store.save().unwrap();

    let again = Store::open(&path).unwrap();
    assert_eq!(again.tally(), tally);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn a_state_file_from_before_the_tally_still_opens() {
    let path = temp_file("old");
    std::fs::write(&path, r#"{ "consented": ["7"], "last_seen": { "111": 50 } }"#).unwrap();
    let store = Store::open(&path).unwrap();
    assert!(store.has_consented("7"));
    assert_eq!(store.seen("111"), Some(50));
    assert_eq!(store.tally(), Tally::default());
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

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

// --- das grammatische Geschlecht je Platz --------------------------------------

fn with_gender(lang: &str, value: &str) -> String {
    GOOD.replace(
        &format!("lang = \"{lang}\""),
        &format!("lang = \"{lang}\"\n  gender = \"{value}\""),
    )
}

#[test]
fn a_seats_gender_is_optional_and_read_when_given() {
    let c = Config::parse(GOOD).unwrap();
    assert!(c.rooms[0].seats.iter().all(|s| s.gender.is_none()));

    let c = Config::parse(&with_gender("Russian", "female")).unwrap();
    assert_eq!(c.seat_of("111").unwrap().gender, None);
    assert_eq!(c.seat_of("222").unwrap().gender, Some(Gender::Female));
    assert!(c.seat_of("333").is_none());

    // Groß und Klein und Leerraum sind kein Grund für eine Ablehnung.
    let c = Config::parse(&with_gender("German", " Male ")).unwrap();
    assert_eq!(c.seat_of("111").unwrap().gender, Some(Gender::Male));
}

#[test]
fn any_other_gender_value_is_refused_loudly() {
    for bad in ["f", "woman", "", "divers", "none", "femal"] {
        let err = Config::parse(&with_gender("Russian", bad))
            .expect_err("a misspelt gender must not silently become no gender");
        assert!(err.contains("gender"), "expected the field in: {err}");
        assert!(err.contains("222"), "expected the channel in: {err}");
    }
}

#[test]
fn the_shipped_example_shows_the_gender_setting_commented_out() {
    let example = include_str!("../../rusty-fish.example.toml");
    assert!(example.contains("# gender = \"female\""));
    let filled = example.replace("host = \"000000000000000000\"", "host = \"42\"");
    let c = Config::parse(&filled).unwrap();
    assert!(c.rooms[0].seats.iter().all(|s| s.gender.is_none()));

    // Wer das Zeichen davor entfernt, bekommt eine gültige Einstellung.
    let switched_on = filled.replace("# gender = \"female\"", "gender = \"female\"");
    let c = Config::parse(&switched_on).unwrap();
    assert_eq!(c.rooms[0].seats.iter().filter(|s| s.gender == Some(Gender::Female)).count(), 1);
}

// --- das grammatische Geschlecht je Person -------------------------------------

fn with_people(block: &str) -> String {
    format!("{GOOD}\n{block}")
}

#[test]
fn a_persons_gender_is_optional_and_read_when_given() {
    let c = Config::parse(GOOD).unwrap();
    assert!(c.people.is_empty());
    assert_eq!(c.gender_of("777"), None);

    let c = Config::parse(&with_people(
        "[[person]]\nid = \"777\"\ngender = \"female\"\n\n[[person]]\nid = \" 888 \"\ngender = \" Male \"\n",
    ))
    .unwrap();
    assert_eq!(c.gender_of("777"), Some(Gender::Female));
    // Groß und Klein und Leerraum sind kein Grund für eine Ablehnung.
    assert_eq!(c.gender_of("888"), Some(Gender::Male));
    assert_eq!(c.gender_of("999"), None);
}

#[test]
fn the_person_goes_before_the_seat_and_the_seat_before_nothing() {
    // Platz 111 (German) ist männlich, Platz 222 (Russian) sagt nichts.
    let text = with_gender("German", "male") + "\n[[person]]\nid = \"777\"\ngender = \"female\"\n";
    let c = Config::parse(&text).unwrap();
    assert_eq!(c.gender_for("777", "111"), Some(Gender::Female), "the person decides");
    assert_eq!(c.gender_for("777", "222"), Some(Gender::Female), "at every seat");
    assert_eq!(c.gender_for("555", "111"), Some(Gender::Male), "else the seat decides");
    assert_eq!(c.gender_for("555", "222"), None, "else nothing is said");
    assert_eq!(c.gender_for("555", "333"), None, "a channel outside every room");
}

#[test]
fn a_wrong_person_entry_is_refused_loudly() {
    for bad in ["f", "woman", "", "divers"] {
        let text = with_people(&format!("[[person]]\nid = \"777\"\ngender = \"{bad}\"\n"));
        let err = Config::parse(&text)
            .expect_err("a misspelt gender must not silently become no gender");
        assert!(err.contains("gender"), "expected the field in: {err}");
        assert!(err.contains("777"), "expected the person in: {err}");
    }
    // Ein Name ist keine Discord-Kennung.
    refused(&with_people("[[person]]\nid = \"quokka\"\ngender = \"female\"\n"), "quokka");
    // Zwei Angaben zur selben Person widersprächen sich oder wären ein Versehen.
    refused(
        &with_people(
            "[[person]]\nid = \"777\"\ngender = \"female\"\n\n[[person]]\nid = \"777\"\ngender = \"male\"\n",
        ),
        "777",
    );
    // Bis zum 20.11.2026 wurde hier auch ein Eintrag ohne `gender` abgelehnt,
    // weil er nichts aussagte. Seit ein Mensch auch Muttersprache und
    // Einladung trägt, ist die Form freiwillig (siehe tests/tables.rs).
    assert!(Config::parse(&with_people("[[person]]\nid = \"777\"\n")).is_ok());
    refused(&with_people("[[person]]\nid = \"777\"\ngender = \"female\"\nnick = \"x\"\n"), "nick");
}

#[test]
fn the_shipped_example_shows_a_person_commented_out() {
    let example = include_str!("../../rusty-fish.example.toml");
    assert!(example.contains("# [[person]]"));
    let filled = example.replace("host = \"000000000000000000\"", "host = \"42\"");
    let c = Config::parse(&filled).unwrap();
    assert!(c.people.is_empty());

    // Wer die Zeichen davor entfernt, bekommt eine gültige Einstellung.
    let switched_on = filled
        .replace("# [[person]]", "[[person]]")
        .replace("# id = \"000000000000000003\"", "id = \"000000000000000003\"")
        .replace("# gender = \"male\"", "gender = \"male\"");
    let c = Config::parse(&switched_on).unwrap();
    assert_eq!(c.gender_of("000000000000000003"), Some(Gender::Male));
    assert!(c.rooms[0].seats.iter().all(|s| s.gender.is_none()));
}

// --- der Hinweis an den Gastgeber ----------------------------------------------

#[test]
fn the_raise_and_the_notified_mark_survive_a_restart() {
    let path = temp_file("raise");
    let mut store = Store::open(&path).unwrap();
    let tally = Tally {
        day: "2026-11-19".to_owned(),
        translation_tokens: 300,
        raised_tokens: 200,
        host_notified: true,
        ..Tally::default()
    };
    store.set_tally(&tally);
    store.save().unwrap();

    let again = Store::open(&path).unwrap();
    assert_eq!(again.tally(), tally);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn numbers_are_grouped_for_reading() {
    assert_eq!(grouped(0), "0");
    assert_eq!(grouped(999), "999");
    assert_eq!(grouped(1000), "1,000");
    assert_eq!(grouped(100_000), "100,000");
    assert_eq!(grouped(1_234_567), "1,234,567");
}

#[test]
fn the_notice_to_the_host_names_both_amounts_and_asks() {
    let text = budget_notice(200_000, 100_000);
    assert!(text.contains("200,000 tokens"), "got: {text}");
    assert!(text.contains("another 100,000 tokens"), "got: {text}");
    assert!(text.contains("untranslated and marked"), "got: {text}");
    assert!(text.trim_end().ends_with('?'), "it must end with the question: {text}");

    assert_eq!(raise_label(100_000), "Raise by 100,000");
    // Discord nimmt für Knöpfe höchstens 80 Zeichen.
    assert!(raise_label(u64::MAX).chars().count() <= 80);
    assert!(KEEP_LABEL.chars().count() <= 80);
}

#[test]
fn the_answers_to_the_host_name_the_state_of_the_day() {
    let raised = raised_notice(200_000, 95_000);
    assert!(raised.contains("200,000"), "got: {raised}");
    assert!(raised.contains("95,000"), "got: {raised}");

    let nothing = nothing_to_raise(100_000, 40_000);
    assert!(nothing.contains("40,000"), "got: {nothing}");
    assert!(nothing.contains("100,000"), "got: {nothing}");
}

#[test]
fn the_service_is_named_the_way_a_person_recognises_it() {
    use rusty_fish_discord::render::service_name;
    assert_eq!(service_name("https://api.openai.com/v1"), "api.openai.com");
    assert_eq!(service_name(" https://api.openai.com/v1/ "), "api.openai.com");
    assert_eq!(service_name("http://localhost:11434/v1"), "localhost:11434");
    assert_eq!(service_name("example.org"), "example.org");
}

#[test]
fn the_consent_prompt_says_whose_service_reads_along() {
    use rusty_fish_discord::render::consent_text;
    let text = consent_text("api.openai.com");
    assert!(text.contains("api.openai.com"));
    assert!(text.contains("chosen by the host"));
    assert!(text.contains("before you agree"));
    assert!(text.contains("OK"));
}

// --- Der KI-Dienst im Profil, seit dem 20.11.2026 ---

#[test]
fn without_a_service_the_settings_mean_openai() {
    use rusty_fish_providers::Provider;
    let c = Config::parse(GOOD).unwrap();
    assert_eq!(c.provider, Provider::OpenAi);
    assert_eq!(c.service_address(), "https://api.openai.com/v1");
    // Ein leerer Wert zählt wie ein fehlender, so wie bei der Adresse.
    let empty = format!("provider = \"\"\n{GOOD}");
    assert_eq!(Config::parse(&empty).unwrap().provider, Provider::OpenAi);
}

#[test]
fn the_service_named_in_the_settings_is_the_one_used() {
    use rusty_fish_providers::Provider;
    let cases = [
        ("xai", Provider::Xai, "https://api.x.ai/v1"),
        ("google", Provider::Google, "https://generativelanguage.googleapis.com/v1beta/openai"),
        ("anthropic", Provider::Anthropic, "https://api.anthropic.com/v1"),
        (" OpenAI ", Provider::OpenAi, "https://api.openai.com/v1"),
    ];
    for (written, provider, address) in cases {
        let text = format!("provider = \"{written}\"\n{GOOD}");
        let c = Config::parse(&text).unwrap_or_else(|e| panic!("{written}: {e}"));
        assert_eq!(c.provider, provider);
        assert_eq!(c.service_address(), address);
    }
}

#[test]
fn an_own_address_goes_before_the_one_of_the_service() {
    let text = format!("provider = \"anthropic\"\nbase_url = \"https://proxy.example/v1\"\n{GOOD}");
    assert_eq!(Config::parse(&text).unwrap().service_address(), "https://proxy.example/v1");
}

#[test]
fn an_own_model_needs_its_address() {
    use rusty_fish_providers::Provider;
    refused(&format!("provider = \"custom\"\n{GOOD}"), "base_url");
    let text = format!("provider = \"custom\"\nbase_url = \"http://localhost:11434/v1\"\n{GOOD}");
    let c = Config::parse(&text).unwrap();
    assert_eq!(c.provider, Provider::Custom);
    assert_eq!(c.service_address(), "http://localhost:11434/v1");
}

#[test]
fn an_unknown_service_is_refused_by_name() {
    refused(&format!("provider = \"mistral\"\n{GOOD}"), "mistral");
    refused(&format!("provider = \"mistral\"\n{GOOD}"), "anthropic");
}

#[test]
fn the_shipped_example_shows_the_service() {
    use rusty_fish_providers::Provider;
    let example = include_str!("../../rusty-fish.example.toml");
    assert!(example.contains("provider = \"openai\""), "the example must show the setting");
    for id in ["xai", "google", "anthropic", "custom"] {
        assert!(example.contains(id), "the example must name {id}");
    }
    let filled = example.replace("host = \"000000000000000000\"", "host = \"42\"");
    assert_eq!(Config::parse(&filled).unwrap().provider, Provider::OpenAi);
}

#[test]
fn the_consent_prompt_names_the_service_really_used_for_every_provider() {
    // Die Bitte um Zustimmung soll sagen, wohin die Sätze gehen. Der Name kommt
    // deshalb aus der Adresse, die wirklich benutzt wird, nicht aus der Kennung:
    // wer bei „anthropic" eine eigene Adresse einträgt, schickt dorthin.
    use rusty_fish_discord::render::{consent_text, service_name};
    let cases = [
        ("", "api.openai.com"),
        ("provider = \"openai\"\n", "api.openai.com"),
        ("provider = \"xai\"\n", "api.x.ai"),
        ("provider = \"google\"\n", "generativelanguage.googleapis.com"),
        ("provider = \"anthropic\"\n", "api.anthropic.com"),
        ("provider = \"custom\"\nbase_url = \"http://localhost:11434/v1\"\n", "localhost:11434"),
        ("provider = \"anthropic\"\nbase_url = \"https://proxy.example/v1\"\n", "proxy.example"),
    ];
    for (head, host) in cases {
        let c = Config::parse(&format!("{head}{GOOD}")).unwrap_or_else(|e| panic!("{head}: {e}"));
        let service = service_name(&c.service_address());
        assert_eq!(service, host, "{head}");
        assert!(consent_text(&service).contains(host), "{head}");
    }
}
