//! Was Türen teilen, wenn ein Profil an mehr als einer steht: der gemischte
//! Raum. Ohne Netz.
//!
//! Seit dem 23.11.2026. Ein Gespräch kann Tische an verschiedenen Türen haben,
//! etwa einen Discord-Kanal und einen Matrix-Raum. Dafür braucht es zweierlei:
//!
//! - Einen Haushalt für beide Türen. Er hält die Regeln, die Zustandsdatei und
//!   den Gesprächskontext; sein Schloss lässt die Nachrichten des Profils
//!   einzeln durch, gleich durch welche Tür sie kommen.
//! - Eine Leitung von Tür zu Tür ([`Lines`]). Die Tür, an der geschrieben
//!   wurde, fragt den Haushalt und setzt ab, was an ihren eigenen Tischen
//!   erscheint. Was an einem Tisch der anderen Tür erscheint, reicht sie
//!   dieser zu und wartet, bis es abgesetzt ist: erst danach wird gespeichert.
//!
//! Die Türen kennen einander weiter nicht. Sie kennen diese Leitung und die
//! Tür, an der ein Tisch steht (`Config::door_of`).
//!
//! Benannte Grenzen:
//! - Wartet eine Tür auf die andere, hält sie das Schloss des Haushalts. Hängt
//!   die andere an ihrem Dienst, wartet das ganze Profil, nicht nur ein Tisch.
//! - Nachgeholt wird je Tür für sich. Was während einer Pause an beiden Türen
//!   geschrieben wurde, kommt je Tür in der Reihenfolge an, in der es
//!   geschrieben wurde, über die Türen hinweg nicht unbedingt.

use crate::config::Door;
use crate::household::{HostNotice, Household};
use rusty_fish_room::Delivery;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::sync::{Mutex, oneshot};

/// Der Schreibende, wie die Tür ihn kennt, an der er schrieb. Eine andere Tür
/// macht daraus, was sie zeigen kann.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Writer {
    /// Die Tür, an der er schrieb.
    pub door: Door,
    /// Der Name, unter dem er dort erscheint, wo er schrieb.
    pub name: String,
    /// Die Adresse seines Bildes, wenn es eine gibt, die jeder abrufen kann.
    pub face: Option<String>,
    /// Was an seiner Nachricht hing, als Text unter ihr: je Anhang eine Zeile
    /// mit seiner Adresse, jede mit einem Zeilenumbruch davor. Leer, wenn
    /// nichts daran hing.
    pub attachments: String,
}

/// Was eine Tür einer anderen zureicht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Zustellungen für Tische der anderen Tür.
    Deliver { deliveries: Vec<Delivery>, writer: Writer },
    /// Der Hinweis an den Gastgeber, dass das Kontingent leer ist: er liest
    /// an der anderen Tür.
    TellHost(HostNotice),
}

/// Ein Auftrag unterwegs zu einer Tür.
pub struct Handed {
    /// Der Name des Profils, zu dem er gehört.
    pub profile: Option<String>,
    pub errand: Errand,
    /// Hierhin sagt die Tür, ob alles ankam, und wenn nicht, warum.
    pub done: oneshot::Sender<Result<(), String>>,
}

/// Ein Profil, das eine Tür abgelehnt oder verloren hat, während eine andere
/// es bedient. Das Programm hält es dann überall an: ein halber Raum, in dem
/// die eine Seite schreibt und die andere nichts liest, wäre schlimmer als
/// keiner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub door: Door,
    pub profile: String,
    pub why: String,
}

/// Die Leitungen zwischen den Türen eines laufenden Fischs. Das Programm legt
/// sie an; eine Tür bekommt beim Öffnen ihr Ende.
#[derive(Default)]
pub struct Lines {
    to: RwLock<HashMap<Door, UnboundedSender<Handed>>>,
    refused: RwLock<Option<UnboundedSender<Refused>>>,
}

impl Lines {
    pub fn new() -> Arc<Lines> {
        Arc::new(Lines::default())
    }

    /// Die Tür öffnet: von jetzt an kommt bei ihr an, was ihr zugereicht
    /// wird. Eine frühere Leitung zu ihr ist damit zu.
    pub fn open(&self, door: Door) -> UnboundedReceiver<Handed> {
        let (to, handed) = unbounded_channel();
        self.to.write().unwrap_or_else(|poisoned| poisoned.into_inner()).insert(door, to);
        handed
    }

    /// Die Tür ist zu. Was ihr noch zugereicht wird, scheitert und sagt es.
    pub fn close(&self, door: Door) {
        self.to.write().unwrap_or_else(|poisoned| poisoned.into_inner()).remove(&door);
    }

    /// Reicht einer Tür einen Auftrag zu und wartet, bis sie ihn erledigt hat.
    pub async fn hand(
        &self,
        door: Door,
        profile: Option<&str>,
        errand: Errand,
    ) -> Result<(), String> {
        let closed = || format!("the {} door is not open", door.label());
        let (done, answer) = oneshot::channel();
        let handed = Handed { profile: profile.map(str::to_owned), errand, done };
        {
            let to = self.to.read().unwrap_or_else(|poisoned| poisoned.into_inner());
            to.get(&door).ok_or_else(closed)?.send(handed).map_err(|_| closed())?;
        }
        // Schließt die Tür, bevor sie antwortet, ist der Auftrag verloren.
        answer.await.unwrap_or_else(|_| Err(closed()))
    }

    /// Das Programm will hören, wenn eine Tür ein geteiltes Profil ablehnt.
    pub fn listen_for_refusals(&self) -> UnboundedReceiver<Refused> {
        let (to, refused) = unbounded_channel();
        *self.refused.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(to);
        refused
    }

    /// Eine Tür sagt, dass sie ein geteiltes Profil nicht bedienen kann.
    pub fn refuse(&self, door: Door, profile: &str, why: &str) {
        let refused = self.refused.read().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(to) = refused.as_ref() {
            let _ = to.send(Refused { door, profile: profile.to_owned(), why: why.to_owned() });
        }
    }
}

/// Eine Leitung, an deren anderem Ende niemand ist: für eine Tür, die allein
/// läuft. Über sie kommt nie etwas.
pub fn no_line() -> UnboundedReceiver<Handed> {
    unbounded_channel().1
}

/// Was ein Profil an jeder seiner Türen dabeihat, wenn es an mehr als einer
/// steht: denselben Haushalt und dieselben Leitungen.
#[derive(Clone)]
pub struct Shared {
    pub house: Arc<Mutex<Household>>,
    pub lines: Arc<Lines>,
}

/// Teilt Zustellungen nach der Tür ihres Tisches: zuerst die für `here`, dann
/// je andere Tür die ihren, in der Reihenfolge, in der sie kamen. Ein Kanal,
/// den das Profil nicht kennt, bleibt bei `here`; dort scheitert er laut.
pub fn by_door(
    config: &crate::config::Config,
    here: Door,
    deliveries: &[Delivery],
) -> (Vec<Delivery>, Vec<(Door, Vec<Delivery>)>) {
    let mut mine = Vec::new();
    let mut others: Vec<(Door, Vec<Delivery>)> = Vec::new();
    for delivery in deliveries {
        match config.door_of(&delivery.channel) {
            Some(door) if door != here => match others.iter_mut().find(|(d, _)| *d == door) {
                Some((_, list)) => list.push(delivery.clone()),
                None => others.push((door, vec![delivery.clone()])),
            },
            _ => mine.push(delivery.clone()),
        }
    }
    (mine, others)
}
