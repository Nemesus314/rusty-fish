// Das Zusammenspiel mit dem echten Programm `rusty-fish`.
//
// Läuft nur, wenn beide Umgebungsvariablen gesetzt sind:
//   RUSTY_FISH_EXE   das Programm
//   RUSTY_FISH_HOME  ein Wegwerf-Ordner
// Der Wegwerf-Ordner ist Pflicht, damit dieser Test nie echte Profile anfasst.
// Geheimnisse werden nur abgefragt, nie gesetzt oder gelöscht, und es wird
// kein Fisch gestartet, der sich bei Discord anmelden könnte.

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:rusty_fish_desk/cli.dart';
import 'package:rusty_fish_desk/model.dart';

void main() {
  final exe = Platform.environment['RUSTY_FISH_EXE'];
  final home = Platform.environment['RUSTY_FISH_HOME'];
  final skip = (exe == null || home == null)
      ? 'needs RUSTY_FISH_EXE and RUSTY_FISH_HOME (a throwaway folder)'
      : null;

  ProfileDoc profile(String name) => ProfileDoc(
    name: name,
    model: 'some-model',
    baseUrl: null,
    host: '42',
    aiNames: const ['Fish', 'Фиш'],
    limits: const Limits(
      translationTokensPerDay: 5000,
      questionsPerGuestPerDay: 3,
    ),
    rooms: const [
      Room(
        passThrough: ['English'],
        contextLines: 4,
        seats: [
          Seat(channel: '7', lang: 'German', gender: Gender.male),
          Seat(channel: '8', lang: 'Russian'),
        ],
      ),
    ],
    people: const [Person(id: '9', gender: Gender.female)],
  );

  test('save, list, show and delete go through the real program', () async {
    final cli = ProcessFishCli(exe!);
    const name = 'desk-probe';
    final doc = profile(name);

    await cli.saveProfile(doc);
    final listed = (await cli.listProfiles()).profiles.firstWhere(
      (entry) => entry.name == name,
    );
    expect(listed.ok, isTrue);
    expect(listed.running, isFalse);
    expect(listed.doc!.toJson(), doc.toJson());

    final shown = await cli.showProfile(name);
    expect(shown.toJson(), doc.toJson());

    await cli.deleteProfile(name);
    final after = await cli.listProfiles();
    expect(after.profiles.where((entry) => entry.name == name), isEmpty);
  }, skip: skip);

  test('what the program refuses comes back in its own words', () async {
    final cli = ProcessFishCli(exe!);
    const oneSeat = ProfileDoc(
      name: 'desk-probe-bad',
      model: 'some-model',
      baseUrl: null,
      host: '42',
      aiNames: [],
      limits: Limits(translationTokensPerDay: 1, questionsPerGuestPerDay: 1),
      rooms: [
        Room(
          passThrough: [],
          contextLines: 1,
          seats: [Seat(channel: '7', lang: 'German')],
        ),
      ],
      people: [],
    );
    await expectLater(
      cli.saveProfile(oneSeat),
      throwsA(
        isA<CliException>().having(
          (e) => e.message,
          'message',
          allOf(contains('two seats'), isNot(startsWith('rusty-fish: '))),
        ),
      ),
    );
    final list = await cli.listProfiles();
    expect(list.profiles.where((entry) => entry.name == 'desk-probe-bad'), isEmpty);
  }, skip: skip);

  test('the key status says only yes or no', () async {
    final cli = ProcessFishCli(exe!);
    final status = await cli.keyStatus();
    expect(status.stored.keys, containsAll(['openai', 'discord']));
    expect(status.stored.values, everyElement(isA<bool>()));
  }, skip: skip);

  // Die drei folgenden Tests brauchen ein Programm, das die Auswahl des
  // KI-Dienstes schon kennt. Ein älteres wird erkannt und übersprungen, damit
  // dieser Test nicht an einem Stand scheitert, den die Maske aushalten muss.
  Future<List<Provider>?> listedProviders(ProcessFishCli cli) async {
    try {
      return await cli.providers();
    } on CliException {
      return null;
    }
  }

  test('the program names its AI services', () async {
    final cli = ProcessFishCli(exe!);
    final providers = await listedProviders(cli);
    if (providers == null) {
      markTestSkipped('this rusty-fish does not know `providers` yet');
      return;
    }
    expect(providers.map((p) => p.id), containsAll(['openai', 'custom']));
    final custom = providers.firstWhere((p) => p.id == 'custom');
    expect(custom.needsAddress, isTrue);
    final status = await cli.keyStatus();
    // Für jeden Dienst sagt die Ablage ja oder nein.
    for (final provider in providers) {
      expect(status.stored.containsKey(provider.id), isTrue, reason: provider.id);
    }
  }, skip: skip);

  test('the chosen service goes through the real program and back', () async {
    final cli = ProcessFishCli(exe!);
    final providers = await listedProviders(cli);
    if (providers == null) {
      markTestSkipped('this rusty-fish does not know `providers` yet');
      return;
    }
    const name = 'desk-probe-service';
    const doc = ProfileDoc(
      name: name,
      provider: 'custom',
      model: 'local-model',
      baseUrl: 'http://localhost:11434/v1',
      host: '42',
      aiNames: [],
      limits: Limits(translationTokensPerDay: 5000, questionsPerGuestPerDay: 3),
      rooms: [
        Room(
          passThrough: [],
          contextLines: 2,
          seats: [
            Seat(channel: '7', lang: 'German'),
            Seat(channel: '8', lang: 'Korean'),
          ],
        ),
      ],
      people: [],
    );
    await cli.saveProfile(doc);
    final shown = await cli.showProfile(name);
    expect(shown.provider, 'custom');
    expect(shown.baseUrl, 'http://localhost:11434/v1');
    final listed = (await cli.listProfiles()).profiles.firstWhere(
      (entry) => entry.name == name,
    );
    expect(listed.doc!.provider, 'custom');
    await cli.deleteProfile(name);
  }, skip: skip);

  test('an own model without an address is refused in the program\'s words', () async {
    final cli = ProcessFishCli(exe!);
    final providers = await listedProviders(cli);
    if (providers == null) {
      markTestSkipped('this rusty-fish does not know `providers` yet');
      return;
    }
    const doc = ProfileDoc(
      name: 'desk-probe-no-address',
      provider: 'custom',
      model: 'local-model',
      baseUrl: null,
      host: '42',
      aiNames: [],
      limits: Limits(translationTokensPerDay: 1, questionsPerGuestPerDay: 1),
      rooms: [
        Room(
          passThrough: [],
          contextLines: 1,
          seats: [
            Seat(channel: '7', lang: 'German'),
            Seat(channel: '8', lang: 'Korean'),
          ],
        ),
      ],
      people: [],
    );
    await expectLater(cli.saveProfile(doc), throwsA(isA<CliException>()));
    final list = await cli.listProfiles();
    expect(
      list.profiles.where((entry) => entry.name == 'desk-probe-no-address'),
      isEmpty,
    );
  }, skip: skip);

  test('a run that cannot start says why and ends', () async {
    final cli = ProcessFishCli(exe!);
    final run = await cli.run('no-such-profile');
    final lines = <String>[];
    final listening = run.lines.listen(lines.add);
    final code = await run.exitCode.timeout(const Duration(seconds: 20));
    await Future<void>.delayed(const Duration(milliseconds: 200));
    await listening.cancel();
    expect(code, isNot(0));
    expect(lines, isNotEmpty);
  }, skip: skip);
}
