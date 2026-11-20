//! Tests zu Tischen, Einladungen und den festen Texten in drei Sprachen.
//! Kein Netz, keine Discord-Verbindung.

use rusty_fish_discord::config::Config;
use rusty_fish_discord::document::{DocPerson, DocSeat, Document};
use rusty_fish_discord::invite::{Due, due};
use rusty_fish_discord::render::{consent_text, consent_text_in, invitation, render, render_in};
use rusty_fish_discord::state::Store;
use rusty_fish_room::tongue::Tongue;
use rusty_fish_room::{BUDGET_USED_UP, Delivery, Gender, Mark, Reason, Sender};
use std::collections::BTreeSet;

const ALL: [Tongue; 3] = [Tongue::German, Tongue::English, Tongue::Russian];

/// Drei Tische in einem Raum. Der Gastgeber ist 100.
fn three(people: &str, owners: [&str; 3]) -> String {
    let owner = |id: &str| if id.is_empty() { String::new() } else { format!("  owner = \"{id}\"\n") };
    format!(
        "model = \"m\"\nhost = \"100\"\n\n[[room]]\npass_through = [\"English\"]\n\n  \
         [[room.seat]]\n  channel = \"111\"\n  lang = \"German\"\n{}\n  \
         [[room.seat]]\n  channel = \"222\"\n  lang = \"Russian\"\n  gender = \"male\"\n{}\n  \
         [[room.seat]]\n  channel = \"333\"\n  lang = \"Korean\"\n{}\n{people}",
        owner(owners[0]),
        owner(owners[1]),
        owner(owners[2]),
    )
}

fn refused(text: &str, expect: &str) {
    let err = Config::parse(text).expect_err("this configuration must be refused");
    assert!(err.contains(expect), "expected {expect:?} in: {err}");
}

// --- Was an einem Tisch gilt -------------------------------------------------

#[test]
fn the_owners_mother_tongue_untranslated_languages_and_form_apply_at_the_table() {
    let people = "[[person]]\nid = \"777\"\nlang = \" Русский \"\nneutral = [\" French\", \"\"]\n\
                  gender = \"female\"\ninvited = true\n";
    let c = Config::parse(&three(people, ["100", "777", ""])).unwrap();
    let table = c.seat_of("222").unwrap();
    assert_eq!(table.lang, "Русский", "the owner's mother tongue goes first");
    assert_eq!(table.neutral.as_deref(), Some(&["French".to_owned()][..]));
    assert_eq!(table.reader(), Some(Gender::Female), "the owner's form goes before the seat's");
    // Für den, der dort schreibt, bleibt es bei Person vor Platz: der Platz
    // sagt „male", die Besitzerin selbst hat ihren eigenen Eintrag.
    assert_eq!(table.gender, Some(Gender::Male), "the seat's own form stays for a writer");
    assert_eq!(c.gender_for("555", "222"), Some(Gender::Male), "an unknown writer at her table");
    assert_eq!(c.gender_for("777", "222"), Some(Gender::Female), "she herself");
    assert_eq!(c.lang_of("222"), Some("Русский"));
    assert_eq!(c.owner_of("222"), Some("777"));
    assert_eq!(c.owner_of("333"), None);
}

#[test]
fn without_details_about_the_owner_the_seat_and_the_room_apply() {
    // Der Besitzer steht nicht unter [[person]]: es bleibt bei dem, was am Platz steht.
    let c = Config::parse(&three("", ["100", "777", ""])).unwrap();
    let table = c.seat_of("222").unwrap();
    assert_eq!(table.lang, "Russian");
    assert_eq!(table.neutral, None, "the room's languages apply");
    assert_eq!(table.reader(), Some(Gender::Male));

    // Er steht dort, aber nur mit einer einzigen Angabe.
    let c = Config::parse(&three("[[person]]\nid = \"777\"\nlang = \"Polish\"\n", ["100", "777", ""]))
        .unwrap();
    let table = c.seat_of("222").unwrap();
    assert_eq!(table.lang, "Polish");
    assert_eq!(table.neutral, None);
    assert_eq!(table.reader(), Some(Gender::Male), "the seat's form stays");
}

#[test]
fn the_host_is_a_person_like_any_other_at_the_own_table() {
    // Auch der Gastgeber darf Muttersprache und unübersetzte Sprachen nennen.
    let people = "[[person]]\nid = \"100\"\nlang = \"Deutsch\"\nneutral = []\ngender = \"male\"\n";
    let c = Config::parse(&three(people, ["100", "", ""])).unwrap();
    let table = c.seat_of("111").unwrap();
    assert_eq!(table.lang, "Deutsch");
    assert_eq!(table.neutral.as_deref(), Some(&[][..]));
    assert_eq!(table.reader(), Some(Gender::Male));
    assert!(table.served);
}

#[test]
fn what_is_said_about_a_person_applies_only_at_tables_that_person_owns() {
    // 777 nennt Polnisch, besitzt aber keinen Tisch: an keinem Tisch ändert sich etwas.
    let people = "[[person]]\nid = \"777\"\nlang = \"Polish\"\nneutral = []\ninvited = true\n";
    let with = Config::parse(&three(people, ["", "", ""])).unwrap();
    let without = Config::parse(&three("", ["", "", ""])).unwrap();
    assert_eq!(with.rooms, without.rooms);
}

#[test]
fn an_empty_list_means_translate_everything_and_is_not_the_same_as_none() {
    let c = Config::parse(&three("[[person]]\nid = \"777\"\nneutral = []\n", ["100", "777", ""]))
        .unwrap();
    assert_eq!(c.seat_of("222").unwrap().neutral.as_deref(), Some(&[][..]));
    assert_eq!(c.people["777"].lang, None);
}

#[test]
fn a_person_needs_only_an_id_since_everything_else_is_optional() {
    let c = Config::parse(&three("[[person]]\nid = \"777\"\n", ["", "", ""])).unwrap();
    let p = &c.people["777"];
    assert_eq!((p.gender, p.lang.clone(), p.neutral.clone(), p.invited), (None, None, None, false));
    assert_eq!(c.gender_of("777"), None);
}

#[test]
fn what_cannot_work_is_refused() {
    refused(&three("", ["100", "quokka", ""]), "owner");
    refused(&three("[[person]]\nid = \"777\"\ninvited = \"yes\"\n", ["", "", ""]), "invited");
    refused(&three("[[person]]\nid = \"777\"\nneutral = \"English\"\n", ["", "", ""]), "neutral");
    refused(&three("[[person]]\nid = \"777\"\ngender = \"divers\"\n", ["", "", ""]), "person 777");
    refused(&three("[[person]]\nid = \"777\"\nlanguage = \"Polish\"\n", ["", "", ""]), "language");
}

// --- Welche Tische dieser Fisch bedient --------------------------------------

#[test]
fn tables_without_an_owner_are_served_as_they_always_were() {
    let c = Config::parse(&three("", ["", "", ""])).unwrap();
    assert!(c.serves("111") && c.serves("222") && c.serves("333"));
    assert!(c.rooms[0].seats.iter().all(|s| s.neutral.is_none()));
    assert!(!c.serves("999"), "a channel outside every room");
}

#[test]
fn the_fish_serves_its_hosts_table_and_those_of_the_invited() {
    let people = "[[person]]\nid = \"777\"\ninvited = true\n\n[[person]]\nid = \"888\"\n";
    let c = Config::parse(&three(people, ["100", "777", "888"])).unwrap();
    assert!(c.serves("111"), "the host's own table");
    assert!(c.serves("222"), "an invited person's table");
    assert!(!c.serves("333"), "a listed person who is not invited");

    // Ein Besitzer, über den nichts eingetragen ist, ist nicht eingeladen.
    let c = Config::parse(&three("", ["100", "777", ""])).unwrap();
    assert!(!c.serves("222"));
    assert!(c.serves("333"), "no owner, so served");
}

#[test]
fn two_fish_by_the_same_rule_serve_every_table_exactly_once() {
    // Dieselben drei Tische, zwei Gastgeber. 100 hat 777 eingeladen; 888
    // betreibt den eigenen Fisch. Keiner weiß vom anderen.
    let owners = ["100", "777", "888"];
    let first = Config::parse(&three("[[person]]\nid = \"777\"\ninvited = true\n", owners)).unwrap();
    let second = Config::parse(&three("", owners).replace("host = \"100\"", "host = \"888\"")).unwrap();
    for channel in ["111", "222", "333"] {
        let by = [first.serves(channel), second.serves(channel)];
        assert_eq!(by.iter().filter(|s| **s).count(), 1, "table {channel} is served by {by:?}");
    }
    assert!(first.serves("111") && first.serves("222") && second.serves("333"));
}

#[test]
fn two_fish_that_each_serve_only_their_host_cover_two_tables_between_them() {
    // Der einfachste Fall: zwei Menschen, zwei Tische, jeder mit eigenem Fisch.
    let two = |host: &str| {
        format!(
            "model = \"m\"\nhost = \"{host}\"\n\n[[room]]\n\n  [[room.seat]]\n  channel = \"111\"\n  \
             lang = \"German\"\n  owner = \"100\"\n\n  [[room.seat]]\n  channel = \"222\"\n  \
             lang = \"Russian\"\n  owner = \"200\"\n"
        )
    };
    let (first, second) = (Config::parse(&two("100")).unwrap(), Config::parse(&two("200")).unwrap());
    assert!(first.serves("111") && !first.serves("222"));
    assert!(!second.serves("111") && second.serves("222"));
}

// --- Von der Einstellung bis zur Zustellung ----------------------------------

/// Eine KI, die sofort antwortet und sich jeden Auftrag merkt.
struct Fake(std::sync::Mutex<Vec<rusty_fish_core::Prompt>>);

impl rusty_fish_core::Model for Fake {
    async fn complete(
        &self,
        prompt: &rusty_fish_core::Prompt,
    ) -> Result<rusty_fish_core::Reply, rusty_fish_core::ModelError> {
        self.0.lock().unwrap().push(prompt.clone());
        Ok(rusty_fish_core::Reply { text: "x".to_owned(), tokens: Some(1) })
    }
}

fn now<F: std::future::Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match f.as_mut().poll(&mut cx) {
        std::task::Poll::Ready(v) => v,
        std::task::Poll::Pending => panic!("the fake model must answer at once"),
    }
}

#[test]
fn settings_with_tables_decide_who_gets_what_in_which_language() {
    use rusty_fish_room::{Incoming, RoomState, relay};
    // 100 ist Gastgeber, 777 eingeladen (Russisch, nichts bleibt unübersetzt),
    // 888 hat den eigenen Fisch.
    let people = "[[person]]\nid = \"777\"\nlang = \"Русский\"\nneutral = []\ngender = \"female\"\n\
                  invited = true\n";
    let c = Config::parse(&three(people, ["100", "777", "888"])).unwrap();
    let say = |channel: &str| {
        let fake = Fake(std::sync::Mutex::new(Vec::new()));
        let msg = Incoming { channel, author: "zebra#1", text: "Hallo", consented: true, author_gender: None };
        let out = now(relay(&fake, &c.rooms[0], &mut RoomState::default(), &msg));
        let to: Vec<String> = out.iter().map(|d| d.channel.clone()).collect();
        (to, fake.0.into_inner().unwrap())
    };

    // Vom Tisch des Gastgebers: nur an den Tisch der Eingeladenen, in ihrer
    // Muttersprache, mit ihrer Anrede, und nichts bleibt unübersetzt.
    let (to, prompts) = say("111");
    assert_eq!(to, ["222"]);
    assert_eq!(prompts.len(), 1);
    let system = &prompts[0].system;
    assert!(system.contains("into Русский"), "got: {system}");
    assert!(system.contains("The reader of your translation is a woman"), "got: {system}");
    assert!(!system.contains("English"), "nothing stays untranslated for her: {system}");

    // Vom nicht bedienten Tisch: an beide bedienten, an ihn selbst nichts.
    let (to, prompts) = say("333");
    assert_eq!(to, ["111", "222"]);
    assert_eq!(prompts.len(), 2);
    // Am Tisch des Gastgebers gilt weiter die Angabe des Raums.
    let german = prompts.iter().find(|p| p.system.contains("into German")).unwrap();
    assert!(german.system.contains("English"), "got: {}", german.system);
}

// --- Wessen Worte dieser Fisch an die KI gäbe --------------------------------

#[test]
fn the_fish_asks_for_consent_only_where_it_would_hand_words_to_the_ai() {
    // 100 ist Gastgeber, 777 eingeladen, 888 hat den eigenen Fisch.
    let people = "[[person]]\nid = \"777\"\ninvited = true\n";
    let c = Config::parse(&three(people, ["100", "777", "888"])).unwrap();
    // Was an 333 geschrieben wird, übersetzt dieser Fisch für 111 und 222:
    // dort bittet er um Zustimmung, obwohl er an 333 sonst nichts schreibt.
    assert!(!c.serves("333"));
    assert!(c.sends_to_ai("333"));
    assert!(c.sends_to_ai("111") && c.sends_to_ai("222"));
    assert!(!c.sends_to_ai("999"), "a channel outside every room");

    // Bedient der Fisch nur den Tisch, an dem geschrieben wird, liest die KI
    // nichts davon: es gibt niemanden, für den er es übersetzen würde.
    let c = Config::parse(&three("", ["100", "777", "888"])).unwrap();
    assert!(c.serves("111") && !c.serves("222") && !c.serves("333"));
    assert!(!c.sends_to_ai("111"), "nothing written here goes to the AI");
    assert!(c.sends_to_ai("222") && c.sends_to_ai("333"), "it is translated for table 111");

    // Mit einer ansprechbaren KI kann am bedienten Tisch eine Frage an sie gehen.
    let named = three("", ["100", "777", "888"]).replace("host = \"100\"", "host = \"100\"\nai_name = \"Fish\"");
    let c = Config::parse(&named).unwrap();
    assert!(c.sends_to_ai("111"));

    // Ein Fisch, der gar keinen Tisch bedient, gibt nichts an die KI.
    let c = Config::parse(&named.replace("host = \"100\"", "host = \"555\"")).unwrap();
    assert!(["111", "222", "333"].iter().all(|ch| !c.serves(ch) && !c.sends_to_ai(ch)));

    // Alte Profile: jeder Tisch ist bedient, an jedem wird gefragt.
    let c = Config::parse(&three("", ["", "", ""])).unwrap();
    assert!(["111", "222", "333"].iter().all(|ch| c.sends_to_ai(ch)));
}

// --- Das Dokument für die Maske ----------------------------------------------

fn doc_with_tables() -> Document {
    let json = r#"{"model": "m", "host": "100", "rooms": [{"pass_through": ["English"], "seats": [
        {"channel": "111", "lang": "German", "owner": "100"},
        {"channel": "222", "lang": "Russian", "gender": "male", "owner": "777"},
        {"channel": "333", "lang": "Korean"}]}],
        "people": [
          {"id": "777", "gender": "female", "lang": "Русский", "neutral": [], "invited": true},
          {"id": "888"}]}"#;
    Document::from_json(json).unwrap()
}

#[test]
fn the_new_details_survive_the_trip_through_the_file_format() {
    let doc = doc_with_tables();
    let (toml, config) = doc.check().expect("a sound document must be accepted");
    assert_eq!(Config::parse(&toml).unwrap(), config);
    assert_eq!(Document::from(&config), doc, "the trip changed the document:\n{toml}");
    // Gezeigt wird, was eingetragen wurde, nicht das Abgeleitete.
    let back = Document::from(&config);
    assert_eq!(back.rooms[0].seats[1].lang, "Russian");
    assert_eq!(back.rooms[0].seats[1].gender.as_deref(), Some("male"));
    assert_eq!(config.seat_of("222").unwrap().lang, "Русский");
}

#[test]
fn people_keep_the_order_in_which_they_were_written() {
    // Die Maske zeigt die Menschen in der Reihenfolge, in der der Gastgeber
    // sie eingetragen hat, und vergleicht nach dem Speichern genau damit.
    // "9" käme nach dem Alphabet hinter "42" und "11" vor beide.
    let json = r#"{"model": "m", "host": "42", "rooms": [{"seats": [
        {"channel": "7", "lang": "German", "owner": "42"},
        {"channel": "8", "lang": "Russian", "owner": "9"},
        {"channel": "10", "lang": "Korean", "owner": "11"}]}],
        "people": [
          {"id": "42", "gender": "male", "lang": "German"},
          {"id": "9", "lang": "Russian", "neutral": ["English"], "invited": true},
          {"id": "11", "lang": "Korean"}]}"#;
    let doc = Document::from_json(json).unwrap();
    let (_, config) = doc.check().unwrap();
    let back = Document::from(&config);
    let ids: Vec<&str> = back.people.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["42", "9", "11"]);
    assert_eq!(back, doc);
}

#[test]
fn the_new_fields_always_go_out_even_when_empty() {
    let (_, config) = doc_with_tables().check().unwrap();
    let v = serde_json::to_value(Document::from(&config)).unwrap();
    let seats = &v["rooms"][0]["seats"];
    assert_eq!(seats[0]["owner"], "100");
    assert!(seats[2].as_object().unwrap().contains_key("owner"));
    assert!(seats[2]["owner"].is_null());
    assert!(!seats[0].as_object().unwrap().contains_key("served"), "only the list says so");

    let bare = &v["people"][1];
    assert_eq!(bare["id"], "888");
    for field in ["gender", "lang", "neutral"] {
        assert!(bare.as_object().unwrap().contains_key(field), "{field} is missing");
        assert!(bare[field].is_null(), "{field}");
    }
    assert_eq!(bare["invited"], false);
    let full = &v["people"][0];
    assert_eq!(full["lang"], "Русский");
    assert_eq!(full["neutral"], serde_json::json!([]));
    assert_eq!(full["invited"], true);
}

#[test]
fn the_list_says_for_every_table_whether_this_fish_serves_it() {
    let (_, config) = doc_with_tables().check().unwrap();
    let listed = Document::from(&config).with_served(&config);
    let served: Vec<Option<bool>> = listed.rooms[0].seats.iter().map(|s| s.served).collect();
    assert_eq!(served, [Some(true), Some(true), Some(true)]);

    let mut doc = doc_with_tables();
    doc.people[0].invited = false;
    let (_, config) = doc.check().unwrap();
    let listed = Document::from(&config).with_served(&config);
    let served: Vec<Option<bool>> = listed.rooms[0].seats.iter().map(|s| s.served).collect();
    assert_eq!(served, [Some(true), Some(false), Some(true)]);
    let v = serde_json::to_value(&listed).unwrap();
    assert_eq!(v["rooms"][0]["seats"][1]["served"], false);
}

#[test]
fn what_the_list_added_is_ignored_when_it_comes_back() {
    // Eine Maske darf einen Eintrag der Liste zurückschicken.
    let (_, config) = doc_with_tables().check().unwrap();
    let json = serde_json::to_string(&Document::from(&config).with_served(&config)).unwrap();
    let again = Document::from_json(&json).unwrap();
    let (toml, _) = again.check().unwrap();
    assert!(!toml.contains("served"), "{toml}");
}

#[test]
fn documents_from_before_stay_valid_and_mean_what_they_meant() {
    let json = r#"{"model": "m", "host": "1", "rooms": [{"seats": [
        {"channel": "111", "lang": "German", "gender": null},
        {"channel": "222", "lang": "Russian", "gender": "female"}]}],
        "people": [{"id": "100", "gender": "male"}]}"#;
    let doc = Document::from_json(json).unwrap();
    assert_eq!(doc.people[0], DocPerson { gender: Some("male".to_owned()), ..DocPerson::new("100") });
    assert_eq!(doc.rooms[0].seats[0], DocSeat::new("111", "German"));
    let (_, config) = doc.check().unwrap();
    assert!(config.serves("111") && config.serves("222"));
    assert_eq!(config.gender_of("100"), Some(Gender::Male));
    assert_eq!(config.seat_of("222").unwrap().gender, Some(Gender::Female));
}

#[test]
fn the_shipped_example_shows_the_new_settings_commented_out() {
    let example = include_str!("../../rusty-fish.example.toml");
    for line in ["# owner = ", "# lang = ", "# neutral = ", "# invited = true"] {
        assert!(example.contains(line), "the example must show {line:?}");
    }
    let filled = example.replace("host = \"000000000000000000\"", "host = \"42\"");
    let c = Config::parse(&filled).unwrap();
    assert!(c.rooms[0].seats.iter().all(|s| s.served && s.neutral.is_none()));

    // Wer die Zeichen davor entfernt, bekommt eine gültige Einstellung.
    let switched_on = filled
        .replace("# owner = \"000000000000000003\"", "owner = \"000000000000000003\"")
        .replace("# [[person]]", "[[person]]")
        .replace("# id = \"000000000000000003\"", "id = \"000000000000000003\"")
        .replace("# lang = \"Russian\"", "lang = \"Russian\"")
        .replace("# neutral = [\"English\"]", "neutral = [\"English\"]")
        .replace("# invited = true", "invited = true");
    let c = Config::parse(&switched_on).unwrap();
    let person = &c.people["000000000000000003"];
    assert_eq!(person.lang.as_deref(), Some("Russian"));
    assert_eq!(person.neutral.as_deref(), Some(&["English".to_owned()][..]));
    assert!(person.invited);
    assert_eq!(c.rooms[0].seats.iter().filter(|s| s.served).count(), 2);
    assert_eq!(c.owner_of("000000000000000002"), Some("000000000000000003"));
}

// --- Die Einladung -----------------------------------------------------------

fn told(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}

fn invited_config(invited: bool) -> Config {
    let people = format!("[[person]]\nid = \"777\"\ninvited = {invited}\n");
    Config::parse(&three(&people, ["100", "777", ""])).unwrap()
}

#[test]
fn an_invited_person_is_told_once_at_their_own_table() {
    let c = invited_config(true);
    let (now, kept) = due(&c, &told(&[]));
    assert_eq!(now, [Due { person: "777".to_owned(), channel: "222".to_owned() }]);
    assert!(kept.is_empty(), "told is only what was really delivered");

    let (now, kept) = due(&c, &told(&["777"]));
    assert!(now.is_empty(), "not a second time");
    assert_eq!(kept, told(&["777"]));
}

#[test]
fn taking_the_tick_away_and_setting_it_again_tells_them_again() {
    let (now, kept) = due(&invited_config(false), &told(&["777"]));
    assert!(now.is_empty());
    assert!(kept.is_empty(), "who is no longer invited is forgotten");
    let (now, _) = due(&invited_config(true), &kept);
    assert_eq!(now.len(), 1, "invited again, told again");
}

#[test]
fn only_people_with_a_table_of_their_own_are_told_and_never_the_host() {
    // 777 ist eingeladen, hat aber keinen Tisch. Der Gastgeber lädt sich nicht selbst ein.
    let people = "[[person]]\nid = \"777\"\ninvited = true\n\n[[person]]\nid = \"100\"\ninvited = true\n";
    let c = Config::parse(&three(people, ["100", "", ""])).unwrap();
    let (now, _) = due(&c, &told(&[]));
    assert!(now.is_empty(), "got {now:?}");
}

#[test]
fn a_person_with_two_tables_is_told_at_the_first() {
    let text = format!(
        "{}\n[[room]]\n\n  [[room.seat]]\n  channel = \"444\"\n  lang = \"Russian\"\n  owner = \"777\"\n\n  \
         [[room.seat]]\n  channel = \"555\"\n  lang = \"German\"\n",
        three("", ["100", "777", ""])
    ) + "\n[[person]]\nid = \"777\"\ninvited = true\n";
    let c = Config::parse(&text).unwrap();
    let (now, _) = due(&c, &told(&[]));
    assert_eq!(now, [Due { person: "777".to_owned(), channel: "222".to_owned() }]);
}

#[test]
fn who_was_told_survives_a_restart() {
    let dir = std::env::temp_dir().join(format!("rusty-fish-test-{}-told", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("state.json");

    let mut store = Store::open(&path).unwrap();
    assert!(store.told().is_empty());
    store.set_told(told(&["777"]));
    store.save().unwrap();
    assert_eq!(Store::open(&path).unwrap().told(), &told(&["777"]));

    // Eine Zustandsdatei von vor dem 20.11.2026 kennt das Feld nicht.
    std::fs::write(&path, r#"{"consented": ["5"], "last_seen": {}}"#).unwrap();
    let old = Store::open(&path).unwrap();
    assert!(old.told().is_empty());
    assert!(old.has_consented("5"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Ein Start des Fischs, so wie die Tür ihn durchläuft: fällige Mitteilungen
/// absetzen und nur merken, was wirklich ankam. Liefert, was fällig war.
fn start(config: &Config, store: &mut Store, arrives: bool) -> Vec<Due> {
    let (now, mut told) = due(config, store.told());
    if arrives {
        told.extend(now.iter().map(|d| d.person.clone()));
    }
    store.set_told(told);
    store.save().unwrap();
    now
}

#[test]
fn over_several_starts_the_invitation_comes_exactly_once_per_tick() {
    let dir = std::env::temp_dir().join(format!("rusty-fish-test-{}-starts", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("state.json");
    let reopen = || Store::open(&path).unwrap();
    let (invited, not_invited) = (invited_config(true), invited_config(false));

    // Erster Start: die Zustellung scheitert. Nichts gilt als mitgeteilt.
    assert_eq!(start(&invited, &mut reopen(), false).len(), 1);
    // Zweiter Start: sie wird wiederholt und kommt an.
    assert_eq!(start(&invited, &mut reopen(), true).len(), 1);
    // Danach nie wieder, über beliebig viele Starts.
    assert!(start(&invited, &mut reopen(), true).is_empty());
    assert!(start(&invited, &mut reopen(), true).is_empty());

    // Der Haken wird entfernt, der Fisch läuft einmal: nichts zu sagen, und
    // der Mensch ist vergessen.
    assert!(start(&not_invited, &mut reopen(), true).is_empty());
    assert!(reopen().told().is_empty());
    // Der Haken wird wieder gesetzt: die Mitteilung kommt erneut, einmal.
    assert_eq!(start(&invited, &mut reopen(), true).len(), 1);
    assert!(start(&invited, &mut reopen(), true).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_state_remembers_only_ids_and_stays_as_it_was_while_nobody_is_invited() {
    let dir = std::env::temp_dir().join(format!("rusty-fish-test-{}-toldraw", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("state.json");

    // Solange niemandem etwas mitgeteilt wurde, steht das Feld nicht in der
    // Datei: ein Programm von vor dem 20.11.2026 liest sie weiter.
    let mut store = Store::open(&path).unwrap();
    store.consent("5");
    store.save().unwrap();
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(!raw.contains("invited_told"), "got {raw}");

    store.set_told(told(&["777"]));
    store.save().unwrap();
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(v["invited_told"], serde_json::json!(["777"]));
    let _ = std::fs::remove_dir_all(&dir);
}

// --- Die festen Texte in drei Sprachen ---------------------------------------

fn delivery(text: &str, mark: Mark) -> Delivery {
    Delivery { channel: "222".to_owned(), text: text.to_owned(), mark, from: Sender::Writer }
}

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
fn the_consent_prompt_exists_as_a_fixed_text_in_three_languages() {
    assert_eq!(consent_text_in(Tongue::English, "api.example"), consent_text("api.example"));
    let de = consent_text_in(Tongue::German, "api.example");
    let ru = consent_text_in(Tongue::Russian, "api.example");
    for text in [&de, &ru] {
        assert!(text.contains("api.example"), "the service must be named: {text}");
        assert!(text.contains("OK"), "the button must be named: {text}");
        assert!(text.contains("Rusty Fish"));
    }
    // Per du, wie am 19.11.2026 entschieden.
    assert!(de.contains(" du ") && !de.contains("Sie "), "got {de}");
    assert!(ru.contains(" ты ") && !ru.contains(" вы ") && !ru.contains("Вы "), "got {ru}");
    assert!(de != ru && de != consent_text("api.example"));
}

#[test]
fn the_invitation_names_the_host_when_the_fish_knows_the_name() {
    for tongue in ALL {
        let named = invitation(tongue, Some("Zebra"));
        assert!(named.matches("Zebra").count() >= 1, "got {named}");
        let unnamed = invitation(tongue, None);
        assert!(!unnamed.contains("Zebra"));
        assert!(!unnamed.contains("{") && !named.contains("{"), "a placeholder was left in");
        assert!(named != unnamed);
        // Ein leerer Name ist kein Name.
        assert_eq!(invitation(tongue, Some("  ")), unnamed);
    }
    assert!(invitation(Tongue::German, None).contains("eingeladen"));
    assert!(invitation(Tongue::English, None).contains("invited"));
    assert!(invitation(Tongue::Russian, None).contains("приглас"));
    let texts = ALL.map(|t| invitation(t, None));
    assert!(texts[0] != texts[1] && texts[1] != texts[2]);
}

#[test]
fn the_invitation_says_what_appears_here_and_who_pays() {
    let de = invitation(Tongue::German, Some("Zebra"));
    assert!(de.contains("Rusty Fish") && de.contains("Sprache") && de.contains("Kosten"), "got {de}");
    assert!(de.contains(" dich ") && !de.contains("Sie "), "per du: {de}");
    let en = invitation(Tongue::English, Some("Zebra"));
    assert!(en.contains("Rusty Fish") && en.contains("language") && en.contains("paid"), "got {en}");
    let ru = invitation(Tongue::Russian, Some("Zebra"));
    assert!(ru.contains("Rusty Fish") && ru.contains("языке") && ru.contains("оплачивает"), "got {ru}");
    assert!(ru.contains(" тебя") && !ru.contains(" вас") && !ru.contains("Вы "), "per ты: {ru}");
}

#[test]
fn a_display_name_cannot_break_the_invitation() {
    // Ein Anzeigename ist frei gewählt: er darf die Mitteilung weder umbrechen
    // noch beliebig verlängern.
    for tongue in ALL {
        let odd = invitation(tongue, Some("  Zebra\nQuokka  "));
        assert!(odd.contains("Zebra Quokka") && !odd.contains('\n'), "got {odd:?}");
        let long = invitation(tongue, Some(&"z".repeat(5000)));
        assert!(long.chars().count() < 600, "got {} characters", long.chars().count());
    }
}

#[test]
fn every_fixed_text_fits_into_one_discord_message() {
    use rusty_fish_discord::render::MESSAGE_LIMIT;
    let service = "a-rather-long-host-name.of-some-ai-service.example";
    for tongue in ALL {
        assert!(consent_text_in(tongue, service).chars().count() < MESSAGE_LIMIT);
        assert!(invitation(tongue, Some(&"z".repeat(5000))).chars().count() < MESSAGE_LIMIT);
    }
}
