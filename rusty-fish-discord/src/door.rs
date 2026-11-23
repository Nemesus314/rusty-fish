//! Der Anschluss an Discord.
//!
//! Der Fisch meldet sich als Bot an und wählt sich dabei von innen bei
//! Discord ein. Er braucht keinen offenen Port und keinen Server, ein
//! gewöhnlicher PC genügt, solange er läuft.
//!
//! Benannte Grenzen dieser ersten Fassung:
//! - Bearbeitete und gelöschte Nachrichten, Reaktionen und Antwort-Bezüge
//!   werden nicht gespiegelt.
//! - Sticker und Einbettungen ohne Text kommen am anderen Platz nicht an.
//!   Anhänge kommen als Adresse an.
//! - Die Zustimmung lässt sich noch nicht im Chat widerrufen.
//! - Die KI hat ein eigenes Bild nur, wenn Discord dem Webhook eines gibt; sie
//!   erscheint unter ihrem Namen mit dem Standardbild.
//! - Der Hinweis an den Gastgeber bei leerem Kontingent geht als
//!   Direktnachricht hinaus. Scheitert sie (etwa weil der Gastgeber keine
//!   Direktnachrichten von Servermitgliedern annimmt), steht der Grund nur in
//!   der Programmausgabe, und es gibt keinen zweiten Versuch vor der nächsten
//!   Erhöhung oder dem nächsten Tag. Der Hinweis ist englisch und wird nicht
//!   übersetzt.
//!
//! - Als Ansprache zählt nur die Erwähnung des Bots selbst am Anfang der
//!   Nachricht (`<@ID>`). Discord legt für einen Bot meist auch eine Rolle
//!   gleichen Namens an; wer in der `@`-Liste die Rolle statt des Bots wählt,
//!   schreibt `<@&ID>` und spricht ihn damit nicht an.
//!
//! Seit dem 20.11.2026 liest der Fisch an allen Tischen eines Raums mit,
//! schreibt aber nur an die, die er bedient (siehe `config::at_the_table`).
//! So kann jeder seinen eigenen Fisch mitbringen. Benannte Grenzen dabei:
//! - Die Fische kennen einander nicht. Was ein Fisch absetzt, überliest jeder
//!   andere wie alles, was von einem Bot oder Webhook kommt. Die Antwort der
//!   ansprechbaren KI erscheint deshalb nur an den Tischen des Fischs, der sie
//!   gab, und fehlt den anderen Fischen als Kontext.
//! - Jeder Fisch bittet für sich um Zustimmung, denn jeder gibt die Sätze an
//!   seinen eigenen KI-Dienst. Wer in einem Raum mit zwei Fischen schreibt,
//!   wird zweimal gefragt.
//! - Die Mitteilung an Eingeladene geht beim Verbinden hinaus, nicht erst mit
//!   der ersten Übersetzung. Scheitert sie, steht der Grund in der
//!   Programmausgabe, und der nächste Start versucht es wieder.
//!
//! Das Kontingent und die ansprechbare KI stehen in `rusty-fish-room`. Was
//! nach einer Nachricht zu geschehen hat, entscheidet seit dem 21.11.2026 der
//! Haushalt (`rusty-fish-house`); er führt auch die Zustandsdatei. Die Tür
//! reicht herein, was ankam, setzt ab, was er entschieden hat, und lässt
//! danach speichern.
//!
//! Seit dem 21.11.2026 bedient ein Fisch mehrere Profile über eine einzige
//! Verbindung. Nemesus an dem Tag: „Wieso kann ich eigentlich nicht alle
//! Profile gleichzeitig laufen lassen, sondern muss mich für einen
//! entscheiden?" Bis dahin war ein Profil ein eigenes Programm, und zwei
//! Programme mit demselben Bot hätten jede Nachricht und jeden Klick doppelt
//! gesehen. Jetzt gibt es eine Verbindung, und jede Nachricht und jeder Klick
//! wird dem Profil zugeordnet, in dessen Kanal er fällt. Jedes Profil hat
//! seinen eigenen Haushalt, seinen eigenen KI-Dienst und sein eigenes Schloss:
//! eine lange Übersetzung in einem Profil hält die anderen nicht auf.
//! Benannte Grenzen dabei:
//! - Ein Kanal gehört zu höchstens einem laufenden Profil. Ein zweites, das
//!   denselben Kanal nennt, wird nicht gestartet.
//! - Alle Profile eines Fischs teilen sich einen Bot.
//!
//! Seit dem 23.11.2026 kann ein Profil auch Tische an einer anderen Tür haben
//! (der gemischte Raum, siehe `rusty_fish_house::between`). Dann teilt diese
//! Tür den Haushalt des Profils mit der anderen. Was an einem Tisch der
//! anderen Tür erscheinen soll, reicht sie ihr zu; was die andere für einen
//! Kanal dieser Tür hat, setzt sie ab. Benannte Grenzen dabei:
//! - Wer an der anderen Tür schrieb, erscheint hier mit ihr hinter dem Namen,
//!   etwa „Robin (Matrix)", und mit dem Standardbild des Webhooks.
//! - Nachgeholt werden nur die Kanäle dieser Tür.
//!
//! In die Ausgabe des Programms gelangen nur Kanal-Kennungen und Fehlergründe,
//! nie der Text einer Nachricht.

use crate::buttons::{Budget, CONSENT, budget_id, budget_of};
use crate::mention::mention_as_call;
use crate::render::{MESSAGE_LIMIT, chunks, hook_name, hook_name_from, render_in};
use rusty_fish_core::Model;
use rusty_fish_house::between::{Errand, Handed, Lines, Writer, by_door};
use rusty_fish_house::clock::today;
use rusty_fish_house::config::{Config, Door};
use rusty_fish_house::household::{Arrived, HostNotice, Household, Terms};
use rusty_fish_house::served::{label_of, title_of};
use rusty_fish_house::texts::{KEPT_NOTICE, NOT_THE_HOST, invitation};
use rusty_fish_room::{Delivery, Sender};
use serenity::all::{
    ButtonStyle, Channel, ChannelId, ClientBuilder, ComponentInteraction, Context, CreateAllowedMentions,
    CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage,
    CreateWebhook, EditInteractionResponse, EventHandler, ExecuteWebhook, GatewayIntents,
    GetMessages, Http, Interaction, Message, MessageId, Ready, UserId, Webhook,
};
use serenity::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::{Mutex, RwLock};

// Was der Aufrufer übergibt und sagen kann, gilt für jede Tür und steht
// seit dem 22.11.2026 im Haus. Unter diesen Namen bleibt es hier erreichbar.
pub use rusty_fish_house::served::{Command, OnChange, Profile};

const WEBHOOK_NAME: &str = "Rusty Fish";

/// Die Antwort auf einen Klick, zu dem gerade kein Profil läuft: der Knopf
/// stammt von einem Profil, das inzwischen angehalten wurde.
const NOT_RUNNING: &str = "The profile this button belongs to is not running right now.";

/// Ein Profil im Lauf: seine Einstellungen, seine KI und alles, was sich
/// ändert.
struct Served<M> {
    name: Option<String>,
    model: M,
    config: Config,
    /// Der Name des KI-Dienstes, wie er in der Bitte um Zustimmung steht.
    service: String,
    /// Was der Fisch sich merkt, und was nach einer Nachricht zu geschehen
    /// hat. Sein Schloss hält die Nachrichten eines Profils in der
    /// Reihenfolge, in der sie geschrieben wurden: Discord meldet sie
    /// gleichzeitig, das Schloss lässt sie einzeln durch. Steht das Profil
    /// auch an einer anderen Tür, ist es derselbe Haushalt wie dort.
    house: Arc<Mutex<Household>>,
    /// Die Leitungen zu den anderen Türen, wenn das Profil auch dort steht.
    lines: Option<Arc<Lines>>,
    /// Die Webhooks der Kanäle, in die der Fisch schon geschrieben hat. Ein
    /// eigenes Schloss: auch was eine andere Tür zureicht, wird hier
    /// abgesetzt, während jene das Schloss des Haushalts hält.
    hooks: Mutex<HashMap<ChannelId, Webhook>>,
}

/// Der Fisch an der Discord-Tür: die Profile, die er gerade bedient.
struct Fish<M> {
    served: RwLock<Vec<Arc<Served<M>>>>,
    /// Die Benutzer-ID des Bots selbst, bekannt ab dem Ready-Ereignis. Null
    /// heißt: noch nicht bekannt. Gebraucht wird sie, um die echte Erwähnung
    /// des Bots am Anfang einer Nachricht zu erkennen.
    me: AtomicU64,
    /// Ob die Verbindung steht. Davor kann der Fisch weder mitteilen noch
    /// nachholen; das geschieht dann für alle Profile auf einmal.
    ready: AtomicBool,
    on_change: OnChange,
}

/// Was Discord meldet, geht an den Fisch.
struct Handler<M>(Arc<Fish<M>>);

/// Startet die Tür mit den ersten Profilen und läuft, bis die Verbindung
/// endet, das Anhalten verlangt wird oder das letzte Profil angehalten ist.
///
/// `commands` kommt vom Aufrufer: wann ein Programm anhalten soll und welche
/// Profile es bedient, weiß das Programm, nicht eine seiner Türen. Endet der
/// Kanal, ohne dass jemand das Anhalten verlangt hat, läuft der Fisch weiter:
/// als Dienst ohne Terminal hat er niemanden, der ihm etwas sagt.
///
/// `lenient` entscheidet, was geschieht, wenn eines der ersten Profile nicht
/// aufgenommen werden kann. Streng (`false`): der Fisch startet nicht, denn
/// der Aufrufer hat genau diese Profile verlangt. Nachsichtig (`true`): das
/// Profil bleibt weg und sagt, warum, und die anderen laufen; so startet
/// „alle Profile". Kann keines aufgenommen werden, startet der Fisch in
/// beiden Fällen nicht.
///
/// Über `handed` kommt, was eine andere Tür dieser zureicht (siehe
/// `rusty_fish_house::between`); wer nur diese eine Tür betreibt, übergibt
/// eine Leitung, an deren anderem Ende niemand ist.
pub async fn run<M>(
    token: &str,
    first: Vec<Profile<M>>,
    lenient: bool,
    commands: UnboundedReceiver<Command<M>>,
    handed: UnboundedReceiver<Handed>,
    on_change: OnChange,
) -> Result<(), String>
where
    M: Model + Send + Sync + 'static,
{
    run_with(Http::new(token), first, lenient, commands, handed, on_change).await
}

/// Wie [`run`], mit einem fertigen Zugang zur Schnittstelle von Discord. So
/// läuft die Tür in Tests gegen eine nachgestellte Seite.
pub async fn run_with<M>(
    http: Http,
    first: Vec<Profile<M>>,
    lenient: bool,
    mut commands: UnboundedReceiver<Command<M>>,
    mut handed: UnboundedReceiver<Handed>,
    on_change: OnChange,
) -> Result<(), String>
where
    M: Model + Send + Sync + 'static,
{
    let fish = Arc::new(Fish {
        served: RwLock::new(Vec::new()),
        me: AtomicU64::new(0),
        ready: AtomicBool::new(false),
        on_change,
    });
    // Erst alles prüfen, dann verbinden: passt ein Profil nicht, scheitert der
    // Fisch, bevor er sich bei Discord zeigt.
    let mut any = false;
    for profile in first {
        let label = label_of(profile.name.as_deref());
        match fish.start(None, profile).await {
            Ok(()) => any = true,
            Err(why) if lenient => eprintln!("rusty-fish: {label}cannot start: {why}"),
            Err(why) => return Err(why),
        }
    }
    if !any {
        return Err("no profile could be started".to_owned());
    }

    // MESSAGE_CONTENT muss im Entwicklerportal von Discord eingeschaltet sein,
    // sonst kommen die Nachrichten ohne Text an.
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let mut client = ClientBuilder::new_with_http(http, intents)
        .event_handler(Handler(fish.clone()))
        .await
        .map_err(|e| format!("cannot set up the Discord connection: {e}"))?;

    // Was eine andere Tür dieser zureicht. Jeder Auftrag für sich: die Tür,
    // die ihn zureicht, wartet auf ihn und hält so die Reihenfolge ihres
    // Profils; ein zweites Profil soll darauf nicht warten. Abgesetzt wird
    // über die Schnittstelle, nicht über die Verbindung: das geht, sobald
    // der Zugang eingerichtet ist.
    tokio::spawn({
        let fish = fish.clone();
        let http = client.http.clone();
        async move {
            while let Some(handed) = handed.recv().await {
                let (fish, http) = (fish.clone(), http.clone());
                tokio::spawn(async move {
                    let done = fish.take(&http, handed.profile.as_deref(), handed.errand).await;
                    let _ = handed.done.send(done);
                });
            }
        }
    });

    // Sauberes Anhalten. Gemessen von Nemesus am 20.11.2026: wurde der Fisch
    // hart beendet, stand der Bot in Discord noch eine Weile als online da,
    // weil Discord erst am ausbleibenden Herzschlag merkt, dass er fort ist.
    // Meldet er sich ab, ist er sofort grau.
    let stopping = Arc::new(AtomicBool::new(false));
    let shards = client.shard_manager.clone();
    let flag = stopping.clone();
    let http = client.http.clone();
    tokio::spawn(async move {
        loop {
            match commands.recv().await {
                Some(Command::Start(profile)) => {
                    let label = label_of(profile.name.as_deref());
                    if let Err(why) = fish.start(Some(&http), *profile).await {
                        eprintln!("rusty-fish: {label}cannot start: {why}");
                    }
                }
                Some(Command::Restart(profile)) => {
                    let label = label_of(profile.name.as_deref());
                    if let Err(why) = fish.restart(&http, *profile).await {
                        eprintln!("rusty-fish: {label}cannot start: {why}");
                    }
                }
                Some(Command::Stop(name)) => {
                    if fish.stop(&name).await == 0 {
                        break;
                    }
                }
                Some(Command::Halt(name, why)) => {
                    let left = fish.stop(&name).await;
                    eprintln!("rusty-fish: {}stopped: {why}", label_of(Some(&name)));
                    if left == 0 {
                        break;
                    }
                }
                Some(Command::StopAll) => break,
                // Niemand kann dem Fisch mehr etwas sagen. Er läuft weiter.
                None => std::future::pending::<()>().await,
            }
        }
        flag.store(true, Ordering::Relaxed);
        eprintln!("rusty-fish: stopping");
        shards.shutdown_all().await;
    });

    match client.start().await {
        Ok(()) => Ok(()),
        // Nach einem gewollten Anhalten ist das Ende der Verbindung kein Fehler.
        Err(_) if stopping.load(Ordering::Relaxed) => Ok(()),
        Err(e) => Err(format!("the Discord connection ended: {e}")),
    }
}

impl<M> Fish<M>
where
    M: Model + Send + Sync + 'static,
{
    /// Nimmt ein Profil in den Lauf. Steht die Verbindung schon (`http`),
    /// teilt der Fisch gleich mit, wer eingeladen ist, und holt nach, was in
    /// den Kanälen dieses Profils verpasst wurde.
    async fn start(&self, http: Option<&Http>, profile: Profile<M>) -> Result<(), String> {
        // Lehnt diese Tür ein Profil ab, das auch an einer anderen steht,
        // muss das Programm es hören: es hält das Profil dann überall an.
        let shared = profile.shared.as_ref().map(|shared| shared.lines.clone());
        let name = profile.name.clone();
        let started = self.take_in(http, profile).await;
        if let (Err(why), Some(lines), Some(name)) = (&started, shared, name) {
            lines.refuse(Door::Discord, &name, why);
        }
        started
    }

    async fn take_in(&self, http: Option<&Http>, profile: Profile<M>) -> Result<(), String> {
        if !profile.config.doors.contains(&Door::Discord) {
            return Err(format!(
                "this profile stands at the {} door, not at Discord",
                profile.config.door.label()
            ));
        }
        // Ein Profil an zwei Türen bringt seinen Haushalt mit; sonst führt
        // ihn diese Tür allein.
        let (house, lines) = match profile.shared {
            Some(shared) => (shared.house, Some(shared.lines)),
            None => {
                let house = Household::open(&profile.config, &profile.state_path)?;
                (Arc::new(Mutex::new(house)), None)
            }
        };
        let served = Arc::new(Served {
            name: profile.name,
            model: profile.model,
            config: profile.config,
            service: profile.service,
            house,
            lines,
            hooks: Mutex::new(HashMap::new()),
        });
        // Prüfen und Aufnehmen unter einem Schloss, zusammen mit der Frage,
        // ob die Verbindung steht: sonst könnte ein Profil genau zwischen dem
        // Verbinden und dem Nachholen hereinkommen und bliebe ohne beides.
        let ready = {
            let mut list = self.served.write().await;
            for other in list.iter() {
                if served.name.is_some() && other.name == served.name {
                    return Err("this profile is already running".to_owned());
                }
                // Ein Kanal in zwei Profilen: jede Nachricht würde doppelt
                // übersetzt, doppelt zugestellt und doppelt bezahlt.
                if let Some(channel) = other.config.shared_channel(&served.config) {
                    return Err(format!(
                        "channel {channel} already belongs to {}, which is running",
                        other.title()
                    ));
                }
            }
            list.push(served.clone());
            self.told(&list);
            self.ready.load(Ordering::Relaxed)
        };
        served.say("serving");
        if ready && let Some(http) = http {
            served.tell_the_invited(http).await;
            served.catch_up(http, self.me.load(Ordering::Relaxed)).await;
        }
        Ok(())
    }

    /// Nimmt ein Profil neu auf: das laufende gleichen Namens geht, dieses
    /// kommt, mit frisch gelesenen Einstellungen und frisch geöffnetem
    /// Zustand. Was das alte gespeichert hat, findet das neue vor.
    ///
    /// Lässt sich das neue nicht aufnehmen, etwa weil es nach der Änderung
    /// einen Kanal mit einem anderen laufenden Profil teilt, läuft danach
    /// keines von beiden: das alte gälte nicht mehr, und das neue darf nicht.
    async fn restart(&self, http: &Http, profile: Profile<M>) -> Result<(), String> {
        if let Some(name) = profile.name.as_deref() {
            let mut list = self.served.write().await;
            list.retain(|served| served.name.as_deref() != Some(name));
        }
        let started = self.start(Some(http), profile).await;
        if started.is_err() {
            let list = self.served.read().await;
            self.told(&list);
        }
        started
    }

    /// Nimmt ein Profil aus dem Lauf. Liefert, wie viele noch laufen.
    async fn stop(&self, name: &str) -> usize {
        let mut list = self.served.write().await;
        let before = list.len();
        list.retain(|served| served.name.as_deref() != Some(name));
        if list.len() < before {
            eprintln!("rusty-fish: {}no longer serving", label_of(Some(name)));
            self.told(&list);
        } else {
            eprintln!("rusty-fish: {}was not running", label_of(Some(name)));
        }
        list.len()
    }

    /// Sagt dem Aufrufer, welche Profile jetzt laufen.
    fn told(&self, list: &[Arc<Served<M>>]) {
        let names: Vec<String> = list.iter().filter_map(|s| s.name.clone()).collect();
        (self.on_change)(&names);
    }

    /// Das Profil, zu dem dieser Kanal gehört.
    async fn at(&self, channel: &str) -> Option<Arc<Served<M>>> {
        let list = self.served.read().await;
        list.iter().find(|s| s.config.room_index(channel).is_some()).cloned()
    }

    /// Das Profil, zu dem ein Knopf am Hinweis zum leeren Kontingent gehört.
    /// Ohne Namen am Knopf nur dann eines, wenn überhaupt nur eines läuft.
    async fn named(&self, name: Option<&str>) -> Option<Arc<Served<M>>> {
        let list = self.served.read().await;
        match name {
            Some(name) => list.iter().find(|s| s.name.as_deref() == Some(name)).cloned(),
            None if list.len() == 1 => list.first().cloned(),
            None => None,
        }
    }

    /// Erledigt, was eine andere Tür dieser zugereicht hat: Zustellungen für
    /// Kanäle dieser Tür, oder der Hinweis an den Gastgeber, der hier liest.
    async fn take(&self, http: &Http, profile: Option<&str>, errand: Errand) -> Result<(), String> {
        let Some(served) = self.named(profile).await else {
            return Err("this profile is not running at the Discord door".to_owned());
        };
        match errand {
            Errand::Deliver { deliveries, writer } => {
                if served.post_all(http, &deliveries, &writer).await {
                    Ok(())
                } else {
                    Err("not everything arrived; see the lines above".to_owned())
                }
            }
            Errand::TellHost(notice) => served.notify_host(http, &notice).await,
        }
    }
}

#[async_trait]
impl<M> EventHandler for Handler<M>
where
    M: Model + Send + Sync + 'static,
{
    async fn ready(&self, ctx: Context, ready: Ready) {
        let fish = &self.0;
        eprintln!("rusty-fish: connected to Discord as {}", ready.user.name);
        // Vor dem Nachholen merken, damit auch verpasste Erwähnungen zählen.
        fish.me.store(ready.user.id.get(), Ordering::Relaxed);
        // Unter dem Schloss der Liste, siehe `Fish::start`.
        let served = {
            let list = fish.served.write().await;
            fish.ready.store(true, Ordering::Relaxed);
            list.clone()
        };
        for profile in served {
            // Erst die Mitteilung, dann das Nachholen: wer eingeladen ist,
            // soll lesen, was der Fisch hier tut, bevor die ersten
            // Übersetzungen kommen.
            profile.tell_the_invited(&ctx.http).await;
            profile.catch_up(&ctx.http, ready.user.id.get()).await;
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        // Was der Fisch selbst abgesetzt hat, darf nicht wieder hereinkommen.
        if msg.author.bot || msg.webhook_id.is_some() {
            return;
        }
        // Ein Kanal außerhalb jedes Raums geht den Fisch nichts an. Geprüft
        // vor jedem Schloss eines Profils, damit so eine Nachricht nicht wartet.
        let Some(profile) = self.0.at(&msg.channel_id.to_string()).await else { return };
        profile.handle(&ctx.http, &msg, self.0.me.load(Ordering::Relaxed)).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Component(click) = interaction else { return };
        let id = click.data.custom_id.as_str();
        // Nur die Kennung des Knopfs, kein Text. Gemessen am 19.11.2026: ein
        // Klick auf die Zustimmung kam nicht an, und ohne diese Zeile war
        // nicht zu sehen, ob Discord ihn überhaupt gemeldet hatte.
        eprintln!("rusty-fish: click received on {id}");
        let (profile, choice) = if id == CONSENT {
            // Die Bitte steht in einem Kanal, und der gehört zu einem Profil.
            (self.0.at(&click.channel_id.to_string()).await, None)
        } else if let Some((choice, name)) = budget_of(id) {
            (self.0.named(name).await, Some(choice))
        } else {
            return;
        };
        let Some(profile) = profile else {
            // Der Knopf stammt von einem Profil, das gerade nicht läuft.
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content(NOT_RUNNING).ephemeral(true),
            );
            if let Err(why) = click.create_response(&ctx.http, response).await {
                eprintln!("rusty-fish: cannot answer a click without a running profile: {why}");
            }
            return;
        };
        match choice {
            None => profile.consent_clicked(&ctx.http, &click).await,
            Some(choice) => profile.budget_clicked(&ctx.http, &click, choice).await,
        }
    }
}

impl<M> Served<M>
where
    M: Model + Send + Sync + 'static,
{
    /// Wie dieses Profil in einer Fehlermeldung heißt.
    fn title(&self) -> String {
        title_of(self.name.as_deref())
    }

    /// Eine Zeile in die Programmausgabe, mit dem Namen des Profils davor.
    /// Kanal-Kennungen und Fehlergründe, nie der Text einer Nachricht.
    fn say(&self, text: impl std::fmt::Display) {
        eprintln!("rusty-fish: {}{text}", label_of(self.name.as_deref()));
    }

    /// Was für einen Aufruf des Haushalts feststeht.
    fn terms<'a>(&'a self, today: &'a str) -> Terms<'a> {
        Terms { config: &self.config, service: &self.service, today }
    }

    /// Jemand hat den Knopf unter der Bitte um Zustimmung gedrückt.
    async fn consent_clicked(&self, http: &Http, click: &ComponentInteraction) {
        // Discord will binnen drei Sekunden eine Antwort, sonst meldet es dem
        // Menschen „Interaktion fehlgeschlagen". Das Schloss kann länger
        // besetzt sein, solange eine Nachricht übersetzt wird. Deshalb erst
        // quittieren (nur für den Klickenden sichtbar), dann speichern.
        if let Err(why) = click.defer_ephemeral(http).await {
            self.say(format_args!("cannot acknowledge the consent click: {why}"));
        }

        // Wer drückt, stimmt für sich selbst zu, für niemanden sonst.
        let user = click.user.id.to_string();
        // Das Schloss bleibt bis zum Ende: was jetzt nachgereicht wird, soll
        // vor dem stehen, was derselbe Mensch gleich danach schreibt.
        let mut house = self.house.lock().await;
        let saved = house.consent(&user, &self.service);
        let reply = match saved {
            Ok(()) => {
                self.say("consent stored");
                "✓"
            }
            Err(why) => {
                self.say(why);
                "⚠"
            }
        };
        let edit = EditInteractionResponse::new().content(reply);
        if let Err(why) = click.edit_response(http, edit).await {
            self.say(format_args!("cannot confirm the consent: {why}"));
        }

        // Was dieser Mensch vor dem Klick schrieb, kam unübersetzt an. Jetzt,
        // da er zugestimmt hat, wird es übersetzt nachgereicht: sonst denkt er,
        // er habe etwas gesagt, und niemand konnte es lesen.
        let today = today();
        let held = house.held_back(&self.model, &self.terms(&today), &user).await;
        if held.is_empty() {
            return;
        }
        // Nur die Zahl, kein Text.
        self.say(format_args!("handing in {} delivery(ies) written before consent", held.len()));
        let name = click
            .member
            .as_ref()
            .and_then(|m| m.nick.clone())
            .or_else(|| click.user.global_name.clone())
            .unwrap_or_else(|| click.user.name.clone());
        let writer = Writer {
            door: Door::Discord,
            name,
            face: Some(click.user.face()),
            attachments: String::new(),
        };
        self.deliver(http, &held, &writer).await;
        if let Err(why) = house.save() {
            self.say(why);
        }
    }

    /// Setzt Zustellungen ab, jede an der Tür ihres Tisches: die für Kanäle
    /// dieser Tür hier, die anderen reicht sie der Tür zu, an der ihr Tisch
    /// steht, und wartet darauf. Liefert, ob alles ankam.
    async fn deliver(&self, http: &Http, deliveries: &[Delivery], writer: &Writer) -> bool {
        let (mine, others) = by_door(&self.config, Door::Discord, deliveries);
        let mut delivered = self.post_all(http, &mine, writer).await;
        for (door, deliveries) in others {
            let errand = Errand::Deliver { deliveries, writer: writer.clone() };
            let handed = match &self.lines {
                Some(lines) => lines.hand(door, self.name.as_deref(), errand).await,
                None => Err("this profile was started at one door only".to_owned()),
            };
            if let Err(why) = handed {
                delivered = false;
                self.say(format_args!("cannot deliver through the {} door: {why}", door.label()));
            }
        }
        delivered
    }

    /// Setzt Zustellungen in Kanälen dieser Tür ab: was ein Mensch schrieb,
    /// unter seinem Namen und Bild; was die KI sagt, unter dem Namen, den der
    /// Gastgeber ihr gab. Liefert, ob alles ankam.
    ///
    /// Wer an einer anderen Tür schrieb, erscheint mit ihr hinter dem Namen,
    /// etwa „Robin (Matrix)", und ohne eigenes Bild, wenn jene Tür keines
    /// hergibt, das Discord abrufen kann (entschieden von Nemesus am
    /// 23.11.2026).
    async fn post_all(&self, http: &Http, deliveries: &[Delivery], writer: &Writer) -> bool {
        let assistant = hook_name(self.config.ai_name().unwrap_or(WEBHOOK_NAME));
        let shown = match writer.door {
            Door::Discord => hook_name(&writer.name),
            other => hook_name_from(&writer.name, other),
        };
        let mut hooks = self.hooks.lock().await;
        let mut delivered = true;
        for delivery in deliveries {
            // Die graue Zeile steht in der Sprache des Tisches, an dem sie
            // erscheint, nicht in der des Tisches, an dem geschrieben wurde.
            let rendered = render_in(delivery, self.config.tongue_at(&delivery.channel));
            let (name, avatar, body) = match delivery.from {
                Sender::Writer => {
                    (shown.as_str(), writer.face.as_deref(), rendered + &writer.attachments)
                }
                Sender::Assistant => (assistant.as_str(), None, rendered),
            };
            let posted = post(http, &mut hooks, &delivery.channel, name, avatar, &body).await;
            if let Err(why) = posted {
                delivered = false;
                self.say(format_args!("cannot deliver to channel {}: {why}", delivery.channel));
            }
        }
        delivered
    }

    /// Jemand hat einen Knopf am Hinweis zum leeren Kontingent gedrückt.
    async fn budget_clicked(&self, http: &Http, click: &ComponentInteraction, choice: Budget) {
        // Nur der Klick des Gastgebers zählt: es ist sein Schlüssel und sein Geld.
        if self.config.host_at(Door::Discord) != Some(click.user.id.to_string().as_str()) {
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content(NOT_THE_HOST).ephemeral(true),
            );
            if let Err(why) = click.create_response(http, response).await {
                self.say(format_args!("cannot answer a click on the budget notice: {why}"));
            }
            return;
        }

        // Discord will binnen drei Sekunden eine Antwort. Das Schloss kann
        // länger besetzt sein, solange eine Nachricht übersetzt wird. Deshalb
        // erst quittieren, dann rechnen, dann den Hinweis ersetzen.
        if let Err(why) = click.defer(http).await {
            self.say(format_args!("cannot acknowledge the host's click: {why}"));
        }

        let text = match choice {
            Budget::Keep => KEPT_NOTICE.to_owned(),
            Budget::Raise => {
                let mut house = self.house.lock().await;
                let today = today();
                let (text, saved) = house.raise(&self.terms(&today));
                if let Err(why) = saved {
                    self.say(why);
                }
                text
            }
        };

        // Die Antwort ersetzt den Hinweis und nimmt die Knöpfe weg, damit kein
        // alter Knopf stehen bleibt.
        let edit = EditInteractionResponse::new().content(text).components(Vec::new());
        if let Err(why) = click.edit_response(http, edit).await {
            self.say(format_args!("cannot answer the host: {why}"));
        }
    }

    /// Sagt dem Gastgeber, und nur ihm, dass das heutige Kontingent verbraucht
    /// ist, und fragt, ob er um denselben Betrag erhöhen will: als
    /// Direktnachricht mit zwei Knöpfen.
    ///
    /// Die Knöpfe tragen den Namen des Profils, und der Hinweis nennt ihn:
    /// laufen mehrere Profile, muss der Gastgeber wissen, welches leer ist,
    /// und der Fisch, welches er erhöhen soll.
    async fn notify_host(&self, http: &Http, notice: &HostNotice) -> Result<(), String> {
        let id = self.config.host_at(Door::Discord).map(str::parse::<u64>);
        let Some(Ok(id)) = id else {
            return Err("the host has no Discord id in this profile".to_owned());
        };
        let name = self.name.as_deref();
        let content = match name {
            // `-# ` ist in Discord eine kleine graue Zeile.
            Some(name) => format!("{}\n-# Profile: {name}", notice.text),
            None => notice.text.clone(),
        };
        let message = CreateMessage::new()
            .content(content)
            .button(
                CreateButton::new(budget_id(Budget::Raise, name))
                    .label(notice.raise_label.as_str())
                    .style(ButtonStyle::Success),
            )
            .button(
                CreateButton::new(budget_id(Budget::Keep, name))
                    .label(notice.keep_label)
                    .style(ButtonStyle::Secondary),
            );
        let sent = match UserId::new(id).create_dm_channel(http).await {
            Ok(dm) => dm.id.send_message(http, message).await.map(|_| ()),
            Err(why) => Err(why),
        };
        sent.map_err(|why| why.to_string())
    }

    /// Eine Nachricht aus einem Kanal dieses Profils: an alle anderen Plätze
    /// des Raums verteilen. `me` ist die Benutzer-ID des Bots.
    async fn handle(&self, http: &Http, msg: &Message, me: u64) {
        // Was der Fisch selbst abgesetzt hat, darf nicht wieder hereinkommen.
        // Auch hier geprüft, weil das Nachholen an `message` vorbeikommt.
        if msg.author.bot || msg.webhook_id.is_some() {
            return;
        }
        let channel = msg.channel_id.to_string();

        let mut house = self.house.lock().await;

        let author = msg.author.id.to_string();
        // Die echte Erwähnung des Bots am Anfang zählt wie sein Rufname. Der
        // Haushalt bekommt dafür den Rufnamen statt des Discord-Kürzels; so
        // sehen weder die KI noch die anderen Plätze das Kürzel. Ohne Rufnamen
        // ist die Funktion aus, und die Nachricht bleibt, wie sie ist.
        let called =
            self.config.ai_name().and_then(|name| mention_as_call(&msg.content, me, name));
        let arrived = Arrived {
            channel: &channel,
            author: &author,
            text: called.as_deref().unwrap_or(&msg.content),
            // Discord zählt Nachrichten mit einer Zahl, die wächst.
            mark: msg.id.get(),
        };
        let today = today();
        // Nichts zu tun, etwa weil Nachholen und Live-Meldung sich überschneiden.
        let Some(steps) = house.message(&self.model, &self.terms(&today), &arrived).await else {
            return;
        };

        // Was ein Mensch schrieb, erscheint unter seinem Namen und Bild. Was
        // die KI sagt, erscheint unter dem Namen, den der Gastgeber ihr gab.
        let writer = Writer {
            door: Door::Discord,
            name: display_name(msg),
            face: Some(msg.author.face()),
            attachments: msg.attachments.iter().map(|a| format!("\n{}", a.url)).collect(),
        };
        let delivered = self.deliver(http, &steps.deliveries, &writer).await;
        // Kam etwas nicht an, soll der Schreibende es an seiner Nachricht sehen.
        if !delivered {
            let _ = msg.react(http, '⚠').await;
        }

        // Die Bitte um Zustimmung geht an den Tisch des Schreibenden.
        if let Some(text) = &steps.ask_consent {
            self.ask_consent(http, msg.channel_id, text).await;
        }

        // Ist das Kontingent leer, erfährt es der Gastgeber, an der Tür, an
        // der er liest.
        if let Some(notice) = &steps.tell_host {
            let door = self.config.host_door();
            let told = match &self.lines {
                Some(lines) if door != Door::Discord => {
                    lines.hand(door, self.name.as_deref(), Errand::TellHost(notice.clone())).await
                }
                _ => self.notify_host(http, notice).await,
            };
            if let Err(why) = told {
                self.say(format_args!("cannot tell the host that the budget is used up: {why}"));
            }
        }

        // Erst jetzt, nach dem Absetzen: stürzt der Fisch vorher ab, gilt die
        // Nachricht als nicht gelesen, und der nächste Start holt sie nach.
        if let Err(why) = house.save() {
            self.say(why);
        }
    }

    /// Teilt Eingeladenen an ihrem Tisch mit, dass dieser Fisch sie jetzt
    /// bedient: einmal je Mensch, gemerkt in der Zustandsdatei.
    ///
    /// Nemesus am 20.11.2026: „Es sollte auch die Funktion ‚Einladen' geben.
    /// Und dann ein Haken, wer eingeladen wird. Das könnte dann per
    /// Systemnachricht mitgeteilt werden."
    async fn tell_the_invited(&self, http: &Http) {
        let mut house = self.house.lock().await;
        // Nichts fällig und niemand zu vergessen: die Datei bleibt, wie sie ist.
        let Some(pending) = house.invitations(&self.config) else { return };

        let mut reached = Vec::new();
        for invite in &pending.due {
            // Wessen Tisch an einer anderen Tür steht, dem sagt es die andere
            // Tür. Hier bleibt er ungemerkt.
            if self.config.door_of(&invite.channel) != Some(Door::Discord) {
                continue;
            }
            let Ok(id) = invite.channel.parse::<u64>() else { continue };
            let channel = ChannelId::new(id);
            let host = self.host_name(http, channel).await;
            let text = invitation(self.config.tongue_at(&invite.channel), host.as_deref());
            // Als der Bot selbst, nicht unter fremdem Namen: es ist seine
            // Mitteilung. Der Anzeigename des Gastgebers ist frei gewählt und
            // darf niemanden anpingen.
            let message =
                CreateMessage::new().content(text).allowed_mentions(CreateAllowedMentions::new());
            match channel.send_message(http, message).await {
                Ok(_) => {
                    reached.push(invite.person.clone());
                    self.say(format_args!("invitation announced in channel {}", invite.channel));
                }
                // Nicht gemerkt: der nächste Start versucht es wieder.
                Err(why) => self.say(format_args!(
                    "cannot announce the invitation in channel {}: {why}",
                    invite.channel
                )),
            }
        }

        if let Err(why) = house.invitations_told(pending, &reached) {
            self.say(why);
        }
    }

    /// Der Name, unter dem der Gastgeber auf dem Server dieses Kanals
    /// erscheint, soweit Discord ihn hergibt. Sonst sein allgemeiner
    /// Anzeigename, sonst nichts: dann nennt die Mitteilung niemanden.
    async fn host_name(&self, http: &Http, channel: ChannelId) -> Option<String> {
        // Ohne Kennung des Gastgebers an dieser Tür nennt die Mitteilung
        // niemanden.
        let host = UserId::new(self.config.host_at(Door::Discord)?.parse::<u64>().ok()?);
        if let Ok(Channel::Guild(place)) = channel.to_channel(http).await
            && let Ok(member) = place.guild_id.member(http, host).await
        {
            return Some(member.display_name().to_owned());
        }
        let user = host.to_user(http).await.ok()?;
        Some(user.global_name.unwrap_or(user.name))
    }

    /// Setzt die Bitte um Zustimmung ab, mit einem Knopf. Den Wortlaut gibt
    /// der Haushalt, in der Sprache des Tisches.
    async fn ask_consent(&self, http: &Http, channel_id: ChannelId, text: &str) {
        // „OK" statt eines Satzes: ein Satz wie „Ich stimme zu" hat in manchen
        // Sprachen ein Geschlecht, und der Fisch weiß nicht, wer drückt.
        let message = CreateMessage::new()
            .content(text)
            .button(CreateButton::new(CONSENT).label("OK").style(ButtonStyle::Success));
        if let Err(why) = channel_id.send_message(http, message).await {
            self.say(format_args!("cannot ask for consent in channel {channel_id}: {why}"));
        }
    }

    /// Holt nach, was geschrieben wurde, während der Fisch dieses Profil nicht
    /// bediente. Bei zwei gespiegelten Kanälen käme sonst nichts beim anderen an.
    async fn catch_up(&self, http: &Http, me: u64) {
        let marks: Vec<(String, u64)> = {
            let house = self.house.lock().await;
            self.config
                .rooms
                .iter()
                .flat_map(|r| r.seats.iter())
                // Nur die Kanäle dieser Tür; die Räume einer anderen holt
                // jene nach.
                .filter(|s| self.config.door_of(&s.channel) == Some(Door::Discord))
                .filter_map(|s| house.seen(&s.channel).map(|id| (s.channel.clone(), id)))
                .collect()
        };

        let mut missed: Vec<Message> = Vec::new();
        for (channel, mut after) in marks {
            let Ok(id) = channel.parse::<u64>() else { continue };
            loop {
                let page = ChannelId::new(id)
                    .messages(http, GetMessages::new().after(MessageId::new(after)).limit(100))
                    .await;
                match page {
                    Ok(page) if page.is_empty() => break,
                    Ok(page) => {
                        let full = page.len() == 100;
                        after = page.iter().map(|m| m.id.get()).max().unwrap_or(after);
                        missed.extend(page);
                        if !full {
                            break;
                        }
                    }
                    Err(why) => {
                        self.say(format_args!("cannot catch up on channel {channel}: {why}"));
                        break;
                    }
                }
            }
        }

        // Über alle Kanäle hinweg in der Reihenfolge, in der geschrieben wurde.
        missed.sort_by_key(|m| m.id);
        // Gezählt werden nur Nachrichten von Menschen. Die eigenen Spiegelungen
        // des Fischs liegen auch im Verlauf, werden aber übersprungen; bis zum
        // 19.11.2026 zählte die Meldung sie mit und war dadurch irreführend.
        let from_people =
            missed.iter().filter(|m| !m.author.bot && m.webhook_id.is_none()).count();
        if from_people > 0 {
            self.say(format_args!("catching up on {from_people} missed message(s)"));
        }
        for msg in &missed {
            self.handle(http, msg, me).await;
        }
    }
}

/// Der Name, unter dem jemand auf diesem Server erscheint.
fn display_name(msg: &Message) -> String {
    msg.member
        .as_ref()
        .and_then(|m| m.nick.clone())
        .or_else(|| msg.author.global_name.clone())
        .unwrap_or_else(|| msg.author.name.clone())
}

/// Setzt einen Text unter fremdem Namen in einem Kanal ab. Ohne Bild gilt das
/// Standardbild des Webhooks.
async fn post(
    http: &Http,
    hooks: &mut HashMap<ChannelId, Webhook>,
    channel: &str,
    name: &str,
    avatar: Option<&str>,
    body: &str,
) -> Result<(), String> {
    let id = channel.parse::<u64>().map_err(|_| "not a channel id".to_owned())?;
    let channel_id = ChannelId::new(id);
    if !hooks.contains_key(&channel_id) {
        hooks.insert(channel_id, find_or_create_hook(http, channel_id).await?);
    }
    for part in chunks(body, MESSAGE_LIMIT) {
        let mut message = ExecuteWebhook::new()
            .content(part)
            .username(name)
            // Eine gespiegelte Nachricht darf niemanden ein zweites Mal anpingen.
            .allowed_mentions(CreateAllowedMentions::new());
        if let Some(url) = avatar {
            message = message.avatar_url(url);
        }
        if let Err(why) = hooks[&channel_id].execute(http, false, message).await {
            // Vielleicht wurde der Webhook gelöscht. Beim nächsten Mal neu suchen.
            hooks.remove(&channel_id);
            return Err(why.to_string());
        }
    }
    Ok(())
}

async fn find_or_create_hook(http: &Http, channel: ChannelId) -> Result<Webhook, String> {
    let existing = channel.webhooks(http).await.map_err(|e| {
        format!("cannot list webhooks (is the permission Manage Webhooks granted?): {e}")
    })?;
    let ours = existing
        .into_iter()
        .find(|h| h.token.is_some() && h.name.as_deref() == Some(WEBHOOK_NAME));
    match ours {
        Some(hook) => Ok(hook),
        None => channel
            .create_webhook(http, CreateWebhook::new(WEBHOOK_NAME))
            .await
            .map_err(|e| format!("cannot create a webhook: {e}")),
    }
}
