// Attrappe statt des Programms `rusty-fish`. Die Tests der Maske brauchen
// weder das Programm noch Discord noch einen Schlüssel.

import 'dart:async';

import 'package:rusty_fish_desk/cli.dart';
import 'package:rusty_fish_desk/model.dart';

class FakeRun implements FishRun {
  final _lines = StreamController<String>.broadcast();
  final _exit = Completer<int>();
  bool stopped = false;

  void emit(String line) => _lines.add(line);

  void end(int code) {
    if (!_exit.isCompleted) {
      _exit.complete(code);
    }
  }

  @override
  Stream<String> get lines => _lines.stream;

  @override
  Future<int> get exitCode => _exit.future;

  @override
  void stop() {
    stopped = true;
    end(-1);
  }
}

class FakeCli implements FishCli {
  FakeCli({
    required this.entries,
    this.keys = const KeyStatus(openai: true, discord: false),
  });

  List<ProfileEntry> entries;
  KeyStatus keys;

  String? saveError;
  String? runError;
  List<String> modelList = ['alpha-model', 'beta-model'];

  final saved = <ProfileDoc>[];
  final deleted = <String>[];
  final secrets = <SecretKind, String>{};
  final forgotten = <SecretKind>[];
  final started = <String>[];
  FakeRun? lastRun;

  @override
  Future<ProfileList> listProfiles() async =>
      ProfileList(dir: r'C:\fish\profiles', profiles: List.of(entries));

  @override
  Future<ProfileDoc> showProfile(String name) async {
    for (final entry in entries) {
      if (entry.name == name && entry.doc != null) {
        return entry.doc!;
      }
    }
    throw CliException('profile $name not found');
  }

  @override
  Future<void> saveProfile(ProfileDoc doc) async {
    final error = saveError;
    if (error != null) {
      throw CliException(error);
    }
    saved.add(doc);
    entries = [
      for (final entry in entries)
        if (entry.name != doc.name) entry,
      entryOf(doc),
    ];
  }

  @override
  Future<void> deleteProfile(String name) async {
    deleted.add(name);
    entries = [
      for (final entry in entries)
        if (entry.name != name) entry,
    ];
  }

  @override
  Future<KeyStatus> keyStatus() async => keys;

  @override
  Future<void> setKey(SecretKind kind, String value) async {
    secrets[kind] = value;
    keys = KeyStatus(
      openai: kind == SecretKind.openai ? true : keys.openai,
      discord: kind == SecretKind.discord ? true : keys.discord,
    );
  }

  @override
  Future<void> forgetKey(SecretKind kind) async {
    forgotten.add(kind);
    secrets.remove(kind);
    keys = KeyStatus(
      openai: kind == SecretKind.openai ? false : keys.openai,
      discord: kind == SecretKind.discord ? false : keys.discord,
    );
  }

  @override
  Future<List<String>> models({String? baseUrl}) async => modelList;

  @override
  Future<FishRun> run(String name) async {
    final error = runError;
    if (error != null) {
      throw CliException(error);
    }
    started.add(name);
    return lastRun = FakeRun();
  }
}

/// Ein gültiges Profil mit zwei Plätzen.
ProfileDoc docOf(
  String name, {
  List<String> aiNames = const [],
  List<Person> people = const [],
}) {
  return ProfileDoc(
    name: name,
    model: 'some-model',
    baseUrl: null,
    host: '100',
    aiNames: aiNames,
    limits: const Limits(
      translationTokensPerDay: 100000,
      questionsPerGuestPerDay: 10,
    ),
    rooms: const [
      Room(
        passThrough: ['English'],
        contextLines: 10,
        seats: [
          Seat(channel: '111', lang: 'German', gender: Gender.male),
          Seat(channel: '222', lang: 'Russian', gender: Gender.female),
        ],
      ),
    ],
    people: people,
  );
}

ProfileEntry entryOf(ProfileDoc doc, {Usage? usage, bool running = false}) {
  return ProfileEntry(
    name: doc.name,
    path: 'C:\\fish\\profiles\\${doc.name}.toml',
    ok: true,
    error: null,
    doc: doc,
    usage: usage,
    running: running,
  );
}

ProfileEntry brokenEntry(String name, String error) {
  return ProfileEntry(
    name: name,
    path: 'C:\\fish\\profiles\\$name.toml',
    ok: false,
    error: error,
    doc: null,
    usage: null,
    running: false,
  );
}
