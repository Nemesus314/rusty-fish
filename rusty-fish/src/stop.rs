//! Wann das Programm anhalten soll.
//!
//! Bis zum 21.11.2026 stand das in der Discord-Tür. Es gehört dem Programm:
//! ein Fisch mit zwei Türen wird einmal angehalten, nicht einmal je Tür.

/// Wartet, bis jemand den Fisch anhalten will: die Zeile „stop" auf der
/// Standardeingabe (so hält ihn die Maske an) oder Strg+C im Terminal.
///
/// Das bloße Ende der Standardeingabe hält ihn nicht an. Als Dienst ohne
/// Terminal hat er gar keine Eingabe, und er soll trotzdem laufen.
pub async fn requested() {
    let (asked, was_asked) = tokio::sync::oneshot::channel::<()>();
    // Ein eigener Thread, weil das Lesen der Eingabe blockiert. Er darf beim
    // Beenden des Programms einfach liegen bleiben.
    std::thread::spawn(move || {
        let stop = std::io::stdin().lines().map_while(Result::ok).any(|line| says_stop(&line));
        if stop {
            let _ = asked.send(());
        } else {
            // Eingabe zu Ende, ohne dass jemand „stop" sagte: den Sender nicht
            // fallen lassen, sonst sähe das für den Empfänger wie ein Signal aus.
            std::mem::forget(asked);
        }
    });
    let ctrl_c = async {
        if tokio::signal::ctrl_c().await.is_err() {
            // Lässt sich Strg+C nicht abfangen, bleibt nur die Eingabe.
            std::future::pending::<()>().await;
        }
    };
    tokio::select! {
        _ = was_asked => {}
        () = ctrl_c => {}
    }
}

/// Ist diese Zeile der Eingabe die Bitte anzuhalten?
///
/// Manche Programme, etwa die PowerShell, setzen ein unsichtbares Zeichen
/// (BOM) vor das erste, was sie in eine Eingabe schreiben. Gemessen am
/// 20.11.2026: ohne das Abstreifen blieb „stop" wirkungslos.
fn says_stop(line: &str) -> bool {
    line.trim().trim_start_matches('\u{feff}').trim().eq_ignore_ascii_case("stop")
}

#[cfg(test)]
mod tests {
    use super::says_stop;

    #[test]
    fn the_word_stop_asks_to_stop_however_it_is_written() {
        assert!(says_stop("stop"));
        assert!(says_stop("  STOP \r"));
        assert!(says_stop("\u{feff}stop"), "PowerShell puts a BOM before the first line");
        assert!(says_stop(" \u{feff} stop "));
    }

    #[test]
    fn anything_else_does_not() {
        assert!(!says_stop(""));
        assert!(!says_stop("stop it"));
        assert!(!says_stop("nonstop"));
    }
}
