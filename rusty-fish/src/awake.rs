//! Den Rechner wach halten, solange der Fisch läuft.
//!
//! Nemesus am 21.11.2026: „Kann man eigentlich den Energiemodus von Windows
//! abrufen und vielleicht sogar ausschalten oder wäre das zuviel des Guten?"
//!
//! Die Einstellungen von Windows fasst der Fisch nicht an: das wäre zu viel,
//! und sie blieben verstellt, wenn er abstürzt. Windows kennt dafür einen
//! eigenen, höflichen Weg, den auch Videoplayer und Kopierprogramme gehen: ein
//! Programm sagt, dass es gerade arbeitet, und solange legt sich der Rechner
//! nicht von selbst schlafen. Endet das Programm, egal wie, gilt das nicht
//! mehr. Verstellt wird nichts.
//!
//! Benannte Grenzen:
//! - Verhindert wird nur das Einschlafen von selbst, nach einer Zeit ohne
//!   Eingabe. Wer den Deckel eines Laptops schließt, den Rechner von Hand
//!   schlafen legt oder den Akku leer laufen lässt, hält den Fisch an.
//! - Der Bildschirm darf weiter ausgehen. Das stört den Fisch nicht.
//! - Nur unter Windows. Auf anderen Systemen tut dieser Aufruf nichts.

/// Bittet das System, nicht von selbst einzuschlafen, solange dieses Programm
/// läuft. Liefert, ob die Bitte angenommen wurde.
pub fn keep() -> bool {
    imp::keep()
}

#[cfg(windows)]
mod imp {
    /// Die Bitte gilt, bis sie zurückgenommen wird oder der Thread endet.
    const ES_CONTINUOUS: u32 = 0x8000_0000;
    /// Der Rechner wird gebraucht. Der Bildschirm nicht: der darf ausgehen.
    const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetThreadExecutionState(flags: u32) -> u32;
    }

    pub fn keep() -> bool {
        // Die Bitte hängt an dem Thread, der sie ausspricht. Deshalb ein
        // eigener, der nichts anderes tut und bis zum Ende des Programms lebt.
        let (said, heard) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new().name("keep-awake".to_owned()).spawn(move || {
            // SAFETY: der Aufruf nimmt nur eine Zahl aus festen Schaltern und
            // gibt den vorigen Zustand zurück; null heißt abgelehnt.
            let before = unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) };
            let _ = said.send(before != 0);
            loop {
                std::thread::park();
            }
        });
        spawned.is_ok() && heard.recv().unwrap_or(false)
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn keep() -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(windows)]
    fn windows_accepts_the_request_to_stay_awake() {
        // Gilt nur, solange dieser Testlauf lebt, und verstellt nichts.
        assert!(super::keep());
        // Ein zweites Mal schadet nicht.
        assert!(super::keep());
    }

    #[test]
    #[cfg(not(windows))]
    fn elsewhere_the_call_does_nothing_and_says_so() {
        assert!(!super::keep());
    }
}
