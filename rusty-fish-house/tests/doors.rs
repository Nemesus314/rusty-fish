//! Tests zur zweiten Tür im Profil: ein Tisch an Matrix, ein Mensch mit einer
//! Matrix-Kennung, und der Ablauf einer Nachricht mit solchen Kennungen.
//! Keine Tür, kein Netz.
//!
//! Seit dem 22.11.2026. Das Haus war dafür gebaut, dass eine zweite Tür
//! dazukommt; diese Tests halten fest, was dafür im Profil gilt.

use rusty_fish_core::{Model, ModelError, Prompt, Reply};
use rusty_fish_house::config::{Config, DOORS, Door};
use rusty_fish_house::document::Document;
use rusty_fish_house::household::{Arrived, Household, Terms};
use rusty_fish_house::texts::{
    consent_text, consent_text_at, consent_text_in, invitation, invitation_at,
};
use rusty_fish_room::tongue::Tongue;
use rusty_fish_room::{Mark, Reason};
use std::future::Future;
use std::path::PathBuf;
use std::task::{Context, Poll, Waker};

const ALL: [Tongue; 3] = [Tongue::German, Tongue::English, Tongue::Russian];

/// Zwei Tische an Matrix: der Gastgeber liest Deutsch, sein Gast Russisch.
const MATRIX: &str = r#"
model = "some-model"
host = "nemo"
ai_name = "Fish"

[[room]]

  [[room.seat]]
  matrix = "!deutsch:example.org"
  lang = "German"
  owner = "nemo"

  [[room.seat]]
  matrix = "!russisch:example.org"
  lang = "Russian"
  owner = "robin"

[[person]]
name = "nemo"
matrix = "@nemo:example.org"

[[person]]
name = "robin"
matrix = "@robin:example.org"
gender = "female"
invited = true
"#;

fn refused(text: &str, expect: &str) {
    let err = Config::parse(text).unwrap_err();
    assert!(err.contains(expect), "expected {expect:?} in: {err}");
}

// --- das Profil ------------------------------------------------------------------

#[test]
fn the_fish_knows_two_doors() {
    assert_eq!(DOORS, ["discord", "matrix"]);
    assert_eq!(Door::Discord.id(), DOORS[0]);
    assert_eq!(Door::Matrix.id(), DOORS[1]);
}

#[test]
fn a_profile_can_stand_at_the_matrix_door() {
    let c = Config::parse(MATRIX).unwrap();
    assert_eq!(c.door, Door::Matrix);
    // Innen steht ein Mensch unter seiner Kennung an dieser Tür.
    assert_eq!(c.host, "@nemo:example.org");
    assert_eq!(c.host_written, "nemo");
    assert_eq!(c.owner_of("!russisch:example.org"), Some("@robin:example.org"));
    assert_eq!(c.name_of("@robin:example.org"), Some("robin"));
    assert_eq!(c.room_index("!deutsch:example.org"), Some(0));
    // Beide Tische werden bedient: der eigene und der der Eingeladenen.
    assert!(c.serves("!deutsch:example.org") && c.serves("!russisch:example.org"));
    // Wer gefragt werden darf, hängt an denselben Regeln wie bei Discord.
    assert!(c.may_ask("@nemo:example.org", "!deutsch:example.org"));
    assert!(c.may_ask("@robin:example.org", "!russisch:example.org"));
    assert!(!c.may_ask("@stranger:example.org", "!russisch:example.org"));
}

#[test]
fn a_profile_without_any_matrix_word_is_a_discord_profile_as_before() {
    let text = MATRIX
        .replace("matrix = \"!deutsch:example.org\"", "discord = \"111\"")
        .replace("matrix = \"!russisch:example.org\"", "channel = \"222\"")
        .replace("matrix = \"@nemo:example.org\"", "discord = \"100\"")
        .replace("matrix = \"@robin:example.org\"", "id = \"7\"");
    let c = Config::parse(&text).unwrap();
    assert_eq!(c.door, Door::Discord);
    assert_eq!(c.host, "100");
    assert_eq!(c.owner_of("222"), Some("7"));
}

#[test]
fn host_and_owner_may_be_a_bare_matrix_id() {
    let text = MATRIX
        .replace("host = \"nemo\"", "host = \"@nemo:example.org\"")
        .replace("owner = \"robin\"", "owner = \"@other:example.org\"");
    let c = Config::parse(&text).unwrap();
    assert_eq!(c.host, "@nemo:example.org");
    // Über diesen Besitzer ist nichts eingetragen: eingeladen ist er nicht.
    assert_eq!(c.owner_of("!russisch:example.org"), Some("@other:example.org"));
    assert!(!c.serves("!russisch:example.org"));
}

#[test]
fn a_person_may_carry_an_id_at_both_doors() {
    let text = MATRIX.replace(
        "matrix = \"@robin:example.org\"",
        "matrix = \"@robin:example.org\"\ndiscord = \"7\"",
    );
    let c = Config::parse(&text).unwrap();
    let robin = &c.people["@robin:example.org"];
    assert_eq!(robin.discord.as_deref(), Some("7"));
    assert_eq!(robin.id_at(Door::Matrix), Some("@robin:example.org"));
    assert_eq!(robin.id_at(Door::Discord), Some("7"));
    // Beide überstehen den Weg durch die Maske.
    let (toml, again) = Document::from(&c).check().unwrap();
    assert_eq!(again, c);
    assert!(toml.contains("matrix = \"@robin:example.org\"") && toml.contains("discord = \"7\""));
}

#[test]
fn the_host_needs_an_id_at_the_door_of_the_profile() {
    // Nur eine Discord-Kennung, aber die Tische stehen an Matrix.
    let text = MATRIX.replace("matrix = \"@nemo:example.org\"", "discord = \"100\"");
    refused(&text, "the host \"nemo\" needs a Matrix id");
    // Eine bloße Discord-Kennung als Gastgeber zeigt hier auf niemanden.
    refused(&MATRIX.replace("host = \"nemo\"", "host = \"100\""), "or a Matrix id (like @name:server)");
}

#[test]
fn a_table_stands_at_one_door() {
    // Ein Tisch an zwei Türen zugleich ist ein Versehen.
    let both = MATRIX.replace(
        "matrix = \"!russisch:example.org\"",
        "matrix = \"!russisch:example.org\"\n  discord = \"222\"",
    );
    refused(&both, "has both `discord` and `matrix`");
}

#[test]
fn matrix_ids_are_checked_for_their_shape() {
    for bad in ["robin", "@robin", "@:example.org", "@robin:", "@ro bin:example.org"] {
        let text = MATRIX.replace("@robin:example.org", bad);
        refused(&text, "must be a Matrix user id like @name:server");
    }
    // Ein Alias ist keine Raumkennung, und ein Name auch nicht.
    for bad in ["#russisch:example.org", "russisch", "!"] {
        let text = MATRIX.replace("!russisch:example.org", bad);
        refused(&text, "starts with ! (not its #alias)");
    }
    // Neuere Räume tragen keinen Servernamen mehr hinter der Kennung.
    let short = MATRIX.replace("!russisch:example.org", "!ZmFrZS1yb29tLWlk");
    assert!(Config::parse(&short).is_ok());
}

#[test]
fn a_name_must_not_look_like_a_matrix_id() {
    refused(&MATRIX.replace("name = \"robin\"", "name = \"@robin\""), "would be taken for a Matrix id");
}

#[test]
fn the_door_of_a_table_survives_the_trip_through_the_form() {
    let c = Config::parse(MATRIX).unwrap();
    let doc = Document::from(&c);
    assert!(doc.rooms[0].seats.iter().all(|seat| seat.door == "matrix"));
    assert_eq!(doc.rooms[0].seats[1].channel, "!russisch:example.org");
    assert_eq!(doc.people[1].matrix.as_deref(), Some("@robin:example.org"));
    assert_eq!(doc.people[1].discord, None);
    let (toml, again) = doc.check().unwrap();
    assert_eq!(again, c);
    assert!(toml.contains("matrix = \"!russisch:example.org\""), "got {toml}");
}

#[test]
fn a_form_can_name_the_matrix_door() {
    let json = r#"{
        "model": "m", "host": "@nemo:example.org",
        "rooms": [{"seats": [
            {"door": "matrix", "channel": "!a:example.org", "lang": "German"},
            {"door": "matrix", "channel": "!b:example.org", "lang": "Russian"}
        ]}],
        "people": [{"name": "robin", "matrix": "@robin:example.org"}]
    }"#;
    let (_, config) = Document::from_json(json).unwrap().check().unwrap();
    assert_eq!(config.door, Door::Matrix);
    assert_eq!(config.people["@robin:example.org"].name.as_deref(), Some("robin"));
}

// --- der Ablauf einer Nachricht --------------------------------------------------

struct Fake;

impl Model for Fake {
    async fn complete(&self, _prompt: &Prompt) -> Result<Reply, ModelError> {
        Ok(Reply { text: "TRANSLATED".to_owned(), tokens: Some(10) })
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

fn state_file(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rusty-fish-doors-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("state.json")
}

#[test]
fn a_message_from_a_matrix_room_takes_the_same_course() {
    let config = Config::parse(MATRIX).unwrap();
    let path = state_file("course");
    let mut house = Household::open(&config, &path).unwrap();
    let terms = Terms { config: &config, service: "ai.example", today: "2026-11-22" };

    // Der Gastgeber hat zugestimmt, weil er den Fisch startet.
    let from_host = Arrived {
        channel: "!deutsch:example.org",
        author: "@nemo:example.org",
        text: "Guten Morgen",
        mark: 1,
    };
    let steps = now(house.message(&Fake, &terms, &from_host)).unwrap();
    assert_eq!(steps.deliveries.len(), 1);
    assert_eq!(steps.deliveries[0].channel, "!russisch:example.org");
    assert_eq!(steps.deliveries[0].mark, Mark::Translated);
    assert_eq!(steps.ask_consent, None);

    // Der Gast wird gefragt, mit dem Wortlaut dieser Tür.
    let from_guest = Arrived {
        channel: "!russisch:example.org",
        author: "@robin:example.org",
        text: "Привет",
        mark: 1,
    };
    let steps = now(house.message(&Fake, &terms, &from_guest)).unwrap();
    assert_eq!(steps.deliveries[0].mark, Mark::Untranslated(Reason::NoConsent));
    assert_eq!(
        steps.ask_consent,
        Some(consent_text_at(Door::Matrix, Tongue::Russian, "ai.example"))
    );

    // Die Zustimmung hängt an der Matrix-Kennung und übersteht den Neustart.
    house.consent("@robin:example.org", "ai.example").unwrap();
    let held = now(house.held_back(&Fake, &terms, "@robin:example.org"));
    assert_eq!(held.len(), 1, "the sentence written before consent is handed in");
    house.save().unwrap();
    let mut again = Household::open(&config, &path).unwrap();
    assert_eq!(again.seen("!russisch:example.org"), Some(1));
    let next = Arrived { mark: 2, ..from_guest };
    let steps = now(again.message(&Fake, &terms, &next)).unwrap();
    assert_eq!(steps.deliveries[0].mark, Mark::Translated);
    assert_eq!(steps.ask_consent, None);
}

// --- der Wortlaut je Tür ---------------------------------------------------------

#[test]
fn at_discord_the_fixed_texts_read_as_before() {
    for tongue in ALL {
        assert_eq!(consent_text_at(Door::Discord, tongue, "ai.example"), consent_text_in(tongue, "ai.example"));
        assert_eq!(invitation_at(Door::Discord, tongue, Some("Zebra")), invitation(tongue, Some("Zebra")));
        assert_eq!(invitation_at(Door::Discord, tongue, None), invitation(tongue, None));
    }
    // Wort für Wort, wie sie bis zum 22.11.2026 im Programm standen.
    assert!(consent_text("ai.example").contains(
        "In this channel everyone writes in their own language, and an AI translates in between. \
         The AI is an outside service chosen by the host of this server (ai.example)."
    ));
    assert!(consent_text("ai.example").contains("Press OK to agree."));
    assert!(consent_text_in(Tongue::German, "ai.example").contains(
        "Dienst, den der Gastgeber dieses Servers gewählt hat (ai.example). Zum Übersetzen"
    ));
    assert!(consent_text_in(Tongue::Russian, "ai.example").contains("Нажми OK, чтобы согласиться."));
    assert!(invitation(Tongue::German, None).contains("in der Sprache dieses Kanals."));
}

#[test]
fn at_matrix_the_request_speaks_of_a_room_and_not_of_the_hosts_server() {
    for tongue in ALL {
        let text = consent_text_at(Door::Matrix, tongue, "ai.example");
        assert!(text.contains("ai.example"), "the service is named: {text}");
        assert!(text.contains("OK"), "the sign to press is named: {text}");
        assert_ne!(text, consent_text_in(tongue, "ai.example"));
    }
    let en = consent_text_at(Door::Matrix, Tongue::English, "ai.example");
    assert!(en.contains("In this room") && !en.contains("channel"), "got {en}");
    // Der Server eines Matrix-Raums gehört meist nicht dem Gastgeber.
    assert!(!en.contains("server"), "got {en}");
    assert!(en.contains("Press OK below this message to agree."), "got {en}");
    let de = consent_text_at(Door::Matrix, Tongue::German, "ai.example");
    assert!(de.contains("In diesem Raum") && !de.contains("Server") && !de.contains("Kanal"), "got {de}");
    let ru = consent_text_at(Door::Matrix, Tongue::Russian, "ai.example");
    assert!(ru.contains("В этой комнате") && !ru.contains("сервера") && !ru.contains("канал"), "got {ru}");

    assert!(invitation_at(Door::Matrix, Tongue::English, Some("Zebra")).contains("of this room"));
    assert!(invitation_at(Door::Matrix, Tongue::German, None).contains("dieses Raums"));
    assert!(invitation_at(Door::Matrix, Tongue::Russian, None).contains("этой комнаты"));
}
