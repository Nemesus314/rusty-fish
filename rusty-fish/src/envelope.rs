//! Der Umschlag: alle Profile samt Bot-Token und KI-Schlüsseln in einer Datei,
//! verschlossen mit einem Kennwort. Für den Umzug auf einen anderen Rechner
//! und als Sicherung, nie zum Weitergeben an andere.
//!
//! Nemesus am 25.11.2026, nachdem „Share rooms" nur ein halbes Profil vom PC
//! nach Linux Mint gebracht hatte: „Die Import / Export Funktion scheint mir
//! essentiell. Das sollten wir gleich in Angriff nehmen." Den Export ohne
//! Geheimnisse (`profile export`) hatte er am 24.11.2026 für sich verworfen:
//! „Noch einmal irgendeinen Mist eintragen."
//!
//! Entschieden von ihm am 25.11.2026:
//! - Eine Datei, keine Zwischenablage.
//! - Das Verfahren: Argon2id und XChaCha20-Poly1305 („ja" zum dritten von drei
//!   gemessenen). Die Sonde dazu liegt neben dem Repository, in
//!   `sonden\umzug-sonde`, mit allen Zahlen in `BEFUND.md`: zwei neue Bausteine
//!   (`argon2`, `blake2`) statt 47 beim offenen Format age; Schreiben oder
//!   Öffnen rund 0,15 s am PC und rund 0,3 s in Linux Mint mit einem
//!   Prozessor; hin und zurück zwischen Windows und Mint Byte für Byte gleich.
//! - Die Namen: der Befehl `envelope` mit `write` und `read`, die Endung
//!   `.rustyfish`, in der Maske „Export…" und „Import…". Die Endung schlägt
//!   die Maske vor, mit dem Datum des Tages; dieses Programm nimmt jeden Namen.
//! - Auf dem Umschlag steht nichts über den Inhalt. Sein Gedanke: der Mensch
//!   benennt die Datei selbst, der Vorschlag trägt das Datum; wer exportiert
//!   oder importiert, weiß, was er tut.
//!
//! Die Regeln stammen aus Paciolis Ausfuhr, wie Neo sie am 24.11.2026 für den
//! Fisch aufgeschrieben hat (`D:\Instanzen\NEO\AN-FISCHLEIN-2026-11-24.md`):
//! - Die Ableitung steht im Kopf und wird beim Öffnen gelesen, nicht
//!   angenommen. Fehlt sie, wird nicht geraten.
//! - Zwölf Zeichen, erzwungen, keine Regel über die Zusammensetzung; gezählt
//!   in Zeichen, nicht in Bytes.
//! - Kein Rückfall auf Schwächeres, und die Zahlen im Kopf haben eine
//!   Untergrenze.
//! - Die erste Zeile muss genau stimmen.
//! - Ein falsches Kennwort heißt: nichts wurde verändert.
//! - Kein stilles Ersetzen: weder eine vorhandene Datei noch ein Profil noch
//!   ein Geheimnis, das am Ziel schon liegt.
//!
//! Benannte Grenzen:
//! - Der Zugang zum Matrix-Konto reist nie mit. Er ist ein Gerät mit eigenen
//!   Schlüsseln, und dasselbe Gerät darf nicht an zwei Orten laufen. Am neuen
//!   Ort meldet man den Fisch neu an.
//! - Die Datei öffnet nur dieses Programm. Ihre Form steht dafür vollständig
//!   im Leitfaden.
//! - Ein vergessenes Kennwort ist endgültig.
//! - Auf einem Linux-Server liegen die Geheimnisse nach dem Einlesen wieder
//!   im Klartext in Dateien (`key store file`): der Umschlag unterwegs ist
//!   dann stärker geschützt als das Ziel.

use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine, engine::general_purpose::STANDARD};
use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use rusty_fish_house::clock::utc_date;
use serde_json::{Value, json};
use zeroize::Zeroize;

/// Die erste Zeile der Datei. Eine andere Fassung wird nicht gelesen: sie
/// müsste geraten werden.
const FIRST_LINE: &str = "RUSTY-FISH-ENVELOPE 1";

/// Woran der Fisch den Inhalt eines Umschlags erkennt, und seine Fassung.
const BODY_FORMAT: &str = "rusty-fish-envelope";
const BODY_VERSION: u64 = 1;

/// Ab dieser Länge gilt ein Kennwort, gezählt in Zeichen.
pub const MIN_PASSWORD_CHARS: usize = 12;

/// Die Ableitung, mit der geschrieben wird: die Zahlen, die libsodium
/// „interactive" nennt und die Pacioli benutzt. Zwei Durchgänge, 64 MiB, ein
/// Strang.
const PASSES: u32 = 2;
const MEMORY_KIB: u32 = 64 * 1024;
const LANES: u32 = 1;

/// Was beim Öffnen noch angenommen wird. Darunter nicht: eine veränderte
/// Datei soll den Aufwand nicht auf fast nichts setzen können. Darüber auch
/// nicht: sonst ließe eine Datei den Rechner beliebig viel Speicher holen.
const MAX_PASSES: u32 = 10;
const MAX_MEMORY_KIB: u32 = 1024 * 1024;

/// Größer wird ein Umschlag nicht; alles darüber wird nicht erst in den
/// Speicher gelesen. Dieselbe Grenze wie bei `profile export`, um ein Drittel
/// erhöht, weil der Leib als Base64 dasteht.
pub const LIMIT: u64 = 24 * 1024 * 1024;

const SALT: usize = 16;
const NONCE: usize = 24;
const TAG: usize = 16;

/// Prüft ein Kennwort, bevor irgendetwas geschrieben wird.
pub fn check_password(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(format!(
            "the password needs at least {MIN_PASSWORD_CHARS} characters; nothing was written"
        ));
    }
    Ok(())
}

/// Was in einen Umschlag kommt: das Dokument von `profile export`, wenn es
/// Profile gibt, und die Geheimnisse, die abgelegt sind.
pub fn body(profiles: Option<&str>, secrets: &[(String, String)]) -> String {
    let secrets: serde_json::Map<String, Value> =
        secrets.iter().map(|(entry, secret)| (entry.clone(), json!(secret))).collect();
    json!({
        "format": BODY_FORMAT,
        "version": BODY_VERSION,
        // Als Text, Zeichen für Zeichen wie `profile export` ihn schreibt:
        // so nimmt das Einlesen denselben Weg wie dort.
        "profiles": profiles,
        "secrets": secrets,
    })
    .to_string()
}

/// Was in einem geöffneten Umschlag lag.
pub struct Contents {
    /// Das Dokument von `profile export`; fehlt, wenn es keine Profile gab.
    pub profiles: Option<String>,
    /// Die Geheimnisse, jedes unter dem Namen seines Eintrags.
    pub secrets: Vec<(String, String)>,
}

/// Liest den Inhalt eines geöffneten Umschlags.
pub fn contents(body: &[u8]) -> Result<Contents, String> {
    let not_one = "the envelope opened, but what it holds is not what this program wrote";
    let body: Value = serde_json::from_slice(body).map_err(|_| not_one.to_owned())?;
    if body["format"] != BODY_FORMAT {
        return Err(not_one.to_owned());
    }
    if body["version"] != BODY_VERSION {
        return Err(format!(
            "the envelope holds form {} of its contents; this program reads form {BODY_VERSION} \
             only. Nothing was changed",
            body["version"]
        ));
    }
    let profiles = match &body["profiles"] {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        _ => return Err(not_one.to_owned()),
    };
    let mut secrets = Vec::new();
    for (entry, secret) in body["secrets"].as_object().ok_or_else(|| not_one.to_owned())? {
        let secret = secret.as_str().ok_or_else(|| not_one.to_owned())?;
        secrets.push((entry.clone(), secret.to_owned()));
    }
    Ok(Contents { profiles, secrets })
}

/// Verschließt einen Inhalt mit einem Kennwort. Liefert den Text der Datei.
pub fn seal(body: &[u8], password: &str, now: u64) -> Result<String, String> {
    let mut salt = [0u8; SALT];
    let mut nonce = [0u8; NONCE];
    // Salz und Nonce sind je Umschlag neu und kommen vom System.
    getrandom::fill(&mut salt).map_err(|e| format!("this system gives no random numbers: {e}"))?;
    getrandom::fill(&mut nonce).map_err(|e| format!("this system gives no random numbers: {e}"))?;
    seal_with(body, password, now, &salt, &nonce)
}

/// Wie [`seal`], mit gegebenem Salz und gegebener Nonce: so können die Tests
/// eine Datei Zeichen für Zeichen erwarten.
fn seal_with(
    body: &[u8],
    password: &str,
    now: u64,
    salt: &[u8; SALT],
    nonce: &[u8; NONCE],
) -> Result<String, String> {
    check_password(password)?;
    let of_day = now % 86_400;
    // Im Kopf steht nur, was zum Öffnen nötig ist, und wann und womit
    // geschrieben wurde: nichts über den Inhalt.
    let head = format!(
        "{FIRST_LINE}\nwritten: {}T{:02}:{:02}:{:02}Z\nprogram: {}\nkdf: argon2id13 {PASSES} \
         {MEMORY_KIB} {LANES}\ncipher: xchacha20poly1305\n",
        utc_date(now),
        of_day / 3_600,
        of_day % 3_600 / 60,
        of_day % 60,
        env!("CARGO_PKG_VERSION"),
    );
    let mut key = derive(password, salt, PASSES, MEMORY_KIB)?;
    // Der Kopf wird mit versiegelt: wer ihn ändert, bekommt die Datei nicht
    // mehr auf. Gemessen in der Sonde: ohne das fiel eine veränderte Zahl im
    // Kopf nicht auf.
    let sealed = XChaCha20Poly1305::new(&key.into())
        .encrypt(&(*nonce).into(), Payload { msg: body, aad: head.as_bytes() });
    key.zeroize();
    let sealed = sealed.map_err(|_| "cannot seal the envelope".to_owned())?;
    let mut raw = Vec::with_capacity(SALT + NONCE + sealed.len());
    raw.extend_from_slice(salt);
    raw.extend_from_slice(nonce);
    raw.extend_from_slice(&sealed);
    let encoded = STANDARD.encode(raw);
    let mut text = head;
    text.push_str("--\n");
    // Zeilen zu 76 Zeichen, wie Pacioli: so geht der Text durch jedes
    // Programm, das Text kann.
    for line in encoded.as_bytes().chunks(76) {
        text.push_str(std::str::from_utf8(line).expect("base64 is ASCII"));
        text.push('\n');
    }
    Ok(text)
}

/// Öffnet den Text einer Datei mit dem Kennwort. Liefert den Inhalt.
pub fn open(text: &str, password: &str) -> Result<Vec<u8>, String> {
    let not_one = "this is not an envelope written by `rusty-fish envelope write`";
    // Ein Programm unterwegs darf die Zeilenenden ändern und ein BOM
    // voranstellen; gemessen in der Sonde mit den Zeilenenden von Windows.
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let (head, encoded) = text.split_once("--\n").ok_or_else(|| not_one.to_owned())?;
    let mut lines = head.lines();
    match lines.next() {
        Some(FIRST_LINE) => {}
        Some(line) if line.starts_with("RUSTY-FISH-ENVELOPE ") => {
            return Err(format!(
                "this envelope is written in another form (\"{line}\"); this program reads \
                 \"{FIRST_LINE}\" only. Nothing was changed"
            ));
        }
        _ => return Err(not_one.to_owned()),
    }
    let mut kdf = None;
    let mut cipher = None;
    for line in lines {
        match line.split_once(": ") {
            Some(("kdf", value)) => kdf = Some(value),
            Some(("cipher", value)) => cipher = Some(value),
            _ => {}
        }
    }
    // Eine geratene Ableitung endete in „falsches Kennwort", einer Meldung
    // über den Menschen für einen Fehler der Datei.
    let kdf = kdf
        .ok_or("the envelope does not say how its key is derived; not opened, nothing was changed")?;
    let unknown = "the envelope names a way to derive its key that this program does not know; not \
                   opened, nothing was changed";
    let (passes, memory_kib) = match kdf.split(' ').collect::<Vec<_>>()[..] {
        ["argon2id13", passes, memory, "1"] => (
            passes.parse::<u32>().map_err(|_| unknown.to_owned())?,
            memory.parse::<u32>().map_err(|_| unknown.to_owned())?,
        ),
        _ => return Err(unknown.to_owned()),
    };
    if !(PASSES..=MAX_PASSES).contains(&passes) || !(MEMORY_KIB..=MAX_MEMORY_KIB).contains(&memory_kib) {
        return Err(format!(
            "the envelope asks for a derivation outside what this program accepts ({passes} passes, \
             {memory_kib} KiB); not opened, nothing was changed"
        ));
    }
    if cipher != Some("xchacha20poly1305") {
        return Err("the envelope names a cipher this program does not know; not opened, nothing \
                    was changed"
            .to_owned());
    }
    let damaged = "the envelope is damaged; not opened, nothing was changed";
    let encoded: String = encoded.lines().map(str::trim).collect();
    let raw = STANDARD.decode(encoded).map_err(|_| damaged.to_owned())?;
    if raw.len() < SALT + NONCE + TAG {
        return Err(damaged.to_owned());
    }
    let (salt, rest) = raw.split_at(SALT);
    let (nonce, sealed) = rest.split_at(NONCE);
    let nonce: [u8; NONCE] = nonce.try_into().expect("split at the length of a nonce");
    let mut key = derive(password, salt, passes, memory_kib)?;
    let opened = XChaCha20Poly1305::new(&key.into())
        .decrypt(&nonce.into(), Payload { msg: sealed, aad: head.as_bytes() });
    key.zeroize();
    // Ob das Kennwort falsch ist oder die Datei verändert, lässt sich nicht
    // unterscheiden; die Meldung sagt beides und dass nichts geschah.
    opened.map_err(|_| "wrong password, or the envelope was changed; nothing was changed".to_owned())
}

fn derive(password: &str, salt: &[u8], passes: u32, memory_kib: u32) -> Result<[u8; 32], String> {
    // Kein Rückfall auf Schwächeres: geht die Ableitung nicht, gibt es
    // keinen Umschlag, und die Meldung sagt, warum.
    let cannot = |e: argon2::Error| format!("cannot derive the key from the password: {e}");
    let params = Params::new(memory_kib, passes, LANES, Some(32)).map_err(cannot)?;
    let mut key = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(cannot)?;
    Ok(key)
}

/// Was beim Einlesen mit den Geheimnissen eines Umschlags geschah.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SecretsTaken {
    /// Abgelegt: am Ziel lag unter diesem Eintrag nichts.
    pub stored: Vec<String>,
    /// Nichts zu tun: am Ziel liegt schon dasselbe.
    pub same: Vec<String>,
    /// Abgelehnt, jedes mit seinem Grund.
    pub refused: Vec<(String, String)>,
}

/// Legt die Geheimnisse eines Umschlags ab. `known` sind die Einträge, die
/// dieses Programm kennt; `load` und `store` sind die Ablage, als Parameter,
/// damit die Tests sie nicht anfassen.
///
/// Ein Geheimnis, das am Ziel schon liegt und ein anderes ist, wird nicht
/// ersetzt: wer es ersetzen will, löscht es vorher selbst (`key forget`).
pub fn take_secrets(
    secrets: &[(String, String)],
    known: &[&str],
    load: &dyn Fn(&str) -> Result<Option<String>, String>,
    store: &dyn Fn(&str, &str) -> Result<(), String>,
) -> Result<SecretsTaken, String> {
    let mut taken = SecretsTaken::default();
    for (entry, secret) in secrets {
        if !known.contains(&entry.as_str()) {
            taken.refused.push((entry.clone(), "this program has no such entry".to_owned()));
        } else if secret.trim().is_empty() {
            taken.refused.push((entry.clone(), "the envelope holds nothing under it".to_owned()));
        } else {
            match load(entry)? {
                Some(here) if here == *secret => taken.same.push(entry.clone()),
                Some(_) => taken.refused.push((
                    entry.clone(),
                    "another one is stored here already; remove it first to replace it".to_owned(),
                )),
                None => {
                    store(entry, secret)?;
                    taken.stored.push(entry.clone());
                }
            }
        }
    }
    Ok(taken)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    const PASSWORD: &str = "a made-up password";
    /// 25.11.2026, 20:38:51 UTC.
    const NOW: u64 = 1_795_639_131;

    fn sealed() -> String {
        seal_with(b"what travels", PASSWORD, NOW, &[7; SALT], &[9; NONCE]).unwrap()
    }

    #[test]
    fn what_goes_in_comes_out_the_same() {
        let text = seal(b"what travels", PASSWORD, NOW).unwrap();
        assert_eq!(open(&text, PASSWORD).unwrap(), b"what travels");
    }

    #[test]
    fn the_head_says_how_to_open_it_and_nothing_about_what_is_inside() {
        let text = sealed();
        let (head, body) = text.split_once("--\n").unwrap();
        assert_eq!(
            head,
            format!(
                "RUSTY-FISH-ENVELOPE 1\nwritten: 2026-11-25T20:38:51Z\nprogram: {}\nkdf: argon2id13 \
                 2 65536 1\ncipher: xchacha20poly1305\n",
                env!("CARGO_PKG_VERSION")
            )
        );
        // Der Leib in Zeilen zu höchstens 76 Zeichen.
        assert!(body.lines().all(|line| !line.is_empty() && line.len() <= 76), "got {body}");
        assert!(!text.contains("what travels"));
        assert!(!text.contains(PASSWORD));
    }

    #[test]
    fn two_envelopes_of_the_same_contents_differ() {
        // Salz und Nonce sind je Umschlag neu.
        let one = seal(b"what travels", PASSWORD, NOW).unwrap();
        let two = seal(b"what travels", PASSWORD, NOW).unwrap();
        assert_ne!(one, two);
    }

    #[test]
    fn a_wrong_password_opens_nothing_and_says_that_nothing_changed() {
        let why = open(&sealed(), "another password!").unwrap_err();
        assert_eq!(why, "wrong password, or the envelope was changed; nothing was changed");
    }

    #[test]
    fn twelve_characters_are_counted_as_characters() {
        // Elf Zeichen reichen nicht.
        assert!(seal(b"x", "elf-zeichen", NOW).unwrap_err().contains("at least 12 characters"));
        // Sechs Umlaute sind zwölf Bytes und doch nur sechs Zeichen; Pacioli
        // zählt Bytes und ließe sie durch (Neos Hinweis vom 24.11.2026).
        assert_eq!("äöüäöü".len(), 12);
        assert!(seal(b"x", "äöüäöü", NOW).is_err());
        let text = seal(b"x", "äöüäöüäöüäöü", NOW).unwrap();
        assert_eq!(open(&text, "äöüäöüäöüäöü").unwrap(), b"x");
    }

    #[test]
    fn a_changed_head_or_body_does_not_open() {
        let text = sealed();
        let changed = "wrong password, or the envelope was changed; nothing was changed";
        // Der Kopf ist mit versiegelt.
        let later = text.replace("2026-11-25T20:38:51Z", "2027-01-01T00:00:00Z");
        assert_eq!(open(&later, PASSWORD).unwrap_err(), changed);
        let other = text.replace("program: ", "program: 9");
        assert_eq!(open(&other, PASSWORD).unwrap_err(), changed);
        // Ein Zeichen im Leib.
        let (head, body) = text.split_once("--\n").unwrap();
        let flipped = if body.starts_with('A') { 'B' } else { 'A' };
        let body = format!("{flipped}{}", &body[1..]);
        assert_eq!(open(&format!("{head}--\n{body}"), PASSWORD).unwrap_err(), changed);
    }

    #[test]
    fn the_derivation_is_read_from_the_head_and_never_guessed() {
        let text = sealed();
        let none = text.replace("kdf: argon2id13 2 65536 1\n", "");
        assert!(open(&none, PASSWORD).unwrap_err().contains("does not say how its key is derived"));
        // Herabgesetzt: nicht geöffnet, und nicht erst gerechnet.
        let weak = text.replace("kdf: argon2id13 2 65536 1", "kdf: argon2id13 1 8 1");
        assert!(open(&weak, PASSWORD).unwrap_err().contains("outside what this program accepts"));
        // Maßlos: eine Datei lässt den Rechner nicht beliebig viel Speicher holen.
        let huge = text.replace("kdf: argon2id13 2 65536 1", "kdf: argon2id13 2 4000000000 1");
        assert!(open(&huge, PASSWORD).unwrap_err().contains("outside what this program accepts"));
        let other = text.replace("kdf: argon2id13", "kdf: scrypt");
        assert!(open(&other, PASSWORD).unwrap_err().contains("does not know"));
        let cipher = text.replace("cipher: xchacha20poly1305", "cipher: rot13");
        assert!(open(&cipher, PASSWORD).unwrap_err().contains("a cipher this program does not know"));
    }

    #[test]
    fn the_first_line_has_to_match_exactly() {
        let text = sealed();
        let next = text.replace("RUSTY-FISH-ENVELOPE 1", "RUSTY-FISH-ENVELOPE 2");
        let why = open(&next, PASSWORD).unwrap_err();
        assert!(why.contains("another form (\"RUSTY-FISH-ENVELOPE 2\")"), "got {why}");
        assert!(open("hello there", PASSWORD).unwrap_err().contains("this is not an envelope"));
        assert!(open("", PASSWORD).unwrap_err().contains("this is not an envelope"));
        // Ein Dokument von `profile export` ist kein Umschlag.
        let export = "{\n  \"format\": \"rusty-fish-profiles\"\n}\n";
        assert!(open(export, PASSWORD).unwrap_err().contains("this is not an envelope"));
    }

    #[test]
    fn line_endings_of_windows_and_a_bom_do_not_matter() {
        let text = sealed();
        let windows = format!("\u{feff}{}", text.replace('\n', "\r\n"));
        assert_eq!(open(&windows, PASSWORD).unwrap(), b"what travels");
    }

    #[test]
    fn a_cut_off_envelope_is_damaged_not_a_wrong_password() {
        let text = sealed();
        let (head, _) = text.split_once("--\n").unwrap();
        let why = open(&format!("{head}--\nAAAA\n"), PASSWORD).unwrap_err();
        assert!(why.contains("damaged"), "got {why}");
        let why = open(&format!("{head}--\n!!!\n"), PASSWORD).unwrap_err();
        assert!(why.contains("damaged"), "got {why}");
    }

    #[test]
    fn the_contents_come_back_as_they_went_in() {
        let secrets = vec![
            ("discord".to_owned(), "made-up token".to_owned()),
            ("openai".to_owned(), "made-up key".to_owned()),
        ];
        let text = body(Some("{ \"format\": \"rusty-fish-profiles\" }\n"), &secrets);
        let back = contents(text.as_bytes()).unwrap();
        assert_eq!(back.profiles.as_deref(), Some("{ \"format\": \"rusty-fish-profiles\" }\n"));
        assert_eq!(back.secrets, secrets);

        // Ohne Profile, nur mit Geheimnissen, ist es auch ein Umschlag.
        let back = contents(body(None, &secrets).as_bytes()).unwrap();
        assert_eq!(back.profiles, None);
        assert_eq!(back.secrets.len(), 2);
    }

    #[test]
    fn contents_of_another_form_are_not_read() {
        assert!(contents(b"hello").is_err());
        assert!(contents(b"{\"format\": \"something else\"}").is_err());
        let next = json!({ "format": BODY_FORMAT, "version": 2, "profiles": null, "secrets": {} });
        // Der Inhalt hat mit Absicht keine Ausgabe für Fehlersuche: darin stehen Geheimnisse.
        let why = contents(next.to_string().as_bytes()).err().unwrap();
        assert!(why.contains("form 2"), "got {why}");
        assert!(why.contains("Nothing was changed"), "got {why}");
    }

    /// Eine Ablage für die Tests: was am Ziel schon liegt, und was dazukommt.
    fn take(here: &[(&str, &str)], secrets: &[(&str, &str)]) -> (SecretsTaken, BTreeMap<String, String>) {
        let store: RefCell<BTreeMap<String, String>> =
            RefCell::new(here.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect());
        let secrets: Vec<(String, String)> =
            secrets.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        let taken = take_secrets(
            &secrets,
            &["discord", "openai", "anthropic"],
            &|entry| Ok(store.borrow().get(entry).cloned()),
            &|entry, secret| {
                store.borrow_mut().insert(entry.to_owned(), secret.to_owned());
                Ok(())
            },
        )
        .unwrap();
        (taken, store.into_inner())
    }

    #[test]
    fn secrets_go_where_nothing_lies_yet() {
        let (taken, store) = take(&[], &[("discord", "token"), ("openai", "key")]);
        assert_eq!(taken.stored, ["discord", "openai"]);
        assert!(taken.same.is_empty() && taken.refused.is_empty());
        assert_eq!(store["discord"], "token");
        assert_eq!(store["openai"], "key");
    }

    #[test]
    fn a_secret_that_lies_here_already_is_never_replaced() {
        let (taken, store) =
            take(&[("discord", "the one here"), ("openai", "key")], &[("discord", "token"), ("openai", "key")]);
        // Dasselbe: nichts zu tun. Ein anderes: abgelehnt, mit dem Grund.
        assert_eq!(taken.same, ["openai"]);
        assert!(taken.stored.is_empty());
        assert_eq!(taken.refused.len(), 1);
        assert_eq!(taken.refused[0].0, "discord");
        assert!(taken.refused[0].1.contains("remove it first"), "got {:?}", taken.refused);
        assert_eq!(store["discord"], "the one here");
    }

    #[test]
    fn an_entry_this_program_does_not_know_is_refused() {
        // Auch der Zugang zu Matrix: er reist nie mit, und stünde er in einer
        // Datei, würde er nicht abgelegt.
        let (taken, store) = take(&[], &[("matrix", "a session"), ("openai", "  ")]);
        assert!(taken.stored.is_empty());
        assert_eq!(taken.refused.len(), 2);
        assert!(store.is_empty());
    }
}
