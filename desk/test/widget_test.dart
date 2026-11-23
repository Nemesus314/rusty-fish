// Tests der Maske gegen die Attrappe. Zuerst geschrieben, gegen ein leeres
// Gerüst gescheitert, dann die Maske dazu gebaut.

import 'package:flutter/services.dart' show SystemChannels;
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:rusty_fish_desk/cli.dart';
import 'package:rusty_fish_desk/log_file.dart';
import 'package:rusty_fish_desk/main.dart';
import 'package:rusty_fish_desk/model.dart';
import 'package:rusty_fish_desk/share.dart';

import 'fake_cli.dart';

const today = '2026-11-20';

Future<void> open(
  WidgetTester tester,
  FakeCli fake, {
  OpenLog Function(String profilesDir, String profile)? openLog,
}) async {
  // Ein Fenster in der Größe eines gewöhnlichen Desktops, damit die Karten
  // so nebeneinander stehen wie im echten Programm.
  tester.view.physicalSize = const Size(1500, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    DeskApp(
      cli: fake,
      refreshEvery: null,
      today: () => today,
      now: () => DateTime(2026, 11, 21, 3, 2, 52),
      openLog: openLog,
    ),
  );
  await tester.pumpAndSettle();
}

/// Schaltet den Fisch an. Die Attrappe sagt danach für jedes genannte Profil,
/// dass der Fisch es bedient, so wie das echte Programm es tut.
Future<void> switchOn(
  WidgetTester tester,
  FakeCli fake, {
  List<String> serving = const [],
}) async {
  await tester.tap(find.byKey(const ValueKey('fish-switch')));
  await tester.pumpAndSettle();
  for (final name in serving) {
    fake.lastRun!.emit('rusty-fish: [$name] serving');
  }
  await tester.pumpAndSettle();
}

/// Der eine Schalter des Fischs.
ButtonStyleButton fishSwitch(WidgetTester tester) =>
    tester.widget<ButtonStyleButton>(find.byKey(const ValueKey('fish-switch')));

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
    // Welcher Dienst mit welchem Modell übersetzt, steht auf der Karte.
    expect(inCard('tea', find.text('ChatGPT (OpenAI) · some-model')), findsOneWidget);
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
    // Keine Karte hat einen eigenen Schalter. Und solange ein Fisch von
    // außen läuft, lässt sich hier keiner anschalten.
    expect(inCard('study', find.text('Start')), findsNothing);
    expect(fishSwitch(tester).onPressed, isNull);

    expect(inCard('old', find.text('Invalid')), findsOneWidget);
    expect(
      inCard('old', find.textContaining('`lang` is empty for channel 222')),
      findsOneWidget,
    );
    // Ein kaputtes Profil lässt sich weder starten noch in der Maske öffnen.
    expect(inCard('old', find.text('Start')), findsNothing);
    expect(inCard('old', find.text('Edit')), findsNothing);
    expect(inCard('old', find.text('Delete')), findsOneWidget);

    expect(find.text('The fish runs while this window is open and this PC is awake.'), findsOneWidget);
    expect(find.textContaining(r'C:\fish\profiles'), findsWidgets);
  });

  testWidgets('the title bar says only whether the secrets are stored', (
    tester,
  ) async {
    await open(tester, threeProfiles());
    // Vier Dienste brauchen einen Schlüssel, einer davon liegt. Das eigene
    // Modell zählt nicht mit, weil es keinen braucht.
    expect(find.text('AI keys: 1 of 4 stored'), findsOneWidget);
    expect(find.text('Bot token: not stored'), findsOneWidget);
  });

  group('the guide for an AI', () {
    /// Fängt ab, was die Maske in die Zwischenablage legt.
    List<String> clipboardOf(WidgetTester tester) {
      final copied = <String>[];
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        (call) async {
          if (call.method == 'Clipboard.setData') {
            copied.add('${(call.arguments as Map)['text']}');
          }
          return null;
        },
      );
      addTearDown(
        () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          SystemChannels.platform,
          null,
        ),
      );
      return copied;
    }

    FakeCli withBot({required bool stored}) => FakeCli(
      entries: [entryOf(docOf('study'))],
      keys: KeyStatus({'openai': true, 'discord': stored}),
    );

    testWidgets('one click copies the link that brings the own bot onto a server', (tester) async {
      final copied = clipboardOf(tester);
      await open(tester, withBot(stored: true));
      await tester.tap(find.byKey(const ValueKey('chip-bot-token')));
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const ValueKey('copy-invite')));
      await tester.pumpAndSettle();
      expect(copied, [
        'https://discord.com/oauth2/authorize?client_id=42&scope=bot&permissions=536939520',
      ]);
      // Was mit dem Link zu tun ist, steht gleich dabei.
      expect(find.textContaining('Send it to whoever owns the server'), findsOneWidget);
    });

    testWidgets('without a stored bot token there is no link to copy', (tester) async {
      await open(tester, withBot(stored: false));
      await tester.tap(find.byKey(const ValueKey('chip-bot-token')));
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('secret-row-discord')), findsOneWidget);
      expect(find.byKey(const ValueKey('copy-invite')), findsNothing);
    });

    testWidgets('a program that cannot make the link says why and copies nothing', (tester) async {
      final copied = clipboardOf(tester);
      final fake = withBot(stored: true)
        ..inviteError = 'cannot read the id of the bot from the stored token';
      await open(tester, fake);
      await tester.tap(find.byKey(const ValueKey('chip-bot-token')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('copy-invite')));
      await tester.pumpAndSettle();
      expect(copied, isEmpty);
      expect(
        find.text('Cannot make the link: cannot read the id of the bot from the stored token'),
        findsOneWidget,
      );
    });

    testWidgets('the About window says what the fish is for, who made it and where to find them', (
      tester,
    ) async {
      final copied = clipboardOf(tester);
      await open(tester, threeProfiles());
      expect(find.text('Who made it'), findsNothing);

      await tester.tap(find.byKey(const ValueKey('about')));
      await tester.pumpAndSettle();
      expect(find.text('Version 0.3.0'), findsOneWidget);
      expect(find.textContaining('each in their own language'), findsOneWidget);
      expect(find.text('Nemesus (Christian Ermisch)'), findsOneWidget);
      expect(find.text('christianermisch.com'), findsOneWidget);
      // Sein Profil steht da; dass der Fisch dort noch nicht liegt, auch.
      expect(find.text('github.com/Nemesus314'), findsOneWidget);
      expect(find.textContaining('not published there yet'), findsOneWidget);

      await tester.tap(find.byKey(const ValueKey('copy-website')));
      await tester.pumpAndSettle();
      expect(copied, ['christianermisch.com']);

      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
      expect(find.text('Who made it'), findsNothing);
    });

    testWidgets('one click puts the program\'s own description on the clipboard', (
      tester,
    ) async {
      final copied = clipboardOf(tester);
      await open(tester, threeProfiles());
      await tester.tap(find.byKey(const ValueKey('copy-guide')));
      await tester.pumpAndSettle();
      expect(copied, ['# Rusty Fish: what it is\n']);
      expect(find.textContaining('Guide copied.'), findsOneWidget);
    });

    testWidgets('a program without a guide says so and copies nothing', (
      tester,
    ) async {
      final copied = clipboardOf(tester);
      final fake = threeProfiles()..guideError = 'unrecognized subcommand \'guide\'';
      await open(tester, fake);
      await tester.tap(find.byKey(const ValueKey('copy-guide')));
      await tester.pumpAndSettle();
      expect(copied, isEmpty);
      expect(
        find.text('Cannot copy the guide: unrecognized subcommand \'guide\''),
        findsOneWidget,
      );
    });
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
    // Ein neues Profil beginnt bei dem Dienst, den es vor der Auswahl gab.
    expect(doc.provider, 'openai');
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

    expect(fake.secrets['discord'], secret);
    expect(find.textContaining('very-secret'), findsNothing);
    for (final field in tester.widgetList<EditableText>(find.byType(EditableText))) {
      expect(field.controller.text, isNot(contains('very-secret')));
    }
    expect(find.text('Bot token: stored'), findsOneWidget);
  });

  testWidgets('forgetting a secret asks first', (tester) async {
    final fake = threeProfiles();
    await open(tester, fake);

    await tester.tap(find.text('AI keys: 1 of 4 stored'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('forget-openai')));
    await tester.pumpAndSettle();
    expect(fake.forgotten, isEmpty);
    expect(find.text('Forget the key for ChatGPT (OpenAI)?'), findsOneWidget);

    await tester.tap(find.text('Forget it'));
    await tester.pumpAndSettle();
    expect(fake.forgotten, ['openai']);
    expect(find.text('AI keys: 0 of 4 stored'), findsOneWidget);
  });

  testWidgets('the secrets list every service and never show a value', (
    tester,
  ) async {
    final fake = threeProfiles();
    await open(tester, fake);
    await tester.tap(find.byKey(const ValueKey('chip-ai-keys')));
    await tester.pumpAndSettle();

    Finder inDialog(Finder what) =>
        find.descendant(of: find.byType(AlertDialog), matching: what);
    Finder inRow(String entry, Finder what) => find.descendant(
      of: find.byKey(ValueKey('secret-row-$entry')),
      matching: what,
    );

    // Jede Anzeige der Titelleiste öffnet ihren eigenen Teil: hier stehen
    // die Schlüssel der Dienste, der Token des Bots und das Matrix-Konto nicht.
    expect(find.byKey(const ValueKey('secret-row-discord')), findsNothing);
    expect(find.byKey(const ValueKey('secret-row-matrix')), findsNothing);
    for (final title in [
      'ChatGPT (OpenAI)',
      'Grok (xAI)',
      'Gemini (Google)',
      'Claude (Anthropic)',
      'Own model or other service',
    ]) {
      expect(inDialog(find.text(title)), findsOneWidget, reason: title);
    }
    expect(inRow('openai', find.text('stored')), findsOneWidget);
    expect(inRow('xai', find.text('not stored')), findsOneWidget);
    // Ein eigenes Modell braucht oft keinen Schlüssel. Das steht dabei.
    expect(inRow('custom', find.textContaining('optional')), findsOneWidget);
    expect(inRow('xai', find.textContaining('optional')), findsNothing);

    const secret = 'xai-very-secret-456';
    await tester.ensureVisible(find.byKey(const ValueKey('store-xai')));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const ValueKey('secret-xai')), secret);
    await tester.tap(find.byKey(const ValueKey('store-xai')));
    await tester.pumpAndSettle();

    expect(fake.secrets['xai'], secret);
    expect(find.textContaining('very-secret'), findsNothing);
    for (final field in tester.widgetList<EditableText>(find.byType(EditableText))) {
      expect(field.controller.text, isNot(contains('very-secret')));
    }
    expect(inRow('xai', find.text('stored')), findsOneWidget);
    expect(find.text('AI keys: 2 of 4 stored'), findsOneWidget);
  });

  testWidgets('the chosen AI service is saved with the profile', (tester) async {
    final fake = threeProfiles();
    await open(tester, fake);

    await tester.tap(inCard('study', find.text('Edit')));
    await tester.pumpAndSettle();
    // Vor einem Wechsel gibt es keinen Hinweis zu den Modellnamen.
    expect(find.textContaining('Model names differ'), findsNothing);

    await tester.tap(find.byKey(const ValueKey('field-provider')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Grok (xAI)').last);
    await tester.pumpAndSettle();
    // Das Modell bleibt stehen, bekommt aber einen Hinweis: bei dem neuen
    // Dienst heißt es anders.
    expect(textOf(tester, const ValueKey('field-model')), 'some-model');
    expect(find.textContaining('Model names differ'), findsOneWidget);

    await tester.tap(find.text('Load models'));
    await tester.pumpAndSettle();
    expect(fake.modelRequests.single.provider, 'xai');
    await tester.tap(find.text('beta-model'));
    await tester.pumpAndSettle();
    // Ein Modell von der Liste dieses Dienstes braucht den Hinweis nicht mehr.
    expect(find.textContaining('Model names differ'), findsNothing);

    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();
    expect(fake.saved.single.provider, 'xai');
    expect(fake.saved.single.model, 'beta-model');
    expect(fake.saved.single.baseUrl, isNull);
    expect(inCard('study', find.text('Grok (xAI) · beta-model')), findsOneWidget);
  });

  testWidgets('an own model asks for its address, the others fold it away', (
    tester,
  ) async {
    final fake = threeProfiles();
    await open(tester, fake);

    await tester.tap(inCard('study', find.text('Edit')));
    await tester.pumpAndSettle();
    // Ein Dienst mit üblicher Adresse: das Feld ist eingeklappt.
    expect(find.text('Different address (optional)'), findsOneWidget);
    expect(find.byKey(const ValueKey('field-base-url')), findsNothing);

    await tester.tap(find.byKey(const ValueKey('field-provider')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Own model or other service').last);
    await tester.pumpAndSettle();
    // Ohne übliche Adresse ist das Feld da, mit einem Beispiel.
    expect(find.text('Different address (optional)'), findsNothing);
    expect(find.byKey(const ValueKey('field-base-url')), findsOneWidget);
    expect(find.textContaining('http://localhost:11434/v1'), findsWidgets);

    await tester.enterText(
      find.byKey(const ValueKey('field-base-url')),
      'http://localhost:11434/v1',
    );
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();
    expect(fake.saved.single.provider, 'custom');
    expect(fake.saved.single.baseUrl, 'http://localhost:11434/v1');
  });

  testWidgets('the folded address opens and is saved as an override', (
    tester,
  ) async {
    final fake = threeProfiles();
    await open(tester, fake);

    await tester.tap(inCard('study', find.text('Edit')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Different address (optional)'));
    await tester.pumpAndSettle();
    await tester.enterText(
      find.byKey(const ValueKey('field-base-url')),
      'https://proxy.example/v1',
    );
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();
    expect(fake.saved.single.provider, 'openai');
    expect(fake.saved.single.baseUrl, 'https://proxy.example/v1');
  });

  testWidgets('a profile whose service has no key cannot be started', (
    tester,
  ) async {
    // Abgelegt ist nur der Schlüssel für OpenAI.
    final fake = FakeCli(
      entries: [
        entryOf(docOf('grok', provider: 'xai')),
        entryOf(
          docOf('local', provider: 'custom', baseUrl: 'http://localhost:11434/v1'),
        ),
        entryOf(docOf('plain')),
      ],
    );
    await open(tester, fake);

    expect(inCard('grok', find.text('Grok (xAI) · some-model')), findsOneWidget);
    expect(inCard('grok', find.text('No key stored for Grok (xAI)')), findsOneWidget);

    // Ein eigenes Modell braucht keinen Schlüssel.
    expect(inCard('local', find.textContaining('No key stored')), findsNothing);
    expect(inCard('plain', find.textContaining('No key stored')), findsNothing);

    // Der Hinweis führt zu den Geheimnissen.
    await tester.tap(inCard('grok', find.text('No key stored for Grok (xAI)')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('secret-row-xai')), findsOneWidget);
  });

  testWidgets('a program that cannot list the services leaves the window usable', (
    tester,
  ) async {
    final fake = threeProfiles()
      ..providersError = "error: unrecognized subcommand 'providers'";
    await open(tester, fake);

    // Die Meldung des Programms steht oben, die Profile sind trotzdem da.
    expect(find.textContaining("unrecognized subcommand 'providers'"), findsOneWidget);
    expect(inCard('study', find.text('Edit')), findsOneWidget);
    expect(inCard('study', find.text('ChatGPT (OpenAI) · some-model')), findsOneWidget);
    expect(find.text('AI keys: 1 of 1 stored'), findsOneWidget);

    await tester.tap(inCard('study', find.text('Edit')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('field-provider')));
    await tester.pumpAndSettle();
    // Zur Auswahl steht nur der eine Dienst, den es vor der Auswahl gab.
    expect(find.text('Grok (xAI)'), findsNothing);
    await tester.tap(find.text('ChatGPT (OpenAI)').last);
    await tester.pumpAndSettle();

    // Ein älteres Programm kennt die Angabe des Dienstes bei den Modellen nicht.
    await tester.tap(find.text('Load models'));
    await tester.pumpAndSettle();
    expect(fake.modelRequests.single.provider, isNull);
    await tester.tap(find.text('beta-model'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();
    expect(fake.saved.single.provider, 'openai');
  });

  // Nemesus am 20.11.2026: „pro Discord-Nummer die Muttersprache und die
  // Neutralsprache festzulegen", und: „Es sollte auch die Funktion ‚Einladen'
  // geben. Und dann ein Haken, wer eingeladen wird."
  group('tables, mother tongues and invitations', () {
    // Gastgeber 100 und zwei weitere Menschen, die Plätze gehören noch niemandem.
    FakeCli withPeople() => FakeCli(
      entries: [
        entryOf(
          docOf(
            'study',
            people: const [
              Person(id: '100', gender: Gender.male, lang: 'German'),
              Person(id: '200', lang: 'Russian'),
              Person(id: '300', gender: Gender.female),
            ],
          ),
        ),
      ],
    );

    Future<void> edit(WidgetTester tester, FakeCli fake, String profile) async {
      await open(tester, fake);
      await tester.tap(inCard(profile, find.text('Edit')));
      await tester.pumpAndSettle();
    }

    Future<void> choose(WidgetTester tester, String key, String option) async {
      await tester.ensureVisible(find.byKey(ValueKey(key)));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(ValueKey(key)));
      await tester.pumpAndSettle();
      await tester.tap(find.text(option).last);
      await tester.pumpAndSettle();
    }

    testWidgets('whose table and the invitation are saved with the profile', (
      tester,
    ) async {
      final fake = withPeople();
      await edit(tester, fake, 'study');

      await choose(tester, 'seat-0-0-owner', 'You (host) · 100 · German');
      await choose(tester, 'seat-0-1-owner', '200 · Russian');
      await tester.ensureVisible(find.byKey(const ValueKey('person-1-invited')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('person-1-invited')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();

      final doc = fake.saved.single;
      expect(doc.rooms.single.seats.map((s) => s.owner), ['100', '200']);
      expect(doc.people.map((p) => p.invited), [false, true, false]);
      // Alles andere an den Menschen bleibt, wie es war.
      expect(doc.people.map((p) => p.lang), ['German', 'Russian', null]);
      expect(doc.people.map((p) => p.gender), [Gender.male, null, Gender.female]);
    });

    testWidgets('the language field says when the owner\'s mother tongue applies', (
      tester,
    ) async {
      final fake = withPeople();
      await edit(tester, fake, 'study');
      expect(find.textContaining('mother tongue of the owner applies'), findsNothing);

      await choose(tester, 'seat-0-1-owner', '200 · Russian');
      expect(
        find.text('The mother tongue of the owner applies: Russian'),
        findsOneWidget,
      );

      // Von diesem Menschen ist keine Muttersprache bekannt: dann gilt die
      // Sprache des Platzes, und es gibt keinen Hinweis.
      await choose(tester, 'seat-0-1-owner', '300');
      expect(find.textContaining('mother tongue of the owner applies'), findsNothing);

      await choose(tester, 'seat-0-1-owner', '200 · Russian');
      await choose(tester, 'seat-0-1-owner', 'nobody in particular');
      expect(find.textContaining('mother tongue of the owner applies'), findsNothing);
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(fake.saved.single.rooms.single.seats.map((s) => s.owner), [null, null]);
    });

    testWidgets('a person carries mother tongue, neutral languages and form', (
      tester,
    ) async {
      final fake = withPeople();
      await edit(tester, fake, 'study');

      await tester.ensureVisible(find.byKey(const ValueKey('person-1-lang')));
      await tester.pumpAndSettle();
      expect(textOf(tester, const ValueKey('person-1-lang')), 'Russian');
      await tester.enterText(find.byKey(const ValueKey('person-1-lang')), 'Polish');
      await tester.enterText(
        find.byKey(const ValueKey('person-1-neutral')),
        'English,  French',
      );
      await choose(tester, 'person-1-form', 'female');

      await tester.ensureVisible(find.text('Add person'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Add person'));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.byKey(const ValueKey('person-3-discord')));
      await tester.pumpAndSettle();
      await tester.enterText(find.byKey(const ValueKey('person-3-discord')), '400');
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();

      final people = fake.saved.single.people;
      expect(people[1].lang, 'Polish');
      expect(people[1].neutral, ['English', 'French']);
      expect(people[1].gender, Gender.female);
      // Ein leeres Feld heißt: es gilt, was im Raum steht.
      expect(people[0].neutral, isNull);
      // Ein neuer Mensch braucht nur seine Kennung.
      expect(people[3].id, '400');
      expect(people[3].gender, isNull);
      expect(people[3].lang, isNull);
      expect(people[3].invited, isFalse);
    });

    testWidgets('the host needs no invitation: the box is ticked and fixed', (
      tester,
    ) async {
      final fake = withPeople();
      await edit(tester, fake, 'study');
      await tester.ensureVisible(find.byKey(const ValueKey('person-0-invited')));
      await tester.pumpAndSettle();

      Checkbox box(int person) =>
          tester.widget<Checkbox>(find.byKey(ValueKey('person-$person-invited')));
      expect(box(0).value, isTrue);
      expect(box(0).onChanged, isNull);
      expect(box(1).value, isFalse);
      expect(box(1).onChanged, isNotNull);
      // Wen man einlädt, für den zahlt man. Das steht dabei.
      expect(find.textContaining('on your key'), findsWidgets);

      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      // Der Haken beim Gastgeber ist nur gezeigt. Im Profil ändert er nichts.
      expect(fake.saved.single.people[0].invited, isFalse);
    });

    testWidgets('a profile with tables goes back out unchanged', (tester) async {
      final fake = FakeCli(entries: [entryOf(tablesDoc('pair'))]);
      await edit(tester, fake, 'pair');
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(fake.saved.single.toJson(), tablesDoc('pair').toJson());
    });

    testWidgets('the card says whom this fish translates for', (tester) async {
      final fake = FakeCli(
        entries: [
          // Ein Profil aus der Zeit vor den Tischen.
          entryOf(docOf('old-style')),
          // Der Gastgeber hat seinen Gast eingeladen.
          entryOf(tablesDoc('pair')),
          // Der Gast ist nicht eingeladen: an seinem Tisch wird nur mitgelesen.
          entryOf(tablesDoc('watch', guestInvited: false)),
        ],
      );
      await open(tester, fake);

      expect(inCard('old-style', find.text('Translates at every table')), findsOneWidget);
      expect(inCard('old-style', find.textContaining('read only')), findsNothing);

      expect(
        inCard('pair', find.text('Translates for you and 1 invited guest')),
        findsOneWidget,
      );
      expect(inCard('pair', find.textContaining('read only')), findsNothing);
      // Die Sprachen der Tische sind die Muttersprachen ihrer Besitzer.
      expect(inCard('pair', find.text('German ⇄ Russian')), findsOneWidget);

      expect(inCard('watch', find.text('Translates for you')), findsOneWidget);
      expect(inCard('watch', find.text('1 table is read only')), findsOneWidget);
    });
  });

  testWidgets('one switch turns the fish on and off for all profiles', (tester) async {
    final fake = FakeCli(entries: [entryOf(docOf('study')), entryOf(docOf('other'))]);
    await open(tester, fake);
    expect(find.text('Switch on'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('fish-switch')));
    await tester.pumpAndSettle();
    // Ein Fisch für alle. Bis er sagt, was er bedient, heißt es "Starting".
    expect(fake.runs, 1);
    expect(inCard('study', find.text('Starting')), findsOneWidget);
    expect(inCard('other', find.text('Starting')), findsOneWidget);

    fake.lastRun!.emit('rusty-fish: [study] serving');
    fake.lastRun!.emit('rusty-fish: [other] serving');
    fake.lastRun!.emit('rusty-fish: connected to Discord as Rusty Fish');
    await tester.pumpAndSettle();
    expect(inCard('study', find.text('Running')), findsOneWidget);
    expect(inCard('other', find.text('Running')), findsOneWidget);
    expect(find.text('Switch off · 2 of 2 running'), findsOneWidget);

    await tester.tap(inCard('study', find.textContaining('Log')));
    await tester.pumpAndSettle();
    // Jede Zeile trägt die Uhrzeit, zu der sie ankam.
    expect(
      find.textContaining('2026-11-21 03:02:52  rusty-fish: connected to Discord as Rusty Fish'),
      findsOneWidget,
    );

    await tester.tap(find.byKey(const ValueKey('fish-switch')));
    await tester.pumpAndSettle();
    expect(fake.lastRun!.stopped, isTrue);
    expect(inCard('study', find.text('Stopped')), findsOneWidget);
    expect(inCard('other', find.text('Stopped')), findsOneWidget);
    expect(find.text('Switch on'), findsOneWidget);
  });

  testWidgets('the log also goes to a file, and the window says where', (
    tester,
  ) async {
    final written = <String>[];
    final opened = <String>[];
    final fake = FakeCli(entries: [entryOf(docOf('study'))]);
    await open(
      tester,
      fake,
      openLog: (dir, profile) {
        opened.add('$dir | $profile');
        return OpenLog(r'C:\fish\logs\study.log', written.add);
      },
    );

    await switchOn(tester, fake, serving: ['study']);
    fake.lastRun!.emit('rusty-fish: connected to Discord as Rusty Fish');
    await tester.pumpAndSettle();
    await tester.tap(inCard('study', find.textContaining('Log')));
    await tester.pumpAndSettle();

    expect(opened, [r'C:\fish\profiles | study']);
    expect(written, [
      '2026-11-21 03:02:52  rusty-fish: [study] serving',
      '2026-11-21 03:02:52  rusty-fish: connected to Discord as Rusty Fish',
    ]);
    expect(find.text(r'Also written to C:\fish\logs\study.log'), findsOneWidget);
  });

  testWidgets('a log file that fails says so once and the fish keeps running', (
    tester,
  ) async {
    final fake = FakeCli(entries: [entryOf(docOf('study'))]);
    await open(
      tester,
      fake,
      openLog: (dir, profile) => OpenLog(
        r'C:\fish\logs\study.log',
        (line) => throw const FormatException('disk full'),
      ),
    );

    await switchOn(tester, fake, serving: ['study']);
    fake.lastRun!.emit('rusty-fish: one');
    fake.lastRun!.emit('rusty-fish: two');
    await tester.pumpAndSettle();
    await tester.tap(inCard('study', find.textContaining('Log')));
    await tester.pumpAndSettle();

    expect(inCard('study', find.text('Running')), findsOneWidget);
    // Die Zeile "serving", die Meldung über die Datei, und die zwei danach.
    expect(find.textContaining('Log (4 lines)'), findsOneWidget);
    expect(find.textContaining('cannot write the log file'), findsOneWidget);
    expect(find.textContaining('Also written to'), findsNothing);
  });

  testWidgets('a fish that ends on its own says why', (tester) async {
    final fake = FakeCli(entries: [entryOf(docOf('study'))]);
    await open(tester, fake);

    await switchOn(tester, fake);
    fake.lastRun!.emit('rusty-fish: Discord bot token not stored');
    await tester.pump();
    fake.lastRun!.end(1);
    await tester.pumpAndSettle();

    expect(inCard('study', find.text('Stopped')), findsOneWidget);
    expect(find.textContaining('Discord bot token not stored'), findsWidgets);
    // Der Grund steht im Wortlaut des Fischs da, ohne Zeitstempel; der gehört
    // nur ins Protokoll, und das ist hier noch zugeklappt.
    expect(find.textContaining('2026-11-21'), findsNothing);
  });

  testWidgets('a start the program refuses says why', (tester) async {
    final fake = FakeCli(entries: [entryOf(docOf('study'))]);
    fake.runError = 'another profile is already running';
    await open(tester, fake);

    await switchOn(tester, fake);
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
    // Gefragt wird nach den Modellen des Dienstes, den das Profil nennt.
    expect(fake.modelRequests.single.provider, 'openai');
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
    tester.view.physicalSize = const Size(1500, 1000);
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
    // Mit `--at` geht das Formular gleich bei einem Abschnitt auf.
    expect(StartRequest.fromArgs(const ['--edit', 'tea'])!.section, isNull);
    final at = StartRequest.fromArgs(const ['--edit', 'tea', '--at', 'people'])!;
    expect(at.profile, 'tea');
    expect(at.section, 'people');
  });

  group('names the host gives', () {
    // Nemo ist der Gastgeber. Robin hat einen Tisch, aber noch keine Kennung.
    FakeCli withNames() => FakeCli(
      entries: [
        entryOf(
          docOf(
            'tea',
            people: const [
              Person(id: '100', name: 'Nemo', lang: 'German'),
              Person(name: 'Robin', lang: 'Russian', invited: true),
            ],
            seats: const [
              Seat(channel: '111', lang: 'German', owner: 'Nemo'),
              Seat(channel: '222', lang: 'English', owner: 'Robin'),
            ],
          ),
        ),
      ],
    );

    testWidgets('the card shows for whom the tables are built and over which door', (tester) async {
      await open(tester, withNames());

      expect(inCard('tea', find.text('Nemo · German ⇄ Robin · Russian')), findsOneWidget);
      expect(inCard('tea', find.text('Translates for you and Robin')), findsOneWidget);
      expect(
        inCard('tea', find.text('Discord ⇄ Discord · not end-to-end encrypted')),
        findsOneWidget,
      );
    });

    testWidgets('a name is typed once and the table follows it', (tester) async {
      final fake = withNames();
      await open(tester, fake);
      await tester.tap(inCard('tea', find.text('Edit')));
      await tester.pumpAndSettle();

      await tester.ensureVisible(find.byKey(const ValueKey('person-1-name')));
      await tester.pumpAndSettle();
      expect(textOf(tester, const ValueKey('person-1-name')), 'Robin');
      expect(textOf(tester, const ValueKey('person-1-discord')), '', reason: 'no id yet');
      // Der Gastgeber steht mit seinem Namen da und ist als solcher erkannt.
      expect(textOf(tester, const ValueKey('person-0-name')), 'Nemo');
      expect(find.text('This is you, the host.'), findsOneWidget);

      await tester.enterText(find.byKey(const ValueKey('person-1-name')), 'Robin Goodfellow');
      await tester.enterText(find.byKey(const ValueKey('person-1-discord')), '200');
      await tester.pumpAndSettle();
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();

      final saved = fake.saved.single;
      expect(saved.people[1].name, 'Robin Goodfellow');
      expect(saved.people[1].id, '200');
      expect(saved.people[1].invited, isTrue);
      // Der Tisch zeigt weiter auf denselben Menschen, unter dem neuen Namen.
      expect(saved.rooms.single.seats[1].owner, 'Robin Goodfellow');
      expect(saved.rooms.single.seats[0].owner, 'Nemo');
    });

    testWidgets('emptying the id before typing a name does not take the table away', (
      tester,
    ) async {
      // Gast 200 hat eine Kennung und einen Tisch, aber noch keinen Namen.
      final fake = FakeCli(entries: [entryOf(tablesDoc('pair'))]);
      await open(tester, fake);
      await tester.tap(inCard('pair', find.text('Edit')));
      await tester.pumpAndSettle();

      await tester.ensureVisible(find.byKey(const ValueKey('person-1-discord')));
      await tester.pumpAndSettle();
      // Erst die Kennung leeren, dann den Namen tippen: für einen Augenblick
      // hat der Mensch keines von beiden.
      await tester.enterText(find.byKey(const ValueKey('person-1-discord')), '');
      await tester.pumpAndSettle();
      await tester.enterText(find.byKey(const ValueKey('person-1-name')), 'Robin');
      await tester.pumpAndSettle();
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();

      final saved = fake.saved.single;
      expect(saved.people[1].name, 'Robin');
      expect(saved.people[1].id, '');
      expect(saved.rooms.single.seats[1].owner, 'Robin', reason: 'the table is still Robin\'s');
    });

    testWidgets('a profile that points at people by id opens with their names', (tester) async {
      // So steht es in einem Profil von vor dem 21.11.2026, dem jemand von
      // Hand Namen gegeben hat: die Tische zeigen noch mit Kennungen.
      final fake = FakeCli(
        entries: [
          entryOf(
            docOf(
              'tea',
              people: const [
                Person(id: '100', name: 'Nemo'),
                Person(id: '200', name: 'Robin', invited: true),
              ],
              seats: const [
                Seat(channel: '111', lang: 'German', owner: '100'),
                Seat(channel: '222', lang: 'Russian', owner: '200'),
              ],
            ),
          ),
        ],
      );
      await open(tester, fake);
      expect(inCard('tea', find.text('Nemo · German ⇄ Robin · Russian')), findsOneWidget);
      expect(inCard('tea', find.text('Translates for you and Robin')), findsOneWidget);

      await tester.tap(inCard('tea', find.text('Edit')));
      await tester.pumpAndSettle();
      expect(find.textContaining('not listed under People'), findsNothing);
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      // Gespeichert zeigen die Tische mit dem Namen auf ihre Besitzer.
      expect(fake.saved.single.rooms.single.seats.map((s) => s.owner), ['Nemo', 'Robin']);
    });
  });

  group('one switch for all profiles', () {
    FakeCli two() => FakeCli(entries: [entryOf(docOf('tea')), entryOf(docOf('probe'))]);

    Future<void> openLogOf(WidgetTester tester, String profile) async {
      await tester.ensureVisible(inCard(profile, find.textContaining('Log')));
      await tester.tap(inCard(profile, find.textContaining('Log')));
      await tester.pumpAndSettle();
    }

    ButtonStyleButton deleteOf(WidgetTester tester, String profile) =>
        tester.widget<ButtonStyleButton>(
          inCard(
            profile,
            find.ancestor(of: find.text('Delete'), matching: find.bySubtype<ButtonStyleButton>()),
          ),
        );

    testWidgets('no card has a switch of its own', (tester) async {
      final fake = two();
      await open(tester, fake);
      expect(find.text('Start'), findsNothing);
      expect(find.text('Stop'), findsNothing);
      await switchOn(tester, fake, serving: ['tea', 'probe']);
      expect(find.text('Start'), findsNothing);
      expect(find.text('Stop'), findsNothing);
    });

    testWidgets('every card shows the lines about its own profile and about the whole fish', (
      tester,
    ) async {
      final fake = two();
      await open(tester, fake);
      await switchOn(tester, fake, serving: ['tea', 'probe']);
      final fish = fake.lastRun!;

      fish.emit('rusty-fish: connected to Discord as Rusty Fish');
      fish.emit('rusty-fish: [tea] consent stored');
      fish.emit('rusty-fish: [probe] catching up on 2 missed message(s)');
      await tester.pumpAndSettle();
      await openLogOf(tester, 'tea');
      await openLogOf(tester, 'probe');

      const at = '2026-11-21 03:02:52  ';
      final tea = tester.widget<Text>(inCard('tea', find.textContaining('connected to Discord')));
      final probe = tester.widget<Text>(inCard('probe', find.textContaining('connected to Discord')));
      // Was den ganzen Fisch betrifft, steht bei beiden.
      expect(tea.data, contains('${at}rusty-fish: connected to Discord as Rusty Fish'));
      expect(probe.data, contains('${at}rusty-fish: connected to Discord as Rusty Fish'));
      // Was ein Profil betrifft, nur bei diesem.
      expect(tea.data, contains('[tea] consent stored'));
      expect(tea.data, isNot(contains('[probe]')));
      expect(probe.data, contains('[probe] catching up'));
      expect(probe.data, isNot(contains('[tea]')));
    });

    testWidgets('a profile the fish cannot start says why, and the others go on', (
      tester,
    ) async {
      final fake = two();
      await open(tester, fake);
      await switchOn(tester, fake, serving: ['tea']);
      expect(inCard('probe', find.text('Starting')), findsOneWidget);

      // So antwortet der Fisch, wenn zwei Profile denselben Kanal nennen.
      fake.lastRun!.emit(
        'rusty-fish: [probe] cannot start: channel 222 already belongs to profile "tea", which is running',
      );
      await tester.pumpAndSettle();
      expect(inCard('probe', find.text('Stopped')), findsOneWidget);
      expect(
        inCard(
          'probe',
          find.text('channel 222 already belongs to profile "tea", which is running'),
        ),
        findsOneWidget,
      );
      expect(inCard('tea', find.text('Running')), findsOneWidget);
      expect(fake.lastRun!.stopped, isFalse);
      // Der Schalter sagt, wie viele wirklich laufen.
      expect(find.text('Switch off · 1 of 2 running'), findsOneWidget);
    });

    // Seit dem 23.11.2026: ein Profil mit Tischen an Discord und an Matrix
    // läuft an beiden Türen oder gar nicht. Hält der Fisch es an, weil eine
    // Tür es nicht bedienen kann, steht der Grund an der Karte, auch wenn
    // die andere Tür es eben noch als bedient gemeldet hat.
    testWidgets('a profile stopped because one of its doors fails says why on its card', (
      tester,
    ) async {
      final fake = two();
      await open(tester, fake);
      await switchOn(tester, fake, serving: ['tea', 'probe']);

      const why =
          'the Matrix door cannot serve it (the fish is not in this room), and a profile with '
          'tables at two doors runs at both or not at all';
      fake.lastRun!.emit('rusty-fish: [probe] cannot start: the fish is not in this room');
      // Die andere Tür hatte es schon aufgenommen und meldet das erst jetzt.
      fake.lastRun!.emit('rusty-fish: [probe] serving');
      fake.lastRun!.emit('rusty-fish: [probe] no longer serving');
      fake.lastRun!.emit('rusty-fish: [probe] stopped: $why');
      await tester.pumpAndSettle();
      expect(inCard('probe', find.text('Stopped')), findsOneWidget);
      expect(inCard('probe', find.text(why)), findsOneWidget);
      expect(inCard('tea', find.text('Running')), findsOneWidget);
      expect(find.text('Switch off · 1 of 2 running'), findsOneWidget);
    });

    testWidgets('when the fish ends by itself the window says why, once', (tester) async {
      final fake = two();
      await open(tester, fake);
      await switchOn(tester, fake, serving: ['tea', 'probe']);

      fake.lastRun!.emit('rusty-fish: the Discord connection ended: gone');
      await tester.pumpAndSettle();
      fake.lastRun!.end(1);
      await tester.pumpAndSettle();
      expect(inCard('tea', find.text('Stopped')), findsOneWidget);
      expect(inCard('probe', find.text('Stopped')), findsOneWidget);
      // Es betrifft den ganzen Fisch, also steht es oben und nicht an jeder Karte.
      expect(find.text('rusty-fish: the Discord connection ended: gone'), findsOneWidget);
      expect(find.text('Switch on'), findsOneWidget);
    });

    testWidgets('a profile changed while the fish is on is loaded again, the others undisturbed', (
      tester,
    ) async {
      final fake = two();
      await open(tester, fake);
      await switchOn(tester, fake, serving: ['tea', 'probe']);
      final fish = fake.lastRun!;

      await tester.tap(inCard('probe', find.text('Edit')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();

      // Kein Aus und An für alle: nur dieses eine Profil wird neu geladen.
      expect(fish.said, ['restart probe']);
      expect(fish.stopped, isFalse);
      expect(fake.runs, 1);
      expect(inCard('probe', find.text('Starting')), findsOneWidget);
      expect(inCard('tea', find.text('Running')), findsOneWidget);

      fish.emit('rusty-fish: [probe] serving');
      await tester.pumpAndSettle();
      expect(inCard('probe', find.text('Running')), findsOneWidget);
    });

    testWidgets('a changed profile that cannot be loaded keeps running as it was', (tester) async {
      final fake = two();
      await open(tester, fake);
      await switchOn(tester, fake, serving: ['tea', 'probe']);
      await tester.tap(inCard('probe', find.text('Edit')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();

      fake.lastRun!.emit('rusty-fish: [probe] cannot restart: Claude API key not stored');
      await tester.pumpAndSettle();
      expect(inCard('probe', find.text('Running')), findsOneWidget);
      expect(
        inCard('probe', find.textContaining('Still running with the settings from before')),
        findsOneWidget,
      );
    });

    testWidgets('a profile created while the fish is on joins it', (tester) async {
      final fake = two();
      await open(tester, fake);
      await switchOn(tester, fake, serving: ['tea', 'probe']);

      await tester.tap(inCard('tea', find.text('Duplicate')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(fake.lastRun!.said, ['start tea-copy']);
      expect(inCard('tea-copy', find.text('Starting')), findsOneWidget);
    });

    testWidgets('a profile can be deleted only while the fish is off', (tester) async {
      final fake = two();
      await open(tester, fake);
      expect(deleteOf(tester, 'tea').onPressed, isNotNull);
      await switchOn(tester, fake, serving: ['tea', 'probe']);
      expect(deleteOf(tester, 'tea').onPressed, isNull);
      expect(deleteOf(tester, 'probe').onPressed, isNull);
    });

    testWidgets('saving a profile while the fish is off starts nothing', (tester) async {
      final fake = two();
      await open(tester, fake);
      await tester.tap(inCard('probe', find.text('Edit')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(fake.runs, 0);
      expect(inCard('probe', find.text('Stopped')), findsOneWidget);
    });
  });

  group('where the AI reads', () {
    testWidgets('every card says where its sentences go', (tester) async {
      final fake = FakeCli(
        entries: [
          entryOf(
            docOf('cloud'),
            ai: const AiReach(service: 'api.openai.com', place: 'outside'),
          ),
          entryOf(
            docOf('home', provider: 'custom', baseUrl: 'http://localhost:11434/v1'),
            ai: const AiReach(service: 'localhost:11434', place: 'this_machine'),
          ),
          // Ein älteres Programm nennt den Ort nicht.
          entryOf(docOf('old')),
        ],
      );
      await open(tester, fake);

      expect(
        inCard('cloud', find.text('AI reads at an outside service · api.openai.com')),
        findsOneWidget,
      );
      expect(inCard('home', find.text('AI runs on this PC · localhost:11434')), findsOneWidget);
      expect(inCard('old', find.textContaining('AI runs')), findsNothing);
      expect(inCard('old', find.textContaining('AI reads')), findsNothing);
    });
  });

  group('rooms shared with someone who brings a fish of their own', () {
    // Nemo ist der Gastgeber und hat Robin eingeladen.
    FakeCli hosted() => FakeCli(
      entries: [
        entryOf(
          docOf(
            'tea',
            people: const [
              Person(id: '100', name: 'Nemo', lang: 'German'),
              Person(id: '200', name: 'Robin', lang: 'Russian', invited: true),
            ],
            seats: const [
              Seat(channel: '111', lang: 'German', owner: 'Nemo'),
              Seat(channel: '222', lang: 'English', owner: 'Robin'),
            ],
          ),
        ),
        entryOf(docOf('study')),
      ],
    );

    /// Fängt ab, was die Maske in die Zwischenablage legt.
    List<String> clipboardOf(WidgetTester tester) {
      final copied = <String>[];
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        (call) async {
          if (call.method == 'Clipboard.setData') {
            copied.add('${(call.arguments as Map)['text']}');
          }
          return null;
        },
      );
      addTearDown(
        () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          SystemChannels.platform,
          null,
        ),
      );
      return copied;
    }

    testWidgets('one click on the card copies the rooms as text', (tester) async {
      final copied = clipboardOf(tester);
      await open(tester, hosted());
      await tester.tap(find.byKey(const ValueKey('share-tea')));
      await tester.pumpAndSettle();

      final shared = readSharedRooms(copied.single)!;
      expect(shared.rooms.single.seats.map((s) => s.channel), ['111', '222']);
      expect(shared.people.map((p) => p.name), ['Nemo', 'Robin']);
      // Was im Text steht und was nicht, sagt die Maske dazu.
      expect(find.textContaining('No key, no token'), findsOneWidget);
    });

    testWidgets('the other person pastes them and says who they are', (tester) async {
      final copied = clipboardOf(tester);
      final fake = hosted();
      await open(tester, fake);
      await tester.tap(find.byKey(const ValueKey('share-tea')));
      await tester.pumpAndSettle();
      final text = copied.single;

      // Robin, am eigenen Rechner, trägt den Raum in ein eigenes Profil ein.
      await tester.tap(inCard('study', find.text('Edit')));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.byKey(const ValueKey('paste-rooms')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('paste-rooms')));
      await tester.pumpAndSettle();
      await tester.enterText(find.byKey(const ValueKey('shared-rooms-text')), text);
      await tester.tap(find.byKey(const ValueKey('read-shared-rooms')));
      await tester.pumpAndSettle();
      expect(find.text('Found one room:'), findsOneWidget);
      expect(find.text('Nemo · German ⇄ Robin · Russian'), findsOneWidget);

      await tester.tap(find.byKey(const ValueKey('shared-me')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Robin').last);
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('use-shared-rooms')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();

      final saved = fake.saved.single;
      // Derselbe Raum, ohne einen einzigen abgetippten Buchstaben.
      final seats = saved.rooms.single.seats;
      expect(seats.map((s) => s.channel), ['111', '222']);
      expect(seats.map((s) => s.owner), ['Nemo', 'Robin']);
      expect(saved.people.map((p) => p.id), ['100', '200']);
      // Robin ist hier der Gastgeber, und niemand ist eingeladen.
      expect(saved.host, 'Robin');
      expect(saved.people.every((p) => !p.invited), isTrue);
    });

    testWidgets('a text that is no room text says so and changes nothing', (tester) async {
      final fake = hosted();
      await open(tester, fake);
      await tester.tap(inCard('study', find.text('Edit')));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.byKey(const ValueKey('paste-rooms')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('paste-rooms')));
      await tester.pumpAndSettle();
      await tester.enterText(find.byKey(const ValueKey('shared-rooms-text')), 'hello there');
      await tester.tap(find.byKey(const ValueKey('read-shared-rooms')));
      await tester.pumpAndSettle();
      expect(find.textContaining('This is not a room text'), findsOneWidget);
      expect(find.byKey(const ValueKey('use-shared-rooms')), findsNothing);

      // Der Knopf des Fensters, nicht der des Formulars dahinter.
      await tester.tap(find.text('Cancel').last);
      await tester.pumpAndSettle();
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      // Das Profil ist geblieben, wie es war.
      expect(fake.saved.single.rooms.single.seats.map((s) => s.channel), ['111', '222']);
      expect(fake.saved.single.host, '100');
    });
  });

  group('light or dark', () {
    Future<void> openWith(WidgetTester tester, ThemeChoice choice) async {
      tester.view.physicalSize = const Size(1500, 1000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        DeskApp(
          cli: FakeCli(entries: [entryOf(docOf('study'))]),
          refreshEvery: null,
          today: () => today,
          theme: choice,
        ),
      );
      await tester.pumpAndSettle();
    }

    Brightness shown(WidgetTester tester) =>
        Theme.of(tester.element(find.byKey(const ValueKey('profile-study')))).brightness;

    testWidgets('one click turns the whole window dark, another one light again', (tester) async {
      final remembered = <ThemeMode>[];
      await openWith(tester, ThemeChoice(initial: ThemeMode.light, remember: remembered.add));
      expect(shown(tester), Brightness.light);
      expect(find.byTooltip('Switch to dark'), findsOneWidget);

      await tester.tap(find.byKey(const ValueKey('toggle-theme')));
      await tester.pumpAndSettle();
      expect(shown(tester), Brightness.dark);
      expect(find.byTooltip('Switch to light'), findsOneWidget);

      await tester.tap(find.byKey(const ValueKey('toggle-theme')));
      await tester.pumpAndSettle();
      expect(shown(tester), Brightness.light);
      expect(remembered, [ThemeMode.dark, ThemeMode.light]);
    });

    testWidgets('a choice from last time is there when the window opens', (tester) async {
      await openWith(tester, ThemeChoice(initial: ThemeMode.dark));
      expect(shown(tester), Brightness.dark);
    });
  });
}
