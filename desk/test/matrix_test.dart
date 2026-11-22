// Tests der Maske zur zweiten Tür, Matrix, gegen die Attrappe: das Konto des
// Fischs im Secrets-Fenster, die Tür eines Profils und die Anzeige in der
// Titelleiste. Kein Programm, kein Matrix, kein Konto.

import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:rusty_fish_desk/main.dart';
import 'package:rusty_fish_desk/model.dart';
import 'package:rusty_fish_desk/share.dart';

import 'fake_cli.dart';

const today = '2026-11-22';

/// Breiter als in den anderen Tests (dort 1400). Die Schrift der Tests ist
/// viel breiter als die echte, und mit der Anzeige zu Matrix stehen drei
/// Anzeigen in der Titelleiste. Gemessen am 22.11.2026 mit der echten Schrift
/// von Windows: die Titelleiste braucht dann in ihrer längsten Form etwa 1080
/// Punkte, und das Fenster öffnet mit 1280.
Future<void> open(WidgetTester tester, FakeCli fake, {double width = 1700}) async {
  tester.view.physicalSize = Size(width, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(DeskApp(cli: fake, refreshEvery: null, today: () => today));
  await tester.pumpAndSettle();
}

Finder inCard(String profile, Finder what) =>
    find.descendant(of: find.byKey(ValueKey('profile-$profile')), matching: what);

String textOf(WidgetTester tester, Key key) {
  final field = find.descendant(of: find.byKey(key), matching: find.byType(EditableText));
  return tester.widget<EditableText>(field).controller.text;
}

Future<void> choose(WidgetTester tester, String key, String option) async {
  await tester.ensureVisible(find.byKey(ValueKey(key)));
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(ValueKey(key)));
  await tester.pumpAndSettle();
  await tester.tap(find.text(option).last);
  await tester.pumpAndSettle();
}

/// Ein Profil mit zwei Tischen in Matrix-Räumen.
ProfileDoc matrixDoc(String name) => ProfileDoc(
  name: name,
  model: 'some-model',
  baseUrl: null,
  host: 'Nemo',
  aiNames: const ['Fish'],
  limits: const Limits(translationTokensPerDay: 100000, questionsPerGuestPerDay: 10),
  rooms: const [
    Room(
      passThrough: ['English'],
      contextLines: 10,
      seats: [
        Seat(door: matrixDoor, channel: '!deutsch:example.org', lang: 'German', owner: 'Nemo'),
        Seat(door: matrixDoor, channel: '!russisch:example.org', lang: 'Russian', owner: 'Robin'),
      ],
    ),
  ],
  people: const [
    Person(name: 'Nemo', matrix: '@nemo:example.org', lang: 'German'),
    Person(name: 'Robin', matrix: '@robin:example.org', id: '7', lang: 'Russian', invited: true),
  ],
);

void main() {
  group('what goes to the program and back', () {
    test('a person carries an id at each door, and only what is there goes out', () {
      const both = Person(name: 'Robin', id: '7', matrix: '@robin:example.org');
      expect(both.toJson()['discord'], '7');
      expect(both.toJson()['matrix'], '@robin:example.org');
      expect(both.idAt(discordDoor), '7');
      expect(both.idAt(matrixDoor), '@robin:example.org');
      final back = Person.fromJson(both.toJson());
      expect(back.id, '7');
      expect(back.matrix, '@robin:example.org');
      // Ein älteres Programm lehnt Felder ab, die es nicht kennt.
      expect(const Person(id: '7').toJson().containsKey('matrix'), isFalse);
      // Was das Programm mit `null` nennt, ist keine Kennung.
      expect(Person.fromJson({'name': 'Robin', 'discord': null, 'matrix': null}).matrix, '');
    });

    test('a profile points at a person by name, else by the id they have', () {
      expect(const Person(name: 'Robin', matrix: '@robin:example.org').ref, 'Robin');
      expect(const Person(matrix: '@robin:example.org').ref, '@robin:example.org');
      expect(const Person(id: '7', matrix: '@robin:example.org').ref, '7');
      final doc = matrixDoc('tea');
      expect(doc.person('@robin:example.org')?.name, 'Robin');
      expect(doc.person('7')?.name, 'Robin');
      expect(doc.isHost(doc.people.first), isTrue);
      expect(doc.isHost(doc.people.last), isFalse);
    });

    test('the door of a profile is the door of its tables', () {
      expect(matrixDoc('tea').door, matrixDoor);
      expect(docOf('study').door, discordDoor);
      expect(ProfileDoc.blank().door, discordDoor);
      final json = matrixDoc('tea').toJson();
      final seats = (json['rooms'] as List).first['seats'] as List;
      expect(seats.map((seat) => seat['door']), ['matrix', 'matrix']);
      expect(ProfileDoc.fromJson(json).toJson(), json);
    });

    test('the card says that a Matrix room is encrypted only up to the fish', () {
      expect(
        connectionText(matrixDoc('tea').rooms.single),
        'Matrix ⇄ Matrix · end-to-end encrypted up to the fish',
      );
    });

    test('the status of the Matrix account names the account and no secret', () {
      final status = MatrixStatus.fromJson({
        'logged_in': true,
        'user': '@fish:example.org',
        'homeserver': 'https://matrix.example.org/',
      });
      expect(status.loggedIn, isTrue);
      expect(status.user, '@fish:example.org');
      final out = MatrixStatus.fromJson({'logged_in': false, 'user': null, 'homeserver': null});
      expect(out.loggedIn, isFalse);
      expect(out.user, isNull);
      // Ein älteres Programm nennt den Eintrag nicht: dann ist er nicht angemeldet.
      expect(const KeyStatus({'discord': true}).matrix, isFalse);
      expect(const KeyStatus({'matrix': true}).matrix, isTrue);
    });

    test('shared rooms carry their door and the Matrix ids, never an invitation', () {
      final text = shareRoomsText(matrixDoc('tea'));
      expect(text, contains('"door": "matrix"'));
      expect(text, contains('@robin:example.org'));
      final read = readSharedRooms(text)!;
      expect(read.rooms.single.seats.map((seat) => seat.door), [matrixDoor, matrixDoor]);
      expect(read.people.last.matrix, '@robin:example.org');
      expect(read.people.last.id, '7');
      expect(read.people.every((person) => !person.invited), isTrue);
    });
  });

  group('the Matrix account of the fish', () {
    testWidgets('logging in sends the user id and the password once and shows neither', (
      tester,
    ) async {
      final fake = FakeCli(entries: [entryOf(docOf('study'))]);
      await open(tester, fake);
      // Das Konto hat sein eigenes Fenster, hinter seiner eigenen Anzeige.
      // Die anderen Geheimnisse stehen dort nicht.
      await tester.tap(find.byKey(const ValueKey('chip-matrix')));
      await tester.pumpAndSettle();
      expect(find.text('Matrix account'), findsOneWidget);
      expect(find.byKey(const ValueKey('secret-row-discord')), findsNothing);
      expect(find.byKey(const ValueKey('secret-row-openai')), findsNothing);
      expect(find.text('not logged in'), findsOneWidget);

      const password = 'not-a-real-password';
      await tester.ensureVisible(find.byKey(const ValueKey('matrix-user')));
      await tester.enterText(find.byKey(const ValueKey('matrix-user')), ' @fish:example.org ');
      await tester.enterText(find.byKey(const ValueKey('matrix-password')), password);
      await tester.tap(find.byKey(const ValueKey('matrix-login')));
      await tester.pumpAndSettle();

      expect(fake.matrixLogins, [(user: '@fish:example.org', password: password)]);
      // Ohne Wiederherstellungsschlüssel geht keiner mit.
      expect(fake.matrixRecoveries, ['']);
      expect(find.text('logged in as @fish:example.org'), findsOneWidget);
      // Das Passwort steht danach nirgends mehr im Fenster.
      expect(find.textContaining('not-a-real'), findsNothing);
      for (final field in tester.widgetList<EditableText>(find.byType(EditableText))) {
        expect(field.controller.text, isNot(contains('not-a-real')));
      }
      // Was das Programm dazu anmerkt, steht da.
      expect(find.textContaining('could not be cross-signed'), findsOneWidget);
      // Die Felder zum Anmelden sind fort, dafür gibt es das Abmelden.
      expect(find.byKey(const ValueKey('matrix-login')), findsNothing);
      expect(find.byKey(const ValueKey('matrix-logout')), findsOneWidget);

      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
      expect(find.text('Matrix: logged in'), findsOneWidget);
    });

    testWidgets('a recovery key goes along with the login once and is shown nowhere', (
      tester,
    ) async {
      final fake = FakeCli(entries: [entryOf(docOf('study'))]);
      await open(tester, fake);
      await tester.tap(find.byKey(const ValueKey('chip-matrix')));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.byKey(const ValueKey('matrix-user')));
      await tester.enterText(find.byKey(const ValueKey('matrix-user')), '@fish:example.org');
      await tester.enterText(find.byKey(const ValueKey('matrix-password')), 'not-a-real-password');
      await tester.enterText(find.byKey(const ValueKey('matrix-recovery')), ' not a real key ');
      // Verdeckt wie das Passwort.
      expect(
        tester.widget<TextField>(find.byKey(const ValueKey('matrix-recovery'))).obscureText,
        isTrue,
      );
      await tester.tap(find.byKey(const ValueKey('matrix-login')));
      await tester.pumpAndSettle();

      expect(fake.matrixRecoveries, ['not a real key']);
      expect(find.textContaining('not a real key'), findsNothing);
      for (final field in tester.widgetList<EditableText>(find.byType(EditableText))) {
        expect(field.controller.text, isNot(contains('not a real')));
      }
    });

    testWidgets('a login the program refuses says why and stores nothing', (tester) async {
      final fake = FakeCli(entries: [entryOf(docOf('study'))])
        ..matrixLoginError = 'the Matrix login failed: wrong password';
      await open(tester, fake);
      await tester.tap(find.byKey(const ValueKey('chip-matrix')));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.byKey(const ValueKey('matrix-user')));
      await tester.enterText(find.byKey(const ValueKey('matrix-user')), '@fish:example.org');
      await tester.enterText(find.byKey(const ValueKey('matrix-password')), 'not-a-real-password');
      await tester.tap(find.byKey(const ValueKey('matrix-login')));
      await tester.pumpAndSettle();

      expect(find.text('the Matrix login failed: wrong password'), findsOneWidget);
      expect(find.text('not logged in'), findsOneWidget);
      expect(textOf(tester, const ValueKey('matrix-password')), isEmpty);
      // Die Kennung bleibt stehen: sie ist kein Geheimnis, und man tippt sie
      // nicht gern zweimal.
      expect(textOf(tester, const ValueKey('matrix-user')), '@fish:example.org');
    });

    testWidgets('without a user id or a password nothing is sent', (tester) async {
      final fake = FakeCli(entries: [entryOf(docOf('study'))]);
      await open(tester, fake);
      await tester.tap(find.byKey(const ValueKey('chip-matrix')));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.byKey(const ValueKey('matrix-login')));
      await tester.tap(find.byKey(const ValueKey('matrix-login')));
      await tester.pumpAndSettle();
      expect(fake.matrixLogins, isEmpty);
      expect(find.text('Enter the user ID of the fish and its password.'), findsOneWidget);
    });

    testWidgets('logging out asks first and says what is lost', (tester) async {
      final fake = FakeCli(
        entries: [entryOf(matrixDoc('tea'))],
        keys: const KeyStatus({'openai': true, 'discord': true, 'matrix': true}),
      )..matrix = const MatrixStatus(loggedIn: true, user: '@fish:example.org');
      await open(tester, fake);
      await tester.tap(find.text('Matrix: logged in'));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.byKey(const ValueKey('matrix-logout')));
      await tester.tap(find.byKey(const ValueKey('matrix-logout')));
      await tester.pumpAndSettle();
      expect(fake.matrixLogouts, 0);
      expect(find.text('Log the fish out of Matrix?'), findsOneWidget);
      expect(find.textContaining('cannot read what was written before'), findsOneWidget);

      await tester.tap(find.widgetWithText(FilledButton, 'Log out'));
      await tester.pumpAndSettle();
      expect(fake.matrixLogouts, 1);
      expect(find.text('not logged in'), findsOneWidget);
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
      // Ein Profil steht an Matrix, und der Fisch ist dort nicht angemeldet:
      // jetzt fehlt etwas, und die Titelleiste sagt es.
      expect(find.text('Matrix: not logged in'), findsOneWidget);
    });

    testWidgets('an older program that knows no Matrix leaves the window usable', (tester) async {
      final fake = FakeCli(entries: [entryOf(docOf('study'))])
        ..matrixUnknown = "unrecognized subcommand 'matrix'";
      await open(tester, fake);
      await tester.tap(find.byKey(const ValueKey('chip-matrix')));
      await tester.pumpAndSettle();
      expect(find.text('this program does not know Matrix'), findsOneWidget);
      expect(find.byKey(const ValueKey('matrix-login')), findsNothing);
      // Kein Fehler des Fensters: die anderen Geheimnisse gehen weiter.
      expect(find.textContaining('unrecognized'), findsNothing);
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('chip-bot-token')));
      await tester.pumpAndSettle();
      expect(find.textContaining('unrecognized'), findsNothing);
      expect(find.byKey(const ValueKey('store-discord')), findsOneWidget);
    });
  });

  group('the door of a profile', () {
    testWidgets('a Matrix profile opens with its rooms and ids and goes back out unchanged', (
      tester,
    ) async {
      final fake = FakeCli(entries: [entryOf(matrixDoc('tea'))]);
      await open(tester, fake);
      expect(
        inCard('tea', find.textContaining('Matrix ⇄ Matrix · end-to-end encrypted up to the fish')),
        findsOneWidget,
      );
      await tester.tap(inCard('tea', find.text('Edit')));
      await tester.pumpAndSettle();

      expect(find.text('Your Matrix user ID'), findsOneWidget);
      expect(find.text('Matrix room ID'), findsNWidgets(2));
      expect(find.text('Matrix user ID'), findsNWidgets(2));
      expect(find.text('Discord channel ID'), findsNothing);
      expect(textOf(tester, const ValueKey('seat-0-1-channel')), '!russisch:example.org');
      expect(textOf(tester, const ValueKey('person-1-id')), '@robin:example.org');

      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      // Auch die Discord-Kennung, die in keinem Feld stand, ist noch da.
      expect(fake.saved.single.toJson(), matrixDoc('tea').toJson());
    });

    testWidgets('choosing Matrix renames the fields and saves every table at that door', (
      tester,
    ) async {
      final fake = FakeCli(
        entries: [
          entryOf(
            docOf(
              'study',
              people: const [Person(name: 'Robin', id: '200', lang: 'Russian')],
            ),
          ),
        ],
      );
      await open(tester, fake);
      await tester.tap(inCard('study', find.text('Edit')));
      await tester.pumpAndSettle();
      expect(find.text('Your Discord user ID'), findsOneWidget);
      expect(textOf(tester, const ValueKey('person-0-id')), '200');

      await choose(tester, 'field-door', 'Matrix');
      expect(find.text('Your Matrix user ID'), findsOneWidget);
      expect(find.text('Matrix room ID'), findsNWidgets(2));
      // An der neuen Tür hat Robin noch keine Kennung.
      expect(textOf(tester, const ValueKey('person-0-id')), isEmpty);

      await tester.enterText(find.byKey(const ValueKey('field-host')), '@nemo:example.org');
      await tester.ensureVisible(find.byKey(const ValueKey('seat-0-0-channel')));
      await tester.enterText(find.byKey(const ValueKey('seat-0-0-channel')), '!a:example.org');
      await tester.enterText(find.byKey(const ValueKey('seat-0-1-channel')), '!b:example.org');
      await tester.ensureVisible(find.byKey(const ValueKey('person-0-id')));
      await tester.enterText(find.byKey(const ValueKey('person-0-id')), '@robin:example.org');
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();

      final doc = fake.saved.single;
      expect(doc.door, matrixDoor);
      expect(doc.rooms.single.seats.map((seat) => seat.door), [matrixDoor, matrixDoor]);
      expect(doc.rooms.single.seats.map((seat) => seat.channel), ['!a:example.org', '!b:example.org']);
      expect(doc.host, '@nemo:example.org');
      // Robin trägt jetzt an beiden Türen eine Kennung; die von Discord ging
      // beim Wechsel nicht verloren.
      expect(doc.people.single.matrix, '@robin:example.org');
      expect(doc.people.single.id, '200');
    });

    testWidgets('switching the door there and back changes nothing', (tester) async {
      final fake = FakeCli(entries: [entryOf(tablesDoc('pair'))]);
      await open(tester, fake);
      await tester.tap(inCard('pair', find.text('Edit')));
      await tester.pumpAndSettle();
      await choose(tester, 'field-door', 'Matrix');
      await choose(tester, 'field-door', 'Discord');
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(fake.saved.single.toJson(), tablesDoc('pair').toJson());
    });

    testWidgets('a new profile starts at Discord, as before', (tester) async {
      final fake = FakeCli(entries: []);
      await open(tester, fake);
      await tester.tap(find.text('New profile'));
      await tester.pumpAndSettle();
      expect(find.text('Your Discord user ID'), findsOneWidget);
      expect(find.text('Discord channel ID'), findsNWidgets(2));
    });
  });

  group('the title bar', () {
    // Seit dem 23.11.2026 hat jedes Geheimnis sein eigenes Fenster, und die
    // Anzeige ist der Weg dorthin. Sie steht also immer da; solange niemand
    // Matrix benutzt, sagt sie nicht, dass etwas fehlt.
    testWidgets('leads to the Matrix account without warning while nobody uses it', (
      tester,
    ) async {
      await open(tester, FakeCli(entries: [entryOf(docOf('study'))]));
      expect(find.byKey(const ValueKey('chip-matrix')), findsOneWidget);
      expect(find.textContaining('Matrix:'), findsNothing);
      await tester.tap(find.byKey(const ValueKey('chip-matrix')));
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('matrix-login')), findsOneWidget);
    });

    testWidgets('warns when a profile stands at Matrix and the fish is not logged in', (
      tester,
    ) async {
      await open(tester, FakeCli(entries: [entryOf(matrixDoc('tea'))]));
      expect(find.text('Matrix: not logged in'), findsOneWidget);
    });

    testWidgets('shows the login even before any profile uses it', (tester) async {
      final fake = FakeCli(
        entries: [entryOf(docOf('study'))],
        keys: const KeyStatus({'openai': true, 'discord': true, 'matrix': true}),
      );
      await open(tester, fake);
      expect(find.text('Matrix: logged in'), findsOneWidget);
    });
  });
}
