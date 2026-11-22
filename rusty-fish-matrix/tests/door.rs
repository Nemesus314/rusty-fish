//! Die ganze Tür gegen einen nachgestellten Matrix-Server im selben Prozess:
//! kein Netz, kein Konto, keine echte KI. Der Server kommt aus der
//! Matrix-Bibliothek selbst (`MatrixMockServer`).
//!
//! Geprüft wird das Zusammenspiel, das sich sonst nur in einem echten Raum
//! zeigt: was die Tür dem Haushalt hereinreicht und was sie danach absetzt.
//! Die Räume hier sind unverschlüsselt; ob der Fisch in einem verschlüsselten
//! Raum liest, zeigt nur ein echter Lauf.

use matrix_sdk::ruma::events::room::member::MembershipState;
use matrix_sdk::ruma::{event_id, room_id, user_id};
use matrix_sdk::test_utils::mocks::MatrixMockServer;
use matrix_sdk_test::JoinedRoomBuilder;
use matrix_sdk_test::event_factory::EventFactory;
use rusty_fish_core::{Model, ModelError, Prompt, Reply};
use rusty_fish_house::config::Config;
use rusty_fish_matrix::door::{Command, Profile, run};
use rusty_fish_matrix::session::Access;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::unbounded_channel;

const PROFILE: &str = r#"
model = "some-model"
host = "nemo"
ai_name = "Fish"

[[room]]

  [[room.seat]]
  matrix = "!deutsch:localhost"
  lang = "German"
  owner = "nemo"

  [[room.seat]]
  matrix = "!russisch:localhost"
  lang = "Russian"
  owner = "robin"

[[person]]
name = "nemo"
matrix = "@nemo:localhost"

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

/// Ein eigener Ordner je Test, leer.
fn folder(test: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("rusty-fish-matrix-door-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("store")).unwrap();
    dir
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

/// Was die Tür abgesetzt hat: Raum, Art des Ereignisses, Inhalt.
async fn sent(server: &MatrixMockServer) -> Vec<(String, String, Value)> {
    let mut out = Vec::new();
    for request in server.received_requests().await.unwrap_or_default() {
        if request.method.as_str() != "PUT" {
            continue;
        }
        let path = request.url.path().replace("%21", "!").replace("%3A", ":").replace("%24", "$");
        let parts: Vec<&str> = path.split('/').collect();
        // /_matrix/client/v3/rooms/{room}/send/{type}/{txn}
        let Some(at) = parts.iter().position(|part| *part == "send") else { continue };
        let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        out.push((parts[at - 1].to_owned(), parts[at + 1].to_owned(), body));
    }
    out
}

/// Wartet, bis die Tür so viele Ereignisse abgesetzt hat, und liefert sie.
async fn sent_at_least(server: &MatrixMockServer, count: usize) -> Vec<(String, String, Value)> {
    for _ in 0..300 {
        let all = sent(server).await;
        if all.len() >= count {
            return all;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // Zum Nachsehen, wenn es nicht stimmt: was der Server gefragt wurde.
    let asked: Vec<String> = server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .map(|request| format!("{} {}", request.method, request.url.path()))
        .collect();
    panic!(
        "the door sent {:?}, expected {count} events; the server was asked: {asked:#?}",
        sent(server).await
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_conversation_takes_its_course_through_the_matrix_door() {
    let server = MatrixMockServer::new().await;
    let dir = folder("course");
    let german = room_id!("!deutsch:localhost");
    let russian = room_id!("!russisch:localhost");
    let nemo = user_id!("@nemo:localhost");
    let robin = user_id!("@robin:localhost");
    let me = user_id!("@fish:localhost");

    server.mock_versions().ok().mount().await;
    server.mock_upload_keys().ok().mount().await;
    server.mock_query_keys().ok().mount().await;
    server.mock_room_state_encryption().plain().mount().await;
    // Jedes abgesetzte Ereignis bekommt dieselbe Kennung zurück.
    server.mock_room_send().ok(event_id!("$from-the-fish")).mount().await;
    // Wenn es nichts Neues gibt, antwortet der Server leer.
    server.mock_sync().ok(|_| {}).with_priority(10).mount().await;

    // Der erste Abgleich nach einer frischen Anmeldung: zwei Räume, eine alte
    // Nachricht, die der Server nur noch einmal zeigt, die Notiz eines
    // anderen Bots, und eine neue Nachricht des Gastgebers.
    let soon = now_ms() + 60_000;
    let in_german = EventFactory::new().room(german);
    let in_russian = EventFactory::new().room(russian);
    server
        .mock_sync()
        .ok(|builder| {
            builder.add_joined_room(
                JoinedRoomBuilder::new(german)
                    .add_state_event(
                        in_german.member(nemo).membership(MembershipState::Join).display_name("Nemo"),
                    )
                    .add_state_event(in_german.member(me).membership(MembershipState::Join))
                    .add_timeline_event(
                        in_german.text_msg("von gestern").sender(nemo).server_ts(1000u64),
                    )
                    .add_timeline_event(
                        in_german.notice("ein anderer Bot").sender(robin).server_ts(soon),
                    )
                    .add_timeline_event(
                        in_german
                            .text_msg("Guten Morgen")
                            .sender(nemo)
                            .server_ts(soon + 1)
                            .event_id(event_id!("$morning")),
                    ),
            );
            builder.add_joined_room(
                JoinedRoomBuilder::new(russian)
                    .add_state_event(in_russian.member(me).membership(MembershipState::Join)),
            );
        })
        .mock_once()
        .with_priority(1)
        .mount()
        .await;

    let secret = serde_json::json!({
        "h": server.uri(), "u": me, "d": "DEVICEID", "t": "1234", "p": "not-a-real-passphrase",
    });
    let access = Access::parse(&secret.to_string()).unwrap();
    let fake = Fake::default();
    let profile = Profile {
        name: Some("tea".to_owned()),
        config: Config::parse(PROFILE).unwrap(),
        state_path: dir.join("tea.state.json"),
        model: fake.clone(),
        service: "ai.example".to_owned(),
    };
    let (to_door, commands) = unbounded_channel();
    let serving = Arc::new(Mutex::new(Vec::new()));
    let on_change = {
        let serving = serving.clone();
        Box::new(move |names: &[String]| *serving.lock().unwrap() = names.to_vec())
    };
    let store = dir.join("store");
    let door = tokio::spawn(async move {
        run(&access, &store, vec![profile], false, commands, on_change).await
    });

    // --- der Gastgeber schreibt: am anderen Tisch erscheint die Übersetzung ---
    let started = std::time::Instant::now();
    let all = sent_at_least(&server, 2).await;
    eprintln!("first deliveries after {:?}", started.elapsed());
    assert_eq!(all.len(), 2, "the old message and the notice are passed over: {all:?}");
    // Zuerst die Mitteilung an die Eingeladene, an ihrem Tisch und in ihrer
    // Sprache: sie soll lesen, was der Fisch hier tut, bevor die ersten
    // Übersetzungen kommen.
    let (room, kind, content) = &all[0];
    assert_eq!((room.as_str(), kind.as_str()), ("!russisch:localhost", "m.room.message"));
    assert!(content["body"].as_str().unwrap().contains("приглашает"), "got {content}");
    assert!(content["body"].as_str().unwrap().contains("этой комнаты"), "got {content}");
    assert!(content.get("rusty_fish").is_none(), "the announcement asks nothing");
    let (room, kind, content) = &all[1];
    assert_eq!((room.as_str(), kind.as_str()), ("!russisch:localhost", "m.room.message"));
    // Der Name des Schreibenden, wie er in seinem Raum erscheint, steht davor.
    assert_eq!(content["body"], "Nemo: TRANSLATED");
    assert_eq!(content["formatted_body"], "<strong>Nemo</strong>: TRANSLATED");
    assert_eq!(content["msgtype"], "m.notice", "the fish sends notices");
    assert_eq!(*serving.lock().unwrap(), ["tea"]);
    assert_eq!(fake.0.lock().unwrap().len(), 1, "one call to the AI for one translation");

    // --- der Gast schreibt, bevor er zugestimmt hat ---------------------------
    server
        .mock_sync()
        .ok(|builder| {
            builder.add_joined_room(JoinedRoomBuilder::new(russian).add_timeline_event(
                in_russian.text_msg("Привет").sender(robin).event_id(event_id!("$privet")),
            ));
        })
        .mock_once()
        .with_priority(1)
        .mount()
        .await;
    let all = sent_at_least(&server, 5).await;
    assert_eq!(all.len(), 5, "got {all:?}");
    // Unübersetzt weitergegeben und so gekennzeichnet, in der Sprache des Tisches.
    let (room, _, content) = &all[2];
    assert_eq!(room, "!deutsch:localhost");
    let body = content["body"].as_str().unwrap();
    assert!(body.starts_with("robin: Привет\n(nicht übersetzt: "), "got {body}");
    // Die Bitte steht am Tisch des Gasts, in seiner Sprache, mit dem Dienst.
    let (room, kind, content) = &all[3];
    assert_eq!((room.as_str(), kind.as_str()), ("!russisch:localhost", "m.room.message"));
    assert!(content["body"].as_str().unwrap().contains("ai.example"));
    assert!(content["body"].as_str().unwrap().contains("В этой комнате"));
    assert_eq!(content["rusty_fish"]["ask"], "consent");
    // Darunter das Zeichen, das der Gast antippt.
    let (room, kind, content) = &all[4];
    assert_eq!((room.as_str(), kind.as_str()), ("!russisch:localhost", "m.reaction"));
    assert_eq!(content["m.relates_to"]["key"], "OK");
    assert_eq!(content["m.relates_to"]["event_id"], "$from-the-fish");
    assert_eq!(fake.0.lock().unwrap().len(), 1, "nothing of the guest went to the AI");

    // --- der Gast tippt das Zeichen an: sein Satz wird nachgereicht ------------
    server
        .mock_sync()
        .ok(|builder| {
            builder.add_joined_room(JoinedRoomBuilder::new(russian).add_timeline_event(
                in_russian.reaction(event_id!("$from-the-fish"), "OK").sender(robin),
            ));
        })
        .mock_once()
        .with_priority(1)
        .mount()
        .await;
    let all = sent_at_least(&server, 6).await;
    let (room, _, content) = &all[5];
    assert_eq!(room, "!deutsch:localhost");
    assert_eq!(content["body"], "robin: TRANSLATED");
    assert_eq!(fake.0.lock().unwrap().len(), 2);
    // Die Zustimmung steht in der Zustandsdatei, unter der Matrix-Kennung.
    let state = std::fs::read_to_string(dir.join("tea.state.json")).unwrap();
    assert!(state.contains("@robin:localhost"), "got {state}");
    assert!(!state.contains("Привет"), "the state holds no message text");

    // --- die Erwähnung des Fischs ist eine Frage an die KI ----------------------
    server
        .mock_sync()
        .ok(|builder| {
            builder.add_joined_room(JoinedRoomBuilder::new(german).add_timeline_event(
                in_german
                    .text_msg("@fish:localhost wie spät ist es?")
                    .sender(nemo)
                    .event_id(event_id!("$question")),
            ));
        })
        .mock_once()
        .with_priority(1)
        .mount()
        .await;
    let all = sent_at_least(&server, 8).await;
    let after: Vec<&(String, String, Value)> = all[6..].iter().collect();
    // Was die KI zu lesen bekam, nennt den Rufnamen, nicht die Kennung.
    let read: String =
        fake.0.lock().unwrap().iter().map(|p| format!("{}\n{}\n", p.system, p.user)).collect();
    assert!(!read.contains("@fish:localhost"), "the id of the fish reached the AI");
    // Die Antwort erscheint unter dem Namen, den der Gastgeber der KI gab.
    assert!(
        after.iter().any(|(_, _, content)| content["body"].as_str().unwrap_or("").starts_with("Fish: ")),
        "got {after:?}"
    );

    // --- anhalten ----------------------------------------------------------------
    let asked: Vec<String> = server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .map(|request| format!("{} {}", request.method, request.url.path()))
        .collect();
    eprintln!("the server was asked {} times: {asked:#?}", asked.len());
    to_door.send(Command::StopAll).unwrap();
    let ended = tokio::time::timeout(Duration::from_secs(10), door).await;
    assert_eq!(ended.expect("the door stops when told").unwrap(), Ok(()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_discord_profile_is_turned_away_before_the_fish_shows_itself() {
    let server = MatrixMockServer::new().await;
    let dir = folder("turned-away");
    let secret = serde_json::json!({
        "h": server.uri(), "u": "@fish:localhost", "d": "DEVICEID", "t": "1234",
        "p": "not-a-real-passphrase",
    });
    let access = Access::parse(&secret.to_string()).unwrap();
    let discord = PROFILE
        .replace("matrix = \"!deutsch:localhost\"", "discord = \"111\"")
        .replace("matrix = \"!russisch:localhost\"", "discord = \"222\"")
        .replace("matrix = \"@nemo:localhost\"", "discord = \"100\"")
        .replace("matrix = \"@robin:localhost\"", "discord = \"7\"");
    let profile = Profile {
        name: Some("tea".to_owned()),
        config: Config::parse(&discord).unwrap(),
        state_path: dir.join("tea.state.json"),
        model: Fake::default(),
        service: "ai.example".to_owned(),
    };
    let (_to_door, commands) = unbounded_channel();
    let ended =
        run(&access, &dir.join("store"), vec![profile], false, commands, Box::new(|_| {})).await;
    let err = ended.unwrap_err();
    assert!(err.contains("stands at the Discord door"), "got {err}");
    // Mit dem Server hat der Fisch dafür nicht gesprochen.
    assert!(server.received_requests().await.unwrap_or_default().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
