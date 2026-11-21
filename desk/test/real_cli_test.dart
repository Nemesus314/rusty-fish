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

  test('the program hands out its own description', () async {
    final cli = ProcessFishCli(exe!);
    final String text;
    try {
      text = await cli.guide();
    } on CliException {
      markTestSkipped('this rusty-fish does not know `guide` yet');
      return;
    }
    expect(text, startsWith('# Rusty Fish'));
    expect(text.trimRight(), matches(RegExp(r'rusty-fish \d+\.\d+\.\d+\.$')));
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

  test('names, a table for someone without an id, and where the AI reads', () async {
    final cli = ProcessFishCli(exe!);
    const name = 'desk-probe-names';
    // Der Gastgeber steht mit seinem Namen da. Robin hat einen Tisch und ist
    // eingeladen, aber noch ohne Kennung. Die KI läuft auf diesem Rechner.
    const doc = ProfileDoc(
      name: name,
      provider: 'custom',
      model: 'local-model',
      baseUrl: 'http://localhost:11434/v1',
      host: 'Nemo',
      aiNames: [],
      limits: Limits(translationTokensPerDay: 1, questionsPerGuestPerDay: 1),
      rooms: [
        Room(
          passThrough: [],
          contextLines: 1,
          seats: [
            Seat(channel: '7', lang: 'German', owner: 'Nemo'),
            Seat(channel: '8', lang: 'English', owner: 'Robin'),
          ],
        ),
      ],
      people: [
        Person(id: '42', name: 'Nemo'),
        Person(name: 'Robin', lang: 'Russian', invited: true),
      ],
    );
    try {
      await cli.saveProfile(doc);
    } on CliException {
      markTestSkipped('this rusty-fish does not know names yet');
      return;
    }
    addTearDown(() => cli.deleteProfile(name));

    final listed = (await cli.listProfiles()).profiles.firstWhere((entry) => entry.name == name);
    expect(listed.ok, isTrue, reason: '${listed.error}');
    final back = listed.doc!;
    expect(back.host, 'Nemo');
    expect(back.people.map((p) => p.name), ['Nemo', 'Robin']);
    expect(back.people[1].id, '', reason: 'no id yet');
    final seats = back.rooms.single.seats;
    expect(seats.map((s) => s.owner), ['Nemo', 'Robin']);
    expect(seats.map((s) => s.door), ['discord', 'discord']);
    // Der Tisch eines Eingeladenen wird bedient, auch ohne dessen Kennung.
    expect(seats.map((s) => s.served), [true, true]);
    expect(back.tableText(seats[1]), 'Robin · Russian');
    expect(servesText(back), 'Translates for you and Robin');

    // Wo die KI liest, sagt das Programm nach der Adresse.
    expect(listed.ai!.place, 'this_machine');
    expect(listed.ai!.service, 'localhost:11434');
    expect(aiPlaceText(listed.ai!), 'AI runs on this PC · localhost:11434');
  }, skip: skip);

  // Braucht ein Programm, das Tische, Muttersprachen und Einladungen schon
  // kennt. Ein älteres lehnt die neuen Felder ab; das wird erkannt und
  // übersprungen, denn diesen Stand muss die Maske aushalten.
  test('tables, mother tongues and invitations go through the real program', () async {
    final cli = ProcessFishCli(exe!);
    const name = 'desk-probe-tables';
    const doc = ProfileDoc(
      name: name,
      model: 'some-model',
      baseUrl: null,
      host: '42',
      aiNames: [],
      limits: Limits(translationTokensPerDay: 5000, questionsPerGuestPerDay: 3),
      rooms: [
        Room(
          passThrough: ['English'],
          contextLines: 2,
          seats: [
            Seat(channel: '7', lang: 'German', owner: '42'),
            Seat(channel: '8', lang: 'Russian', owner: '9'),
            Seat(channel: '10', lang: 'Korean', owner: '11'),
          ],
        ),
      ],
      people: [
        Person(id: '42', gender: Gender.male, lang: 'German'),
        Person(id: '9', lang: 'Russian', neutral: ['English'], invited: true),
        Person(id: '11', lang: 'Korean'),
      ],
    );
    try {
      await cli.saveProfile(doc);
    } on CliException catch (e) {
      markTestSkipped('this rusty-fish does not know tables yet: ${e.message}');
      return;
    }
    final shown = await cli.showProfile(name);
    expect(shown.toJson(), doc.toJson());
    final listed = (await cli.listProfiles()).profiles.firstWhere(
      (entry) => entry.name == name,
    );
    final seats = listed.doc!.rooms.single.seats;
    expect(seats.map((s) => s.owner), ['42', '9', '11']);
    // Bedient werden der Tisch des Gastgebers und der des eingeladenen Gastes.
    // Am dritten liest der Fisch nur mit.
    expect(seats.map((s) => s.served), [true, true, false]);
    expect(servesText(listed.doc!), 'Translates for you and 1 invited guest');
    expect(readOnlyText(listed.doc!), '1 table is read only');
    await cli.deleteProfile(name);
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
