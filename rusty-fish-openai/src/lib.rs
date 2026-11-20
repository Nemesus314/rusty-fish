//! Anbieter für OpenAI und für jeden Dienst, der dasselbe Chat-Format spricht.
//!
//! Gewählt ist das Format „chat completions", nicht das neuere eigene Format
//! von OpenAI, weil andere Anbieter und selbst gehostete Modelle genau dieses
//! nachbauen. Nemesus am 18.11.2026: „Welche KI übersetzt, möchte ich
//! auswählen. [...] Ich möchte sie auch wechseln können." Adresse und Modell
//! sind deshalb Einstellungen, nichts davon ist fest verdrahtet.
//!
//! Seit dem 20.11.2026 gibt es ein zweites Drahtformat, „responses". Die
//! Dokumentation von xAI (Grok) nennt es als ihre Hauptschnittstelle und
//! zeigte am selben Tag keinen Pfad für chat completions. Beide Formate teilen
//! Adresse, Schlüssel-Kopf und Fehlerbehandlung, deshalb liegen sie in einem
//! Anschluss.

use rusty_fish_core::{Model, ModelError, Prompt, Reply};
use serde_json::{Value, json};
use std::time::Duration;

pub use rusty_fish_core::Secret;

pub const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

/// Wie Auftrag und Antwort auf der Leitung aussehen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wire {
    /// `POST {base}/chat/completions`: OpenAI, Gemini über seine
    /// Kompatibilitätsadresse, selbst gehostete Modelle.
    ChatCompletions,
    /// `POST {base}/responses`: xAI.
    Responses,
}

/// Ein Anschluss an einen Dienst im OpenAI-Format.
#[derive(Debug, Clone)]
pub struct OpenAi {
    http: reqwest::Client,
    base_url: String,
    model: String,
    /// Ohne Schlüssel wird kein Authorization-Kopf gesendet. Das ist für ein
    /// selbst gehostetes Modell gedacht, das keinen verlangt.
    key: Option<Secret>,
    wire: Wire,
}

impl OpenAi {
    /// Ein Anschluss im Format „chat completions".
    pub fn new(base_url: &str, model: &str, key: Option<Secret>) -> Result<Self, ModelError> {
        Self::build(base_url, model, key, Wire::ChatCompletions)
    }

    /// Ein Anschluss im Format „responses".
    pub fn responses(base_url: &str, model: &str, key: Option<Secret>) -> Result<Self, ModelError> {
        Self::build(base_url, model, key, Wire::Responses)
    }

    fn build(
        base_url: &str,
        model: &str,
        key: Option<Secret>,
        wire: Wire,
    ) -> Result<Self, ModelError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| ModelError(format!("cannot set up the connection: {}", e.without_url())))?;
        Ok(OpenAi {
            http,
            base_url: base_url.trim().trim_end_matches('/').to_owned(),
            model: model.trim().to_owned(),
            key,
            wire,
        })
    }

    fn authorized(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.key {
            Some(key) => request.bearer_auth(key.expose()),
            None => request,
        }
    }

    /// Welche Modelle dieser Schlüssel sieht. Gemessen statt aus dem Kopf.
    ///
    /// Benannte Grenze: für xAI ist der Pfad `{base}/models` eine Annahme. Die
    /// am 20.11.2026 gelesene Seite der xAI-Dokumentation nannte keine
    /// Modellliste, nur dass sich die Schnittstelle mit den Bibliotheken von
    /// OpenAI benutzen lässt.
    pub async fn models(&self) -> Result<Vec<String>, ModelError> {
        let response = self
            .authorized(self.http.get(format!("{}/models", self.base_url)))
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
        let (path, body) = match self.wire {
            Wire::ChatCompletions => ("chat/completions", request_body(&self.model, prompt)),
            Wire::Responses => ("responses", responses_body(&self.model, prompt)),
        };
        let response = self
            .authorized(self.http.post(format!("{}/{path}", self.base_url)))
            .json(&body)
            .send()
            .await
            .map_err(network_error)?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(network_error)?;
        match self.wire {
            Wire::ChatCompletions => read_completion(status, &body),
            Wire::Responses => read_response(status, &body),
        }
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

/// Was im Format „responses" an den Dienst geht. Rein, ohne Netz.
///
/// Die Felder stammen aus der Dokumentation von xAI (gelesen am 20.11.2026):
/// `instructions` ist dort der Weg für den Systemteil, `input` der Text.
/// `store: false`, weil der Dienst Aufträge und Antworten sonst dreißig Tage
/// zum späteren Abruf aufbewahrt. Der Fisch ruft nie etwas wieder ab, und was
/// Menschen einander schreiben, soll nirgends länger liegen als nötig.
pub fn responses_body(model: &str, prompt: &Prompt) -> Value {
    json!({
        "model": model,
        "instructions": prompt.system,
        "input": prompt.user,
        "store": false,
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
        // Der Grund steht je nach Dienst als `error.message` oder gleich als
        // Text in `error`.
        let reason = parsed
            .as_ref()
            .and_then(|v| v["error"]["message"].as_str().or_else(|| v["error"].as_str()))
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

/// Liest eine Antwort im Format „responses". Rein, ohne Netz.
///
/// Die Form stammt aus der Dokumentation von xAI (gelesen am 20.11.2026): der
/// Text steht in `output[]` in Einträgen vom Typ `message`, darin in
/// `content[]` in Stücken vom Typ `output_text`. Einträge vom Typ `reasoning`
/// sind das Nachdenken des Modells und gehören nicht zur Antwort.
pub fn read_response(status: u16, body: &str) -> Result<Reply, ModelError> {
    let v = accepted(status, body)?;
    // Fail closed: nur „completed" ist eine fertige Antwort. Die Dokumentation
    // nennt daneben „in_progress" und „incomplete".
    if let Some(state) = v["status"].as_str() {
        if state != "completed" {
            return Err(ModelError(format!("the answer is incomplete ({state})")));
        }
    }
    let mut text = String::new();
    for item in v["output"].as_array().into_iter().flatten() {
        if item["type"] != "message" {
            continue;
        }
        for piece in item["content"].as_array().into_iter().flatten() {
            if piece["type"] == "output_text" {
                text.push_str(piece["text"].as_str().unwrap_or(""));
            }
        }
    }
    if text.is_empty() {
        return Err(ModelError("the answer contains no text".to_owned()));
    }
    // Die Dokumentation zeigt den Verbrauch unter zwei Namenspaaren.
    let usage = &v["usage"];
    let sum = |a: &str, b: &str| Some(usage[a].as_u64()? + usage[b].as_u64()?);
    let tokens = usage["total_tokens"]
        .as_u64()
        .or_else(|| sum("input_tokens", "output_tokens"))
        .or_else(|| sum("prompt_tokens", "completion_tokens"));
    Ok(Reply { text, tokens })
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
