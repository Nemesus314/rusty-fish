//! Tests des bearbeitbaren Dokuments. Kein Netz, keine Dateien.

use rusty_fish_discord::config::Config;
use rusty_fish_discord::document::{DocLimits, DocPerson, DocRoom, DocSeat, Document};

fn seat(channel: &str, lang: &str, gender: Option<&str>) -> DocSeat {
    DocSeat { gender: gender.map(str::to_owned), ..DocSeat::new(channel, lang) }
}

/// Ein Mensch, über den nur die Form eingetragen ist, wie vor dem 20.11.2026.
fn person(id: &str, gender: &str) -> DocPerson {
    DocPerson { gender: Some(gender.to_owned()), ..DocPerson::new(id) }
}

fn full() -> Document {
    Document {
        name: None,
        provider: "custom".to_owned(),
        model: "some-model".to_owned(),
        base_url: Some("http://localhost:11434/v1".to_owned()),
        host: "100".to_owned(),
        ai_names: vec!["Fish".to_owned(), "Фиш".to_owned()],
        limits: DocLimits { translation_tokens_per_day: 2500, questions_per_guest_per_day: 3 },
        rooms: vec![
            DocRoom {
                pass_through: vec!["English".to_owned()],
                context_lines: 4,
                seats: vec![seat("111", "German", Some("male")), seat("222", "Russian", Some("female"))],
            },
            DocRoom {
                pass_through: vec![],
                context_lines: 0,
                seats: vec![seat("333", "Korean", None), seat("444", "German", None)],
            },
        ],
        people: vec![person("100", "male"), person("777", "female")],
    }
}

fn minimal() -> Document {
    Document {
        name: None,
        provider: "openai".to_owned(),
        model: "m".to_owned(),
        base_url: None,
        host: "1".to_owned(),
        ai_names: vec![],
        limits: DocLimits::default(),
        rooms: vec![DocRoom {
            pass_through: vec![],
            context_lines: 10,
            seats: vec![seat("111", "German", None), seat("222", "Russian", None)],
        }],
        people: vec![],
    }
}

#[test]
fn a_document_survives_the_trip_through_the_file_format() {
    for doc in [full(), minimal()] {
        let (toml, config) = doc.check().expect("a sound document must be accepted");
        // Was gespeichert würde, liest der Fisch genau so wieder.
        assert_eq!(Config::parse(&toml).unwrap(), config);
        assert_eq!(Document::from(&config), doc, "the trip changed the document:\n{toml}");
    }
}

#[test]
fn awkward_characters_survive_too() {
    let mut doc = minimal();
    doc.model = "a \"quoted\" model \\ with a backslash".to_owned();
    doc.ai_names = vec!["Рыбка".to_owned(), "O'Fish".to_owned(), "Fish # not a comment".to_owned()];
    doc.rooms[0].seats[0].lang = "Deutsch (Österreich)".to_owned();
    doc.rooms[0].pass_through = vec!["English, British".to_owned()];
    let (_, config) = doc.check().unwrap();
    assert_eq!(Document::from(&config), doc);
}

#[test]
fn what_is_trimmed_on_reading_is_trimmed_in_the_document() {
    let mut doc = minimal();
    doc.model = "  m  ".to_owned();
    doc.base_url = Some("   ".to_owned());
    doc.rooms[0].seats[0].gender = Some(" Female ".to_owned());
    let (_, config) = doc.check().unwrap();
    let back = Document::from(&config);
    assert_eq!(back.model, "m");
    assert_eq!(back.base_url, None);
    assert_eq!(back.rooms[0].seats[0].gender.as_deref(), Some("female"));
}

fn refused(doc: &Document, expect: &str) {
    let err = doc.check().expect_err("this document must be refused");
    assert!(err.contains(expect), "expected {expect:?} in: {err}");
}

#[test]
fn a_document_is_refused_by_the_same_check_as_a_file() {
    let mut d = minimal();
    d.model = " ".to_owned();
    refused(&d, "model");

    let mut d = minimal();
    d.host = "me".to_owned();
    refused(&d, "host");

    let mut d = minimal();
    d.rooms[0].seats.pop();
    refused(&d, "two seats");

    let mut d = minimal();
    d.rooms.clear();
    refused(&d, "room");

    let mut d = minimal();
    d.rooms[0].seats[1].channel = "111".to_owned();
    refused(&d, "used twice");

    let mut d = minimal();
    d.rooms[0].seats[1].channel = "#general".to_owned();
    refused(&d, "#general");

    let mut d = minimal();
    d.rooms[0].seats[0].gender = Some("divers".to_owned());
    refused(&d, "channel 111");

    let mut d = minimal();
    d.ai_names = vec!["Fish".to_owned(), " ".to_owned()];
    refused(&d, "ai_name");

    let mut d = minimal();
    d.people = vec![person("9", "male"), person("9", "male")];
    refused(&d, "listed twice");
}

#[test]
fn reads_what_a_form_would_send() {
    let json = r#"{
        "name": "tea",
        "model": "some-model",
        "base_url": null,
        "host": "100",
        "ai_names": ["Fish"],
        "limits": {"translation_tokens_per_day": 5000, "questions_per_guest_per_day": 2},
        "rooms": [{"pass_through": ["English"], "context_lines": 6, "seats": [
            {"channel": "111", "lang": "German", "gender": null},
            {"channel": "222", "lang": "Russian", "gender": "female"}
        ]}],
        "people": [{"id": "100", "gender": "male"}]
    }"#;
    let doc = Document::from_json(json).unwrap();
    assert_eq!(doc.name.as_deref(), Some("tea"));
    assert_eq!(doc.limits.translation_tokens_per_day, 5000);
    assert_eq!(doc.rooms[0].seats[1].gender.as_deref(), Some("female"));
    doc.check().unwrap();
}

#[test]
fn what_a_form_may_leave_out_gets_the_usual_defaults() {
    let json = r#"{"model": "m", "host": "1", "rooms": [{"seats": [
        {"channel": "111", "lang": "German"}, {"channel": "222", "lang": "Russian"}]}]}"#;
    let doc = Document::from_json(json).unwrap();
    assert_eq!(doc, minimal());
}

#[test]
fn a_typo_in_a_field_name_is_said_out_loud() {
    let json = r#"{"model": "m", "hots": "1", "rooms": []}"#;
    let err = Document::from_json(json).unwrap_err();
    assert!(err.contains("hots"), "got {err}");
    let err = Document::from_json("{ not json").unwrap_err();
    assert!(err.contains("JSON"), "got {err}");
}

#[test]
fn the_document_goes_out_as_json_in_the_agreed_shape() {
    let (_, config) = full().check().unwrap();
    let mut doc = Document::from(&config);
    doc.name = Some("tea".to_owned());
    let v: serde_json::Value = serde_json::to_value(&doc).unwrap();
    assert_eq!(v["name"], "tea");
    assert_eq!(v["ai_names"][1], "Фиш");
    assert_eq!(v["limits"]["translation_tokens_per_day"], 2500);
    assert_eq!(v["rooms"][0]["seats"][1]["gender"], "female");
    assert!(v["rooms"][1]["seats"][0]["gender"].is_null());
    assert_eq!(v["people"][1]["id"], "777");
    assert_eq!(v["base_url"], "http://localhost:11434/v1");
    assert_eq!(v["provider"], "custom");
}

// --- Der KI-Dienst im Profil, seit dem 20.11.2026 ---

#[test]
fn every_service_survives_the_trip_through_the_file_format() {
    for id in ["openai", "xai", "google", "anthropic"] {
        let mut doc = minimal();
        doc.provider = id.to_owned();
        let (toml, config) = doc.check().unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(config.provider.id(), id);
        assert_eq!(Document::from(&config), doc, "the trip changed the document:\n{toml}");
    }
}

#[test]
fn a_document_without_a_service_means_openai() {
    let json = r#"{"model": "m", "host": "1", "rooms": [{"seats": [
        {"channel": "111", "lang": "German"}, {"channel": "222", "lang": "Russian"}]}]}"#;
    let doc = Document::from_json(json).unwrap();
    assert_eq!(doc.provider, "openai");
    let (_, config) = doc.check().unwrap();
    assert_eq!(config.provider.id(), "openai");
}

#[test]
fn the_service_is_always_part_of_what_goes_out() {
    let (_, config) = minimal().check().unwrap();
    let v: serde_json::Value = serde_json::to_value(Document::from(&config)).unwrap();
    assert_eq!(v["provider"], "openai");
}

#[test]
fn an_unknown_service_and_an_own_model_without_address_are_refused() {
    let mut d = minimal();
    d.provider = "mistral".to_owned();
    refused(&d, "mistral");

    let mut d = minimal();
    d.provider = "custom".to_owned();
    refused(&d, "base_url");
}

/// Das mitgelieferte Beispiel muss sich als Dokument zeigen und wieder
/// speichern lassen, sobald es ausgefüllt ist.
#[test]
fn the_shipped_example_can_be_shown_and_saved_again() {
    let example = include_str!("../../rusty-fish.example.toml")
        .replace("host = \"000000000000000000\"", "host = \"42\"");
    let config = Config::parse(&example).unwrap();
    let doc = Document::from(&config);
    let (_, again) = doc.check().unwrap();
    assert_eq!(again, config);
}
