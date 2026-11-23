//! Die ganze Tür gegen ein nachgestelltes Discord im selben Prozess: kein
//! Netz nach außen, kein Konto, keine echte KI.
//!
//! Nachgestellt sind die beiden Seiten, über die ein Bot mit Discord spricht:
//! die Schnittstelle (ein Server, der Anfragen annimmt und sich merkt) und die
//! Verbindung, über die Discord meldet, was geschieht (ein WebSocket, über
//! den der Test Nachrichten und Klicks hineinreicht). Geprüft wird das
//! Zusammenspiel, das sich sonst nur auf einem echten Server zeigt.
//!
//! Seit dem 23.11.2026, mit dem gemischten Raum. Die Matrix-Tür ist hier
//! nicht dabei: an ihrer Stelle steht der Test am anderen Ende der Leitung,
//! so wie in `rusty-fish-matrix/tests/mixed.rs` umgekehrt.
//!
//! Was die Attrappe nicht kann: sie prüft nicht, ob Discord eine Anfrage
//! annähme (Rechte, Längen, verbotene Namen), und sie drosselt nicht.

use futures_util::{SinkExt, StreamExt};
use rusty_fish_core::{Model, ModelError, Prompt, Reply};
use rusty_fish_discord::buttons::CONSENT;
use rusty_fish_discord::door::{Command, Profile, run_with};
use rusty_fish_house::between::{Errand, Lines, Shared, Writer, no_line};
use rusty_fish_house::config::{Config, Door};
use rusty_fish_house::household::{Arrived, HostNotice, Household, Terms};
use rusty_fish_room::{Mark, Reason};
use serde_json::{Value, json};
use serenity::all::HttpBuilder;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio_tungstenite::tungstenite::Message as Frame;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Ein Gespräch an zwei Türen: der Gastgeber liest Deutsch in einem
/// Discord-Kanal, sein Gast Englisch in einem Matrix-Raum.
const MIXED: &str = r#"
model = "some-model"
host = "nemo"
ai_name = "Fish"

[[room]]

  [[room.seat]]
  discord = "111"
  lang = "German"
  owner = "nemo"

  [[room.seat]]
  matrix = "!english:localhost"
  lang = "English"
  owner = "robin"

[[person]]
name = "nemo"
discord = "100"

[[person]]
name = "robin"
matrix = "@robin:localhost"
invited = true
"#;

/// Ein Gespräch nur bei Discord, wie von Anfang an.
const PLAIN: &str = r#"
model = "some-model"
host = "100"

[[room]]

  [[room.seat]]
  channel = "111"
  lang = "German"

  [[room.seat]]
  channel = "222"
  lang = "English"
"#;

/// Attrappe einer KI: antwortet sofort, immer dasselbe, und zählt mit.
#[derive(Clone, Default)]
struct Fake(Arc<Mutex<Vec<Prompt>>>);

impl Model for Fake {
    async fn complete(&self, prompt: &Prompt) -> Result<Reply, ModelError> {
        self.0.lock().unwrap().push(prompt.clone());
        Ok(Reply { text: "TRANSLATED".to_owned(), tokens: Some(10) })
    }
}

fn folder(test: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("rusty-fish-discord-door-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn user(id: &str, name: &str, shown: &str) -> Value {
    json!({ "id": id, "username": name, "discriminator": "0", "global_name": shown, "avatar": null })
}

/// Eine Nachricht, wie Discord sie meldet.
fn message(id: u64, channel: &str, author: Value, content: &str) -> Value {
    json!({
        "id": id.to_string(), "channel_id": channel, "author": author, "content": content,
        "timestamp": "2026-11-23T10:00:00+00:00", "edited_timestamp": null, "tts": false,
        "mention_everyone": false, "mentions": [], "mention_roles": [], "attachments": [],
        "embeds": [], "pinned": false, "type": 0,
    })
}

/// Das nachgestellte Discord.
struct Discord {
    rest: MockServer,
    /// Was Discord der Tür meldet: Art des Ereignisses und Inhalt.
    events: UnboundedSender<(String, Value)>,
    /// Wird gesetzt, sobald der Bot sich über die Verbindung angemeldet hat.
    identified: Arc<tokio::sync::Notify>,
}

impl Discord {
    async fn start() -> Discord {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let gateway = format!("ws://{}", listener.local_addr().unwrap());
        let (events, to_send) = unbounded_channel::<(String, Value)>();
        let identified = Arc::new(tokio::sync::Notify::new());
        tokio::spawn(serve_gateway(listener, to_send, gateway.clone(), identified.clone()));

        let rest = MockServer::start().await;
        let bot = user("999", "fish", "Fish");
        let from_the_fish = message(5000, "111", bot.clone(), "");
        let answers = [
            ("GET", r"^/api/v10/gateway$", 200, json!({ "url": gateway })),
            // Der Webhook des Fischs ist schon da.
            (
                "GET",
                r"^/api/v10/channels/\d+/webhooks$",
                200,
                json!([{ "id": "900", "type": 1, "guild_id": "1", "channel_id": "111",
                         "name": "Rusty Fish", "avatar": null, "token": "hook-token",
                         "application_id": null }]),
            ),
            ("POST", r"^/api/v10/webhooks/900/hook-token$", 204, Value::Null),
            ("POST", r"^/api/v10/channels/\d+/messages$", 200, from_the_fish.clone()),
            ("POST", r"^/api/v10/interactions/\d+/[^/]+/callback$", 204, Value::Null),
            ("PATCH", r"^/api/v10/webhooks/\d+/[^/]+/messages/@original$", 200, from_the_fish),
            (
                "POST",
                r"^/api/v10/users/@me/channels$",
                200,
                json!({ "id": "777", "type": 1, "last_message_id": null,
                        "recipients": [user("100", "nemo", "Nemo")] }),
            ),
        ];
        for (verb, route, status, body) in answers {
            let answer = match body {
                Value::Null => ResponseTemplate::new(status),
                body => ResponseTemplate::new(status).set_body_json(body),
            };
            Mock::given(method(verb)).and(path_regex(route)).respond_with(answer).mount(&rest).await;
        }
        // Alles andere gibt es hier nicht; die Tür soll es nicht brauchen.
        Mock::given(path("/never")).respond_with(ResponseTemplate::new(404)).mount(&rest).await;
        Discord { rest, events, identified }
    }

    /// Der Zugang der Tür zu dieser Schnittstelle.
    fn http(&self) -> serenity::all::Http {
        HttpBuilder::new("not-a-real-token")
            .proxy(self.rest.uri())
            .ratelimiter_disabled(true)
            .build()
    }

    /// Discord meldet der Tür etwas.
    fn tell(&self, kind: &str, data: Value) {
        self.events.send((kind.to_owned(), data)).unwrap();
    }

    /// Was die Tür bei Discord abgesetzt hat: Verb, Pfad, Inhalt. Ohne die
    /// Fragen nach der Verbindung und nach Webhooks.
    async fn sent(&self) -> Vec<(String, String, Value)> {
        let mut out = Vec::new();
        for request in self.rest.received_requests().await.unwrap_or_default() {
            let verb = request.method.as_str().to_owned();
            if verb == "GET" {
                continue;
            }
            let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
            out.push((verb, request.url.path().to_owned(), body));
        }
        out
    }

    async fn sent_at_least(&self, count: usize) -> Vec<(String, String, Value)> {
        for _ in 0..300 {
            let all = self.sent().await;
            if all.len() >= count {
                return all;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let asked: Vec<String> = self
            .rest
            .received_requests()
            .await
            .unwrap_or_default()
            .iter()
            .map(|request| format!("{} {}", request.method, request.url.path()))
            .collect();
        panic!("the door sent {:?}, expected {count}; it asked: {asked:#?}", self.sent().await);
    }
}

/// Die Verbindung: begrüßt den Bot, nimmt seine Anmeldung an, antwortet auf
/// seinen Herzschlag und meldet, was der Test hineinreicht.
async fn serve_gateway(
    listener: TcpListener,
    mut to_send: UnboundedReceiver<(String, Value)>,
    gateway: String,
    identified: Arc<tokio::sync::Notify>,
) {
    // Eine Verbindung genügt; baut der Bot neu auf, nimmt die Schleife sie an.
    while let Ok((stream, _)) = listener.accept().await {
        let Ok(mut socket) = tokio_tungstenite::accept_async(stream).await else { continue };
        let hello = json!({ "op": 10, "d": { "heartbeat_interval": 45000 } });
        if socket.send(Frame::Text(hello.to_string())).await.is_err() {
            continue;
        }
        let mut sequence = 0u64;
        loop {
            tokio::select! {
                frame = socket.next() => {
                    let Some(Ok(frame)) = frame else { break };
                    let Frame::Text(text) = frame else { continue };
                    let Ok(said) = serde_json::from_str::<Value>(&text) else { continue };
                    match said["op"].as_u64() {
                        // Die Anmeldung: Discord antwortet mit „bereit".
                        Some(2) => {
                            sequence += 1;
                            let ready = json!({ "op": 0, "t": "READY", "s": sequence, "d": {
                                "v": 10, "user": { "id": "999", "username": "fish",
                                    "discriminator": "0", "global_name": null, "avatar": null,
                                    "bot": true },
                                "guilds": [], "session_id": "session",
                                "resume_gateway_url": gateway, "shard": [0, 1],
                                "application": { "id": "999", "flags": 0 },
                            }});
                            if socket.send(Frame::Text(ready.to_string())).await.is_err() {
                                break;
                            }
                            identified.notify_one();
                        }
                        // Der Herzschlag wird quittiert.
                        Some(1) => {
                            let ack = json!({ "op": 11 });
                            if socket.send(Frame::Text(ack.to_string())).await.is_err() {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                event = to_send.recv() => {
                    let Some((kind, data)) = event else { return };
                    sequence += 1;
                    let event = json!({ "op": 0, "t": kind, "s": sequence, "d": data });
                    if socket.send(Frame::Text(event.to_string())).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}

/// Wartet, bis am Matrix-Ende der Leitung so viele Aufträge angekommen sind.
async fn handed_at_least(
    seen: &Arc<Mutex<Vec<(Option<String>, Errand)>>>,
    count: usize,
) -> Vec<(Option<String>, Errand)> {
    for _ in 0..300 {
        let all = seen.lock().unwrap().clone();
        if all.len() >= count {
            return all;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the Matrix end of the line got {:?}, expected {count}", seen.lock().unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_conversation_at_discord_alone_takes_its_course_as_before() {
    let discord = Discord::start().await;
    let dir = folder("plain");
    let fake = Fake::default();
    let profile = Profile {
        name: Some("plain".to_owned()),
        config: Config::parse(PLAIN).unwrap(),
        state_path: dir.join("plain.state.json"),
        model: fake.clone(),
        service: "ai.example".to_owned(),
        shared: None,
    };
    let (to_door, commands) = unbounded_channel();
    let http = discord.http();
    let door = tokio::spawn(async move {
        run_with(http, vec![profile], false, commands, no_line(), Box::new(|_| {})).await
    });
    tokio::time::timeout(Duration::from_secs(10), discord.identified.notified())
        .await
        .expect("the bot identifies itself");

    // Der Gastgeber schreibt: am anderen Tisch erscheint die Übersetzung
    // unter seinem Namen und mit seinem Bild.
    discord.tell("MESSAGE_CREATE", message(6001, "111", user("100", "nemo", "Nemo"), "Guten Morgen"));
    let all = discord.sent_at_least(1).await;
    assert_eq!(all.len(), 1, "got {all:?}");
    let (verb, path, body) = &all[0];
    assert_eq!((verb.as_str(), path.as_str()), ("POST", "/api/v10/webhooks/900/hook-token"));
    assert_eq!(body["content"], "TRANSLATED");
    assert_eq!(body["username"], "Nemo");
    assert!(body["avatar_url"].as_str().is_some_and(|url| url.starts_with("https://")), "got {body}");
    assert_eq!(fake.0.lock().unwrap().len(), 1);

    // Ein Gast wird um Zustimmung gebeten, mit einem Knopf, an seinem Tisch.
    discord.tell("MESSAGE_CREATE", message(6002, "222", user("555", "guest", "Guest"), "Hello"));
    let all = discord.sent_at_least(3).await;
    assert_eq!(all[1].1, "/api/v10/webhooks/900/hook-token");
    assert!(all[1].2["content"].as_str().unwrap().starts_with("Hello\n-# nicht übersetzt"));
    assert_eq!(all[2].1, "/api/v10/channels/222/messages");
    assert!(all[2].2["content"].as_str().unwrap().contains("In this channel"), "got {:?}", all[2]);
    assert_eq!(all[2].2["components"][0]["components"][0]["custom_id"], CONSENT);

    to_door.send(Command::StopAll).unwrap();
    let ended = tokio::time::timeout(Duration::from_secs(10), door).await;
    assert_eq!(ended.expect("the door stops when told").unwrap(), Ok(()));
    let state = std::fs::read_to_string(dir.join("plain.state.json")).unwrap();
    assert!(state.contains("6002"), "the reading mark is the id of the message: {state}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_discord_door_shares_a_household_and_hands_over_what_belongs_to_matrix() {
    let discord = Discord::start().await;
    let dir = folder("mixed");

    // --- was das Programm für ein Profil an zwei Türen anlegt -------------------
    let config = Config::parse(MIXED).unwrap();
    let state_path = dir.join("tea.state.json");
    let lines = Lines::new();
    let to_discord = lines.open(Door::Discord);
    let mut at_matrix = lines.open(Door::Matrix);
    let house = Arc::new(tokio::sync::Mutex::new(Household::open(&config, &state_path).unwrap()));
    let shared = Shared { house: house.clone(), lines: lines.clone() };

    // Das Matrix-Ende der Leitung: merkt sich, was ankommt, und sagt, es sei
    // abgesetzt.
    let seen: Arc<Mutex<Vec<(Option<String>, Errand)>>> = Arc::default();
    tokio::spawn({
        let seen = seen.clone();
        async move {
            while let Some(handed) = at_matrix.recv().await {
                seen.lock().unwrap().push((handed.profile.clone(), handed.errand.clone()));
                let _ = handed.done.send(Ok(()));
            }
        }
    });

    let fake = Fake::default();
    let profile = Profile {
        name: Some("tea".to_owned()),
        config: config.clone(),
        state_path: state_path.clone(),
        model: fake.clone(),
        service: "ai.example".to_owned(),
        shared: Some(shared),
    };
    let (to_door, commands) = unbounded_channel();
    let http = discord.http();
    let door = tokio::spawn(async move {
        run_with(http, vec![profile], false, commands, to_discord, Box::new(|_| {})).await
    });
    tokio::time::timeout(Duration::from_secs(10), discord.identified.notified())
        .await
        .expect("the bot identifies itself");

    // --- Discord nach Matrix -------------------------------------------------------
    // Der Gastgeber schreibt im Kanal. Im Kanal selbst erscheint nichts; für
    // den Matrix-Raum kommt die Zustellung am anderen Ende der Leitung an.
    discord.tell("MESSAGE_CREATE", message(6001, "111", user("100", "nemo", "Nemo"), "Guten Morgen"));
    let handed = handed_at_least(&seen, 1).await;
    let (profile, errand) = &handed[0];
    assert_eq!(profile.as_deref(), Some("tea"));
    let Errand::Deliver { deliveries, writer } = errand else { panic!("got {errand:?}") };
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].channel, "!english:localhost");
    assert_eq!(deliveries[0].text, "TRANSLATED");
    assert_eq!(deliveries[0].mark, Mark::Translated);
    assert_eq!((writer.door, writer.name.as_str()), (Door::Discord, "Nemo"));
    assert!(writer.face.is_some(), "Discord gives a picture anyone can fetch");
    assert!(discord.sent().await.is_empty(), "nothing is posted at Discord: {:?}", discord.sent().await);

    // --- Matrix nach Discord -------------------------------------------------------
    // Der Gast schreibt im Matrix-Raum, bevor er zugestimmt hat. Die
    // Matrix-Tür fragt den gemeinsamen Haushalt und reicht der Discord-Tür zu,
    // was im Kanal erscheint.
    let terms = Terms { config: &config, service: "ai.example", today: "2026-11-23" };
    let robin = Writer {
        door: Door::Matrix,
        name: "Robin".to_owned(),
        face: None,
        attachments: String::new(),
    };
    {
        let mut house = house.lock().await;
        let arrived = Arrived {
            channel: "!english:localhost",
            author: "@robin:localhost",
            text: "Hello",
            mark: 1_800_000_000_000,
        };
        let steps = house.message(&fake, &terms, &arrived).await.unwrap();
        assert_eq!(steps.deliveries[0].channel, "111");
        assert_eq!(steps.deliveries[0].mark, Mark::Untranslated(Reason::NoConsent));
        assert!(steps.ask_consent.is_some(), "the request goes to the guest's own table");
        let errand = Errand::Deliver { deliveries: steps.deliveries, writer: robin.clone() };
        assert_eq!(lines.hand(Door::Discord, Some("tea"), errand).await, Ok(()));
        house.save().unwrap();
    }
    let all = discord.sent_at_least(1).await;
    assert_eq!(all.len(), 1, "got {all:?}");
    let (verb, path, body) = &all[0];
    assert_eq!((verb.as_str(), path.as_str()), ("POST", "/api/v10/webhooks/900/hook-token"));
    // Unter dem Namen aus dem Matrix-Raum, mit der Tür dahinter, ohne Bild.
    assert_eq!(body["username"], "Robin (Matrix)");
    assert!(body.get("avatar_url").is_none(), "got {body}");
    // Die graue Zeile steht in der Sprache des Tisches, an dem sie erscheint.
    assert!(body["content"].as_str().unwrap().starts_with("Hello\n-# nicht übersetzt"), "got {body}");

    // Nach seiner Zustimmung (im Matrix-Raum) kommt der Satz übersetzt nach.
    {
        let mut house = house.lock().await;
        house.consent("@robin:localhost", "ai.example").unwrap();
        let held = house.held_back(&fake, &terms, "@robin:localhost").await;
        assert_eq!(held.len(), 1);
        let errand = Errand::Deliver { deliveries: held, writer: robin.clone() };
        assert_eq!(lines.hand(Door::Discord, Some("tea"), errand).await, Ok(()));
        house.save().unwrap();
    }
    let all = discord.sent_at_least(2).await;
    assert_eq!(all[1].2["content"], "TRANSLATED");
    assert_eq!(all[1].2["username"], "Robin (Matrix)");

    // --- ein Gast bei Discord: Bitte mit Knopf, dann der Klick ------------------------
    discord.tell("MESSAGE_CREATE", message(6002, "111", user("555", "guest", "Gast"), "Hallo"));
    let handed = handed_at_least(&seen, 2).await;
    let Errand::Deliver { deliveries, writer } = &handed[1].1 else { panic!("got {handed:?}") };
    assert_eq!(deliveries[0].mark, Mark::Untranslated(Reason::NoConsent));
    assert_eq!(writer.name, "Gast");
    let all = discord.sent_at_least(3).await;
    assert_eq!(all[2].1, "/api/v10/channels/111/messages");
    assert!(all[2].2["content"].as_str().unwrap().contains("In diesem Kanal"), "got {:?}", all[2]);
    assert_eq!(all[2].2["components"][0]["components"][0]["custom_id"], CONSENT);

    discord.tell(
        "INTERACTION_CREATE",
        json!({
            "id": "8001", "application_id": "999", "type": 3, "token": "click-token",
            "version": 1, "channel_id": "111", "locale": "en-US", "entitlements": [],
            "attachment_size_limit": 1_000_000,
            "data": { "custom_id": CONSENT, "component_type": 2 },
            "user": user("555", "guest", "Gast"),
            "message": message(5000, "111", user("999", "fish", "Fish"), "the request"),
        }),
    );
    let handed = handed_at_least(&seen, 3).await;
    let Errand::Deliver { deliveries, writer } = &handed[2].1 else { panic!("got {handed:?}") };
    assert_eq!(deliveries[0].channel, "!english:localhost");
    assert_eq!(deliveries[0].mark, Mark::Translated);
    assert_eq!(writer.name, "Gast");

    // --- der Hinweis zum leeren Kontingent, zugereicht von der Matrix-Tür ------------
    // Der Tisch des Gastgebers steht bei Discord; dort liest er den Hinweis.
    assert_eq!(config.host_door(), Door::Discord);
    let notice = HostNotice {
        text: "the budget is used up".to_owned(),
        raise_label: "Raise by 100,000".to_owned(),
        keep_label: "Leave it",
    };
    assert_eq!(lines.hand(Door::Discord, Some("tea"), Errand::TellHost(notice)).await, Ok(()));
    let all = discord.sent().await;
    let dm = all.iter().find(|(_, path, _)| path == "/api/v10/channels/777/messages");
    let (_, _, body) = dm.unwrap_or_else(|| panic!("no message to the host among {all:?}"));
    assert!(body["content"].as_str().unwrap().contains("the budget is used up"));
    assert!(body["content"].as_str().unwrap().contains("Profile: tea"));

    // --- ein Zustand für beide Türen ------------------------------------------------
    let state: Value =
        serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
    assert_eq!(state["last_seen"]["111"], 6002);
    assert_eq!(state["last_seen"]["!english:localhost"], 1_800_000_000_000u64);
    assert_eq!(state["consented"], json!(["555", "@robin:localhost"]));

    to_door.send(Command::StopAll).unwrap();
    let ended = tokio::time::timeout(Duration::from_secs(10), door).await;
    assert_eq!(ended.expect("the door stops when told").unwrap(), Ok(()));
    let _ = std::fs::remove_dir_all(&dir);
}
