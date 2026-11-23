// Tests zur Anrede eines Menschen in der Maske: das Feld „Formality" mit den
// Werten „unknown", „familiar" und „respectful". Gegen die Attrappe.
//
// Seit dem 23.11.2026. Nemesus an dem Tag: „in Korea redest du ja anders mit
// einer älteren Person als mit einer jüngeren." Gemessen wurde danach, dass
// ein Hinweis zur Anrede wirkt und das Alter darüber hinaus nichts bringt.

import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:rusty_fish_desk/main.dart';
import 'package:rusty_fish_desk/model.dart';
import 'package:rusty_fish_desk/share.dart';

import 'fake_cli.dart';

Future<void> open(WidgetTester tester, FakeCli fake) async {
  tester.view.physicalSize = const Size(1700, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(DeskApp(cli: fake, refreshEvery: null, today: () => '2026-11-23'));
  await tester.pumpAndSettle();
}

Finder inCard(String profile, Finder what) =>
    find.descendant(of: find.byKey(ValueKey('profile-$profile')), matching: what);

Future<void> choose(WidgetTester tester, String key, String option) async {
  await tester.ensureVisible(find.byKey(ValueKey(key)));
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(ValueKey(key)));
  await tester.pumpAndSettle();
  await tester.tap(find.text(option).last);
  await tester.pumpAndSettle();
}

String chosen(WidgetTester tester, String key) =>
    tester.widget<DropdownButton<String>>(find.byKey(ValueKey(key))).value ?? '';

/// Der Gastgeber liest Deutsch, seine Eingeladene Koreanisch.
ProfileDoc doc({String? formality}) => docOf(
  'tea',
  people: [
    const Person(name: 'Nemo', id: '100', lang: 'German'),
    Person(name: 'Mina', id: '7', lang: 'Korean', formality: formality, invited: true),
  ],
);

void main() {
  group('what goes to the program and back', () {
    test('the formality of a person is read and written only where it is set', () {
      final mina = Person.fromJson({'discord': '7', 'formality': 'respectful'});
      expect(mina.formality, respectfulFormality);
      expect(mina.toJson()['formality'], 'respectful');
      // Ohne Angabe geht das Feld nicht hinaus: ein älteres Programm kennt
      // es nicht.
      final nemo = Person.fromJson({'discord': '100'});
      expect(nemo.formality, isNull);
      expect(nemo.toJson().containsKey('formality'), isFalse);
      // Ein leerer Text ist keine Angabe.
      expect(Person.fromJson({'discord': '8', 'formality': ' '}).formality, isNull);
    });

    test('rooms shared as text carry it, like the grammatical form', () {
      final shared = readSharedRooms(shareRoomsText(doc(formality: 'respectful')))!;
      expect(shared.people.map((person) => person.formality), [null, 'respectful']);
    });
  });

  group('the field in the form', () {
    testWidgets('a profile without it shows "unknown" and goes back out unchanged', (
      tester,
    ) async {
      final fake = FakeCli(entries: [entryOf(doc())]);
      await open(tester, fake);
      await tester.tap(inCard('tea', find.text('Edit')));
      await tester.pumpAndSettle();
      expect(find.text('Formality'), findsNWidgets(2), reason: 'one field per person');
      expect(chosen(tester, 'person-0-formality'), '');
      expect(chosen(tester, 'person-1-formality'), '');
      // Die Vorgabe sagt, was man weiß: nichts.
      expect(find.text('unknown'), findsNWidgets(2));
      expect(find.text('as written'), findsNothing);

      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(fake.saved.single.toJson(), doc().toJson());
      expect(fake.saved.single.people.every((person) => person.formality == null), isTrue);
    });

    testWidgets('choosing "respectful" saves it with that person and nobody else', (
      tester,
    ) async {
      final fake = FakeCli(entries: [entryOf(doc())]);
      await open(tester, fake);
      await tester.tap(inCard('tea', find.text('Edit')));
      await tester.pumpAndSettle();
      await choose(tester, 'person-1-formality', 'respectful');
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();

      final people = fake.saved.single.people;
      expect(people[0].formality, isNull);
      expect(people[1].formality, respectfulFormality);
      // Sonst ändert sich an dem Menschen nichts.
      expect(people[1].toJson()..remove('formality'), doc().people[1].toJson());
    });

    testWidgets('a set value opens as it is, can become "familiar", and can be taken away', (
      tester,
    ) async {
      final fake = FakeCli(entries: [entryOf(doc(formality: 'respectful'))]);
      await open(tester, fake);
      await tester.tap(inCard('tea', find.text('Edit')));
      await tester.pumpAndSettle();
      expect(chosen(tester, 'person-1-formality'), 'respectful');

      await choose(tester, 'person-1-formality', 'familiar');
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(fake.saved.last.people[1].formality, familiarFormality);

      await tester.tap(inCard('tea', find.text('Edit')));
      await tester.pumpAndSettle();
      await choose(tester, 'person-1-formality', 'unknown');
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(fake.saved.last.people[1].formality, isNull);
    });

    testWidgets('a value this window does not know stays as it is', (tester) async {
      final fake = FakeCli(entries: [entryOf(doc(formality: 'ceremonial'))]);
      await open(tester, fake);
      await tester.tap(inCard('tea', find.text('Edit')));
      await tester.pumpAndSettle();
      expect(chosen(tester, 'person-1-formality'), 'ceremonial');
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(fake.saved.single.people[1].formality, 'ceremonial');
    });

    testWidgets('a new person starts "unknown"', (tester) async {
      final fake = FakeCli(entries: [entryOf(doc())]);
      await open(tester, fake);
      await tester.tap(inCard('tea', find.text('Edit')));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Add person'));
      await tester.tap(find.text('Add person'));
      await tester.pumpAndSettle();
      expect(chosen(tester, 'person-2-formality'), '');
    });
  });
}
