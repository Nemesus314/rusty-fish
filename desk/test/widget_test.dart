// Tests der Maske gegen die Attrappe. Zuerst geschrieben, gegen ein leeres
// Gerüst gescheitert, dann die Maske dazu gebaut.

import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:rusty_fish_desk/cli.dart';
import 'package:rusty_fish_desk/main.dart';
import 'package:rusty_fish_desk/model.dart';

import 'fake_cli.dart';

const today = '2026-11-20';

Future<void> open(WidgetTester tester, FakeCli fake) async {
  // Ein Fenster in der Größe eines gewöhnlichen Desktops, damit die Karten
  // so nebeneinander stehen wie im echten Programm.
  tester.view.physicalSize = const Size(1400, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    DeskApp(cli: fake, refreshEvery: null, today: () => today),
  );
  await tester.pumpAndSettle();
}

Finder inCard(String profile, Finder what) {
  return find.descendant(
    of: find.byKey(ValueKey('profile-$profile')),
    matching: what,
  );
}

String textOf(WidgetTester tester, Key key) {
  final field = find.descendant(
    of: find.byKey(key),
    matching: find.byType(EditableText),
  );
  return tester.widget<EditableText>(field).controller.text;
}

FakeCli threeProfiles() {
  return FakeCli(
    entries: [
      entryOf(
        docOf(
          'tea',
          aiNames: ['Fish', 'Фиш'],
          people: const [
            Person(id: '100', gender: Gender.male),
            Person(id: '200', gender: Gender.female),
          ],
        ),
        usage: const Usage(
          day: today,
          translationTokens: 3231,
          allowedToday: 100000,
        ),
        running: true,
      ),
      entryOf(
        docOf('study'),
        usage: const Usage(
          day: '2026-11-18',
          translationTokens: 999,
          allowedToday: 100000,
        ),
      ),
      brokenEntry('old', 'configuration: `lang` is empty for channel 222'),
    ],
  );
}

void main() {
  startRequests();

  testWidgets('the overview shows a valid, a running and a broken profile', (
    tester,
  ) async {
    await open(tester, threeProfiles());

    expect(inCard('tea', find.text('tea')), findsOneWidget);
    expect(inCard('tea', find.text('German ⇄ Russian')), findsOneWidget);
    expect(inCard('tea', find.textContaining('111')), findsOneWidget);
    expect(inCard('tea', find.textContaining('222')), findsOneWidget);
    expect(inCard('tea', find.text('2 people listed')), findsOneWidget);
    expect(inCard('tea', find.text('AI answers to Fish, Фиш')), findsOneWidget);
    // Nemesus am 20.11.2026: in der Übersicht war nicht zu sehen, welche
    // Sprache unübersetzt bleibt.
    expect(inCard('tea', find.text('English stays untranslated')), findsOneWidget);
    expect(
      inCard('tea', find.text('3,231 of 100,000 tokens today')),
      findsOneWidget,
    );
    // Dieses Profil läuft, wurde aber nicht von diesem Fenster gestartet.
    expect(inCard('tea', find.text('Running')), findsOneWidget);
    expect(inCard('tea', find.textContaining('outside this window')), findsOneWidget);

    expect(inCard('study', find.text('Stopped')), findsOneWidget);
    expect(inCard('study', find.text('translator only')), findsOneWidget);
    expect(inCard('study', find.text('no people listed')), findsOneWidget);
    // Der gespeicherte Verbrauch ist von einem früheren Tag.
    expect(inCard('study', find.text('not used today')), findsOneWidget);
    expect(inCard('study', find.text('Start')), findsOneWidget);

    expect(inCard('old', find.text('Invalid')), findsOneWidget);
    expect(
      inCard('old', find.textContaining('`lang` is empty for channel 222')),
      findsOneWidget,
    );
    // Ein kaputtes Profil lässt sich weder starten noch in der Maske öffnen.
    expect(inCard('old', find.text('Start')), findsNothing);
    expect(inCard('old', find.text('Edit')), findsNothing);
    expect(inCard('old', find.text('Delete')), findsOneWidget);

    expect(find.text('The fish runs while this window is open.'), findsOneWidget);
    expect(find.textContaining(r'C:\fish\profiles'), findsWidgets);
  });

  testWidgets('the two secrets show only whether they are stored', (
    tester,
  ) async {
    await open(tester, threeProfiles());
    expect(find.text('AI key: stored'), findsOneWidget);
    expect(find.text('Bot token: not stored'), findsOneWidget);
  });

  testWidgets('a refused save shows the reason and keeps what was typed', (
    tester,
  ) async {
    final fake = threeProfiles();
    await open(tester, fake);

    await tester.tap(inCard('study', find.text('Edit')));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('field-model')),
      'my-new-model',
    );
    fake.saveError = 'configuration: `model` is not known to the provider';
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();

    expect(
      find.textContaining('`model` is not known to the provider'),
      findsOneWidget,
    );
    expect(find.text('Save'), findsOneWidget, reason: 'the form stays open');
    expect(textOf(tester, const ValueKey('field-model')), 'my-new-model');
    expect(fake.saved, isEmpty);

    // Nach der Korrektur geht dasselbe Formular durch und schließt sich.
    fake.saveError = null;
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();
    expect(fake.saved.single.model, 'my-new-model');
    expect(fake.saved.single.name, 'study');
    expect(find.text('Save'), findsNothing);
    expect(inCard('study', find.text('study')), findsOneWidget);
  });

  testWidgets('saving sends every part of the profile back unchanged', (
    tester,
  ) async {
    final fake = threeProfiles();
    await open(tester, fake);

    await tester.tap(inCard('study', find.text('Edit')));
    await tester.pumpAndSettle();
    // Der Name eines bestehenden Profils ist fest.
    expect(textOf(tester, const ValueKey('field-name')), 'study');
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();

    final doc = fake.saved.single;
    expect(doc.toJson(), docOf('study').toJson());
  });

  testWidgets('a new profile starts with one room of two seats', (tester) async {
    final fake = threeProfiles();
    await open(tester, fake);

    await tester.tap(find.text('New profile'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const ValueKey('field-name')), 'fresh');
    await tester.enterText(find.byKey(const ValueKey('field-model')), 'm');
    await tester.enterText(find.byKey(const ValueKey('field-host')), '42');
    await tester.enterText(find.byKey(const ValueKey('seat-0-0-channel')), '7');
    await tester.enterText(find.byKey(const ValueKey('seat-0-0-lang')), 'German');
    await tester.enterText(find.byKey(const ValueKey('seat-0-1-channel')), '8');
    await tester.enterText(find.byKey(const ValueKey('seat-0-1-lang')), 'Korean');
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();

    final doc = fake.saved.single;
    expect(doc.name, 'fresh');
    expect(doc.host, '42');
    expect(doc.aiNames, isEmpty);
    expect(doc.rooms.single.seats.map((s) => '${s.channel}:${s.lang}'), [
      '7:German',
      '8:Korean',
    ]);
    expect(doc.rooms.single.seats.every((s) => s.gender == null), isTrue);
    // Entschieden von Nemesus am 20.11.2026 („Ja, vorbelegen"): ein neues
    // Profil bringt Englisch als neutrale Sprache mit. Es bleibt ein Feld,
    // das man leeren kann.
    expect(doc.rooms.single.passThrough, ['English']);
  });

  testWidgets('a name the program would refuse is flagged while typing', (
    tester,
  ) async {
    final fake = threeProfiles();
    await open(tester, fake);

    await tester.tap(find.text('New profile'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const ValueKey('field-name')), 'Tea Time');
    // Abwarten, bis der Hinweis den Hilfetext ganz abgelöst hat.
    await tester.pumpAndSettle();
    expect(find.textContaining('Only a-z, 0-9'), findsOneWidget);
    expect(find.textContaining('are possible'), findsOneWidget);

    // Ein Name, den es schon gibt, würde ein Profil ersetzen.
    await tester.enterText(find.byKey(const ValueKey('field-name')), 'study');
    await tester.pumpAndSettle();
    expect(find.text('A profile named study already exists.'), findsOneWidget);
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();
    expect(fake.saved, isEmpty);
  });

  testWidgets('duplicating opens a copy under a new name', (tester) async {
    final fake = threeProfiles();
    await open(tester, fake);

    await tester.tap(inCard('study', find.text('Duplicate')));
    await tester.pumpAndSettle();
    expect(textOf(tester, const ValueKey('field-name')), 'study-copy');
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();

    expect(fake.saved.single.name, 'study-copy');
    expect(fake.saved.single.rooms.single.seats.length, 2);
    expect(inCard('study', find.text('study')), findsOneWidget);
    expect(inCard('study-copy', find.text('study-copy')), findsOneWidget);
  });

  testWidgets('deleting asks first and says where the profile goes', (
    tester,
  ) async {
    final fake = threeProfiles();
    await open(tester, fake);

    await tester.tap(inCard('study', find.text('Delete')));
    await tester.pumpAndSettle();
    expect(find.textContaining('trash'), findsWidgets);
    expect(fake.deleted, isEmpty);

    await tester.tap(find.text('Move to trash'));
    await tester.pumpAndSettle();
    expect(fake.deleted, ['study']);
    expect(find.byKey(const ValueKey('profile-study')), findsNothing);
  });

  testWidgets('a stored secret is nowhere in the window afterwards', (
    tester,
  ) async {
    final fake = threeProfiles();
    await open(tester, fake);
    const secret = 'tok-very-secret-123';

    await tester.tap(find.text('Bot token: not stored'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const ValueKey('secret-discord')), secret);
    await tester.tap(find.byKey(const ValueKey('store-discord')));
    await tester.pumpAndSettle();

    expect(fake.secrets[SecretKind.discord], secret);
    expect(find.textContaining('very-secret'), findsNothing);
    for (final field in tester.widgetList<EditableText>(find.byType(EditableText))) {
      expect(field.controller.text, isNot(contains('very-secret')));
    }
    expect(find.text('Bot token: stored'), findsOneWidget);
  });

  testWidgets('forgetting a secret asks first', (tester) async {
    final fake = threeProfiles();
    await open(tester, fake);

    await tester.tap(find.text('AI key: stored'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('forget-openai')));
    await tester.pumpAndSettle();
    expect(fake.forgotten, isEmpty);

    await tester.tap(find.text('Forget it'));
    await tester.pumpAndSettle();
    expect(fake.forgotten, [SecretKind.openai]);
    expect(find.text('AI key: not stored'), findsOneWidget);
  });

  testWidgets('starting and stopping a profile, with its log', (tester) async {
    final fake = FakeCli(entries: [entryOf(docOf('study')), entryOf(docOf('other'))]);
    await open(tester, fake);

    await tester.tap(inCard('study', find.text('Start')));
    await tester.pumpAndSettle();
    expect(fake.started, ['study']);
    expect(inCard('study', find.text('Running')), findsOneWidget);
    expect(inCard('study', find.text('Stop')), findsOneWidget);
    // Ein Bot, ein Fisch: das andere Profil lässt sich solange nicht starten.
    final otherStart = tester.widget<ButtonStyleButton>(
      inCard('other', find.ancestor(of: find.text('Start'), matching: find.bySubtype<ButtonStyleButton>())),
    );
    expect(otherStart.onPressed, isNull);

    fake.lastRun!.emit('rusty-fish: connected to Discord as Rusty Fish');
    await tester.pumpAndSettle();
    await tester.tap(inCard('study', find.textContaining('Log')));
    await tester.pumpAndSettle();
    expect(find.textContaining('connected to Discord as Rusty Fish'), findsOneWidget);

    await tester.tap(inCard('study', find.text('Stop')));
    await tester.pumpAndSettle();
    expect(fake.lastRun!.stopped, isTrue);
    expect(inCard('study', find.text('Stopped')), findsOneWidget);
    expect(inCard('study', find.text('Start')), findsOneWidget);
  });

  testWidgets('a fish that ends on its own says why', (tester) async {
    final fake = FakeCli(entries: [entryOf(docOf('study'))]);
    await open(tester, fake);

    await tester.tap(inCard('study', find.text('Start')));
    await tester.pumpAndSettle();
    fake.lastRun!.emit('rusty-fish: Discord bot token not stored');
    await tester.pump();
    fake.lastRun!.end(1);
    await tester.pumpAndSettle();

    expect(inCard('study', find.text('Stopped')), findsOneWidget);
    expect(find.textContaining('Discord bot token not stored'), findsWidgets);
  });

  testWidgets('a start the program refuses says why', (tester) async {
    final fake = FakeCli(entries: [entryOf(docOf('study'))]);
    fake.runError = 'another profile is already running';
    await open(tester, fake);

    await tester.tap(inCard('study', find.text('Start')));
    await tester.pumpAndSettle();
    expect(find.textContaining('another profile is already running'), findsOneWidget);
    expect(inCard('study', find.text('Stopped')), findsOneWidget);
  });

  testWidgets('loading the models offers them for the model field', (
    tester,
  ) async {
    final fake = threeProfiles();
    await open(tester, fake);

    await tester.tap(inCard('study', find.text('Edit')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Load models'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('beta-model'));
    await tester.pumpAndSettle();
    expect(textOf(tester, const ValueKey('field-model')), 'beta-model');
  });

  testWidgets('without the program the window says where it looked', (
    tester,
  ) async {
    await tester.pumpWidget(
      const DeskApp(
        cli: null,
        refreshEvery: null,
        location: FishLocation(
          found: null,
          checked: [r'X:\app\rusty-fish.exe', r'X:\target\debug\rusty-fish.exe'],
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.textContaining('rusty-fish was not found'), findsOneWidget);
    expect(find.text(r'X:\app\rusty-fish.exe'), findsOneWidget);
    expect(find.text(r'X:\target\debug\rusty-fish.exe'), findsOneWidget);
  });
}

// Startargumente: das Fenster kann gleich mit einer bestimmten Ansicht aufgehen.
void startRequests() {
  testWidgets('the window can open straight into editing a profile', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1400, 1000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
      DeskApp(
        cli: threeProfiles(),
        refreshEvery: null,
        today: () => today,
        startWith: const StartRequest.edit('study'),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Edit study'), findsOneWidget);
    expect(textOf(tester, const ValueKey('field-name')), 'study');
  });

  test('start arguments are read, anything else opens the overview', () {
    expect(StartRequest.fromArgs(const []), isNull);
    expect(StartRequest.fromArgs(const ['--whatever']), isNull);
    expect(StartRequest.fromArgs(const ['--edit']), isNull);
    expect(StartRequest.fromArgs(const ['--edit', 'tea'])!.profile, 'tea');
    expect(StartRequest.fromArgs(const ['--new'])!.newProfile, isTrue);
    expect(StartRequest.fromArgs(const ['--secrets'])!.secrets, isTrue);
  });
}
