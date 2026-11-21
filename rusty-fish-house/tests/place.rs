//! Tests dazu, wo die KI liest. Kein Netz: geurteilt wird nur nach der Adresse.

use rusty_fish_house::config::Config;
use rusty_fish_house::place::{Place, place_of};

#[test]
fn the_services_people_know_are_outside() {
    for address in [
        "https://api.openai.com/v1",
        "https://api.anthropic.com",
        "https://api.x.ai/v1",
        "https://generativelanguage.googleapis.com/v1beta/openai",
        "https://openrouter.ai/api/v1",
    ] {
        assert_eq!(place_of(address), Place::Outside, "{address}");
    }
}

#[test]
fn an_address_that_means_this_computer_is_this_machine() {
    for address in [
        "http://localhost:11434/v1",
        "http://LOCALHOST/v1",
        "http://127.0.0.1:8080/v1",
        "http://127.5.5.5",
        "http://[::1]:8000/v1",
        "http://ollama.localhost:11434",
        "localhost:11434",
    ] {
        assert_eq!(place_of(address), Place::ThisMachine, "{address}");
    }
}

#[test]
fn a_private_address_or_a_name_only_the_own_network_knows_is_the_own_network() {
    for address in [
        "http://192.168.1.20:11434/v1",
        "http://10.0.0.7/v1",
        "http://172.16.4.4:8000",
        "http://172.31.255.1",
        "http://169.254.1.1",
        "http://[fd12:3456::1]:8000/v1",
        "http://[fe80::1]/v1",
        "http://nas:11434/v1",
        "http://workstation.local:11434",
        "http://ai.lan/v1",
        "http://box.home.arpa.",
    ] {
        assert_eq!(place_of(address), Place::OwnNetwork, "{address}");
    }
}

#[test]
fn in_doubt_an_address_counts_as_outside() {
    for address in [
        // Knapp neben den privaten Bereichen.
        "http://172.32.0.1",
        "http://192.169.0.1",
        "http://11.0.0.1",
        "http://[2001:db8::1]/v1",
        // Ein eigener Server im Internet sieht aus wie jeder andere.
        "https://ai.example.org/v1",
        // Ein Name, der nur so tut.
        "https://localhost.example.org/v1",
        "https://192.168.1.20.example.org/v1",
        // Vor dem @ steht eine Anmeldung, kein Rechner.
        "https://localhost@api.example.org/v1",
        "",
        "   ",
    ] {
        assert_eq!(place_of(address), Place::Outside, "{address:?}");
    }
}

#[test]
fn an_ipv4_address_written_the_ipv6_way_is_what_it_is() {
    assert_eq!(place_of("http://[::ffff:127.0.0.1]:8000"), Place::ThisMachine);
    assert_eq!(place_of("http://[::ffff:192.168.0.9]"), Place::OwnNetwork);
    assert_eq!(place_of("http://[::ffff:8.8.8.8]"), Place::Outside);
}

#[test]
fn a_profile_says_where_its_ai_reads() {
    const ROOMS: &str = r#"
[[room]]

  [[room.seat]]
  discord = "111"
  lang = "German"

  [[room.seat]]
  discord = "222"
  lang = "Russian"
"#;
    // Ohne Angabe gilt der Vorgabe-Dienst, und der ist draußen.
    let plain = Config::parse(&format!("model = \"m\"\nhost = \"100\"\n{ROOMS}")).unwrap();
    assert_eq!(plain.ai_place(), Place::Outside);

    let own = "provider = \"custom\"\nbase_url = \"http://localhost:11434/v1\"\n";
    let own = Config::parse(&format!("{own}model = \"m\"\nhost = \"100\"\n{ROOMS}")).unwrap();
    assert_eq!(own.ai_place(), Place::ThisMachine);

    // Es zählt die Adresse, die wirklich benutzt wird, nicht der Name des
    // Dienstes: wer einen bekannten Dienst über eine eigene Adresse anspricht,
    // schickt dorthin.
    let detour = "provider = \"openai\"\nbase_url = \"http://192.168.1.20:8000/v1\"\n";
    let detour = Config::parse(&format!("{detour}model = \"m\"\nhost = \"100\"\n{ROOMS}")).unwrap();
    assert_eq!(detour.ai_place(), Place::OwnNetwork);
}

#[test]
fn every_place_has_its_own_word_for_the_list() {
    assert_eq!(Place::ThisMachine.id(), "this_machine");
    assert_eq!(Place::OwnNetwork.id(), "own_network");
    assert_eq!(Place::Outside.id(), "outside");
}
