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
//! - Es gibt noch kein Kontingent. Das kommt, bevor die KI ansprechbar wird.
//!
//! In die Ausgabe des Programms gelangen nur Kanal-Kennungen und Fehlergründe,
//! nie der Text einer Nachricht.

use crate::config::Config;
use crate::render::{MESSAGE_LIMIT, chunks, hook_name, render};
use crate::state::Store;
use rusty_fish_core::{Model, Outcome, Request, translate};
use rusty_fish_room::{Incoming, RoomState, relay};
use serenity::all::{
    ButtonStyle, ChannelId, Client, Context, CreateAllowedMentions, CreateButton,
    CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage, CreateWebhook,
    EventHandler, ExecuteWebhook, GatewayIntents, GetMessages, Http, Interaction, Message,
    MessageId, Ready, UserId, Webhook,
};
use serenity::async_trait;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tokio::sync::Mutex;

const CONSENT_BUTTON: &str = "rusty-fish-consent";
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
        self.catch_up(&ctx.http).await;
    }

    async fn message(&self, ctx: Context, msg: Message) {
        self.handle(&ctx.http, &msg).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Component(click) = interaction else { return };
        if click.data.custom_id != CONSENT_BUTTON {
            return;
        }
        // Wer drückt, stimmt für sich selbst zu, für niemanden sonst.
        let saved = {
            let mut inner = self.inner.lock().await;
            inner.store.consent(&click.user.id.to_string());
            inner.store.save()
        };
        let reply = match saved {
            Ok(()) => "✓",
            Err(why) => {
                eprintln!("rusty-fish: {why}");
                "⚠"
            }
        };
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().content(reply).ephemeral(true),
        );
        if let Err(why) = click.create_response(&ctx.http, response).await {
            eprintln!("rusty-fish: cannot confirm the consent: {why}");
        }
    }
}

impl<M> Fish<M>
where
    M: Model + Send + Sync + 'static,
{
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
        let consented = author == self.config.host || inner.store.has_consented(&author);
        let incoming =
            Incoming { channel: &channel, author: &author, text: &msg.content, consented };
        let room = &self.config.rooms[room_index];
        let deliveries =
            relay(&self.model, room, &mut inner.states[room_index], &incoming).await;

        let name = hook_name(&display_name(msg));
        let avatar = msg.author.face();
        let attachments: String = msg.attachments.iter().map(|a| format!("\n{}", a.url)).collect();
        let mut delivered = true;
        for delivery in &deliveries {
            let body = render(delivery) + &attachments;
            let posted =
                post(http, &mut inner.hooks, &delivery.channel, &name, &avatar, &body).await;
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
            self.ask_consent(http, msg.channel_id, &channel).await;
        }

        inner.store.saw(&channel, msg.id.get());
        if let Err(why) = inner.store.save() {
            eprintln!("rusty-fish: {why}");
        }
    }

    /// Bittet in der Sprache des Platzes um Zustimmung.
    async fn ask_consent(&self, http: &Http, channel_id: ChannelId, channel: &str) {
        let lang = self.config.lang_of(channel).unwrap_or("English");
        let request = Request {
            speaker: "fish",
            text: CONSENT_TEXT,
            target_lang: lang,
            pass_through: &[],
            context: &[],
        };
        let text = match translate(&self.model, &request).await {
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

/// Setzt einen Text unter fremdem Namen in einem Kanal ab.
async fn post(
    http: &Http,
    hooks: &mut HashMap<ChannelId, Webhook>,
    channel: &str,
    name: &str,
    avatar: &str,
    body: &str,
) -> Result<(), String> {
    let id = channel.parse::<u64>().map_err(|_| "not a channel id".to_owned())?;
    let channel_id = ChannelId::new(id);
    if !hooks.contains_key(&channel_id) {
        hooks.insert(channel_id, find_or_create_hook(http, channel_id).await?);
    }
    for part in chunks(body, MESSAGE_LIMIT) {
        let message = ExecuteWebhook::new()
            .content(part)
            .username(name)
            .avatar_url(avatar)
            // Eine gespiegelte Nachricht darf niemanden ein zweites Mal anpingen.
            .allowed_mentions(CreateAllowedMentions::new());
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
