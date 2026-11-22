//! Tests zu den Namen, die der Gastgeber Menschen im Profil gibt, und zur
//! Tür, die ein Tisch nennt. Kein Netz.
//!
//! Nemesus am 21.11.2026: „Die Namen, die ich vergebe, gehören in der Windows
//! Version auf jeden Fall in die Maske. So kann man sofort sehen, für wen man
//! die Tische gebaut hat."

use rusty_fish_house::config::Config;
use rusty_fish_house::document::{DocPerson, Document};
use rusty_fish_house::invite::{Due, due};
use rusty_fish_room::Gender;
use std::collections::BTreeSet;

/// Zwei Tische, zwei Menschen mit Namen. `robin` steht, wie er übergeben wird.
fn named(robin: &str) -> String {
    format!(
        r#"
model = "some-model"
host = "nemo"

[[room]]

  [[room.seat]]
  discord = "111"
  lang = "German"
  owner = "nemo"

  [[room.seat]]
  discord = "222"
  lang = "English"
  owner = "robin"

[[person]]
name = "nemo"
discord = "100"

[[person]]
{robin}
"#
    )
}

const ROBIN: &str =
    "name = \"robin\"\ndiscord = \"7\"\nlang = \"Russian\"\ngender = \"female\"\ninvited = true";
const ROBIN_WITHOUT_ID: &str = "name = \"robin\"\nlang = \"Russian\"\ninvited = true";

fn refused(text: &str, expect: &str) {
    let err = Config::parse(text).unwrap_err();
    assert!(err.contains(expect), "expected {expect:?} in: {err}");
}

// --- Namen statt Zahlen --------------------------------------------------------

#[test]
fn host_and_owner_point_at_people_by_the_names_the_host_gave() {
    let c = Config::parse(&named(ROBIN)).unwrap();
    // An der Tür zählt weiter die Kennung.
    assert_eq!(c.host, "100");
    assert_eq!(c.host_written, "nemo");
    assert_eq!(c.owner_of("111"), Some("100"));
    assert_eq!(c.owner_of("222"), Some("7"));
    assert_eq!(c.name_of("7"), Some("robin"));
    // Was über den Menschen eingetragen ist, gilt an seinem Tisch und für ihn.
    assert_eq!(c.lang_of("222"), Some("Russian"));
    assert_eq!(c.gender_of("7"), Some(Gender::Female));
    assert!(c.serves("111") && c.serves("222"));
}

#[test]
fn a_table_can_be_built_for_someone_whose_id_is_not_known_yet() {
    let c = Config::parse(&named(ROBIN_WITHOUT_ID)).unwrap();
    assert_eq!(c.owner_of("222"), Some("robin"));
    assert_eq!(c.people["robin"].discord, None);
    // Sprache und Einladung gelten schon.
    assert_eq!(c.lang_of("222"), Some("Russian"));
    assert!(c.serves("222"));
    // Die Mitteilung zur Einladung geht an den Tisch; dafür braucht es keine Kennung.
    let (now, _) = due(&c, &BTreeSet::new());
    assert_eq!(now, [Due { person: "robin".to_owned(), channel: "222".to_owned() }]);
}

#[test]
fn getting_an_id_later_does_not_announce_the_invitation_a_second_time() {
    // Benachrichtigt wurde unter dem Namen, als es noch keine Kennung gab.
    let told: BTreeSet<String> = ["robin".to_owned()].into();
    let c = Config::parse(&named(ROBIN)).unwrap();
    let (now, kept) = due(&c, &told);
    assert!(now.is_empty(), "got {now:?}");
    // Von jetzt an gemerkt unter der Kennung.
    assert_eq!(kept, ["7".to_owned()].into());
}

#[test]
fn the_host_needs_an_id_because_the_door_has_to_recognise_the_host() {
    let text = named(ROBIN).replace("name = \"nemo\"\ndiscord = \"100\"", "name = \"nemo\"");
    refused(&text, "the host \"nemo\" needs a Discord id");
}

#[test]
fn the_old_words_still_read_and_mean_the_same() {
    let old = r#"
model = "some-model"
host = "100"

[[room]]

  [[room.seat]]
  channel = "111"
  lang = "German"
  owner = "100"

  [[room.seat]]
  channel = "222"
  lang = "English"
  owner = "7"

[[person]]
id = "100"

[[person]]
id = "7"
lang = "Russian"
gender = "female"
invited = true
"#;
    let old = Config::parse(old).unwrap();
    let new = Config::parse(&named(ROBIN)).unwrap();
    assert_eq!(old.host, new.host);
    assert_eq!(old.rooms, new.rooms, "the room logic sees no difference");
    assert_eq!(old.people["7"].name, None);
    assert_eq!(old.host_written, "100");
}

#[test]
fn what_would_be_ambiguous_or_about_nobody_is_refused() {
    // Ein Name, den es nicht gibt, wäre ein Tisch für niemanden.
    refused(&named(ROBIN).replace("owner = \"robin\"", "owner = \"robn\""), "owner");
    refused(&named(ROBIN).replace("host = \"nemo\"", "host = \"captain\""), "host");
    // Ein Name aus Ziffern wäre von einer Kennung nicht zu unterscheiden.
    refused(&named("name = \"1234\"\ndiscord = \"7\""), "digits only");
    refused(&named("name = \"nemo\"\ndiscord = \"7\""), "used for two people");
    refused(&named("lang = \"Russian\""), "needs a `name` or a `discord` id");
    refused(&named("name = \"robin\"\ndiscord = \"7\"\nid = \"7\""), "both `discord` and `id`");
    refused(&named("name = \"robin\"\ndiscord = \"100\""), "listed twice");
    let both = named(ROBIN).replace("discord = \"222\"", "discord = \"222\"\n  channel = \"222\"");
    refused(&both, "both `discord` and `channel`");
    let none = named(ROBIN).replace("  discord = \"222\"\n", "");
    refused(&none, "needs `discord`");
}

#[test]
fn a_name_is_kept_in_one_line_and_found_again_however_it_was_spaced() {
    let text = named("name = \"  robin   of the  north \"\ndiscord = \"7\"")
        .replace("owner = \"robin\"", "owner = \"robin of   the north\"");
    let c = Config::parse(&text).unwrap();
    assert_eq!(c.name_of("7"), Some("robin of the north"));
    assert_eq!(c.owner_of("222"), Some("7"));
}

// --- Mehrere Profile in einem Fisch ---------------------------------------------

#[test]
fn two_profiles_may_run_together_only_if_no_channel_belongs_to_both() {
    let tea = Config::parse(&named(ROBIN)).unwrap();
    // Dieselben Menschen, andere Kanäle: das geht nebeneinander.
    let probe = named(ROBIN).replace("\"111\"", "\"333\"").replace("\"222\"", "\"444\"");
    let probe = Config::parse(&probe).unwrap();
    assert_eq!(tea.shared_channel(&probe), None);
    assert_eq!(probe.shared_channel(&tea), None);

    // Ein Kanal in beiden: jede Nachricht dort würde doppelt übersetzt.
    let overlap = Config::parse(&named(ROBIN).replace("\"111\"", "\"333\"")).unwrap();
    assert_eq!(tea.shared_channel(&overlap), Some("222"));
    assert_eq!(overlap.shared_channel(&tea), Some("222"));
    // Ein Profil überschneidet sich mit sich selbst.
    assert!(tea.shared_channel(&tea).is_some());
}

// --- Das Dokument für die Maske -----------------------------------------------

#[test]
fn the_mask_gets_the_names_and_the_file_keeps_them() {
    let config = Config::parse(&named(ROBIN)).unwrap();
    let doc = Document::from(&config);
    assert_eq!(doc.host, "nemo");
    assert_eq!(doc.rooms[0].seats[1].owner.as_deref(), Some("robin"));
    assert_eq!(doc.rooms[0].seats[1].door, "discord");
    assert_eq!(doc.people[1].name.as_deref(), Some("robin"));
    assert_eq!(doc.people[1].discord.as_deref(), Some("7"));

    // Gespeichert und wieder gelesen ist es dasselbe Profil.
    let (toml, again) = doc.check().unwrap();
    assert_eq!(again, config);
    for line in ["host = \"nemo\"", "name = \"robin\"", "discord = \"7\"", "owner = \"robin\""] {
        assert!(toml.contains(line), "the file must say {line:?}:\n{toml}");
    }
    // Der Tisch nennt seine Tür.
    assert!(toml.contains("  discord = \"222\""), "got:\n{toml}");
    assert!(!toml.contains("channel ="), "the old word is no longer written:\n{toml}");
}

#[test]
fn a_person_with_only_a_name_survives_the_trip_too() {
    let config = Config::parse(&named(ROBIN_WITHOUT_ID)).unwrap();
    let doc = Document::from(&config);
    let expected = DocPerson {
        lang: Some("Russian".to_owned()),
        invited: true,
        ..DocPerson::named("robin")
    };
    assert_eq!(doc.people[1], expected);
    let (toml, again) = doc.check().unwrap();
    assert_eq!(again, config);
    assert!(!toml.contains("discord = \"\""), "an absent id is not written as an empty one");
}

#[test]
fn a_form_may_still_say_id_and_must_not_name_a_door_that_is_not_built() {
    let json = r#"{
        "model": "m", "host": "100",
        "rooms": [{"seats": [
            {"channel": "111", "lang": "German"},
            {"channel": "222", "lang": "Russian"}
        ]}],
        "people": [{"id": "7", "gender": "female"}]
    }"#;
    let doc = Document::from_json(json).unwrap();
    assert_eq!(doc.people[0].discord.as_deref(), Some("7"));
    assert_eq!(doc.rooms[0].seats[0].door, "discord", "without a door, Discord is meant");
    doc.check().unwrap();

    // Bis zum 22.11.2026 stand hier "matrix"; seitdem ist diese Tür gebaut.
    let mut elsewhere = doc.clone();
    elsewhere.rooms[0].seats[1].door = "signal".to_owned();
    let err = elsewhere.check().unwrap_err();
    assert!(err.contains("signal") && err.contains("discord, matrix"), "got {err}");
}
