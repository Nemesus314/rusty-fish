//! Profile: mehrere gespeicherte Einstellungen, jede mit ihrem eigenen Zustand.
//!
//! Nemesus am 20.11.2026: „Wir könnten Profile speichern. Dann bräuchten wir
//! Bots nicht mehr clonen. Du öffnest das Win-Programm und bekommst eine Maske
//! mit allen Profilen. Jedes kannst du editieren und danach speichern.
//! Gespeicherte Profile kannst du starten."
//!
//! Ein Profil ist eine Einstellungsdatei im bekannten Format, `<name>.toml`,
//! mit ihrer Zustandsdatei `<name>.state.json` daneben. Dieses Programm ist der
//! einzige Leser und Schreiber dieser Dateien. Eine Oberfläche spricht nur
//! über die Befehle `rusty-fish profile …` mit ihm und tauscht JSON aus. So
//! läuft dasselbe Profil später auch auf einem Server ohne Bildschirm.

use crate::lock;
use rusty_fish_house::clock::utc_date;
use rusty_fish_house::config::Config;
use rusty_fish_house::document::Document;
use rusty_fish_house::state::Store;
use rusty_fish_house::texts::service_name;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Der Bot-Eintrag in der Ablage, nach dem die Sperre benannt ist.
pub const BOT: &str = "discord";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Other,
}

pub fn this_os() -> Os {
    if cfg!(windows) {
        Os::Windows
    } else if cfg!(target_os = "macos") {
        Os::MacOs
    } else {
        Os::Other
    }
}

/// Der Ordner, in dem Rusty Fish seine Dinge ablegt. `get` liest eine
/// Umgebungsvariable; als Parameter, damit die Tests die Umgebung des
/// Prozesses nicht verändern müssen.
///
/// `RUSTY_FISH_HOME` ersetzt den ganzen Ordner, für Tests und Sonderfälle.
/// Bewusst ohne eigene Bibliothek: es sind drei Fälle.
pub fn home(get: &dyn Fn(&str) -> Option<String>, os: Os) -> Result<PathBuf, String> {
    // Leer gesetzt zählt als nicht gesetzt.
    let set = |key: &str| get(key).map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
    if let Some(explicit) = set("RUSTY_FISH_HOME") {
        return Ok(PathBuf::from(explicit));
    }
    let found = match os {
        Os::Windows => set("APPDATA").map(|appdata| PathBuf::from(appdata).join("rusty-fish")),
        Os::MacOs => set("HOME").map(|home| {
            PathBuf::from(home).join("Library").join("Application Support").join("rusty-fish")
        }),
        Os::Other => set("XDG_CONFIG_HOME")
            .map(|config| PathBuf::from(config).join("rusty-fish"))
            .or_else(|| set("HOME").map(|home| PathBuf::from(home).join(".config").join("rusty-fish"))),
    };
    // Lieber laut scheitern als Profile an einem geratenen Ort ablegen.
    found.ok_or_else(|| {
        "cannot tell where to keep the profiles on this system; set RUSTY_FISH_HOME to a folder"
            .to_owned()
    })
}

/// Namen, die Windows für Geräte reserviert. Eine Datei `con.toml` lässt sich
/// dort nicht anlegen.
const DEVICE_NAMES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Prüft einen Profilnamen: nur a-z, 0-9, Bindestrich und Unterstrich, 1 bis
/// 40 Zeichen. Der Name wird ein Dateiname, deshalb so eng.
pub fn valid_name(name: &str) -> Result<(), String> {
    let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_';
    if name.is_empty() || name.len() > 40 || !name.chars().all(allowed) {
        return Err(format!(
            "a profile name may use only a-z, 0-9, '-' and '_', and be 1 to 40 characters long; \
             got {name:?}"
        ));
    }
    if DEVICE_NAMES.contains(&name) {
        return Err(format!("{name:?} is a name Windows keeps for devices; choose another one"));
    }
    Ok(())
}

/// Zeitstempel für Dateinamen im Papierkorb, in UTC: "20261120-070102".
pub fn stamp(seconds_since_epoch: u64) -> String {
    let date = utc_date(seconds_since_epoch).replace('-', "");
    let of_day = seconds_since_epoch % 86_400;
    format!("{date}-{:02}{:02}{:02}", of_day / 3_600, of_day % 3_600 / 60, of_day % 60)
}

fn no_such_profile(name: &str) -> String {
    format!("there is no profile named \"{name}\"")
}

/// Liest die Kennung eines Eintrags im Papierkorb: den Namen des Profils und
/// wann es gelöscht wurde, etwa `tea-time-20261120-070102` oder, bei zwei
/// Löschungen in derselben Sekunde, `tea-time-20261120-070102-2`.
///
/// Die Kennung ist der Dateiname ohne Endung. Sie kommt von außen (aus der
/// Maske oder vom Terminal); alles, was nicht so aussieht, wird abgelehnt,
/// damit niemand damit auf eine Datei außerhalb des Papierkorbs zeigt.
fn trashed(id: &str) -> Result<(String, String), String> {
    let not_one = || format!("{id:?} is no entry of the trash");
    let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_';
    if id.is_empty() || id.len() > 80 || !id.chars().all(allowed) {
        return Err(not_one());
    }
    let digits = |part: &str, len: usize| part.len() == len && part.chars().all(|c| c.is_ascii_digit());
    let parts: Vec<&str> = id.split('-').collect();
    let n = parts.len();
    // Mit Zähler am Ende, oder ohne.
    let stamp_at = if n >= 4
        && digits(parts[n - 3], 8)
        && digits(parts[n - 2], 6)
        && parts[n - 1].chars().all(|c| c.is_ascii_digit())
        && !digits(parts[n - 1], 6)
    {
        n - 3
    } else if n >= 3 && digits(parts[n - 2], 8) && digits(parts[n - 1], 6) {
        n - 2
    } else {
        return Err(not_one());
    };
    let name = parts[..stamp_at].join("-");
    valid_name(&name).map_err(|_| not_one())?;
    let (date, time) = (parts[stamp_at], parts[stamp_at + 1]);
    let deleted = format!(
        "{}-{}-{} {}:{}:{} UTC",
        &date[..4],
        &date[4..6],
        &date[6..],
        &time[..2],
        &time[2..4],
        &time[4..]
    );
    Ok((name, deleted))
}

/// Schreibt eine Datei so, dass ein Absturz mittendrin keine halbe Datei unter
/// dem richtigen Namen hinterlässt: erst daneben schreiben, dann umbenennen.
fn write_whole(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut tmp = path.to_owned().into_os_string();
    tmp.push(".tmp");
    let done = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, path));
    if done.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    done.map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Der Profilordner.
#[derive(Debug, Clone)]
pub struct Folder {
    dir: PathBuf,
}

impl Folder {
    pub fn at(dir: PathBuf) -> Folder {
        Folder { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn config_path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.toml"))
    }

    pub fn state_path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.state.json"))
    }

    /// Alle Profile, als das JSON, das `profile list --json` ausgibt.
    pub fn list(&self) -> Result<Value, String> {
        let names = self.names()?;
        let running = lock::running(&self.dir, BOT).map(|r| r.profiles).unwrap_or_default();
        let profiles: Vec<Value> =
            names.iter().map(|name| self.entry(name, running.contains(name))).collect();
        Ok(json!({ "dir": self.dir.to_string_lossy(), "profiles": profiles }))
    }

    /// Die Namen aller gespeicherten Profile, sortiert.
    pub fn names(&self) -> Result<Vec<String>, String> {
        let mut names = Vec::new();
        match std::fs::read_dir(&self.dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let file = entry.file_name().to_string_lossy().into_owned();
                    // Ein Profil ist eine Datei `<name>.toml` mit gültigem Namen.
                    // Alles andere im Ordner (Zustände, Papierkorb, Sperren,
                    // fremde Dateien) ist keines.
                    let Some(name) = file.strip_suffix(".toml") else { continue };
                    if valid_name(name).is_ok() && entry.path().is_file() {
                        names.push(name.to_owned());
                    }
                }
            }
            // Den Ordner gibt es erst, wenn das erste Profil gespeichert ist.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("cannot read {}: {e}", self.dir.display())),
        }
        names.sort();
        Ok(names)
    }

    /// Ein Eintrag der Liste. Ein kaputtes Profil bricht die Liste nicht ab:
    /// es erscheint mit `ok: false` und dem Grund.
    fn entry(&self, name: &str, running: bool) -> Value {
        let path = self.config_path(name);
        let mut entry = json!({
            "name": name,
            "path": path.to_string_lossy(),
            "ok": false,
            "error": null,
            "usage": null,
            "running": running,
        });
        let config = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))
            .and_then(|text| Config::parse(&text));
        let config = match config {
            Ok(config) => config,
            Err(why) => {
                entry["error"] = json!(why);
                return entry;
            }
        };

        // Dieselben Felder wie im bearbeitbaren Dokument, nicht noch einmal
        // von Hand aufgezählt. Dazu je Platz, ob dieser Fisch ihn bedient:
        // das ist berechnet und steht deshalb nur hier, nicht im Dokument.
        let document = Document::from(&config).with_served(&config);
        if let Ok(Value::Object(fields)) = serde_json::to_value(document) {
            for (key, value) in fields {
                entry[key.as_str()] = value;
            }
        }
        entry["ok"] = json!(true);
        // Wohin die Sätze gehen: der Dienst, wie die Bitte um Zustimmung ihn
        // nennt, und ob er auf diesem Rechner, im eigenen Netz oder draußen
        // steht. Berechnet, deshalb nur hier und nicht im Dokument.
        entry["ai"] = json!({
            "service": service_name(&config.service_address()),
            "place": config.ai_place().id(),
        });

        let state = self.state_path(name);
        if state.exists() {
            match Store::open(&state) {
                Ok(store) => {
                    let tally = store.tally();
                    // Ohne Datum wurde noch nie gezählt.
                    if !tally.day.is_empty() {
                        entry["usage"] = json!({
                            "day": tally.day,
                            "translation_tokens": tally.translation_tokens,
                            "allowed_today": tally.allowed_today(&config.limits),
                        });
                    }
                }
                // Mit beschädigtem Zustand startet der Fisch nicht. Das soll
                // die Liste sagen, statt es bis zum Start zu verschweigen.
                Err(why) => {
                    entry["ok"] = json!(false);
                    entry["error"] = json!(why);
                }
            }
        }
        entry
    }

    /// Das bearbeitbare Dokument eines Profils.
    pub fn show(&self, name: &str) -> Result<Document, String> {
        valid_name(name)?;
        let text = self.read_config(name)?;
        let mut document = Document::from(&Config::parse(&text)?);
        document.name = Some(name.to_owned());
        Ok(document)
    }

    /// Der Text der Einstellungsdatei eines Profils.
    pub fn read_config(&self, name: &str) -> Result<String, String> {
        let path = self.config_path(name);
        std::fs::read_to_string(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                no_such_profile(name)
            } else {
                format!("cannot read {}: {e}", path.display())
            }
        })
    }

    /// Prüft ein Dokument und schreibt es als `<name>.toml`. Erst bei
    /// bestandener Prüfung wird geschrieben; der Zustand bleibt unberührt.
    ///
    /// Der Name kommt vom Befehl. Ein `name` im Dokument wird überlesen, damit
    /// „unter neuem Namen speichern" dasselbe Dokument schicken kann.
    pub fn save(&self, name: &str, json: &str) -> Result<(), String> {
        valid_name(name)?;
        // Eine Wahrheit: das Dokument wird zu TOML und dann gelesen wie jede
        // Datei. Was der Fisch beim Start ablehnen würde, wird nie gespeichert.
        let (toml, _) = Document::from_json(json)?.check()?;
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| format!("cannot create {}: {e}", self.dir.display()))?;
        write_whole(&self.config_path(name), toml.as_bytes())
    }

    /// Verschiebt ein Profil samt Zustand in den Papierkorb. Gelöscht wird
    /// nichts. Liefert den Ort der verschobenen Einstellungsdatei.
    pub fn delete(&self, name: &str, stamp: &str) -> Result<PathBuf, String> {
        valid_name(name)?;
        let config = self.config_path(name);
        if !config.is_file() {
            return Err(no_such_profile(name));
        }
        // Einem laufenden Fisch die Dateien wegzunehmen hieße, dass er seinen
        // Zustand an einen Ort schreibt, den es nicht mehr gibt.
        if lock::running(&self.dir, BOT).is_some_and(|r| r.profiles.iter().any(|p| p == name)) {
            return Err(format!("profile \"{name}\" is running; stop it before deleting it"));
        }

        let trash = self.trash_dir();
        std::fs::create_dir_all(&trash)
            .map_err(|e| format!("cannot create {}: {e}", trash.display()))?;
        // Zwei Löschungen in derselben Sekunde dürfen einander nicht überschreiben.
        let mut base = format!("{name}-{stamp}");
        let mut n = 1;
        while trash.join(format!("{base}.toml")).exists()
            || trash.join(format!("{base}.state.json")).exists()
        {
            n += 1;
            base = format!("{name}-{stamp}-{n}");
        }

        let moved = trash.join(format!("{base}.toml"));
        std::fs::rename(&config, &moved)
            .map_err(|e| format!("cannot move {}: {e}", config.display()))?;
        let state = self.state_path(name);
        if state.exists() {
            std::fs::rename(&state, trash.join(format!("{base}.state.json")))
                .map_err(|e| format!("cannot move {}: {e}", state.display()))?;
        }
        Ok(moved)
    }

    fn trash_dir(&self) -> PathBuf {
        self.dir.join("trash")
    }

    /// Was im Papierkorb liegt, als das JSON, das `profile trash --json`
    /// ausgibt: das zuletzt Gelöschte zuerst.
    ///
    /// Nemesus am 23.11.2026: „gibt es auch einen Weg, das gelöschte Profil
    /// vom Papierkorb zurück zu bringen oder endgültig zu löschen?" Bis dahin
    /// ging beides nur von Hand im Ordner.
    pub fn trash(&self) -> Result<Value, String> {
        let trash = self.trash_dir();
        let mut found: Vec<(String, String, String, bool)> = Vec::new();
        match std::fs::read_dir(&trash) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let file = entry.file_name().to_string_lossy().into_owned();
                    let Some(id) = file.strip_suffix(".toml") else { continue };
                    // Was nicht aussieht wie von `delete` abgelegt, ist kein
                    // Eintrag: die Maske soll nichts anbieten, was das
                    // Programm danach nicht zurückholen kann.
                    let Ok((name, deleted)) = trashed(id) else { continue };
                    let state = trash.join(format!("{id}.state.json")).is_file();
                    found.push((id.to_owned(), name, deleted, state));
                }
            }
            // Den Ordner gibt es erst, wenn das erste Profil gelöscht ist.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("cannot read {}: {e}", trash.display())),
        }
        // Nach dem Zeitpunkt, das Neueste zuerst; bei gleichem nach Namen.
        found.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
        let deleted: Vec<Value> = found
            .into_iter()
            .map(|(id, name, deleted, state)| {
                json!({ "id": id, "name": name, "deleted": deleted, "state": state })
            })
            .collect();
        Ok(json!({ "dir": trash.to_string_lossy(), "deleted": deleted }))
    }

    /// Holt ein gelöschtes Profil samt Zustand aus dem Papierkorb zurück,
    /// unter seinem alten Namen oder unter `name`. Liefert den Namen, unter
    /// dem es jetzt steht.
    ///
    /// Es wird verschoben, nicht kopiert: danach liegt es nicht mehr im
    /// Papierkorb. Ein Profil, das es unter dem Namen schon gibt, wird nie
    /// ersetzt.
    pub fn restore(&self, id: &str, name: Option<&str>) -> Result<String, String> {
        let (old_name, _) = trashed(id)?;
        let name = name.unwrap_or(&old_name);
        valid_name(name)?;
        let trash = self.trash_dir();
        let config = trash.join(format!("{id}.toml"));
        if !config.is_file() {
            return Err(format!("there is nothing named \"{id}\" in the trash"));
        }
        if self.config_path(name).exists() || self.state_path(name).exists() {
            return Err(format!(
                "a profile named \"{name}\" already exists; put this one back under another name"
            ));
        }
        // Erst der Zustand, dann die Einstellungen: ein Profil gilt erst als
        // da, wenn seine Einstellungsdatei da ist.
        let state = trash.join(format!("{id}.state.json"));
        if state.is_file() {
            std::fs::rename(&state, self.state_path(name))
                .map_err(|e| format!("cannot move {}: {e}", state.display()))?;
        }
        std::fs::rename(&config, self.config_path(name))
            .map_err(|e| format!("cannot move {}: {e}", config.display()))?;
        Ok(name.to_owned())
    }

    /// Löscht ein Profil im Papierkorb endgültig: seine Einstellungen und
    /// was der Fisch sich dazu gemerkt hat. Das lässt sich nicht
    /// zurücknehmen.
    ///
    /// Benannte Grenze: das Protokoll des Profils im Ordner `logs` bleibt
    /// liegen. Es gehört zum Namen, nicht zu dieser einen Löschung, und die
    /// Maske führt es weiter, wenn es wieder ein Profil dieses Namens gibt.
    pub fn erase(&self, id: &str) -> Result<(), String> {
        trashed(id)?;
        let trash = self.trash_dir();
        let config = trash.join(format!("{id}.toml"));
        if !config.is_file() {
            return Err(format!("there is nothing named \"{id}\" in the trash"));
        }
        let state = trash.join(format!("{id}.state.json"));
        if state.is_file() {
            std::fs::remove_file(&state)
                .map_err(|e| format!("cannot erase {}: {e}", state.display()))?;
        }
        std::fs::remove_file(&config).map_err(|e| format!("cannot erase {}: {e}", config.display()))
    }

    /// Übernimmt eine vorhandene Einstellungsdatei Byte für Byte als Profil,
    /// samt der Zustandsdatei daneben, falls es eine gibt.
    ///
    /// Byte für Byte, nicht gelesen und neu geschrieben: so bleiben die
    /// Kommentare der Datei erhalten, bis sie zum ersten Mal aus der Maske
    /// gespeichert wird. Eine Datei, die der Fisch nicht lesen kann, wird
    /// trotzdem übernommen und erscheint in der Liste mit ihrem Fehler.
    pub fn import(&self, source: &Path, name: &str) -> Result<(), String> {
        valid_name(name)?;
        if self.config_path(name).exists() || self.state_path(name).exists() {
            return Err(format!("a profile named \"{name}\" already exists"));
        }
        let settings = std::fs::read(source)
            .map_err(|e| format!("cannot read {}: {e}", source.display()))?;
        // Der Zustand liegt neben den Einstellungen, wie `run --config` ihn ablegt.
        let source_state = source.with_extension("state.json");
        let state = if source_state.is_file() {
            Some(
                std::fs::read(&source_state)
                    .map_err(|e| format!("cannot read {}: {e}", source_state.display()))?,
            )
        } else {
            None
        };

        std::fs::create_dir_all(&self.dir)
            .map_err(|e| format!("cannot create {}: {e}", self.dir.display()))?;
        // Erst der Zustand, dann die Einstellungen: ein Profil gilt erst als
        // da, wenn seine Einstellungsdatei da ist.
        if let Some(state) = state {
            write_whole(&self.state_path(name), &state)?;
        }
        write_whole(&self.config_path(name), &settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock::Running;

    fn temp(name: &str) -> Folder {
        let dir =
            std::env::temp_dir().join(format!("rusty-fish-prof-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Folder::at(dir.join("profiles"))
    }

    fn clean(folder: &Folder) {
        let _ = std::fs::remove_dir_all(folder.dir().parent().unwrap());
    }

    const DOC: &str = r#"{
        "name": "ignored",
        "model": "some-model",
        "base_url": null,
        "host": "100",
        "ai_names": ["Fish", "Фиш"],
        "limits": {"translation_tokens_per_day": 5000, "questions_per_guest_per_day": 2},
        "rooms": [{"pass_through": ["English"], "context_lines": 6, "seats": [
            {"channel": "111", "lang": "German", "gender": "male"},
            {"channel": "222", "lang": "Russian", "gender": "female"}
        ]}],
        "people": [{"id": "100", "gender": "male"}]
    }"#;

    #[test]
    fn the_list_says_where_the_ai_of_a_profile_reads() {
        let f = temp("ai-place");
        f.save("outside", DOC).unwrap();
        let own = DOC.replace(
            "\"base_url\": null,",
            "\"provider\": \"custom\", \"base_url\": \"http://localhost:11434/v1\",",
        );
        f.save("own", &own).unwrap();

        let v = f.list().unwrap();
        let by_name = |name: &str| {
            v["profiles"].as_array().unwrap().iter().find(|p| p["name"] == name).unwrap().clone()
        };
        // Der Dienst heißt, wie die Bitte um Zustimmung ihn nennt.
        assert_eq!(by_name("outside")["ai"], json!({"service": "api.openai.com", "place": "outside"}));
        assert_eq!(
            by_name("own")["ai"],
            json!({"service": "localhost:11434", "place": "this_machine"})
        );
        clean(&f);
    }

    // --- Namen ---------------------------------------------------------

    #[test]
    fn names_that_make_safe_file_names_pass() {
        for name in ["tea", "tea-time_2", "0", &"a".repeat(40)] {
            assert!(valid_name(name).is_ok(), "{name:?} should pass");
        }
    }

    #[test]
    fn other_names_are_refused_with_the_rule_spelled_out() {
        let long = "a".repeat(41);
        for name in ["", "Tea", "tea time", "tée", "a/b", "a\\b", "..", ".hidden", "tea.toml", &long] {
            let err = valid_name(name).expect_err(&format!("{name:?} should be refused"));
            assert!(err.contains("a-z"), "the message should name the rule: {err}");
        }
    }

    #[test]
    fn names_windows_keeps_for_devices_are_refused() {
        for name in ["con", "nul", "prn", "aux", "com1", "lpt9"] {
            assert!(valid_name(name).is_err(), "{name:?} should be refused");
        }
        assert!(valid_name("console").is_ok());
        assert!(valid_name("com10").is_ok());
    }

    // --- Ordner --------------------------------------------------------

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| pairs.iter().find(|(k, _)| *k == key).map(|(_, v)| (*v).to_owned())
    }

    #[test]
    fn the_folder_follows_each_systems_custom() {
        let win = env(&[("APPDATA", "C:\\Users\\x\\AppData\\Roaming"), ("HOME", "/ignored")]);
        assert_eq!(
            home(&win, Os::Windows).unwrap(),
            PathBuf::from("C:\\Users\\x\\AppData\\Roaming").join("rusty-fish")
        );

        let mac = env(&[("HOME", "/Users/x")]);
        assert_eq!(
            home(&mac, Os::MacOs).unwrap(),
            PathBuf::from("/Users/x").join("Library").join("Application Support").join("rusty-fish")
        );

        let xdg = env(&[("XDG_CONFIG_HOME", "/cfg"), ("HOME", "/home/x")]);
        assert_eq!(home(&xdg, Os::Other).unwrap(), PathBuf::from("/cfg").join("rusty-fish"));

        let plain = env(&[("HOME", "/home/x")]);
        assert_eq!(
            home(&plain, Os::Other).unwrap(),
            PathBuf::from("/home/x").join(".config").join("rusty-fish")
        );
    }

    #[test]
    fn an_explicit_home_replaces_the_folder_everywhere() {
        let set = env(&[("RUSTY_FISH_HOME", "/elsewhere"), ("APPDATA", "C:\\x"), ("HOME", "/h")]);
        for os in [Os::Windows, Os::MacOs, Os::Other] {
            assert_eq!(home(&set, os).unwrap(), PathBuf::from("/elsewhere"));
        }
        // Leer gesetzt zählt als nicht gesetzt.
        let empty = env(&[("RUSTY_FISH_HOME", " "), ("HOME", "/h")]);
        assert_eq!(home(&empty, Os::Other).unwrap(), PathBuf::from("/h").join(".config").join("rusty-fish"));
    }

    #[test]
    fn without_any_hint_the_folder_is_an_error_not_a_guess() {
        let nothing = env(&[]);
        for os in [Os::Windows, Os::MacOs, Os::Other] {
            assert!(home(&nothing, os).is_err());
        }
    }

    #[test]
    fn stamps_sort_by_time_and_fit_in_a_file_name() {
        assert_eq!(stamp(0), "19700101-000000");
        // 20.11.2026, 07:01:02 UTC
        assert_eq!(stamp(1_795_158_062), "20261120-070102");
    }

    // --- Speichern und Zeigen -------------------------------------------

    #[test]
    fn a_saved_profile_can_be_shown_again() {
        let f = temp("saveshow");
        f.save("tea", DOC).unwrap();
        assert!(f.config_path("tea").exists());

        let doc = f.show("tea").unwrap();
        // Der Name kommt vom Befehl, nicht aus dem Dokument.
        assert_eq!(doc.name.as_deref(), Some("tea"));
        assert_eq!(doc.model, "some-model");
        assert_eq!(doc.ai_names, ["Fish", "Фиш"]);
        assert_eq!(doc.limits.translation_tokens_per_day, 5000);
        assert_eq!(doc.rooms[0].seats[1].gender.as_deref(), Some("female"));

        // Was auf der Platte liegt, liest der Fisch wie jede Einstellungsdatei.
        let text = std::fs::read_to_string(f.config_path("tea")).unwrap();
        Config::parse(&text).unwrap();
        clean(&f);
    }

    #[test]
    fn saving_leaves_the_state_alone() {
        let f = temp("state");
        f.save("tea", DOC).unwrap();
        let state = br#"{"consented":["7"],"last_seen":{"111":50}}"#;
        std::fs::write(f.state_path("tea"), state).unwrap();
        f.save("tea", &DOC.replace("some-model", "another-model")).unwrap();
        assert_eq!(std::fs::read(f.state_path("tea")).unwrap(), state);
        assert_eq!(f.show("tea").unwrap().model, "another-model");
        clean(&f);
    }

    #[test]
    fn a_document_that_would_not_start_is_not_written() {
        let f = temp("refuse");
        // Neu: es entsteht keine Datei.
        let err = f.save("tea", &DOC.replace("\"host\": \"100\"", "\"host\": \"me\"")).unwrap_err();
        assert!(err.contains("host"), "got {err}");
        assert!(!f.config_path("tea").exists());

        // Vorhanden: die alte Fassung bleibt Byte für Byte stehen.
        f.save("tea", DOC).unwrap();
        let before = std::fs::read(f.config_path("tea")).unwrap();
        assert!(f.save("tea", "{ not json").is_err());
        assert!(f.save("tea", &DOC.replace("\"222\"", "\"111\"")).is_err());
        assert_eq!(std::fs::read(f.config_path("tea")).unwrap(), before);
        // Es bleibt auch keine halbe Datei daneben liegen.
        let leftovers: Vec<_> = std::fs::read_dir(f.dir())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n != "tea.toml")
            .collect();
        assert!(leftovers.is_empty(), "left behind: {leftovers:?}");
        clean(&f);
    }

    #[test]
    fn a_bad_name_is_refused_before_anything_is_touched() {
        let f = temp("badname");
        assert!(f.save("Tea Time", DOC).is_err());
        assert!(f.show("../tea").is_err());
        assert!(f.delete("a/b", "x").is_err());
        assert!(!f.dir().exists(), "nothing may be created for a refused name");
        clean(&f);
    }

    #[test]
    fn showing_a_missing_profile_says_which_one() {
        let f = temp("missing");
        let err = f.show("tea").unwrap_err();
        assert!(err.contains("tea"), "got {err}");
        clean(&f);
    }

    // --- Liste -----------------------------------------------------------

    #[test]
    fn an_empty_or_missing_folder_lists_nothing() {
        let f = temp("empty");
        let v = f.list().unwrap();
        assert_eq!(v["profiles"].as_array().unwrap().len(), 0);
        assert_eq!(v["dir"], f.dir().to_string_lossy().as_ref());
        clean(&f);
    }

    #[test]
    fn the_list_shows_sound_and_broken_profiles_side_by_side() {
        let f = temp("list");
        f.save("tea", DOC).unwrap();
        f.save("coffee", &DOC.replace("some-model", "other-model")).unwrap();
        std::fs::write(f.config_path("broken"), "model = \"m\"\nhots = \"1\"\n").unwrap();
        // Was kein Profil ist, wird übergangen.
        std::fs::write(f.dir().join("notes.txt"), "x").unwrap();
        std::fs::write(f.dir().join("Bad Name.toml"), "x").unwrap();

        let v = f.list().unwrap();
        let profiles = v["profiles"].as_array().unwrap();
        let names: Vec<&str> = profiles.iter().map(|p| p["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["broken", "coffee", "tea"]);

        let broken = &profiles[0];
        assert_eq!(broken["ok"], false);
        assert!(broken["error"].as_str().unwrap().contains("hots"));
        assert_eq!(broken["running"], false);
        assert!(broken["usage"].is_null());

        let tea = &profiles[2];
        assert_eq!(tea["ok"], true);
        assert!(tea["error"].is_null());
        assert_eq!(tea["path"], f.config_path("tea").to_string_lossy().as_ref());
        assert_eq!(tea["model"], "some-model");
        assert!(tea["base_url"].is_null());
        assert_eq!(tea["host"], "100");
        assert_eq!(tea["ai_names"][1], "Фиш");
        assert_eq!(tea["limits"]["translation_tokens_per_day"], 5000);
        assert_eq!(tea["limits"]["questions_per_guest_per_day"], 2);
        assert_eq!(tea["rooms"][0]["context_lines"], 6);
        assert_eq!(tea["rooms"][0]["pass_through"][0], "English");
        assert_eq!(tea["rooms"][0]["seats"][0]["channel"], "111");
        assert_eq!(tea["rooms"][0]["seats"][1]["gender"], "female");
        assert_eq!(tea["people"][0]["discord"], "100");
        assert_eq!(tea["people"][0]["gender"], "male");
        // Ohne Zustandsdatei gibt es keinen Verbrauch zu zeigen.
        assert!(tea["usage"].is_null());
        assert_eq!(tea["running"], false);
        clean(&f);
    }

    #[test]
    fn the_list_shows_todays_use_from_the_state() {
        let f = temp("usage");
        f.save("tea", DOC).unwrap();
        std::fs::write(
            f.state_path("tea"),
            r#"{"day":"2026-11-20","translation_tokens":1234,"raised_tokens":5000}"#,
        )
        .unwrap();
        let v = f.list().unwrap();
        let usage = &v["profiles"][0]["usage"];
        assert_eq!(usage["day"], "2026-11-20");
        assert_eq!(usage["translation_tokens"], 1234);
        // Tagesbetrag 5000 plus 5000 nachgelegt.
        assert_eq!(usage["allowed_today"], 10000);
        clean(&f);
    }

    #[test]
    fn a_damaged_state_does_not_hide_the_profile() {
        let f = temp("badstate");
        f.save("tea", DOC).unwrap();
        std::fs::write(f.state_path("tea"), "{ not json").unwrap();
        let v = f.list().unwrap();
        let tea = &v["profiles"][0];
        assert_eq!(tea["ok"], false);
        assert!(tea["error"].as_str().unwrap().contains("damaged"));
        // Die Einstellungen selbst sind lesbar und bleiben sichtbar.
        assert_eq!(tea["model"], "some-model");
        clean(&f);
    }

    #[test]
    fn the_list_marks_the_profile_that_is_running() {
        let f = temp("running");
        f.save("tea", DOC).unwrap();
        f.save("coffee", DOC).unwrap();
        let who = Running { profiles: vec!["tea".to_owned()], pid: 1, started: 1 };
        let held = lock::acquire(f.dir(), BOT, &who).unwrap();
        let v = f.list().unwrap();
        assert_eq!(v["profiles"][0]["name"], "coffee");
        assert_eq!(v["profiles"][0]["running"], false);
        assert_eq!(v["profiles"][1]["running"], true);
        drop(held);
        assert_eq!(f.list().unwrap()["profiles"][1]["running"], false);
        clean(&f);
    }

    // --- Tische, Muttersprachen, Einladungen (seit dem 20.11.2026) ---------

    /// Gastgeber 42 mit eigenem Tisch, 9 eingeladen, 11 mit eigenem Fisch.
    const TABLES: &str = r#"{
        "model": "some-model",
        "host": "42",
        "rooms": [{"pass_through": ["English"], "context_lines": 2, "seats": [
            {"channel": "7", "lang": "German", "owner": "42"},
            {"channel": "8", "lang": "Russian", "owner": "9"},
            {"channel": "10", "lang": "Korean", "owner": "11"}
        ]}],
        "people": [
            {"id": "42", "gender": "male", "lang": "German"},
            {"id": "9", "lang": "Русский", "neutral": ["English"], "invited": true},
            {"id": "11", "lang": "Korean"}
        ]
    }"#;

    #[test]
    fn the_list_says_for_every_table_whether_the_fish_serves_it() {
        let f = temp("served");
        f.save("tea", TABLES).unwrap();
        let v = f.list().unwrap();
        let tea = &v["profiles"][0];
        assert_eq!(tea["ok"], true, "got {tea}");
        let seats = tea["rooms"][0]["seats"].as_array().unwrap();
        let served: Vec<&Value> = seats.iter().map(|s| &s["served"]).collect();
        assert_eq!(served, [&json!(true), &json!(true), &json!(false)]);
        let owners: Vec<&Value> = seats.iter().map(|s| &s["owner"]).collect();
        assert_eq!(owners, [&json!("42"), &json!("9"), &json!("11")]);
        // Die Menschen stehen da, wie sie eingetragen wurden, mit allen Feldern.
        assert_eq!(
            tea["people"],
            json!([
                {"name": null, "discord": "42", "matrix": null, "gender": "male", "lang": "German", "neutral": null, "invited": false},
                {"name": null, "discord": "9", "matrix": null, "gender": null, "lang": "Русский", "neutral": ["English"], "invited": true},
                {"name": null, "discord": "11", "matrix": null, "gender": null, "lang": "Korean", "neutral": null, "invited": false},
            ])
        );
        clean(&f);
    }

    #[test]
    fn showing_a_profile_gives_the_new_fields_but_nothing_computed() {
        let f = temp("showtables");
        f.save("tea", TABLES).unwrap();
        let v = serde_json::to_value(f.show("tea").unwrap()).unwrap();
        let seat = &v["rooms"][0]["seats"][1];
        assert_eq!(seat["owner"], "9");
        // Am Platz steht, was eingetragen wurde, nicht die Muttersprache des Besitzers.
        assert_eq!(seat["lang"], "Russian");
        assert!(!seat.as_object().unwrap().contains_key("served"), "only the list says so");
        assert_eq!(v["people"][1]["invited"], true);
        assert_eq!(v["people"][1]["neutral"], json!(["English"]));
        // Was `show` ausgibt, nimmt `save` unverändert wieder an.
        let before = std::fs::read(f.config_path("tea")).unwrap();
        f.save("tea", &v.to_string()).unwrap();
        assert_eq!(std::fs::read(f.config_path("tea")).unwrap(), before);
        clean(&f);
    }

    #[test]
    fn an_entry_of_the_list_can_be_saved_again_without_its_computed_part() {
        // `served` ist berechnet. Kommt es zurück, wird es überlesen. Die
        // übrigen Felder der Liste (Pfad, Verbrauch, ...) gehören nicht zum
        // Dokument und werden weiter laut abgelehnt.
        let f = temp("listback");
        f.save("tea", TABLES).unwrap();
        let before = std::fs::read(f.config_path("tea")).unwrap();
        let entry = f.list().unwrap()["profiles"][0].clone();
        let doc = json!({
            "model": entry["model"], "host": entry["host"],
            "rooms": entry["rooms"], "people": entry["people"],
        });
        assert_eq!(doc["rooms"][0]["seats"][2]["served"], false);
        f.save("tea", &doc.to_string()).unwrap();
        assert_eq!(std::fs::read(f.config_path("tea")).unwrap(), before);
        assert!(f.save("tea", &entry.to_string()).is_err());
        clean(&f);
    }

    #[test]
    fn a_profile_from_before_the_tables_reads_as_it_always_did() {
        let f = temp("oldprofile");
        std::fs::create_dir_all(f.dir()).unwrap();
        // So schrieb das Programm ein Profil vor dem 20.11.2026.
        let old = "provider = \"openai\"\nmodel = \"some-model\"\nhost = \"100\"\n\n[limits]\n\
                   translation_tokens_per_day = 5000\nquestions_per_guest_per_day = 2\n\n[[room]]\n\
                   pass_through = [\"English\"]\ncontext_lines = 6\n\n  [[room.seat]]\n  \
                   channel = \"111\"\n  lang = \"German\"\n  gender = \"male\"\n\n  [[room.seat]]\n  \
                   channel = \"222\"\n  lang = \"Russian\"\n  gender = \"female\"\n\n[[person]]\n\
                   id = \"100\"\ngender = \"male\"\n";
        std::fs::write(f.config_path("tea"), old).unwrap();

        let v = f.list().unwrap();
        let tea = &v["profiles"][0];
        assert_eq!(tea["ok"], true, "got {tea}");
        for seat in tea["rooms"][0]["seats"].as_array().unwrap() {
            assert!(seat["owner"].is_null());
            assert_eq!(seat["served"], true, "every table is served, as before");
        }
        assert_eq!(
            tea["people"],
            json!([{"name": null, "discord": "100", "matrix": null, "gender": "male", "lang": null, "neutral": null, "invited": false}])
        );

        let doc = f.show("tea").unwrap();
        assert_eq!(doc.rooms[0].seats[1].gender.as_deref(), Some("female"));
        assert_eq!(doc.rooms[0].seats[1].owner, None);
        // Wird es unverändert gespeichert, bedeutet es dasselbe wie zuvor.
        let before = Config::parse(old).unwrap();
        f.save("tea", &serde_json::to_string(&doc).unwrap()).unwrap();
        let after = Config::parse(&std::fs::read_to_string(f.config_path("tea")).unwrap()).unwrap();
        assert_eq!(after, before);
        clean(&f);
    }

    // --- Löschen ---------------------------------------------------------

    #[test]
    fn deleting_moves_profile_and_state_to_the_trash() {
        let f = temp("delete");
        f.save("tea", DOC).unwrap();
        std::fs::write(f.state_path("tea"), "{}").unwrap();
        let config_bytes = std::fs::read(f.config_path("tea")).unwrap();

        let moved = f.delete("tea", "20261120-070102").unwrap();
        assert!(!f.config_path("tea").exists());
        assert!(!f.state_path("tea").exists());
        let trash = f.dir().join("trash");
        assert_eq!(moved, trash.join("tea-20261120-070102.toml"));
        assert_eq!(std::fs::read(&moved).unwrap(), config_bytes);
        assert!(trash.join("tea-20261120-070102.state.json").exists());
        // Der Papierkorb ist kein Profil.
        assert_eq!(f.list().unwrap()["profiles"].as_array().unwrap().len(), 0);
        clean(&f);
    }

    // --- der Papierkorb, seit dem 23.11.2026 -------------------------------

    #[test]
    fn the_trash_lists_what_was_deleted_the_latest_first() {
        let f = temp("trash-list");
        assert_eq!(f.trash().unwrap()["deleted"], json!([]), "no trash folder yet");
        for name in ["tea", "tea-time"] {
            f.save(name, DOC).unwrap();
        }
        std::fs::write(f.state_path("tea"), "{}").unwrap();
        f.delete("tea", "20261120-070102").unwrap();
        f.delete("tea-time", "20261121-080000").unwrap();
        // Ein zweites Profil gleichen Namens, in derselben Sekunde gelöscht.
        f.save("tea", DOC).unwrap();
        f.delete("tea", "20261120-070102").unwrap();
        // Was nicht von `delete` stammt, ist kein Eintrag.
        std::fs::write(f.dir().join("trash").join("notes.toml"), "x").unwrap();
        std::fs::write(f.dir().join("trash").join("readme.txt"), "x").unwrap();

        let trash = f.trash().unwrap();
        assert_eq!(trash["dir"], f.dir().join("trash").to_string_lossy().into_owned());
        assert_eq!(
            trash["deleted"],
            json!([
                { "id": "tea-time-20261121-080000", "name": "tea-time",
                  "deleted": "2026-11-21 08:00:00 UTC", "state": false },
                { "id": "tea-20261120-070102", "name": "tea",
                  "deleted": "2026-11-20 07:01:02 UTC", "state": true },
                { "id": "tea-20261120-070102-2", "name": "tea",
                  "deleted": "2026-11-20 07:01:02 UTC", "state": false },
            ])
        );
        clean(&f);
    }

    #[test]
    fn a_deleted_profile_comes_back_with_its_state() {
        let f = temp("trash-restore");
        f.save("tea", DOC).unwrap();
        let state = br#"{"consented":["7"]}"#;
        std::fs::write(f.state_path("tea"), state).unwrap();
        let config = std::fs::read(f.config_path("tea")).unwrap();
        f.delete("tea", "20261120-070102").unwrap();

        assert_eq!(f.restore("tea-20261120-070102", None).unwrap(), "tea");
        assert_eq!(std::fs::read(f.config_path("tea")).unwrap(), config);
        assert_eq!(std::fs::read(f.state_path("tea")).unwrap(), state);
        // Verschoben, nicht kopiert: der Papierkorb ist danach leer.
        assert_eq!(f.trash().unwrap()["deleted"], json!([]));
        assert_eq!(f.names().unwrap(), ["tea"]);
        // Ein zweites Mal gibt es nichts zurückzuholen.
        let err = f.restore("tea-20261120-070102", None).unwrap_err();
        assert!(err.contains("nothing named \"tea-20261120-070102\" in the trash"), "got {err}");
        clean(&f);
    }

    #[test]
    fn putting_back_never_replaces_a_profile_that_is_there() {
        let f = temp("trash-taken");
        f.save("tea", DOC).unwrap();
        f.delete("tea", "20261120-070102").unwrap();
        // Inzwischen gibt es wieder ein Profil dieses Namens.
        f.save("tea", DOC).unwrap();
        let err = f.restore("tea-20261120-070102", None).unwrap_err();
        assert!(err.contains("a profile named \"tea\" already exists"), "got {err}");
        assert!(err.contains("under another name"), "the message says what to do: {err}");
        // Unter einem anderen Namen geht es.
        assert_eq!(f.restore("tea-20261120-070102", Some("tea-old")).unwrap(), "tea-old");
        assert_eq!(f.names().unwrap(), ["tea", "tea-old"]);
        // Ein Name, den das Programm nicht annimmt, auch hier nicht.
        f.delete("tea-old", "20261122-090000").unwrap();
        assert!(f.restore("tea-old-20261122-090000", Some("Tea Old")).is_err());
        clean(&f);
    }

    #[test]
    fn erasing_removes_settings_and_state_for_good() {
        let f = temp("trash-erase");
        f.save("tea", DOC).unwrap();
        std::fs::write(f.state_path("tea"), "{}").unwrap();
        f.save("probe", DOC).unwrap();
        f.delete("tea", "20261120-070102").unwrap();
        f.delete("probe", "20261120-070103").unwrap();

        f.erase("tea-20261120-070102").unwrap();
        let trash = f.dir().join("trash");
        assert!(!trash.join("tea-20261120-070102.toml").exists());
        assert!(!trash.join("tea-20261120-070102.state.json").exists());
        // Der andere Eintrag bleibt.
        let left = f.trash().unwrap();
        assert_eq!(left["deleted"].as_array().unwrap().len(), 1);
        assert_eq!(left["deleted"][0]["name"], "probe");
        let err = f.erase("tea-20261120-070102").unwrap_err();
        assert!(err.contains("nothing named"), "got {err}");
        clean(&f);
    }

    #[test]
    fn only_entries_of_the_trash_can_be_put_back_or_erased() {
        let f = temp("trash-ids");
        f.save("tea", DOC).unwrap();
        // Ein gespeichertes Profil ist kein Eintrag des Papierkorbs, und kein
        // Weg führt aus dem Ordner hinaus.
        for id in [
            "tea",
            "../tea",
            "..\\tea",
            "tea-20261120-070102/../../tea",
            "tea-2026112-070102",
            "-20261120-070102",
            "TEA-20261120-070102",
            "",
        ] {
            let err = f.erase(id).unwrap_err();
            assert!(err.contains("is no entry of the trash"), "{id:?}: {err}");
            assert!(f.restore(id, None).is_err(), "{id:?}");
        }
        assert!(f.config_path("tea").exists(), "nothing happened to the saved profile");
        clean(&f);
    }

    #[test]
    fn a_running_profile_is_not_deleted() {
        let f = temp("delrun");
        f.save("tea", DOC).unwrap();
        f.save("coffee", DOC).unwrap();
        let who = Running { profiles: vec!["tea".to_owned()], pid: 1, started: 1 };
        let held = lock::acquire(f.dir(), BOT, &who).unwrap();
        let err = f.delete("tea", "s").unwrap_err();
        assert!(err.contains("running"), "got {err}");
        assert!(f.config_path("tea").exists());
        // Ein anderes Profil darf währenddessen weg.
        f.delete("coffee", "s").unwrap();
        drop(held);
        f.delete("tea", "s").unwrap();
        clean(&f);
    }

    #[test]
    fn deleting_what_is_not_there_is_an_error() {
        let f = temp("delmissing");
        let err = f.delete("tea", "s").unwrap_err();
        assert!(err.contains("tea"), "got {err}");
        clean(&f);
    }

    // --- Übernehmen ------------------------------------------------------

    #[test]
    fn importing_copies_settings_and_state_byte_for_byte() {
        let f = temp("import");
        let outside = f.dir().parent().unwrap().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let source = outside.join("rusty-fish.toml");
        // Mit Kommentar und Windows-Zeilenenden: nichts davon darf sich ändern.
        let settings = "# mein Kommentar\r\nmodel = \"m\"\r\nhost = \"1\"\r\n\r\n[[room]]\r\n[[room.seat]]\r\nchannel = \"111\"\r\nlang = \"German\"\r\n[[room.seat]]\r\nchannel = \"222\"\r\nlang = \"Russian\"\r\n";
        std::fs::write(&source, settings).unwrap();
        std::fs::write(outside.join("rusty-fish.state.json"), r#"{"consented":["7"]}"#).unwrap();

        f.import(&source, "tea").unwrap();
        assert_eq!(std::fs::read(f.config_path("tea")).unwrap(), settings.as_bytes());
        assert_eq!(std::fs::read(f.state_path("tea")).unwrap(), br#"{"consented":["7"]}"#);
        // Die Quelle bleibt, wo sie ist.
        assert!(source.exists());
        assert_eq!(f.show("tea").unwrap().model, "m");
        clean(&f);
    }

    #[test]
    fn importing_without_a_state_brings_no_state() {
        let f = temp("importnostate");
        let outside = f.dir().parent().unwrap().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let source = outside.join("x.toml");
        std::fs::write(&source, "anything").unwrap();
        f.import(&source, "tea").unwrap();
        assert!(!f.state_path("tea").exists());
        clean(&f);
    }

    #[test]
    fn importing_refuses_a_taken_name_and_an_unreadable_file() {
        let f = temp("importrefuse");
        f.save("tea", DOC).unwrap();
        let before = std::fs::read(f.config_path("tea")).unwrap();
        let outside = f.dir().parent().unwrap().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let source = outside.join("x.toml");
        std::fs::write(&source, "other").unwrap();

        let err = f.import(&source, "tea").unwrap_err();
        assert!(err.contains("already"), "got {err}");
        assert_eq!(std::fs::read(f.config_path("tea")).unwrap(), before);

        let err = f.import(&outside.join("gibt-es-nicht.toml"), "new").unwrap_err();
        assert!(err.contains("gibt-es-nicht"), "got {err}");
        assert!(!f.config_path("new").exists());
        clean(&f);
    }
}
