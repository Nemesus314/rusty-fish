//! Tests der Liste der Dienste. Kein Netz, kein Schlüssel.

use rusty_fish_core::Secret;
use rusty_fish_providers::{AnyModel, Provider, address, catalog, connect};

#[test]
fn there_are_exactly_these_five_services() {
    let ids: Vec<&str> = Provider::ALL.iter().map(|p| p.id()).collect();
    assert_eq!(ids, ["openai", "xai", "google", "anthropic", "custom"]);
}

#[test]
fn every_id_reads_back_as_its_service() {
    for provider in Provider::ALL {
        assert_eq!(Provider::parse(provider.id()), Ok(provider));
    }
    // Wie im Rest der Einstellungen: Groß/Klein und Leerraum sind egal.
    assert_eq!(Provider::parse(" Anthropic "), Ok(Provider::Anthropic));
}

#[test]
fn an_unknown_service_is_refused_and_the_known_ones_are_named() {
    let err = Provider::parse("mistral").unwrap_err();
    assert!(err.contains("mistral"), "got {err}");
    for provider in Provider::ALL {
        assert!(err.contains(provider.id()), "the message must name {}: {err}", provider.id());
    }
    assert!(Provider::parse("").is_err());
}

#[test]
fn the_catalog_is_what_a_window_needs_to_show() {
    let list = catalog();
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 5);

    let by_id = |id: &str| list.iter().find(|p| p["id"] == id).unwrap().clone();
    assert_eq!(by_id("openai")["label"], "ChatGPT (OpenAI)");
    assert_eq!(by_id("openai")["base_url"], "https://api.openai.com/v1");
    assert_eq!(by_id("xai")["label"], "Grok (xAI)");
    assert_eq!(by_id("xai")["base_url"], "https://api.x.ai/v1");
    assert_eq!(by_id("google")["label"], "Gemini (Google)");
    assert_eq!(
        by_id("google")["base_url"],
        "https://generativelanguage.googleapis.com/v1beta/openai"
    );
    assert_eq!(by_id("anthropic")["label"], "Claude (Anthropic)");
    assert_eq!(by_id("anthropic")["base_url"], "https://api.anthropic.com/v1");
    assert_eq!(by_id("custom")["label"], "Own model or other service");
    assert!(by_id("custom")["base_url"].is_null());

    for entry in list {
        let custom = entry["id"] == "custom";
        assert_eq!(entry["needs_key"], !custom, "{entry}");
        // Genau diese vier Felder, damit der Vertrag mit der Maske nicht leise wächst.
        assert_eq!(entry.as_object().unwrap().len(), 4, "{entry}");
    }
}

#[test]
fn the_address_in_the_profile_goes_before_the_one_of_the_service() {
    assert_eq!(address(Provider::OpenAi, None).unwrap(), "https://api.openai.com/v1");
    assert_eq!(address(Provider::Xai, Some("  ")).unwrap(), "https://api.x.ai/v1");
    assert_eq!(
        address(Provider::Anthropic, Some("https://proxy.example/v1/")).unwrap(),
        "https://proxy.example/v1/"
    );
    assert_eq!(
        address(Provider::Custom, Some("http://localhost:11434/v1")).unwrap(),
        "http://localhost:11434/v1"
    );
}

#[test]
fn an_own_model_without_an_address_is_refused() {
    let err = address(Provider::Custom, None).unwrap_err();
    assert!(err.contains("base_url"), "got {err}");
    assert!(address(Provider::Custom, Some("")).is_err());
}

#[test]
fn a_service_that_needs_a_key_does_not_connect_without_one() {
    for provider in [Provider::OpenAi, Provider::Xai, Provider::Google, Provider::Anthropic] {
        let err = connect(provider, None, "m", None).unwrap_err();
        assert!(err.contains(provider.label()), "got {err}");
    }
    // Ein eigenes Modell darf ohne Schlüssel laufen.
    assert!(connect(Provider::Custom, Some("http://localhost:11434/v1"), "m", None).is_ok());
}

#[test]
fn claude_gets_its_own_connection_and_the_others_the_shared_one() {
    let key = || Some(Secret::new("not-a-real-key"));
    assert!(matches!(connect(Provider::Anthropic, None, "m", key()), Ok(AnyModel::Anthropic(_))));
    for provider in [Provider::OpenAi, Provider::Xai, Provider::Google] {
        assert!(matches!(connect(provider, None, "m", key()), Ok(AnyModel::OpenAi(_))));
    }
}

#[test]
fn a_connected_service_can_cross_threads() {
    // Die Discord-Tür läuft auf mehreren Threads.
    fn assert_send_sync<T: Send + Sync + 'static>(_: &T) {}
    let model = connect(Provider::Custom, Some("http://localhost:1/v1"), "m", None).unwrap();
    assert_send_sync(&model);
}
