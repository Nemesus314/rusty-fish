//! Der Haushalt im Lauf: was sich ändert, während der Fisch läuft, und der
//! Ablauf einer Nachricht. Rein, ohne Netz.
//!
//! Die Arbeit ist geteilt. Der Haushalt entscheidet, was zu geschehen hat:
//! was an welchem Tisch erscheint, ob um Zustimmung zu bitten ist, ob der
//! Gastgeber einen Hinweis bekommt. Die Tür setzt es ab und sagt danach
//! `save`. So steht der Ablauf einmal da und läuft in Tests ohne Netz.
//!
//! Die Reihenfolge trägt eine Regel aus Bismarck: „Der Fisch darf husten,
//! aber nie das Gespräch verschlucken". Gespeichert wird erst nach dem
//! Absetzen. Stürzt der Fisch dazwischen ab, gilt die Nachricht als nicht
//! gelesen, und der nächste Start holt sie nach.

use crate::config::Config;
use crate::invite::{Due, due};
use crate::state::Store;
use crate::texts::{
    KEEP_LABEL, budget_notice, consent_text, consent_text_in, nothing_to_raise, raise_label,
    raised_notice,
};
use rusty_fish_core::{Model, Outcome, Request, translate};
use rusty_fish_room::tongue::known_tongue;
use rusty_fish_room::{
    Delivery, House, Incoming, Metered, RoomState, Tally, budget_ran_out, converse,
};
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

/// Was für einen Aufruf feststeht: das Profil, der Name des KI-Dienstes und
/// das heutige Datum.
///
/// Das Profil gehört nicht dem Haushalt, sondern kommt mit jedem Aufruf. So
/// kann eine Tür es lesen, ohne auf den Haushalt zu warten, während der
/// gerade eine Nachricht übersetzen lässt. Es muss bei jedem Aufruf dasselbe
/// sein wie bei [`Household::open`].
#[derive(Debug, Clone, Copy)]
pub struct Terms<'a> {
    pub config: &'a Config,
    /// Der Name des KI-Dienstes, wie er in der Bitte um Zustimmung steht.
    pub service: &'a str,
    /// UTC-Datum, etwa "2026-11-21". Von außen, damit hier keine Uhr läuft.
    pub today: &'a str,
}

/// Eine Nachricht, wie eine Tür sie hereinreicht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrived<'a> {
    /// Kennung des Kanals, so wie sie im Profil steht.
    pub channel: &'a str,
    /// Kennung des Schreibenden, so wie sie im Profil steht.
    pub author: &'a str,
    /// Der Text. Eine Ansprache, die nur diese Tür kennt (etwa eine
    /// Erwähnung), hat sie schon in den Rufnamen übersetzt.
    pub text: &'a str,
    /// Die laufende Nummer der Nachricht an dieser Tür. Eine Nachricht mit
    /// einer Nummer bis zur Lesemarke ihres Kanals gilt als schon verarbeitet.
    pub mark: u64,
}

/// Was die Tür nach einer Nachricht abzusetzen hat, in dieser Reihenfolge.
/// Danach sagt sie [`Household::save`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "what the household decided has to be delivered, then saved"]
pub struct Steps {
    /// Was an welchem Tisch erscheint.
    pub deliveries: Vec<Delivery>,
    /// Die Bitte um Zustimmung, in der Sprache des Tisches, an dem geschrieben
    /// wurde. Dort setzt die Tür sie ab, mit einer Möglichkeit zuzustimmen.
    pub ask_consent: Option<String>,
    /// Der Hinweis an den Gastgeber, und nur an ihn, dass das heutige
    /// Kontingent verbraucht ist.
    pub tell_host: Option<HostNotice>,
}

/// Der Hinweis an den Gastgeber samt den Beschriftungen seiner zwei Antworten.
///
/// Nemesus am 19.11.2026: „Meine Idee wäre eine Systemnachricht, der nur an
/// den Gastgeber geht. Gleich mit Abfage, ob er es wieder um den selben
/// Betrag erhöhen soll, mit Beitragnennung".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostNotice {
    pub text: String,
    /// Erhöhen, mit Betrag. Die Antwort darauf gibt [`Household::raise`].
    pub raise_label: String,
    /// Es dabei belassen.
    pub keep_label: &'static str,
}

/// Fällige Mitteilungen an Eingeladene, siehe [`Household::invitations`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "say afterwards who was reached, or the invitations come again"]
pub struct Invitations {
    /// An wen, und an welchem Tisch.
    pub due: Vec<Due>,
    kept: BTreeSet<String>,
}

/// Alles, was sich im Lauf eines Fischs ändert.
pub struct Household {
    store: Store,
    states: Vec<RoomState>,
    /// Wen der Fisch in diesem Lauf schon um Zustimmung gebeten hat.
    asked: HashSet<String>,
}

impl Household {
    /// Öffnet die Zustandsdatei des Profils. Fehlt sie, beginnt der Fisch
    /// leer. Der Gesprächskontext beginnt immer leer: er liegt nur im
    /// Arbeitsspeicher.
    pub fn open(config: &Config, state_path: &Path) -> Result<Household, String> {
        Ok(Household {
            store: Store::open(state_path)?,
            states: config.rooms.iter().map(|_| RoomState::default()).collect(),
            asked: HashSet::new(),
        })
    }

    /// Eine Nachricht ist angekommen: was ist abzusetzen?
    ///
    /// `None`, wenn es nichts zu tun gibt: der Kanal gehört zu keinem Raum,
    /// oder die Nachricht ist schon verarbeitet, etwa weil Nachholen und
    /// Live-Meldung sich überschneiden. Dann ändert sich auch nichts.
    ///
    /// Was die Tür selbst abgesetzt hat, darf sie nicht wieder hereinreichen.
    pub async fn message<M: Model + Sync>(
        &mut self,
        model: &M,
        terms: &Terms<'_>,
        msg: &Arrived<'_>,
    ) -> Option<Steps> {
        let config = terms.config;
        let room_index = config.room_index(msg.channel)?;
        if self.store.seen(msg.channel).is_some_and(|seen| seen >= msg.mark) {
            return None;
        }

        // Wer den Fisch startet, hat zugestimmt.
        let is_host = msg.author == config.host;
        // Die Zustimmung gilt dem KI-Dienst, den die Bitte genannt hat. Nach
        // einem Wechsel des Dienstes wird neu gefragt.
        let consented = is_host || self.store.has_consented(msg.author, terms.service);
        if consented {
            self.store.settle_service(msg.author, terms.service);
        }
        let names: Vec<&str> = config.ai_names.iter().map(String::as_str).collect();
        let incoming = Incoming {
            channel: msg.channel,
            author: msg.author,
            text: msg.text,
            consented,
            // Die Raum-Logik kennt keine Kennungen. Was der Gastgeber über
            // diese Person eingetragen hat, wird hier hineingereicht.
            author_gender: config.gender_of(msg.author),
        };
        let house = House { limits: &config.limits, ai_names: &names, today: terms.today };
        let mut tally = self.store.tally();
        let deliveries = converse(
            model,
            &config.rooms[room_index],
            &mut self.states[room_index],
            &mut tally,
            &house,
            &incoming,
            is_host,
        )
        .await;

        // Die Bitte geht an den Tisch des Schreibenden, auch wenn der Fisch
        // dort sonst nichts schreibt: er übersetzt die Sätze ja für andere
        // Tische. Sie unterbleibt nur, wo er von diesem Menschen ohnehin nie
        // etwas an die KI gäbe (siehe `Config::sends_to_ai`). Einmal je Lauf.
        let ask_consent = if !consented
            && config.sends_to_ai(msg.channel)
            && self.asked.insert(msg.author.to_owned())
        {
            Some(consent_request(model, terms, msg.channel, msg.author, &mut tally).await)
        } else {
            None
        };

        // Ging etwas unübersetzt hinaus, weil das Kontingent leer ist, erfährt
        // es der Gastgeber, einmal je Erschöpfung.
        let tell_host = if budget_ran_out(&deliveries) && tally.notice_due(&config.limits) {
            let step = config.limits.translation_tokens_per_day;
            Some(HostNotice {
                text: budget_notice(tally.allowed_today(&config.limits), step),
                raise_label: raise_label(step),
                keep_label: KEEP_LABEL,
            })
        } else {
            None
        };

        self.store.set_tally(&tally);
        self.store.saw(msg.channel, msg.mark);
        Some(Steps { deliveries, ask_consent, tell_host })
    }

    /// Schreibt die Zustandsdatei. Nach dem Absetzen, nicht davor.
    pub fn save(&self) -> Result<(), String> {
        self.store.save()
    }

    /// Bis wohin dieser Kanal verarbeitet ist. Von dort holt die Tür nach,
    /// was geschrieben wurde, während der Fisch nicht lief.
    pub fn seen(&self, channel: &str) -> Option<u64> {
        self.store.seen(channel)
    }

    /// Dieser Mensch hat zugestimmt: für sich selbst und für niemanden sonst,
    /// und dem KI-Dienst, den die Bitte genannt hat, keinem anderen. Wird
    /// sofort gespeichert.
    ///
    /// Nemesus am 19.11.2026: wer zustimmt, soll wissen, wohin seine Sätze
    /// gehen. Bis zum 21.11.2026 galt eine Zustimmung trotzdem weiter, wenn
    /// der Gastgeber danach den Dienst wechselte.
    pub fn consent(&mut self, user: &str, service: &str) -> Result<(), String> {
        self.store.consent(user, service);
        self.store.save()
    }

    /// Der Gastgeber will das heutige Kontingent erhöhen. Legt genau einen
    /// Tagesbetrag nach, aber nur, wenn es verbraucht ist: ein doppelter
    /// Klick erhöht nicht doppelt. Zurück kommen die Antwort an ihn und ob
    /// das Speichern gelang.
    ///
    /// Ob wirklich der Gastgeber fragt, prüft die Tür: sie weiß, wer drückt.
    pub fn raise(&mut self, terms: &Terms<'_>) -> (String, Result<(), String>) {
        let limits = &terms.config.limits;
        let mut tally = self.store.tally();
        // Seit der Hinweis hinausging, kann ein neuer Tag begonnen haben.
        tally.start_day(terms.today);
        let raised = tally.raise(limits);
        let (allowed, left) = (tally.allowed_today(limits), tally.tokens_left(limits));
        self.store.set_tally(&tally);
        let saved = self.store.save();
        let text =
            if raised { raised_notice(allowed, left) } else { nothing_to_raise(allowed, left) };
        (text, saved)
    }

    /// Wem jetzt mitzuteilen ist, dass er eingeladen ist: einmal je Mensch,
    /// an seinem Tisch. `None`, wenn nichts fällig und niemand zu vergessen
    /// ist; dann bleibt die Zustandsdatei, wie sie ist.
    ///
    /// Den Wortlaut gibt `texts::invitation`. Die Tür setzt ihn ab und sagt
    /// danach mit [`Household::invitations_told`], wen sie erreicht hat.
    pub fn invitations(&self, config: &Config) -> Option<Invitations> {
        let (now, kept) = due(config, self.store.told());
        if now.is_empty() && kept == *self.store.told() {
            return None;
        }
        Some(Invitations { due: now, kept })
    }

    /// Merkt sich, wen die Mitteilung erreicht hat, und speichert. Wen sie
    /// nicht erreicht hat, bleibt ungemerkt: der nächste Start versucht es
    /// wieder.
    pub fn invitations_told(
        &mut self,
        pending: Invitations,
        reached: &[String],
    ) -> Result<(), String> {
        let mut told = pending.kept;
        told.extend(reached.iter().cloned());
        self.store.set_told(told);
        self.store.save()
    }
}

/// Die Bitte um Zustimmung in der Sprache des Tisches.
///
/// Auf Deutsch, Englisch und Russisch steht die Bitte fest im Programm: sie
/// kostet nichts und kommt auch an, wenn die KI ausfällt. Für jede andere
/// Sprache übersetzt die KI die englische Fassung, wie bis zum 20.11.2026
/// für alle.
async fn consent_request<M: Model + Sync>(
    model: &M,
    terms: &Terms<'_>,
    channel: &str,
    author: &str,
    tally: &mut Tally,
) -> String {
    let config = terms.config;
    let lang = config.lang_of(channel).unwrap_or("English");
    if let Some(tongue) = known_tongue(lang) {
        return consent_text_in(tongue, terms.service);
    }
    let base = consent_text(terms.service);
    let request = Request {
        speaker: "fish",
        text: &base,
        target_lang: lang,
        pass_through: &[],
        context: &[],
        writer_gender: None,
        // Die Bitte spricht den Menschen an, der eben geschrieben hat: zuerst
        // die Angabe zu ihm selbst, sonst die des Platzes.
        reader_gender: config.gender_for(author, channel),
        // Entschieden von Nemesus am 19.11.2026: die Begrüßung duzt.
        informal: true,
    };
    // Auch diese Übersetzung zählt gegen den Übersetzungstopf. Ist er leer,
    // fragt der Fisch auf Englisch.
    let meter = Metered::new(model, tally.tokens_left(&config.limits));
    let translated = translate(&meter, &request).await;
    tally.spend(meter.used());
    match translated {
        Ok(Outcome::Translated(text)) => text,
        // Lieber auf Englisch fragen als gar nicht.
        _ => base,
    }
}
