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

/// Ein Platz im Raum: ein Kanal, eine Sprache. Seit dem 20.11.2026 kann er der
/// Tisch eines bestimmten Menschen sein.
///
/// Nemesus am 20.11.2026: „ob es nicht sinnvoller wäre, pro Discord-Nummer die
/// Muttersprache und die Neutralsprache festzulegen. [...] Jeder Rusty-Fish
/// würde für den API-Eigner übersetzen." Jeder Fisch liest an allen Tischen
/// mit, schreibt aber nur an die, die er bedient.
class Seat {
  const Seat({
    required this.channel,
    required this.lang,
    this.gender,
    this.owner,
    this.served = true,
  });

  final String channel;

  /// Die Sprache des Platzes. Sie gilt, solange der Besitzer keine
  /// Muttersprache nennt.
  final String lang;
  final Gender? gender;

  /// Die Discord-Kennung des Menschen, dessen Tisch das ist. Ohne Angabe
  /// gehört der Platz niemandem im Besonderen, wie vor dem 20.11.2026.
  final String? owner;

  /// Ob dieser Fisch an diesem Tisch schreibt. Das entscheidet das Programm
  /// und nennt es nur in der Liste der Profile; die Maske schickt es nie
  /// zurück. Ein älteres Programm nennt es nicht: dort bedient der Fisch
  /// jeden Tisch.
  final bool served;

  factory Seat.fromJson(Map<String, dynamic> json) => Seat(
    channel: '${json['channel'] ?? ''}',
    lang: '${json['lang'] ?? ''}',
    gender: genderFromJson(json['gender']),
    owner: _text(json['owner']),
    served: json['served'] != false,
  );

  /// Was nicht gesetzt ist, geht nicht hinaus. Ein älteres Programm lehnt
  /// Felder ab, die es nicht kennt, und nimmt ein Profil ohne die neuen
  /// Angaben so weiter an.
  Map<String, dynamic> toJson() => {
    'channel': channel,
    'lang': lang,
    'gender': genderToJson(gender),
    'owner': ?owner,
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
///
/// Seit dem 20.11.2026 gehören auch Muttersprache und neutrale Sprachen zum
/// Menschen und nicht mehr nur zum Kanal, so wie zuvor schon die Form.
class Person {
  const Person({
    required this.id,
    this.gender,
    this.lang,
    this.neutral,
    this.invited = false,
  });

  final String id;

  /// Die grammatische Form. Bis zum 20.11.2026 Pflicht, seitdem freiwillig:
  /// wer nur seine Muttersprache nennen will, muss keine Form angeben.
  final Gender? gender;

  /// Die Muttersprache. In sie wird an seinem Tisch übersetzt.
  final String? lang;

  /// Sprachen, die für diesen Menschen unübersetzt bleiben. Ohne Angabe gilt,
  /// was im Raum steht. Eine leere Liste ist eine andere Angabe: für diesen
  /// Menschen bleibt dann nichts unübersetzt.
  final List<String>? neutral;

  /// Nemesus am 20.11.2026: „Es sollte auch die Funktion ‚Einladen' geben. Und
  /// dann ein Haken, wer eingeladen wird." Wen der Gastgeber einlädt, für den
  /// übersetzt sein Fisch auf seine Rechnung.
  final bool invited;

  factory Person.fromJson(Map<String, dynamic> json) {
    final neutral = json['neutral'];
    return Person(
      id: '${json['id'] ?? ''}',
      // Bis zum 20.11.2026 stand hier „?? Gender.female": eine fehlende Form
      // wurde still zur weiblichen. Seit die Form freiwillig ist, bleibt eine
      // fehlende Angabe eine fehlende Angabe.
      gender: genderFromJson(json['gender']),
      lang: _text(json['lang']),
      neutral: neutral is List ? _strings(neutral) : null,
      invited: json['invited'] == true,
    );
  }

  /// Was nicht gesetzt ist, geht nicht hinaus (siehe [Seat.toJson]).
  Map<String, dynamic> toJson() => {
    'id': id,
    'gender': genderToJson(gender),
    'lang': ?lang,
    'neutral': ?neutral,
    if (invited) 'invited': true,
  };
}

/// Für wen dieser Fisch übersetzt, als eine Zeile für die Karte.
///
/// Die Regel wendet das Programm an und nennt sie je Platz als „served". Die
/// Maske zählt nur zusammen, was es gesagt hat.
String servesText(ProfileDoc doc) {
  final seats = [for (final room in doc.rooms) ...room.seats];
  // Ein Profil aus der Zeit vor den Tischen: kein Platz gehört jemandem, und
  // der Fisch schreibt überall.
  if (seats.every((seat) => seat.owner == null)) {
    return 'Translates at every table';
  }
  final served = seats.where((seat) => seat.served);
  final host = served.any((seat) => seat.owner == doc.host);
  final guests = {
    for (final seat in served)
      if (seat.owner != null && seat.owner != doc.host) seat.owner!,
  }.length;
  final open = served.where((seat) => seat.owner == null).length;

  final people = [
    if (host) 'you',
    if (guests == 1) '1 invited guest',
    if (guests > 1) '$guests invited guests',
  ];
  final tables = open == 1 ? '1 table' : '$open tables';
  if (people.isEmpty) {
    return open == 0 ? 'Translates at no table' : 'Translates at $tables without an owner';
  }
  final forWhom = 'Translates for ${people.join(' and ')}';
  return open == 0 ? forWhom : '$forWhom, and at $tables without an owner';
}

/// Tische, an denen dieser Fisch nur mitliest. Dort schreibt der Fisch dessen,
/// dem der Tisch gehört, oder niemand.
String? readOnlyText(ProfileDoc doc) {
  final count = [
    for (final room in doc.rooms)
      for (final seat in room.seats)
        if (!seat.served) seat,
  ].length;
  return switch (count) {
    0 => null,
    1 => '1 table is read only',
    _ => '$count tables are read only',
  };
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

/// Ein KI-Dienst, so wie das Programm ihn nennt (`providers --json`).
///
/// Nemesus am 20.11.2026: „Die Möglichkeit, die KIs im Profil zu wechseln
/// sollte jedoch erhalten bleiben." Welche Dienste es gibt und wie sie heißen,
/// weiß das Programm. Die Maske zeigt seine Liste und erfindet keine eigene.
class Provider {
  const Provider({
    required this.id,
    required this.label,
    required this.baseUrl,
    required this.needsKey,
  });

  /// Die Kennung im Profil und zugleich der Name des Schlüssels in der Ablage.
  final String id;

  /// So heißt der Dienst für Menschen, etwa „Grok (xAI)".
  final String label;

  /// Die übliche Adresse des Dienstes. Fehlt sie, muss das Profil eine nennen.
  final String? baseUrl;

  /// Ob der Dienst ohne Schlüssel gar nicht antwortet. Ein eigenes Modell auf
  /// dem eigenen Rechner braucht oft keinen.
  final bool needsKey;

  /// Ohne übliche Adresse muss die Person selbst eine angeben.
  bool get needsAddress => baseUrl == null;

  factory Provider.fromJson(Map<String, dynamic> json) {
    final id = '${json['id'] ?? ''}';
    final label = json['label'];
    final baseUrl = json['base_url'];
    return Provider(
      id: id,
      label: label is String && label.trim().isNotEmpty ? label : id,
      baseUrl: baseUrl is String && baseUrl.trim().isNotEmpty ? baseUrl : null,
      // Im Zweifel braucht ein Dienst einen Schlüssel: lieber einmal zu viel
      // danach fragen als einen Start erlauben, der sicher scheitert.
      needsKey: json['needs_key'] != false,
    );
  }
}

/// Die Kennung, die gilt, wenn ein Profil keinen Dienst nennt. Profile aus der
/// Zeit vor der Auswahl sprachen alle mit OpenAI.
const String defaultProviderId = 'openai';

/// Was die Maske zeigt, solange das Programm seine Dienste nicht nennen kann
/// (ein älteres Programm kennt den Aufruf noch nicht). Genau der eine Dienst,
/// den es vor der Auswahl gab, damit die Maske benutzbar bleibt.
const List<Provider> fallbackProviders = [
  Provider(
    id: defaultProviderId,
    label: 'ChatGPT (OpenAI)',
    baseUrl: 'https://api.openai.com/v1',
    needsKey: true,
  ),
];

/// Ein Profil, so wie es bearbeitet und gespeichert wird.
class ProfileDoc {
  const ProfileDoc({
    required this.name,
    this.provider = defaultProviderId,
    required this.model,
    required this.baseUrl,
    required this.host,
    required this.aiNames,
    required this.limits,
    required this.rooms,
    required this.people,
  });

  final String name;

  /// Die Kennung des KI-Dienstes, etwa „openai" oder „xai".
  final String provider;
  final String model;

  /// Eine abweichende Adresse. Bei einem Dienst ohne übliche Adresse Pflicht;
  /// das prüft das Programm.
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
    final provider = json['provider'];
    return ProfileDoc(
      name: '${json['name'] ?? ''}',
      // Fehlt die Angabe (ein älteres Programm kennt sie nicht), gilt OpenAI.
      provider: provider is String && provider.trim().isNotEmpty
          ? provider.trim()
          : defaultProviderId,
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
    'provider': provider,
    'model': model,
    'base_url': baseUrl,
    'host': host,
    'ai_names': aiNames,
    'limits': limits.toJson(),
    'rooms': [for (final room in rooms) room.toJson()],
    'people': [for (final person in people) person.toJson()],
  };

  /// Der Mensch mit dieser Kennung, falls das Profil ihn nennt.
  Person? person(String? id) {
    if (id == null) {
      return null;
    }
    for (final person in people) {
      if (person.id == id) {
        return person;
      }
    }
    return null;
  }

  /// In welche Sprache an diesem Tisch übersetzt wird: die Muttersprache des
  /// Besitzers, sonst die Sprache des Platzes. So wendet es das Programm an;
  /// die Maske zeigt es nur.
  String languageAt(Seat seat) => person(seat.owner)?.lang ?? seat.lang;

  ProfileDoc renamed(String newName) => ProfileDoc(
    name: newName,
    provider: provider,
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

/// Der Eintrag der Ablage, unter dem der Token des Bots liegt. Die Schlüssel
/// der KI-Dienste liegen unter der Kennung ihres Dienstes ([Provider.id]).
const String botTokenEntry = 'discord';

/// So heißt der Token des Bots in der Oberfläche.
const String botTokenLabel = 'Bot token';

/// Welche Geheimnisse in der Ablage des Systems liegen. Die Werte selbst
/// kommen nie in die Oberfläche zurück.
///
/// Bis zum 20.11.2026 gab es genau zwei Geheimnisse, den einen KI-Schlüssel
/// und den Token. Seit jedes Profil seinen Dienst wählen kann, hat jeder
/// Dienst seinen eigenen Schlüssel. Ein Eintrag, den das Programm nicht
/// nennt, gilt als nicht abgelegt.
class KeyStatus {
  const KeyStatus(this.stored);

  final Map<String, bool> stored;

  bool has(String entry) => stored[entry] == true;

  bool get discord => has(botTokenEntry);

  factory KeyStatus.fromJson(Map<String, dynamic> json) =>
      KeyStatus({for (final item in json.entries) item.key: item.value == true});
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

/// Ein Text oder nichts. Ein leerer Text ist keine Angabe.
String? _text(Object? value) =>
    value is String && value.trim().isNotEmpty ? value.trim() : null;

List<String> _strings(Object? value) =>
    value is List ? [for (final item in value) '$item'] : const [];

Map<String, dynamic> _map(Object? value) =>
    value is Map ? value.map((key, item) => MapEntry('$key', item)) : const {};

List<Map<String, dynamic>> _maps(Object? value) =>
    value is List ? [for (final item in value) _map(item)] : const [];
