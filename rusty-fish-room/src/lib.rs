//! Rusty Fish, die Raum-Logik.
//!
//! Ein Raum besteht aus Plätzen. Jeder Platz ist ein Kanal mit einer Sprache.
//! Was an einem Platz geschrieben wird, erscheint an allen anderen Plätzen in
//! deren Sprache. So sieht jeder nur seine eigene Sprache, obwohl alle im
//! selben Gespräch sind.
//!
//! Diese Schicht weiß nichts von Discord oder Matrix. Die Türen bleiben dünn:
//! sie reichen Nachrichten herein und stellen zu, was hier herauskommt.
//! Dieselbe Logik trägt deshalb später auch die Matrix-Tür.
//!
//! Zwei Regeln sind hier festgemacht:
//!
//! - Aus Bismarck: „Der Fisch darf husten, aber nie das Gespräch verschlucken
//!   — Zustellung immer, ehrlich markiert." Jede Nachricht kommt an jedem
//!   anderen Platz an, notfalls im Original und mit Grund.
//! - Ohne Zustimmung des Schreibenden geht kein Wort von ihm an eine KI.
//!   Nemesus am 18.11.2026: „Dass wir das natürlich den | die anderen Menschen
//!   wissen lassen, dass eine KI mitliest, ist natürlich Ehrensache."

use rusty_fish_core::{Line, Model, Outcome, Request, translate};
use std::collections::VecDeque;

/// Ein Platz im Raum: ein Kanal, eine Sprache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seat {
    /// Kennung des Kanals, so wie die Tür sie kennt.
    pub channel: String,
    /// Sprache der Leser an diesem Platz, als Wort ("Russian").
    pub lang: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Room {
    pub seats: Vec<Seat>,
    /// Sprachen, die unübersetzt durchgehen.
    pub pass_through: Vec<String>,
    /// Wie viele frühere Nachrichten die KI als Kontext sieht. Jede Zeile
    /// kostet bei jeder Übersetzung mit, deshalb eine Einstellung.
    pub context_lines: usize,
}

/// Was der Raum sich merkt: die letzten Nachrichten, im Original.
#[derive(Debug, Clone, Default)]
pub struct RoomState {
    recent: VecDeque<(String, String)>,
}

/// Eine Nachricht, die an einem Platz geschrieben wurde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incoming<'a> {
    pub channel: &'a str,
    /// Kennung des Schreibenden. Sie erreicht die KI nie.
    pub author: &'a str,
    pub text: &'a str,
    /// Hat der Schreibende zugestimmt, dass eine KI seine Sätze liest?
    pub consented: bool,
}

/// Wie eine Zustellung zu kennzeichnen ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mark {
    /// Der Text ist eine Übersetzung.
    Translated,
    /// Der Text ist das Original, und das ist richtig so: er steht schon in
    /// der Sprache des Platzes oder in einer unübersetzten Sprache.
    Original,
    /// Der Text ist das Original, obwohl er übersetzt werden sollte.
    Untranslated(Reason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// Der Schreibende hat der KI noch nicht zugestimmt.
    NoConsent,
    /// Der Fisch hat gehustet. Der Grund steht dabei.
    Failed(String),
}

/// Was an einem anderen Platz erscheinen soll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    pub channel: String,
    pub text: String,
    pub mark: Mark,
}

/// Verteilt eine Nachricht an alle anderen Plätze des Raums und merkt sie
/// sich danach als Kontext für die folgenden.
pub async fn relay<M: Model + Sync>(
    model: &M,
    room: &Room,
    state: &mut RoomState,
    msg: &Incoming<'_>,
) -> Vec<Delivery> {
    if !room.seats.iter().any(|s| s.channel == msg.channel) {
        return Vec::new();
    }
    let others = room.seats.iter().filter(|s| s.channel != msg.channel);

    // Ohne Zustimmung: zustellen, ehrlich markiert, und nichts merken. Was
    // nicht gemerkt wird, kann auch später nicht als Kontext zur KI gelangen.
    if !msg.consented {
        return others
            .map(|s| Delivery {
                channel: s.channel.clone(),
                text: msg.text.to_owned(),
                mark: Mark::Untranslated(Reason::NoConsent),
            })
            .collect();
    }

    let context: Vec<Line<'_>> =
        state.recent.iter().map(|(speaker, text)| Line { speaker, text }).collect();
    let pass_through: Vec<&str> = room.pass_through.iter().map(String::as_str).collect();

    // Eine Sprache, eine bezahlte Übersetzung, auch wenn mehrere Plätze sie lesen.
    let mut done: Vec<(String, String, Mark)> = Vec::new();
    let mut out = Vec::new();
    for seat in others {
        let lang = seat.lang.trim();
        let key = lang.to_lowercase();
        let (text, mark) = match done.iter().find(|(k, _, _)| *k == key) {
            Some((_, text, mark)) => (text.clone(), mark.clone()),
            None => {
                let request = Request {
                    speaker: msg.author,
                    text: msg.text,
                    target_lang: lang,
                    pass_through: &pass_through,
                    context: &context,
                };
                let (text, mark) = match translate(model, &request).await {
                    Ok(Outcome::Translated(t)) => (t, Mark::Translated),
                    Ok(Outcome::Skip) => (msg.text.to_owned(), Mark::Original),
                    // Der Fisch hustet: das Original geht hinaus, mit dem Grund.
                    Err(e) => {
                        (msg.text.to_owned(), Mark::Untranslated(Reason::Failed(e.to_string())))
                    }
                };
                done.push((key, text.clone(), mark.clone()));
                (text, mark)
            }
        };
        out.push(Delivery { channel: seat.channel.clone(), text, mark });
    }

    drop(context);
    state.remember(msg.author, msg.text, room.context_lines);
    out
}

impl RoomState {
    fn remember(&mut self, author: &str, text: &str, limit: usize) {
        self.recent.push_back((author.to_owned(), text.to_owned()));
        while self.recent.len() > limit {
            self.recent.pop_front();
        }
    }
}
