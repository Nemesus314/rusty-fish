//! Das heutige Datum in UTC. Das Kontingent gilt pro Tag, und der Tag soll
//! überall derselbe sein, egal in welcher Zeitzone der Rechner steht.
//!
//! Bewusst ohne eigene Bibliothek: gebraucht wird nur das Datum.

use std::time::{SystemTime, UNIX_EPOCH};

/// Datum zu Sekunden seit dem 1.1.1970, als "2026-11-19". Rein.
pub fn utc_date(seconds_since_epoch: u64) -> String {
    // Tage seit 1970 in Jahr, Monat, Tag umrechnen. Das Verfahren zählt in
    // Zeitaltern von 400 Jahren, in denen sich der Kalender genau wiederholt,
    // und lässt das Jahr im März beginnen, damit der Schalttag am Ende steht.
    let days = seconds_since_epoch / 86_400 + 719_468; // Tage seit dem 1.3.0000
    let era = days / 146_097;
    let day_of_era = days % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153; // 0 = März
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 { shifted_month + 3 } else { shifted_month - 9 };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Das heutige Datum laut Uhr des Rechners.
pub fn today() -> String {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    utc_date(seconds)
}
