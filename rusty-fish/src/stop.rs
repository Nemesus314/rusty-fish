//! Was dem laufenden Fisch von außen gesagt wird: anhalten, ein weiteres
//! Profil bedienen, ein Profil nicht mehr bedienen.
//!
//! Bis zum 21.11.2026 stand das Anhalten in der Discord-Tür. Es gehört dem
//! Programm: ein Fisch mit zwei Türen wird einmal angehalten, nicht einmal je
//! Tür. Seit demselben Tag bedient ein Fisch mehrere Profile; die Maske sagt
//! ihm über dieselbe Leitung, welche.

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

/// Eine Zeile der Standardeingabe, verstanden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// `stop`: den ganzen Fisch anhalten.
    StopAll,
    /// `start NAME`: ein weiteres Profil bedienen.
    Start(String),
    /// `stop NAME`: dieses Profil nicht mehr bedienen.
    Stop(String),
    /// `restart NAME`: dieses Profil neu laden, etwa nachdem es geändert
    /// wurde. Anders als `stop` und `start` hintereinander hält das den Fisch
    /// auch dann nicht an, wenn es sein einziges Profil ist.
    Restart(String),
}

/// Hört zu, was dem Fisch gesagt wird: Zeilen auf der Standardeingabe (so
/// spricht die Maske mit ihm) und das Signal des Systems zum Anhalten, das
/// wie `stop` gilt: Strg+C im Terminal, unter Linux auch SIGTERM.
///
/// Das bloße Ende der Standardeingabe hält ihn nicht an, und der Kanal bleibt
/// offen. Als Dienst ohne Terminal hat der Fisch gar keine Eingabe, und er
/// soll trotzdem laufen.
pub fn listen() -> UnboundedReceiver<Input> {
    let (said, heard) = unbounded_channel();
    // Ein eigener Thread, weil das Lesen der Eingabe blockiert. Er darf beim
    // Beenden des Programms einfach liegen bleiben.
    let lines = said.clone();
    std::thread::spawn(move || {
        for line in std::io::stdin().lines().map_while(Result::ok) {
            if let Some(input) = parse(&line)
                && lines.send(input).is_err()
            {
                break;
            }
        }
    });
    on_signal(said);
    heard
}

/// Sagt `stop`, sobald das System den Fisch anhalten will.
fn on_signal(said: UnboundedSender<Input>) {
    // Hier schon angemeldet, nicht erst in der Aufgabe unten: ein Signal, das
    // gleich nach dem Start kommt, soll den Fisch nicht noch hart beenden.
    let asked = asked_to_stop();
    tokio::spawn(async move {
        if asked.await {
            let _ = said.send(Input::StopAll);
        }
        // Dieser Sender bleibt bis zum Ende des Programms am Leben. Endet die
        // Eingabe, oder lässt sich das Signal nicht abfangen, sieht der
        // Empfänger deshalb kein Ende des Kanals, das wie ein Signal aussähe.
        std::future::pending::<()>().await;
        drop(said);
    });
}

/// Wartet auf das Signal zum Anhalten. Liefert `false`, wenn sich keines
/// abfangen lässt.
///
/// Unter Unix sind es zwei: SIGINT (Strg+C im Terminal) und SIGTERM. Mit
/// SIGTERM hält ein Dienstverwalter wie systemd einen Dienst an, und auch ein
/// schlichtes `kill`. Bis zum 24.11.2026 fing der Fisch nur Strg+C: SIGTERM
/// beendete ihn auf der Stelle, ohne die Zeile zum Ausschalten an den Tischen.
///
/// Benannte Grenzen:
/// - Nur das erste Signal zählt. Ein zweites, während der Fisch anhält,
///   beschleunigt nichts; wie bisher bei Strg+C.
/// - SIGHUP (das Terminal wird geschlossen) bleibt, wie das System es hält:
///   es beendet den Fisch sofort. Finge der Fisch es ab, liefe er auch unter
///   `nohup` nicht mehr weiter.
#[cfg(unix)]
fn asked_to_stop() -> impl Future<Output = bool> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut interrupt = signal(SignalKind::interrupt()).ok();
    let mut terminate = signal(SignalKind::terminate()).ok();
    async move {
        match (&mut interrupt, &mut terminate) {
            (Some(interrupt), Some(terminate)) => tokio::select! {
                got = interrupt.recv() => got.is_some(),
                got = terminate.recv() => got.is_some(),
            },
            (Some(only), None) | (None, Some(only)) => only.recv().await.is_some(),
            (None, None) => false,
        }
    }
}

/// Außerhalb von Unix gibt es nur Strg+C.
#[cfg(not(unix))]
fn asked_to_stop() -> impl Future<Output = bool> {
    async { tokio::signal::ctrl_c().await.is_ok() }
}

/// Versteht eine Zeile der Eingabe. Groß und klein ist beim Befehl egal, beim
/// Namen des Profils nicht.
///
/// Manche Programme, etwa die PowerShell, setzen ein unsichtbares Zeichen
/// (BOM) vor das erste, was sie in eine Eingabe schreiben. Gemessen am
/// 20.11.2026: ohne das Abstreifen blieb „stop" wirkungslos.
pub fn parse(line: &str) -> Option<Input> {
    let line = line.trim().trim_start_matches('\u{feff}').trim();
    let mut words = line.split_whitespace();
    let command = words.next()?.to_ascii_lowercase();
    let name = words.next();
    // Ein drittes Wort gehört zu keinem Befehl. Lieber nichts tun als raten.
    if words.next().is_some() {
        return None;
    }
    match (command.as_str(), name) {
        ("stop", None) => Some(Input::StopAll),
        ("stop", Some(name)) => Some(Input::Stop(name.to_owned())),
        ("start", Some(name)) => Some(Input::Start(name.to_owned())),
        ("restart", Some(name)) => Some(Input::Restart(name.to_owned())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Input, parse};

    #[test]
    fn the_word_stop_asks_to_stop_however_it_is_written() {
        assert_eq!(parse("stop"), Some(Input::StopAll));
        assert_eq!(parse("  STOP \r"), Some(Input::StopAll));
        assert_eq!(parse("\u{feff}stop"), Some(Input::StopAll), "PowerShell puts a BOM first");
        assert_eq!(parse(" \u{feff} stop "), Some(Input::StopAll));
    }

    #[test]
    fn a_profile_can_be_added_and_taken_away_by_name() {
        assert_eq!(parse("start tea-time"), Some(Input::Start("tea-time".to_owned())));
        assert_eq!(parse("  Start   probe \r"), Some(Input::Start("probe".to_owned())));
        assert_eq!(parse("stop probe"), Some(Input::Stop("probe".to_owned())));
        // Der Name bleibt, wie er geschrieben ist.
        assert_eq!(parse("stop Probe"), Some(Input::Stop("Probe".to_owned())));
        // Ein geändertes Profil wird neu geladen, ohne den Fisch anzuhalten.
        assert_eq!(parse("restart probe"), Some(Input::Restart("probe".to_owned())));
    }

    #[test]
    fn anything_else_does_nothing() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("start"), None, "start needs a name");
        assert_eq!(parse("stop it now"), None);
        assert_eq!(parse("nonstop"), None);
        assert_eq!(parse("restart"), None, "restart needs a name");
        assert_eq!(parse("reload probe"), None);
    }

    /// So hält systemd einen Dienst an. Das Signal geht an diesen Testlauf
    /// selbst; weil der Fisch es abfängt, überlebt der Testlauf es.
    #[cfg(unix)]
    #[tokio::test]
    async fn the_signal_of_a_service_manager_asks_to_stop() {
        use std::time::Duration;
        let (said, mut heard) = tokio::sync::mpsc::unbounded_channel();
        super::on_signal(said);
        let sent = std::process::Command::new("sh")
            .args(["-c", &format!("kill -TERM {}", std::process::id())])
            .status()
            .expect("sh runs");
        assert!(sent.success(), "the signal was not sent");
        let input = tokio::time::timeout(Duration::from_secs(5), heard.recv()).await;
        assert_eq!(input.expect("the signal was not heard"), Some(Input::StopAll));
        // Der Kanal bleibt danach offen: kein Ende, das wie ein zweites
        // Signal aussähe.
        let more = tokio::time::timeout(Duration::from_millis(100), heard.recv()).await;
        assert!(more.is_err(), "the channel closed or spoke again: {more:?}");
    }
}
