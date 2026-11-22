//! Der Link, mit dem der eigene Bot auf einen Discord-Server kommt. Rein,
//! ohne Netz.
//!
//! Gemessen am 22.11.2026 im ersten Lauf mit zwei Fischen: bringt jeder seinen
//! eigenen Fisch mit, muss der Bot des einen auf den Server des anderen. Dafür
//! schickt man dem Besitzer des Servers einen Link. Den hat Fischlein für
//! Nemesus von Hand zusammengesetzt; ein Freund ohne Entwicklerwissen hätte
//! niemanden, der das tut. Nemesus dazu: „dann müssten also meine Freunde mir
//! diesen Link schicken".
//!
//! Der Link enthält nur die Kennung des Bots, kein Geheimnis. Die Kennung
//! steht im ersten Teil des Tokens, lesbar für jeden, der das Token hat; das
//! Token selbst verlässt das Programm dabei nicht.
//!
//! Benannte Grenze: bei sehr alten Bots (angelegt vor 2016) ist die Kennung
//! des Bots nicht die Kennung der Anwendung, und der Link stimmt nicht. Dann
//! hilft nur die „Application ID" aus dem Entwicklerportal.

/// Die vier Rechte, die der Fisch braucht, als die Zahl, die Discord im Link
/// erwartet: Kanäle ansehen, Nachrichten senden, Nachrichtenverlauf lesen,
/// Webhooks verwalten.
pub const PERMISSIONS: u64 = (1 << 10) | (1 << 11) | (1 << 16) | (1 << 29);

/// Die Kennung des Bots, gelesen aus seinem Token. `None`, wenn der Text
/// nicht wie ein Token von Discord aussieht.
pub fn bot_id(token: &str) -> Option<String> {
    let first = token.trim().split('.').next()?;
    let id = String::from_utf8(base64(first)?).ok()?;
    // Eine Kennung von Discord: Ziffern, nicht null, höchstens 20 Stellen.
    let sound = !id.is_empty()
        && id.len() <= 20
        && id.bytes().all(|b| b.is_ascii_digit())
        && id.bytes().any(|b| b != b'0');
    sound.then_some(id)
}

/// Der Link für den Bot mit dieser Kennung. Wer ihn öffnet, wählt den Server
/// und sieht die Rechte, bevor er zustimmt.
pub fn link(bot_id: &str) -> String {
    format!(
        "https://discord.com/oauth2/authorize?client_id={bot_id}&scope=bot&permissions={PERMISSIONS}"
    )
}

/// Liest Base64, in der gewöhnlichen und in der Schreibweise für Adressen,
/// mit oder ohne Füllzeichen am Ende. Von Hand, weil es wenige Zeilen sind
/// und dafür keine weitere Bibliothek ins Programm soll.
fn base64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut bits, mut count) = (0u32, 0u32);
    for c in text.trim_end_matches('=').bytes() {
        let value = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(value);
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Eine erfundene Kennung und ein erfundenes Token in der Gestalt, die
    // Discord ihnen gibt. Kein echtes.
    const ID: &str = "123456789012345678";
    const TOKEN: &str = "MTIzNDU2Nzg5MDEyMzQ1Njc4.GAbcde.not-a-real-secret_0123456789";

    #[test]
    fn the_id_of_the_bot_is_read_from_the_first_part_of_its_token() {
        assert_eq!(bot_id(TOKEN).as_deref(), Some(ID));
        assert_eq!(bot_id(&format!("  {TOKEN}\n")).as_deref(), Some(ID));
        // Mit Füllzeichen, wie manche Werkzeuge es schreiben.
        assert_eq!(bot_id("MTIzNDU2Nzg5MDEyMzQ1Njc4OQ==.x.y").as_deref(), Some("1234567890123456789"));
    }

    #[test]
    fn what_is_no_token_gives_no_id_instead_of_a_wrong_link() {
        assert_eq!(bot_id(""), None);
        assert_eq!(bot_id("not a token"), None);
        // Base64, aber keine Zahl dahinter.
        assert_eq!(bot_id("aGVsbG8.x.y"), None);
        // Nur Nullen wären keine Kennung.
        assert_eq!(bot_id("MDAwMA.x.y"), None);
        assert_eq!(bot_id("!!!.x.y"), None);
    }

    #[test]
    fn the_link_asks_for_exactly_the_four_rights_the_fish_needs() {
        // Kanäle ansehen 1024, Nachrichten senden 2048, Verlauf lesen 65536,
        // Webhooks verwalten 536870912.
        assert_eq!(PERMISSIONS, 1024 + 2048 + 65_536 + 536_870_912);
        assert_eq!(
            link(ID),
            "https://discord.com/oauth2/authorize?client_id=123456789012345678&scope=bot&permissions=536939520"
        );
    }

    #[test]
    fn the_link_carries_nothing_of_the_secret() {
        let link = link(&bot_id(TOKEN).unwrap());
        assert!(!link.contains("GAbcde") && !link.contains("not-a-real-secret"));
    }
}
