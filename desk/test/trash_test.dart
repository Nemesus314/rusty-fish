// Tests zum Papierkorb der Maske, gegen die Attrappe: gelöschte Profile
// ansehen, zurückholen, endgültig löschen. Kein Programm, kein Ordner.
//
// Seit dem 23.11.2026. Nemesus an dem Tag: „gibt es auch einen Weg, das
// gelöschte Profil vom Papierkorb zurück zu bringen oder endgültig zu
// löschen?"

import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:rusty_fish_desk/main.dart';
import 'package:rusty_fish_desk/model.dart';

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

/// Löscht ein Profil über seine Karte, mit der Rückfrage.
Future<void> delete(WidgetTester tester, String profile) async {
  await tester.tap(inCard(profile, find.text('Delete')));
  await tester.pumpAndSettle();
  await tester.tap(find.text('Move to trash'));
  await tester.pumpAndSettle();
}

Future<void> openTrash(WidgetTester tester) async {
  await tester.tap(find.byKey(const ValueKey('open-trash')));
  await tester.pumpAndSettle();
}

void main() {
  test('the trash is read in the form the program sends', () {
    final trash = Trash.fromJson({
      'dir': r'C:\fish\profiles\trash',
      'deleted': [
        {
          'id': 'tea-time-20261121-080000',
          'name': 'tea-time',
          'deleted': '2026-11-21 08:00:00 UTC',
          'state': true,
        },
        {'id': 'tea-20261120-070102-2', 'name': 'tea', 'deleted': '2026-11-20 07:01:02 UTC'},
      ],
    });
    expect(trash.dir, r'C:\fish\profiles\trash');
    expect(trash.deleted.map((entry) => entry.name), ['tea-time', 'tea']);
    expect(trash.deleted.first.id, 'tea-time-20261121-080000');
    expect(trash.deleted.map((entry) => entry.state), [true, false]);
  });

  testWidgets('an empty trash says so', (tester) async {
    await open(tester, FakeCli(entries: [entryOf(docOf('study'))]));
    await openTrash(tester);
    expect(find.text('The trash is empty.'), findsOneWidget);
    expect(find.textContaining(r'Kept in C:\fish\profiles\trash'), findsOneWidget);
    await tester.tap(find.text('Close'));
    await tester.pumpAndSettle();
    expect(find.text('The trash is empty.'), findsNothing);
  });

  testWidgets('the question before deleting says where to get the profile back', (tester) async {
    await open(tester, FakeCli(entries: [entryOf(docOf('study'))]));
    await tester.tap(inCard('study', find.text('Delete')));
    await tester.pumpAndSettle();
    expect(find.textContaining('under "Trash"'), findsOneWidget);
    expect(find.textContaining('put it back, or erase it for good'), findsOneWidget);
  });

  testWidgets('a deleted profile is listed and comes back as a card', (tester) async {
    final fake = FakeCli(entries: [entryOf(docOf('study')), entryOf(docOf('tea'))]);
    await open(tester, fake);
    await delete(tester, 'tea');
    expect(find.byKey(const ValueKey('profile-tea')), findsNothing);

    await openTrash(tester);
    final id = fake.trashed.single.id;
    expect(find.byKey(ValueKey('trash-$id')), findsOneWidget);
    expect(find.textContaining('deleted 2026-11-23'), findsOneWidget);

    await tester.tap(find.byKey(ValueKey('put-back-$id')));
    await tester.pumpAndSettle();
    expect(fake.restored, ['tea']);
    expect(find.text('The trash is empty.'), findsOneWidget);
    await tester.tap(find.text('Close'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('profile-tea')), findsOneWidget);
    expect(inCard('tea', find.text('Stopped')), findsOneWidget);
  });

  testWidgets('a name that is taken again asks for another one and replaces nothing', (
    tester,
  ) async {
    final fake = FakeCli(entries: [entryOf(docOf('tea'))]);
    await open(tester, fake);
    await delete(tester, 'tea');
    // Inzwischen gibt es wieder ein Profil dieses Namens.
    fake.entries = [entryOf(docOf('tea'))];
    await tester.pumpWidget(DeskApp(cli: fake, refreshEvery: null, today: () => '2026-11-23'));
    await tester.pumpAndSettle();

    await openTrash(tester);
    final id = fake.trashed.single.id;
    await tester.tap(find.byKey(ValueKey('put-back-$id')));
    await tester.pumpAndSettle();
    expect(find.textContaining('There is a profile named "tea" again'), findsOneWidget);
    final field = find.descendant(
      of: find.byKey(const ValueKey('put-back-name')),
      matching: find.byType(EditableText),
    );
    expect(tester.widget<EditableText>(field).controller.text, 'tea-restored');
    await tester.tap(find.byKey(const ValueKey('put-back-confirm')));
    await tester.pumpAndSettle();

    expect(fake.restored, ['tea-restored']);
    await tester.tap(find.text('Close'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('profile-tea')), findsOneWidget);
    expect(find.byKey(const ValueKey('profile-tea-restored')), findsOneWidget);
  });

  testWidgets('erasing asks first, says that it cannot be undone, and then is gone', (
    tester,
  ) async {
    final fake = FakeCli(entries: [entryOf(docOf('study')), entryOf(docOf('tea'))]);
    await open(tester, fake);
    await delete(tester, 'tea');
    await openTrash(tester);
    final id = fake.trashed.single.id;

    await tester.tap(find.byKey(ValueKey('erase-$id')));
    await tester.pumpAndSettle();
    expect(find.text('Erase tea for good?'), findsOneWidget);
    expect(find.textContaining('This cannot be undone'), findsOneWidget);
    expect(find.textContaining('log file of this name stays'), findsOneWidget);
    // Abbrechen löscht nichts.
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(fake.erased, isEmpty);
    expect(find.byKey(ValueKey('trash-$id')), findsOneWidget);

    await tester.tap(find.byKey(ValueKey('erase-$id')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('erase-confirm')));
    await tester.pumpAndSettle();
    expect(fake.erased, [id]);
    expect(find.text('The trash is empty.'), findsOneWidget);
    await tester.tap(find.text('Close'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('profile-tea')), findsNothing);
  });

  testWidgets('a profile put back while the fish is on is taken up at once', (tester) async {
    final fake = FakeCli(entries: [entryOf(docOf('study')), entryOf(docOf('tea'))]);
    await open(tester, fake);
    await delete(tester, 'tea');
    await tester.tap(find.byKey(const ValueKey('fish-switch')));
    await tester.pumpAndSettle();
    fake.lastRun!.emit('rusty-fish: [study] serving');
    await tester.pumpAndSettle();

    await openTrash(tester);
    await tester.tap(find.byKey(ValueKey('put-back-${fake.trashed.single.id}')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Close'));
    await tester.pumpAndSettle();
    expect(fake.lastRun!.said, contains('start tea'));
    expect(inCard('tea', find.text('Starting')), findsOneWidget);
  });

  testWidgets('a program that does not know the trash says so in its own words', (tester) async {
    final fake = FakeCli(entries: [entryOf(docOf('study'))])
      ..trashError = "error: unrecognized subcommand 'trash'";
    await open(tester, fake);
    await openTrash(tester);
    expect(find.textContaining("unrecognized subcommand 'trash'"), findsOneWidget);
    expect(find.text('The trash is empty.'), findsNothing);
  });
}
