//! Tests der Tische: was an einem Tisch gilt, welche Tische ein Fisch bedient,
//! und in welcher Sprache der Fisch selbst spricht. Keine Tür, kein Netz.

use rusty_fish_core::{Model, ModelError, Prompt, Reply};
use rusty_fish_room::tongue::{Tongue, no_answer, no_questions_left, tongue_of};
use rusty_fish_room::{
    Delivery, Formality, Gender, House, Incoming, Limits, Mark, NO_ANSWER, NO_QUESTIONS_LEFT,
    Reason, Room, RoomState, Seat, Sender, Tally, converse, relay,
};
use std::future::Future;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

struct Fake {
    answer: Result<String, String>,
    asked: Mutex<Vec<Prompt>>,
}

impl Fake {
    fn says(text: &str) -> Self {
        Fake { answer: Ok(text.to_owned()), asked: Mutex::new(Vec::new()) }
    }

    fn fails(why: &str) -> Self {
        Fake { answer: Err(why.to_owned()), asked: Mutex::new(Vec::new()) }
    }

    fn prompts(&self) -> Vec<Prompt> {
        self.asked.lock().unwrap().clone()
    }

    fn is_question(prompt: &Prompt) -> bool {
        prompt.user.contains("<message ")
    }

    fn questions(&self) -> usize {
        self.prompts().iter().filter(|p| Fake::is_question(p)).count()
    }

    fn translations(&self) -> Vec<Prompt> {
        self.prompts().into_iter().filter(|p| !Fake::is_question(p)).collect()
    }
}

impl Model for Fake {
    async fn complete(&self, prompt: &Prompt) -> Result<Reply, ModelError> {
        self.asked.lock().unwrap().push(prompt.clone());
        match &self.answer {
            Ok(text) => Ok(Reply { text: text.clone(), tokens: Some(10) }),
            Err(why) => Err(ModelError(why.clone())),
        }
    }
}

fn now<F: Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = Context::from_waker(Waker::noop());
    match f.as_mut().poll(&mut cx) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("the fake model must answer at once"),
    }
}

fn from<'a>(channel: &'a str, author: &'a str, text: &'a str) -> Incoming<'a> {
    Incoming { channel, author, text, consented: true, author_gender: None }
}

fn room(seats: Vec<Seat>) -> Room {
    Room { seats, pass_through: vec!["English".to_owned()], context_lines: 5 }
}

fn channels(out: &[Delivery]) -> Vec<&str> {
    out.iter().map(|d| d.channel.as_str()).collect()
}

/// Der Auftrag, der in diese Sprache übersetzen sollte.
fn prompt_into<'a>(prompts: &'a [Prompt], lang: &str) -> &'a Prompt {
    prompts
        .iter()
        .find(|p| p.system.contains(&format!("into {lang}")))
        .unwrap_or_else(|| panic!("no prompt translates into {lang}"))
}

// --- Was an einem Tisch gilt -------------------------------------------------

#[test]
fn a_table_keeps_its_own_untranslated_languages() {
    let fake = Fake::says("x");
    let room = room(vec![
        Seat::new("de", "German"),
        // Für diesen Tisch bleibt nichts unübersetzt.
        Seat { neutral: Some(vec![]), ..Seat::new("ru", "Russian") },
        // Dieser Tisch hat keine eigene Angabe: es gilt die des Raums.
        Seat::new("ko", "Korean"),
        Seat { neutral: Some(vec!["Spanish".to_owned()]), ..Seat::new("fr", "French") },
    ]);
    let mut state = RoomState::default();
    let out = now(relay(&fake, &room, &mut state, &from("de", "n", "Hallo zusammen")));
    assert_eq!(channels(&out), ["ru", "ko", "fr"]);

    let prompts = fake.prompts();
    let ru = prompt_into(&prompts, "Russian");
    assert!(!ru.system.contains("English"), "nothing stays untranslated here: {}", ru.system);
    let ko = prompt_into(&prompts, "Korean");
    assert!(ko.system.contains("English"), "the room's languages apply: {}", ko.system);
    let fr = prompt_into(&prompts, "French");
    assert!(fr.system.contains("Spanish"), "the table's own languages apply: {}", fr.system);
    assert!(!fr.system.contains("English"), "and replace the room's: {}", fr.system);
}

#[test]
fn tables_share_a_translation_only_when_everything_agrees() {
    let fake = Fake::says("Привет");
    let room = room(vec![
        Seat::new("de", "German"),
        Seat::new("ru1", "Russian"),
        Seat::new("ru2", " russian"),
        // Gleiche Sprache, aber andere unübersetzte Sprachen.
        Seat { neutral: Some(vec![]), ..Seat::new("ru3", "Russian") },
        // Gleiche Sprache, aber andere Anrede.
        Seat { gender: Some(Gender::Female), ..Seat::new("ru4", "Russian") },
        // Die Reihenfolge und Schreibung der unübersetzten Sprachen ist gleichgültig.
        Seat { neutral: Some(vec![" english ".to_owned()]), ..Seat::new("ru5", "Russian") },
    ]);
    let mut state = RoomState::default();
    let out = now(relay(&fake, &room, &mut state, &from("de", "n", "Hallo")));
    assert_eq!(out.len(), 5);
    assert_eq!(fake.prompts().len(), 3, "one paid translation per kind of table");
}

#[test]
fn the_owners_form_is_for_reading_and_the_seats_form_stays_for_writing() {
    let room = room(vec![
        Seat::new("de", "German"),
        // Am Platz steht „männlich", der Tisch gehört aber einer Frau.
        Seat {
            gender: Some(Gender::Male),
            reader_gender: Some(Gender::Female),
            ..Seat::new("ru", "Russian")
        },
    ]);
    let mut state = RoomState::default();

    // Was hier gelesen wird, spricht die Besitzerin an.
    let fake = Fake::says("x");
    let _ = now(relay(&fake, &room, &mut state, &from("de", "n", "Hast du geschlafen?")));
    let p = fake.prompts().pop().unwrap();
    assert!(p.system.contains("The reader of your translation is a woman"), "got: {}", p.system);

    // Wer hier schreibt und der Tür unbekannt ist, behält die Angabe des Platzes.
    let fake = Fake::says("x");
    let _ = now(relay(&fake, &room, &mut state, &from("ru", "a", "Я устал.")));
    let p = fake.prompts().pop().unwrap();
    assert!(p.system.contains("The writer of the text is a man."), "got: {}", p.system);

    // Die Angabe zur Person selbst geht weiter jeder Angabe des Platzes vor.
    let fake = Fake::says("x");
    let mut msg = from("ru", "b", "Я устала.");
    msg.author_gender = Some(Gender::Female);
    let _ = now(relay(&fake, &room, &mut state, &msg));
    let p = fake.prompts().pop().unwrap();
    assert!(p.system.contains("The writer of the text is a woman."), "got: {}", p.system);
}

#[test]
fn without_an_owners_form_the_seats_form_addresses_the_reader() {
    let seat = Seat { gender: Some(Gender::Male), ..Seat::new("ru", "Russian") };
    assert_eq!(seat.reader(), Some(Gender::Male));
    assert_eq!(Seat::new("ru", "Russian").reader(), None);
    let owned = Seat { reader_gender: Some(Gender::Female), ..seat };
    assert_eq!(owned.reader(), Some(Gender::Female));
}

#[test]
fn tables_with_the_same_form_for_the_reader_share_a_translation() {
    // Woher die Anrede kommt, vom Besitzer oder vom Platz, ist gleichgültig.
    let fake = Fake::says("Привет");
    let room = room(vec![
        Seat::new("de", "German"),
        Seat { gender: Some(Gender::Female), ..Seat::new("ru1", "Russian") },
        Seat {
            gender: Some(Gender::Male),
            reader_gender: Some(Gender::Female),
            ..Seat::new("ru2", "Russian")
        },
    ]);
    let mut state = RoomState::default();
    let out = now(relay(&fake, &room, &mut state, &from("de", "n", "Hallo")));
    assert_eq!(out.len(), 2);
    assert_eq!(fake.prompts().len(), 1);
}

// --- Welche Tische dieser Fisch bedient --------------------------------------

fn three_tables() -> Room {
    room(vec![
        Seat::new("de", "German"),
        Seat { served: false, ..Seat::new("ru", "Russian") },
        Seat::new("ko", "Korean"),
    ])
}

#[test]
fn a_table_this_fish_does_not_serve_gets_nothing() {
    let fake = Fake::says("x");
    let mut state = RoomState::default();
    let out = now(relay(&fake, &three_tables(), &mut state, &from("de", "n", "Hallo")));
    assert_eq!(channels(&out), ["ko"]);
    assert_eq!(fake.prompts().len(), 1, "no call for a table that is not served");
}

#[test]
fn what_is_written_at_such_a_table_still_reaches_the_served_ones() {
    let fake = Fake::says("x");
    let mut state = RoomState::default();
    let out = now(relay(&fake, &three_tables(), &mut state, &from("ru", "a", "Привет")));
    assert_eq!(channels(&out), ["de", "ko"]);
    assert!(out.iter().all(|d| d.mark == Mark::Translated));
}

#[test]
fn without_consent_the_original_goes_only_to_served_tables() {
    let fake = Fake::says("x");
    let mut state = RoomState::default();
    let mut msg = from("de", "n", "Hallo");
    msg.consented = false;
    let out = now(relay(&fake, &three_tables(), &mut state, &msg));
    assert_eq!(channels(&out), ["ko"]);
    assert_eq!(out[0].mark, Mark::Untranslated(Reason::NoConsent));
    assert!(fake.prompts().is_empty());
}

struct Setup {
    limits: Limits,
    state: RoomState,
    tally: Tally,
}

impl Setup {
    fn new(questions: u32) -> Self {
        Setup {
            limits: Limits { translation_tokens_per_day: 100_000, questions_per_guest_per_day: questions },
            state: RoomState::default(),
            tally: Tally::default(),
        }
    }

    fn say(&mut self, fake: &Fake, room: &Room, msg: &Incoming<'_>) -> Vec<Delivery> {
        let names = ["Fish"];
        let house = House { limits: &self.limits, ai_names: &names, today: "2026-11-20" };
        now(converse(fake, room, &mut self.state, &mut self.tally, &house, msg, false))
    }
}

#[test]
fn the_ai_answers_only_at_tables_this_fish_serves() {
    let fake = Fake::says("ANSWER");
    let mut s = Setup::new(5);
    let out = s.say(&fake, &three_tables(), &from("de", "guest", "Fish, bist du da?"));
    // Die Frage geht an den bedienten Tisch, die Antwort an den Tisch des
    // Fragenden und an den bedienten; der nicht bediente bekommt nichts.
    assert!(channels(&out).iter().all(|c| *c != "ru"), "got {:?}", channels(&out));
    let answers: Vec<&str> =
        out.iter().filter(|d| d.from == Sender::Assistant).map(|d| d.channel.as_str()).collect();
    assert_eq!(answers, ["de", "ko"]);
}

#[test]
fn a_question_at_a_table_this_fish_does_not_serve_is_not_answered() {
    // Der Fragende könnte die Antwort an seinem Tisch nie lesen. Seine
    // Nachricht selbst geht wie jede andere an die bedienten Tische.
    let fake = Fake::says("x");
    let mut s = Setup::new(5);
    let out = s.say(&fake, &three_tables(), &from("ru", "guest", "Fish, ты здесь?"));
    assert_eq!(channels(&out), ["de", "ko"]);
    assert!(out.iter().all(|d| d.from == Sender::Writer));
    assert_eq!(fake.questions(), 0, "no call to the AI");
    assert_eq!(s.tally.questions.get("guest"), None, "and nothing counted");
    assert_eq!(fake.translations().len(), 2);
}

// --- In welcher Sprache der Fisch selbst spricht -----------------------------

#[test]
fn the_three_languages_are_recognised_however_they_are_written() {
    for word in ["German", "german", " Deutsch ", "de", "DE", "de-AT", "deu", "ger", "Немецкий"] {
        assert_eq!(tongue_of(word), Tongue::German, "{word:?}");
    }
    for word in ["Russian", "russisch", "ru", "RU", "ru_RU", "rus", "Русский", " русский "] {
        assert_eq!(tongue_of(word), Tongue::Russian, "{word:?}");
    }
    for word in ["English", "englisch", "en", "en-GB", "eng", "Английский"] {
        assert_eq!(tongue_of(word), Tongue::English, "{word:?}");
    }
}

#[test]
fn every_other_language_gets_english() {
    for word in ["Korean", "한국어", "fr", "", "   ", "Klingon", "ruanda"] {
        assert_eq!(tongue_of(word), Tongue::English, "{word:?}");
    }
}

#[test]
fn the_fixed_texts_exist_in_all_three_languages() {
    assert_eq!(no_questions_left(Tongue::English), NO_QUESTIONS_LEFT);
    assert_eq!(no_answer(Tongue::English), NO_ANSWER);
    for text in [no_questions_left(Tongue::German), no_answer(Tongue::German)] {
        assert!(text.starts_with("Ich k"), "got {text:?}");
    }
    assert!(no_questions_left(Tongue::Russian).contains("вопрос"));
    assert!(no_answer(Tongue::Russian).contains("ответ"));
    // Drei verschiedene Texte, nicht dreimal derselbe.
    let all = [Tongue::German, Tongue::English, Tongue::Russian].map(no_questions_left);
    assert!(all[0] != all[1] && all[1] != all[2] && all[0] != all[2]);
}

#[test]
fn the_refusal_comes_in_the_language_of_the_askers_table() {
    for (channel, tongue) in [("de", Tongue::German), ("ru", Tongue::Russian), ("ko", Tongue::English)] {
        let fake = Fake::says("x");
        let room = room(vec![
            Seat::new("de", "German"),
            Seat::new("ru", "Russian"),
            Seat::new("ko", "Korean"),
        ]);
        let mut s = Setup::new(0);
        let out = s.say(&fake, &room, &from(channel, "guest", "Fish, hello?"));
        let notice = out.iter().find(|d| d.mark == Mark::Notice).expect("a refusal");
        assert_eq!(notice.channel, channel);
        assert_eq!(notice.text, no_questions_left(tongue), "at table {channel}");
    }
}

#[test]
fn a_failed_answer_is_said_in_the_language_of_the_askers_table() {
    struct Half(Fake);
    impl Model for Half {
        async fn complete(&self, prompt: &Prompt) -> Result<Reply, ModelError> {
            if Fake::is_question(prompt) {
                Err(ModelError("provider down".to_owned()))
            } else {
                self.0.complete(prompt).await
            }
        }
    }
    let half = Half(Fake::says("x"));
    let room = room(vec![Seat::new("de", "German"), Seat::new("ru", "Russian")]);
    let mut state = RoomState::default();
    let mut tally = Tally::default();
    let limits = Limits { translation_tokens_per_day: 100_000, questions_per_guest_per_day: 3 };
    let names = ["Fish"];
    let house = House { limits: &limits, ai_names: &names, today: "2026-11-20" };
    let msg = from("ru", "guest", "Fish, ты здесь?");
    let out = now(converse(&half, &room, &mut state, &mut tally, &house, &msg, false));
    let notice = out.iter().find(|d| d.mark == Mark::Notice).expect("a notice");
    assert!(notice.text.starts_with(no_answer(Tongue::Russian)), "got {:?}", notice.text);
    assert!(notice.text.contains("provider down"), "the reason stays as the service gave it");
}

#[test]
fn a_failed_translation_keeps_its_reason_untouched() {
    // Der technische Grund bleibt, wie der Dienst ihn meldet; übersetzt wird
    // erst beim Anzeigen nur der feste Teil davor.
    let fake = Fake::fails("HTTP 429: quota");
    let room = room(vec![Seat::new("de", "German"), Seat::new("ru", "Russian")]);
    let mut state = RoomState::default();
    let out = now(relay(&fake, &room, &mut state, &from("de", "n", "Hallo")));
    assert_eq!(out[0].mark, Mark::Untranslated(Reason::Failed("HTTP 429: quota".to_owned())));
}

// --- die Anrede am Tisch, seit dem 23.11.2026 ----------------------------------

#[test]
fn the_owners_formality_goes_with_what_is_read_at_their_table() {
    let room = room(vec![
        Seat::new("de", "German"),
        Seat { reader_formality: Some(Formality::Respectful), ..Seat::new("ko", "Korean") },
    ]);
    let mut state = RoomState::default();

    // Was an ihrem Tisch gelesen wird, spricht sie respektvoll an.
    let fake = Fake::says("x");
    let _ = now(relay(&fake, &room, &mut state, &from("de", "n", "Hast du heute Zeit?")));
    let p = prompt_into(&fake.prompts(), "Korean").clone();
    assert!(p.system.contains("addressed respectfully"), "got: {}", p.system);

    // Was sie schreibt, geht an den anderen Tisch ohne diesen Hinweis: dort
    // liest jemand, für den nichts eingetragen ist.
    let fake = Fake::says("x");
    let _ = now(relay(&fake, &room, &mut state, &from("ko", "a", "안녕하세요")));
    let p = prompt_into(&fake.prompts(), "German").clone();
    assert!(!p.system.contains("addressed respectfully"), "got: {}", p.system);
    assert!(!p.system.contains("familiar way"), "got: {}", p.system);
}

#[test]
fn tables_with_a_different_formality_get_translations_of_their_own() {
    let room = room(vec![
        Seat::new("de", "German"),
        Seat::new("ko1", "Korean"),
        Seat::new("ko2", "Korean"),
        Seat { reader_formality: Some(Formality::Respectful), ..Seat::new("ko3", "Korean") },
        Seat { reader_formality: Some(Formality::Familiar), ..Seat::new("ko4", "Korean") },
        Seat { reader_formality: Some(Formality::Respectful), ..Seat::new("ko5", "Korean") },
    ]);
    let mut state = RoomState::default();
    let fake = Fake::says("x");
    let out = now(relay(&fake, &room, &mut state, &from("de", "n", "Kommst du mit?")));
    assert_eq!(channels(&out), ["ko1", "ko2", "ko3", "ko4", "ko5"]);
    // Drei Arten von Tisch: ohne Angabe, respektvoll, vertraut. Gleiche Tische
    // teilen sich eine bezahlte Übersetzung.
    let prompts = fake.prompts();
    assert_eq!(prompts.len(), 3, "one paid translation per kind of table");
    assert_eq!(prompts.iter().filter(|p| p.system.contains("addressed respectfully")).count(), 1);
    assert_eq!(prompts.iter().filter(|p| p.system.contains("familiar way")).count(), 1);
    // Wo nichts eingetragen ist, gilt in einer Sprache mit Stufen die
    // alltägliche höfliche, und nur dort.
    let safe = "ordinary polite everyday forms";
    assert_eq!(prompts.iter().filter(|p| p.system.contains(safe)).count(), 1);

    // An einem russischen Tisch ohne Eintrag steht davon nichts.
    let plain = Room {
        seats: vec![Seat::new("de", "German"), Seat::new("ru", "Russian")],
        pass_through: vec!["English".to_owned()],
        context_lines: 5,
    };
    let fake = Fake::says("x");
    let _ = now(relay(&fake, &plain, &mut RoomState::default(), &from("de", "n", "Kommst du?")));
    assert!(!fake.prompts()[0].system.contains(safe), "got: {}", fake.prompts()[0].system);
}
