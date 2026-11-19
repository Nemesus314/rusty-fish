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
//!
//! Dazu kommen das Kontingent und die ansprechbare KI, siehe [`converse`].

use rusty_fish_core::{
    Ask, Line, Model, ModelError, Outcome, Prompt, Reply, Request, addressed, ask, translate,
};
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};

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
    /// Eine feste Mitteilung des Fischs, etwa eine Absage. Kein Gesprächsbeitrag.
    Notice,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// Der Schreibende hat der KI noch nicht zugestimmt.
    NoConsent,
    /// Der Fisch hat gehustet. Der Grund steht dabei.
    Failed(String),
}

/// Von wem eine Zustellung stammt. Die Tür setzt sie unter dem passenden
/// Namen ab: dem des Menschen oder dem der KI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sender {
    /// Der Mensch, der die Nachricht geschrieben hat.
    Writer,
    /// Die KI, unter dem Namen, den der Gastgeber ihr gegeben hat.
    Assistant,
}

/// Was an einem Platz erscheinen soll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    pub channel: String,
    pub text: String,
    pub mark: Mark,
    pub from: Sender,
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

    // Ohne Zustimmung: zustellen, ehrlich markiert, und nichts merken. Was
    // nicht gemerkt wird, kann auch später nicht als Kontext zur KI gelangen.
    if !msg.consented {
        return room
            .seats
            .iter()
            .filter(|s| s.channel != msg.channel)
            .map(|s| Delivery {
                channel: s.channel.clone(),
                text: msg.text.to_owned(),
                mark: Mark::Untranslated(Reason::NoConsent),
                from: Sender::Writer,
            })
            .collect();
    }

    let context = state.lines();
    let out = spread(model, room, &context, msg.channel, msg.author, msg.text, Sender::Writer).await;
    drop(context);
    state.remember(msg.author, msg.text, room.context_lines);
    out
}

/// Bringt einen Text an alle Plätze außer dem, an dem er entstand, jeweils in
/// der Sprache des Platzes.
async fn spread<M: Model + Sync>(
    model: &M,
    room: &Room,
    context: &[Line<'_>],
    origin: &str,
    speaker: &str,
    text: &str,
    from: Sender,
) -> Vec<Delivery> {
    let pass_through: Vec<&str> = room.pass_through.iter().map(String::as_str).collect();

    // Eine Sprache, eine bezahlte Übersetzung, auch wenn mehrere Plätze sie lesen.
    let mut done: Vec<(String, String, Mark)> = Vec::new();
    let mut out = Vec::new();
    for seat in room.seats.iter().filter(|s| s.channel != origin) {
        let lang = seat.lang.trim();
        let key = lang.to_lowercase();
        let (translated, mark) = match done.iter().find(|(k, _, _)| *k == key) {
            Some((_, translated, mark)) => (translated.clone(), mark.clone()),
            None => {
                let request = Request {
                    speaker,
                    text,
                    target_lang: lang,
                    pass_through: &pass_through,
                    context,
                };
                let (translated, mark) = match translate(model, &request).await {
                    Ok(Outcome::Translated(t)) => (t, Mark::Translated),
                    Ok(Outcome::Skip) => (text.to_owned(), Mark::Original),
                    // Der Fisch hustet: das Original geht hinaus, mit dem Grund.
                    Err(e) => (text.to_owned(), Mark::Untranslated(Reason::Failed(e.to_string()))),
                };
                done.push((key, translated.clone(), mark.clone()));
                (translated, mark)
            }
        };
        out.push(Delivery { channel: seat.channel.clone(), text: translated, mark, from });
    }
    out
}

impl RoomState {
    fn lines(&self) -> Vec<Line<'_>> {
        self.recent.iter().map(|(speaker, text)| Line { speaker, text }).collect()
    }

    fn remember(&mut self, author: &str, text: &str, limit: usize) {
        self.recent.push_back((author.to_owned(), text.to_owned()));
        while self.recent.len() > limit {
            self.recent.pop_front();
        }
    }
}

// ---------------------------------------------------------------------------
// Das Kontingent
//
// Nemesus am 18.11.2026: „Wichtig wäre hier ein Limit, bestimmen zu können.
// Sagen wir ich bin Gastgeber... Nicht, dass jemand dauernd Fragen an die KI
// stellt, die dann über meine API laufen."
//
// Zwei getrennte Töpfe pro Tag: Tokens für das Übersetzen der Nachrichten von
// Menschen, und Fragen je Gast. Getrennt, damit Fragen nie das Übersetzen leer
// machen: das Gespräch selbst geht vor.
// ---------------------------------------------------------------------------

/// Was der Gastgeber pro Tag erlaubt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Tokens für das Übersetzen der Nachrichten von Menschen, für alle zusammen.
    pub translation_tokens_per_day: u64,
    /// Fragen an die KI je Gast. Der Gastgeber selbst hat keine Grenze.
    pub questions_per_guest_per_day: u32,
}

impl Default for Limits {
    /// Vorsichtige Vorgaben. 100 000 Tokens reichen grob für einige hundert
    /// Nachrichten mit Kontext; zehn Fragen je Gast halten einen Dauerfrager
    /// auf. Beides sind Schätzungen, keine Messungen.
    fn default() -> Self {
        Limits { translation_tokens_per_day: 100_000, questions_per_guest_per_day: 10 }
    }
}

/// Der Zähler eines Tages. Die Tür legt ihn in ihrer Zustandsdatei ab, damit
/// er einen Neustart überlebt. Er enthält Kennungen und Zahlen, keinen Text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tally {
    /// Der Tag, für den gezählt wird, als UTC-Datum ("2026-11-19").
    pub day: String,
    pub translation_tokens: u64,
    pub questions: BTreeMap<String, u32>,
}

impl Tally {
    /// Beginnt bei null, sobald das Datum wechselt. Das Datum kommt von außen,
    /// damit diese Schicht keine Uhr braucht.
    pub fn start_day(&mut self, today: &str) {
        if self.day != today {
            *self = Tally { day: today.to_owned(), ..Tally::default() };
        }
    }

    /// Wie viele Tokens heute noch zum Übersetzen frei sind.
    pub fn tokens_left(&self, limits: &Limits) -> u64 {
        limits.translation_tokens_per_day.saturating_sub(self.translation_tokens)
    }

    pub fn spend(&mut self, tokens: u64) {
        self.translation_tokens = self.translation_tokens.saturating_add(tokens);
    }

    /// Darf dieser Gast heute noch fragen?
    pub fn may_ask(&self, guest: &str, limits: &Limits) -> bool {
        self.questions.get(guest).copied().unwrap_or(0) < limits.questions_per_guest_per_day
    }

    pub fn asked(&mut self, guest: &str) {
        let count = self.questions.entry(guest.to_owned()).or_insert(0);
        *count = count.saturating_add(1);
    }
}

/// Grobe Schätzung, wenn der Anbieter keinen Verbrauch meldet: drei Zeichen je Token.
fn estimate(chars: usize) -> u64 {
    (chars as u64).div_ceil(3)
}

/// Was die KI meldet, wenn der Übersetzungstopf leer ist. Es erscheint als
/// Grund an der unübersetzt zugestellten Nachricht.
pub const BUDGET_USED_UP: &str = "today's translation budget is used up";

/// Zählt an der Grenze zur KI, weil dort jeder Aufruf vorbeikommt, auch der
/// gescheiterte. Ist nichts mehr frei, findet kein Aufruf statt.
///
/// Benannte Grenzen:
/// - Die Grenze ist weich: geprüft wird vor dem Aufruf, gezählt danach. Der
///   letzte Aufruf des Tages kann den Topf überziehen.
/// - Meldet der Anbieter keinen Verbrauch, wird aus der Zeichenzahl geschätzt
///   (drei Zeichen je Token). Das ist grob und je nach Schrift verschieden.
/// - Ein gescheiterter Aufruf zählt mit der Schätzung seines Auftrags, weil
///   hier niemand weiß, ob der Anbieter ihn berechnet hat.
pub struct Metered<'a, M> {
    model: &'a M,
    left: u64,
    used: AtomicU64,
}

impl<'a, M> Metered<'a, M> {
    pub fn new(model: &'a M, left: u64) -> Self {
        Metered { model, left, used: AtomicU64::new(0) }
    }

    /// Was über diesen Zähler verbraucht wurde.
    pub fn used(&self) -> u64 {
        self.used.load(Ordering::Relaxed)
    }
}

impl<M: Model + Sync> Model for Metered<'_, M> {
    async fn complete(&self, prompt: &Prompt) -> Result<Reply, ModelError> {
        if self.used() >= self.left {
            return Err(ModelError(BUDGET_USED_UP.to_owned()));
        }
        let asked = estimate(prompt.system.chars().count() + prompt.user.chars().count());
        let result = self.model.complete(prompt).await;
        let spent = match &result {
            Ok(reply) => {
                reply.tokens.unwrap_or_else(|| asked + estimate(reply.text.chars().count()))
            }
            Err(_) => asked,
        };
        self.used.fetch_add(spent, Ordering::Relaxed);
        result
    }
}

// ---------------------------------------------------------------------------
// Das Gespräch mit Kontingent und ansprechbarer KI
// ---------------------------------------------------------------------------

/// Die Kennung, unter der die KI als Sprecherin im Raum steht. Sie kann mit
/// keiner Kennung eines Menschen zusammenfallen.
pub const ASSISTANT: &str = "rusty-fish:assistant";

/// Die Absage, wenn ein Gast für heute keine Fragen mehr frei hat.
pub const NO_QUESTIONS_LEFT: &str =
    "I can't answer right now: your questions to me for today are used up.";

/// Der Anfang der Mitteilung, wenn die KI nicht antworten konnte.
pub const NO_ANSWER: &str = "I could not answer";

/// Was der Gastgeber für den Raum festgelegt hat, plus das heutige Datum.
#[derive(Debug, Clone)]
pub struct House<'a> {
    pub limits: &'a Limits,
    /// Der Name der KI. Fehlt er, ist sie nicht ansprechbar.
    pub ai_name: Option<&'a str>,
    /// UTC-Datum, etwa "2026-11-19".
    pub today: &'a str,
}

/// Eine Nachricht im Raum, mit allem, was dazugehört:
///
/// 1. Sie wird an die anderen Plätze verteilt. Das zählt gegen den
///    Übersetzungstopf; ist er leer, geht sie unübersetzt und markiert hinaus.
/// 2. Spricht sie die KI mit Namen an, antwortet die KI einmal, in der Sprache
///    des Platzes, an dem gefragt wurde. Die Antwort erscheint an jedem Platz
///    in dessen Sprache und wird wie jede Nachricht als Kontext gemerkt.
///
/// Die Frage und das Übersetzen ihrer Antwort gehen nicht über den
/// Übersetzungstopf. Sie sind durch die Zahl der Fragen je Gast begrenzt.
pub async fn converse<M: Model + Sync>(
    model: &M,
    room: &Room,
    state: &mut RoomState,
    tally: &mut Tally,
    house: &House<'_>,
    msg: &Incoming<'_>,
    is_host: bool,
) -> Vec<Delivery> {
    tally.start_day(house.today);
    let Some(seat) = room.seats.iter().find(|s| s.channel == msg.channel) else {
        return Vec::new();
    };

    // Ob die KI angesprochen ist, entscheidet der Name am Anfang, nicht die KI.
    let name = house.ai_name.filter(|name| addressed(msg.text, name).is_some());
    // Ohne Zustimmung des Fragenden geht auch keine Frage an die KI.
    let name = name.filter(|_| msg.consented);

    // Der Kontext der Frage ist das Gespräch davor. Gleich wird die Frage
    // selbst gemerkt; sie soll nicht doppelt im Auftrag stehen.
    let before: Vec<(String, String)> = match name {
        Some(_) => state.recent.iter().cloned().collect(),
        None => Vec::new(),
    };

    // 1. Die Nachricht selbst, über den Übersetzungstopf.
    let meter = Metered::new(model, tally.tokens_left(house.limits));
    let mut out = relay(&meter, room, state, msg).await;
    tally.spend(meter.used());

    // 2. Die Antwort der KI, falls sie angesprochen wurde.
    let Some(name) = name else { return out };
    let notice = |text: String| Delivery {
        channel: msg.channel.to_owned(),
        text,
        mark: Mark::Notice,
        from: Sender::Assistant,
    };

    if !is_host {
        if !tally.may_ask(msg.author, house.limits) {
            out.push(notice(NO_QUESTIONS_LEFT.to_owned()));
            return out;
        }
        // Gezählt wird vor dem Aufruf: auch ein gescheiterter Versuch kann
        // gekostet haben.
        tally.asked(msg.author);
    }

    let context: Vec<Line<'_>> =
        before.iter().map(|(speaker, text)| Line { speaker, text }).collect();
    let question = Ask {
        name,
        own_speaker: ASSISTANT,
        speaker: msg.author,
        text: msg.text,
        answer_lang: &seat.lang,
        context: &context,
    };
    let answer = match ask(model, &question).await {
        Ok(answer) => answer,
        Err(e) => {
            let why: String = e.to_string().chars().take(120).collect();
            out.push(notice(format!("{NO_ANSWER}: {why}")));
            return out;
        }
    };

    // Am Platz des Fragenden steht die Antwort, wie die KI sie gab. An die
    // anderen Plätze geht sie wie die Nachricht eines Menschen, in deren
    // Sprache. Das läuft nicht über den Übersetzungstopf: Fragen dürfen ihn
    // nie leeren. Begrenzt ist es durch die Zahl der Fragen.
    out.push(Delivery {
        channel: msg.channel.to_owned(),
        text: answer.clone(),
        mark: Mark::Original,
        from: Sender::Assistant,
    });
    let context = state.lines();
    out.extend(
        spread(model, room, &context, msg.channel, ASSISTANT, &answer, Sender::Assistant).await,
    );
    drop(context);
    state.remember(ASSISTANT, &answer, room.context_lines);
    out
}
