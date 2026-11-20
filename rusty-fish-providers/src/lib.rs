//! Die KI-Dienste, mit denen Rusty Fish sprechen kann: eine Liste, und der Weg
//! zu jedem von ihnen.
//!
//! Entschieden von Nemesus am 20.11.2026: „Wir lassen Rusty Fish, wie gedacht.
//! Die Möglichkeit, die KIs im Profil zu wechseln sollte jedoch erhalten
//! bleiben." Gemeint sind ChatGPT, Grok, Gemini, Claude und ein selbst
//! gehostetes Modell, wie im WordPress-Plugin Pacioli.
//!
//! Diese Liste ist die eine Wahrheit. Die Einstellungen prüfen gegen sie, die
//! Ablage benennt ihre Einträge nach ihr, und eine Oberfläche zeigt sie nur an
//! (`rusty-fish providers --json`). Wer einen Dienst hinzufügt, tut es hier.

use rusty_fish_anthropic::{ANTHROPIC_BASE_URL, Anthropic};
use rusty_fish_core::{Model, ModelError, Prompt, Reply, Secret};
use rusty_fish_openai::{OPENAI_BASE_URL, OpenAi};
use serde_json::{Value, json};

/// Adresse von xAI, aus dessen Dokumentation (gelesen am 20.11.2026).
pub const XAI_BASE_URL: &str = "https://api.x.ai/v1";

/// Geminis Adresse für das OpenAI-Format, aus Googles Dokumentation (gelesen
/// am 20.11.2026). Darunter liegen `/chat/completions` und `/models`.
pub const GOOGLE_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/openai";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    OpenAi,
    Xai,
    Google,
    Anthropic,
    /// Ein selbst gehostetes Modell oder ein anderer Dienst im OpenAI-Format.
    /// Die Adresse steht im Profil; ein Schlüssel ist freiwillig.
    Custom,
}

impl Provider {
    pub const ALL: [Provider; 5] =
        [Provider::OpenAi, Provider::Xai, Provider::Google, Provider::Anthropic, Provider::Custom];

    /// Die Kennung: so steht der Dienst im Profil, und so heißt sein Eintrag
    /// in der Schlüsselablage.
    pub fn id(self) -> &'static str {
        match self {
            Provider::OpenAi => "openai",
            Provider::Xai => "xai",
            Provider::Google => "google",
            Provider::Anthropic => "anthropic",
            Provider::Custom => "custom",
        }
    }

    /// Der Name, den ein Mensch wiedererkennt.
    pub fn label(self) -> &'static str {
        match self {
            Provider::OpenAi => "ChatGPT (OpenAI)",
            Provider::Xai => "Grok (xAI)",
            Provider::Google => "Gemini (Google)",
            Provider::Anthropic => "Claude (Anthropic)",
            Provider::Custom => "Own model or other service",
        }
    }

    /// Die Adresse des Dienstes. `None` bei „custom": dort nennt sie das Profil.
    pub fn base_url(self) -> Option<&'static str> {
        match self {
            Provider::OpenAi => Some(OPENAI_BASE_URL),
            Provider::Xai => Some(XAI_BASE_URL),
            Provider::Google => Some(GOOGLE_BASE_URL),
            Provider::Anthropic => Some(ANTHROPIC_BASE_URL),
            Provider::Custom => None,
        }
    }

    /// Ob der Dienst ohne Schlüssel gar nicht antwortet.
    pub fn needs_key(self) -> bool {
        self != Provider::Custom
    }

    /// Liest eine Kennung. Unbekanntes wird laut abgelehnt und nennt, was es gibt.
    pub fn parse(id: &str) -> Result<Provider, String> {
        let wanted = id.trim().to_ascii_lowercase();
        Provider::ALL.into_iter().find(|p| p.id() == wanted).ok_or_else(|| {
            let known: Vec<&str> = Provider::ALL.iter().map(|p| p.id()).collect();
            format!("unknown AI service {id:?}; known are {}", known.join(", "))
        })
    }
}

/// Die Liste, wie `rusty-fish providers --json` sie ausgibt.
pub fn catalog() -> Value {
    Provider::ALL
        .iter()
        .map(|p| {
            json!({
                "id": p.id(),
                "label": p.label(),
                "base_url": p.base_url(),
                "needs_key": p.needs_key(),
            })
        })
        .collect()
}

/// Die Adresse, die wirklich benutzt wird: die aus dem Profil, sonst die des
/// Dienstes. Bei „custom" muss das Profil eine nennen.
pub fn address(provider: Provider, base_url: Option<&str>) -> Result<String, String> {
    let own = base_url.map(str::trim).filter(|url| !url.is_empty());
    match own.or(provider.base_url()) {
        Some(url) => Ok(url.to_owned()),
        None => Err(format!(
            "{} needs `base_url`, the address of the service",
            provider.label()
        )),
    }
}

/// Ein verbundener Dienst, gleich welcher. Die Tür und der Kern sehen nur
/// [`Model`].
#[derive(Debug, Clone)]
pub enum AnyModel {
    OpenAi(OpenAi),
    Anthropic(Anthropic),
}

/// Stellt den Anschluss für einen Dienst her. Kein Netz: verbunden wird erst
/// beim ersten Auftrag.
pub fn connect(
    provider: Provider,
    base_url: Option<&str>,
    model: &str,
    key: Option<Secret>,
) -> Result<AnyModel, String> {
    let address = address(provider, base_url)?;
    let key = match (key, provider.needs_key()) {
        (None, true) => {
            return Err(format!("{} needs an API key", provider.label()));
        }
        (key, _) => key,
    };
    let connected = match provider {
        // xAI nennt die Responses-Schnittstelle als seine Hauptschnittstelle.
        Provider::Xai => OpenAi::responses(&address, model, key).map(AnyModel::OpenAi),
        Provider::OpenAi | Provider::Google | Provider::Custom => {
            OpenAi::new(&address, model, key).map(AnyModel::OpenAi)
        }
        Provider::Anthropic => match key {
            Some(key) => Anthropic::new(&address, model, key).map(AnyModel::Anthropic),
            None => return Err(format!("{} needs an API key", provider.label())),
        },
    };
    connected.map_err(|e| e.to_string())
}

impl AnyModel {
    /// Welche Modelle dieser Schlüssel bei diesem Dienst sieht.
    pub async fn models(&self) -> Result<Vec<String>, ModelError> {
        match self {
            AnyModel::OpenAi(service) => service.models().await,
            AnyModel::Anthropic(service) => service.models().await,
        }
    }
}

impl Model for AnyModel {
    async fn complete(&self, prompt: &Prompt) -> Result<Reply, ModelError> {
        match self {
            AnyModel::OpenAi(service) => service.complete(prompt).await,
            AnyModel::Anthropic(service) => service.complete(prompt).await,
        }
    }
}
