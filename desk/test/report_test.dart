// Tests zu dem, was seit dem 23.11.2026 an jeder Karte neu ist: zwei Knöpfe,
// die Stand und Protokoll eines Profils für eine KI in die Zwischenablage
// legen (ohne und mit den Menschen), und die Hinweise, die erst nach einem
// Klick auf das ⓘ erscheinen statt beim Überfahren mit der Maus.

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:rusty_fish_desk/main.dart';
import 'package:rusty_fish_desk/model.dart';
import 'package:rusty_fish_desk/report.dart';

import 'fake_cli.dart';

/// Ein Gespräch an zwei Türen, mit zwei Menschen.
ProfileDoc bridge() => const ProfileDoc(
  name: 'bridge',
  model: 'some-model',
  baseUrl: null,
  host: 'Nemo',
  aiNames: ['Fish'],
  limits: Limits(translationTokensPerDay: 100000, questionsPerGuestPerDay: 10),
  rooms: [
    Room(
      passThrough: ['Russian'],
      contextLines: 10,
      seats: [
        Seat(channel: '111', lang: 'German', owner: 'Nemo'),
        Seat(door: matrixDoor, channel: '!english:example.org', lang: 'English', owner: 'Robin'),
      ],
    ),
  ],
  people: [
    Person(name: 'Nemo', id: '100', matrix: '@nemo:example.org', lang: 'German'),
    Person(name: 'Robin', matrix: '@robin:example.org', lang: 'English', invited: true),
  ],
);

const log = [
  '2026-11-23 12:39:47  rusty-fish: connected to Discord as Rusty Fish',
  '2026-11-23 12:39:49  rusty-fish: connected to Matrix as @fish:example.org',
  '2026-11-23 12:58:45  rusty-fish: [bridge] cannot see channel 111: Discord knows no channel '
      'with this id that the bot can see.',
  '2026-11-23 12:59:01  rusty-fish: [bridge] cannot decrypt a message in room '
      '!english:example.org from @robin:example.org: its sender\'s app did not give the fish '
      'the key',
];

Future<void> open(WidgetTester tester, FakeCli fake) async {
  tester.view.physicalSize = const Size(1700, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(DeskApp(cli: fake, refreshEvery: null, today: () => '2026-11-23'));
  await tester.pumpAndSettle();
}

Finder inCard(String profile, Finder what) =>
    find.descendant(of: find.byKey(ValueKey('profile-$profile')), matching: what);

/// Fängt ab, was die Maske in die Zwischenablage legt.
List<String> clipboardOf(WidgetTester tester) {
  final copied = <String>[];
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(SystemChannels.platform, (
    call,
  ) async {
    if (call.method == 'Clipboard.setData') {
      copied.add('${(call.arguments as Map)['text']}');
    }
    return null;
  });
  addTearDown(
    () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      null,
    ),
  );
  return copied;
}

void main() {
  group('the text for an AI', () {
    final entry = entryOf(
      bridge(),
      ai: const AiReach(service: 'api.openai.com', place: 'outside'),
    );

    test('without people holds status, rooms, AI and log, and nobody', () {
      final text = reportForAi(
        entry: entry,
        status: 'Running',
        log: log,
        service: 'OpenAI (ChatGPT)',
        withPeople: false,
      );
      expect(text, contains('profile "bridge"'));
      expect(text, contains('Status: Running'));
      // Je Raum die Türen und Sprachen, und über welche Türen er läuft.
      expect(text, contains('- Room 1: Discord, German ⇄ Matrix, English · neutral: Russian'));
      expect(text, contains('Discord ⇄ Matrix · not end-to-end encrypted on Discord'));
      expect(text, contains('AI: OpenAI (ChatGPT) · some-model'));
      expect(text, contains('Sends to an outside service · api.openai.com'));
      expect(text, contains('Log (4 lines'));
      expect(text, contains('cannot see channel 111'));
      // Niemand: kein Name, keine Kennung eines Menschen, kein Gastgeber.
      expect(text, contains('Left out: the people of this profile'));
      for (final gone in ['Nemo', 'Robin', '@nemo', '@robin', '"100"', 'host']) {
        expect(
          text.replaceAll('and the host.', ''),
          isNot(contains(gone)),
          reason: '$gone must stay out',
        );
      }
      // Auch nicht dort, wo eine Zeile des Protokolls eine Kennung nennt.
      expect(text, contains('from @…: its sender'));
      expect(text, contains('connected to Matrix as @…'));
      // Die Kennungen der Tische stehen im Protokoll, wie an der Karte.
      expect(text, contains('!english:example.org'));
    });

    test('with people adds the whole profile and leaves the log as it is', () {
      final text = reportForAi(
        entry: entry,
        status: 'Stopped',
        log: log,
        runError: 'the Matrix door cannot serve it',
        withPeople: true,
      );
      expect(text, contains('With the full profile'));
      expect(text, contains('Why it is not running: the Matrix door cannot serve it'));
      expect(text, contains('from @robin:example.org: its sender'));
      expect(text, contains('"name": "Robin"'));
      expect(text, contains('"matrix": "@nemo:example.org"'));
      expect(text, contains('"host": "Nemo"'));
    });

    test('says so when there is no log, and gives the reason of a refused profile', () {
      final text = reportForAi(
        entry: brokenEntry('broken', 'configuration: `model` is empty'),
        status: 'Invalid',
        log: const [],
        withPeople: false,
      );
      expect(text, contains('Status: Invalid'));
      expect(text, contains('The program refuses this profile: configuration: `model` is empty'));
      expect(text, contains('Log: empty.'));
    });

    test('never holds a secret: there is none in what it is made from', () {
      // Der Text entsteht aus dem Profil und dem Protokoll. Ein Schlüssel
      // oder Token steht in keinem von beiden; die Ablage dafür ist woanders.
      final text = reportForAi(entry: entry, status: 'Running', log: log, withPeople: true);
      expect(text, contains('Never in here: a key, a token, the text of a message.'));
      expect(text.toLowerCase(), isNot(contains('api_key')));
      expect(text.toLowerCase(), isNot(contains('password')));
    });
  });

  group('two buttons on every card', () {
    FakeCli fake() => FakeCli(
      entries: [
        entryOf(bridge(), ai: const AiReach(service: 'api.openai.com', place: 'outside')),
        brokenEntry('broken', 'configuration: `model` is empty'),
      ],
    );

    testWidgets('the first copies status and log without the people', (tester) async {
      final copied = clipboardOf(tester);
      await open(tester, fake());
      await tester.tap(find.byKey(const ValueKey('copy-ai-bridge')));
      await tester.pumpAndSettle();

      expect(copied.single, contains('Status: Stopped'));
      expect(copied.single, contains('Discord, German ⇄ Matrix, English'));
      expect(copied.single, isNot(contains('Robin')));
      // Die Maske sagt, was im Text steht und was nicht.
      expect(find.textContaining('No people, no key, no token'), findsOneWidget);
    });

    testWidgets('the second copies the full profile and says that it holds the names', (
      tester,
    ) async {
      final copied = clipboardOf(tester);
      await open(tester, fake());
      await tester.tap(find.byKey(const ValueKey('copy-ai-people-bridge')));
      await tester.pumpAndSettle();

      expect(copied.single, contains('"name": "Robin"'));
      expect(find.textContaining('it holds the names you give people'), findsOneWidget);
    });

    testWidgets('what the fish said in this run is in the text', (tester) async {
      final copied = clipboardOf(tester);
      final cli = fake();
      await open(tester, cli);
      await tester.tap(find.byKey(const ValueKey('fish-switch')));
      await tester.pumpAndSettle();
      cli.lastRun!.emit('rusty-fish: [bridge] serving');
      cli.lastRun!.emit('rusty-fish: [bridge] cannot see channel 111: no such channel');
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('copy-ai-bridge')));
      await tester.pumpAndSettle();

      expect(copied.single, contains('Status: Running'));
      expect(copied.single, contains('[bridge] cannot see channel 111: no such channel'));
    });

    testWidgets('a profile the program refuses has only the first', (tester) async {
      final copied = clipboardOf(tester);
      await open(tester, fake());
      expect(find.byKey(const ValueKey('copy-ai-people-broken')), findsNothing);
      await tester.tap(find.byKey(const ValueKey('copy-ai-broken')));
      await tester.pumpAndSettle();
      expect(copied.single, contains('The program refuses this profile'));
    });
  });

  // Nemesus am 23.11.2026, beim Überfliegen der Karten mit der Maus: zwei
  // Hilfetexte gingen auf, „Das finde ich sehr störend."
  group('what a line cannot say', () {
    testWidgets('appears on a click on the sign at its end, not when the mouse passes', (
      tester,
    ) async {
      await open(
        tester,
        FakeCli(
          entries: [
            entryOf(bridge(), ai: const AiReach(service: 'api.openai.com', place: 'outside')),
          ],
        ),
      );
      // Kein Text, der beim Darüberfahren aufgeht.
      expect(find.byTooltip(connectionHint), findsNothing);
      expect(find.byTooltip(aiPlaceHint), findsNothing);
      expect(find.text(connectionHint), findsNothing);
      expect(find.text(aiPlaceHint), findsNothing);

      Finder sign(String key) =>
          find.descendant(of: find.byKey(ValueKey(key)), matching: find.byType(InkWell));

      await tester.tap(sign('hint-door-bridge-0'));
      await tester.pumpAndSettle();
      expect(find.text(connectionHint), findsOneWidget);
      expect(find.text(aiPlaceHint), findsNothing);

      await tester.tap(sign('hint-ai-bridge'));
      await tester.pumpAndSettle();
      expect(find.text(aiPlaceHint), findsOneWidget);

      // Ein zweiter Klick nimmt ihn wieder weg.
      await tester.tap(sign('hint-door-bridge-0'));
      await tester.pumpAndSettle();
      expect(find.text(connectionHint), findsNothing);
      expect(find.text(aiPlaceHint), findsOneWidget);
    });
  });
}
