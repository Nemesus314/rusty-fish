//! Die Sitzung des Fischs gegen einen nachgestellten Matrix-Server: kein
//! Netz, kein Konto.

use matrix_sdk::test_utils::mocks::MatrixMockServer;
use rusty_fish_matrix::session::{Access, connect, logout};
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
