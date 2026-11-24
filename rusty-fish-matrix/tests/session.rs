//! Die Sitzung des Fischs gegen einen nachgestellten Matrix-Server: kein
//! Netz, kein Konto.

use matrix_sdk::ruma::CanonicalJsonValue;
use matrix_sdk::test_utils::mocks::MatrixMockServer;
use matrix_sdk_crypto::vodozemac::{Curve25519PublicKey, Curve25519SecretKey, Ed25519SecretKey};
use rusty_fish_matrix::session::{Access, Device, connect, device, login, logout};
use serde_json::{Value, json};
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, ResponseTemplate};
use std::path::{Path, PathBuf};

/// Ein eigener Ordner je Test, leer.
fn folder(test: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("rusty-fish-matrix-session-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("store")).unwrap();
    dir
}

/// Die Dateien, die die Matrix-Bibliothek in der Ablage angelegt hat.
fn kept(store: &Path) -> Vec<String> {
    std::fs::read_dir(store)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("matrix-sdk-"))
        .collect()
}

/// Aus dem ersten echten Lauf am 22.11.2026: der Server nahm den Zugang des
/// Fischs nicht mehr an. Abmelden scheiterte daran, dass das Programm seine
/// Ablage zum Abmelden selbst öffnete und sie dann unter Windows nicht
/// löschen konnte. Der Zugang blieb gespeichert, die Maske zeigte weiter
/// „logged in" und bot kein neues Anmelden an.
#[tokio::test(flavor = "multi_thread")]
async fn a_fish_the_server_has_thrown_out_can_still_log_out() {
    let server = MatrixMockServer::new().await;
    let dir = folder("thrown-out");
    let store = dir.join("store");
    let secret = serde_json::json!({
        "h": server.uri(), "u": "@fish:localhost", "d": "DEVICEID", "t": "1234",
        "p": "not-a-real-passphrase",
    });
    let access = Access::parse(&secret.to_string()).unwrap();

    // Eine Ablage wie nach einem früheren Lauf. In einem eigenen Faden mit
    // eigener Laufzeit angelegt: ist sie zu Ende, hält dieser Test selbst
    // keine der Dateien mehr offen.
    {
        let (access, store) = (access.clone(), store.clone());
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async { connect(&access, &store).await.map(|_| ()) })
        })
        .join()
        .unwrap()
        .unwrap();
    }
    assert!(!kept(&store).is_empty(), "the store holds the device");

    // Der Server kennt den Zugang nicht mehr.
    server.mock_logout().error_unknown_token(false).mount().await;

    let told = logout(&access, &store).await.expect("logging out works all the same");
    assert!(told.is_some(), "it says that the server was not told");
    assert!(kept(&store).is_empty(), "the store is empty: {:?}", kept(&store));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Ein Konto, das keine Wiederherstellung eingerichtet hat: der Fisch meldet
/// sich trotzdem an und sagt, warum sein Gerät ungeprüft bleibt. Der Weg,
/// auf dem der Schlüssel das Gerät wirklich ausweist, braucht ein echtes
/// Konto und ist hier nicht nachgestellt.
#[tokio::test(flavor = "multi_thread")]
async fn a_recovery_key_for_an_account_without_recovery_does_not_stop_the_login() {
    let server = MatrixMockServer::new().await;
    let dir = folder("no-recovery");
    let store = dir.join("store");

    server.mock_versions().ok().mount().await;
    Mock::given(method("GET"))
        .and(path("/_matrix/client/v3/login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "flows": [{ "type": "m.login.password" }],
        })))
        .mount(server.server())
        .await;
    server.mock_login().ok().mount().await;
    server.mock_upload_keys().ok().mount().await;
    server.mock_query_keys().ok().mount().await;
    server.mock_upload_cross_signing_keys().ok().mount().await;
    server.mock_upload_cross_signing_signatures().ok().mount().await;
    Mock::given(method("GET"))
        .and(path_regex("/account_data/m.secret_storage.default_key$"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errcode": "M_NOT_FOUND", "error": "Account data not found",
        })))
        .mount(server.server())
        .await;

    let uri = server.uri();
    let logged_in = login(&store, "@fish:localhost", "not-a-real-password", Some(&uri), Some("not a key"))
        .await
        .expect("the login itself works");
    let notes = logged_in.notes.join(" | ");
    assert!(notes.contains("this account has no recovery set up"), "got {notes}");
    assert!(notes.contains("the fish works all the same"), "got {notes}");
    assert!(!notes.contains("not a key"), "a note never repeats what was typed");
    assert!(!kept(&store).is_empty(), "the device has its store");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Der Zugang eines Fischs an diesem Server. Kein echter.
fn access_at(uri: &str) -> Access {
    let secret = json!({
        "h": uri, "u": "@fish:localhost", "d": "DEVICEID", "t": "not-a-real-token",
        "p": "not-a-real-passphrase",
    });
    Access::parse(&secret.to_string()).unwrap()
}

/// Das Gerät des Fischs, wie ein Server es zeigt: mit frisch gewürfelten
/// Schlüsseln, und gezeichnet mit `signer`, wenn es einen gibt.
fn shown_device(signer: Option<&Ed25519SecretKey>) -> Value {
    let own = Ed25519SecretKey::new();
    let curve = Curve25519PublicKey::from(&Curve25519SecretKey::new());
    let mut shown = json!({
        "user_id": "@fish:localhost",
        "device_id": "DEVICEID",
        "algorithms": ["m.olm.v1.curve25519-aes-sha2", "m.megolm.v1.aes-sha2"],
        "keys": {
            "curve25519:DEVICEID": curve.to_base64(),
            "ed25519:DEVICEID": own.public_key().to_base64(),
        },
    });
    // Jedes Gerät zeichnet sich selbst; das allein weist es nicht aus.
    let canonical = CanonicalJsonValue::try_from(shown.clone()).unwrap().to_string();
    let mut signatures = serde_json::Map::new();
    signatures.insert(
        "ed25519:DEVICEID".to_owned(),
        own.sign(canonical.as_bytes()).to_base64().into(),
    );
    if let Some(signer) = signer {
        signatures.insert(
            format!("ed25519:{}", signer.public_key().to_base64()),
            signer.sign(canonical.as_bytes()).to_base64().into(),
        );
    }
    shown["signatures"] = json!({ "@fish:localhost": signatures });
    shown
}

/// Der Schlüssel, mit dem ein Konto seine Geräte zeichnet, wie ein Server
/// ihn nennt.
fn signing_key(key: &Ed25519SecretKey) -> Value {
    let public = key.public_key().to_base64();
    let id = format!("ed25519:{public}");
    json!({ "user_id": "@fish:localhost", "usage": ["self_signing"], "keys": { id: public } })
}

/// Der Server antwortet auf die Frage nach den Geräten mit `body`.
async fn answers(server: &MatrixMockServer, status: u16, body: Value) {
    Mock::given(method("POST"))
        .and(path("/_matrix/client/v3/keys/query"))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(server.server())
        .await;
}

/// Nemesus am 23.11.2026, nach der Anmeldung mit dem
/// Wiederherstellungsschlüssel: ein Haken als dauerhafte Anzeige, dass der
/// Fisch ausgewiesen ist. Dafür fragt das Programm den Server.
#[tokio::test(flavor = "multi_thread")]
async fn a_device_the_account_has_signed_counts_as_verified() {
    let server = MatrixMockServer::new().await;
    let account = Ed25519SecretKey::new();
    answers(
        &server,
        200,
        json!({
            "device_keys": { "@fish:localhost": { "DEVICEID": shown_device(Some(&account)) } },
            "self_signing_keys": { "@fish:localhost": signing_key(&account) },
        }),
    )
    .await;
    assert_eq!(device(&access_at(&server.uri())).await, Device::Verified);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_nobody_has_signed_counts_as_unverified() {
    let server = MatrixMockServer::new().await;
    let account = Ed25519SecretKey::new();
    answers(
        &server,
        200,
        json!({
            "device_keys": { "@fish:localhost": { "DEVICEID": shown_device(None) } },
            "self_signing_keys": { "@fish:localhost": signing_key(&account) },
        }),
    )
    .await;
    assert_eq!(device(&access_at(&server.uri())).await, Device::Unverified);
}

/// Eine Unterschrift, die nicht vom Schlüssel des Kontos stammt, zählt nicht:
/// der Haken hängt an der geprüften Unterschrift, nicht daran, dass eine da
/// ist.
#[tokio::test(flavor = "multi_thread")]
async fn a_signature_from_another_key_does_not_count() {
    let server = MatrixMockServer::new().await;
    let account = Ed25519SecretKey::new();
    let stranger = Ed25519SecretKey::new();
    // Gezeichnet vom Fremden, aber unter dem Namen des Kontoschlüssels.
    let mut shown = shown_device(Some(&stranger));
    let theirs = format!("ed25519:{}", stranger.public_key().to_base64());
    let forged = shown["signatures"]["@fish:localhost"]
        .as_object_mut()
        .unwrap()
        .remove(&theirs)
        .unwrap();
    let id = format!("ed25519:{}", account.public_key().to_base64());
    shown["signatures"]["@fish:localhost"][id] = forged;
    answers(
        &server,
        200,
        json!({
            "device_keys": { "@fish:localhost": { "DEVICEID": shown } },
            "self_signing_keys": { "@fish:localhost": signing_key(&account) },
        }),
    )
    .await;
    assert_eq!(device(&access_at(&server.uri())).await, Device::Unverified);
}

/// Aus dem ersten echten Lauf am 22.11.2026: der Server nahm den Zugang nicht
/// mehr an, und die Maske zeigte weiter „logged in". Die Frage an den Server
/// sagt es jetzt.
#[tokio::test(flavor = "multi_thread")]
async fn an_access_the_server_refuses_is_told_apart() {
    let server = MatrixMockServer::new().await;
    answers(
        &server,
        401,
        json!({ "errcode": "M_UNKNOWN_TOKEN", "error": "Invalid access token passed." }),
    )
    .await;
    assert_eq!(device(&access_at(&server.uri())).await, Device::Refused);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_does_not_answer_leaves_the_question_open() {
    // Eine Adresse, an der niemand zuhört: einen Augenblick belegt, um sie
    // zu kennen, und wieder freigegeben.
    //
    // Bis zum 24.11.2026 stand hier die Adresse eines nachgestellten Servers,
    // der schon wieder freigegeben war. Die kommen aus einem Vorrat: ein
    // Nachbartest bekam denselben Server und antwortete für ihn. Unter Linux
    // gemessen scheiterte der Test daran in zwei von zwei Läufen.
    let uri = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", listener.local_addr().unwrap())
    };
    match device(&access_at(&uri)).await {
        Device::Unknown(why) => {
            assert!(!why.contains("not-a-real-token"), "the reason never shows the access: {why}")
        }
        other => panic!("expected an open question, got {other:?}"),
    }
}
