//! Tests der Teile der Tür, die ohne Verbindung zu Matrix auskommen: wie eine
//! Zustellung aussieht, die Erwähnung des Fischs, die Zeichen, die bei Matrix
//! die Knöpfe ersetzen, und der Zugang, wie er in der Schlüsselablage liegt.

use rusty_fish_house::config::Door;
use rusty_fish_house::texts::{consent_text_at, invitation_at};
use rusty_fish_matrix::mention::{mention_as_call, mention_question};
use rusty_fish_matrix::render::{
    MESSAGE_LIMIT, escape, plain, render_in, shown_name, without_quote,
};
use rusty_fish_matrix::session::{Access, user_id};
use rusty_fish_matrix::signs::{Ask, Budget, CONSENT_KEY, FIELD, notice, reaction};
use rusty_fish_room::tongue::Tongue;
use rusty_fish_room::{BUDGET_USED_UP, Delivery, Mark, Reason, Sender};
use serde_json::json;

const ALL: [Tongue; 3] = [Tongue::German, Tongue::English, Tongue::Russian];

fn delivery(text: &str, mark: Mark) -> Delivery {
    Delivery {
        channel: "!table:example.org".to_owned(),
        text: text.to_owned(),
        mark,
        from: Sender::Writer,
    }
}

// --- wie eine Zustellung aussieht ----------------------------------------------

#[test]
fn a_delivery_carries_the_name_of_who_wrote_it() {
    let parts = render_in(&delivery("Guten Morgen", Mark::Translated), Tongue::German, "Robin");
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].body, "Robin: Guten Morgen");
    assert_eq!(parts[0].html, "<strong>Robin</strong>: Guten Morgen");
}

#[test]
fn an_untranslated_delivery_says_so_in_the_language_of_its_table() {
    let d = delivery("Привет", Mark::Untranslated(Reason::NoConsent));
    let de = &render_in(&d, Tongue::German, "Robin")[0];
    assert!(de.body.starts_with("Robin: Привет\n(nicht übersetzt: "), "got {}", de.body);
    assert!(de.html.contains("<br><em>nicht übersetzt: "), "got {}", de.html);
    let en = &render_in(&d, Tongue::English, "Robin")[0];
    assert!(en.body.contains("\n(not translated: the writer has not agreed"), "got {}", en.body);
    let ru = &render_in(&d, Tongue::Russian, "Robin")[0];
    assert!(ru.body.contains("\n(не переведено: "), "got {}", ru.body);

    // Das verbrauchte Kontingent hat seinen festen Wortlaut.
    let used = delivery("Hallo", Mark::Untranslated(Reason::Failed(BUDGET_USED_UP.to_owned())));
    assert!(render_in(&used, Tongue::German, "Robin")[0].body.contains("Kontingent"));
    // Was keine Kennzeichnung braucht, bekommt keine.
    for mark in [Mark::Translated, Mark::Original, Mark::Notice] {
        let parts = render_in(&delivery("Hallo", mark), Tongue::German, "Robin");
        assert_eq!(parts[0].body, "Robin: Hallo");
    }
}

#[test]
fn nothing_a_person_wrote_can_act_as_markup() {
    let d = delivery("<b>laut</b> & \"leise\"\nzweite Zeile", Mark::Original);
    let part = &render_in(&d, Tongue::English, "<i>Robin</i>")[0];
    assert_eq!(
        part.html,
        "<strong>&lt;i&gt;Robin&lt;/i&gt;</strong>: &lt;b&gt;laut&lt;/b&gt; &amp; \
         &quot;leise&quot;<br>zweite Zeile"
    );
    // Der reine Text bleibt, wie er geschrieben wurde.
    assert_eq!(part.body, "<i>Robin</i>: <b>laut</b> & \"leise\"\nzweite Zeile");
    assert_eq!(escape("a\r\nb"), "a<br>b");
}

#[test]
fn a_very_long_delivery_is_split_and_loses_nothing() {
    let long = "wort ".repeat(3000);
    let d = delivery(&long, Mark::Untranslated(Reason::NoConsent));
    let parts = render_in(&d, Tongue::English, "Robin");
    assert!(parts.len() > 1);
    let mut together = String::new();
    for (n, part) in parts.iter().enumerate() {
        let text = part.body.strip_prefix("Robin: ").expect("every part names the writer");
        let last = n == parts.len() - 1;
        // Die Kennzeichnung steht einmal, unter dem letzten Stück.
        assert_eq!(text.contains("(not translated"), last);
        let text = text.split("\n(not translated").next().unwrap();
        assert!(text.chars().count() <= MESSAGE_LIMIT);
        together.push_str(text);
    }
    assert_eq!(together, long);
    // Ein Ereignis von Matrix fasst 65536 Bytes, und der Text steht zweimal darin.
    for part in &parts {
        assert!(part.body.len() + part.html.len() < 60_000);
    }
}

#[test]
fn a_name_stays_on_one_line_and_is_never_empty() {
    assert_eq!(shown_name("  Zebra\nQuokka  "), "Zebra Quokka");
    assert_eq!(shown_name("   "), "Guest");
    assert_eq!(shown_name(&"z".repeat(500)).chars().count(), 80);
    let parts = render_in(&delivery("Hallo", Mark::Original), Tongue::English, "A\nB");
    assert_eq!(parts[0].body, "A B: Hallo");
}

#[test]
fn the_fixed_texts_fit_into_one_message() {
    for tongue in ALL {
        let consent = consent_text_at(Door::Matrix, tongue, &"s".repeat(200));
        assert!(consent.chars().count() < MESSAGE_LIMIT);
        let invited = invitation_at(Door::Matrix, tongue, Some(&"z".repeat(5000)));
        assert!(invited.chars().count() < MESSAGE_LIMIT);
        // Ein fester Text geht in beiden Fassungen gleich hinaus.
        assert_eq!(plain(&consent).body, consent);
    }
}

#[test]
fn a_reply_loses_the_quote_an_older_app_puts_before_it() {
    let reply = "> <@robin:example.org> Wie geht es dir?\n> zweite Zeile\n\nGut, danke!";
    assert_eq!(without_quote(reply), "Gut, danke!");
    // Ein neueres Programm sendet kein Zitat: der Text bleibt.
    assert_eq!(without_quote("Gut, danke!"), "Gut, danke!");
    // Ein Zitat ohne Antwort dahinter wird nicht zu einer leeren Nachricht.
    assert_eq!(without_quote("> nur ein Zitat"), "> nur ein Zitat");
    assert_eq!(without_quote("> Zitat\nohne Leerzeile"), "> Zitat\nohne Leerzeile");
    assert_eq!(without_quote("> Zitat\n\nerste\n\n> eigenes Zitat"), "erste\n\n> eigenes Zitat");
}

// --- die Erwähnung -------------------------------------------------------------

const OWN: [&str; 3] = ["Rusty Fish", "fish", "@fish:example.org"];

#[test]
fn a_mention_of_the_fish_at_the_start_is_a_question() {
    assert_eq!(mention_question("Rusty Fish: wie spät ist es?", true, &OWN), Some("wie spät ist es?"));
    assert_eq!(mention_question("@Rusty Fish wie spät?", true, &OWN), Some("wie spät?"));
    assert_eq!(mention_question("  rusty fish, wie spät?", true, &OWN), Some("wie spät?"));
    assert_eq!(mention_question("fish: wie spät?", true, &OWN), Some("wie spät?"));
    assert_eq!(mention_question("@fish:example.org: wie spät?", true, &OWN), Some("wie spät?"));
}

#[test]
fn the_full_id_counts_even_without_the_mention_field() {
    // Ältere Programme setzen das Feld nicht; die ganze Kennung meint niemand sonst.
    assert_eq!(mention_question("@fish:example.org wie spät?", false, &OWN), Some("wie spät?"));
    // Ein bloßer Name ohne das Feld ist ein gewöhnlicher Satzanfang.
    assert_eq!(mention_question("Rusty Fish: wie spät?", false, &OWN), None);
    assert_eq!(mention_question("fish schmeckt gut", false, &OWN), None);
}

#[test]
fn what_is_no_mention_at_the_start_does_not_call() {
    // Mitten im Satz.
    assert_eq!(mention_question("Frag doch Rusty Fish, wie spät es ist", true, &OWN), None);
    // Ohne Text dahinter.
    assert_eq!(mention_question("Rusty Fish", true, &OWN), None);
    assert_eq!(mention_question("Rusty Fish:  ", true, &OWN), None);
    // Der Name muss zu Ende sein.
    assert_eq!(mention_question("fishing ist schön", true, &OWN), None);
    // Eine andere Kennung.
    assert_eq!(mention_question("@fish:other.example wie spät?", false, &OWN), None);
    assert_eq!(mention_question("@fisher:example.org wie spät?", true, &["@fish:example.org"]), None);
    // Ohne Namen spricht nichts an.
    assert_eq!(mention_question("Rusty Fish: wie spät?", true, &[]), None);
    assert_eq!(mention_question("irgendwas", true, &["", "  "]), None);
}

#[test]
fn a_mention_becomes_the_call_name_the_room_understands() {
    assert_eq!(
        mention_as_call("Rusty Fish: wie spät ist es?", true, &OWN, "Fish"),
        Some("Fish, wie spät ist es?".to_owned())
    );
    // Die Kennung des Fischs erreicht weder die KI noch die anderen Plätze.
    let called = mention_as_call("@fish:example.org wie spät?", true, &OWN, " Фиш ").unwrap();
    assert_eq!(called, "Фиш, wie spät?");
    assert_eq!(mention_as_call("ganz normaler Satz", true, &OWN, "Fish"), None);
}

// --- die Zeichen statt der Knöpfe -----------------------------------------------

fn consent() -> Ask {
    Ask::Consent { service: "ai.example".to_owned() }
}

fn budget(profile: Option<&str>) -> Ask {
    Ask::Budget {
        profile: profile.map(str::to_owned),
        raise: "Raise by 100,000".to_owned(),
        keep: "Leave it".to_owned(),
    }
}

#[test]
fn a_request_survives_the_trip_through_its_own_message() {
    for ask in [consent(), budget(Some("tea-time")), budget(None)] {
        let content = notice("text", "text", Some(&ask));
        assert_eq!(Ask::from_content(&content), Some(ask));
    }
    // Eine gewöhnliche Zustellung fragt nichts.
    assert_eq!(Ask::from_content(&notice("text", "text", None)), None);
    assert_eq!(Ask::from_content(&json!({ "body": "hi" })), None);
    // Ein Feld, das sich nicht lesen lässt, fragt auch nichts.
    assert_eq!(Ask::from_content(&json!({ FIELD: { "ask": "something-else" } })), None);
    assert_eq!(Ask::from_content(&json!({ FIELD: { "ask": "budget" } })), None);
    assert_eq!(Ask::from_content(&json!({ FIELD: "consent" })), None);
}

#[test]
fn the_sign_under_the_request_is_the_ok_the_request_speaks_of() {
    assert_eq!(consent().keys(), [CONSENT_KEY]);
    for tongue in ALL {
        let text = consent_text_at(Door::Matrix, tongue, "ai.example");
        assert!(text.contains(CONSENT_KEY), "the request names the sign: {text}");
    }
    assert!(consent().agrees("OK", "ai.example"));
    assert!(consent().agrees(" OK ", "ai.example"));
    // Ein anderes Zeichen unter der Bitte ist keine Zustimmung.
    assert!(!consent().agrees("👍", "ai.example"));
    assert!(!consent().agrees("ok", "ai.example"));
}

#[test]
fn consent_under_an_old_request_does_not_cover_a_new_service() {
    // Die Bitte nannte ai.example; inzwischen übersetzt ein anderer Dienst.
    assert!(!consent().agrees("OK", "other.example"));
}

#[test]
fn the_budget_notice_carries_both_answers_and_its_profile() {
    let ask = budget(Some("tea-time"));
    assert_eq!(ask.keys(), ["Raise by 100,000", "Leave it"]);
    assert_eq!(ask.budget_choice("Raise by 100,000"), Some(Budget::Raise));
    assert_eq!(ask.budget_choice("Leave it"), Some(Budget::Keep));
    assert_eq!(ask.budget_choice("OK"), None);
    // Die Zeichen der einen Frage beantworten die andere nicht.
    assert!(!ask.agrees("OK", "ai.example"));
    assert_eq!(consent().budget_choice("Raise by 100,000"), None);
}

#[test]
fn the_fish_sends_notices_that_mention_nobody() {
    let content = notice("Robin: hi", "<strong>Robin</strong>: hi", None);
    // Eine Notiz: daran erkennt ein anderer Fisch, was er zu überlesen hat.
    assert_eq!(content["msgtype"], "m.notice");
    assert_eq!(content["body"], "Robin: hi");
    assert_eq!(content["format"], "org.matrix.custom.html");
    assert_eq!(content["formatted_body"], "<strong>Robin</strong>: hi");
    assert_eq!(content["m.mentions"], json!({}));
    assert!(content.get(FIELD).is_none());
}

#[test]
fn a_sign_points_at_its_message() {
    let content = reaction("$event", "OK");
    assert_eq!(
        content["m.relates_to"],
        json!({ "rel_type": "m.annotation", "event_id": "$event", "key": "OK" })
    );
}

// --- der Zugang ------------------------------------------------------------------

/// Ein erfundener Zugang. Kein echter Schlüssel.
const SECRET: &str = r#"{"h":"https://matrix.example.org/","u":"@fish:example.org","d":"DEVICEID","t":"not-a-real-token","p":"not-a-real-passphrase"}"#;

#[test]
fn the_access_survives_the_trip_through_the_credential_store() {
    let access = Access::parse(SECRET).unwrap();
    assert_eq!(access.user_id().as_str(), "@fish:example.org");
    assert_eq!(access.homeserver(), "https://matrix.example.org/");
    let again = Access::parse(&access.to_secret()).unwrap();
    assert_eq!(again.to_secret(), access.to_secret());
    // Ein Zeilenende vom Einlesen stört nicht.
    assert!(Access::parse(&format!("{SECRET}\r\n")).is_ok());
}

#[test]
fn the_access_never_shows_what_is_secret() {
    let access = Access::parse(SECRET).unwrap();
    let shown = format!("{access:?}");
    assert!(shown.contains("@fish:example.org"));
    assert!(!shown.contains("not-a-real-token"), "got {shown}");
    assert!(!shown.contains("not-a-real-passphrase"), "got {shown}");
}

#[test]
fn a_damaged_access_is_refused_without_quoting_it() {
    let err = Access::parse("{\"t\":\"not-a-real-token\"").unwrap_err();
    assert!(err.contains("log in to Matrix again"), "got {err}");
    assert!(!err.contains("not-a-real-token"), "got {err}");
    assert!(Access::parse("").is_err());
}

#[test]
fn the_access_fits_into_the_credential_store_of_windows() {
    // Die Ablage von Windows fasst 2560 Bytes je Eintrag, in UTF-16 also 1280
    // Zeichen. Ein Zugangsschlüssel ist je nach Server bis zu einigen hundert
    // Zeichen lang.
    let long = SECRET.replace("not-a-real-token", &"t".repeat(600)).replace(
        "not-a-real-passphrase",
        &"p".repeat(64),
    );
    let access = Access::parse(&long).unwrap();
    assert!(access.to_secret().chars().count() < 1280, "{}", access.to_secret().len());
}

#[test]
fn a_user_id_is_checked_before_anything_is_sent() {
    assert_eq!(user_id(" @fish:example.org ").unwrap().as_str(), "@fish:example.org");
    for bad in ["fish", "@fish", "fish@example.org", ""] {
        let err = user_id(bad).unwrap_err();
        assert!(err.contains("@name:server"), "got {err}");
    }
}
