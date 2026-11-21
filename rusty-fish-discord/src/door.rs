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
//! In die Ausgabe des Programms gelangen nur Kanal-Kennungen und Fehlergründe,
//! nie der Text einer Nachricht.

use crate::mention::mention_as_call;
use crate::render::{MESSAGE_LIMIT, chunks, hook_name, render_in};
use rusty_fish_core::Model;
use rusty_fish_house::clock::today;
use rusty_fish_house::config::Config;
use rusty_fish_house::household::{Arrived, HostNotice, Household, Terms};
use rusty_fish_house::texts::{KEPT_NOTICE, NOT_THE_HOST, invitation};
use rusty_fish_room::Sender;
use serenity::all::{
    ButtonStyle, Channel, ChannelId, Client, ComponentInteraction, Context, CreateAllowedMentions,
    CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage,
    CreateWebhook, EditInteractionResponse, EventHandler, ExecuteWebhook, GatewayIntents,
    GetMessages, Http, Interaction, Message, MessageId, Ready, UserId, Webhook,
};
use serenity::async_trait;
use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::Mutex;

const CONSENT_BUTTON: &str = "rusty-fish-consent";
/// Die beiden Knöpfe am Hinweis an den Gastgeber.
const RAISE_BUTTON: &str = "rusty-fish-raise";
const KEEP_BUTTON: &str = "rusty-fish-keep";
const WEBHOOK_NAME: &str = "Rusty Fish";

/// Der Fisch an der Discord-Tür: die KI, die Einstellungen und alles, was
/// sich im Lauf ändert.
struct Fish<M> {
    model: M,
    config: Config,
    /// Der Name des KI-Dienstes, wie er in der Bitte um Zustimmung steht.
    service: String,
    /// Die Benutzer-ID des Bots selbst, bekannt ab dem Ready-Ereignis. Null
    /// heißt: noch nicht bekannt. Gebraucht wird sie, um die echte Erwähnung
    /// des Bots am Anfang einer Nachricht zu erkennen.
    me: AtomicU64,
    /// Ein einziges Schloss für alles Veränderliche. Es hält die Nachrichten
    /// eines Raums in der Reihenfolge, in der sie geschrieben wurden: Discord
    /// meldet sie gleichzeitig, das Schloss lässt sie einzeln durch.
    inner: Mutex<Inner>,
}

struct Inner {
    /// Was der Fisch sich merkt, und was nach einer Nachricht zu geschehen hat.
    house: Household,
    hooks: HashMap<ChannelId, Webhook>,
}

/// Startet die Tür und läuft, bis die Verbindung endet oder `stop` eintritt.
///
/// `service` ist der Name des KI-Dienstes, zu dem `model` spricht. Er kommt
/// vom Aufrufer, weil nur der die Adresse kennt, die wirklich benutzt wird.
///
/// `stop` kommt ebenfalls vom Aufrufer: wann ein Programm anhalten soll,
/// weiß das Programm, nicht eine seiner Türen.
pub async fn run<M, S>(
    token: &str,
    config: Config,
    state_path: &Path,
    model: M,
    service: &str,
    stop: S,
) -> Result<(), String>
where
    M: Model + Send + Sync + 'static,
    S: Future<Output = ()> + Send + 'static,
{
    let house = Household::open(&config, state_path)?;
    let fish = Fish {
        model,
        config,
        service: service.to_owned(),
        me: AtomicU64::new(0),
        inner: Mutex::new(Inner { house, hooks: HashMap::new() }),
    };

    // MESSAGE_CONTENT muss im Entwicklerportal von Discord eingeschaltet sein,
    // sonst kommen die Nachrichten ohne Text an.
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(token, intents)
        .event_handler(fish)
        .await
        .map_err(|e| format!("cannot set up the Discord connection: {e}"))?;

    // Sauberes Anhalten. Gemessen von Nemesus am 20.11.2026: wurde der Fisch
    // hart beendet, stand der Bot in Discord noch eine Weile als online da,
    // weil Discord erst am ausbleibenden Herzschlag merkt, dass er fort ist.
    // Meldet er sich ab, ist er sofort grau.
    let stopping = Arc::new(AtomicBool::new(false));
    let shards = client.shard_manager.clone();
    let flag = stopping.clone();
    tokio::spawn(async move {
        stop.await;
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

#[async_trait]
impl<M> EventHandler for Fish<M>
where
    M: Model + Send + Sync + 'static,
{
    async fn ready(&self, ctx: Context, ready: Ready) {
        eprintln!("rusty-fish: connected to Discord as {}", ready.user.name);
        // Vor dem Nachholen merken, damit auch verpasste Erwähnungen zählen.
        self.me.store(ready.user.id.get(), Ordering::Relaxed);
        // Erst die Mitteilung, dann das Nachholen: wer eingeladen ist, soll
        // lesen, was der Fisch hier tut, bevor die ersten Übersetzungen kommen.
        self.tell_the_invited(&ctx.http).await;
        self.catch_up(&ctx.http).await;
    }

    async fn message(&self, ctx: Context, msg: Message) {
        self.handle(&ctx.http, &msg).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Component(click) = interaction else { return };
        // Nur die Kennung des Knopfs, kein Text. Gemessen am 19.11.2026: ein
        // Klick auf die Zustimmung kam nicht an, und ohne diese Zeile war
        // nicht zu sehen, ob Discord ihn überhaupt gemeldet hatte.
        eprintln!("rusty-fish: click received on {}", click.data.custom_id);
        match click.data.custom_id.as_str() {
            CONSENT_BUTTON => self.consent_clicked(&ctx.http, &click).await,
            RAISE_BUTTON | KEEP_BUTTON => self.budget_clicked(&ctx.http, &click).await,
            _ => {}
        }
    }
}

impl<M> Fish<M>
where
    M: Model + Send + Sync + 'static,
{
    /// Jemand hat den Knopf unter der Bitte um Zustimmung gedrückt.
    async fn consent_clicked(&self, http: &Http, click: &ComponentInteraction) {
        // Discord will binnen drei Sekunden eine Antwort, sonst meldet es dem
        // Menschen „Interaktion fehlgeschlagen". Das Schloss kann länger
        // besetzt sein, solange eine Nachricht übersetzt wird. Deshalb erst
        // quittieren (nur für den Klickenden sichtbar), dann speichern.
        if let Err(why) = click.defer_ephemeral(http).await {
            eprintln!("rusty-fish: cannot acknowledge the consent click: {why}");
        }

        // Wer drückt, stimmt für sich selbst zu, für niemanden sonst.
        let saved = self.inner.lock().await.house.consent(&click.user.id.to_string());
        let reply = match saved {
            Ok(()) => {
                eprintln!("rusty-fish: consent stored");
                "✓"
            }
            Err(why) => {
                eprintln!("rusty-fish: {why}");
                "⚠"
            }
        };
        let edit = EditInteractionResponse::new().content(reply);
        if let Err(why) = click.edit_response(http, edit).await {
            eprintln!("rusty-fish: cannot confirm the consent: {why}");
        }
    }

    /// Jemand hat einen Knopf am Hinweis zum leeren Kontingent gedrückt.
    async fn budget_clicked(&self, http: &Http, click: &ComponentInteraction) {
        // Nur der Klick des Gastgebers zählt: es ist sein Schlüssel und sein Geld.
        if click.user.id.to_string() != self.config.host {
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content(NOT_THE_HOST).ephemeral(true),
            );
            if let Err(why) = click.create_response(http, response).await {
                eprintln!("rusty-fish: cannot answer a click on the budget notice: {why}");
            }
            return;
        }

        // Discord will binnen drei Sekunden eine Antwort. Das Schloss kann
        // länger besetzt sein, solange eine Nachricht übersetzt wird. Deshalb
        // erst quittieren, dann rechnen, dann den Hinweis ersetzen.
        if let Err(why) = click.defer(http).await {
            eprintln!("rusty-fish: cannot acknowledge the host's click: {why}");
        }

        let text = if click.data.custom_id == KEEP_BUTTON {
            KEPT_NOTICE.to_owned()
        } else {
            let mut inner = self.inner.lock().await;
            let today = today();
            let (text, saved) = inner.house.raise(&self.terms(&today));
            if let Err(why) = saved {
                eprintln!("rusty-fish: {why}");
            }
            text
        };

        // Die Antwort ersetzt den Hinweis und nimmt die Knöpfe weg, damit kein
        // alter Knopf stehen bleibt.
        let edit = EditInteractionResponse::new().content(text).components(Vec::new());
        if let Err(why) = click.edit_response(http, edit).await {
            eprintln!("rusty-fish: cannot answer the host: {why}");
        }
    }

    /// Was für einen Aufruf des Haushalts feststeht.
    fn terms<'a>(&'a self, today: &'a str) -> Terms<'a> {
        Terms { config: &self.config, service: &self.service, today }
    }

    /// Sagt dem Gastgeber, und nur ihm, dass das heutige Kontingent verbraucht
    /// ist, und fragt, ob er um denselben Betrag erhöhen will: als
    /// Direktnachricht mit zwei Knöpfen.
    async fn notify_host(&self, http: &Http, notice: &HostNotice) {
        let Ok(id) = self.config.host.parse::<u64>() else { return };
        let message = CreateMessage::new()
            .content(notice.text.as_str())
            .button(
                CreateButton::new(RAISE_BUTTON)
                    .label(notice.raise_label.as_str())
                    .style(ButtonStyle::Success),
            )
            .button(
                CreateButton::new(KEEP_BUTTON)
                    .label(notice.keep_label)
                    .style(ButtonStyle::Secondary),
            );
        let sent = match UserId::new(id).create_dm_channel(http).await {
            Ok(dm) => dm.id.send_message(http, message).await.map(|_| ()),
            Err(why) => Err(why),
        };
        if let Err(why) = sent {
            eprintln!("rusty-fish: cannot tell the host that the budget is used up: {why}");
        }
    }

    /// Eine Nachricht aus einem Kanal: an alle anderen Plätze des Raums verteilen.
    async fn handle(&self, http: &Http, msg: &Message) {
        // Was der Fisch selbst abgesetzt hat, darf nicht wieder hereinkommen.
        if msg.author.bot || msg.webhook_id.is_some() {
            return;
        }
        let channel = msg.channel_id.to_string();
        // Ein Kanal außerhalb jedes Raums geht den Fisch nichts an. Geprüft
        // vor dem Schloss, damit so eine Nachricht nicht darauf wartet.
        if self.config.room_index(&channel).is_none() {
            return;
        }

        let mut guard = self.inner.lock().await;
        let inner = &mut *guard;

        let author = msg.author.id.to_string();
        // Die echte Erwähnung des Bots am Anfang zählt wie sein Rufname. Der
        // Haushalt bekommt dafür den Rufnamen statt des Discord-Kürzels; so
        // sehen weder die KI noch die anderen Plätze das Kürzel. Ohne Rufnamen
        // ist die Funktion aus, und die Nachricht bleibt, wie sie ist.
        let called = self
            .config
            .ai_name()
            .and_then(|name| mention_as_call(&msg.content, self.me.load(Ordering::Relaxed), name));
        let arrived = Arrived {
            channel: &channel,
            author: &author,
            text: called.as_deref().unwrap_or(&msg.content),
            // Discord zählt Nachrichten mit einer Zahl, die wächst.
            mark: msg.id.get(),
        };
        let today = today();
        // Nichts zu tun, etwa weil Nachholen und Live-Meldung sich überschneiden.
        let Some(steps) = inner.house.message(&self.model, &self.terms(&today), &arrived).await
        else {
            return;
        };

        // Was ein Mensch schrieb, erscheint unter seinem Namen und Bild. Was
        // die KI sagt, erscheint unter dem Namen, den der Gastgeber ihr gab.
        let writer = hook_name(&display_name(msg));
        let face = msg.author.face();
        let assistant = hook_name(self.config.ai_name().unwrap_or(WEBHOOK_NAME));
        let attachments: String = msg.attachments.iter().map(|a| format!("\n{}", a.url)).collect();
        let mut delivered = true;
        for delivery in &steps.deliveries {
            // Die graue Zeile steht in der Sprache des Tisches, an dem sie
            // erscheint, nicht in der des Tisches, an dem geschrieben wurde.
            let rendered = render_in(delivery, self.config.tongue_at(&delivery.channel));
            let (name, avatar, body) = match delivery.from {
                Sender::Writer => (writer.as_str(), Some(face.as_str()), rendered + &attachments),
                Sender::Assistant => (assistant.as_str(), None, rendered),
            };
            let posted = post(http, &mut inner.hooks, &delivery.channel, name, avatar, &body).await;
            if let Err(why) = posted {
                delivered = false;
                eprintln!("rusty-fish: cannot deliver to channel {}: {why}", delivery.channel);
            }
        }
        // Kam etwas nicht an, soll der Schreibende es an seiner Nachricht sehen.
        if !delivered {
            let _ = msg.react(http, '⚠').await;
        }

        // Die Bitte um Zustimmung geht an den Tisch des Schreibenden.
        if let Some(text) = &steps.ask_consent {
            self.ask_consent(http, msg.channel_id, text).await;
        }

        // Ist das Kontingent leer, erfährt es der Gastgeber.
        if let Some(notice) = &steps.tell_host {
            self.notify_host(http, notice).await;
        }

        // Erst jetzt, nach dem Absetzen: stürzt der Fisch vorher ab, gilt die
        // Nachricht als nicht gelesen, und der nächste Start holt sie nach.
        if let Err(why) = inner.house.save() {
            eprintln!("rusty-fish: {why}");
        }
    }

    /// Teilt Eingeladenen an ihrem Tisch mit, dass dieser Fisch sie jetzt
    /// bedient: einmal je Mensch, gemerkt in der Zustandsdatei.
    ///
    /// Nemesus am 20.11.2026: „Es sollte auch die Funktion ‚Einladen' geben.
    /// Und dann ein Haken, wer eingeladen wird. Das könnte dann per
    /// Systemnachricht mitgeteilt werden."
    async fn tell_the_invited(&self, http: &Http) {
        let mut inner = self.inner.lock().await;
        // Nichts fällig und niemand zu vergessen: die Datei bleibt, wie sie ist.
        let Some(pending) = inner.house.invitations(&self.config) else { return };

        let mut reached = Vec::new();
        for invite in &pending.due {
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
                    eprintln!("rusty-fish: invitation announced in channel {}", invite.channel);
                }
                // Nicht gemerkt: der nächste Start versucht es wieder.
                Err(why) => eprintln!(
                    "rusty-fish: cannot announce the invitation in channel {}: {why}",
                    invite.channel
                ),
            }
        }

        if let Err(why) = inner.house.invitations_told(pending, &reached) {
            eprintln!("rusty-fish: {why}");
        }
    }

    /// Der Name, unter dem der Gastgeber auf dem Server dieses Kanals
    /// erscheint, soweit Discord ihn hergibt. Sonst sein allgemeiner
    /// Anzeigename, sonst nichts: dann nennt die Mitteilung niemanden.
    async fn host_name(&self, http: &Http, channel: ChannelId) -> Option<String> {
        let host = UserId::new(self.config.host.parse::<u64>().ok()?);
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
            .button(CreateButton::new(CONSENT_BUTTON).label("OK").style(ButtonStyle::Success));
        if let Err(why) = channel_id.send_message(http, message).await {
            eprintln!("rusty-fish: cannot ask for consent in channel {channel_id}: {why}");
        }
    }

    /// Holt nach, was geschrieben wurde, während der Fisch nicht lief. Bei
    /// zwei gespiegelten Kanälen käme sonst nichts beim anderen an.
    async fn catch_up(&self, http: &Http) {
        let marks: Vec<(String, u64)> = {
            let inner = self.inner.lock().await;
            self.config
                .rooms
                .iter()
                .flat_map(|r| r.seats.iter())
                .filter_map(|s| inner.house.seen(&s.channel).map(|id| (s.channel.clone(), id)))
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
                        eprintln!("rusty-fish: cannot catch up on channel {channel}: {why}");
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
            eprintln!("rusty-fish: catching up on {from_people} missed message(s)");
        }
        for msg in &missed {
            self.handle(http, msg).await;
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
