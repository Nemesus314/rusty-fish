// Die Formen, in denen die Oberfläche und das Programm `rusty-fish` Profile
// austauschen. Die Oberfläche prüft nichts davon selbst: gültig ist, was das
// Programm annimmt. So gibt es genau eine Stelle, die Einstellungen beurteilt.

enum Gender { female, male }

Gender? genderFromJson(Object? value) {
  return switch (value) {
    'female' => Gender.female,
    'male' => Gender.male,
    _ => null,
  };
}

String? genderToJson(Gender? gender) => gender?.name;

/// Ein Platz im Raum: ein Kanal, eine Sprache.
class Seat {
  const Seat({required this.channel, required this.lang, this.gender});

  final String channel;
  final String lang;
  final Gender? gender;

  factory Seat.fromJson(Map<String, dynamic> json) => Seat(
    channel: '${json['channel'] ?? ''}',
    lang: '${json['lang'] ?? ''}',
    gender: genderFromJson(json['gender']),
  );

  Map<String, dynamic> toJson() => {
    'channel': channel,
    'lang': lang,
    'gender': genderToJson(gender),
  };
}

/// Ein Gespräch: mehrere Plätze, die einander gespiegelt werden.
class Room {
  const Room({
    required this.passThrough,
    required this.contextLines,
    required this.seats,
  });

  final List<String> passThrough;
  final int contextLines;
  final List<Seat> seats;

  factory Room.fromJson(Map<String, dynamic> json) => Room(
    passThrough: _strings(json['pass_through']),
    contextLines: _int(json['context_lines']),
    seats: _maps(json['seats']).map(Seat.fromJson).toList(),
  );

  Map<String, dynamic> toJson() => {
    'pass_through': passThrough,
    'context_lines': contextLines,
    'seats': [for (final seat in seats) seat.toJson()],
  };
}

/// Wer mit welchen Formen schreibt, egal an welchem Platz.
class Person {
  const Person({required this.id, required this.gender});

  final String id;
  final Gender gender;

  factory Person.fromJson(Map<String, dynamic> json) => Person(
    id: '${json['id'] ?? ''}',
    gender: genderFromJson(json['gender']) ?? Gender.female,
  );

  Map<String, dynamic> toJson() => {'id': id, 'gender': gender.name};
}

/// Was der Gastgeber pro Tag erlaubt.
class Limits {
  const Limits({
    required this.translationTokensPerDay,
    required this.questionsPerGuestPerDay,
  });

  final int translationTokensPerDay;
  final int questionsPerGuestPerDay;

  factory Limits.fromJson(Map<String, dynamic> json) => Limits(
    translationTokensPerDay: _int(json['translation_tokens_per_day']),
    questionsPerGuestPerDay: _int(json['questions_per_guest_per_day']),
  );

  Map<String, dynamic> toJson() => {
    'translation_tokens_per_day': translationTokensPerDay,
    'questions_per_guest_per_day': questionsPerGuestPerDay,
  };
}

/// Der Verbrauch eines Tages, so wie der Fisch ihn zuletzt gespeichert hat.
class Usage {
  const Usage({
    required this.day,
    required this.translationTokens,
    required this.allowedToday,
  });

  final String day;
  final int translationTokens;
  final int allowedToday;

  factory Usage.fromJson(Map<String, dynamic> json) => Usage(
    day: '${json['day'] ?? ''}',
    translationTokens: _int(json['translation_tokens']),
    allowedToday: _int(json['allowed_today']),
  );
}

/// Ein Profil, so wie es bearbeitet und gespeichert wird.
class ProfileDoc {
  const ProfileDoc({
    required this.name,
    required this.model,
    required this.baseUrl,
    required this.host,
    required this.aiNames,
    required this.limits,
    required this.rooms,
    required this.people,
  });

  final String name;
  final String model;
  final String? baseUrl;
  final String host;
  final List<String> aiNames;
  final Limits limits;
  final List<Room> rooms;
  final List<Person> people;

  /// Ein neues Profil: ein Raum mit zwei leeren Plätzen, denn weniger als
  /// zwei Plätze ergeben kein Gespräch. Die Zahlen sind die Vorgaben des
  /// Fischs; er nennt seine eigenen, falls sie sich einmal ändern.
  factory ProfileDoc.blank() => const ProfileDoc(
    name: '',
    model: '',
    baseUrl: null,
    host: '',
    aiNames: [],
    limits: Limits(
      translationTokensPerDay: 100000,
      questionsPerGuestPerDay: 10,
    ),
    rooms: [
      Room(
        // Entschieden von Nemesus am 20.11.2026: Englisch ist als neutrale
        // Sprache vorbelegt. Vorausgesetzt wird es nicht; wer es nicht will,
        // leert das Feld.
        passThrough: ['English'],
        contextLines: 10,
        seats: [
          Seat(channel: '', lang: ''),
          Seat(channel: '', lang: ''),
        ],
      ),
    ],
    people: [],
  );

  factory ProfileDoc.fromJson(Map<String, dynamic> json) {
    final baseUrl = json['base_url'];
    return ProfileDoc(
      name: '${json['name'] ?? ''}',
      model: '${json['model'] ?? ''}',
      baseUrl: baseUrl is String && baseUrl.trim().isNotEmpty ? baseUrl : null,
      host: '${json['host'] ?? ''}',
      aiNames: _strings(json['ai_names']),
      limits: Limits.fromJson(_map(json['limits'])),
      rooms: _maps(json['rooms']).map(Room.fromJson).toList(),
      people: _maps(json['people']).map(Person.fromJson).toList(),
    );
  }

  Map<String, dynamic> toJson() => {
    'name': name,
    'model': model,
    'base_url': baseUrl,
    'host': host,
    'ai_names': aiNames,
    'limits': limits.toJson(),
    'rooms': [for (final room in rooms) room.toJson()],
    'people': [for (final person in people) person.toJson()],
  };

  ProfileDoc renamed(String newName) => ProfileDoc(
    name: newName,
    model: model,
    baseUrl: baseUrl,
    host: host,
    aiNames: aiNames,
    limits: limits,
    rooms: rooms,
    people: people,
  );
}

/// Ein Eintrag der Übersicht. Ein kaputtes Profil hat kein [doc], dafür einen
/// Fehlertext vom Programm.
class ProfileEntry {
  const ProfileEntry({
    required this.name,
    required this.path,
    required this.ok,
    required this.error,
    required this.doc,
    required this.usage,
    required this.running,
  });

  final String name;
  final String path;
  final bool ok;
  final String? error;
  final ProfileDoc? doc;
  final Usage? usage;
  final bool running;

  factory ProfileEntry.fromJson(Map<String, dynamic> json) {
    final ok = json['ok'] == true;
    final usage = json['usage'];
    final error = json['error'];
    return ProfileEntry(
      name: '${json['name'] ?? ''}',
      path: '${json['path'] ?? ''}',
      ok: ok,
      error: error is String ? error : null,
      doc: ok ? ProfileDoc.fromJson(json) : null,
      usage: usage is Map ? Usage.fromJson(_map(usage)) : null,
      running: json['running'] == true,
    );
  }
}

class ProfileList {
  const ProfileList({required this.dir, required this.profiles});

  final String dir;
  final List<ProfileEntry> profiles;

  factory ProfileList.fromJson(Map<String, dynamic> json) => ProfileList(
    dir: '${json['dir'] ?? ''}',
    profiles: _maps(json['profiles']).map(ProfileEntry.fromJson).toList(),
  );
}

/// Ob die beiden Geheimnisse in der Ablage des Systems liegen. Die Werte
/// selbst kommen nie in die Oberfläche zurück.
class KeyStatus {
  const KeyStatus({required this.openai, required this.discord});

  final bool openai;
  final bool discord;

  factory KeyStatus.fromJson(Map<String, dynamic> json) =>
      KeyStatus(openai: json['openai'] == true, discord: json['discord'] == true);
}

/// Die Geheimnisse, die der Fisch kennt.
enum SecretKind {
  openai('openai', 'AI key'),
  discord('discord', 'Bot token');

  const SecretKind(this.cliName, this.label);

  /// So heißt das Geheimnis beim Aufruf des Programms.
  final String cliName;

  /// So heißt es in der Oberfläche.
  final String label;
}

/// Profilnamen, wie das Programm sie annimmt. Die Oberfläche prüft das nur,
/// um den Hinweis schon beim Tippen zu geben; entscheiden tut das Programm.
final RegExp profileNamePattern = RegExp(r'^[a-z0-9_-]{1,40}$');

/// 3231 wird zu „3,231". Englische Schreibweise, wie alle Texte der Oberfläche.
String groupThousands(int value) {
  final digits = value.abs().toString();
  final out = StringBuffer(value < 0 ? '-' : '');
  for (var i = 0; i < digits.length; i++) {
    if (i > 0 && (digits.length - i) % 3 == 0) {
      out.write(',');
    }
    out.write(digits[i]);
  }
  return out.toString();
}

int _int(Object? value) => switch (value) {
  int v => v,
  num v => v.toInt(),
  String v => int.tryParse(v) ?? 0,
  _ => 0,
};

List<String> _strings(Object? value) =>
    value is List ? [for (final item in value) '$item'] : const [];

Map<String, dynamic> _map(Object? value) =>
    value is Map ? value.map((key, item) => MapEntry('$key', item)) : const {};

List<Map<String, dynamic>> _maps(Object? value) =>
    value is List ? [for (final item in value) _map(item)] : const [];
