//! Anbieter für OpenAI und für jeden Dienst, der dasselbe Chat-Format spricht.
//!
//! Gewählt ist das Format „chat completions", nicht das neuere eigene Format
//! von OpenAI, weil andere Anbieter und selbst gehostete Modelle genau dieses
//! nachbauen. Nemesus am 18.11.2026: „Welche KI übersetzt, möchte ich
//! auswählen. [...] Ich möchte sie auch wechseln können." Adresse und Modell
//! sind deshalb Einstellungen, nichts davon ist fest verdrahtet.

use rusty_fish_core::{Model, ModelError, Prompt, Reply};
use serde_json::{Value, json};
use std::fmt;
use std::time::Duration;

pub const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

/// Ein Schlüssel. Er lässt sich nicht versehentlich ausgeben: `Debug` zeigt
/// ihn nie, `Display` gibt es nicht.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Secret(value.into())
    }

    /// Der einzige Weg an den Wert. Wer ihn ruft, übergibt ihn sofort an den
    /// Dienst, für den er gedacht ist, und gibt ihn nirgends aus.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

/// Ein Anschluss an einen Dienst im OpenAI-Format.
#[derive(Debug, Clone)]
pub struct OpenAi {
    http: reqwest::Client,
    base_url: String,
    model: String,
    key: Secret,
}

impl OpenAi {
    pub fn new(base_url: &str, model: &str, key: Secret) -> Result<Self, ModelError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| ModelError(format!("cannot set up the connection: {}", e.without_url())))?;
        Ok(OpenAi {
            http,
            base_url: base_url.trim().trim_end_matches('/').to_owned(),
            model: model.trim().to_owned(),
            key,
        })
    }

    /// Welche Modelle dieser Schlüssel sieht. Gemessen statt aus dem Kopf.
    pub async fn models(&self) -> Result<Vec<String>, ModelError> {
        let response = self
            .http
            .get(format!("{}/models", self.base_url))
            .bearer_auth(self.key.expose())
            .send()
            .await
            .map_err(network_error)?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(network_error)?;
        read_models(status, &body)
    }
}

impl Model for OpenAi {
    async fn complete(&self, prompt: &Prompt) -> Result<Reply, ModelError> {
        let response = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(self.key.expose())
            .json(&request_body(&self.model, prompt))
            .send()
            .await
            .map_err(network_error)?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(network_error)?;
        read_completion(status, &body)
    }
}

fn network_error(e: reqwest::Error) -> ModelError {
    ModelError(format!("cannot reach the provider: {}", e.without_url()))
}

/// Was an den Dienst geht. Rein, ohne Netz.
pub fn request_body(model: &str, prompt: &Prompt) -> Value {
    json!({
        "model": model,
        "messages": [
            { "role": "system", "content": prompt.system },
            { "role": "user", "content": prompt.user },
        ],
    })
}

/// Prüft den Status und liefert den Inhalt als JSON. Gemeinsam für alle Antworten.
fn accepted(status: u16, body: &str) -> Result<Value, ModelError> {
    // Bei abgelehntem Schlüssel wird nichts vom Dienst wiederholt: manche
    // Dienste zitieren in ihrer Meldung Teile des Schlüssels.
    if status == 401 || status == 403 {
        return Err(ModelError(format!("the provider rejected the key (HTTP {status})")));
    }
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    if !(200..300).contains(&status) {
        let reason = parsed
            .as_ref()
            .and_then(|v| v["error"]["message"].as_str())
            .unwrap_or("no reason given");
        let reason: String = reason.chars().take(300).collect();
        return Err(ModelError(format!("HTTP {status}: {reason}")));
    }
    parsed.ok_or_else(|| ModelError("the provider's answer is not JSON".to_owned()))
}

/// Liest die Antwort auf einen Übersetzungsauftrag. Rein, ohne Netz.
pub fn read_completion(status: u16, body: &str) -> Result<Reply, ModelError> {
    let v = accepted(status, body)?;
    let choice = &v["choices"][0];
    // Fail closed: eine abgeschnittene Übersetzung sähe aus wie eine vollständige.
    if let Some(reason) = choice["finish_reason"].as_str() {
        if reason != "stop" {
            return Err(ModelError(format!("the answer is incomplete ({reason})")));
        }
    }
    let Some(text) = choice["message"]["content"].as_str() else {
        return Err(ModelError("the answer contains no text".to_owned()));
    };
    Ok(Reply { text: text.to_owned(), tokens: v["usage"]["total_tokens"].as_u64() })
}

/// Liest die Liste der Modelle. Rein, ohne Netz.
pub fn read_models(status: u16, body: &str) -> Result<Vec<String>, ModelError> {
    let v = accepted(status, body)?;
    let Some(data) = v["data"].as_array() else {
        return Err(ModelError("the answer contains no model list".to_owned()));
    };
    let mut ids: Vec<String> =
        data.iter().filter_map(|m| m["id"].as_str()).map(str::to_owned).collect();
    ids.sort();
    Ok(ids)
}
