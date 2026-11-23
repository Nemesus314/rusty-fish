//! Die Sprachen, die der Fisch beim Eintragen vorschlägt. Rein, ohne Netz.
//!
//! Nemesus am 24.11.2026: „Im Moment muss man die Sprachen manuell eingeben.
//! Das könnte Fehler verursachen. Besser wäre, wenn Vorschläge kommen würden,
//! je nachdem, was ich tippe. Beispielsweise ich tippe Ko und ergänzt wird
//! Koreanisch."
//!
//! Die Sprache eines Tisches bleibt ein freies Wort: jedes andere gilt
//! weiter, etwa eine Variante, die hier nicht steht. Die Liste hilft nur beim
//! Tippen. Sie steht im Programm und nicht in der Maske, weil das Programm an
//! diesen Namen etwas festmacht: die festen Texte des Fischs (Deutsch,
//! Englisch, Russisch) und die Sprachen, die eine Höflichkeitsstufe verlangen
//! (siehe [`crate::asks_for_a_level`]). Wer „Deutsch" tippt, bekommt
//! „German" vorgeschlagen, das Wort, das der Fisch kennt.
//!
//! Benannte Grenzen:
//! - Die Liste ist eine Auswahl, kein Verzeichnis aller Sprachen.
//! - Die anderen Namen sind der deutsche und, wo ich mir sicher bin, der in
//!   der Sprache selbst. Sie dienen nur dem Finden.

use crate::asks_for_a_level;

/// Eine Sprache, wie der Fisch sie vorschlägt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Language {
    /// Das Wort, das ins Profil kommt: der englische Name.
    pub name: &'static str,
    /// Andere Namen, unter denen jemand sie beim Tippen sucht.
    pub also: &'static [&'static str],
}

impl Language {
    /// Verlangt diese Sprache eine Höflichkeitsstufe? Dann nimmt der Fisch
    /// die alltägliche höfliche, solange über den Leser nichts eingetragen ist.
    pub fn asks_for_a_level(&self) -> bool {
        asks_for_a_level(self.name)
    }
}

const fn lang(name: &'static str, also: &'static [&'static str]) -> Language {
    Language { name, also }
}

/// Die Vorschläge, in der Reihenfolge, in der sie angeboten werden: nach dem
/// englischen Namen.
pub const LANGUAGES: &[Language] = &[
    lang("Afrikaans", &[]),
    lang("Albanian", &["Albanisch", "Shqip"]),
    lang("Arabic", &["Arabisch", "العربية"]),
    lang("Armenian", &["Armenisch", "Հայերեն"]),
    lang("Balinese", &["Balinesisch"]),
    lang("Bengali", &["Bengalisch", "Bangla", "বাংলা"]),
    lang("Brazilian Portuguese", &["Brasilianisch", "Português do Brasil"]),
    lang("Bulgarian", &["Bulgarisch", "Български"]),
    lang("Burmese", &["Birmanisch", "Myanmar", "မြန်မာ"]),
    lang("Cantonese", &["Kantonesisch", "廣東話", "粵語"]),
    lang("Catalan", &["Katalanisch", "Català"]),
    lang("Croatian", &["Kroatisch", "Hrvatski"]),
    lang("Czech", &["Tschechisch", "Čeština"]),
    lang("Danish", &["Dänisch", "Dansk"]),
    lang("Dutch", &["Niederländisch", "Holländisch", "Nederlands"]),
    lang("English", &["Englisch"]),
    lang("Esperanto", &[]),
    lang("Estonian", &["Estnisch", "Eesti"]),
    lang("European Portuguese", &["Português de Portugal"]),
    lang("Finnish", &["Finnisch", "Suomi"]),
    lang("French", &["Französisch", "Français"]),
    lang("Georgian", &["Georgisch", "ქართული"]),
    lang("German", &["Deutsch"]),
    lang("Greek", &["Griechisch", "Ελληνικά"]),
    lang("Gujarati", &["ગુજરાતી"]),
    lang("Hebrew", &["Hebräisch", "עברית"]),
    lang("Hindi", &["हिन्दी"]),
    lang("Hungarian", &["Ungarisch", "Magyar"]),
    lang("Icelandic", &["Isländisch", "Íslenska"]),
    lang("Indonesian", &["Indonesisch", "Bahasa Indonesia"]),
    lang("Italian", &["Italienisch", "Italiano"]),
    lang("Japanese", &["Japanisch", "日本語"]),
    lang("Javanese", &["Javanisch", "Basa Jawa"]),
    lang("Kannada", &["ಕನ್ನಡ"]),
    lang("Kazakh", &["Kasachisch", "Қазақша"]),
    lang("Khmer", &["Kambodschanisch", "Cambodian", "ខ្មែរ"]),
    lang("Korean", &["Koreanisch", "한국어"]),
    lang("Lao", &["Laotisch", "Laotian", "ລາວ"]),
    lang("Latvian", &["Lettisch", "Latviešu"]),
    lang("Lithuanian", &["Litauisch", "Lietuvių"]),
    lang("Malay", &["Malaiisch", "Bahasa Melayu"]),
    lang("Malayalam", &["മലയാളം"]),
    lang("Mandarin", &["Chinesisch", "Chinese", "Hochchinesisch", "中文", "普通话"]),
    lang("Marathi", &["मराठी"]),
    lang("Mongolian", &["Mongolisch", "Монгол"]),
    lang("Nepali", &["Nepalesisch", "नेपाली"]),
    lang("Norwegian", &["Norwegisch", "Norsk"]),
    lang("Persian", &["Persisch", "Farsi", "فارسی"]),
    lang("Polish", &["Polnisch", "Polski"]),
    lang("Portuguese", &["Portugiesisch", "Português"]),
    lang("Punjabi", &["Pandschabi", "Panjabi", "ਪੰਜਾਬੀ"]),
    lang("Romanian", &["Rumänisch", "Română"]),
    lang("Russian", &["Russisch", "Русский"]),
    lang("Serbian", &["Serbisch", "Српски"]),
    lang("Simplified Chinese", &["Vereinfachtes Chinesisch", "简体中文"]),
    lang("Sinhala", &["Singhalesisch", "Sinhalese", "සිංහල"]),
    lang("Slovak", &["Slowakisch", "Slovenčina"]),
    lang("Slovenian", &["Slowenisch", "Slovenščina"]),
    lang("Spanish", &["Spanisch", "Español"]),
    lang("Sundanese", &["Sundanesisch", "Basa Sunda"]),
    lang("Swahili", &["Suaheli", "Kiswahili"]),
    lang("Swedish", &["Schwedisch", "Svenska"]),
    lang("Tagalog", &["Filipino", "Philippinisch"]),
    lang("Tamil", &["Tamilisch", "தமிழ்"]),
    lang("Telugu", &["తెలుగు"]),
    lang("Thai", &["Thailändisch", "ไทย"]),
    lang("Tibetan", &["Tibetisch", "བོད་སྐད"]),
    lang("Traditional Chinese (Taiwan)", &["Traditionelles Chinesisch", "Taiwanesisch", "繁體中文"]),
    lang("Turkish", &["Türkisch", "Türkçe"]),
    lang("Ukrainian", &["Ukrainisch", "Українська"]),
    lang("Urdu", &["اردو"]),
    lang("Vietnamese", &["Vietnamesisch", "Tiếng Việt"]),
];
