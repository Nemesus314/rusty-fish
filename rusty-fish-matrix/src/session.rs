//! Das Konto des Fischs bei Matrix und seine Sitzung.
//!
//! Bei Matrix gibt es kein Entwicklerportal und keinen Bot-Token. Der Fisch
//! meldet sich als gewöhnliches Konto an, mit Kennung und Passwort, und
//! bekommt dafür eine Sitzung: ein eigenes Gerät dieses Kontos mit einem
//! Zugangsschlüssel. Das Passwort wird dabei einmal gebraucht und nirgends
//! aufbewahrt.
//!
//! Aufbewahrt wird zweierlei, an zwei Orten:
//! - Der Zugang ([`Access`]): Server, Kennung, Gerät, Zugangsschlüssel und
//!   das Kennwort der Ablage. Er ist ein Geheimnis und gehört in die
//!   Schlüsselablage des Systems; das besorgt der Aufrufer.
//! - Die Ablage des Geräts: ein Ordner mit den Geräteschlüsseln, ohne die der
//!   Fisch in einem verschlüsselten Raum nichts lesen kann. Sie liegt im
//!   eigenen Ordner des Fischs, verschlossen mit dem Kennwort aus dem Zugang.
//!
//! Benannte Grenzen:
//! - Es gibt keine Sicherung der Geräteschlüssel, mit Absicht. Geht die
//!   Ablage verloren, meldet man den Fisch neu an; er ist dann ein neues
//!   Gerät und liest, was ab da geschrieben wird.
//! - Das Gerät des Fischs zeichnet das Konto selbst gegen: bei einem neuen
//!   Konto legt der Fisch die Schlüssel dafür an, bei einem Konto, das sie
//!   schon hat, weist er sich mit dem Wiederherstellungsschlüssel aus. Ohne
//!   beides zeigen manche Programme einen Hinweis an seinen Nachrichten.
//! - Anmelden geht nur mit Passwort. Ein Server, der nur die Anmeldung über
//!   den Browser anbietet, wird nicht bedient.

use matrix_sdk::authentication::matrix::MatrixSession;
use matrix_sdk::encryption::secret_storage::SecretStorageError;
use matrix_sdk::ruma::api::client::uiaa::{AuthData, Password, UserIdentifier};
use matrix_sdk::ruma::events::secret::request::SecretName;
use matrix_sdk::ruma::{OwnedUserId, UserId};
use matrix_sdk::store::RoomLoadSettings;
use matrix_sdk::{Client, SessionMeta, SessionTokens};
use matrix_sdk_crypto::types::{CrossSigningKey, DeviceKeys, SelfSigningPubkey};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Wie das Gerät des Fischs in der Geräteliste des Kontos heißt.
const DEVICE_NAME: &str = "Rusty Fish";

/// Der Zugang des Fischs zu seinem Matrix-Konto: alles, was es braucht, um
/// die Sitzung wiederaufzunehmen. Ein Geheimnis.
///
/// Die Feldnamen sind kurz, weil die Schlüsselablage von Windows wenig Platz
/// je Eintrag hat.
#[derive(Clone, Serialize, Deserialize)]
pub struct Access {
    #[serde(rename = "h")]
    homeserver: String,
    #[serde(rename = "u")]
    user_id: OwnedUserId,
    #[serde(rename = "d")]
    device_id: String,
    #[serde(rename = "t")]
    access_token: String,
    /// Das Kennwort, mit dem die Ablage des Geräts verschlossen ist.
    #[serde(rename = "p")]
    passphrase: String,
}

/// Zeigt nie, was geheim ist.
impl std::fmt::Debug for Access {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Access")
            .field("homeserver", &self.homeserver)
            .field("user_id", &self.user_id)
            .field("device_id", &self.device_id)
            .finish_non_exhaustive()
    }
}

impl Access {
    /// Liest den Zugang, wie er in der Schlüsselablage liegt.
    pub fn parse(secret: &str) -> Result<Access, String> {
        // Der Grund wird nicht weitergegeben: er könnte ein Stück des
        // Geheimnisses zitieren.
        serde_json::from_str(secret.trim()).map_err(|_| {
            "the stored Matrix access cannot be read; log in to Matrix again".to_owned()
        })
    }

    /// Der Zugang als ein Text für die Schlüsselablage.
    pub fn to_secret(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Die Kennung des Kontos, etwa `@fish:example.org`. Kein Geheimnis.
    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    /// Die Adresse des Servers. Kein Geheimnis.
    pub fn homeserver(&self) -> &str {
        &self.homeserver
    }

    fn session(&self) -> MatrixSession {
        MatrixSession {
            meta: SessionMeta {
                user_id: self.user_id.clone(),
                device_id: self.device_id.as_str().into(),
            },
            tokens: SessionTokens { access_token: self.access_token.clone(), refresh_token: None },
        }
    }
}

/// Was eine Anmeldung ergibt: der Zugang, und was dabei zu sagen war.
#[derive(Debug)]
pub struct LoggedIn {
    pub access: Access,
    /// Hinweise für den Menschen, ohne Geheimnis: etwa dass das Gerät nicht
    /// gegengezeichnet werden konnte.
    pub notes: Vec<String>,
}

/// Prüft die Kennung eines Kontos, wie ein Mensch sie eintippt.
pub fn user_id(typed: &str) -> Result<OwnedUserId, String> {
    UserId::parse(typed.trim()).map_err(|_| {
        format!("a Matrix user id looks like @name:server, got {:?}", typed.trim())
    })
}

/// Meldet den Fisch an. Er wird ein neues Gerät des Kontos; eine Ablage von
/// einer früheren Anmeldung wird dafür geleert, denn ihre Schlüssel gehören
/// zu einem anderen Gerät.
///
/// Ohne `homeserver` wird der Server aus der Kennung gefunden, so wie es die
/// Programme für Menschen tun.
///
/// `recovery` ist der Wiederherstellungsschlüssel des Kontos, wenn es einen
/// hat. Mit ihm weist sich das neue Gerät als Gerät dieses Kontos aus. Er
/// wird einmal gebraucht und nirgends aufbewahrt.
pub async fn login(
    store: &Path,
    user: &str,
    password: &str,
    homeserver: Option<&str>,
    recovery: Option<&str>,
) -> Result<LoggedIn, String> {
    let user = user_id(user)?;
    if password.is_empty() {
        return Err("nothing entered, nothing done".to_owned());
    }
    clear(store)?;
    create_private(store).map_err(|e| format!("cannot create {}: {e}", store.display()))?;
    let passphrase = passphrase()?;

    let builder = Client::builder().sqlite_store(store, Some(&passphrase));
    let builder = match homeserver.map(str::trim).filter(|h| !h.is_empty()) {
        Some(url) => builder.homeserver_url(url),
        None => builder.server_name(user.server_name()),
    };
    let client = builder
        .build()
        .await
        .map_err(|e| format!("cannot reach the Matrix server of {user}: {e}"))?;

    let offered = client
        .matrix_auth()
        .get_login_types()
        .await
        .map_err(|e| format!("cannot ask the Matrix server how to log in: {e}"))?;
    let with_password = offered.flows.iter().any(|flow| flow.login_type() == "m.login.password");
    if !with_password {
        return Err(format!(
            "the Matrix server {} offers no login with a password, and this fish knows no other",
            client.homeserver()
        ));
    }

    client
        .matrix_auth()
        .login_username(user.as_str(), password)
        .initial_device_display_name(DEVICE_NAME)
        .send()
        .await
        .map_err(|e| format!("the Matrix login failed: {e}"))?;
    let session = client
        .matrix_auth()
        .session()
        .ok_or_else(|| "the Matrix login gave no session".to_owned())?;

    // Das Gerät gegenzeichnen, solange das Passwort da ist. Ohne das zeigen
    // manche Programme an jeder Nachricht des Fischs eine Warnung. Scheitert
    // es, läuft der Fisch trotzdem.
    let mut notes = Vec::new();
    if let Err(why) = cross_sign(&client, &session.meta.user_id, password).await {
        notes.push(format!(
            "the device could not be cross-signed ({why}); the fish works all the same, some \
             apps show a warning at its messages"
        ));
    }

    // Hat das Konto seine Schlüssel schon, etwa weil ein Mensch sich mit einem
    // anderen Programm angemeldet hat, kann nur der Wiederherstellungsschlüssel
    // das neue Gerät ausweisen. Scheitert es, läuft der Fisch trotzdem.
    if let Some(key) = recovery.map(str::trim).filter(|key| !key.is_empty()) {
        match prove(&client, &session.meta.user_id, key).await {
            Ok(()) => notes.push(
                "the account's recovery key has signed this device; apps show the fish as a \
                 verified device of its account"
                    .to_owned(),
            ),
            Err(why) => notes.push(format!(
                "the recovery key did not verify the device ({why}); the fish works all the \
                 same, some apps show a warning at its messages"
            )),
        }
    }

    let access = Access {
        homeserver: client.homeserver().to_string(),
        user_id: session.meta.user_id,
        device_id: session.meta.device_id.to_string(),
        access_token: session.tokens.access_token,
        passphrase,
    };
    Ok(LoggedIn { access, notes })
}

/// Legt für ein neues Konto die Schlüssel an, mit denen es seine eigenen
/// Geräte gegenzeichnet. Hat das Konto sie schon, geschieht nichts.
async fn cross_sign(client: &Client, user: &UserId, password: &str) -> Result<(), String> {
    let encryption = client.encryption();
    let first = match encryption.bootstrap_cross_signing_if_needed(None).await {
        Ok(()) => return Ok(()),
        Err(first) => first,
    };
    // Der Server will das Passwort noch einmal sehen, für genau diesen Schritt.
    let Some(asked) = first.as_uiaa_response() else { return Err(first.to_string()) };
    let mut auth = Password::new(UserIdentifier::from(user.to_owned()), password.to_owned());
    auth.session = asked.session.clone();
    encryption
        .bootstrap_cross_signing_if_needed(Some(AuthData::Password(auth)))
        .await
        .map_err(|e| e.to_string())
}

/// Weist das Gerät mit dem Wiederherstellungsschlüssel des Kontos aus: holt
/// damit die Schlüssel, mit denen das Konto seine Geräte gegenzeichnet, und
/// zeichnet dieses Gerät.
///
/// Nur diese Schlüssel. Die Sicherung der Raumschlüssel, die derselbe
/// Wiederherstellungsschlüssel öffnen würde, bleibt unberührt: der Fisch
/// nimmt an ihr nicht teil.
///
/// Stand 23.11.2026: einmal gegen ein echtes Konto bei matrix.org gelaufen
/// (Nemesus); der Server zeigt das Gerät seither als gegengezeichnet.
async fn prove(client: &Client, user: &UserId, key: &str) -> Result<(), String> {
    let encryption = client.encryption();
    let secrets =
        encryption.secret_storage().open_secret_store(key).await.map_err(|e| match e {
            SecretStorageError::MissingKeyInfo { .. } => {
                "this account has no recovery set up; set it up in a Matrix app first".to_owned()
            }
            SecretStorageError::SecretStorageKey(_) => {
                "the recovery key does not fit this account".to_owned()
            }
            other => other.to_string(),
        })?;

    let mut held = Vec::new();
    for name in [
        SecretName::CrossSigningMasterKey,
        SecretName::CrossSigningSelfSigningKey,
        SecretName::CrossSigningUserSigningKey,
    ] {
        let secret = secrets
            .get_secret(name)
            .await
            .map_err(|e| e.to_string())?
            .ok_or("the recovery of this account holds no signing keys")?;
        held.push(secret);
    }
    // Ein Fehler hier darf nichts von dem zeigen, was in dem Bündel steht.
    let bundle = serde_json::from_value(serde_json::json!({
        "cross_signing": {
            "master_key": held[0],
            "self_signing_key": held[1],
            "user_signing_key": held[2],
        },
        "backup": null,
    }))
    .map_err(|_| "the signing keys from the recovery cannot be read".to_owned())?;
    encryption.import_secrets_bundle(&bundle).await.map_err(|e| e.to_string())?;

    // Das eigene Gerät vom Server holen, zeichnen, und nachsehen, ob der
    // Server es danach als gegengezeichnet zeigt.
    encryption.request_user_identity(user).await.map_err(|e| e.to_string())?;
    let own = encryption
        .get_own_device()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("the server does not show the fish's own device yet")?;
    own.verify().await.map_err(|e| e.to_string())?;
    encryption.request_user_identity(user).await.map_err(|e| e.to_string())?;
    let own = encryption
        .get_own_device()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("the server does not show the fish's own device")?;
    if own.is_cross_signed_by_owner() {
        Ok(())
    } else {
        Err("the device was signed, but the server does not show it as verified".to_owned())
    }
}

/// Nimmt die Sitzung wieder auf: öffnet die Ablage des Geräts und stellt den
/// Zugang ein. Spricht noch nicht mit dem Server.
pub async fn connect(access: &Access, store: &Path) -> Result<Client, String> {
    // Den Ordner legt sonst die Bibliothek an, mit den üblichen Rechten.
    // Gemessen am 25.11.2026: ein Fisch, der ohne vorherige Anmeldung auf
    // diesem Rechner startete, bekam ihn unter Linux mit 0755. Im gewöhnlichen
    // Ablauf legt die Anmeldung ihn vorher an; hier noch einmal, damit es
    // nicht davon abhängt.
    create_private(store).map_err(|e| format!("cannot create {}: {e}", store.display()))?;
    let client = Client::builder()
        .homeserver_url(&access.homeserver)
        .sqlite_store(store, Some(&access.passphrase))
        .build()
        .await
        .map_err(|e| {
            format!(
                "cannot open the fish's Matrix device store in {} ({e}); log in to Matrix again",
                store.display()
            )
        })?;
    client
        .matrix_auth()
        .restore_session(access.session(), RoomLoadSettings::default())
        .await
        .map_err(|e| format!("cannot take up the Matrix session again ({e}); log in to Matrix again"))?;
    Ok(client)
}

/// Die Sitzung ohne die Ablage des Geräts: genug, um dem Server etwas zu
/// sagen, zu wenig, um in einem verschlüsselten Raum zu lesen.
async fn without_store(access: &Access) -> Result<Client, String> {
    let client = Client::builder()
        .homeserver_url(&access.homeserver)
        .build()
        .await
        .map_err(|e| format!("cannot reach the Matrix server: {e}"))?;
    client
        .matrix_auth()
        .restore_session(access.session(), RoomLoadSettings::default())
        .await
        .map_err(|e| format!("cannot take up the Matrix session again: {e}"))?;
    Ok(client)
}

/// Meldet den Fisch ab: die Sitzung endet beim Server, und die Ablage des
/// Geräts wird geleert. Erreicht er den Server nicht, wird trotzdem geleert;
/// zurück kommt dann der Grund als Hinweis.
pub async fn logout(access: &Access, store: &Path) -> Result<Option<String>, String> {
    // Dem Server genügt der Zugang. Die Ablage wird dafür nicht geöffnet:
    // die Bibliothek hielte ihre Dateien offen, solange dieses Programm
    // läuft, und Windows löscht keine offene Datei. Gemessen am 22.11.2026
    // im ersten echten Lauf: Abmelden scheiterte genau daran.
    let told = match without_store(access).await {
        Ok(client) => client.matrix_auth().logout().await.err().map(|e| e.to_string()),
        Err(why) => Some(why),
    };
    clear(store)?;
    Ok(told.map(|why| {
        format!(
            "the Matrix server was not told ({why}); remove the device \"{DEVICE_NAME}\" in the \
             account's own settings"
        )
    }))
}

/// Was der Server über das Gerät des Fischs zeigt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Device {
    /// Das Konto hat dieses Gerät gegengezeichnet, und die Unterschrift stimmt.
    Verified,
    /// Der Server zeigt das Gerät ohne gültige Unterschrift des Kontos.
    Unverified,
    /// Der Server nimmt den gespeicherten Zugang nicht mehr an.
    Refused,
    /// Es ließ sich nicht klären; der Grund, ohne Geheimnis.
    Unknown(String),
}

/// Wie lange auf die Antwort des Servers gewartet wird. Die Maske fragt beim
/// Öffnen; sie soll daran nicht hängen.
const ASK_FOR: std::time::Duration = std::time::Duration::from_secs(10);

/// Fragt den Server, ob das Gerät des Fischs vom Konto gegengezeichnet ist.
///
/// Gewünscht von Nemesus am 23.11.2026, nachdem die Anmeldung mit dem
/// Wiederherstellungsschlüssel am echten Konto gelaufen war: ein Haken als
/// dauerhafte Anzeige, nicht nur der eine Satz nach dem Anmelden.
///
/// Gefragt wird mit einer einzelnen Anfrage, ohne die Ablage des Geräts und
/// ohne die Sitzung der Bibliothek. So geht es auch, während der Fisch läuft,
/// und nichts wird dabei beim Server verändert.
///
/// Geprüft wird die Unterschrift des Geräts mit dem Schlüssel, mit dem das
/// Konto seine Geräte zeichnet, so wie der Server ihn nennt. Dass dieser
/// Schlüssel selbst der richtige ist, prüft hier niemand; das tut ein Mensch,
/// der das Konto in seinem eigenen Programm bestätigt.
pub async fn device(access: &Access) -> Device {
    let unknown = |why: String| Device::Unknown(why);
    let http = match matrix_sdk::reqwest::Client::builder().timeout(ASK_FOR).build() {
        Ok(http) => http,
        Err(e) => return unknown(format!("cannot ask the Matrix server: {e}")),
    };
    let user = access.user_id.as_str();
    let device = access.device_id.as_str();
    let asked = serde_json::json!({ "device_keys": { user: [device] } });
    let url = format!("{}/_matrix/client/v3/keys/query", access.homeserver.trim_end_matches('/'));
    // Der Grund eines Fehlers nennt die Adresse, nie den Zugangsschlüssel:
    // der steht in einer Kopfzeile, nicht in der Adresse.
    let response = match http
        .post(url)
        .bearer_auth(&access.access_token)
        .header("content-type", "application/json")
        .body(asked.to_string())
        .send()
        .await
    {
        Ok(response) => response,
        Err(e) => return unknown(format!("cannot reach the Matrix server: {e}")),
    };
    let status = response.status();
    if status.as_u16() == 401 {
        return Device::Refused;
    }
    if !status.is_success() {
        return unknown(format!("the Matrix server answered {status}"));
    }
    let body: serde_json::Value = match response.bytes().await {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(body) => body,
            Err(_) => return unknown("the answer of the Matrix server cannot be read".to_owned()),
        },
        Err(e) => return unknown(format!("the answer of the Matrix server broke off: {e}")),
    };

    let shown = &body["device_keys"][user][device];
    if shown.is_null() {
        return unknown("the Matrix server does not show the fish's device".to_owned());
    }
    let Ok(keys) = serde_json::from_value::<DeviceKeys>(shown.clone()) else {
        return unknown("the fish's device, as the Matrix server shows it, cannot be read".to_owned());
    };
    // Ein Server, der ein anderes Gerät zeigt als das gefragte, beweist nichts.
    if keys.user_id != access.user_id || keys.device_id.as_str() != device {
        return Device::Unverified;
    }
    let signing = serde_json::from_value::<CrossSigningKey>(body["self_signing_keys"][user].clone())
        .ok()
        .and_then(|key| SelfSigningPubkey::try_from(key).ok());
    match signing {
        Some(signing) if signing.verify_device_keys(&keys).is_ok() => Device::Verified,
        _ => Device::Unverified,
    }
}

/// Leert die Ablage des Geräts: die Dateien, die die Matrix-Bibliothek dort
/// angelegt hat, und nichts sonst. Der Ordner selbst bleibt.
pub fn clear(store: &Path) -> Result<(), String> {
    let entries = match std::fs::read_dir(store) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("cannot read {}: {e}", store.display())),
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("matrix-sdk-") && entry.path().is_file() {
            std::fs::remove_file(entry.path()).map_err(|e| {
                format!(
                    "cannot remove {} ({e}); if the fish is running, switch it off first",
                    entry.path().display()
                )
            })?;
        }
    }
    Ok(())
}

/// Legt die Ablage des Geräts an. Unter Unix nur für das Konto des Fischs
/// (0700), wie das Programm seine übrigen Ordner anlegt; ein Ordner, den es
/// schon gibt, bleibt, wie er ist.
fn create_private(store: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(store)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(store)
    }
}

/// Ein zufälliges Kennwort für die Ablage des Geräts.
fn passphrase() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("this system gave no random numbers: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
