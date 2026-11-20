//! Tests des Formats, ohne Netz und ohne Schlüssel.

use rusty_fish_anthropic::{MAX_TOKENS, read_message, read_models_page, request_body};
use rusty_fish_core::Prompt;

fn prompt() -> Prompt {
    Prompt { system: "Translate.".to_owned(), user: "Hallo".to_owned() }
}

#[test]
fn the_system_part_stands_on_top_and_the_text_is_the_one_user_message() {
    let body = request_body("some-model", &prompt());
    assert_eq!(body["model"], "some-model");
    assert_eq!(body["system"], "Translate.");
    assert_eq!(body["messages"].as_array().unwrap().len(), 1);
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "Hallo");
    assert_eq!(body["max_tokens"], MAX_TOKENS);
    // Sampling-Parameter lehnt diese Fassung der Schnittstelle ab.
    for forbidden in ["temperature", "top_p", "top_k"] {
        assert!(body.get(forbidden).is_none(), "{forbidden} must not be sent");
    }
}

#[test]
fn reads_text_and_usage() {
    let body = r#"{"id":"msg_1","type":"message","role":"assistant","model":"some-model",
        "content":[{"type":"text","text":"Привет","citations":null}],
        "stop_reason":"end_turn","stop_sequence":null,"stop_details":null,
        "usage":{"input_tokens":40,"output_tokens":2,"cache_read_input_tokens":0}}"#;
    let reply = read_message(200, body).unwrap();
    assert_eq!(reply.text, "Привет");
    assert_eq!(reply.tokens, Some(42));
}

#[test]
fn thinking_blocks_are_skipped_and_text_blocks_joined() {
    let body = r#"{"content":[
        {"type":"thinking","thinking":"","signature":"abc"},
        {"type":"text","text":"Guten "},
        {"type":"text","text":"Morgen"}],
        "stop_reason":"end_turn","usage":{"input_tokens":1,"output_tokens":1}}"#;
    assert_eq!(read_message(200, body).unwrap().text, "Guten Morgen");
}

#[test]
fn a_cut_off_answer_fails_loudly() {
    // Eine abgeschnittene Übersetzung sähe aus wie eine vollständige.
    let body = r#"{"content":[{"type":"text","text":"Прив"}],"stop_reason":"max_tokens"}"#;
    let err = read_message(200, body).unwrap_err().0;
    assert!(err.contains("max_tokens"), "got {err}");
}

#[test]
fn a_declined_request_fails_loudly_and_says_so() {
    let body = r#"{"content":[],"stop_reason":"refusal",
        "stop_details":{"type":"refusal","category":"cyber","explanation":"declined"}}"#;
    let err = read_message(200, body).unwrap_err().0;
    assert!(err.contains("declined"), "got {err}");
}

#[test]
fn any_other_stop_reason_than_a_finished_turn_fails() {
    for reason in ["tool_use", "pause_turn", "model_context_window_exceeded"] {
        let body = format!(r#"{{"content":[{{"type":"text","text":"x"}}],"stop_reason":"{reason}"}}"#);
        assert!(read_message(200, &body).is_err(), "{reason} must not pass as an answer");
    }
    // Ein reguläres Ende, auch an einer Stoppfolge, ist eine Antwort.
    let ok = r#"{"content":[{"type":"text","text":"x"}],"stop_reason":"stop_sequence"}"#;
    assert_eq!(read_message(200, ok).unwrap().text, "x");
}

#[test]
fn an_answer_without_text_fails_loudly() {
    let only_thinking = r#"{"content":[{"type":"thinking","thinking":""}],"stop_reason":"end_turn"}"#;
    assert!(read_message(200, only_thinking).is_err());
    assert!(read_message(200, r#"{"content":[],"stop_reason":"end_turn"}"#).is_err());
    assert!(read_message(200, "<html>ok</html>").is_err());
}

#[test]
fn a_rejected_key_says_so_without_repeating_anything() {
    let body = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key sk-ant-abc123"}}"#;
    for status in [401, 403] {
        let err = read_message(status, body).unwrap_err().0;
        assert!(err.contains(&status.to_string()), "got {err}");
        assert!(!err.contains("sk-ant"), "nothing of a key may be repeated: {err}");
    }
}

#[test]
fn other_failures_carry_status_and_the_providers_reason() {
    let body = r#"{"type":"error","error":{"type":"not_found_error","message":"model: no-such-model"},"request_id":"req_1"}"#;
    let err = read_message(404, body).unwrap_err().0;
    assert!(err.contains("404") && err.contains("model: no-such-model"), "got {err}");
    let err = read_message(529, "<html>overloaded</html>").unwrap_err().0;
    assert!(err.contains("529"), "got {err}");
}

#[test]
fn a_page_of_models_names_where_the_next_one_starts() {
    let first = r#"{"data":[{"id":"zeta","type":"model"},{"id":"alpha","type":"model"}],
        "first_id":"zeta","has_more":true,"last_id":"alpha"}"#;
    let (ids, next) = read_models_page(200, first).unwrap();
    assert_eq!(ids, ["zeta", "alpha"]);
    assert_eq!(next.as_deref(), Some("alpha"));

    let last = r#"{"data":[{"id":"mid","type":"model"}],"first_id":"mid","has_more":false,"last_id":"mid"}"#;
    let (ids, next) = read_models_page(200, last).unwrap();
    assert_eq!(ids, ["mid"]);
    assert_eq!(next, None);
}

#[test]
fn a_model_list_that_is_none_fails_loudly() {
    assert!(read_models_page(200, r#"{"models":[]}"#).is_err());
    assert!(read_models_page(401, r#"{"data":[]}"#).is_err());
    // „Es geht weiter", aber ohne Angabe, wo: lieber scheitern als im Kreis fragen.
    assert!(read_models_page(200, r#"{"data":[{"id":"a"}],"has_more":true,"last_id":null}"#).is_err());
}

#[test]
fn the_list_is_asked_for_in_large_pages() {
    use rusty_fish_anthropic::models_url;
    assert_eq!(models_url("https://api.anthropic.com/v1", None), "https://api.anthropic.com/v1/models?limit=1000");
    assert_eq!(
        models_url("https://api.anthropic.com/v1", Some("claude-x-1")),
        "https://api.anthropic.com/v1/models?limit=1000&after_id=claude-x-1"
    );
    // Eine Kennung mit Zeichen, die in einer Adresse etwas bedeuten, darf sie nicht umbauen.
    assert_eq!(
        models_url("https://h/v1", Some("a b&c=d")),
        "https://h/v1/models?limit=1000&after_id=a%20b%26c%3Dd"
    );
}
