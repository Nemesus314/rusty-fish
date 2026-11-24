//! Schlüsselablage. Die eine Stelle, die je Betriebssystem anders ist, deshalb
//! gekapselt: Windows-Anmeldeinformationsverwaltung, macOS-Schlüsselbund,
//! Secret Service unter Linux.
//!
//! Vorbild ist Pacioli: kein echter Schlüssel im Klartext in einer Datei oder
//! Datenbank des Dienstes, und der Dienst sieht ihn nur im Moment des Aufrufs.
//!
//! Seit dem 24.11.2026 gibt es davon eine benannte Ausnahme, entschieden von
//! Nemesus („Ja, Weg A"): ein Linux-Server ohne Desktop hat keinen Secret
//! Service (gemessen: „No default store has been set", und kein Fisch
//! startet). Dort kann man den Fisch seine Geheimnisse in Dateien in seinem
//! eigenen Ordner halten lassen, nur für sein Konto lesbar, im Klartext. Das
//! geschieht nie von selbst, nur auf `rusty-fish key store file`, und nur
//! unter Unix; unter Windows bleibt alles, wie es war.
//!
//! Benannte Grenzen der Dateien:
//! - Geschützt sind sie allein durch die Rechte der Dateien. Wer auf dem
//!   Server root ist, wer ein Backup des Ordners hat oder die Platte, liest
//!   sie. Verschlüsselt ist nichts.
//! - Ein Server, der ohne einen Menschen neu starten soll, muss seine
//!   Geheimnisse selbst aufschließen können; das gälte auf jedem anderen Weg
//!   ebenso.

use keyring::v1::{Entry, Error};
use rusty_fish_core::Secret;
use std::path::{Path, PathBuf};

const SERVICE: &str = "rusty-fish";

/// Der Ordner im Ordner des Fischs, in dem die Dateien liegen. Dass es ihn
/// gibt, ist zugleich der Vermerk, dass die Dateien gelten: so gilt es für
/// das Terminal und für den Dienst gleich, anders als eine Umgebungsvariable,
/// die an einer der beiden Stellen fehlen kann. Den Namen des Ordners und den
/// des Befehls (`key store file`) hat Nemesus am 24.11.2026 bestätigt („Ja, so
/// lassen").
pub const FOLDER: &str = "secrets";

/// Wo die Geheimnisse liegen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// In der Ablage des Systems.
    System,
    /// In Dateien in diesem Ordner.
    #[cfg(unix)]
    Files(PathBuf),
}

/// Wo die Geheimnisse dieses Fischs liegen.
pub fn place() -> Place {
    // Lässt sich der Ordner des Fischs nicht bestimmen, gibt es dort auch
    // keinen Vermerk.
    match crate::profiles::home(&|key| std::env::var(key).ok(), crate::profiles::this_os()) {
        Ok(home) => place_in(&home),
        Err(_) => Place::System,
    }
}

/// Wie [`place`], für einen gegebenen Ordner des Fischs.
#[cfg(unix)]
pub fn place_in(home: &Path) -> Place {
    let dir = home.join(FOLDER);
    if dir.is_dir() { Place::Files(dir) } else { Place::System }
}

/// Außerhalb von Unix gibt es nur die Ablage des Systems, was auch immer im
/// Ordner des Fischs liegt.
#[cfg(not(unix))]
pub fn place_in(_home: &Path) -> Place {
    Place::System
}

/// Der Ort eines Geheimnisses in Meldungen: „… stored in {}."
pub fn described(place: &Place) -> String {
    match place {
        Place::System => "this system's credential store".to_owned(),
        #[cfg(unix)]
        Place::Files(dir) => format!("a file in {}", dir.display()),
    }
}

/// Von nun an liegen die Geheimnisse in Dateien im Ordner des Fischs. Liefert
/// den Ordner. Was in der Ablage des Systems liegt, bleibt dort und wird
/// nicht mehr gesehen.
#[cfg(unix)]
pub fn keep_in_files(home: &Path) -> Result<PathBuf, String> {
    files::open(&home.join(FOLDER))
}

#[cfg(not(unix))]
pub fn keep_in_files(_home: &Path) -> Result<PathBuf, String> {
    Err("secrets in files are for Linux servers; on this system they stay in the credential store"
        .to_owned())
}

/// Von nun an liegen die Geheimnisse wieder in der Ablage des Systems. Solange
/// noch eine Datei da ist, geschieht nichts: ein Geheimnis verschwindet nur
/// mit `key forget` oder beim Abmelden von Matrix, nie nebenbei.
#[cfg(unix)]
pub fn keep_in_system(home: &Path) -> Result<(), String> {
    files::close(&home.join(FOLDER))
}

#[cfg(not(unix))]
pub fn keep_in_system(_home: &Path) -> Result<(), String> {
    Ok(())
}

pub fn store(provider: &str, secret: &str) -> Result<(), String> {
    match place() {
        Place::System => entry(provider)?
            .set_password(secret)
            .map_err(|e| format!("cannot store the key: {e}")),
        #[cfg(unix)]
        Place::Files(dir) => files::store(&dir, provider, secret),
    }
}

pub fn load(provider: &str) -> Result<Option<Secret>, String> {
    match place() {
        Place::System => match entry(provider)?.get_password() {
            Ok(value) => Ok(Some(Secret::new(value))),
            Err(Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("cannot read the key: {e}")),
        },
        #[cfg(unix)]
        Place::Files(dir) => files::load(&dir, provider),
    }
}

/// Liefert `false`, wenn es nichts zu löschen gab.
pub fn forget(provider: &str) -> Result<bool, String> {
    match place() {
        Place::System => match entry(provider)?.delete_credential() {
            Ok(()) => Ok(true),
            Err(Error::NoEntry) => Ok(false),
            Err(e) => Err(format!("cannot delete the key: {e}")),
        },
        #[cfg(unix)]
        Place::Files(dir) => files::forget(&dir, provider),
    }
}

fn entry(provider: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, provider)
        .map_err(|e| format!("cannot open the credential store: {e}{}", way_out()))
}

/// Was man tun kann, wenn es keine Ablage des Systems gibt. So scheitert ein
/// Linux-Server ohne Desktop, und die Meldung der Ablage allein sagt nicht,
/// wie es weitergeht.
fn way_out() -> &'static str {
    if cfg!(all(unix, not(target_os = "macos"))) {
        ". On a server without a desktop there is none; `rusty-fish key store file` keeps the \
         secrets in files instead"
    } else {
        ""
    }
}

/// Die Dateien: je Eintrag eine, benannt wie der Eintrag.
#[cfg(unix)]
mod files {
    use rusty_fish_core::Secret;
    use std::fs::{DirBuilder, OpenOptions, Permissions};
    use std::io::{ErrorKind, Write};
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    use std::path::{Path, PathBuf};

    /// Nur das Konto des Fischs darf in den Ordner sehen.
    const FOLDER_MODE: u32 = 0o700;
    /// Nur das Konto des Fischs darf die Datei lesen und schreiben.
    const FILE_MODE: u32 = 0o600;

    /// Legt den Ordner an, nur für dieses Konto. Gibt es ihn schon, werden
    /// seine Rechte so gesetzt.
    pub fn open(dir: &Path) -> Result<PathBuf, String> {
        let cannot = |e: std::io::Error| format!("cannot create {}: {e}", dir.display());
        DirBuilder::new().recursive(true).mode(FOLDER_MODE).create(dir).map_err(cannot)?;
        std::fs::set_permissions(dir, Permissions::from_mode(FOLDER_MODE)).map_err(cannot)?;
        Ok(dir.to_path_buf())
    }

    /// Nimmt den Ordner fort, wenn kein Geheimnis mehr darin liegt.
    pub fn close(dir: &Path) -> Result<(), String> {
        if !dir.is_dir() {
            return Ok(());
        }
        let cannot = |e: std::io::Error| format!("cannot read {}: {e}", dir.display());
        let mut left = Vec::new();
        for entry in std::fs::read_dir(dir).map_err(cannot)? {
            let name = entry.map_err(cannot)?.file_name().to_string_lossy().into_owned();
            // Ein liegengebliebener Entwurf ist kein Geheimnis, das jemand vermisst.
            if is_draft(&name) {
                let _ = std::fs::remove_file(dir.join(&name));
            } else {
                left.push(name);
            }
        }
        if !left.is_empty() {
            left.sort();
            return Err(format!(
                "there are still secrets in {}: {}. Remove each with `rusty-fish key forget`, \
                 and the Matrix account with `rusty-fish matrix logout`, first",
                dir.display(),
                left.join(", ")
            ));
        }
        std::fs::remove_dir(dir).map_err(|e| format!("cannot remove {}: {e}", dir.display()))
    }

    pub fn store(dir: &Path, entry: &str, secret: &str) -> Result<(), String> {
        let path = path(dir, entry)?;
        let draft = dir.join(format!(".{entry}.tmp"));
        let cannot = |e: std::io::Error| format!("cannot store the key in {}: {e}", path.display());
        // Erst daneben, dann umbenennen: niemand liest ein halbes Geheimnis,
        // und die Datei ist vom ersten Augenblick an nur für dieses Konto da.
        let _ = std::fs::remove_file(&draft);
        let written = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(FILE_MODE)
            .open(&draft)
            .and_then(|mut file| {
                file.write_all(secret.as_bytes())?;
                file.sync_all()
            })
            .and_then(|()| std::fs::rename(&draft, &path));
        if written.is_err() {
            let _ = std::fs::remove_file(&draft);
        }
        written.map_err(cannot)
    }

    pub fn load(dir: &Path, entry: &str) -> Result<Option<Secret>, String> {
        let path = path(dir, entry)?;
        let cannot = |e: std::io::Error| format!("cannot read the key from {}: {e}", path.display());
        let mode = match std::fs::metadata(&path) {
            Ok(found) => found.permissions().mode(),
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(cannot(e)),
        };
        // Wie ssh es mit seinen Schlüsseln hält: eine Datei, in die andere
        // sehen können, wird nicht benutzt, und die Meldung sagt, was zu tun ist.
        if mode & 0o077 != 0 {
            return Err(format!(
                "{} can be read by other accounts of this system; run `chmod 600 {}` first",
                path.display(),
                path.display()
            ));
        }
        let text = std::fs::read_to_string(&path).map_err(cannot)?;
        // Wer die Datei von Hand geschrieben hat, hat meist einen Zeilenumbruch
        // am Ende; der gehört nicht zum Geheimnis.
        let secret = text.trim_end_matches(['\r', '\n']);
        Ok((!secret.is_empty()).then(|| Secret::new(secret.to_owned())))
    }

    pub fn forget(dir: &Path, entry: &str) -> Result<bool, String> {
        let path = path(dir, entry)?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
            Err(e) => Err(format!("cannot delete {}: {e}", path.display())),
        }
    }

    /// Die Datei eines Eintrags. Die Namen der Einträge stehen fest und sind
    /// schlichte Wörter; alles andere wird abgelehnt, damit nie ein Name aus
    /// dem Ordner hinausführt.
    fn path(dir: &Path, entry: &str) -> Result<PathBuf, String> {
        let plain = !entry.is_empty()
            && entry.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !plain {
            return Err(format!("\"{entry}\" is not a name a secret can be kept under"));
        }
        Ok(dir.join(entry))
    }

    fn is_draft(name: &str) -> bool {
        name.starts_with('.') && name.ends_with(".tmp")
    }
}

/// Diese beiden Tests fassen die echte Ablage des Systems an und laufen nur
/// auf Wunsch, nacheinander in zwei getrennten Prozessen. Erst so zeigt sich,
/// dass der Eintrag den Prozess überlebt und nicht nur im Speicher lag.
///
/// cargo test -p rusty-fish -- --ignored selftest_1
/// cargo test -p rusty-fish -- --ignored selftest_2
#[cfg(test)]
mod tests {
    use super::*;

    const PROBE: &str = "selftest";

    #[test]
    #[ignore = "touches the real credential store"]
    fn selftest_1_store() {
        store(PROBE, "not-a-real-key").unwrap();
        assert!(load(PROBE).unwrap().is_some());
    }

    #[test]
    #[ignore = "touches the real credential store"]
    fn selftest_2_find_and_forget() {
        assert!(load(PROBE).unwrap().is_some(), "the entry did not survive the process");
        assert!(forget(PROBE).unwrap());
        assert!(load(PROBE).unwrap().is_none());
        assert!(!forget(PROBE).unwrap());
    }

    #[test]
    fn a_folder_that_was_never_asked_for_keeps_the_secrets_in_the_system() {
        let home = std::env::temp_dir().join(format!("rusty-fish-keys-{}-none", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        assert_eq!(place_in(&home), Place::System);
        assert_eq!(described(&Place::System), "this system's credential store");
        let _ = std::fs::remove_dir_all(home);
    }

    /// Unter Windows bleibt alles, wie es war: dort gibt es die Dateien nicht,
    /// auch wenn jemand den Ordner von einem Server herüberkopiert hat.
    #[test]
    #[cfg(not(unix))]
    fn outside_unix_there_are_no_secrets_in_files() {
        let home = std::env::temp_dir().join(format!("rusty-fish-keys-{}-copied", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join(FOLDER)).unwrap();
        assert_eq!(place_in(&home), Place::System);
        assert!(keep_in_files(&home).is_err());
        let _ = std::fs::remove_dir_all(home);
    }
}

/// Die Dateien, in leeren Wegwerf-Ordnern. Nur unter Unix: dort gibt es sie.
#[cfg(all(test, unix))]
mod file_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn temp(name: &str) -> PathBuf {
        let home = std::env::temp_dir().join(format!("rusty-fish-keys-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        home
    }

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn files_are_used_only_after_they_were_asked_for() {
        let home = temp("asked");
        assert_eq!(place_in(&home), Place::System);
        let dir = keep_in_files(&home).unwrap();
        assert_eq!(dir, home.join(FOLDER));
        assert_eq!(place_in(&home), Place::Files(dir.clone()));
        // Ein zweites Mal schadet nicht.
        assert_eq!(keep_in_files(&home).unwrap(), dir);
        keep_in_system(&home).unwrap();
        assert_eq!(place_in(&home), Place::System);
        assert!(!dir.exists());
        // Zurück geht auch, wenn nie etwas da war.
        keep_in_system(&home).unwrap();
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn a_secret_kept_in_a_file_is_found_again_and_can_be_forgotten() {
        let home = temp("round");
        let dir = keep_in_files(&home).unwrap();
        assert!(files::load(&dir, "custom").unwrap().is_none());
        files::store(&dir, "custom", "not-a-real-key").unwrap();
        assert_eq!(files::load(&dir, "custom").unwrap().unwrap().expose(), "not-a-real-key");
        // Ein neues ersetzt das alte, und kein Entwurf bleibt liegen.
        files::store(&dir, "custom", "another-word").unwrap();
        assert_eq!(files::load(&dir, "custom").unwrap().unwrap().expose(), "another-word");
        let kept: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(kept, ["custom"]);
        assert!(files::forget(&dir, "custom").unwrap());
        assert!(files::load(&dir, "custom").unwrap().is_none());
        assert!(!files::forget(&dir, "custom").unwrap());
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn only_this_account_can_read_the_folder_and_the_files() {
        let home = temp("modes");
        let dir = keep_in_files(&home).unwrap();
        files::store(&dir, "discord", "not-a-real-token").unwrap();
        assert_eq!(mode(&dir), 0o700);
        assert_eq!(mode(&dir.join("discord")), 0o600);
        // Auch ein Ordner, den es schon gab und in den andere sehen konnten.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        keep_in_files(&home).unwrap();
        assert_eq!(mode(&dir), 0o700);
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn a_file_others_can_read_is_refused_and_the_message_says_what_to_do() {
        let home = temp("open");
        let dir = keep_in_files(&home).unwrap();
        files::store(&dir, "openai", "not-a-real-key").unwrap();
        let file = dir.join("openai");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        let why = files::load(&dir, "openai").unwrap_err();
        assert!(why.contains("can be read by other accounts"), "got {why}");
        assert!(why.contains("chmod 600"), "got {why}");
        assert!(!why.contains("not-a-real-key"), "the reason never shows the secret: {why}");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(files::load(&dir, "openai").unwrap().is_some());
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn a_file_written_by_hand_may_end_with_a_line_break() {
        let home = temp("hand");
        let dir = keep_in_files(&home).unwrap();
        let file = dir.join("xai");
        std::fs::write(&file, "not-a-real-key\r\n").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(files::load(&dir, "xai").unwrap().unwrap().expose(), "not-a-real-key");
        // Eine leere Datei ist kein Geheimnis.
        std::fs::write(&file, "\n").unwrap();
        assert!(files::load(&dir, "xai").unwrap().is_none());
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn no_name_leads_out_of_the_folder() {
        let home = temp("names");
        let dir = keep_in_files(&home).unwrap();
        for name in ["", "..", "../outside", "a/b", ".hidden", "with space"] {
            assert!(files::store(&dir, name, "x").is_err(), "{name:?} was accepted");
            assert!(files::load(&dir, name).is_err(), "{name:?} was accepted");
            assert!(files::forget(&dir, name).is_err(), "{name:?} was accepted");
        }
        assert!(!home.join("outside").exists());
        // Jeder Eintrag, den der Fisch kennt, ist ein solcher Name.
        for entry in crate::secrets::entries().into_iter().chain([crate::secrets::MATRIX]) {
            files::store(&dir, entry, "x").unwrap();
        }
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn the_way_back_never_takes_a_secret_along() {
        let home = temp("back");
        let dir = keep_in_files(&home).unwrap();
        files::store(&dir, "discord", "not-a-real-token").unwrap();
        files::store(&dir, "matrix", "not-a-real-access").unwrap();
        let why = keep_in_system(&home).unwrap_err();
        assert!(why.contains("discord, matrix"), "got {why}");
        assert!(why.contains("rusty-fish key forget"), "got {why}");
        assert_eq!(place_in(&home), Place::Files(dir.clone()));
        assert!(files::load(&dir, "discord").unwrap().is_some());
        files::forget(&dir, "discord").unwrap();
        files::forget(&dir, "matrix").unwrap();
        // Ein liegengebliebener Entwurf hält den Weg zurück nicht auf.
        std::fs::write(dir.join(".discord.tmp"), "x").unwrap();
        keep_in_system(&home).unwrap();
        assert_eq!(place_in(&home), Place::System);
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn the_place_is_named_in_messages() {
        let said = described(&Place::Files(PathBuf::from("/srv/fish/secrets")));
        assert_eq!(said, "a file in /srv/fish/secrets");
    }
}
