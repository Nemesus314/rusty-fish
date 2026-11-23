//! Tests der Sprachliste, die der Fisch beim Eintragen vorschlägt.

use rusty_fish_core::languages::LANGUAGES;
use std::collections::BTreeSet;

fn named(name: &str) -> bool {
    LANGUAGES.iter().any(|language| language.name == name)
}

#[test]
fn the_list_holds_the_words_the_fish_itself_knows() {
    // Die festen Texte des Fischs gibt es in diesen drei Sprachen.
    for name in ["German", "English", "Russian"] {
        assert!(named(name), "{name}");
    }
    // Und die Varianten, die sich als freies Wort bewährt haben.
    for name in ["Brazilian Portuguese", "Traditional Chinese (Taiwan)", "Cantonese", "Mandarin"] {
        assert!(named(name), "{name}");
    }
}

#[test]
fn every_language_that_asks_for_a_level_is_on_it_and_says_so() {
    // Die 24 Sprachen der Tabelle, die Nemesus am 23.11.2026 gewählt hat.
    let levelled: Vec<&str> = LANGUAGES
        .iter()
        .filter(|language| language.asks_for_a_level())
        .map(|language| language.name)
        .collect();
    assert_eq!(
        levelled,
        [
            "Balinese", "Bengali", "Burmese", "Gujarati", "Hindi", "Japanese", "Javanese",
            "Kannada", "Khmer", "Korean", "Lao", "Malayalam", "Marathi", "Nepali", "Punjabi",
            "Sinhala", "Sundanese", "Tagalog", "Tamil", "Telugu", "Thai", "Tibetan", "Urdu",
            "Vietnamese",
        ]
    );
    // Mandarin und Kantonesisch brauchen keine Regel; gemessen am 24.11.2026.
    for name in ["Mandarin", "Cantonese", "Simplified Chinese", "German", "Russian", "Malay"] {
        let language = LANGUAGES.iter().find(|language| language.name == name).unwrap();
        assert!(!language.asks_for_a_level(), "{name}");
    }
}

#[test]
fn a_name_stands_for_one_language() {
    let mut seen = BTreeSet::new();
    for language in LANGUAGES {
        assert_eq!(language.name, language.name.trim());
        assert!(!language.name.is_empty());
        assert!(seen.insert(language.name.to_lowercase()), "{} is listed twice", language.name);
    }
    // Ein anderer Name zeigt auf keine zweite Sprache und wiederholt keinen
    // englischen Namen: sonst böte die Maske für ein Wort zwei an.
    let mut other = BTreeSet::new();
    for language in LANGUAGES {
        for also in language.also {
            assert_eq!(*also, also.trim());
            assert!(!seen.contains(&also.to_lowercase()), "{also} is also a name");
            assert!(other.insert(also.to_lowercase()), "{also} stands for two languages");
        }
    }
}

#[test]
fn the_list_is_sorted_by_name_as_it_is_shown() {
    let names: Vec<&str> = LANGUAGES.iter().map(|language| language.name).collect();
    let mut sorted = names.clone();
    sorted.sort_by_key(|name| name.to_lowercase());
    assert_eq!(names, sorted);
}

#[test]
fn a_german_or_native_name_leads_to_the_word_the_fish_knows() {
    let find = |typed: &str| {
        LANGUAGES
            .iter()
            .find(|language| language.also.contains(&typed))
            .map(|language| language.name)
    };
    assert_eq!(find("Deutsch"), Some("German"));
    assert_eq!(find("Koreanisch"), Some("Korean"));
    assert_eq!(find("한국어"), Some("Korean"));
    assert_eq!(find("Filipino"), Some("Tagalog"));
    assert_eq!(find("Thailändisch"), Some("Thai"));
}
