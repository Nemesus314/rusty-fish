//! Die Kennungen der Knöpfe, die der Fisch in Discord setzt. Rein, ohne Netz.
//!
//! Ein Klick kommt als Kennung des Knopfs zurück. Seit ein Fisch mehrere
//! Profile bedient, muss ein Knopf sagen können, zu welchem er gehört.
//!
//! - Der Knopf unter der Bitte um Zustimmung braucht das nicht: er steht in
//!   einem Kanal, und ein Kanal gehört zu genau einem Profil.
//! - Die Knöpfe am Hinweis zum leeren Kontingent stehen in einer
//!   Direktnachricht an den Gastgeber. Dort gibt es keinen Kanal eines Raums,
//!   deshalb trägt die Kennung den Namen des Profils.

/// Der Knopf unter der Bitte um Zustimmung.
pub const CONSENT: &str = "rusty-fish-consent";

const RAISE: &str = "rusty-fish-raise";
const KEEP: &str = "rusty-fish-keep";

/// Was der Gastgeber am Hinweis zum leeren Kontingent wählen kann.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Budget {
    /// Um einen Tagesbetrag erhöhen.
    Raise,
    /// Es dabei belassen.
    Keep,
}

/// Die Kennung eines Knopfs am Hinweis zum leeren Kontingent. Mit dem Namen
/// des Profils, wenn es einen hat: `rusty-fish-raise:tea-time`.
pub fn budget_id(choice: Budget, profile: Option<&str>) -> String {
    let base = match choice {
        Budget::Raise => RAISE,
        Budget::Keep => KEEP,
    };
    match profile {
        Some(name) => format!("{base}:{name}"),
        None => base.to_owned(),
    }
}

/// Liest die Kennung eines geklickten Knopfs: welche Wahl, und für welches
/// Profil. `None`, wenn es keiner der beiden Knöpfe ist.
///
/// Ohne Namen kommt der Knopf von einem Fisch, der mit einer einzelnen
/// Einstellungsdatei lief, oder von einem Hinweis aus der Zeit vor dem
/// 21.11.2026.
pub fn budget_of(custom_id: &str) -> Option<(Budget, Option<&str>)> {
    let (base, profile) = match custom_id.split_once(':') {
        Some((base, name)) if !name.is_empty() => (base, Some(name)),
        Some(_) => return None,
        None => (custom_id, None),
    };
    let choice = match base {
        RAISE => Budget::Raise,
        KEEP => Budget::Keep,
        _ => return None,
    };
    Some((choice, profile))
}
