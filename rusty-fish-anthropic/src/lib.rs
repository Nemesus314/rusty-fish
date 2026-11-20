//! Anbieter für Claude, über die Messages-Schnittstelle von Anthropic.
//!
//! Claude spricht ein eigenes Format, deshalb ein eigener kleiner Anschluss
//! nach dem Muster von `rusty-fish-openai`. Für Rust gibt es keine offizielle
//! Bibliothek von Anthropic; der Anschluss spricht die Schnittstelle direkt.
//!
//! Adresse, Köpfe, Versionskopf, Form von Auftrag und Antwort sowie die
//! Modellliste sind am 20.11.2026 aus Anthropics eigener Dokumentation
//! übernommen, nichts davon aus dem Gedächtnis:
//! - `POST {base}/messages` mit den Köpfen `x-api-key`, `anthropic-version`
//!   und `content-type`; der Systemteil steht oben als `system`, nicht als
//!   Nachricht; `max_tokens` ist Pflicht.
//! - Die Antwort trägt `content` als Liste von Blöcken; Text steht in Blöcken
//!   vom Typ `text`. Daneben können Denk-Blöcke stehen, die übersprungen werden.
//! - `GET {base}/models` liefert `data`, `has_more` und `last_id`; weiter geht
//!   es mit `after_id`.
//!
//! Entschieden beim Bau, von Nemesus änderbar: Lehnt Claude einen Auftrag ab
//! (`stop_reason: "refusal"`), ist das hier ein lauter Fehler, und die
//! Nachricht geht unübersetzt und markiert hinaus. Anthropic bietet für
//! einige Modelle an, eine Ablehnung von einem anderen Modell auffangen zu
//! lassen. Das ist nicht eingeschaltet, weil das Modell im Profil frei gewählt
//! wird und der Schalter nicht für jedes Modell gilt.

use rusty_fish_core::{Model, ModelError, Prompt, Reply, Secret};
use serde_json::{Value, json};
use std::time::Duration;

pub const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com/v1";

/// Die Fassung der Schnittstelle, gegen die dieser Anschluss geschrieben ist.
pub const ANTHROPIC_VERSION: &str = "2026-10-21";

/// Obergrenze für eine Antwort. Anthropics Empfehlung für Aufrufe ohne
/// Streaming; eine zu knappe Grenze schneidet die Antwort ab, und eine
/// abgeschnittene Übersetzung ist hier ein Fehler.
pub const MAX_TOKENS: u32 = 16_000;

/// Ein Anschluss an Claude.
#[derive(Debug, Clone)]
pub struct Anthropic {
    http: reqwest::Client,
    base_url: String,
    model: String,
    key: Secret,
}

impl Anthropic {
    pub fn new(base_url: &str, model: &str, key: Secret) -> Result<Self, ModelError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| ModelError(format!("cannot set up the connection: {}", e.without_url())))?;
        Ok(Anthropic {
            http,
            base_url: base_url.trim().trim_end_matches('/').to_owned(),
            model: model.trim().to_owned(),
            key,
        })
    }

    fn headers(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        request.header("x-api-key", self.key.expose()).header("anthropic-version", ANTHROPIC_VERSION)
    }

    /// Welche Modelle dieser Schlüssel sieht. Die Liste kommt seitenweise.
    pub async fn models(&self) -> Result<Vec<String>, ModelError> {
        let mut ids = Vec::new();
        let mut after: Option<String> = None;
        // Zehn Seiten zu je tausend sind weit mehr, als es Modelle gibt. Die
        // Grenze steht nur da, damit eine fehlerhafte Antwort keine
        // Endlosschleife auslöst.
        for _ in 0..10 {
            let request = self.http.get(models_url(&self.base_url, after.as_deref()));
            let response = self.headers(request).send().await.map_err(network_error)?;
            let status = response.status().as_u16();
            let body = response.text().await.map_err(network_error)?;
            let (page, next) = read_models_page(status, &body)?;
            ids.extend(page);
            match next {
                Some(id) => after = Some(id),
                None => break,
            }
        }
        ids.sort();
        Ok(ids)
    }
}

impl Model for Anthropic {
    async fn complete(&self, prompt: &Prompt) -> Result<Reply, ModelError> {
        let response = self
            .headers(self.http.post(format!("{}/messages", self.base_url)))
            .json(&request_body(&self.model, prompt))
            .send()
            .await
            .map_err(network_error)?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(network_error)?;
        read_message(status, &body)
    }
}

fn network_error(e: reqwest::Error) -> ModelError {
    ModelError(format!("cannot reach the provider: {}", e.without_url()))
}

/// Die Adresse einer Seite der Modellliste: tausend Einträge, das Höchste, was
/// die Schnittstelle erlaubt, und ab der zweiten Seite hinter `after`.
pub fn models_url(base_url: &str, after: Option<&str>) -> String {
    let mut url = format!("{}/models?limit=1000", base_url.trim().trim_end_matches('/'));
    if let Some(id) = after {
        url.push_str("&after_id=");
        // Von Hand maskiert, weil es eine einzige Stelle ist: alles außer
        // Buchstaben, Ziffern und den vier unverfänglichen Zeichen wird %XX.
        for byte in id.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    url.push(byte as char);
                }
                other => url.push_str(&format!("%{other:02X}")),
            }
        }
    }
    url
}

/// Was an Claude geht. Rein, ohne Netz.
///
/// Bewusst ohne `temperature` und Verwandte: diese Fassung der Schnittstelle
/// lehnt sie ab. Auch ohne Angabe zum Nachdenken: jedes Modell hält es damit,
/// wie es für dieses Modell vorgesehen ist.
pub fn request_body(model: &str, prompt: &Prompt) -> Value {
    json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "system": prompt.system,
        "messages": [
            { "role": "user", "content": prompt.user },
        ],
    })
}

/// Prüft den Status und liefert den Inhalt als JSON.
fn accepted(status: u16, body: &str) -> Result<Value, ModelError> {
    // Bei abgelehntem Schlüssel wird nichts vom Dienst wiederholt: seine
    // Meldung könnte Teile des Schlüssels zitieren.
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

/// Liest die Antwort auf einen Auftrag. Rein, ohne Netz.
pub fn read_message(status: u16, body: &str) -> Result<Reply, ModelError> {
    let v = accepted(status, body)?;
    // Fail closed: eine Antwort gilt nur, wenn Claude von sich aus fertig war.
    // An der Längengrenze abgeschnitten, sähe eine Übersetzung aus wie eine
    // vollständige; alle übrigen Gründe sind hier ebenso wenig eine Antwort.
    match v["stop_reason"].as_str() {
        Some("end_turn") | Some("stop_sequence") => {}
        Some("refusal") => {
            return Err(ModelError("the model declined this request".to_owned()));
        }
        Some(other) => {
            return Err(ModelError(format!("the answer is incomplete ({other})")));
        }
        None => return Err(ModelError("the answer does not say how it ended".to_owned())),
    }
    // Text steht in Blöcken vom Typ `text`. Denk-Blöcke werden übersprungen.
    let mut text = String::new();
    for block in v["content"].as_array().into_iter().flatten() {
        if block["type"] == "text" {
            text.push_str(block["text"].as_str().unwrap_or(""));
        }
    }
    if text.is_empty() {
        return Err(ModelError("the answer contains no text".to_owned()));
    }
    let usage = &v["usage"];
    let tokens = match (usage["input_tokens"].as_u64(), usage["output_tokens"].as_u64()) {
        (Some(input), Some(output)) => Some(input + output),
        _ => None,
    };
    Ok(Reply { text, tokens })
}

/// Liest eine Seite der Modellliste. Liefert die Kennungen und, falls es eine
/// weitere Seite gibt, die Kennung, hinter der sie beginnt. Rein, ohne Netz.
pub fn read_models_page(status: u16, body: &str) -> Result<(Vec<String>, Option<String>), ModelError> {
    let v = accepted(status, body)?;
    let Some(data) = v["data"].as_array() else {
        return Err(ModelError("the answer contains no model list".to_owned()));
    };
    let ids = data.iter().filter_map(|m| m["id"].as_str()).map(str::to_owned).collect();
    let next = if v["has_more"].as_bool() == Some(true) {
        match v["last_id"].as_str() {
            Some(id) => Some(id.to_owned()),
            None => {
                return Err(ModelError(
                    "the model list says there is more but not where it continues".to_owned(),
                ));
            }
        }
    } else {
        None
    };
    Ok((ids, next))
}
