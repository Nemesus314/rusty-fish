//! Was dem laufenden Fisch von außen gesagt wird: anhalten, ein weiteres
//! Profil bedienen, ein Profil nicht mehr bedienen.
//!
//! Bis zum 21.11.2026 stand das Anhalten in der Discord-Tür. Es gehört dem
//! Programm: ein Fisch mit zwei Türen wird einmal angehalten, nicht einmal je
//! Tür. Seit demselben Tag bedient ein Fisch mehrere Profile; die Maske sagt
//! ihm über dieselbe Leitung, welche.

use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

/// Eine Zeile der Standardeingabe, verstanden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// `stop`: den ganzen Fisch anhalten.
    StopAll,
    /// `start NAME`: ein weiteres Profil bedienen.
    Start(String),
    /// `stop NAME`: dieses Profil nicht mehr bedienen.
    Stop(String),
}

/// Hört zu, was dem Fisch gesagt wird: Zeilen auf der Standardeingabe (so
/// spricht die Maske mit ihm) und Strg+C im Terminal, das wie `stop` gilt.
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
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            let _ = said.send(Input::StopAll);
        }
        // Dieser Sender bleibt bis zum Ende des Programms am Leben. Endet die
        // Eingabe, oder lässt sich Strg+C nicht abfangen, sieht der Empfänger
        // deshalb kein Ende des Kanals, das wie ein Signal aussähe.
        std::future::pending::<()>().await;
        drop(said);
    });
    heard
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
    }

    #[test]
    fn anything_else_does_nothing() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("start"), None, "start needs a name");
        assert_eq!(parse("stop it now"), None);
        assert_eq!(parse("nonstop"), None);
        assert_eq!(parse("restart probe"), None);
    }
}
