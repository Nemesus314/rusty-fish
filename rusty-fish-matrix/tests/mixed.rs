//! Der gemischte Raum von der Matrix-Tür aus gesehen: ein Gespräch mit einem
//! Tisch in einem Discord-Kanal und einem in einem Matrix-Raum.
//!
//! Die Matrix-Tür läuft ganz, gegen den nachgestellten Server der
//! Matrix-Bibliothek. Die Discord-Tür ist hier nicht dabei: an ihrer Stelle
//! steht der Test selbst am anderen Ende der Leitung. Er tut, was sie täte:
//! er reicht dem gemeinsamen Haushalt eine Nachricht aus dem Kanal herein und
//! der Matrix-Tür zu, was im Raum erscheinen soll; und er nimmt entgegen, was
//! die Matrix-Tür für den Kanal zureicht.
//!
//! Geprüft wird der Kern: zwei Türen teilen sich den Haushalt eines Profils,
//! und eine Zustellung für einen Tisch an der anderen Tür wird dieser Tür
//! zugereicht. Kein Netz, kein Konto, keine echte KI.

use matrix_sdk::ruma::events::room::member::MembershipState;
use matrix_sdk::ruma::{event_id, room_id, user_id};
use matrix_sdk::test_utils::mocks::MatrixMockServer;
use matrix_sdk_test::JoinedRoomBuilder;
use matrix_sdk_test::event_factory::EventFactory;
use rusty_fish_core::{Model, ModelError, Prompt, Reply};
use rusty_fish_house::between::{Errand, Lines, Shared, Writer};
use rusty_fish_house::config::{Config, Door};
use rusty_fish_house::household::{Arrived, Household, Terms};
use rusty_fish_house::texts::switched;
use rusty_fish_matrix::door::{Command, Profile, run};
use rusty_fish_matrix::session::Access;
use rusty_fish_room::tongue::Tongue;
use rusty_fish_room::{Mark, Reason};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::unbounded_channel;

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
        .join(format!("rusty-fish-matrix-mixed-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("store")).unwrap();
    dir
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

/// Was die Matrix-Tür abgesetzt hat: Raum, Art des Ereignisses, Inhalt. Ohne
/// die Zeilen zum Ein- und Ausschalten.
async fn sent(server: &MatrixMockServer) -> Vec<(String, String, Value)> {
    let mut out = Vec::new();
    for request in server.received_requests().await.unwrap_or_default() {
        if request.method.as_str() != "PUT" {
            continue;
        }
        let path = request.url.path().replace("%21", "!").replace("%3A", ":").replace("%24", "$");
        let parts: Vec<&str> = path.split('/').collect();
        let Some(at) = parts.iter().position(|part| *part == "send") else { continue };
        let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        let switch = [Tongue::German, Tongue::English, Tongue::Russian]
            .iter()
            .any(|t| [true, false].iter().any(|on| body["body"] == switched(*t, *on)));
        if !switch {
            out.push((parts[at - 1].to_owned(), parts[at + 1].to_owned(), body));
        }
    }
    out
}

async fn sent_at_least(server: &MatrixMockServer, count: usize) -> Vec<(String, String, Value)> {
    for _ in 0..300 {
        let all = sent(server).await;
        if all.len() >= count {
            return all;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the door sent {:?}, expected {count} events", sent(server).await);
}

/// Wartet, bis am Discord-Ende der Leitung so viele Aufträge angekommen sind.
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
    panic!("the Discord end of the line got {:?}, expected {count}", seen.lock().unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn two_doors_share_a_household_and_hand_each_other_what_to_deliver() {
    let server = MatrixMockServer::new().await;
    let dir = folder("core");
    let english = room_id!("!english:localhost");
    let robin = user_id!("@robin:localhost");
    let me = user_id!("@fish:localhost");

    server.mock_versions().ok().mount().await;
    server.mock_upload_keys().ok().mount().await;
    server.mock_query_keys().ok().mount().await;
    server.mock_room_state_encryption().plain().mount().await;
    server.mock_room_send().ok(event_id!("$from-the-fish")).mount().await;
    server.mock_sync().ok(|_| {}).with_priority(10).mount().await;
    let in_english = EventFactory::new().room(english);
    server
        .mock_sync()
        .ok(|builder| {
            builder.add_joined_room(
                JoinedRoomBuilder::new(english)
                    .add_state_event(
                        in_english
                            .member(robin)
                            .membership(MembershipState::Join)
                            .display_name("Robin"),
                    )
                    .add_state_event(in_english.member(me).membership(MembershipState::Join)),
            );
        })
        .mock_once()
        .with_priority(1)
        .mount()
        .await;

    // --- was das Programm für ein Profil an zwei Türen anlegt -------------------
    let config = Config::parse(MIXED).unwrap();
    assert_eq!(config.doors, [Door::Discord, Door::Matrix]);
    let state_path = dir.join("tea.state.json");
    let lines = Lines::new();
    let to_matrix = lines.open(Door::Matrix);
    let mut at_discord = lines.open(Door::Discord);
    let house = Arc::new(tokio::sync::Mutex::new(Household::open(&config, &state_path).unwrap()));
    let shared = Shared { house: house.clone(), lines: lines.clone() };

    // Das Discord-Ende der Leitung: merkt sich, was ankommt, und sagt, es sei
    // abgesetzt.
    let seen: Arc<Mutex<Vec<(Option<String>, Errand)>>> = Arc::default();
    tokio::spawn({
        let seen = seen.clone();
        async move {
            while let Some(handed) = at_discord.recv().await {
                seen.lock().unwrap().push((handed.profile.clone(), handed.errand.clone()));
                let _ = handed.done.send(Ok(()));
            }
        }
    });

    let secret = serde_json::json!({
        "h": server.uri(), "u": me, "d": "DEVICEID", "t": "1234", "p": "not-a-real-passphrase",
    });
    let access = Access::parse(&secret.to_string()).unwrap();
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
    let store = dir.join("store");
    let door = tokio::spawn(async move {
        run(&access, &store, vec![profile], false, commands, to_matrix, Box::new(|_| {})).await
    });

    // Die Eingeladene liest an ihrem Tisch, dass der Fisch sie bedient. Der
    // Gastgeber hat an dieser Tür keine Kennung; die Mitteilung nennt niemanden.
    let all = sent_at_least(&server, 1).await;
    assert_eq!(all[0].0, "!english:localhost");
    assert!(all[0].2["body"].as_str().unwrap().contains("of this room"), "got {:?}", all[0]);

    // --- Discord nach Matrix -------------------------------------------------------
    // Der Gastgeber schreibt im Discord-Kanal. Die Discord-Tür fragt den
    // gemeinsamen Haushalt und reicht der Matrix-Tür zu, was im Raum erscheint.
    let terms = Terms { config: &config, service: "ai.example", today: "2026-11-23" };
    let discord_mark = 1_300_000_000_000_000_001u64;
    let steps = {
        let mut house = house.lock().await;
        let arrived =
            Arrived { channel: "111", author: "100", text: "Guten Morgen", mark: discord_mark };
        let steps = house.message(&fake, &terms, &arrived).await.unwrap();
        assert_eq!(steps.deliveries.len(), 1);
        assert_eq!(steps.deliveries[0].channel, "!english:localhost");
        assert_eq!(steps.deliveries[0].mark, Mark::Translated);
        assert_eq!(steps.ask_consent, None, "the host has agreed by starting the fish");
        let writer = Writer {
            door: Door::Discord,
            name: "Nemo".to_owned(),
            face: Some("https://cdn.example/nemo.png".to_owned()),
            attachments: "\nhttps://cdn.example/picture.png".to_owned(),
        };
        let errand = Errand::Deliver { deliveries: steps.deliveries.clone(), writer };
        assert_eq!(lines.hand(Door::Matrix, Some("tea"), errand).await, Ok(()));
        house.save().unwrap();
        steps
    };
    let all = sent_at_least(&server, 2).await;
    assert_eq!(all.len(), 2, "got {all:?}");
    let (room, kind, content) = &all[1];
    assert_eq!((room.as_str(), kind.as_str()), ("!english:localhost", "m.room.message"));
    // Der Name aus Discord steht davor, der Anhang als Adresse darunter.
    assert_eq!(content["body"], "Nemo: TRANSLATED\nhttps://cdn.example/picture.png");
    assert_eq!(content["msgtype"], "m.notice");
    drop(steps);

    // --- Matrix nach Discord -------------------------------------------------------
    // Der Gast schreibt im Matrix-Raum, bevor er zugestimmt hat. Für den
    // Discord-Kanal kommt die Zustellung am anderen Ende der Leitung an,
    // unübersetzt und so gekennzeichnet; die Bitte steht an seinem Tisch.
    let soon = now_ms() + 60_000;
    server
        .mock_sync()
        .ok(|builder| {
            builder.add_joined_room(JoinedRoomBuilder::new(english).add_timeline_event(
                in_english
                    .text_msg("Hello")
                    .sender(robin)
                    .server_ts(soon)
                    .event_id(event_id!("$hello")),
            ));
        })
        .mock_once()
        .with_priority(1)
        .mount()
        .await;
    let handed = handed_at_least(&seen, 1).await;
    let (profile, errand) = &handed[0];
    assert_eq!(profile.as_deref(), Some("tea"));
    let Errand::Deliver { deliveries, writer } = errand else { panic!("got {errand:?}") };
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].channel, "111");
    assert_eq!(deliveries[0].text, "Hello");
    assert_eq!(deliveries[0].mark, Mark::Untranslated(Reason::NoConsent));
    // Unter diesem Namen erscheint er in seinem Raum; ein Bild reicht die
    // Matrix-Tür nicht weiter.
    assert_eq!(writer.name, "Robin");
    assert_eq!(writer.door, Door::Matrix);
    assert_eq!(writer.face, None);
    let all = sent_at_least(&server, 4).await;
    assert_eq!(all.len(), 4, "the request and its sign, at the guest's table: {all:?}");
    assert_eq!(all[2].0, "!english:localhost");
    assert!(all[2].2["body"].as_str().unwrap().contains("In this room"), "got {:?}", all[2]);
    assert_eq!(all[2].2["rusty_fish"]["ask"], "consent");
    assert_eq!(all[3].2["m.relates_to"]["key"], "OK");
    assert_eq!(fake.0.lock().unwrap().len(), 1, "nothing of the guest went to the AI");

    // Der Gast tippt das Zeichen an: sein Satz wird übersetzt nachgereicht,
    // wieder durch die Discord-Tür.
    server
        .mock_sync()
        .ok(|builder| {
            builder.add_joined_room(JoinedRoomBuilder::new(english).add_timeline_event(
                in_english.reaction(event_id!("$from-the-fish"), "OK").sender(robin),
            ));
        })
        .mock_once()
        .with_priority(1)
        .mount()
        .await;
    let handed = handed_at_least(&seen, 2).await;
    let Errand::Deliver { deliveries, writer } = &handed[1].1 else { panic!("got {handed:?}") };
    assert_eq!(deliveries[0].channel, "111");
    assert_eq!(deliveries[0].text, "TRANSLATED");
    assert_eq!(deliveries[0].mark, Mark::Translated);
    assert_eq!(writer.name, "Robin");

    // --- ein Zustand für beide Türen ------------------------------------------------
    // Die Lesemarken stehen nebeneinander, jede so gezählt wie an ihrer Tür:
    // bei Discord die Kennung der Nachricht, bei Matrix ihre Zeit.
    for _ in 0..100 {
        if house.lock().await.seen("!english:localhost").is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let state: Value =
        serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
    assert_eq!(state["last_seen"]["111"], discord_mark);
    assert_eq!(state["last_seen"]["!english:localhost"], soon);
    assert_eq!(state["consented"], serde_json::json!(["@robin:localhost"]));
    assert!(!state.to_string().contains("Hello"), "the state holds no message text");

    // Ist die andere Tür zu, sagt die Leitung es, statt zu warten.
    lines.close(Door::Discord);
    let writer = Writer {
        door: Door::Matrix,
        name: "Robin".to_owned(),
        face: None,
        attachments: String::new(),
    };
    let errand = Errand::Deliver { deliveries: Vec::new(), writer };
    assert_eq!(
        lines.hand(Door::Discord, Some("tea"), errand).await,
        Err("the Discord door is not open".to_owned())
    );

    to_door.send(Command::StopAll).unwrap();
    let ended = tokio::time::timeout(Duration::from_secs(10), door).await;
    assert_eq!(ended.expect("the door stops when told").unwrap(), Ok(()));
    let _ = std::fs::remove_dir_all(&dir);
}
