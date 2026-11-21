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
    /// Die Namen der Profile, die dieser Fisch gerade bedient. Leer, wenn er
    /// mit `--config` gestartet wurde. Seit dem 21.11.2026 können es mehrere
    /// sein; der Fisch schreibt die Liste neu, wenn eines dazukommt oder geht.
    pub profiles: Vec<String>,
    pub pid: u32,
    /// Startzeit in Sekunden seit dem 1.1.1970.
    pub started: u64,
}

/// Die gehaltene Sperre. Sie endet, wenn dieser Wert endet.
#[derive(Debug)]
pub struct Held {
    _file: File,
    info: PathBuf,
    pid: u32,
    started: u64,
}

impl Held {
    /// Schreibt neu, welche Profile dieser Fisch bedient. Die Sperre selbst
    /// bleibt, wie sie ist.
    pub fn set_profiles(&self, profiles: &[String]) -> Result<(), String> {
        let who = Running { profiles: profiles.to_vec(), pid: self.pid, started: self.started };
        write_info(&self.info, &who)
    }
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
                        Some(Running { profiles, pid, .. }) if profiles.is_empty() => {
                            format!(" (process {pid})")
                        }
                        Some(Running { profiles, pid, .. }) => {
                            let names: Vec<String> =
                                profiles.iter().map(|name| format!("\"{name}\"")).collect();
                            let word = if names.len() == 1 { "profile" } else { "profiles" };
                            format!(" ({word} {}, process {pid})", names.join(", "))
                        }
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

    write_info(&info_path, who)?;
    Ok(Held { _file: file, info: info_path, pid: who.pid, started: who.started })
}

/// Schreibt die Notiz so, dass niemand sie halb liest: erst daneben, dann
/// umbenennen. Die Maske liest sie, während der Fisch sie neu schreibt.
fn write_info(info_path: &Path, who: &Running) -> Result<(), String> {
    let note = json!({ "profiles": who.profiles, "pid": who.pid, "started": who.started });
    let mut tmp = info_path.to_path_buf().into_os_string();
    tmp.push(".tmp");
    std::fs::write(&tmp, note.to_string())
        .and_then(|()| std::fs::rename(&tmp, info_path))
        .map_err(|e| format!("cannot write {}: {e}", info_path.display()))
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
            let unknown = Running { profiles: Vec::new(), pid: 0, started: 0 };
            Some(read_info(&info_path).unwrap_or(unknown))
        }
        Err(TryLockError::Error(_)) => None,
    }
}

fn read_info(path: &Path) -> Option<Running> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(Running {
        profiles: match v["profiles"].as_array() {
            Some(names) => names.iter().filter_map(|n| n.as_str().map(str::to_owned)).collect(),
            // So schrieb es ein Fisch von vor dem 21.11.2026: ein Name oder keiner.
            None => v["profile"].as_str().map(str::to_owned).into_iter().collect(),
        },
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
        Running {
            profiles: profile.map(str::to_owned).into_iter().collect(),
            pid: 4242,
            started: 1_800_000_000,
        }
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

    #[test]
    fn a_fish_says_which_profiles_it_serves_as_they_come_and_go() {
        let dir = temp("several");
        let held = acquire(&dir, "discord", &who(Some("tea"))).unwrap();
        let names = |list: &[&str]| list.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();

        held.set_profiles(&names(&["tea", "probe"])).unwrap();
        let now = running(&dir, "discord").unwrap();
        assert_eq!(now.profiles, ["tea", "probe"]);
        // Wer der Fisch ist und seit wann er läuft, ändert sich dabei nicht.
        assert_eq!((now.pid, now.started), (4242, 1_800_000_000));

        // Ein zweiter Fisch wird abgelehnt und erfährt, wer alles läuft.
        let err = acquire(&dir, "discord", &who(Some("other"))).unwrap_err();
        assert!(err.contains("profiles \"tea\", \"probe\""), "got {err}");

        held.set_profiles(&names(&["probe"])).unwrap();
        assert_eq!(running(&dir, "discord").unwrap().profiles, ["probe"]);
        drop(held);
        assert_eq!(running(&dir, "discord"), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_note_from_before_several_profiles_is_still_understood() {
        let dir = temp("oldnote");
        let held = acquire(&dir, "discord", &who(Some("tea"))).unwrap();
        // So schrieb es ein Fisch von vor dem 21.11.2026.
        let note = dir.join(".locks").join("discord.running.json");
        std::fs::write(&note, r#"{"profile": "tea", "pid": 7, "started": 9}"#).unwrap();
        assert_eq!(running(&dir, "discord").unwrap().profiles, ["tea"]);
        std::fs::write(&note, r#"{"profile": null, "pid": 7, "started": 9}"#).unwrap();
        assert!(running(&dir, "discord").unwrap().profiles.is_empty());
        drop(held);
        let _ = std::fs::remove_dir_all(dir);
    }
}
