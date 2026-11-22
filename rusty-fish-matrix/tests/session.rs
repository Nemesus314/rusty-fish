//! Die Sitzung des Fischs gegen einen nachgestellten Matrix-Server: kein
//! Netz, kein Konto.

use matrix_sdk::test_utils::mocks::MatrixMockServer;
use rusty_fish_matrix::session::{Access, connect, login, logout};
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
