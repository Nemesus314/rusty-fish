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
    Ask, Line, Model, ModelError, Outcome, Prompt, Reply, Request, addressed_any, ask, translate,
};
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};

pub use rusty_fish_core::Gender;

pub mod tongue;

/// Ein Platz im Raum: ein Kanal, eine Sprache. Seit dem 20.11.2026 auch: der
/// Tisch eines Menschen.
///
/// Nemesus am 20.11.2026: „ob es nicht sinnvoller wäre, pro Discord-Nummer die
/// Muttersprache und die Neutralsprache festzulegen. Dann wäre die Aufgabe
/// [...]: Übersetze alles für eine Russin, halte dich von Englisch fern."
///
/// Diese Schicht kennt weiter keine Discord-Nummern. Die Tür trägt hier ein,
/// was am Tisch gilt: hat der Tisch einen Besitzer, sind das dessen
/// Muttersprache (`lang`), dessen unübersetzte Sprachen (`neutral`) und dessen
/// Anrede (`reader_gender`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seat {
    /// Kennung des Kanals, so wie die Tür sie kennt.
    pub channel: String,
    /// Sprache der Leser an diesem Platz, als Wort ("Russian").
    pub lang: String,
    /// Grammatisches Geschlecht der Person, die gewöhnlich an diesem Platz
    /// sitzt, freiwillig. Es gilt für das, was hier gelesen wird, und für das,
    /// was hier jemand schreibt, über den die Tür nichts Eigenes weiß (siehe
    /// [`Incoming::author_gender`]).
    ///
    /// Benannte Grenzen:
    /// - Sitzen an einem Platz mehrere Menschen verschiedenen Geschlechts,
    ///   bleibt die Angabe weg. Dann wählt die KI wie bisher selbst.
    /// - Der Leser ist immer „wer hier gewöhnlich sitzt". Wer wirklich
    ///   mitliest, weiß der Fisch nicht: schaut jemand am Platz des anderen
    ///   zu, liest er dort Sätze, deren Anrede dem anderen gilt.
    pub gender: Option<Gender>,
    /// Die Anrede des Menschen, dem dieser Tisch gehört, falls die Tür sie
    /// kennt. Sie geht für das, was hier gelesen wird, der Angabe des Platzes
    /// vor, siehe [`Seat::reader`].
    ///
    /// Ein eigenes Feld und nicht einfach in `gender` eingetragen, weil die
    /// Angabe des Platzes weiter für den gilt, der hier schreibt und über den
    /// die Tür nichts Eigenes weiß. Der Besitzer liest an seinem Tisch; wer
    /// dort schreibt, kann ein anderer sein.
    pub reader_gender: Option<Gender>,
    /// Sprachen, die an diesem Tisch unübersetzt durchgehen. `None` heißt: es
    /// gilt die Angabe des Raums. Eine leere Liste heißt: hier wird alles
    /// übersetzt. Nicht jeder in einer großen Runde liest Englisch.
    pub neutral: Option<Vec<String>>,
    /// Schreibt dieser Fisch an diesen Tisch?
    ///
    /// Nemesus am 20.11.2026: „Stelle dir vor, wir sind im Chat mit 10
    /// Menschen, die jeweils eine unterschiedliche Sprache sprechen. Jeder
    /// Rusty-Fish würde für den API-Eigner übersetzen. (Kostenaufteilung,
    /// ähnlich wie bei Pacioli.)"
    ///
    /// Jeder Fisch liest an allen Tischen mit, schreibt aber nur an die, die
    /// er bedient: den seines Gastgebers und die der Eingeladenen. So
    /// übersetzt jeder Fisch mit dem Schlüssel seines Besitzers, nichts
    /// erscheint doppelt, und die Fische müssen einander nicht kennen.
    pub served: bool,
}

impl Seat {
    /// Ein Tisch ohne Besonderheiten: keine Anrede, die unübersetzten Sprachen
    /// des Raums, und dieser Fisch bedient ihn.
    pub fn new(channel: &str, lang: &str) -> Seat {
        Seat {
            channel: channel.to_owned(),
            lang: lang.to_owned(),
            gender: None,
            reader_gender: None,
            neutral: None,
            served: true,
        }
    }

    /// Wie der Mensch angesprochen wird, der an diesem Tisch liest: mit der
    /// Anrede des Besitzers, sonst mit der des Platzes, sonst ohne Hinweis.
    pub fn reader(&self) -> Option<Gender> {
        self.reader_gender.or(self.gender)
    }
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
    /// Grammatisches Geschlecht des Schreibenden selbst, falls die Tür es für
    /// diese Person kennt. Es geht der Angabe des Platzes vor.
    ///
    /// Entschieden am 19.11.2026: beide Kanäle bleiben für beide offen.
    /// Nemesus: „Sie soll ruhig auch den Kanal #tee sehen können und sich
    /// reinschalten." Hinge das Geschlecht nur am Platz, bekäme, wer am Platz
    /// des anderen schreibt, dessen Formen.
    pub author_gender: Option<Gender>,
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
    let Some(origin) = room.seats.iter().find(|s| s.channel == msg.channel) else {
        return Vec::new();
    };

    // Ohne Zustimmung: zustellen, ehrlich markiert, und nichts merken. Was
    // nicht gemerkt wird, kann auch später nicht als Kontext zur KI gelangen.
    if !msg.consented {
        return room
            .seats
            .iter()
            // Auch das Original geht nur an Tische, die dieser Fisch bedient.
            .filter(|s| s.channel != msg.channel && s.served)
            .map(|s| Delivery {
                channel: s.channel.clone(),
                text: msg.text.to_owned(),
                mark: Mark::Untranslated(Reason::NoConsent),
                from: Sender::Writer,
            })
            .collect();
    }

    let context = state.lines();
    let piece = Piece {
        origin: msg.channel,
        speaker: msg.author,
        writer_gender: writer_gender(msg, origin),
        text: msg.text,
        from: Sender::Writer,
    };
    let out = spread(model, room, &context, &piece).await;
    drop(context);
    state.remember(msg.author, msg.text, room.context_lines);
    out
}

/// Das grammatische Geschlecht dessen, der schreibt: zuerst die Angabe zur
/// Person, sonst die des Platzes, an dem geschrieben wird, sonst keine.
fn writer_gender(msg: &Incoming<'_>, seat: &Seat) -> Option<Gender> {
    msg.author_gender.or(seat.gender)
}

/// Ein Text auf dem Weg an die anderen Plätze.
struct Piece<'a> {
    /// Der Kanal, an dem der Text entstand. Dorthin geht er nicht noch einmal.
    origin: &'a str,
    speaker: &'a str,
    /// Geschlecht des Schreibenden, falls es für ihn selbst oder für seinen
    /// Platz angegeben ist. Die KI als Schreibende hat keines.
    writer_gender: Option<Gender>,
    text: &'a str,
    from: Sender,
}

/// Bringt einen Text an alle Plätze außer dem, an dem er entstand, jeweils in
/// der Sprache des Platzes.
async fn spread<M: Model + Sync>(
    model: &M,
    room: &Room,
    context: &[Line<'_>],
    piece: &Piece<'_>,
) -> Vec<Delivery> {
    let text = piece.text;

    // Eine Sprache, eine bezahlte Übersetzung, auch wenn mehrere Plätze sie
    // lesen. Das gilt nur für Plätze, an denen dasselbe herauskäme: gleiche
    // Sprache, gleiche unübersetzte Sprachen und gleiche Anrede. „Hast du
    // geschlafen?" lautet im Russischen an eine Frau anders als an einen Mann,
    // und ein englischer Satz bleibt nur dort stehen, wo Englisch gelesen wird.
    type Kind = (String, Vec<String>, Option<Gender>);
    let mut done: Vec<(Kind, String, Mark)> = Vec::new();
    let mut out = Vec::new();
    // Nur an Tische, die dieser Fisch bedient. Für die anderen übersetzt der
    // Fisch ihres Besitzers, mit dessen Schlüssel.
    for seat in room.seats.iter().filter(|s| s.channel != piece.origin && s.served) {
        let lang = seat.lang.trim();
        // Die eigenen unübersetzten Sprachen des Tisches, sonst die des Raums.
        let untranslated = seat.neutral.as_ref().unwrap_or(&room.pass_through);
        let pass_through: Vec<&str> = untranslated.iter().map(String::as_str).collect();
        // Für den Vergleich zählt, welche Sprachen es sind, nicht wie und in
        // welcher Reihenfolge sie geschrieben stehen.
        let mut same: Vec<String> = untranslated
            .iter()
            .map(|l| l.trim().to_lowercase())
            .filter(|l| !l.is_empty())
            .collect();
        same.sort();
        same.dedup();
        let reader = seat.reader();
        let key: Kind = (lang.to_lowercase(), same, reader);
        let (translated, mark) = match done.iter().find(|(k, _, _)| *k == key) {
            Some((_, translated, mark)) => (translated.clone(), mark.clone()),
            None => {
                let request = Request {
                    speaker: piece.speaker,
                    text,
                    target_lang: lang,
                    pass_through: &pass_through,
                    context,
                    writer_gender: piece.writer_gender,
                    reader_gender: reader,
                    // Was Menschen einander schreiben, behält seine Anrede.
                    informal: false,
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
        out.push(Delivery {
            channel: seat.channel.clone(),
            text: translated,
            mark,
            from: piece.from,
        });
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
    /// Was der Gastgeber heute zusätzlich zum Tagesbetrag freigegeben hat.
    pub raised_tokens: u64,
    /// Ob der Gastgeber über die laufende Erschöpfung schon benachrichtigt ist.
    pub host_notified: bool,
}

impl Tally {
    /// Wie viele Tokens heute insgesamt erlaubt sind: der eingestellte
    /// Tagesbetrag plus das, was der Gastgeber heute nachgelegt hat.
    pub fn allowed_today(&self, limits: &Limits) -> u64 {
        limits.translation_tokens_per_day.saturating_add(self.raised_tokens)
    }

    /// Ist das heutige Kontingent verbraucht und gäbe es etwas nachzulegen?
    /// Bei einem Tagesbetrag von null will der Gastgeber keine Übersetzung;
    /// „um weitere 0 Tokens erhöhen" wäre Unsinn.
    fn used_up(&self, limits: &Limits) -> bool {
        limits.translation_tokens_per_day > 0 && self.tokens_left(limits) == 0
    }

    /// Legt genau einen Tagesbetrag nach, aber nur, wenn das Kontingent
    /// verbraucht ist. Liefert, ob erhöht wurde. So erhöht ein doppelter Klick
    /// des Gastgebers nicht doppelt.
    ///
    /// Nemesus am 19.11.2026: „Gleich mit Abfage, ob er es wieder um den selben
    /// Betrag erhöhen soll, mit Beitragnennung".
    pub fn raise(&mut self, limits: &Limits) -> bool {
        if !self.used_up(limits) {
            return false;
        }
        self.raised_tokens = self.raised_tokens.saturating_add(limits.translation_tokens_per_day);
        // Eine neue Erschöpfung soll wieder gemeldet werden.
        self.host_notified = false;
        true
    }

    /// Ist ein Hinweis an den Gastgeber fällig? Wahr genau einmal je
    /// Erschöpfung: der Aufruf merkt sich, dass benachrichtigt wurde.
    pub fn notice_due(&mut self, limits: &Limits) -> bool {
        if self.host_notified || !self.used_up(limits) {
            return false;
        }
        self.host_notified = true;
        true
    }

    /// Beginnt bei null, sobald das Datum wechselt. Das Datum kommt von außen,
    /// damit diese Schicht keine Uhr braucht.
    pub fn start_day(&mut self, today: &str) {
        if self.day != today {
            *self = Tally { day: today.to_owned(), ..Tally::default() };
        }
    }

    /// Wie viele Tokens heute noch zum Übersetzen frei sind.
    pub fn tokens_left(&self, limits: &Limits) -> u64 {
        self.allowed_today(limits).saturating_sub(self.translation_tokens)
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

/// Ging in diesen Zustellungen etwas unübersetzt hinaus, weil das Kontingent
/// leer war? Die Tür fragt das, um den Gastgeber zu benachrichtigen.
pub fn budget_ran_out(deliveries: &[Delivery]) -> bool {
    // Erkannt am Grund, den der Zähler selbst setzt. Ein Ausfall des Anbieters
    // trägt einen anderen Grund und zählt hier nicht.
    deliveries.iter().any(|d| match &d.mark {
        Mark::Untranslated(Reason::Failed(why)) => why.contains(BUDGET_USED_UP),
        _ => false,
    })
}

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

/// Die Absage, wenn ein Gast für heute keine Fragen mehr frei hat, auf
/// Englisch. Die deutsche und die russische Fassung stehen in [`tongue`];
/// gewählt wird nach der Sprache des Tisches, an dem gefragt wurde.
pub const NO_QUESTIONS_LEFT: &str =
    "I can't answer right now: your questions to me for today are used up.";

/// Der Anfang der Mitteilung, wenn die KI nicht antworten konnte, auf
/// Englisch. Die anderen Fassungen stehen in [`tongue`].
pub const NO_ANSWER: &str = "I could not answer";

/// Was der Gastgeber für den Raum festgelegt hat, plus das heutige Datum.
#[derive(Debug, Clone)]
pub struct House<'a> {
    pub limits: &'a Limits,
    /// Die Rufnamen der KI. Der erste ist der Name, unter dem sie erscheint
    /// und den sie im Auftrag trägt; jeder gilt als Ansprache. Ohne Rufnamen
    /// ist sie nicht ansprechbar.
    pub ai_names: &'a [&'a str],
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

    // Ob die KI angesprochen ist, entscheidet ein Rufname am Anfang, nicht die
    // KI. Jeder Rufname gilt; wer sie ist, sagt der erste.
    let name = house
        .ai_names
        .first()
        .copied()
        .filter(|_| addressed_any(msg.text, house.ai_names).is_some());
    // Ohne Zustimmung des Fragenden geht auch keine Frage an die KI.
    let name = name.filter(|_| msg.consented);
    // Wer an einem Tisch fragt, den dieser Fisch nicht bedient, könnte die
    // Antwort dort nie lesen: an solche Tische schreibt der Fisch nichts. Dann
    // gibt es keine Frage, keinen Aufruf und keinen Zähler. Die Nachricht
    // selbst geht wie jede andere an die bedienten Tische.
    let name = name.filter(|_| seat.served);
    // Was der Fisch selbst sagt, steht in der Sprache des Tisches, an dem es
    // erscheint.
    let tongue = tongue::tongue_of(&seat.lang);

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
            out.push(notice(tongue::no_questions_left(tongue).to_owned()));
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
        asker_gender: writer_gender(msg, seat),
    };
    let answer = match ask(model, &question).await {
        Ok(answer) => answer,
        Err(e) => {
            let why: String = e.to_string().chars().take(120).collect();
            // Der Grund bleibt, wie der Dienst ihn meldet.
            out.push(notice(format!("{}: {why}", tongue::no_answer(tongue))));
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
    let piece = Piece {
        origin: msg.channel,
        speaker: ASSISTANT,
        // Benannte Grenze: die KI hat kein eingestelltes Geschlecht. Spricht
        // sie in einer Sprache wie Russisch über sich selbst, wählt sie die Form.
        writer_gender: None,
        text: &answer,
        from: Sender::Assistant,
    };
    out.extend(spread(model, room, &context, &piece).await);
    drop(context);
    state.remember(ASSISTANT, &answer, room.context_lines);
    out
}
