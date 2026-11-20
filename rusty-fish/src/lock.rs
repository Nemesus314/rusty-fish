//! Ein Bot, ein Fisch.
//!
//! Discord meldet jede Nachricht an jede Verbindung, die sich mit demselben
//! Bot-Token angemeldet hat. Liefen zwei Fische mit demselben Bot, würde jeder
//! von ihnen jede Nachricht übersetzen und zustellen: alles stünde doppelt da,
//! und bezahlt würde auch doppelt. Deshalb hält ein laufender Fisch eine
//! exklusive Sperre auf eine Datei, die nach dem Bot-Eintrag in der Ablage
//! benannt ist, und ein zweiter Start scheitert laut, bevor er sich bei Discord
//! zeigt.
//!
//! Die Sperre gehört dem Betriebssystem. Stürzt der Fisch ab, ist sie frei,
//! ohne dass jemand aufräumen muss. Welches Profil läuft, steht in einer Datei
//! daneben; ihr wird nur geglaubt, solange die Sperre wirklich belegt ist.
//!
//! Benannte Grenze: solange es nur einen Bot-Eintrag gibt (`discord`), läuft
//! höchstens ein Profil zugleich. Mehrere Profile nebeneinander brauchen
//! entweder je einen eigenen Bot oder einen Fisch, der mehrere Profile bedient.

use serde_json::json;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Der Satz, an dem die Oberfläche erkennt, dass schon ein Fisch läuft.
pub const ALREADY_RUNNING: &str = "another Rusty Fish is already running with this bot";

/// Wer die Sperre gerade hält.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Running {
    /// Name des Profils, oder `None`, wenn mit `--config` gestartet wurde.
    pub profile: Option<String>,
    pub pid: u32,
    /// Startzeit in Sekunden seit dem 1.1.1970.
    pub started: u64,
}

/// Die gehaltene Sperre. Sie endet, wenn dieser Wert endet.
#[derive(Debug)]
pub struct Held {
    _file: File,
    info: PathBuf,
}

impl Drop for Held {
    fn drop(&mut self) {
        // Nur Ordnung: geglaubt wird der Datei ohnehin nur bei belegter Sperre.
        let _ = std::fs::remove_file(&self.info);
    }
}

fn paths(profiles: &Path, bot: &str) -> (PathBuf, PathBuf) {
    let dir = profiles.join(".locks");
    (dir.join(format!("{bot}.lock")), dir.join(format!("{bot}.running.json")))
}

/// Nimmt die Sperre für diesen Bot oder scheitert mit [`ALREADY_RUNNING`].
pub fn acquire(profiles: &Path, bot: &str, who: &Running) -> Result<Held, String> {
    let (lock_path, info_path) = paths(profiles, bot);
    if let Some(dir) = lock_path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| format!("cannot open {}: {e}", lock_path.display()))?;

    // Ein paar kurze Versuche: wer gerade nur nachsieht, ob ein Fisch läuft
    // (siehe `running`), hält die Sperre für einen Augenblick selbst.
    let mut tries = 0;
    loop {
        match file.try_lock() {
            Ok(()) => break,
            Err(TryLockError::WouldBlock) => {
                tries += 1;
                if tries >= 5 {
                    let by = match read_info(&info_path) {
                        Some(Running { profile: Some(name), pid, .. }) => {
                            format!(" (profile \"{name}\", process {pid})")
                        }
                        Some(Running { profile: None, pid, .. }) => format!(" (process {pid})"),
                        None => String::new(),
                    };
                    return Err(format!("{ALREADY_RUNNING}{by}"));
                }
                std::thread::sleep(Duration::from_millis(60));
            }
            Err(TryLockError::Error(e)) => {
                return Err(format!("cannot lock {}: {e}", lock_path.display()));
            }
        }
    }

    let note = json!({ "profile": who.profile, "pid": who.pid, "started": who.started });
    std::fs::write(&info_path, note.to_string())
        .map_err(|e| format!("cannot write {}: {e}", info_path.display()))?;
    Ok(Held { _file: file, info: info_path })
}

/// Wer mit diesem Bot gerade läuft, falls jemand läuft.
pub fn running(profiles: &Path, bot: &str) -> Option<Running> {
    let (lock_path, info_path) = paths(profiles, bot);
    // Gibt es die Datei nicht, lief hier noch nie ein Fisch.
    let file = OpenOptions::new().read(true).write(true).open(&lock_path).ok()?;
    match file.try_lock() {
        // Frei: niemand läuft, egal was in der Notiz steht.
        Ok(()) => {
            let _ = file.unlock();
            None
        }
        // Belegt: jetzt, und nur jetzt, wird der Notiz geglaubt.
        Err(TryLockError::WouldBlock) => {
            Some(read_info(&info_path).unwrap_or(Running { profile: None, pid: 0, started: 0 }))
        }
        Err(TryLockError::Error(_)) => None,
    }
}

fn read_info(path: &Path) -> Option<Running> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(Running {
        profile: v["profile"].as_str().map(str::to_owned),
        pid: u32::try_from(v["pid"].as_u64()?).ok()?,
        started: v["started"].as_u64()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rusty-fish-lock-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn who(profile: Option<&str>) -> Running {
        Running { profile: profile.map(str::to_owned), pid: 4242, started: 1_800_000_000 }
    }

    #[test]
    fn nobody_runs_in_a_fresh_folder() {
        let dir = temp("fresh");
        assert_eq!(running(&dir, "discord"), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_second_fish_with_the_same_bot_is_refused() {
        let dir = temp("second");
        let first = acquire(&dir, "discord", &who(Some("tea"))).unwrap();
        let err = acquire(&dir, "discord", &who(Some("other"))).unwrap_err();
        assert!(err.contains(ALREADY_RUNNING), "got {err}");
        // Der Erste läuft weiter und ist weiter der, der genannt wird.
        assert_eq!(running(&dir, "discord"), Some(who(Some("tea"))));
        drop(first);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn after_the_first_has_ended_the_next_may_start() {
        let dir = temp("next");
        let first = acquire(&dir, "discord", &who(Some("tea"))).unwrap();
        drop(first);
        assert_eq!(running(&dir, "discord"), None);
        let second = acquire(&dir, "discord", &who(None)).unwrap();
        assert_eq!(running(&dir, "discord"), Some(who(None)));
        drop(second);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn looking_does_not_take_the_lock_away() {
        let dir = temp("look");
        for _ in 0..3 {
            assert_eq!(running(&dir, "discord"), None);
        }
        let held = acquire(&dir, "discord", &who(Some("tea"))).unwrap();
        for _ in 0..3 {
            assert!(running(&dir, "discord").is_some());
        }
        drop(held);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_stale_note_without_a_lock_is_not_believed() {
        let dir = temp("stale");
        // So sieht es nach einem Absturz aus: die Notiz liegt noch da, die
        // Sperre hält niemand mehr.
        let held = acquire(&dir, "discord", &who(Some("tea"))).unwrap();
        let note = std::fs::read(dir.join(".locks").join("discord.running.json")).unwrap();
        drop(held);
        std::fs::write(dir.join(".locks").join("discord.running.json"), note).unwrap();
        assert_eq!(running(&dir, "discord"), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn another_bot_is_another_lock() {
        let dir = temp("otherbot");
        let a = acquire(&dir, "discord", &who(Some("tea"))).unwrap();
        let b = acquire(&dir, "discord-2", &who(Some("coffee"))).unwrap();
        assert_eq!(running(&dir, "discord-2"), Some(who(Some("coffee"))));
        drop((a, b));
        let _ = std::fs::remove_dir_all(dir);
    }
}
