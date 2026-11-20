//! Wer eine Mitteilung über seine Einladung bekommen soll. Rein, ohne Netz.
//!
//! Nemesus am 20.11.2026: „Es sollte auch die Funktion ‚Einladen' geben. Und
//! dann ein Haken, wer eingeladen wird. Das könnte dann per Systemnachricht
//! mitgeteilt werden."

use crate::config::Config;
use std::collections::BTreeSet;

/// Eine fällige Mitteilung: an wen, und an welchem Tisch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Due {
    pub person: String,
    pub channel: String,
}

/// Wem der Fisch jetzt mitteilen soll, dass er eingeladen ist, und wer danach
/// noch als benachrichtigt gilt.
///
/// `told` sind die, denen es schon mitgeteilt wurde. Zurück kommen
/// - die fälligen Mitteilungen, eine je eingeladenem Menschen, an seinem
///   ersten Tisch in der Reihenfolge der Einstellungen;
/// - wer von `told` weiter als benachrichtigt gilt. Wer nicht mehr eingeladen
///   ist oder keinen Tisch mehr hat, fällt heraus: wird der Haken später
///   wieder gesetzt, ist die Mitteilung wieder fällig.
///
/// Wem eine fällige Mitteilung wirklich zugestellt wurde, trägt die Tür
/// selbst nach. So wird eine gescheiterte Zustellung beim nächsten Start
/// wiederholt, statt als erledigt zu gelten.
///
/// Der Gastgeber bekommt keine: er lädt sich nicht selbst ein. Ein
/// Eingeladener ohne eigenen Tisch bekommt auch keine: es gibt keinen Ort,
/// an dem sich für ihn etwas ändert.
///
/// Benannte Grenze: der Fisch sieht die Einstellungen nur beim Start. Wird
/// der Haken entfernt und wieder gesetzt, ohne dass der Fisch dazwischen
/// einmal lief, hat er vom Entfernen nichts gesehen und teilt nichts erneut mit.
pub fn due(config: &Config, told: &BTreeSet<String>) -> (Vec<Due>, BTreeSet<String>) {
    let mut now: Vec<Due> = Vec::new();
    let mut kept = BTreeSet::new();
    for seat in config.rooms.iter().flat_map(|room| room.seats.iter()) {
        let Some(owner) = config.owner_of(&seat.channel) else { continue };
        let invited = config.people.get(owner).is_some_and(|person| person.invited);
        if !invited || owner == config.host {
            continue;
        }
        if told.contains(owner) {
            kept.insert(owner.to_owned());
        } else if !now.iter().any(|due| due.person == owner) {
            now.push(Due { person: owner.to_owned(), channel: seat.channel.clone() });
        }
    }
    (now, kept)
}
