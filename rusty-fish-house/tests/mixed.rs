//! Der gemischte Raum im Profil und im Haushalt: ein Gespräch, dessen Tische
//! an verschiedenen Türen stehen. Keine Tür, kein Netz.
//!
//! Seit dem 23.11.2026. Bis dahin lehnte das Profil so einen Raum ab.

use rusty_fish_core::{Model, ModelError, Prompt, Reply};
use rusty_fish_house::between::{Errand, Lines, Writer, by_door};
use rusty_fish_house::config::{Config, Door};
use rusty_fish_house::document::Document;
use rusty_fish_house::household::{Arrived, Household, Terms};
use rusty_fish_house::texts::consent_text_at;
use rusty_fish_room::tongue::Tongue;
use rusty_fish_room::{Delivery, Mark, Reason, Sender};
use std::future::Future;
use std::path::PathBuf;
use std::task::{Context, Poll, Waker};

/// Der Gastgeber liest Deutsch in einem Discord-Kanal, sein Gast Englisch in
/// einem Matrix-Raum. Beide tragen an beiden Türen eine Kennung.
const MIXED: &str = r#"
model = "some-model"
host = "nemo"
ai_name = "Fish"

[[room]]

  [[room.seat]]
  discord = "111"
  lang = "German"
  owner = "nemo"

  [[room.seat]]
  matrix = "!english:example.org"
  lang = "English"
  owner = "robin"

[[person]]
name = "nemo"
discord = "100"
matrix = "@nemo:example.org"

[[person]]
name = "robin"
discord = "7"
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
fn the_tables_of_one_room_may_stand_at_different_doors() {
    let c = Config::parse(MIXED).unwrap();
    assert_eq!(c.doors, [Door::Discord, Door::Matrix], "the door of the first table comes first");
    assert_eq!(c.door, Door::Discord);
    assert!(c.is_mixed());
    assert_eq!(c.door_of("111"), Some(Door::Discord));
    assert_eq!(c.door_of("!english:example.org"), Some(Door::Matrix));
    assert_eq!(c.door_of("999"), None);
    // Ein Raum, zwei Tische, beide bedient.
    assert_eq!(c.room_index("111"), c.room_index("!english:example.org"));
    assert!(c.serves("111") && c.serves("!english:example.org"));
}

#[test]
fn a_profile_at_one_door_reads_as_before() {
    let plain = MIXED.replace("matrix = \"!english:example.org\"", "discord = \"222\"");
    let c = Config::parse(&plain).unwrap();
    assert_eq!(c.doors, [Door::Discord]);
    assert!(!c.is_mixed());
    // Die Matrix-Kennung steht nur dabei: unter ihr wird niemand gefunden.
    assert_eq!(c.key_of("@robin:example.org"), "@robin:example.org");
    assert!(!c.is_host("@nemo:example.org"));
    assert_eq!(c.host_at(Door::Matrix), Some("@nemo:example.org"), "it is still written down");
    assert_eq!(c.host_door(), Door::Discord);
}

#[test]
fn a_person_is_found_under_the_id_at_either_door() {
    let c = Config::parse(MIXED).unwrap();
    // Innen steht ein Mensch unter der Kennung an der ersten Tür.
    assert_eq!(c.host, "100");
    assert_eq!(c.key_of("@robin:example.org"), "7");
    assert_eq!(c.key_of("7"), "7");
    assert_eq!(c.owner_of("!english:example.org"), Some("7"));
    // Was über ihn eingetragen ist, gilt an beiden Türen.
    assert_eq!(c.gender_of("@robin:example.org"), c.gender_of("7"));
    assert!(c.gender_of("7").is_some());
    assert!(c.may_ask("@robin:example.org", "!english:example.org"));
    assert!(c.may_ask("7", "111"));
    assert!(!c.may_ask("@stranger:example.org", "!english:example.org"));
    // Wer nicht eingetragen ist, steht unter seiner Kennung selbst.
    assert_eq!(c.key_of("@stranger:example.org"), "@stranger:example.org");
}

#[test]
fn the_host_is_the_host_at_every_door_where_he_carries_an_id() {
    let c = Config::parse(MIXED).unwrap();
    assert!(c.is_host("100") && c.is_host("@nemo:example.org"));
    assert!(!c.is_host("7") && !c.is_host("@robin:example.org"));
    assert_eq!(c.host_at(Door::Discord), Some("100"));
    assert_eq!(c.host_at(Door::Matrix), Some("@nemo:example.org"));

    // Eine Kennung an einer der beiden Türen genügt.
    let only_discord = MIXED.replace("matrix = \"@nemo:example.org\"\n", "");
    let c = Config::parse(&only_discord).unwrap();
    assert_eq!(c.host_at(Door::Matrix), None);
    assert!(!c.is_host("@nemo:example.org"));
    let only_matrix = MIXED.replace("discord = \"100\"\n", "");
    let c = Config::parse(&only_matrix).unwrap();
    assert_eq!(c.host, "@nemo:example.org", "the key is the id he has");
    assert_eq!(c.owner_of("111"), Some("@nemo:example.org"));
    assert!(c.serves("111"));

    // `host` darf auch eine bloße Kennung an einer der beiden Türen sein.
    let bare = MIXED.replace("host = \"nemo\"", "host = \"@nemo:example.org\"");
    assert_eq!(Config::parse(&bare).unwrap().host, "100", "it points at the same person");

    // Ohne jede Kennung sagt die Meldung, was fehlt, für beide Türen.
    let none = only_discord.replace("discord = \"100\"\n", "");
    refused(&none, "the host \"nemo\" needs a Discord or Matrix id");
    refused(&none, "add `discord = \"...\"` or `matrix = \"...\"`");
    refused(
        &MIXED.replace("host = \"nemo\"", "host = \"nobody\""),
        "a Discord id (digits only, not zero) or Matrix id (like @name:server)",
    );
}

#[test]
fn the_hosts_notice_goes_to_the_door_of_his_own_table() {
    // Entschieden von Nemesus am 23.11.2026.
    let c = Config::parse(MIXED).unwrap();
    assert_eq!(c.host_door(), Door::Discord, "his table is the Discord channel");

    // Gehört ihm der Matrix-Raum, liest er dort.
    let swapped = MIXED
        .replace("owner = \"nemo\"", "owner = \"x\"")
        .replace("owner = \"robin\"", "owner = \"nemo\"")
        .replace("owner = \"x\"", "owner = \"robin\"");
    assert_eq!(Config::parse(&swapped).unwrap().host_door(), Door::Matrix);

    // Hat er an der Tür seines Tisches keine Kennung, dort, wo er eine hat.
    let no_id_there = swapped.replace("matrix = \"@nemo:example.org\"\n", "");
    assert_eq!(Config::parse(&no_id_there).unwrap().host_door(), Door::Discord);

    // Gehört ihm kein Tisch, an der ersten Tür, an der er eine Kennung hat.
    let no_table = MIXED.replace("  owner = \"nemo\"\n", "");
    assert_eq!(Config::parse(&no_table).unwrap().host_door(), Door::Discord);
}

#[test]
fn an_id_belongs_to_one_person_at_whichever_door() {
    // Robin steht unter ihrer Discord-Kennung; ein zweiter Eintrag nennt ihre
    // Matrix-Kennung noch einmal.
    let twice = format!("{MIXED}\n[[person]]\nname = \"zoe\"\nmatrix = \"@robin:example.org\"\n");
    refused(&twice, "person @robin:example.org is listed twice");
}

#[test]
fn a_mixed_room_survives_the_trip_through_the_form() {
    let c = Config::parse(MIXED).unwrap();
    let doc = Document::from(&c);
    let doors: Vec<&str> = doc.rooms[0].seats.iter().map(|seat| seat.door.as_str()).collect();
    assert_eq!(doors, ["discord", "matrix"], "every table names its own door");
    let (toml, again) = doc.check().unwrap();
    assert_eq!(again, c);
    assert!(toml.contains("discord = \"111\"") && toml.contains("matrix = \"!english:example.org\""));
    // Und ein zweites Mal unverändert.
    let (second, _) = Document::from(&again).check().unwrap();
    assert_eq!(second, toml);

    // Eine Maske kann so einen Raum anlegen.
    let json = r#"{
        "model": "m", "host": "nemo",
        "rooms": [{"seats": [
            {"door": "discord", "channel": "111", "lang": "German", "owner": "nemo"},
            {"door": "matrix", "channel": "!b:example.org", "lang": "English"}
        ]}],
        "people": [{"name": "nemo", "discord": "100"}]
    }"#;
    let (_, config) = Document::from_json(json).unwrap().check().unwrap();
    assert_eq!(config.doors, [Door::Discord, Door::Matrix]);
}

// --- zwischen den Türen ----------------------------------------------------------

fn delivery(channel: &str) -> Delivery {
    Delivery {
        channel: channel.to_owned(),
        text: "x".to_owned(),
        mark: Mark::Translated,
        from: Sender::Writer,
    }
}

#[test]
fn deliveries_are_split_by_the_door_of_their_table() {
    let both = MIXED.replace(
        "[[person]]\nname = \"nemo\"",
        "[[room]]\n\n  [[room.seat]]\n  discord = \"211\"\n  lang = \"German\"\n\n  \
         [[room.seat]]\n  discord = \"212\"\n  lang = \"English\"\n\n[[person]]\nname = \"nemo\"",
    );
    let c = Config::parse(&both).unwrap();
    let all = [delivery("!english:example.org"), delivery("111"), delivery("212")];
    let (mine, others) = by_door(&c, Door::Discord, &all);
    assert_eq!(mine, [delivery("111"), delivery("212")]);
    assert_eq!(others, [(Door::Matrix, vec![delivery("!english:example.org")])]);
    let (mine, others) = by_door(&c, Door::Matrix, &all);
    assert_eq!(mine, [delivery("!english:example.org")]);
    assert_eq!(others, [(Door::Discord, vec![delivery("111"), delivery("212")])]);
}

fn now<F: Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = Context::from_waker(Waker::noop());
    match f.as_mut().poll(&mut cx) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("this must be ready at once"),
    }
}

#[test]
fn a_door_that_is_not_open_says_so_instead_of_waiting() {
    let lines = Lines::new();
    let writer = Writer {
        door: Door::Discord,
        name: "Nemo".to_owned(),
        face: None,
        attachments: String::new(),
    };
    let errand = Errand::Deliver { deliveries: vec![delivery("!a:example.org")], writer };
    assert_eq!(
        now(lines.hand(Door::Matrix, Some("tea"), errand.clone())),
        Err("the Matrix door is not open".to_owned())
    );
    // Eine Tür, die schließt, bevor sie antwortet, lässt niemanden warten.
    let handed = lines.open(Door::Matrix);
    drop(handed);
    assert_eq!(
        now(lines.hand(Door::Matrix, Some("tea"), errand)),
        Err("the Matrix door is not open".to_owned())
    );
    // Eine Ablehnung, der niemand zuhört, geht verloren und stört nicht.
    lines.refuse(Door::Matrix, "tea", "why");
    let mut refusals = lines.listen_for_refusals();
    lines.refuse(Door::Discord, "tea", "no room");
    let refusal = refusals.try_recv().unwrap();
    assert_eq!((refusal.door, refusal.profile.as_str(), refusal.why.as_str()), (Door::Discord, "tea", "no room"));
}

// --- der Ablauf einer Nachricht --------------------------------------------------

struct Fake;

impl Model for Fake {
    async fn complete(&self, _prompt: &Prompt) -> Result<Reply, ModelError> {
        Ok(Reply { text: "TRANSLATED".to_owned(), tokens: Some(10) })
    }
}

fn state_file(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rusty-fish-mixed-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("state.json")
}

#[test]
fn a_message_crosses_from_one_door_to_the_other() {
    let config = Config::parse(MIXED).unwrap();
    let path = state_file("course");
    let mut house = Household::open(&config, &path).unwrap();
    let terms = Terms { config: &config, service: "ai.example", today: "2026-11-23" };

    // Der Gastgeber schreibt bei Discord: es erscheint im Matrix-Raum.
    let discord_mark = 1_300_000_000_000_000_001;
    let from_host =
        Arrived { channel: "111", author: "100", text: "Guten Morgen", mark: discord_mark };
    let steps = now(house.message(&Fake, &terms, &from_host)).unwrap();
    assert_eq!(steps.deliveries.len(), 1);
    assert_eq!(steps.deliveries[0].channel, "!english:example.org");
    assert_eq!(steps.deliveries[0].mark, Mark::Translated);
    assert_eq!(steps.ask_consent, None);

    // Schreibt er im Matrix-Raum, ist er auch dort der Gastgeber: keine Bitte.
    let matrix_mark = 1_800_000_000_000;
    let host_at_matrix = Arrived {
        channel: "!english:example.org",
        author: "@nemo:example.org",
        text: "Hello",
        mark: matrix_mark,
    };
    let steps = now(house.message(&Fake, &terms, &host_at_matrix)).unwrap();
    assert_eq!(steps.deliveries[0].channel, "111");
    assert_eq!(steps.deliveries[0].mark, Mark::Translated);
    assert_eq!(steps.ask_consent, None);

    // Die Lesemarken stehen nebeneinander, jede so gezählt wie an ihrer Tür.
    house.save().unwrap();
    let again = Household::open(&config, &path).unwrap();
    assert_eq!(again.seen("111"), Some(discord_mark));
    assert_eq!(again.seen("!english:example.org"), Some(matrix_mark));
}

#[test]
fn consent_hangs_on_the_id_and_the_request_speaks_the_words_of_its_door() {
    let config = Config::parse(MIXED).unwrap();
    let path = state_file("consent");
    let mut house = Household::open(&config, &path).unwrap();
    let terms = Terms { config: &config, service: "ai.example", today: "2026-11-23" };

    // Der Gast schreibt im Matrix-Raum: die Bitte steht dort, mit den Worten
    // dieser Tür, auch wenn die erste Tür des Profils Discord ist.
    let at_matrix = Arrived {
        channel: "!english:example.org",
        author: "@robin:example.org",
        text: "Hello",
        mark: 1,
    };
    let steps = now(house.message(&Fake, &terms, &at_matrix)).unwrap();
    assert_eq!(steps.deliveries[0].mark, Mark::Untranslated(Reason::NoConsent));
    assert_eq!(
        steps.ask_consent,
        Some(consent_text_at(Door::Matrix, Tongue::English, "ai.example"))
    );
    house.consent("@robin:example.org", "ai.example").unwrap();
    let held = now(house.held_back(&Fake, &terms, "@robin:example.org"));
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].channel, "111", "handed in at the table of the other door");

    // Dieselbe Frau schreibt bei Discord, unter ihrer Kennung dort. Dass beide
    // Kennungen ein Mensch sind, behauptet nur der Gastgeber: sie wird dort
    // für sich gefragt, mit den Worten jener Tür.
    let at_discord = Arrived { channel: "111", author: "7", text: "Hallo", mark: 5 };
    let steps = now(house.message(&Fake, &terms, &at_discord)).unwrap();
    assert_eq!(steps.deliveries[0].mark, Mark::Untranslated(Reason::NoConsent));
    assert_eq!(
        steps.ask_consent,
        Some(consent_text_at(Door::Discord, Tongue::German, "ai.example"))
    );
    // Im Matrix-Raum gilt ihre Zustimmung weiter.
    let next = Arrived { mark: 2, ..at_matrix };
    let steps = now(house.message(&Fake, &terms, &next)).unwrap();
    assert_eq!(steps.deliveries[0].mark, Mark::Translated);
    assert_eq!(steps.ask_consent, None);
}

#[test]
fn an_invitation_is_due_at_the_table_of_the_invited_whatever_its_door() {
    let config = Config::parse(MIXED).unwrap();
    let path = state_file("invited");
    let mut house = Household::open(&config, &path).unwrap();
    let pending = house.invitations(&config).unwrap();
    assert_eq!(pending.due.len(), 1);
    assert_eq!(pending.due[0].channel, "!english:example.org");
    // Die Discord-Tür erreicht dort niemanden und merkt sich niemanden; die
    // Mitteilung bleibt fällig, bis die Matrix-Tür sie abgesetzt hat.
    house.invitations_told(pending, &[]).unwrap();
    let pending = house.invitations(&config).unwrap();
    assert_eq!(pending.due.len(), 1);
    let reached = [pending.due[0].person.clone()];
    house.invitations_told(pending, &reached).unwrap();
    assert!(house.invitations(&config).is_none());
}

// --- die Beispieldatei -----------------------------------------------------------

#[test]
fn the_example_file_shows_how_a_room_becomes_a_mixed_room() {
    // Ausgefüllt, wie es jemand tun würde: die eigene Kennung statt der Nullen.
    let example = include_str!("../../rusty-fish.example.toml")
        .replace("host = \"000000000000000000\"", "host = \"42\"");
    assert_eq!(Config::parse(&example).unwrap().doors, [Door::Discord], "as it stands");
    let seat = "  # [[room.seat]]\n  # matrix = \"!abcdefghijklmnopqr\"\n  # lang = \"English\"\n";
    assert!(example.contains(seat), "the example names a seat at the Matrix door");
    let mixed = example.replace(seat, &seat.replace("  # ", "  "));
    let c = Config::parse(&mixed).unwrap();
    assert_eq!(c.doors, [Door::Discord, Door::Matrix]);
    assert_eq!(c.rooms[0].seats.len(), 3);
    assert_eq!(c.host_door(), Door::Discord);
}
