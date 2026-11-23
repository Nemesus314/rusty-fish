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
}
