//! Die Anrede eines Menschen im Profil: vertraut, respektvoll oder, ohne
//! Angabe, wie im Original. Keine Tür, kein Netz.
//!
//! Seit dem 23.11.2026. Nemesus an dem Tag: „in Korea redest du ja anders mit
//! einer älteren Person als mit einer jüngeren."

use rusty_fish_house::config::Config;
use rusty_fish_house::document::Document;
use rusty_fish_room::Formality;

/// Der Gastgeber liest Deutsch, seine Eingeladene Koreanisch; sie wird
/// respektvoll angesprochen.
const PROFILE: &str = r#"
model = "some-model"
host = "nemo"

[[room]]

  [[room.seat]]
  discord = "111"
  lang = "German"
  owner = "nemo"

  [[room.seat]]
  discord = "222"
  lang = "Korean"
  owner = "mina"

[[person]]
name = "nemo"
discord = "100"

[[person]]
name = "mina"
discord = "7"
formality = "respectful"
invited = true
"#;

#[test]
fn a_person_can_be_addressed_respectfully_or_in_the_familiar_way() {
    let c = Config::parse(PROFILE).unwrap();
    assert_eq!(c.people["7"].formality, Some(Formality::Respectful));
    assert_eq!(c.people["100"].formality, None, "without an entry: as the writer wrote");

    let familiar = PROFILE.replace("\"respectful\"", "\"Familiar\"");
    assert_eq!(Config::parse(&familiar).unwrap().people["7"].formality, Some(Formality::Familiar));
    // Ein leerer Eintrag heißt wie ein fehlender.
    let empty = PROFILE.replace("\"respectful\"", "\"\"");
    assert_eq!(Config::parse(&empty).unwrap().people["7"].formality, None);
}

#[test]
fn it_goes_with_what_the_person_reads_at_their_own_table() {
    let c = Config::parse(PROFILE).unwrap();
    assert_eq!(c.seat_of("222").unwrap().reader_formality, Some(Formality::Respectful));
    // Am Tisch des Gastgebers gilt, was für ihn eingetragen ist: nichts.
    assert_eq!(c.seat_of("111").unwrap().reader_formality, None);
    // Ein Tisch ohne Besitzer hat keine Anrede.
    let nobody = PROFILE.replace("  owner = \"mina\"\n", "");
    assert_eq!(Config::parse(&nobody).unwrap().seat_of("222").unwrap().reader_formality, None);
}

#[test]
fn any_other_word_is_refused_with_what_to_write_instead() {
    for bad in ["polite", "older", "formal", "as written"] {
        let err = Config::parse(&PROFILE.replace("\"respectful\"", &format!("\"{bad}\"")))
            .unwrap_err();
        assert!(err.contains("`formality` for person 7"), "{bad}: {err}");
        assert!(err.contains("\"familiar\" or \"respectful\""), "{bad}: {err}");
        assert!(err.contains("leave it out"), "{bad}: {err}");
    }
}

#[test]
fn it_survives_the_trip_through_the_form() {
    let c = Config::parse(PROFILE).unwrap();
    let doc = Document::from(&c);
    assert_eq!(doc.people[1].formality.as_deref(), Some("respectful"));
    assert_eq!(doc.people[0].formality, None);
    let (toml, again) = doc.check().unwrap();
    assert_eq!(again, c);
    assert_eq!(toml.matches("formality = ").count(), 1, "written only where it is set: {toml}");
    assert!(toml.contains("formality = \"respectful\""));

    // Das Dokument nennt das Feld nur, wo es gesetzt ist: eine ältere Maske,
    // die es nicht kennt, liest ein Profil ohne Anrede weiter.
    let json = serde_json::to_string(&Document::from(&c)).unwrap();
    assert_eq!(json.matches("\"formality\"").count(), 1, "got {json}");

    // Eine Maske kann es setzen, und ein Dokument ohne das Feld gilt wie bisher.
    let from_form = r#"{
        "model": "m", "host": "100",
        "rooms": [{"seats": [
            {"channel": "111", "lang": "German"},
            {"channel": "222", "lang": "Korean", "owner": "7"}
        ]}],
        "people": [{"discord": "7", "formality": "familiar"}, {"discord": "8"}]
    }"#;
    let (_, config) = Document::from_json(from_form).unwrap().check().unwrap();
    assert_eq!(config.people["7"].formality, Some(Formality::Familiar));
    assert_eq!(config.people["8"].formality, None);
    assert_eq!(config.seat_of("222").unwrap().reader_formality, Some(Formality::Familiar));
}

#[test]
fn a_profile_without_the_entry_is_written_as_before() {
    let plain = PROFILE.replace("formality = \"respectful\"\n", "");
    let c = Config::parse(&plain).unwrap();
    let (toml, _) = Document::from(&c).check().unwrap();
    assert!(!toml.contains("formality"), "got {toml}");
}
