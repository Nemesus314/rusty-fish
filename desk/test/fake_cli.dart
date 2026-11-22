// Attrappe statt des Programms `rusty-fish`. Die Tests der Maske brauchen
// weder das Programm noch Discord noch Matrix noch einen Schlüssel.

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

  /// Was die Maske dem laufenden Fisch gesagt hat, Zeile für Zeile.
  final List<String> said = [];

  @override
  void say(String line) => said.add(line);

  @override
  void stop() {
    stopped = true;
    end(-1);
  }
}

/// Die Dienste, so wie das Programm sie nennt.
const fakeProviders = [
  Provider(
    id: 'openai',
    label: 'ChatGPT (OpenAI)',
    baseUrl: 'https://api.openai.com/v1',
    needsKey: true,
  ),
  Provider(
    id: 'xai',
    label: 'Grok (xAI)',
    baseUrl: 'https://api.x.ai/v1',
    needsKey: true,
  ),
  Provider(
    id: 'google',
    label: 'Gemini (Google)',
    baseUrl: 'https://generativelanguage.googleapis.com/v1beta/openai',
    needsKey: true,
  ),
  Provider(
    id: 'anthropic',
    label: 'Claude (Anthropic)',
    baseUrl: 'https://api.anthropic.com/v1',
    needsKey: true,
  ),
  Provider(
    id: 'custom',
    label: 'Own model or other service',
    baseUrl: null,
    needsKey: false,
  ),
];

class FakeCli implements FishCli {
  FakeCli({
    required this.entries,
    this.keys = const KeyStatus({'openai': true, 'discord': false}),
  });

  List<ProfileEntry> entries;
  KeyStatus keys;

  String? saveError;
  String? runError;

  /// Gesetzt: das Programm kennt den Aufruf `providers` nicht (älterer Stand).
  String? providersError;
  List<Provider> providerList = fakeProviders;
  List<String> modelList = ['alpha-model', 'beta-model'];

  final saved = <ProfileDoc>[];
  final deleted = <String>[];
  final secrets = <String, String>{};
  final forgotten = <String>[];
  final started = <String>[];

  /// Mit welchem Dienst und welcher Adresse nach Modellen gefragt wurde.
  final modelRequests = <({String? provider, String? baseUrl})>[];
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
  Future<List<Provider>> providers() async {
    final error = providersError;
    if (error != null) {
      throw CliException(error);
    }
    return providerList;
  }

  @override
  Future<void> setKey(String entry, String value) async {
    secrets[entry] = value;
    keys = KeyStatus({...keys.stored, entry: true});
  }

  @override
  Future<void> forgetKey(String entry) async {
    forgotten.add(entry);
    secrets.remove(entry);
    keys = KeyStatus({...keys.stored, entry: false});
  }

  @override
  Future<List<String>> models({String? provider, String? baseUrl}) async {
    modelRequests.add((provider: provider, baseUrl: baseUrl));
    return modelList;
  }

  @override
  Future<FishRun> runAll() async {
    final error = runError;
    if (error != null) {
      throw CliException(error);
    }
    runs++;
    return lastRun = FakeRun();
  }

  /// Wie oft die Maske einen Fisch gestartet hat.
  int runs = 0;

  /// Ob und als wer der Fisch bei Matrix angemeldet ist.
  MatrixStatus matrix = const MatrixStatus(loggedIn: false);

  /// Gesetzt: das Programm kennt die Aufrufe zu Matrix nicht (älterer Stand).
  String? matrixUnknown;

  /// Gesetzt: das Programm lehnt die Anmeldung ab, mit diesem Grund.
  String? matrixLoginError;

  /// Womit die Maske den Fisch angemeldet hat. Das Passwort steht hier nur,
  /// damit ein Test sehen kann, dass es ankam; es ist keines.
  final matrixLogins = <({String user, String password})>[];
  int matrixLogouts = 0;

  @override
  Future<MatrixStatus> matrixStatus() async {
    final unknown = matrixUnknown;
    if (unknown != null) {
      throw CliException(unknown);
    }
    return matrix;
  }

  @override
  Future<String> matrixLogin(String user, String password) async {
    final error = matrixLoginError;
    if (error != null) {
      throw CliException(error);
    }
    matrixLogins.add((user: user, password: password));
    matrix = MatrixStatus(loggedIn: true, user: user, homeserver: 'https://matrix.example.org/');
    keys = KeyStatus({...keys.stored, matrixEntry: true});
    return 'Logged in to Matrix as $user at https://matrix.example.org/.\n'
        'Note: the device could not be cross-signed (not allowed).\n';
  }

  @override
  Future<String> matrixLogout() async {
    matrixLogouts++;
    matrix = const MatrixStatus(loggedIn: false);
    keys = KeyStatus({...keys.stored, matrixEntry: false});
    return 'Logged out of Matrix. The device store is empty.\n';
  }

  /// Gesetzt: das Programm kennt den Aufruf `guide` nicht (älterer Stand).
  String? guideError;

  @override
  Future<String> guide() async {
    final error = guideError;
    if (error != null) {
      throw CliException(error);
    }
    return '# Rusty Fish: what it is\n';
  }

  /// Gesetzt: das Programm kann den Link nicht nennen, mit diesem Grund.
  String? inviteError;

  @override
  Future<String> inviteLink() async {
    final error = inviteError;
    if (error != null) {
      throw CliException(error);
    }
    return 'https://discord.com/oauth2/authorize?client_id=42&scope=bot&permissions=536939520';
  }
}

/// Ein gültiges Profil mit zwei Plätzen.
ProfileDoc docOf(
  String name, {
  String provider = 'openai',
  String? baseUrl,
  List<String> aiNames = const [],
  List<Person> people = const [],
  List<Seat>? seats,
}) {
  return ProfileDoc(
    name: name,
    provider: provider,
    model: 'some-model',
    baseUrl: baseUrl,
    host: '100',
    aiNames: aiNames,
    limits: const Limits(
      translationTokensPerDay: 100000,
      questionsPerGuestPerDay: 10,
    ),
    rooms: [
      Room(
        passThrough: const ['English'],
        contextLines: 10,
        // Ohne Angabe: zwei Plätze ohne Besitzer, wie ein Profil aus der Zeit,
        // bevor die Sprache an die Person gebunden wurde.
        seats:
            seats ??
            const [
              Seat(channel: '111', lang: 'German', gender: Gender.male),
              Seat(channel: '222', lang: 'Russian', gender: Gender.female),
            ],
      ),
    ],
    people: people,
  );
}

/// Gastgeber 100 (Deutsch) und Gast 200 (Russisch), jeder mit eigenem Tisch.
/// [guestInvited] und [guestServed] gehören zusammen: das Programm bedient den
/// Tisch eines Gastes nur, wenn er eingeladen ist.
ProfileDoc tablesDoc(String name, {bool guestInvited = true, bool? guestServed}) {
  return docOf(
    name,
    people: [
      const Person(id: '100', gender: Gender.male, lang: 'German'),
      Person(
        id: '200',
        gender: Gender.female,
        lang: 'Russian',
        neutral: const ['English'],
        invited: guestInvited,
      ),
    ],
    seats: [
      const Seat(channel: '111', lang: 'German', owner: '100'),
      Seat(
        channel: '222',
        lang: 'Russian',
        owner: '200',
        served: guestServed ?? guestInvited,
      ),
    ],
  );
}

ProfileEntry entryOf(ProfileDoc doc, {Usage? usage, bool running = false, AiReach? ai}) {
  return ProfileEntry(
    ai: ai,
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
