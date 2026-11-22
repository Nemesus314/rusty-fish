//! Das Profil eines Fischs, als TOML-Datei. Nichts ist auf einen bestimmten
//! Betreiber zugeschnitten: andere sollen den Fisch bei sich laufen lassen.
//!
//! Seit dem 21.11.2026 trägt ein Mensch im Profil einen Namen, den der
//! Gastgeber vergibt, und daran hängt seine Kennung an einer Tür. Der Name ist
//! das Etikett des Gastgebers, wie ein Eintrag im Adressbuch: er verlässt das
//! Profil nie, weder zur KI noch zu Discord. `host` und `owner` zeigen mit dem
//! Namen auf einen Menschen; die Discord-Kennung an derselben Stelle gilt
//! weiter, so wie Profile bis dahin geschrieben waren.
//!
//! Benannte Grenzen:
//! - Es gibt eine Tür, Discord. Ein Tisch nennt sie schon (`discord = "..."`),
//!   damit ein Raum später Tische aus mehreren Türen haben kann.
//! - Innen steht ein Mensch unter einem Schlüssel: seiner Discord-Kennung,
//!   sonst seinem Namen. Mit einer zweiten Tür braucht es dafür eine Adresse
//!   aus Tür und Kennung.
//! - Zustimmung, Fragenzähler und Lesemarken hängen an der Kennung, nicht am
//!   Namen. Dass zwei Kennungen derselbe Mensch sind, behauptet nur der
//!   Gastgeber; darauf darf keine Zustimmung ruhen.

use rusty_fish_providers::Provider;
use rusty_fish_room::tongue::{Tongue, tongue_of};
use rusty_fish_room::{Gender, Limits, Room, Seat};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Der KI-Dienst, der übersetzt. Fehlt die Angabe, gilt OpenAI, damit
    /// Profile von vor dem 20.11.2026 unverändert laufen.
    ///
    /// Entschieden von Nemesus am 20.11.2026: „Die Möglichkeit, die KIs im
    /// Profil zu wechseln sollte jedoch erhalten bleiben."
    pub provider: Provider,
    /// Modell, das übersetzt. Siehe `rusty-fish models`.
    pub model: String,
    /// Eine eigene Adresse statt der des Dienstes. Bei einem selbst gehosteten
    /// Modell („custom") ist sie Pflicht, sonst eine Überschreibung.
    pub base_url: Option<String>,
    /// Discord-Kennung des Gastgebers. Wer den Fisch startet, hat zugestimmt.
    pub host: String,
    /// Der Gastgeber, wie er im Profil steht: sein Name oder seine Kennung.
    /// Die Maske zeigt und speichert das Eingetragene.
    pub host_written: String,
    /// Die Rufnamen der KI. Der erste ist der Name, unter dem sie erscheint;
    /// jeder gilt als Ansprache. Ist die Liste leer, übersetzt sie nur.
    /// Nemesus am 18.11.2026: „Den können wir frei wählen."
    ///
    /// Mehrere, weil ein Gast mit kyrillischer Tastatur für einen lateinischen
    /// Namen jedes Mal umschalten müsste (entschieden am 19.11.2026).
    pub ai_names: Vec<String>,
    /// Was der Gastgeber pro Tag erlaubt.
    pub limits: Limits,
    /// Die Räume, so wie die Raum-Logik sie braucht: an jedem Tisch steht
    /// schon, was dort gilt (siehe [`Written`] für das, was eingetragen wurde).
    pub rooms: Vec<Room>,
    /// Was der Gastgeber über einzelne Menschen eingetragen hat. Freiwillig.
    /// Der Schlüssel ist die Discord-Kennung des Menschen; hat er noch keine,
    /// sein Name. Die beiden können nicht zusammenfallen: eine Kennung besteht
    /// nur aus Ziffern, ein Name nie.
    ///
    /// Entschieden am 19.11.2026: beide Kanäle bleiben für beide offen.
    /// Nemesus: „Sie soll ruhig auch den Kanal #tee sehen können und sich
    /// reinschalten." Seine Frage, weil das Geschlecht bis dahin am Platz
    /// hing: „Wie können wir das ändern?"
    pub people: BTreeMap<String, Person>,
    /// Die Schlüssel der Menschen in der Reihenfolge, in der sie eingetragen
    /// sind. Die Maske zeigt sie so wieder und findet nach dem Speichern vor,
    /// was sie geschickt hat; nach Kennung sortiert stünde "9" hinter "42".
    pub people_order: Vec<String>,
    /// Was zu jedem Platz in der Datei steht, nach Kanal. Die Maske zeigt und
    /// speichert das Eingetragene, nicht das daraus Abgeleitete.
    pub written: BTreeMap<String, Written>,
}

/// Was der Gastgeber über einen Menschen eingetragen hat.
///
/// Nemesus am 20.11.2026: „ob es nicht sinnvoller wäre, pro Discord-Nummer die
/// Muttersprache und die Neutralsprache festzulegen. Dann wäre die Aufgabe
/// [...]: Übersetze alles für eine Russin, halte dich von Englisch fern."
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Person {
    /// Der Name, den der Gastgeber diesem Menschen gibt. Er steht nur im
    /// Profil und in der Maske; so sieht man, für wen ein Tisch gebaut ist.
    ///
    /// Nemesus am 21.11.2026: „Die Namen, die ich vergebe, gehören in der
    /// Windows Version auf jeden Fall in die Maske. So kann man sofort sehen,
    /// für wen man die Tische gebaut hat."
    pub name: Option<String>,
    /// Die Kennung dieses Menschen an der Discord-Tür. Sie darf fehlen: ein
    /// Tisch lässt sich für jemanden bauen, dessen Kennung man noch nicht hat.
    /// Dann gilt an seinem Tisch schon alles, nur seine eigenen Formen als
    /// Schreibender kennt der Fisch erst mit der Kennung.
    pub discord: Option<String>,
    /// Grammatisches Geschlecht, als Hinweis für die Grammatik.
    pub gender: Option<Gender>,
    /// Die Muttersprache: in sie wird an seinem Tisch übersetzt.
    pub lang: Option<String>,
    /// Sprachen, die für diesen Menschen unübersetzt bleiben. `None` heißt: es
    /// gilt die Angabe des Raums. Eine leere Liste heißt: alles übersetzen.
    pub neutral: Option<Vec<String>>,
    /// Hat der Gastgeber diesen Menschen eingeladen? Dann bedient sein Fisch
    /// auch dessen Tisch, auf seine Rechnung.
    ///
    /// Nemesus am 20.11.2026: „Es sollte auch die Funktion ‚Einladen' geben.
    /// Und dann ein Haken, wer eingeladen wird."
    pub invited: bool,
}

/// Was zu einem Platz in der Datei steht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub lang: String,
    pub gender: Option<Gender>,
    /// Der Mensch, dessen Tisch das ist: sein Schlüssel in [`Config::people`].
    /// Wer mit einer bloßen Discord-Kennung eingetragen ist, muss dafür nicht
    /// unter `[[person]]` stehen.
    pub owner: Option<String>,
    /// Der Besitzer, wie er im Profil steht: sein Name oder seine Kennung.
    pub owner_written: Option<String>,
}

/// Die Türen, die dieser Fisch kennt. Ein Tisch gehört zu genau einer.
pub const DOORS: [&str; 1] = ["discord"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    provider: Option<String>,
    model: String,
    base_url: Option<String>,
    host: String,
    ai_name: Option<RawNames>,
    #[serde(default)]
    limits: RawLimits,
    #[serde(rename = "room", default)]
    rooms: Vec<RawRoom>,
    #[serde(rename = "person", default)]
    people: Vec<RawPerson>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPerson {
    name: Option<String>,
    discord: Option<String>,
    /// Das alte Wort für `discord`, aus Profilen von vor dem 21.11.2026.
    id: Option<String>,
    gender: Option<String>,
    lang: Option<String>,
    neutral: Option<Vec<String>>,
    #[serde(default)]
    invited: bool,
}

/// `ai_name` darf ein einzelner Text sein, wie von Anfang an, oder eine Liste.
#[derive(Deserialize)]
#[serde(untagged)]
enum RawNames {
    One(String),
    Many(Vec<String>),
}

/// Jede Grenze darf einzeln fehlen; dann gilt die vorsichtige Vorgabe.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawLimits {
    translation_tokens_per_day: Option<u64>,
    questions_per_guest_per_day: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRoom {
    #[serde(default)]
    pass_through: Vec<String>,
    #[serde(default = "default_context_lines")]
    context_lines: usize,
    #[serde(rename = "seat", default)]
    seats: Vec<RawSeat>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSeat {
    /// Der Kanal dieses Tisches an der Discord-Tür.
    discord: Option<String>,
    /// Das alte Wort für `discord`, aus Profilen von vor dem 21.11.2026.
    channel: Option<String>,
    lang: String,
    gender: Option<String>,
    owner: Option<String>,
}

fn default_context_lines() -> usize {
    10
}

impl Config {
    /// Liest und prüft die Einstellungen. Jeder Fehler ist laut und nennt die Stelle.
    pub fn parse(text: &str) -> Result<Config, String> {
        let raw: RawConfig = toml::from_str(text).map_err(|e| format!("configuration: {e}"))?;

        let model = raw.model.trim().to_owned();
        if model.is_empty() {
            return Err("configuration: `model` is empty".to_owned());
        }
        // Die Menschen zuerst: `host` und `owner` zeigen mit Namen auf sie, und
        // was an einem Tisch gilt, hängt an seinem Besitzer.
        let mut people = BTreeMap::new();
        let mut people_order = Vec::new();
        // Vom Namen zum Schlüssel.
        let mut names: BTreeMap<String, String> = BTreeMap::new();
        for (n, person) in raw.people.into_iter().enumerate() {
            let name = read_person_name(person.name.as_deref())?;
            let discord = match (filled(person.discord.as_deref()), filled(person.id.as_deref())) {
                (Some(_), Some(_)) => {
                    return Err(format!(
                        "configuration: person {} has both `discord` and `id`; `id` is the old \
                         word for `discord`, give one of them",
                        n + 1
                    ));
                }
                (Some(id), None) => Some(discord_id(id, "discord")?),
                (None, Some(id)) => Some(discord_id(id, "id")?),
                (None, None) => None,
            };
            // Die Kennung, sonst der Name. Eines von beiden muss da sein,
            // sonst wäre der Eintrag über niemanden.
            let Some(key) = discord.clone().or_else(|| name.clone()) else {
                return Err(format!(
                    "configuration: person {} needs a `name` or a `discord` id",
                    n + 1
                ));
            };
            let entry = Person {
                // Bis zum 20.11.2026 war das Geschlecht hier Pflicht, weil ein
                // Eintrag sonst nichts aussagte. Seit ein Mensch auch Sprache
                // und Einladung trägt, ist es freiwillig wie die anderen.
                gender: read_gender(person.gender.as_deref(), &format!("person {key}"))?,
                lang: person.lang.map(|l| l.trim().to_owned()).filter(|l| !l.is_empty()),
                neutral: person.neutral.map(clean_langs),
                invited: person.invited,
                name: name.clone(),
                discord,
            };
            // Zwei Einträge zur selben Person wären ein Versehen oder ein Widerspruch.
            if people.insert(key.clone(), entry).is_some() {
                return Err(format!("configuration: person {key} is listed twice"));
            }
            if let Some(name) = name
                && names.insert(name.clone(), key.clone()).is_some()
            {
                return Err(format!("configuration: the name {name:?} is used for two people"));
            }
            people_order.push(key);
        }

        // Der Gastgeber: sein Name oder, wie von Anfang an, seine Kennung. Die
        // Kennung braucht der Fisch in jedem Fall: an ihr erkennt er an der
        // Tür, wer der Gastgeber ist, und an sie schickt er dessen Hinweise.
        let host_written = raw.host.trim().to_owned();
        let host = points_at(&host_written, "host", &names)?;
        if !is_id(&host) {
            return Err(format!(
                "configuration: the host {host_written:?} needs a Discord id: add \
                 `discord = \"...\"` to that `[[person]]`"
            ));
        }
        if raw.rooms.is_empty() {
            return Err("configuration: no `[[room]]` is defined".to_owned());
        }

        let mut written = BTreeMap::new();
        let mut rooms = Vec::new();
        for (n, room) in raw.rooms.into_iter().enumerate() {
            if room.seats.len() < 2 {
                return Err(format!("configuration: room {} needs at least two seats", n + 1));
            }
            let mut seats = Vec::new();
            for seat in room.seats {
                let channel = match (filled(seat.discord.as_deref()), filled(seat.channel.as_deref()))
                {
                    (Some(_), Some(_)) => {
                        return Err(format!(
                            "configuration: a seat in room {} has both `discord` and `channel`; \
                             `channel` is the old word for `discord`, give one of them",
                            n + 1
                        ));
                    }
                    (Some(id), None) => discord_id(id, "discord")?,
                    (None, Some(id)) => discord_id(id, "channel")?,
                    (None, None) => {
                        return Err(format!(
                            "configuration: a seat in room {} needs `discord`, the id of its \
                             Discord channel",
                            n + 1
                        ));
                    }
                };
                let lang = seat.lang.trim().to_owned();
                if lang.is_empty() {
                    return Err(format!("configuration: `lang` is empty for channel {channel}"));
                }
                // Ein Kanal an zwei Plätzen würde Nachrichten im Kreis schicken.
                if written.contains_key(&channel) {
                    return Err(format!("configuration: channel {channel} is used twice"));
                }
                let gender = read_gender(seat.gender.as_deref(), &format!("channel {channel}"))?;
                // Ein leerer Besitzer heißt wie ein fehlender: der Tisch gehört niemandem.
                let owner_written = filled(seat.owner.as_deref()).map(str::to_owned);
                let owner = match &owner_written {
                    None => None,
                    Some(who) => Some(points_at(who, "owner", &names)?),
                };
                seats.push(at_the_table(&channel, &lang, gender, owner.as_deref(), &host, &people));
                written.insert(channel, Written { lang, gender, owner, owner_written });
            }
            rooms.push(Room {
                seats,
                pass_through: room.pass_through,
                context_lines: room.context_lines,
            });
        }

        let base_url = raw.base_url.map(|u| u.trim().to_owned()).filter(|u| !u.is_empty());
        let ai_names = read_names(raw.ai_name)?;
        let defaults = Limits::default();
        let limits = Limits {
            translation_tokens_per_day: raw
                .limits
                .translation_tokens_per_day
                .unwrap_or(defaults.translation_tokens_per_day),
            questions_per_guest_per_day: raw
                .limits
                .questions_per_guest_per_day
                .unwrap_or(defaults.questions_per_guest_per_day),
        };
        // Fehlt der Dienst oder ist er leer, gilt OpenAI: so liefen alle
        // Profile, bevor es die Wahl gab. Ein unbekannter wird laut abgelehnt.
        let provider = match raw.provider.as_deref().map(str::trim) {
            None | Some("") => Provider::OpenAi,
            Some(id) => {
                Provider::parse(id).map_err(|why| format!("configuration: `provider`: {why}"))?
            }
        };
        // Die eine Stelle, die über die Adresse entscheidet, prüft auch, ob es
        // eine gibt. Bei einem eigenen Modell muss sie im Profil stehen.
        rusty_fish_providers::address(provider, base_url.as_deref())
            .map_err(|why| format!("configuration: {why}"))?;
        Ok(Config {
            provider,
            model,
            base_url,
            host,
            host_written,
            ai_names,
            limits,
            rooms,
            people,
            people_order,
            written,
        })
    }

    /// Die Adresse des Dienstes, die wirklich benutzt wird: die eigene aus dem
    /// Profil, sonst die des Dienstes. Die Bitte um Zustimmung nennt den Dienst
    /// nach dieser Adresse.
    pub fn service_address(&self) -> String {
        // Dass es eine gibt, hat `parse` geprüft.
        rusty_fish_providers::address(self.provider, self.base_url.as_deref())
            .unwrap_or_default()
    }

    /// Ein Kanal, den dieses Profil und das andere beide nennen, falls es
    /// einen gibt. Zwei solche Profile dürfen nicht zugleich laufen: jede
    /// Nachricht in diesem Kanal würde zweimal übersetzt und zugestellt.
    pub fn shared_channel(&self, other: &Config) -> Option<&str> {
        self.rooms
            .iter()
            .flat_map(|room| room.seats.iter())
            .map(|seat| seat.channel.as_str())
            .find(|channel| other.room_index(channel).is_some())
    }

    /// Wo der KI-Dienst dieses Profils liest: auf diesem Rechner, im eigenen
    /// Netz oder draußen. Geurteilt wird nach der Adresse, die wirklich
    /// benutzt wird (siehe [`crate::place`]).
    pub fn ai_place(&self) -> crate::place::Place {
        crate::place::place_of(&self.service_address())
    }

    /// Das grammatische Geschlecht, das der Gastgeber für diese Person
    /// eingetragen hat.
    pub fn gender_of(&self, user: &str) -> Option<Gender> {
        self.people.get(user).and_then(|p| p.gender)
    }

    /// Das grammatische Geschlecht für jemanden, der an diesem Kanal schreibt
    /// oder dort als Schreibender angesprochen wird: zuerst die Angabe zur
    /// Person, sonst die des Platzes, sonst keine. Die Anrede des Besitzers
    /// gilt hier nicht; sie gilt für das, was an seinem Tisch gelesen wird
    /// (siehe [`Seat::reader`]).
    pub fn gender_for(&self, user: &str, channel: &str) -> Option<Gender> {
        self.gender_of(user).or_else(|| self.seat_of(channel).and_then(|s| s.gender))
    }

    /// Der Name, unter dem die KI erscheint: ihr erster Rufname.
    pub fn ai_name(&self) -> Option<&str> {
        self.ai_names.first().map(String::as_str)
    }

    /// In welchem Raum liegt dieser Kanal?
    pub fn room_index(&self, channel: &str) -> Option<usize> {
        self.rooms.iter().position(|r| r.seats.iter().any(|s| s.channel == channel))
    }

    /// Der Platz an diesem Kanal, mit dem, was dort gilt.
    pub fn seat_of(&self, channel: &str) -> Option<&Seat> {
        self.rooms.iter().flat_map(|r| r.seats.iter()).find(|s| s.channel == channel)
    }

    /// In welche Sprache wird an diesem Kanal übersetzt? Die Muttersprache
    /// des Besitzers, sonst die Sprache des Platzes.
    pub fn lang_of(&self, channel: &str) -> Option<&str> {
        self.seat_of(channel).map(|s| s.lang.as_str())
    }

    /// Die Sprache der festen Texte an diesem Tisch: Deutsch, Englisch oder
    /// Russisch, je nach Sprache des Tisches; für jede andere Englisch.
    pub fn tongue_at(&self, channel: &str) -> Tongue {
        tongue_of(self.lang_of(channel).unwrap_or(""))
    }

    /// Schreibt dieser Fisch an diesen Kanal? Ein Kanal außerhalb jedes
    /// Raums wird nicht bedient.
    pub fn serves(&self, channel: &str) -> bool {
        self.seat_of(channel).is_some_and(|s| s.served)
    }

    /// Wessen Tisch ist dieser Kanal? Der Schlüssel des Menschen in
    /// [`Config::people`]: seine Discord-Kennung, sonst sein Name.
    pub fn owner_of(&self, channel: &str) -> Option<&str> {
        self.written.get(channel).and_then(|w| w.owner.as_deref())
    }

    /// Der Name, den der Gastgeber dem Menschen mit diesem Schlüssel gab.
    pub fn name_of(&self, key: &str) -> Option<&str> {
        self.people.get(key).and_then(|p| p.name.as_deref())
    }

    /// Darf dieser Mensch an diesem Kanal die KI dieses Fischs ansprechen?
    ///
    /// Nemesus am 22.11.2026, nach dem ersten Lauf mit zwei Fischen in einem
    /// Raum: „eigentlich sollte es ja nicht sein, dass ich als Nemesus den
    /// russischen Pesce Fisch ansprechen kann. Genau so wenig, sollte Nemitest
    /// den Fisch im deutschen Kanal ansprechen können."
    ///
    /// Jeder bringt seine eigene KI mit und bezahlt sie. Ansprechen dürfen
    /// sie deshalb die, für die dieser Fisch da ist: der Gastgeber und wen er
    /// eingeladen hat. Bis dahin durfte jeder, der an einem bedienten Tisch
    /// schrieb; im Lauf mit zwei Fischen verbrauchte so der eine die Tokens
    /// des anderen.
    ///
    /// Zwei Fälle, in denen der Fisch die Menschen nicht auseinanderhalten
    /// kann und deshalb weiter jeden fragen lässt, der an diesem Tisch
    /// schreibt (das Fragenlimit je Gast gilt):
    /// - Der Tisch hat keinen Besitzer. So sind Profile von vor dem 20.11.2026.
    /// - Der Tisch gehört einem Eingeladenen, dessen Kennung noch fehlt. Wer
    ///   dort schreibt, ist vermutlich er; wissen kann es der Fisch nicht.
    pub fn may_ask(&self, user: &str, channel: &str) -> bool {
        if self.ai_names.is_empty() {
            return false;
        }
        // Nur an einem Tisch, an den dieser Fisch schreibt: sonst könnte der
        // Fragende die Antwort dort nie lesen.
        if !self.seat_of(channel).is_some_and(|seat| seat.served) {
            return false;
        }
        if user == self.host || self.people.get(user).is_some_and(|person| person.invited) {
            return true;
        }
        match self.owner_of(channel) {
            None => true,
            Some(owner) => {
                self.people.get(owner).is_some_and(|p| p.invited && p.discord.is_none())
            }
        }
    }

    /// Gäbe dieser Fisch an die KI, was dieser Mensch an diesem Kanal schreibt?
    ///
    /// Nur dann bittet er ihn dort um Zustimmung. Das ist der Fall, wenn er
    /// im selben Raum einen anderen Tisch bedient (für den übersetzt er es)
    /// oder wenn dieser Mensch hier die KI ansprechen darf (dann kann eine
    /// Frage an sie gehen, siehe [`Config::may_ask`]).
    ///
    /// Warum nicht einfach an jedem Tisch: bringt jeder seinen eigenen Fisch
    /// mit, bedient jeder Fisch nur den Tisch seines Gastgebers. Wer dort als
    /// Gast schreibt, bekäme sonst von diesem Fisch eine Bitte um Zustimmung
    /// für eine KI, die von ihm nie ein Wort liest.
    pub fn sends_to_ai(&self, channel: &str, user: &str) -> bool {
        let Some(room) = self.room_index(channel).map(|n| &self.rooms[n]) else { return false };
        room.seats.iter().any(|seat| seat.served && seat.channel != channel)
            || self.may_ask(user, channel)
    }
}

/// Was an einem Tisch gilt. Hat er einen Besitzer, gehen dessen Angaben vor:
/// seine Muttersprache vor der Sprache des Platzes, seine unübersetzten
/// Sprachen vor denen des Raums, seine Anrede vor der des Platzes.
///
/// Bedient wird ein Tisch,
/// - wenn er keinen Besitzer hat (so laufen Profile von vor dem 20.11.2026
///   unverändert weiter),
/// - wenn er dem Gastgeber gehört,
/// - oder wenn sein Besitzer eingeladen ist.
///
/// Für alle anderen Tische übersetzt der Fisch ihres Besitzers.
fn at_the_table(
    channel: &str,
    lang: &str,
    gender: Option<Gender>,
    owner: Option<&str>,
    host: &str,
    people: &BTreeMap<String, Person>,
) -> Seat {
    // Der Besitzer muss nicht unter `[[person]]` stehen. Dann ist über ihn
    // nichts bekannt: es bleibt bei Platz und Raum, und eingeladen ist er nicht.
    let person = owner.and_then(|id| people.get(id));
    let served = match owner {
        None => true,
        Some(id) => id == host || person.is_some_and(|p| p.invited),
    };
    Seat {
        channel: channel.to_owned(),
        lang: person.and_then(|p| p.lang.clone()).unwrap_or_else(|| lang.to_owned()),
        // Die Angabe des Platzes bleibt stehen: sie gilt weiter für den, der
        // hier schreibt und selbst keinen Eintrag hat (Person vor Platz).
        gender,
        // Gelesen wird am Tisch vom Besitzer. Kennt die Tür seine Anrede
        // nicht, fällt die Raum-Logik auf die des Platzes zurück.
        reader_gender: person.and_then(|p| p.gender),
        neutral: person.and_then(|p| p.neutral.clone()),
        served,
    }
}

/// Eine Liste von Sprachen, wie ein Mensch sie eintippt: Leerraum weg, leere
/// Einträge weg. Die Reihenfolge bleibt.
fn clean_langs(langs: Vec<String>) -> Vec<String> {
    langs.into_iter().map(|l| l.trim().to_owned()).filter(|l| !l.is_empty()).collect()
}

/// Liest die Rufnamen der KI.
///
/// - Fehlt der Wert, ist die Liste leer oder steht dort ein einzelner leerer
///   Text, dann ist die KI nicht ansprechbar. So war es von Anfang an.
/// - Ein leerer Eintrag in einer Liste wird laut abgelehnt: wer eine Liste
///   schreibt, meint jeden Eintrag, und ein leerer ist ein Versehen.
fn read_names(raw: Option<RawNames>) -> Result<Vec<String>, String> {
    match raw {
        None => Ok(Vec::new()),
        Some(RawNames::One(name)) => {
            let name = name.trim();
            Ok(if name.is_empty() { Vec::new() } else { vec![name.to_owned()] })
        }
        Some(RawNames::Many(names)) => {
            let mut out = Vec::new();
            for (n, name) in names.iter().enumerate() {
                let name = name.trim();
                if name.is_empty() {
                    return Err(format!("configuration: entry {} of `ai_name` is empty", n + 1));
                }
                out.push(name.to_owned());
            }
            Ok(out)
        }
    }
}

/// Liest die Angabe zum grammatischen Geschlecht eines Platzes oder einer
/// Person. Erlaubt sind "female" und "male"; alles andere wird laut abgelehnt,
/// damit ein Tippfehler nicht still zu „keine Angabe" wird. `whose` nennt in
/// der Fehlermeldung die Stelle, etwa "channel 222" oder "person 777".
fn read_gender(value: Option<&str>, whose: &str) -> Result<Option<Gender>, String> {
    let Some(value) = value else { return Ok(None) };
    match value.trim().to_ascii_lowercase().as_str() {
        "female" => Ok(Some(Gender::Female)),
        "male" => Ok(Some(Gender::Male)),
        _ => Err(format!(
            "configuration: `gender` for {whose} must be \"female\" or \"male\", got {value:?}"
        )),
    }
}

/// Ein eingetragener Text ohne Leerraum drumherum; ein leerer gilt als keiner.
fn filled(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

/// Sieht dieser Text aus wie eine Kennung, also nur Ziffern?
fn is_id(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_ascii_digit())
}

/// Liest den Namen, den der Gastgeber einem Menschen gibt. Ein leerer gilt
/// als keiner.
///
/// Ein Name aus lauter Ziffern wird abgelehnt: an `host` und `owner` wäre er
/// von einer Kennung nicht zu unterscheiden.
fn read_person_name(value: Option<&str>) -> Result<Option<String>, String> {
    let Some(name) = filled(value) else { return Ok(None) };
    if is_id(name) {
        return Err(format!(
            "configuration: the name {name:?} consists of digits only; that would be taken for \
             an id"
        ));
    }
    // In einer Zeile: der Name steht in Listen und Auswahlfeldern der Maske.
    Ok(Some(name.split_whitespace().collect::<Vec<_>>().join(" ")))
}

/// Auf wen `host` oder `owner` zeigt: der Schlüssel des Menschen.
///
/// Ziffern sind eine Discord-Kennung, wie von Anfang an; dieser Mensch muss
/// nicht unter `[[person]]` stehen. Alles andere ist ein Name und muss dort
/// stehen, sonst wäre ein Tippfehler ein Tisch für niemanden.
fn points_at(
    value: &str,
    field: &str,
    names: &BTreeMap<String, String>,
) -> Result<String, String> {
    let value = value.trim();
    if is_id(value) {
        return discord_id(value, field);
    }
    let name = value.split_whitespace().collect::<Vec<_>>().join(" ");
    names.get(&name).cloned().ok_or_else(|| {
        format!(
            "configuration: `{field}` must be the name of a `[[person]]` or a Discord id (digits \
             only, not zero), got {value:?}"
        )
    })
}

/// Discord-Kennungen sind Zahlen. Ein Kanalname wie "#general" ist keine.
fn discord_id(value: &str, field: &str) -> Result<String, String> {
    let value = value.trim();
    match value.parse::<u64>() {
        Ok(n) if n > 0 => Ok(value.to_owned()),
        _ => Err(format!(
            "configuration: `{field}` must be a Discord id (digits only, not zero), got {value:?}"
        )),
    }
}
