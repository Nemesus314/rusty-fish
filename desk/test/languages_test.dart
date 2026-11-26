// Tests zu den Vorschlägen in den Sprachfeldern. Gegen die Attrappe.
//
// Nemesus am 24.11.2026: „Im Moment muss man die Sprachen manuell eingeben.
// Das könnte Fehler verursachen. Besser wäre, wenn Vorschläge kommen würden,
// je nachdem, was ich tippe. Beispielsweise ich tippe Ko und ergänzt wird
// Koreanisch."

import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:rusty_fish_desk/main.dart';
import 'package:rusty_fish_desk/model.dart';

import 'fake_cli.dart';

Future<void> open(WidgetTester tester, FakeCli fake) async {
  tester.view.physicalSize = const Size(1700, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(DeskApp(cli: fake, refreshEvery: null, today: () => '2026-11-24'));
  await tester.pumpAndSettle();
}

Future<void> edit(WidgetTester tester, String profile) async {
  await tester.tap(
    find.descendant(of: find.byKey(ValueKey('profile-$profile')), matching: find.text('Edit')),
  );
  await tester.pumpAndSettle();
}

Future<void> type(WidgetTester tester, String key, String text) async {
  await tester.ensureVisible(find.byKey(ValueKey(key)));
  await tester.pumpAndSettle();
  await tester.enterText(find.byKey(ValueKey(key)), text);
  await tester.pumpAndSettle();
}

String textOf(WidgetTester tester, String key) =>
    tester.widget<TextField>(find.byKey(ValueKey(key))).controller!.text;

Finder suggestion(String name) => find.byKey(ValueKey('suggest-$name'));

ProfileDoc doc() => docOf(
  'tea',
  people: [
    const Person(name: 'Nemo', id: '100', lang: 'German'),
    const Person(name: 'Mina', id: '7', lang: 'Russian', invited: true),
  ],
);

void main() {
  group('which languages fit what was typed', () {
    List<String> names(String typed) => [
      for (final hit in suggestLanguages(fakeLanguages, typed)) hit.name,
    ];

    test('the beginning of a name is enough', () {
      expect(names('Ko'), ['Korean']);
      expect(names('ko'), ['Korean']);
      expect(names(' Ko '), ['Korean']);
    });

    test('another name leads to the word the fish reads, and says how it was found', () {
      final hits = suggestLanguages(fakeLanguages, 'Deu');
      expect(hits, [(name: 'German', via: 'Deutsch')]);
      expect(names('Koreanisch'), ['Korean']);
      expect(names('한'), ['Korean']);
    });

    test('names first, then other names, then a word inside a name', () {
      expect(names('Ka'), ['Kannada', 'Cantonese']);
      expect(names('Chinese'), ['Mandarin', 'Simplified Chinese']);
      expect(names('Port'), ['Portuguese', 'Brazilian Portuguese']);
    });

    test('nothing to suggest where nothing or a listed name stands', () {
      expect(names(''), isEmpty);
      expect(names('  '), isEmpty);
      expect(names('Korean'), isEmpty);
      expect(names('Portuguese'), isEmpty);
      expect(names('Klingon'), isEmpty);
      // Klein geschrieben ist es noch nicht das Wort der Liste.
      expect(names('german'), ['German']);
    });

    test('what the program names is read; what is no language is left out', () {
      final korean = Language.fromJson({
        'name': 'Korean',
        'also': ['Koreanisch', 7, ' '],
        'level': true,
      });
      expect(korean.name, 'Korean');
      expect(korean.also, ['Koreanisch']);
      expect(Language.fromJson({'name': 'Thai'}).also, isEmpty);
    });
  });

  group('the fields in the form', () {
    testWidgets('typing "Ko" at a table offers Korean, and a tap takes it', (tester) async {
      final fake = FakeCli(entries: [entryOf(doc())]);
      await open(tester, fake);
      await edit(tester, 'tea');
      expect(suggestion('Korean'), findsNothing);

      await type(tester, 'seat-0-0-lang', 'Ko');
      expect(suggestion('Korean'), findsOneWidget);
      await tester.tap(suggestion('Korean'));
      await tester.pumpAndSettle();
      expect(textOf(tester, 'seat-0-0-lang'), 'Korean');
      expect(suggestion('Korean'), findsNothing);

      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(fake.saved.single.rooms[0].seats[0].lang, 'Korean');
    });

    testWidgets('the Enter key takes the first suggestion', (tester) async {
      final fake = FakeCli(entries: [entryOf(doc())]);
      await open(tester, fake);
      await edit(tester, 'tea');
      await type(tester, 'person-1-lang', 'Deutsch');
      // Gefunden über den deutschen Namen; der steht dabei.
      expect(
        find.descendant(of: suggestion('German'), matching: find.text('Deutsch')),
        findsOneWidget,
      );
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await tester.pumpAndSettle();
      expect(textOf(tester, 'person-1-lang'), 'German');

      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(fake.saved.single.people[1].lang, 'German');
    });

    testWidgets('with several languages only the last one is completed', (tester) async {
      final fake = FakeCli(entries: [entryOf(doc())]);
      await open(tester, fake);
      await edit(tester, 'tea');
      await type(tester, 'room-0-pass', 'English, Rus');
      await tester.tap(suggestion('Russian'));
      await tester.pumpAndSettle();
      expect(textOf(tester, 'room-0-pass'), 'English, Russian');

      await type(tester, 'person-1-neutral', 'Kor');
      await tester.tap(suggestion('Korean'));
      await tester.pumpAndSettle();
      expect(textOf(tester, 'person-1-neutral'), 'Korean');

      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(fake.saved.single.rooms[0].passThrough, ['English', 'Russian']);
      expect(fake.saved.single.people[1].neutral, ['Korean']);
    });

    testWidgets('a word that is not on the list stays as typed', (tester) async {
      final fake = FakeCli(entries: [entryOf(doc())]);
      await open(tester, fake);
      await edit(tester, 'tea');
      await type(tester, 'seat-0-0-lang', 'Swiss German');
      expect(find.byWidgetPredicate((w) => '${w.key}'.contains('suggest-')), findsNothing);
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(fake.saved.single.rooms[0].seats[0].lang, 'Swiss German');
    });

    testWidgets('a field that already holds a listed name offers nothing when entered', (
      tester,
    ) async {
      final fake = FakeCli(entries: [entryOf(doc())]);
      await open(tester, fake);
      await edit(tester, 'tea');
      await tester.ensureVisible(find.byKey(const ValueKey('person-0-lang')));
      await tester.tap(find.byKey(const ValueKey('person-0-lang')));
      await tester.pumpAndSettle();
      expect(textOf(tester, 'person-0-lang'), 'German');
      expect(find.byWidgetPredicate((w) => '${w.key}'.contains('suggest-')), findsNothing);
    });

    testWidgets('an older program names no languages: the fields work as before', (
      tester,
    ) async {
      final fake = FakeCli(entries: [entryOf(doc())])
        ..languagesError = "error: unrecognized subcommand 'languages'";
      await open(tester, fake);
      // Die Übersicht sagt dazu nichts: es fehlt nur eine Hilfe.
      expect(find.textContaining('languages'), findsNothing);
      await edit(tester, 'tea');
      await type(tester, 'seat-0-0-lang', 'Ko');
      expect(suggestion('Korean'), findsNothing);
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(fake.saved.single.rooms[0].seats[0].lang, 'Ko');
    });
  });

  // Nemesus am 26.11.2026: „Wobei ich mich frage, was passiert, wenn man eine
  // Sprache falsch schreibt?" Bis dahin: nichts, und nichts sagte es.
  group('a word that is not on the list is named under the field', () {
    test('what the hint says depends on what the list offers for the word', () {
      // Alles auf der Liste: nichts zu sagen.
      expect(languageHint(fakeLanguages, 'English'), isNull);
      expect(languageHint(fakeLanguages, ''), isNull);
      expect(languageHint(fakeLanguages, 'English, Russian', many: true), isNull);

      // Ein Tippfehler, zu dem die Liste nichts anbietet.
      expect(
        languageHint(fakeLanguages, 'Englich'),
        '"Englich" is not on the list of languages. It is kept as typed and goes to the AI like that.',
      );
      // Der deutsche Name, oder klein geschrieben: genau ein Name passt.
      expect(
        languageHint(fakeLanguages, 'Deutsch'),
        '"Deutsch" is not the word the fish knows: pick "German" from the suggestions.',
      );
      expect(languageHint(fakeLanguages, 'english'), contains('pick "English"'));
      // Ein Anfang, zu dem mehrere passen.
      expect(languageHint(fakeLanguages, 'K'), contains('not a whole name of the list'));

      // Mehrere Sprachen: nur die genannt, die nicht stimmen.
      final several = languageHint(fakeLanguages, 'English, Russisch, Klingon', many: true)!;
      expect(several, isNot(contains('"English"')));
      expect(several, contains('pick "Russian"'));
      expect(several, contains('"Klingon" is not on the list'));
      // Ohne `many` ist das Ganze ein Wort.
      expect(languageHint(fakeLanguages, 'English, Russian'), contains('"English, Russian"'));

      // Ein älteres Programm nennt keine Liste: dann weiß die Maske nichts.
      expect(languageHint(const [], 'Englich'), isNull);
    });

    testWidgets('the hint appears when the field is left, not while typing', (tester) async {
      final fake = FakeCli(entries: [entryOf(doc())]);
      await open(tester, fake);
      await edit(tester, 'tea');
      // Was im Profil steht, steht auf der Liste: kein Hinweis.
      expect(find.byKey(const ValueKey('language-hint-room-0-pass')), findsNothing);

      await type(tester, 'room-0-pass', 'Englich');
      // Noch im Feld: kein Hinweis, die Vorschläge wären jetzt dran.
      expect(find.byKey(const ValueKey('language-hint-room-0-pass')), findsNothing);

      FocusManager.instance.primaryFocus?.unfocus();
      await tester.pumpAndSettle();
      expect(
        tester.widget<Text>(find.byKey(const ValueKey('language-hint-room-0-pass'))).data,
        contains('"Englich" is not on the list of languages'),
      );

      // Das Feld bleibt frei: gespeichert wird, was dasteht.
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(fake.saved.single.rooms[0].passThrough, ['Englich']);
    });

    testWidgets('a name of the list, taken from the suggestions, clears the hint', (tester) async {
      final fake = FakeCli(entries: [entryOf(doc())]);
      await open(tester, fake);
      await edit(tester, 'tea');
      await type(tester, 'person-1-lang', 'Deutsch');
      FocusManager.instance.primaryFocus?.unfocus();
      await tester.pumpAndSettle();
      expect(
        tester.widget<Text>(find.byKey(const ValueKey('language-hint-person-1-lang'))).data,
        '"Deutsch" is not the word the fish knows: pick "German" from the suggestions.',
      );

      await type(tester, 'person-1-lang', 'Deu');
      await tester.tap(suggestion('German'));
      await tester.pumpAndSettle();
      FocusManager.instance.primaryFocus?.unfocus();
      await tester.pumpAndSettle();
      expect(textOf(tester, 'person-1-lang'), 'German');
      expect(find.byKey(const ValueKey('language-hint-person-1-lang')), findsNothing);
    });

    testWidgets('a profile opened with a word off the list shows the hint at once', (tester) async {
      final fake = FakeCli(
        entries: [
          entryOf(
            docOf(
              'tea',
              seats: const [
                Seat(channel: '111', lang: 'German'),
                Seat(channel: '222', lang: 'Rusian'),
              ],
            ),
          ),
        ],
      );
      await open(tester, fake);
      await edit(tester, 'tea');
      expect(find.byKey(const ValueKey('language-hint-seat-0-0-lang')), findsNothing);
      expect(
        tester.widget<Text>(find.byKey(const ValueKey('language-hint-seat-0-1-lang'))).data,
        contains('"Rusian" is not on the list of languages'),
      );
    });

    testWidgets('an older program names no languages: no hint at all', (tester) async {
      final fake = FakeCli(entries: [entryOf(doc())])
        ..languagesError = "error: unrecognized subcommand 'languages'";
      await open(tester, fake);
      await edit(tester, 'tea');
      await type(tester, 'room-0-pass', 'Englich');
      FocusManager.instance.primaryFocus?.unfocus();
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('language-hint-room-0-pass')), findsNothing);
    });
  });
}
