//! Der Anschluss an Matrix.
//!
//! Der Fisch meldet sich als gewöhnliches Konto an und wählt sich von innen
//! bei seinem Server ein, wie bei Discord: kein offener Port, kein eigener
//! Server, ein gewöhnlicher PC genügt, solange er läuft. Ein Tisch ist ein
//! Matrix-Raum, in den der Gastgeber das Konto des Fischs eingeladen hat.
//!
//! Was nach einer Nachricht zu geschehen hat, entscheidet der Haushalt
//! (`rusty-fish-house`), derselbe wie an der Discord-Tür. Diese Tür reicht
//! herein, was ankam, setzt ab, was er entschieden hat, und lässt danach
//! speichern. Keine Regel steht hier ein zweites Mal.
//!
//! Was bei Matrix anders ist als bei Discord:
//! - Der Fisch kann nicht unter fremdem Namen schreiben. Eine Zustellung ist
//!   seine Nachricht, und der Name des Schreibenden steht davor (`render`).
//! - Es gibt keine Knöpfe. Unter der Bitte um Zustimmung und unter dem
//!   Hinweis zum leeren Kontingent stehen Zeichen, die man antippt (`signs`).
//! - Es gibt kein Kennzeichen für Bots am Konto. Der Fisch sendet als
//!   „Notiz" und überliest alle Notizen und alles Eigene; so überliest er
//!   auch jeden anderen Fisch und jeden Bot, der sich an die Regel hält.
//! - Ein Raum kann Ende zu Ende verschlüsselt sein. Dann ist der Fisch ein
//!   Mitglied mit eigenen Geräteschlüsseln (`session`) und liest mit wie
//!   jedes Mitglied. Die Verschlüsselung reicht bis zu ihm: was er übersetzen
//!   lässt, geht danach an den KI-Dienst des Gastgebers.
//! - Matrix zählt Nachrichten nicht. Die Lesemarke des Haushalts ist deshalb
//!   die Zeit der Nachricht in Millisekunden; kämen zwei mit derselben Zeit
//!   oder eine mit einer früheren (die Uhren zweier Server gehen nie ganz
//!   gleich), zählt die Tür weiter, statt eine zu verschlucken.
//!
//! Benannte Grenzen dieser ersten Fassung:
//! - Bearbeitete und gelöschte Nachrichten werden nicht gespiegelt, Bilder,
//!   Dateien und Sprachnachrichten auch nicht. Fremde Reaktionen bleiben, wo
//!   sie sind.
//! - Nachgeholt wird, was der Server beim nächsten Verbinden nachreicht:
//!   höchstens [`CATCH_UP`] Nachrichten je Raum. Waren es mehr, sagt es die
//!   Programmausgabe. Was geschrieben wurde, während nur dieses eine Profil
//!   angehalten war und der Fisch für andere weiterlief, wird nicht
//!   nachgeholt.
//! - Nach einer neuen Anmeldung ist der Fisch ein neues Gerät. Was in einem
//!   verschlüsselten Raum davor geschrieben wurde, kann er nicht lesen.
//! - Kann er eine Nachricht nicht entschlüsseln, steht das in der
//!   Programmausgabe, und am anderen Tisch erscheint nichts.
//! - Die Zustimmung lässt sich noch nicht im Chat widerrufen.
//! - Der Hinweis an den Gastgeber bei leerem Kontingent geht in einen Raum zu
//!   zweit, den der Fisch dafür anlegt; der Gastgeber muss die Einladung
//!   dorthin annehmen. Der Hinweis ist englisch und wird nicht übersetzt.
//! - Alle Profile eines Fischs teilen sich ein Konto, und ein Raum gehört zu
//!   höchstens einem laufenden Profil.
//!
//! In die Ausgabe des Programms gelangen nur Kennungen und Fehlergründe, nie
//! der Text einer Nachricht.

use crate::mention::mention_as_call;
use crate::render::{Rendered, plain, render_in, shown_name, without_quote};
use crate::session::{Access, connect};
use crate::signs::{Ask, Budget, CONSENT_KEY, notice, reaction};
use matrix_sdk::config::SyncSettings;
use matrix_sdk::ruma::api::client::filter::FilterDefinition;
use matrix_sdk::ruma::api::client::sync::sync_events::v3::Filter;
use matrix_sdk::ruma::events::reaction::OriginalSyncReactionEvent;
use matrix_sdk::ruma::events::room::encrypted::OriginalSyncRoomEncryptedEvent;
use matrix_sdk::ruma::events::room::member::StrippedRoomMemberEvent;
use matrix_sdk::ruma::events::room::message::{
    MessageType, OriginalSyncRoomMessageEvent, Relation,
};
use matrix_sdk::ruma::{
    EventId, OwnedEventId, OwnedRoomId, OwnedUserId, RoomId, UInt, UserId,
};
use matrix_sdk::sync::SyncResponse;
use matrix_sdk::{Client, LoopCtrl, Room, RoomState};
use rusty_fish_core::Model;
use rusty_fish_house::clock::today;
use rusty_fish_house::config::{Config, Door};
use rusty_fish_house::household::{Arrived, HostNotice, Household, Terms};
use rusty_fish_house::served::{label_of, title_of};
use rusty_fish_house::texts::{KEPT_NOTICE, invitation_at};
use rusty_fish_room::{Delivery, Sender};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::sync::{Mutex, Notify, RwLock, watch};

pub use rusty_fish_house::served::{Command, OnChange, Profile};

/// Der Name vor dem, was die KI sagt, wenn der Gastgeber ihr keinen gab.
const FISH_NAME: &str = "Rusty Fish";

/// So viele Nachrichten je Raum reicht der Server nach einer Pause nach.
pub const CATCH_UP: u32 = 100;

/// So viele Nachrichten merkt sich ein Profil als schon verarbeitet, damit
/// eine doppelt gemeldete nicht zweimal übersetzt wird.
const REMEMBERED: usize = 1000;

/// Ein Profil im Lauf: seine Einstellungen, seine KI und alles, was sich
/// ändert.
struct Served<M> {
    name: Option<String>,
    model: M,
    config: Config,
    /// Der Name des KI-Dienstes, wie er in der Bitte um Zustimmung steht.
    service: String,
    /// Seit wann der Fisch dieses Profil bedient, in Millisekunden. In einem
    /// Raum, den er noch nie bedient hat, beginnt er hier und nicht bei dem,
    /// was dort früher geschrieben wurde.
    since: u64,
    /// Wann der Fisch in diesem Lauf welchen Raum betreten hat. Betritt er
    /// einen Raum, zeigt ihm der Server, was dort zuletzt geschrieben wurde;
    /// das ist älter als sein Eintritt und wird nicht übersetzt.
    entered: std::sync::Mutex<HashMap<OwnedRoomId, u64>>,
    /// Ein Schloss für alles Veränderliche dieses Profils.
    inner: Mutex<Inner>,
}

struct Inner {
    /// Was der Fisch sich merkt, und was nach einer Nachricht zu geschehen hat.
    house: Household,
    /// Die zuletzt verarbeiteten Nachrichten, nach ihrer Kennung.
    handled: HashSet<OwnedEventId>,
    handled_order: VecDeque<OwnedEventId>,
}

impl Inner {
    fn remember(&mut self, event: OwnedEventId) {
        if self.handled.insert(event.clone()) {
            self.handled_order.push_back(event);
            while self.handled_order.len() > REMEMBERED {
                if let Some(old) = self.handled_order.pop_front() {
                    self.handled.remove(&old);
                }
            }
        }
    }
}

/// Eine Nachricht eines Menschen, wie die Tür sie gehört hat.
struct Heard {
    room: OwnedRoomId,
    event: OwnedEventId,
    sender: OwnedUserId,
    /// Die Zeit der Nachricht, wie der Server des Schreibenden sie angibt.
    at: u64,
    text: String,
    /// Nennt die Nachricht den Fisch in ihrem Feld für Erwähnungen?
    mentioned: bool,
    /// Kann diese Nachricht eine alte sein, die der Server nur noch einmal
    /// zeigt (nach einer neuen Anmeldung, oder weil der Fisch den Raum eben
    /// erst betreten hat)? Dann gilt sie nur, wenn sie neuer ist als die
    /// Lesemarke.
    maybe_old: bool,
}

/// Ein laufendes Profil in der Liste des Fischs. Der Sender gehört zum
/// Eintrag: verlässt das Profil die Liste, endet seine Warteschlange, und
/// wer sie abarbeitet, hört von selbst auf.
struct Entry<M> {
    served: Arc<Served<M>>,
    queue: UnboundedSender<Heard>,
}

/// Was der Fisch über eine eigene Nachricht weiß, die etwas fragt.
struct Asked {
    ask: Ask,
    /// Die Zeichen, die er selbst daruntergesetzt hat.
    signs: Vec<OwnedEventId>,
}

/// Der Fisch an der Matrix-Tür: sein Konto und die Profile, die er bedient.
struct Fish<M> {
    client: Client,
    me: OwnedUserId,
    served: RwLock<Vec<Entry<M>>>,
    /// Ob die Verbindung steht. Davor kann der Fisch weder mitteilen noch
    /// Räume betreten.
    ready: watch::Sender<bool>,
    /// Die Räume, in denen der Fisch beim letzten Abgleich mit dem Server
    /// schon war. `None` vor dem ersten Abgleich einer leeren Ablage: dann
    /// kann alles, was kommt, alt sein.
    known: std::sync::Mutex<Option<HashSet<OwnedRoomId>>>,
    /// Die Räume, die der Fisch in diesem Lauf betreten hat und die ihm der
    /// Server seither noch nicht gezeigt hat. Was er beim ersten Zeigen
    /// mitbringt, kann alt sein.
    fresh: std::sync::Mutex<HashSet<OwnedRoomId>>,
    /// Die eigenen Nachrichten dieses Laufs, die etwas fragen.
    asked: Mutex<HashMap<OwnedEventId, Asked>>,
    on_change: OnChange,
}

/// Startet die Tür mit den ersten Profilen und läuft, bis die Verbindung
/// endet, das Anhalten verlangt wird oder das letzte Profil angehalten ist.
///
/// `access` ist der Zugang aus der Schlüsselablage, `store` der Ordner mit
/// den Geräteschlüsseln des Fischs. `commands` und `lenient` bedeuten
/// dasselbe wie an der Discord-Tür.
pub async fn run<M>(
    access: &Access,
    store: &Path,
    first: Vec<Profile<M>>,
    lenient: bool,
    mut commands: UnboundedReceiver<Command<M>>,
    on_change: OnChange,
) -> Result<(), String>
where
    M: Model + Send + Sync + 'static,
{
    // Die Ablage öffnen und die Sitzung einstellen; mit dem Server spricht
    // das noch nicht.
    let client = connect(access, store).await?;
    // War der Fisch mit dieser Ablage schon einmal verbunden, kennt sie seine
    // Räume, und der Server reicht nur nach, was neu ist.
    let known: Option<HashSet<OwnedRoomId>> = if client.rooms().is_empty() {
        None
    } else {
        Some(client.joined_rooms().iter().map(|room| room.room_id().to_owned()).collect())
    };
    let fish = Arc::new(Fish {
        me: access.user_id().to_owned(),
        client: client.clone(),
        served: RwLock::new(Vec::new()),
        ready: watch::channel(false).0,
        known: std::sync::Mutex::new(known),
        fresh: std::sync::Mutex::new(HashSet::new()),
        asked: Mutex::new(HashMap::new()),
        on_change,
    });

    // Erst alles prüfen, dann verbinden: passt ein Profil nicht, scheitert
    // der Fisch, bevor er sich bei Matrix zeigt.
    let mut any = false;
    for profile in first {
        let label = label_of(profile.name.as_deref());
        match Fish::start(&fish, profile).await {
            Ok(()) => any = true,
            Err(why) if lenient => eprintln!("rusty-fish: {label}cannot start: {why}"),
            Err(why) => return Err(why),
        }
    }
    if !any {
        return Err("no profile could be started".to_owned());
    }

    client.add_event_handler({
        let fish = fish.clone();
        move |event: OriginalSyncRoomMessageEvent, room: Room| {
            let fish = fish.clone();
            async move { fish.hear(event, room).await }
        }
    });
    client.add_event_handler({
        let fish = fish.clone();
        move |event: OriginalSyncReactionEvent, room: Room| {
            let fish = fish.clone();
            async move { fish.sign_set(event, room).await }
        }
    });
    client.add_event_handler({
        let fish = fish.clone();
        move |event: StrippedRoomMemberEvent, room: Room| {
            let fish = fish.clone();
            async move { fish.invited(event, room).await }
        }
    });
    client.add_event_handler({
        let fish = fish.clone();
        move |event: OriginalSyncRoomEncryptedEvent, room: Room| {
            let fish = fish.clone();
            async move { fish.unreadable(event, room).await }
        }
    });

    // Was dem laufenden Fisch gesagt wird.
    let stop = Arc::new(Notify::new());
    tokio::spawn({
        let fish = fish.clone();
        let stop = stop.clone();
        async move {
            loop {
                match commands.recv().await {
                    Some(Command::Start(profile)) => {
                        let label = label_of(profile.name.as_deref());
                        if let Err(why) = Fish::start(&fish, *profile).await {
                            eprintln!("rusty-fish: {label}cannot start: {why}");
                        }
                    }
                    Some(Command::Restart(profile)) => {
                        let label = label_of(profile.name.as_deref());
                        if let Err(why) = Fish::restart(&fish, *profile).await {
                            eprintln!("rusty-fish: {label}cannot start: {why}");
                        }
                    }
                    Some(Command::Stop(name)) => {
                        if fish.stop(&name).await == 0 {
                            break;
                        }
                    }
                    Some(Command::StopAll) => break,
                    // Niemand kann dem Fisch mehr etwas sagen. Er läuft weiter.
                    None => std::future::pending::<()>().await,
                }
            }
            eprintln!("rusty-fish: stopping the Matrix door");
            stop.notify_one();
        }
    });

    // Je Raum reicht der Server nach einer Pause höchstens so viele
    // Nachrichten nach; ohne Angabe wären es je nach Server nur zehn.
    let mut filter = FilterDefinition::with_lazy_loading();
    filter.room.timeline.limit = Some(UInt::from(CATCH_UP));
    let settings = SyncSettings::default()
        .filter(Filter::FilterDefinition(filter))
        .timeout(Duration::from_secs(30));

    let listening = client.sync_with_result_callback(settings, |result| {
        let fish = fish.clone();
        async move {
            match result {
                Ok(response) => {
                    fish.synced(&response).await;
                    Ok(LoopCtrl::Continue)
                }
                // Der Server kennt den Zugang nicht mehr: weiterzuversuchen
                // hülfe nicht.
                Err(why) if logged_out(&why) => Err(why),
                // Alles andere ist vermutlich das Netz. Die Bibliothek
                // versucht es nach einer Pause wieder.
                Err(why) => {
                    eprintln!("rusty-fish: the Matrix connection stumbled: {why}");
                    Ok(LoopCtrl::Continue)
                }
            }
        }
    });
    tokio::select! {
        ended = listening => match ended {
            Ok(()) => Ok(()),
            Err(why) if logged_out(&why) => Err(
                "the Matrix server no longer accepts the stored access; log in to Matrix again"
                    .to_owned(),
            ),
            Err(why) => Err(format!("the Matrix connection ended: {why}")),
        },
        // Nach einem gewollten Anhalten ist das Ende kein Fehler.
        () = stop.notified() => Ok(()),
    }
}

/// Sagt dieser Fehler, dass der Server den Zugang nicht mehr kennt?
fn logged_out(error: &matrix_sdk::Error) -> bool {
    use matrix_sdk::ruma::api::error::ErrorKind;
    matches!(error.client_api_error_kind(), Some(ErrorKind::UnknownToken { .. }))
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl<M> Fish<M>
where
    M: Model + Send + Sync + 'static,
{
    /// Nimmt ein Profil in den Lauf. Steht die Verbindung schon, betritt der
    /// Fisch gleich die Räume, in die er eingeladen ist, und teilt mit, wer
    /// eingeladen ist; sonst geschieht beides, sobald sie steht.
    async fn start(fish: &Arc<Fish<M>>, profile: Profile<M>) -> Result<(), String> {
        if profile.config.door != Door::Matrix {
            return Err(format!(
                "this profile stands at the {} door, not at Matrix",
                profile.config.door.label()
            ));
        }
        let house = Household::open(&profile.config, &profile.state_path)?;
        let served = Arc::new(Served {
            name: profile.name,
            model: profile.model,
            config: profile.config,
            service: profile.service,
            since: now_ms(),
            entered: std::sync::Mutex::new(HashMap::new()),
            inner: Mutex::new(Inner {
                house,
                handled: HashSet::new(),
                handled_order: VecDeque::new(),
            }),
        });
        let (queue, heard) = unbounded_channel();
        {
            let mut list = fish.served.write().await;
            for other in list.iter().map(|entry| &entry.served) {
                if served.name.is_some() && other.name == served.name {
                    return Err("this profile is already running".to_owned());
                }
                // Ein Raum in zwei Profilen: jede Nachricht würde doppelt
                // übersetzt, doppelt zugestellt und doppelt bezahlt.
                if let Some(room) = other.config.shared_channel(&served.config) {
                    return Err(format!(
                        "room {room} already belongs to {}, which is running",
                        title_of(other.name.as_deref())
                    ));
                }
            }
            list.push(Entry { served: served.clone(), queue });
            fish.told(&list);
        }
        served.say("serving");
        tokio::spawn(work(fish.clone(), served, heard));
        Ok(())
    }

    /// Nimmt ein Profil neu auf: das laufende gleichen Namens geht, dieses
    /// kommt, mit frisch gelesenen Einstellungen und frisch geöffnetem
    /// Zustand. Lässt sich das neue nicht aufnehmen, läuft danach keines von
    /// beiden, wie an der Discord-Tür.
    async fn restart(fish: &Arc<Fish<M>>, profile: Profile<M>) -> Result<(), String> {
        if let Some(name) = profile.name.as_deref() {
            let mut list = fish.served.write().await;
            list.retain(|entry| entry.served.name.as_deref() != Some(name));
        }
        let started = Fish::start(fish, profile).await;
        if started.is_err() {
            let list = fish.served.read().await;
            fish.told(&list);
        }
        started
    }

    /// Nimmt ein Profil aus dem Lauf. Liefert, wie viele noch laufen.
    async fn stop(&self, name: &str) -> usize {
        let mut list = self.served.write().await;
        let before = list.len();
        list.retain(|entry| entry.served.name.as_deref() != Some(name));
        if list.len() < before {
            eprintln!("rusty-fish: {}no longer serving", label_of(Some(name)));
            self.told(&list);
        } else {
            eprintln!("rusty-fish: {}was not running at the Matrix door", label_of(Some(name)));
        }
        list.len()
    }

    /// Sagt dem Aufrufer, welche Profile jetzt laufen.
    fn told(&self, list: &[Entry<M>]) {
        let names: Vec<String> = list.iter().filter_map(|e| e.served.name.clone()).collect();
        (self.on_change)(&names);
    }

    /// Das Profil, zu dem dieser Raum gehört, und seine Warteschlange.
    async fn at(&self, room: &RoomId) -> Option<(Arc<Served<M>>, UnboundedSender<Heard>)> {
        let list = self.served.read().await;
        list.iter()
            .find(|entry| entry.served.config.room_index(room.as_str()).is_some())
            .map(|entry| (entry.served.clone(), entry.queue.clone()))
    }

    /// Das Profil, zu dem ein Hinweis zum leeren Kontingent gehört. Ohne
    /// Namen am Hinweis nur dann eines, wenn überhaupt nur eines läuft.
    async fn named(&self, name: Option<&str>) -> Option<Arc<Served<M>>> {
        let list = self.served.read().await;
        match name {
            Some(name) => list
                .iter()
                .find(|entry| entry.served.name.as_deref() == Some(name))
                .map(|entry| entry.served.clone()),
            None if list.len() == 1 => list.first().map(|entry| entry.served.clone()),
            None => None,
        }
    }

    /// Kann, was jetzt aus diesem Raum kommt, alt sein?
    fn maybe_old(&self, room: &RoomId) -> bool {
        if self.fresh.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).contains(room) {
            return true;
        }
        let known = self.known.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        known.as_ref().is_none_or(|rooms| !rooms.contains(room))
    }

    /// Ein Abgleich mit dem Server ist durch.
    async fn synced(&self, response: &SyncResponse) {
        // Was der Server nicht mehr nachgereicht hat, soll wenigstens
        // dastehen: der Fisch hat dann etwas verschluckt.
        for (room, update) in &response.rooms.joined {
            if update.timeline.limited
                && !self.maybe_old(room)
                && let Some((served, _)) = self.at(room).await
            {
                served.say(format_args!(
                    "more than {CATCH_UP} messages were missed in room {room}; the oldest are \
                     not caught up"
                ));
            }
        }
        // Ein eben betretener Raum ist jetzt gezeigt worden; was von nun an
        // aus ihm kommt, ist neu.
        {
            let mut fresh = self.fresh.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            for room in response.rooms.joined.keys() {
                fresh.remove(room);
            }
        }
        let joined: HashSet<OwnedRoomId> =
            self.client.joined_rooms().iter().map(|room| room.room_id().to_owned()).collect();
        *self.known.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(joined);
        if !*self.ready.borrow() {
            eprintln!("rusty-fish: connected to Matrix as {}", self.me);
            self.ready.send_replace(true);
        }
    }

    /// Jemand hat in einem Raum geschrieben.
    async fn hear(&self, event: OriginalSyncRoomMessageEvent, room: Room) {
        // Was der Fisch selbst abgesetzt hat, darf nicht wieder hereinkommen.
        if event.sender == self.me {
            return;
        }
        // Ein Raum außerhalb jedes Profils geht den Fisch nichts an.
        let Some((_, queue)) = self.at(room.room_id()).await else { return };
        // Eine Bearbeitung ist keine neue Nachricht.
        if matches!(event.content.relates_to, Some(Relation::Replacement(_))) {
            return;
        }
        // Nur Text. Eine Notiz kommt von einem Bot, auch von einem anderen
        // Fisch; Bilder und Dateien spiegelt diese Fassung nicht.
        let MessageType::Text(text) = &event.content.msgtype else { return };
        let body = match event.content.relates_to {
            Some(Relation::Reply { .. }) => without_quote(&text.body),
            _ => text.body.as_str(),
        };
        let heard = Heard {
            room: room.room_id().to_owned(),
            event: event.event_id,
            sender: event.sender,
            at: u64::from(event.origin_server_ts.0),
            text: body.to_owned(),
            mentioned: event
                .content
                .mentions
                .as_ref()
                .is_some_and(|mentions| mentions.user_ids.contains(&self.me)),
            maybe_old: self.maybe_old(room.room_id()),
        };
        // Der Abgleich mit dem Server wartet nicht auf die Übersetzung: die
        // Nachricht kommt in die Warteschlange ihres Profils.
        let _ = queue.send(heard);
    }

    /// Jemand hat ein Zeichen unter eine Nachricht gesetzt.
    async fn sign_set(&self, event: OriginalSyncReactionEvent, room: Room) {
        if event.sender == self.me {
            return;
        }
        let target = &event.content.relates_to.event_id;
        let key = event.content.relates_to.key.as_str();
        // Menschen setzen ständig Zeichen unter Nachrichten. Beim Server
        // nachgesehen wird nur, wo es eine Antwort an den Fisch sein kann:
        // ein „OK" in einem Raum eines Profils, oder ein Zeichen eines
        // Gastgebers außerhalb davon (der Hinweis zum Kontingent steht in
        // einem Raum zu zweit).
        let look_up = if self.at(room.room_id()).await.is_some() {
            key.trim() == CONSENT_KEY
        } else {
            let list = self.served.read().await;
            list.iter().any(|entry| entry.served.config.host == event.sender.as_str())
        };
        let Some(ask) = self.asked_by(&room, target, look_up).await else { return };
        let at = u64::from(event.origin_server_ts.0);
        match &ask {
            Ask::Consent { .. } => {
                // Die Bitte steht in einem Raum, und der gehört zu einem Profil.
                let Some((served, _)) = self.at(room.room_id()).await else { return };
                // Ein altes Zeichen, das der Server nur noch einmal zeigt,
                // ist keine neue Zustimmung.
                if self.maybe_old(room.room_id()) && at <= served.since {
                    return;
                }
                if ask.agrees(key, &served.service) {
                    served.consent_given(self, &event.sender).await;
                }
            }
            Ask::Budget { profile, .. } => {
                let Some(choice) = ask.budget_choice(key) else { return };
                let Some(served) = self.named(profile.as_deref()).await else {
                    eprintln!(
                        "rusty-fish: a sign was set on a budget notice, but its profile is not \
                         running"
                    );
                    return;
                };
                if self.maybe_old(room.room_id()) && at <= served.since {
                    return;
                }
                // Nur das Zeichen des Gastgebers zählt: es ist sein Schlüssel
                // und sein Geld.
                if event.sender.as_str() != served.config.host {
                    served.say("someone who is not the host set a sign on the budget notice");
                    return;
                }
                served.budget_chosen(self, &room, target, choice).await;
            }
        }
    }

    /// Was die Nachricht fragt, auf die ein Zeichen zeigt, falls sie vom
    /// Fisch stammt und etwas fragt. Aus dem Gedächtnis dieses Laufs, sonst
    /// aus der Nachricht selbst: so gilt eine Bitte auch nach einem Neustart.
    async fn asked_by(&self, room: &Room, target: &EventId, look_up: bool) -> Option<Ask> {
        if let Some(asked) = self.asked.lock().await.get(target) {
            return Some(asked.ask.clone());
        }
        if !look_up {
            return None;
        }
        let event = room.event(target, None).await.ok()?;
        let event: serde_json::Value = event.raw().deserialize_as().ok()?;
        if event.get("sender")?.as_str()? != self.me.as_str() {
            return None;
        }
        Ask::from_content(event.get("content")?)
    }

    /// Der Fisch ist in einen Raum eingeladen worden. Er betritt ihn, wenn er
    /// ein Tisch eines laufenden Profils ist, und sonst nicht.
    async fn invited(&self, event: StrippedRoomMemberEvent, room: Room) {
        if event.state_key != self.me || room.state() != RoomState::Invited {
            return;
        }
        match self.at(room.room_id()).await {
            Some((served, _)) => served.enter(self, &room).await,
            None => eprintln!(
                "rusty-fish: invited to room {}, which is no table of a running profile; not \
                 joining",
                room.room_id()
            ),
        }
    }

    /// Eine Nachricht, die der Fisch nicht entschlüsseln konnte.
    async fn unreadable(&self, event: OriginalSyncRoomEncryptedEvent, room: Room) {
        if event.sender == self.me || self.maybe_old(room.room_id()) {
            return;
        }
        if let Some((served, _)) = self.at(room.room_id()).await {
            served.say(format_args!(
                "cannot decrypt a message in room {} from {}: its sender's app did not give the \
                 fish the key",
                room.room_id(),
                event.sender
            ));
        }
    }

    /// Der Raum mit dieser Kennung, wenn der Fisch darin ist.
    fn room(&self, channel: &str) -> Result<Room, String> {
        let id = RoomId::parse(channel).map_err(|_| "not a room id".to_owned())?;
        match self.client.get_room(&id) {
            Some(room) if room.state() == RoomState::Joined => Ok(room),
            _ => Err(format!("the fish is not in this room; invite {} there", self.me)),
        }
    }

    /// Setzt eine Nachricht des Fischs ab. Fragt sie etwas, setzt er die
    /// Zeichen darunter, die man antippt, und merkt sie sich.
    async fn post(&self, room: &Room, text: &Rendered, ask: Option<Ask>) -> Result<(), String> {
        let content = notice(&text.body, &text.html, ask.as_ref());
        let sent = room.send_raw("m.room.message", content).await.map_err(|e| e.to_string())?;
        let Some(ask) = ask else { return Ok(()) };
        let event = sent.response.event_id;
        let mut signs = Vec::new();
        for key in ask.keys() {
            match room.send_raw("m.reaction", reaction(event.as_str(), key)).await {
                Ok(sent) => signs.push(sent.response.event_id),
                Err(why) => return Err(format!("the request is there, but not its sign: {why}")),
            }
        }
        self.asked.lock().await.insert(event, Asked { ask, signs });
        Ok(())
    }

    /// Der Name, unter dem jemand in diesem Raum erscheint.
    async fn name_in(&self, room: &Room, user: &UserId) -> String {
        match room.get_member_no_sync(user).await {
            Ok(Some(member)) => member.name().to_owned(),
            _ => user.localpart().to_owned(),
        }
    }
}

/// Arbeitet ab, was in den Räumen eines Profils geschrieben wurde, eines nach
/// dem anderen, bis das Profil die Liste verlässt.
async fn work<M>(fish: Arc<Fish<M>>, served: Arc<Served<M>>, mut heard: UnboundedReceiver<Heard>)
where
    M: Model + Send + Sync + 'static,
{
    // Erst wenn die Verbindung steht. Bis dahin sammelt sich, was der Server
    // nachreicht.
    let mut ready = fish.ready.subscribe();
    if ready.wait_for(|ready| *ready).await.is_err() {
        return;
    }
    // Erst die Räume und die Mitteilung, dann das Nachgeholte: wer eingeladen
    // ist, soll lesen, was der Fisch hier tut, bevor die ersten Übersetzungen
    // kommen.
    served.settle(&fish).await;

    let mut batch = Vec::new();
    while let Some(first) = heard.recv().await {
        batch.push(first);
        while let Ok(more) = heard.try_recv() {
            batch.push(more);
        }
        // Der Server reicht Raum für Raum nach. Über alle Räume hinweg gilt
        // die Reihenfolge, in der geschrieben wurde.
        batch.sort_by_key(|heard| heard.at);
        if batch.len() > 1 {
            served.say(format_args!("{} messages waiting", batch.len()));
        }
        for heard in batch.drain(..) {
            served.handle(&fish, heard).await;
        }
    }
}

impl<M> Served<M>
where
    M: Model + Send + Sync + 'static,
{
    /// Eine Zeile in die Programmausgabe, mit dem Namen des Profils davor.
    /// Kennungen und Fehlergründe, nie der Text einer Nachricht.
    fn say(&self, text: impl std::fmt::Display) {
        eprintln!("rusty-fish: {}{text}", label_of(self.name.as_deref()));
    }

    /// Was für einen Aufruf des Haushalts feststeht.
    fn terms<'a>(&'a self, today: &'a str) -> Terms<'a> {
        Terms { config: &self.config, service: &self.service, today }
    }

    /// Aus dem ersten echten Lauf am 22.11.2026: im Profil standen die
    /// Kennungen zweier Spaces. Ein Space ist für Matrix auch ein Raum, der
    /// Fisch trat ein und meldete nichts; schreiben kann dort niemand.
    fn not_a_table(&self, room: &RoomId) {
        self.say(format_args!(
            "{room} is a space, not a room: nobody can write there; put the id of a room in its \
             place"
        ));
    }

    /// Betritt einen Raum, in den der Fisch eingeladen ist. Gleich nach der
    /// Einladung kennt der Server den Raum manchmal noch nicht ganz; deshalb
    /// mehr als ein Versuch.
    async fn enter(&self, fish: &Fish<M>, room: &Room) {
        if room.is_space() {
            self.not_a_table(room.room_id());
            return;
        }
        // Vor der Bitte gemerkt: was ab jetzt geschrieben wird, gilt.
        let asked = now_ms();
        // Bis der Server den Raum gezeigt hat, kann alles aus ihm alt sein.
        let fresh = |is: bool| {
            let mut fresh = fish.fresh.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if is {
                fresh.insert(room.room_id().to_owned());
            } else {
                fresh.remove(room.room_id());
            }
        };
        fresh(true);
        let mut wait = Duration::from_secs(1);
        for attempt in 1..=3 {
            match room.join().await {
                Ok(()) => {
                    self.entered
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .insert(room.room_id().to_owned(), asked);
                    self.say(format_args!("joined room {}", room.room_id()));
                    return;
                }
                Err(why) if attempt == 3 => {
                    fresh(false);
                    self.say(format_args!("cannot join room {}: {why}", room.room_id()));
                }
                Err(_) => {
                    tokio::time::sleep(wait).await;
                    wait *= 2;
                }
            }
        }
    }

    /// Die Verbindung steht: die Räume dieses Profils betreten, in die der
    /// Fisch eingeladen ist, sagen, in welchen er fehlt, und Eingeladenen
    /// mitteilen, dass er sie bedient.
    async fn settle(&self, fish: &Fish<M>) {
        for seat in self.config.rooms.iter().flat_map(|room| room.seats.iter()) {
            let Ok(id) = RoomId::parse(&seat.channel) else { continue };
            match fish.client.get_room(&id) {
                Some(room) if room.is_space() => self.not_a_table(room.room_id()),
                Some(room) if room.state() == RoomState::Joined => {}
                Some(room) if room.state() == RoomState::Invited => self.enter(fish, &room).await,
                _ => self.say(format_args!(
                    "the fish is not in room {}; invite {} there",
                    seat.channel, fish.me
                )),
            }
        }
        self.tell_the_invited(fish).await;
    }

    /// Teilt Eingeladenen an ihrem Tisch mit, dass dieser Fisch sie jetzt
    /// bedient: einmal je Mensch, gemerkt in der Zustandsdatei.
    async fn tell_the_invited(&self, fish: &Fish<M>) {
        let mut inner = self.inner.lock().await;
        // Nichts fällig und niemand zu vergessen: die Datei bleibt, wie sie ist.
        let Some(pending) = inner.house.invitations(&self.config) else { return };

        let mut reached = Vec::new();
        for invite in &pending.due {
            let sent = match fish.room(&invite.channel) {
                Ok(room) => {
                    let host = match UserId::parse(&self.config.host) {
                        Ok(host) => Some(fish.name_in(&room, &host).await),
                        Err(_) => None,
                    };
                    let text = invitation_at(
                        Door::Matrix,
                        self.config.tongue_at(&invite.channel),
                        host.as_deref(),
                    );
                    fish.post(&room, &plain(&text), None).await
                }
                Err(why) => Err(why),
            };
            match sent {
                Ok(()) => {
                    reached.push(invite.person.clone());
                    self.say(format_args!("invitation announced in room {}", invite.channel));
                }
                // Nicht gemerkt: der nächste Start versucht es wieder.
                Err(why) => self.say(format_args!(
                    "cannot announce the invitation in room {}: {why}",
                    invite.channel
                )),
            }
        }

        if let Err(why) = inner.house.invitations_told(pending, &reached) {
            self.say(why);
        }
    }

    /// Eine Nachricht aus einem Raum dieses Profils: an alle anderen Plätze
    /// des Raums verteilen.
    async fn handle(&self, fish: &Fish<M>, heard: Heard) {
        let channel = heard.room.as_str();
        let mut guard = self.inner.lock().await;
        let inner = &mut *guard;

        // Dieselbe Nachricht ein zweites Mal gemeldet.
        if inner.handled.contains(&heard.event) {
            return;
        }
        let seen = inner.house.seen(channel);
        // Wo der Fisch in einem Raum ohne Lesemarke beginnt: bei seinem
        // Eintritt, wenn er ihn in diesem Lauf betreten hat, sonst beim Start
        // dieses Profils.
        let begun = self
            .entered
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&heard.room)
            .copied()
            .unwrap_or(self.since);
        if heard.maybe_old && heard.at <= seen.unwrap_or(begun) {
            return;
        }
        // Matrix zählt nicht; die Tür tut es. Die Zeit der Nachricht, aber
        // immer über der Lesemarke: zwei Nachrichten in derselben
        // Millisekunde oder eine von einem Server mit nachgehender Uhr sollen
        // nicht als schon gelesen gelten.
        let mark = heard.at.max(seen.map_or(0, |seen| seen + 1));

        let Ok(here) = fish.room(channel) else {
            self.say(format_args!("heard a message in room {channel}, but the fish is not in it"));
            return;
        };
        // Die Erwähnung des Fischs am Anfang zählt wie sein Rufname. Der
        // Haushalt bekommt dafür den Rufnamen; so sehen weder die KI noch die
        // anderen Plätze die Kennung des Fischs. Ohne Rufnamen ist die
        // Funktion aus, und die Nachricht bleibt, wie sie ist.
        let called = match self.config.ai_name() {
            Some(name) => {
                let shown = fish.name_in(&here, &fish.me).await;
                let own = [shown.as_str(), fish.me.localpart(), fish.me.as_str()];
                mention_as_call(&heard.text, heard.mentioned, &own, name)
            }
            None => None,
        };
        let arrived = Arrived {
            channel,
            author: heard.sender.as_str(),
            text: called.as_deref().unwrap_or(&heard.text),
            mark,
        };
        let today = today();
        let Some(steps) = inner.house.message(&self.model, &self.terms(&today), &arrived).await
        else {
            return;
        };
        inner.remember(heard.event.clone());

        let writer = fish.name_in(&here, &heard.sender).await;
        let delivered = self.deliver(fish, &steps.deliveries, &writer).await;
        // Kam etwas nicht an, soll der Schreibende es an seiner Nachricht sehen.
        if !delivered {
            let _ = here.send_raw("m.reaction", reaction(heard.event.as_str(), "⚠")).await;
        }

        // Die Bitte um Zustimmung geht an den Tisch des Schreibenden.
        if let Some(text) = &steps.ask_consent {
            let ask = Ask::Consent { service: self.service.clone() };
            if let Err(why) = fish.post(&here, &plain(text), Some(ask)).await {
                self.say(format_args!("cannot ask for consent in room {channel}: {why}"));
            }
        }

        // Ist das Kontingent leer, erfährt es der Gastgeber.
        if let Some(notice) = &steps.tell_host {
            self.notify_host(fish, notice).await;
        }

        // Erst jetzt, nach dem Absetzen: stürzt der Fisch vorher ab, gilt die
        // Nachricht als nicht gelesen.
        if let Err(why) = inner.house.save() {
            self.say(why);
        }
    }

    /// Setzt Zustellungen ab: was ein Mensch schrieb, mit seinem Namen davor;
    /// was die KI sagt, mit dem Namen, den der Gastgeber ihr gab. Liefert, ob
    /// alles ankam.
    async fn deliver(&self, fish: &Fish<M>, deliveries: &[Delivery], writer: &str) -> bool {
        let assistant = self.config.ai_name().unwrap_or(FISH_NAME);
        let mut delivered = true;
        for delivery in deliveries {
            let name = match delivery.from {
                Sender::Writer => writer,
                Sender::Assistant => assistant,
            };
            // Die Kennzeichnung steht in der Sprache des Tisches, an dem sie
            // erscheint, nicht in der des Tisches, an dem geschrieben wurde.
            let parts = render_in(delivery, self.config.tongue_at(&delivery.channel), name);
            let sent = match fish.room(&delivery.channel) {
                Ok(room) => {
                    let mut sent = Ok(());
                    for part in &parts {
                        sent = fish.post(&room, part, None).await;
                        if sent.is_err() {
                            break;
                        }
                    }
                    sent
                }
                Err(why) => Err(why),
            };
            if let Err(why) = sent {
                delivered = false;
                self.say(format_args!("cannot deliver to room {}: {why}", delivery.channel));
            }
        }
        delivered
    }

    /// Jemand hat das Zeichen unter der Bitte um Zustimmung gesetzt.
    async fn consent_given(&self, fish: &Fish<M>, user: &UserId) {
        // Wer das Zeichen setzt, stimmt für sich selbst zu, für niemanden
        // sonst. Das Schloss bleibt bis zum Ende: was jetzt nachgereicht wird,
        // soll vor dem stehen, was derselbe Mensch gleich danach schreibt.
        let mut guard = self.inner.lock().await;
        let inner = &mut *guard;
        match inner.house.consent(user.as_str(), &self.service) {
            Ok(()) => self.say("consent stored"),
            Err(why) => {
                self.say(why);
                return;
            }
        }

        // Was dieser Mensch davor schrieb, kam unübersetzt an. Jetzt wird es
        // übersetzt nachgereicht.
        let today = today();
        let held = inner.house.held_back(&self.model, &self.terms(&today), user.as_str()).await;
        if held.is_empty() {
            return;
        }
        // Nur die Zahl, kein Text.
        self.say(format_args!("handing in {} delivery(ies) written before consent", held.len()));
        // Sein Name, wie er an einem der Tische erscheint, an die es geht.
        let mut writer = user.localpart().to_owned();
        if let Some(room) = held.first().and_then(|d| fish.room(&d.channel).ok()) {
            writer = fish.name_in(&room, user).await;
        }
        self.deliver(fish, &held, &shown_name(&writer)).await;
        if let Err(why) = inner.house.save() {
            self.say(why);
        }
    }

    /// Sagt dem Gastgeber, und nur ihm, dass das heutige Kontingent verbraucht
    /// ist, und fragt, ob er um denselben Betrag erhöhen will: in einem Raum
    /// zu zweit, mit zwei Zeichen zum Antippen.
    async fn notify_host(&self, fish: &Fish<M>, notice: &HostNotice) {
        let Ok(host) = UserId::parse(&self.config.host) else { return };
        let room = match fish.client.get_dm_room(&host) {
            Some(room) if room.state() == RoomState::Joined => Ok(room),
            _ => fish.client.create_dm(&host).await.map_err(|e| e.to_string()),
        };
        let name = self.name.as_deref();
        // Laufen mehrere Profile, muss der Gastgeber wissen, welches leer ist.
        let text = match name {
            Some(name) => Rendered {
                body: format!("{}\n(Profile: {name})", notice.text),
                html: format!(
                    "{}<br><em>Profile: {}</em>",
                    crate::render::escape(&notice.text),
                    crate::render::escape(name)
                ),
            },
            None => plain(&notice.text),
        };
        let ask = Ask::Budget {
            profile: name.map(str::to_owned),
            raise: notice.raise_label.clone(),
            keep: notice.keep_label.to_owned(),
        };
        let sent = match room {
            Ok(room) => fish.post(&room, &text, Some(ask)).await,
            Err(why) => Err(why),
        };
        if let Err(why) = sent {
            self.say(format_args!("cannot tell the host that the budget is used up: {why}"));
        }
    }

    /// Der Gastgeber hat am Hinweis zum leeren Kontingent gewählt.
    async fn budget_chosen(&self, fish: &Fish<M>, room: &Room, notice: &EventId, choice: Budget) {
        let text = match choice {
            Budget::Keep => KEPT_NOTICE.to_owned(),
            Budget::Raise => {
                let mut inner = self.inner.lock().await;
                let today = today();
                let (text, saved) = inner.house.raise(&self.terms(&today));
                if let Err(why) = saved {
                    self.say(why);
                }
                text
            }
        };
        if let Err(why) = fish.post(room, &plain(&text), None).await {
            self.say(format_args!("cannot answer the host: {why}"));
        }
        // Die eigenen Zeichen unter dem Hinweis wegnehmen, damit kein altes
        // stehen bleibt. Nur die aus diesem Lauf sind bekannt.
        let signs = fish.asked.lock().await.remove(notice).map(|asked| asked.signs);
        for sign in signs.into_iter().flatten() {
            let _ = room.redact(&sign, None, None).await;
        }
    }
}
