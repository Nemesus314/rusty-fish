// Export und Import: alle Profile samt Bot-Token und KI-Schlüsseln in einer
// Datei mit Kennwort. Die Maske fragt und zeigt; was geschieht, entscheidet
// das Programm, hier die Attrappe.

import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:rusty_fish_desk/envelope.dart';
import 'package:rusty_fish_desk/main.dart';
import 'package:rusty_fish_desk/model.dart';

import 'fake_cli.dart';

/// Steht anstelle des Dialogs des Systems: gibt zurück, was der Mensch
/// gewählt hätte, und merkt sich, wonach gefragt wurde.
class FakeFiles implements FileChooser {
  FakeFiles({this.save, this.chosen});

  /// Der Ort, den der Mensch zum Speichern wählt; `null` heißt abgebrochen.
  String? save;

  /// Die Datei, die der Mensch zum Öffnen wählt; `null` heißt abgebrochen.
  String? chosen;

  /// Die Namen, die die Maske vorgeschlagen hat.
  final suggested = <String>[];
  int opened = 0;

  @override
  Future<String?> saveAs(String suggestedName) async {
    suggested.add(suggestedName);
    return save;
  }

  @override
  Future<String?> open() async {
    opened++;
    return chosen;
  }
}

Future<void> open(WidgetTester tester, FakeCli fake, FakeFiles files) async {
  tester.view.physicalSize = const Size(1500, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    DeskApp(
      cli: fake,
      refreshEvery: null,
      today: () => '2026-11-25',
      now: () => DateTime(2026, 11, 25, 21, 30),
      files: files,
    ),
  );
  await tester.pumpAndSettle();
}

/// Tippt beide Kennwörter in das Fenster von „Export…".
Future<void> typePasswords(WidgetTester tester, String password, String again) async {
  await tester.enterText(find.byKey(const ValueKey('export-password')), password);
  await tester.enterText(find.byKey(const ValueKey('export-password-again')), again);
  await tester.pumpAndSettle();
}

String typed(WidgetTester tester, String key) =>
    tester.widget<TextField>(find.byKey(ValueKey(key))).controller!.text;

void main() {
  const password = 'a made-up password';

  group('what the window makes of names and answers', () {
    test('the suggested name carries the date of the day and the ending', () {
      expect(suggestedEnvelopeName(DateTime(2026, 11, 25, 21, 30)), 'rusty-fish-2026-11-25.rustyfish');
      expect(suggestedEnvelopeName(DateTime(2027, 1, 5)), 'rusty-fish-2027-01-05.rustyfish');
    });

    test('a name typed without an ending gets the ending, another one stays', () {
      expect(envelopePath(r'C:\Users\nemo\Desktop\umzug'), r'C:\Users\nemo\Desktop\umzug.rustyfish');
      expect(envelopePath('/home/nemo/umzug'), '/home/nemo/umzug.rustyfish');
      expect(envelopePath('/home/nemo/umzug.rustyfish'), '/home/nemo/umzug.rustyfish');
      expect(envelopePath('/home/nemo/umzug.txt'), '/home/nemo/umzug.txt');
      // Ein Punkt im Ordner ist keine Endung der Datei.
      expect(envelopePath('/home/nemo/my.files/umzug'), '/home/nemo/my.files/umzug.rustyfish');
    });

    test('a secret is called what the window calls it elsewhere', () {
      expect(secretLabel('discord', fakeProviders), 'Discord bot token');
      expect(secretLabel('openai', fakeProviders), 'ChatGPT (OpenAI) API key');
      expect(secretLabel('something-new', fakeProviders), 'something-new');
    });

    test('the answer of the program after writing is read as it comes', () {
      // Wörtlich, was `envelope write --json` am 25.11.2026 ausgab.
      final written = EnvelopeWritten.fromJson(const {
        'matrix_left_out': false,
        'profiles': 2,
        'running': false,
        'secrets': ['discord', 'openai'],
      });
      expect(written.profiles, 2);
      expect(written.secrets, ['discord', 'openai']);
      expect(written.matrixLeftOut, isFalse);
      expect(written.running, isFalse);
    });

    test('the answer of the program after reading is read as it comes', () {
      // Wörtlich, was `envelope read --json` am 25.11.2026 beim zweiten
      // Einlesen derselben Datei ausgab.
      final read = EnvelopeRead.fromJson(const {
        'created': <String>[],
        'refused': [
          {
            'name': 'abend',
            'why': 'a profile of this name already exists here; delete it first to replace it',
          },
          {
            'name': 'tee',
            'why': 'a profile of this name already exists here; delete it first to replace it',
          },
        ],
        'secrets': {
          'refused': <Object>[],
          'same': ['discord', 'openai'],
          'stored': <String>[],
        },
      });
      expect(read.created, isEmpty);
      expect(read.refused.map((r) => r.name), ['abend', 'tee']);
      expect(read.refused.first.why, contains('already exists here'));
      expect(read.same, ['discord', 'openai']);
      expect(read.stored, isEmpty);
      expect(read.secretsRefused, isEmpty);
    });
  });

  group('export', () {
    FakeCli fish() => FakeCli(entries: [entryOf(docOf('tea')), entryOf(docOf('study'))]);

    testWidgets('asks for the password twice, then where to save, and says what went in', (
      tester,
    ) async {
      final fake = fish();
      // Der Mensch tippt im Dialog einen eigenen Namen, ohne Endung.
      final files = FakeFiles(save: '/home/nemo/umzug');
      await open(tester, fake, files);
      await tester.tap(find.byKey(const ValueKey('open-export')));
      await tester.pumpAndSettle();
      // Ohne Kennwort führt nichts zum Dialog des Systems.
      expect(tester.widget<FilledButton>(find.byKey(const ValueKey('export-choose'))).onPressed, isNull);

      await typePasswords(tester, password, password);
      await tester.tap(find.byKey(const ValueKey('export-choose')));
      await tester.pumpAndSettle();

      // Der Vorschlag trägt das Datum; geschrieben wird, was gewählt wurde.
      expect(files.suggested, ['rusty-fish-2026-11-25.rustyfish']);
      final call = fake.envelopesWritten.single;
      expect(call.path, '/home/nemo/umzug.rustyfish');
      expect(call.password, password);
      expect(call.again, password);

      expect(find.text('Written to /home/nemo/umzug.rustyfish'), findsOneWidget);
      expect(find.text('In it: 2 profiles'), findsOneWidget);
      expect(find.text('In it: Discord bot token'), findsOneWidget);
      expect(find.text('In it: ChatGPT (OpenAI) API key'), findsOneWidget);
      // Das Kennwort steht danach nirgends mehr im Fenster.
      expect(find.byKey(const ValueKey('export-password')), findsNothing);
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
      expect(find.text('Export'), findsNothing);
    });

    testWidgets('passwords that differ or are too short never reach the dialog of the system', (
      tester,
    ) async {
      final fake = fish();
      final files = FakeFiles(save: '/home/nemo/umzug.rustyfish');
      await open(tester, fake, files);
      await tester.tap(find.byKey(const ValueKey('open-export')));
      await tester.pumpAndSettle();

      await typePasswords(tester, password, '$password.');
      await tester.tap(find.byKey(const ValueKey('export-choose')));
      await tester.pumpAndSettle();
      expect(find.text('The two passwords differ.'), findsOneWidget);

      await typePasswords(tester, 'elf-zeichen', 'elf-zeichen');
      await tester.tap(find.byKey(const ValueKey('export-choose')));
      await tester.pumpAndSettle();
      expect(find.text('The password needs at least 12 characters.'), findsOneWidget);

      // Sechs Umlaute sind zwölf Bytes und doch nur sechs Zeichen.
      await typePasswords(tester, 'äöüäöü', 'äöüäöü');
      await tester.tap(find.byKey(const ValueKey('export-choose')));
      await tester.pumpAndSettle();
      expect(find.text('The password needs at least 12 characters.'), findsOneWidget);

      expect(files.suggested, isEmpty);
      expect(fake.envelopesWritten, isEmpty);
    });

    testWidgets('leaving the dialog of the system writes nothing', (tester) async {
      final fake = fish();
      final files = FakeFiles();
      await open(tester, fake, files);
      await tester.tap(find.byKey(const ValueKey('open-export')));
      await tester.pumpAndSettle();
      await typePasswords(tester, password, password);
      await tester.tap(find.byKey(const ValueKey('export-choose')));
      await tester.pumpAndSettle();

      expect(files.suggested, hasLength(1));
      expect(fake.envelopesWritten, isEmpty);
      // Das Fenster steht noch da, mit dem Getippten.
      expect(typed(tester, 'export-password'), password);
    });

    testWidgets('what the program refuses is said in its words, and nothing has to be typed again', (
      tester,
    ) async {
      final fake = fish()
        ..envelopeWriteError =
            '/home/nemo/umzug.rustyfish already exists; choose another name, nothing is replaced';
      final files = FakeFiles(save: '/home/nemo/umzug.rustyfish');
      await open(tester, fake, files);
      await tester.tap(find.byKey(const ValueKey('open-export')));
      await tester.pumpAndSettle();
      await typePasswords(tester, password, password);
      await tester.tap(find.byKey(const ValueKey('export-choose')));
      await tester.pumpAndSettle();

      expect(find.textContaining('already exists; choose another name'), findsOneWidget);
      expect(typed(tester, 'export-password'), password);
      expect(typed(tester, 'export-password-again'), password);

      // Ein anderer Name, und es geht.
      fake.envelopeWriteError = null;
      files.save = '/home/nemo/umzug-2.rustyfish';
      await tester.tap(find.byKey(const ValueKey('export-choose')));
      await tester.pumpAndSettle();
      expect(find.text('Written to /home/nemo/umzug-2.rustyfish'), findsOneWidget);
    });

    testWidgets('says what is not in the file and what a running fish still learns', (tester) async {
      final fake = fish()
        ..envelopeWritten = const EnvelopeWritten(
          profiles: 1,
          secrets: ['discord'],
          matrixLeftOut: true,
          running: true,
        );
      await open(tester, fake, FakeFiles(save: '/home/nemo/umzug.rustyfish'));
      await tester.tap(find.byKey(const ValueKey('open-export')));
      await tester.pumpAndSettle();
      // Schon vor dem Schreiben: der Zugang zu Matrix reist nie mit.
      expect(find.textContaining('Not in the file: the Matrix account'), findsOneWidget);
      await typePasswords(tester, password, password);
      await tester.tap(find.byKey(const ValueKey('export-choose')));
      await tester.pumpAndSettle();

      expect(find.text('In it: 1 profile'), findsOneWidget);
      expect(find.textContaining('The Matrix account of the fish is not in it'), findsOneWidget);
      expect(find.textContaining('The fish is on.'), findsOneWidget);
    });
  });

  group('import', () {
    FakeCli fish() => FakeCli(entries: [entryOf(docOf('tea'))]);

    testWidgets('takes what the file holds and shows it; a new profile appears as a card', (
      tester,
    ) async {
      final fake = fish()
        ..envelopeRead = const EnvelopeRead(
          created: ['moved'],
          refused: [
            (
              name: 'tea',
              why: 'a profile of this name already exists here; delete it first to replace it',
            ),
          ],
          stored: ['discord'],
          same: ['openai'],
          secretsRefused: [
            (
              name: 'anthropic',
              why: 'another one is stored here already; remove it first to replace it',
            ),
          ],
        );
      final files = FakeFiles(chosen: '/home/nemo/umzug.rustyfish');
      await open(tester, fake, files);
      expect(find.byKey(const ValueKey('profile-moved')), findsNothing);
      await tester.tap(find.byKey(const ValueKey('open-import')));
      await tester.pumpAndSettle();
      // Ohne Datei und Kennwort geschieht nichts.
      expect(tester.widget<FilledButton>(find.byKey(const ValueKey('import-read'))).onPressed, isNull);
      expect(find.text('No file chosen yet.'), findsOneWidget);

      await tester.tap(find.byKey(const ValueKey('import-choose')));
      await tester.pumpAndSettle();
      expect(find.text('/home/nemo/umzug.rustyfish'), findsOneWidget);
      await tester.enterText(find.byKey(const ValueKey('import-password')), password);
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('import-read')));
      await tester.pumpAndSettle();

      final call = fake.envelopesRead.single;
      expect(call.path, '/home/nemo/umzug.rustyfish');
      expect(call.password, password);

      expect(find.text('Profiles taken: moved'), findsOneWidget);
      expect(find.text('Stored: Discord bot token'), findsOneWidget);
      expect(find.text('Here already, the same: ChatGPT (OpenAI) API key'), findsOneWidget);
      // Was hier schon lag, blieb, mit dem Grund des Programms.
      expect(find.text('Left as it was'), findsOneWidget);
      expect(find.textContaining('Profile tea: a profile of this name already exists here'), findsOneWidget);
      expect(find.textContaining('Claude (Anthropic) API key: another one is stored here'), findsOneWidget);
      // Was nie mitreist, und der eine Ort des Bots.
      expect(find.textContaining('The Matrix account of the fish is never in the file'), findsOneWidget);
      expect(find.textContaining('One bot runs in one place only'), findsOneWidget);

      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('profile-moved')), findsOneWidget);
      expect(find.byKey(const ValueKey('profile-tea')), findsOneWidget);
    });

    testWidgets('a wrong password changes nothing and says so', (tester) async {
      final fake = fish()
        ..envelopeReadError = 'wrong password, or the envelope was changed; nothing was changed';
      await open(tester, fake, FakeFiles(chosen: '/home/nemo/umzug.rustyfish'));
      await tester.tap(find.byKey(const ValueKey('open-import')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('import-choose')));
      await tester.pumpAndSettle();
      await tester.enterText(find.byKey(const ValueKey('import-password')), 'another password!');
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('import-read')));
      await tester.pumpAndSettle();

      expect(
        find.text('wrong password, or the envelope was changed; nothing was changed'),
        findsOneWidget,
      );
      // Das Fenster bleibt, die Datei ist noch gewählt.
      expect(find.text('/home/nemo/umzug.rustyfish'), findsOneWidget);
      expect(find.byKey(const ValueKey('import-read')), findsOneWidget);

      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('profile-tea')), findsOneWidget);
    });

    testWidgets('leaving the dialog of the system chooses nothing', (tester) async {
      final fake = fish();
      final files = FakeFiles();
      await open(tester, fake, files);
      await tester.tap(find.byKey(const ValueKey('open-import')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('import-choose')));
      await tester.pumpAndSettle();

      expect(files.opened, 1);
      expect(find.text('No file chosen yet.'), findsOneWidget);
      expect(fake.envelopesRead, isEmpty);
    });

    testWidgets('a profile that comes in while the fish is on is taken up at once', (tester) async {
      final fake = fish()..envelopeRead = const EnvelopeRead(created: ['moved']);
      await open(tester, fake, FakeFiles(chosen: '/home/nemo/umzug.rustyfish'));
      await tester.tap(find.byKey(const ValueKey('fish-switch')));
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const ValueKey('open-import')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('import-choose')));
      await tester.pumpAndSettle();
      await tester.enterText(find.byKey(const ValueKey('import-password')), password);
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('import-read')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();

      // Wie nach dem Zurückholen aus dem Papierkorb: der laufende Fisch
      // bekommt gesagt, dass es das Profil gibt.
      expect(fake.lastRun!.said, ['start moved']);
    });
  });
}
