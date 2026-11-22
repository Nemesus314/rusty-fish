//! Die Zeichen unter einer Nachricht, die bei Matrix die Knöpfe ersetzen.
//! Rein, ohne Netz.
//!
//! Matrix kennt keine Knöpfe. Es kennt Reaktionen: kleine Zeichen unter einer
//! Nachricht, die jeder mit einem Tipp setzen kann, und die nicht nur ein
//! Emoji, sondern auch ein kurzes Wort sein dürfen. Der Fisch setzt unter
//! seine Bitte selbst das Zeichen „OK"; wer zustimmt, tippt es an. Unter dem
//! Hinweis zum leeren Kontingent stehen zwei Zeichen mit denselben Worten wie
//! die Knöpfe bei Discord. So stimmt der Wortlaut der Bitte („Drücke OK") an
//! beiden Türen.
//!
//! Damit der Fisch eine Reaktion auch nach einem Neustart zuordnen kann,
//! steht in seiner eigenen Nachricht ein zusätzliches Feld, das sagt, was sie
//! fragt. Kommt eine Reaktion, holt er die Nachricht, auf die sie zeigt, und
//! liest das Feld. In einem verschlüsselten Raum ist es mit verschlüsselt.

use serde_json::{Value, json};

/// Das Zeichen unter der Bitte um Zustimmung. „OK" statt eines Satzes: ein
/// Satz wie „Ich stimme zu" hat in manchen Sprachen ein Geschlecht, und der
/// Fisch weiß nicht, wer drückt.
pub const CONSENT_KEY: &str = "OK";

/// Das Feld in einer Nachricht des Fischs, das sagt, was sie fragt.
pub const FIELD: &str = "rusty_fish";

/// Was eine Nachricht des Fischs fragt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// Die Bitte um Zustimmung. Sie steht in einem Raum, und der gehört zu
    /// einem Profil. Sie merkt sich den KI-Dienst, den sie genannt hat: wer
    /// unter einer alten Bitte zustimmt, nachdem der Gastgeber den Dienst
    /// gewechselt hat, hat dem neuen nicht zugestimmt.
    Consent { service: String },
    /// Der Hinweis an den Gastgeber, dass das Kontingent verbraucht ist. Er
    /// steht in einem Raum nur mit dem Gastgeber; deshalb nennt er das Profil
    /// und die Worte seiner zwei Antworten.
    Budget { profile: Option<String>, raise: String, keep: String },
}

/// Was der Gastgeber am Hinweis zum leeren Kontingent wählen kann.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Budget {
    /// Um einen Tagesbetrag erhöhen.
    Raise,
    /// Es dabei belassen.
    Keep,
}

impl Ask {
    /// Das Feld, wie es in der Nachricht steht.
    pub fn to_field(&self) -> Value {
        match self {
            Ask::Consent { service } => json!({ "ask": "consent", "service": service }),
            Ask::Budget { profile, raise, keep } => {
                json!({ "ask": "budget", "profile": profile, "raise": raise, "keep": keep })
            }
        }
    }

    /// Liest das Feld aus dem Inhalt einer Nachricht. `None`, wenn sie nichts
    /// fragt oder das Feld nicht zu lesen ist.
    pub fn from_content(content: &Value) -> Option<Ask> {
        let field = content.get(FIELD)?;
        match field.get("ask")?.as_str()? {
            "consent" => {
                Some(Ask::Consent { service: field.get("service")?.as_str()?.to_owned() })
            }
            "budget" => Some(Ask::Budget {
                profile: field.get("profile").and_then(Value::as_str).map(str::to_owned),
                raise: field.get("raise")?.as_str()?.to_owned(),
                keep: field.get("keep")?.as_str()?.to_owned(),
            }),
            _ => None,
        }
    }

    /// Die Zeichen, die der Fisch selbst unter diese Nachricht setzt, damit
    /// ein Tipp genügt.
    pub fn keys(&self) -> Vec<&str> {
        match self {
            Ask::Consent { .. } => vec![CONSENT_KEY],
            Ask::Budget { raise, keep, .. } => vec![raise, keep],
        }
    }

    /// Stimmt dieses Zeichen der Bitte zu, und zwar dem KI-Dienst, der jetzt
    /// übersetzt?
    pub fn agrees(&self, key: &str, service_now: &str) -> bool {
        matches!(self, Ask::Consent { service } if service == service_now)
            && key.trim() == CONSENT_KEY
    }

    /// Welche der zwei Antworten dieses Zeichen am Hinweis zum Kontingent ist.
    pub fn budget_choice(&self, key: &str) -> Option<Budget> {
        let Ask::Budget { raise, keep, .. } = self else { return None };
        let key = key.trim();
        if key == raise {
            Some(Budget::Raise)
        } else if key == keep {
            Some(Budget::Keep)
        } else {
            None
        }
    }
}

/// Der Inhalt einer Nachricht des Fischs, wie er hinausgeht: eine Notiz mit
/// reinem Text und HTML, die niemanden erwähnt, und wenn sie etwas fragt, mit
/// dem Feld dazu.
pub fn notice(body: &str, html: &str, ask: Option<&Ask>) -> Value {
    let mut content = json!({
        "msgtype": "m.notice",
        "body": body,
        "format": "org.matrix.custom.html",
        "formatted_body": html,
        // Eine gespiegelte Nachricht darf niemanden ein zweites Mal rufen.
        "m.mentions": {},
    });
    if let Some(ask) = ask {
        content[FIELD] = ask.to_field();
    }
    content
}

/// Der Inhalt einer Reaktion: dieses Zeichen unter dieser Nachricht.
pub fn reaction(event_id: &str, key: &str) -> Value {
    json!({
        "m.relates_to": { "rel_type": "m.annotation", "event_id": event_id, "key": key }
    })
}
