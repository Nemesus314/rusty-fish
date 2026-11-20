//! Tests des Formats, ohne Netz und ohne Schlüssel.

use rusty_fish_core::Prompt;
use rusty_fish_openai::{Secret, read_completion, read_models, request_body};

fn prompt() -> Prompt {
    Prompt { system: "Translate.".to_owned(), user: "Hallo".to_owned() }
}

#[test]
fn request_carries_model_and_both_parts_in_order() {
    let body = request_body("some-model", &prompt());
    assert_eq!(body["model"], "some-model");
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "Translate.");
    assert_eq!(body["messages"][1]["role"], "user");
    assert_eq!(body["messages"][1]["content"], "Hallo");
    // Manche Modelle lehnen eine gesetzte Temperatur ab, also gar keine senden.
    assert!(body.get("temperature").is_none());
}

#[test]
fn reads_text_and_usage() {
    let body = r#"{"choices":[{"message":{"role":"assistant","content":"Привет"},"finish_reason":"stop"}],
                   "usage":{"prompt_tokens":40,"completion_tokens":2,"total_tokens":42}}"#;
    let reply = read_completion(200, body).unwrap();
    assert_eq!(reply.text, "Привет");
    assert_eq!(reply.tokens, Some(42));
}

#[test]
fn usage_may_be_missing() {
    // Selbst gehostete Modelle melden den Verbrauch nicht immer.
    let body = r#"{"choices":[{"message":{"content":"Привет"},"finish_reason":"stop"}]}"#;
    assert_eq!(read_completion(200, body).unwrap().tokens, None);
}

#[test]
fn rejected_key_says_so_without_repeating_anything() {
    let body = r#"{"error":{"message":"Incorrect API key provided: sk-abc123","type":"invalid_request_error"}}"#;
    let err = read_completion(401, body).unwrap_err().0;
    assert!(err.contains("401"), "got {err}");
    assert!(!err.contains("sk-"), "nothing of a key may be repeated: {err}");
}

#[test]
fn other_failures_carry_status_and_the_providers_reason() {
    let body = r#"{"error":{"message":"You exceeded your current quota.","type":"insufficient_quota"}}"#;
    let err = read_completion(429, body).unwrap_err().0;
    assert!(err.contains("429"), "got {err}");
    assert!(err.contains("You exceeded your current quota."), "got {err}");
}

#[test]
fn a_failure_that_is_not_json_is_still_reported() {
    let err = read_completion(502, "<html>Bad Gateway</html>").unwrap_err().0;
    assert!(err.contains("502"), "got {err}");
}

#[test]
fn a_cut_off_answer_fails_loudly() {
    // Eine abgeschnittene Übersetzung sähe aus wie eine vollständige.
    let body = r#"{"choices":[{"message":{"content":"Прив"},"finish_reason":"length"}]}"#;
    assert!(read_completion(200, body).is_err());
}

#[test]
fn an_answer_without_text_fails_loudly() {
    let body = r#"{"choices":[{"message":{"content":null,"refusal":"I can't help with that."},"finish_reason":"stop"}]}"#;
    assert!(read_completion(200, body).is_err());
    assert!(read_completion(200, r#"{"choices":[]}"#).is_err());
    assert!(read_completion(200, "<html>ok</html>").is_err());
}

#[test]
fn lists_models_sorted() {
    let body = r#"{"object":"list","data":[{"id":"zeta"},{"id":"alpha"},{"id":"mid"}]}"#;
    assert_eq!(read_models(200, body).unwrap(), ["alpha", "mid", "zeta"]);
    assert!(read_models(401, body).is_err());
}

#[test]
fn a_secret_never_prints() {
    let shown = format!("{:?}", Secret::new("sk-very-secret"));
    assert!(!shown.contains("very-secret"), "got {shown}");
}

// --- Das zweite Drahtformat, „responses" (xAI), seit dem 20.11.2026 ---

use rusty_fish_openai::{read_response, responses_body};

#[test]
fn a_responses_request_carries_the_system_part_as_instructions() {
    let body = responses_body("some-model", &prompt());
    assert_eq!(body["model"], "some-model");
    assert_eq!(body["instructions"], "Translate.");
    assert_eq!(body["input"], "Hallo");
    // Der Dienst soll das Gespräch nicht zum späteren Abruf aufbewahren.
    assert_eq!(body["store"], false);
    assert!(body.get("temperature").is_none());
}

#[test]
fn reads_text_and_usage_from_a_response() {
    let body = r#"{"object":"response","status":"completed","output":[
        {"type":"message","role":"assistant","status":"completed",
         "content":[{"type":"output_text","text":"Привет","logprobs":null,"annotations":[]}]}],
        "usage":{"input_tokens":40,"output_tokens":2,"total_tokens":42}}"#;
    let reply = read_response(200, body).unwrap();
    assert_eq!(reply.text, "Привет");
    assert_eq!(reply.tokens, Some(42));
}

#[test]
fn reasoning_items_are_skipped() {
    let body = r#"{"status":"completed","output":[
        {"type":"reasoning","id":"","summary":[{"type":"summary_text","text":"thinking about it"}],"status":"completed"},
        {"type":"message","role":"assistant","content":[{"type":"output_text","text":"Hallo"}]}]}"#;
    let reply = read_response(200, body).unwrap();
    assert_eq!(reply.text, "Hallo");
    assert_eq!(reply.tokens, None);
}

#[test]
fn usage_of_a_response_is_summed_when_no_total_is_given() {
    let body = r#"{"status":"completed","output":[
        {"type":"message","content":[{"type":"output_text","text":"x"}]}],
        "usage":{"input_tokens":30,"output_tokens":5}}"#;
    assert_eq!(read_response(200, body).unwrap().tokens, Some(35));
    // Ein Beispiel in der Dokumentation zeigte die Namen des Chat-Formats.
    let other = r#"{"status":"completed","output":[
        {"type":"message","content":[{"type":"output_text","text":"x"}]}],
        "usage":{"prompt_tokens":7,"completion_tokens":3}}"#;
    assert_eq!(read_response(200, other).unwrap().tokens, Some(10));
}

#[test]
fn an_unfinished_response_fails_loudly() {
    // Eine abgeschnittene Übersetzung sähe aus wie eine vollständige.
    for status in ["incomplete", "in_progress"] {
        let body = format!(
            r#"{{"status":"{status}","output":[{{"type":"message","content":[{{"type":"output_text","text":"Прив"}}]}}]}}"#
        );
        let err = read_response(200, &body).unwrap_err().0;
        assert!(err.contains(status), "got {err}");
    }
}

#[test]
fn a_response_without_text_fails_loudly() {
    assert!(read_response(200, r#"{"status":"completed","output":[]}"#).is_err());
    let only_reasoning = r#"{"status":"completed","output":[{"type":"reasoning","summary":[]}]}"#;
    assert!(read_response(200, only_reasoning).is_err());
    assert!(read_response(200, "<html>ok</html>").is_err());
}

#[test]
fn a_rejected_key_on_a_response_repeats_nothing() {
    let err = read_response(401, r#"{"error":"Incorrect API key provided: xai-abc123"}"#).unwrap_err().0;
    assert!(err.contains("401"), "got {err}");
    assert!(!err.contains("xai-"), "nothing of a key may be repeated: {err}");
}

#[test]
fn a_failure_whose_error_is_a_plain_text_is_still_reported() {
    // Die Form der Fehlerantwort stand nicht in der gelesenen Dokumentation;
    // beide üblichen Formen werden gelesen.
    let err = read_response(429, r#"{"error":"You are out of credits."}"#).unwrap_err().0;
    assert!(err.contains("429") && err.contains("out of credits"), "got {err}");
    let err = read_response(429, r#"{"error":{"message":"Slow down."}}"#).unwrap_err().0;
    assert!(err.contains("Slow down."), "got {err}");
}
