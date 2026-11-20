//! Schlüsselablage. Die eine Stelle, die je Betriebssystem anders ist, deshalb
//! gekapselt: Windows-Anmeldeinformationsverwaltung, macOS-Schlüsselbund,
//! Secret Service unter Linux.
//!
//! Vorbild ist Pacioli: kein echter Schlüssel im Klartext in einer Datei oder
//! Datenbank des Dienstes, und der Dienst sieht ihn nur im Moment des Aufrufs.

use keyring::v1::{Entry, Error};
use rusty_fish_core::Secret;

const SERVICE: &str = "rusty-fish";

fn entry(provider: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, provider).map_err(|e| format!("cannot open the credential store: {e}"))
}

pub fn store(provider: &str, secret: &str) -> Result<(), String> {
    entry(provider)?
        .set_password(secret)
        .map_err(|e| format!("cannot store the key: {e}"))
}

pub fn load(provider: &str) -> Result<Option<Secret>, String> {
    match entry(provider)?.get_password() {
        Ok(value) => Ok(Some(Secret::new(value))),
        Err(Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("cannot read the key: {e}")),
    }
}

/// Liefert `false`, wenn es nichts zu löschen gab.
pub fn forget(provider: &str) -> Result<bool, String> {
    match entry(provider)?.delete_credential() {
        Ok(()) => Ok(true),
        Err(Error::NoEntry) => Ok(false),
        Err(e) => Err(format!("cannot delete the key: {e}")),
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
}
