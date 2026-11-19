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
//! Das Kontingent und die ansprechbare KI stehen in `rusty-fish-room`. Die
//! Tür reicht nur das heutige Datum und den Tageszähler hinein und legt den
//! Zähler in ihrer Zustandsdatei ab.
//!
//! In die Ausgabe des Programms gelangen nur Kanal-Kennungen und Fehlergründe,
//! nie der Text einer Nachricht.

use crate::clock::today;
use crate::config::Config;
use crate::mention::mention_as_call;
use crate::render::{
    KEEP_LABEL, KEPT_NOTICE, MESSAGE_LIMIT, NOT_THE_HOST, budget_notice, chunks, hook_name,
    nothing_to_raise, raise_label, raised_notice, render,
};
use crate::state::Store;
use rusty_fish_core::{Model, Outcome, Request, translate};
use rusty_fish_room::{
    House, Incoming, Metered, RoomState, Sender, Tally, budget_ran_out, converse,
};
use serenity::all::{
    ButtonStyle, ChannelId, Client, ComponentInteraction, Context, CreateAllowedMentions,
    CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage,
    CreateWebhook, EditInteractionResponse, EventHandler, ExecuteWebhook, GatewayIntents,
    GetMessages, Http, Interaction, Message, MessageId, Ready, UserId, Webhook,
};
use serenity::async_trait;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;

const CONSENT_BUTTON: &str = "rusty-fish-consent";
/// Die beiden Knöpfe am Hinweis an den Gastgeber.
const RAISE_BUTTON: &str = "rusty-fish-raise";
const KEEP_BUTTON: &str = "rusty-fish-keep";
const WEBHOOK_NAME: &str = "Rusty Fish";

/// Der Text, mit dem der Fisch um Zustimmung bittet. Er wird in die Sprache
/// des Platzes übersetzt; er enthält kein Wort eines Menschen, deshalb darf
/// er vor der Zustimmung zur KI.
const CONSENT_TEXT: &str = "Hello! I am Rusty Fish, a translation bot. In this channel everyone \
    writes in their own language, and an AI translates in between. To do that, the AI reads what \
    you write here. Nothing you write is sent to the AI before you agree. Press OK to agree. \
    Until then your messages are passed on untranslated.";

struct Fish<M> {
    model: M,
    config: Config,
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
    states: Vec<RoomState>,
    store: Store,
    hooks: HashMap<ChannelId, Webhook>,
    /// Wen der Fisch in diesem Lauf schon um Zustimmung gebeten hat.
    asked: HashSet<UserId>,
}

/// Startet die Tür und läuft, bis die Verbindung endet.
pub async fn run<M>(token: &str, config: Config, state_path: &Path, model: M) -> Result<(), String>
where
    M: Model + Send + Sync + 'static,
{
    let store = Store::open(state_path)?;
    let states = config.rooms.iter().map(|_| RoomState::default()).collect();
    let fish = Fish {
        model,
        config,
        me: AtomicU64::new(0),
        inner: Mutex::new(Inner { states, store, hooks: HashMap::new(), asked: HashSet::new() }),
    };

    // MESSAGE_CONTENT muss im Entwicklerportal von Discord eingeschaltet sein,
    // sonst kommen die Nachrichten ohne Text an.
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(token, intents)
        .event_handler(fish)
        .await
        .map_err(|e| format!("cannot set up the Discord connection: {e}"))?;
    client.start().await.map_err(|e| format!("the Discord connection ended: {e}"))
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
        let saved = {
            let mut inner = self.inner.lock().await;
            inner.store.consent(&click.user.id.to_string());
            inner.store.save()
        };
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
            let limits = &self.config.limits;
            let mut inner = self.inner.lock().await;
            let mut tally = inner.store.tally();
            // Seit der Hinweis hinausging, kann ein neuer Tag begonnen haben.
            tally.start_day(&today());
            let raised = tally.raise(limits);
            let (allowed, left) = (tally.allowed_today(limits), tally.tokens_left(limits));
            inner.store.set_tally(&tally);
            if let Err(why) = inner.store.save() {
                eprintln!("rusty-fish: {why}");
            }
            if raised { raised_notice(allowed, left) } else { nothing_to_raise(allowed, left) }
        };

        // Die Antwort ersetzt den Hinweis und nimmt die Knöpfe weg, damit kein
        // alter Knopf stehen bleibt.
        let edit = EditInteractionResponse::new().content(text).components(Vec::new());
        if let Err(why) = click.edit_response(http, edit).await {
            eprintln!("rusty-fish: cannot answer the host: {why}");
        }
    }

    /// Sagt dem Gastgeber, und nur ihm, dass das heutige Kontingent verbraucht
    /// ist, und fragt, ob er um denselben Betrag erhöhen will.
    ///
    /// Nemesus am 19.11.2026: „Meine Idee wäre eine Systemnachricht, der nur an
    /// den Gastgeber geht. Gleich mit Abfage, ob er es wieder um den selben
    /// Betrag erhöhen soll, mit Beitragnennung".
    async fn notify_host(&self, http: &Http, tally: &Tally) {
        let Ok(id) = self.config.host.parse::<u64>() else { return };
        let limits = &self.config.limits;
        let step = limits.translation_tokens_per_day;
        let message = CreateMessage::new()
            .content(budget_notice(tally.allowed_today(limits), step))
            .button(
                CreateButton::new(RAISE_BUTTON)
                    .label(raise_label(step))
                    .style(ButtonStyle::Success),
            )
            .button(CreateButton::new(KEEP_BUTTON).label(KEEP_LABEL).style(ButtonStyle::Secondary));
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
        let Some(room_index) = self.config.room_index(&channel) else { return };

        let mut guard = self.inner.lock().await;
        let inner = &mut *guard;
        // Schon verarbeitet, etwa weil Nachholen und Live-Meldung sich überschneiden.
        if inner.store.seen(&channel).is_some_and(|seen| seen >= msg.id.get()) {
            return;
        }

        let author = msg.author.id.to_string();
        let is_host = author == self.config.host;
        let consented = is_host || inner.store.has_consented(&author);
        let names: Vec<&str> = self.config.ai_names.iter().map(String::as_str).collect();
        // Die echte Erwähnung des Bots am Anfang zählt wie sein Rufname. Die
        // Raum-Logik bekommt dafür den Rufnamen statt des Discord-Kürzels; so
        // sehen weder die KI noch die anderen Plätze das Kürzel. Ohne Rufnamen
        // ist die Funktion aus, und die Nachricht bleibt, wie sie ist.
        let called = names
            .first()
            .and_then(|name| mention_as_call(&msg.content, self.me.load(Ordering::Relaxed), name));
        let text = called.as_deref().unwrap_or(&msg.content);
        let incoming = Incoming { channel: &channel, author: &author, text, consented };
        let room = &self.config.rooms[room_index];
        let today = today();
        let house = House { limits: &self.config.limits, ai_names: &names, today: &today };
        let mut tally = inner.store.tally();
        let deliveries = converse(
            &self.model,
            room,
            &mut inner.states[room_index],
            &mut tally,
            &house,
            &incoming,
            is_host,
        )
        .await;

        // Was ein Mensch schrieb, erscheint unter seinem Namen und Bild. Was
        // die KI sagt, erscheint unter dem Namen, den der Gastgeber ihr gab.
        let writer = hook_name(&display_name(msg));
        let face = msg.author.face();
        let assistant = hook_name(self.config.ai_name().unwrap_or(WEBHOOK_NAME));
        let attachments: String = msg.attachments.iter().map(|a| format!("\n{}", a.url)).collect();
        let mut delivered = true;
        for delivery in &deliveries {
            let (name, avatar, body) = match delivery.from {
                Sender::Writer => {
                    (writer.as_str(), Some(face.as_str()), render(delivery) + &attachments)
                }
                Sender::Assistant => (assistant.as_str(), None, render(delivery)),
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

        if !consented && inner.asked.insert(msg.author.id) {
            self.ask_consent(http, msg.channel_id, &channel, &mut tally).await;
        }

        // Ging etwas unübersetzt hinaus, weil das Kontingent leer ist, erfährt
        // es der Gastgeber, einmal je Erschöpfung.
        if budget_ran_out(&deliveries) && tally.notice_due(&self.config.limits) {
            self.notify_host(http, &tally).await;
        }

        inner.store.set_tally(&tally);
        inner.store.saw(&channel, msg.id.get());
        if let Err(why) = inner.store.save() {
            eprintln!("rusty-fish: {why}");
        }
    }

    /// Bittet in der Sprache des Platzes um Zustimmung.
    async fn ask_consent(
        &self,
        http: &Http,
        channel_id: ChannelId,
        channel: &str,
        tally: &mut Tally,
    ) {
        let seat = self.config.seat_of(channel);
        let lang = seat.map(|s| s.lang.as_str()).unwrap_or("English");
        let request = Request {
            speaker: "fish",
            text: CONSENT_TEXT,
            target_lang: lang,
            pass_through: &[],
            context: &[],
            writer_gender: None,
            // Die Bitte spricht die Person an diesem Platz an.
            reader_gender: seat.and_then(|s| s.gender),
        };
        // Auch diese Übersetzung zählt gegen den Übersetzungstopf. Ist er
        // leer, fragt der Fisch auf Englisch.
        let meter = Metered::new(&self.model, tally.tokens_left(&self.config.limits));
        let translated = translate(&meter, &request).await;
        tally.spend(meter.used());
        let text = match translated {
            Ok(Outcome::Translated(t)) => t,
            // Lieber auf Englisch fragen als gar nicht.
            _ => CONSENT_TEXT.to_owned(),
        };
        // „OK" statt eines Satzes: ein Satz wie „Ich stimme zu" hat in manchen
        // Sprachen ein Geschlecht, und der Fisch weiß nicht, wer drückt.
        let message = CreateMessage::new()
            .content(text)
            .button(CreateButton::new(CONSENT_BUTTON).label("OK").style(ButtonStyle::Success));
        if let Err(why) = channel_id.send_message(http, message).await {
            eprintln!("rusty-fish: cannot ask for consent in channel {channel}: {why}");
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
                .filter_map(|s| inner.store.seen(&s.channel).map(|id| (s.channel.clone(), id)))
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
        if !missed.is_empty() {
            eprintln!("rusty-fish: catching up on {} missed message(s)", missed.len());
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
