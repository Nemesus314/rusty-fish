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
    this.door = discordDoor,
    this.gender,
    this.owner,
    this.served = true,
  });

  /// Die Tür, zu der dieser Tisch gehört: Discord oder Matrix.
  final String door;

  /// Die Kennung des Kanals an dieser Tür; bei Matrix die des Raums.
  final String channel;

  /// Die Sprache des Platzes. Sie gilt, solange der Besitzer keine
  /// Muttersprache nennt.
  final String lang;
  final Gender? gender;

  /// Der Mensch, dessen Tisch das ist: sein Name, wie unter den Menschen des
  /// Profils, oder seine Kennung an der Tür (siehe [Person.ref]). Ohne Angabe
  /// gehört der Platz niemandem im Besonderen, wie vor dem 20.11.2026.
  final String? owner;

  /// Ob dieser Fisch an diesem Tisch schreibt. Das entscheidet das Programm
  /// und nennt es nur in der Liste der Profile; die Maske schickt es nie
  /// zurück. Ein älteres Programm nennt es nicht: dort bedient der Fisch
  /// jeden Tisch.
  final bool served;

  factory Seat.fromJson(Map<String, dynamic> json) => Seat(
    // Ein älteres Programm nennt keine Tür: dann ist es Discord.
    door: _text(json['door']) ?? discordDoor,
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
    // Discord ist die Tür, die gilt, wenn keine genannt ist.
    if (door != discordDoor) 'door': door,
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
    this.id = '',
    this.matrix = '',
    this.name,
    this.gender,
    this.formality,
    this.lang,
    this.neutral,
    this.invited = false,
  });

  /// Die Kennung an der Discord-Tür. Leer, solange man sie noch nicht hat:
  /// ein Tisch lässt sich schon vorher für jemanden bauen.
  final String id;

  /// Die Kennung an der Matrix-Tür, etwa „@robin:example.org". Leer, wenn es
  /// keine gibt. Ein Mensch kann an beiden Türen eine tragen.
  final String matrix;

  /// Die Kennung dieses Menschen an dieser Tür.
  String idAt(String door) => door == matrixDoor ? matrix : id;

  /// Der Name, den der Gastgeber diesem Menschen gibt. Er steht nur im Profil
  /// und in der Maske; weder Discord noch die KI bekommen ihn.
  ///
  /// Entschieden von Nemesus am 21.11.2026: die Namen, die er vergibt,
  /// gehören in die Maske, damit man sofort sieht, für wen ein Tisch gebaut ist.
  final String? name;

  /// Womit das Profil auf diesen Menschen zeigt (an einem Tisch, als
  /// Gastgeber): mit seinem Namen, sonst mit seiner Kennung.
  String get ref => name ?? (id.isNotEmpty ? id : matrix);

  /// Zeigt dieser Name oder diese Kennung auf diesen Menschen?
  bool answersTo(String ref) =>
      ref.isNotEmpty && (name == ref || id == ref || matrix == ref);

  /// Die grammatische Form. Bis zum 20.11.2026 Pflicht, seitdem freiwillig:
  /// wer nur seine Muttersprache nennen will, muss keine Form angeben.
  final Gender? gender;

  /// Wie dieser Mensch angesprochen wird, in dem, was er an seinem Tisch
  /// liest: [familiarFormality] oder [respectfulFormality]. Ohne Angabe folgt
  /// die Anrede dem, was geschrieben wurde.
  ///
  /// Seit dem 23.11.2026, für Sprachen, die Freunde und Ältere verschieden
  /// ansprechen. Nemesus an dem Tag: „in Korea redest du ja anders mit einer
  /// älteren Person als mit einer jüngeren." Ein Text und kein Aufzählungstyp:
  /// ein Wert, den diese Maske nicht kennt, bleibt beim Öffnen und Speichern
  /// stehen.
  final String? formality;

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
      // Bis zum 21.11.2026 hieß die Kennung im Austausch `id`.
      id: _text(json['discord']) ?? _text(json['id']) ?? '',
      matrix: _text(json['matrix']) ?? '',
      name: _text(json['name']),
      // Bis zum 20.11.2026 stand hier „?? Gender.female": eine fehlende Form
      // wurde still zur weiblichen. Seit die Form freiwillig ist, bleibt eine
      // fehlende Angabe eine fehlende Angabe.
      gender: genderFromJson(json['gender']),
      formality: _text(json['formality']),
      lang: _text(json['lang']),
      neutral: neutral is List ? _strings(neutral) : null,
      invited: json['invited'] == true,
    );
  }

  /// Was nicht gesetzt ist, geht nicht hinaus (siehe [Seat.toJson]).
  Map<String, dynamic> toJson() => {
    'name': ?name,
    if (id.isNotEmpty) 'discord': id,
    // Ein älteres Programm kennt das Feld nicht und lehnt es ab. Es geht nur
    // hinaus, wenn es etwas trägt.
    if (matrix.isNotEmpty) 'matrix': matrix,
    'gender': genderToJson(gender),
    // Ein älteres Programm kennt das Feld nicht und lehnt es ab. Es geht nur
    // hinaus, wenn es etwas trägt.
    'formality': ?formality,
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
  final host = served.any(doc.ownedByHost);
  // Jeder Gast einmal, auch wenn ihm mehrere Tische gehören oder das Profil
  // einmal mit dem Namen und einmal mit der Kennung auf ihn zeigt.
  final guests = {
    for (final seat in served)
      if (seat.owner != null && !doc.ownedByHost(seat))
        doc.person(seat.owner)?.ref ?? seat.owner!,
  };
  final open = served.where((seat) => seat.owner == null).length;

  // Wer einen Namen hat, wird genannt; die anderen werden gezählt.
  final named = [for (final ref in guests) ?doc.person(ref)?.name];
  final unnamed = guests.length - named.length;
  final people = [
    if (host) 'you',
    ...named,
    if (unnamed == 1) '1 invited guest',
    if (unnamed > 1) '$unnamed invited guests',
  ];
  final tables = open == 1 ? '1 table' : '$open tables';
  if (people.isEmpty) {
    return open == 0 ? 'Translates at no table' : 'Translates at $tables without an owner';
  }
  // "you and Robin", ab dreien "you, Robin and 1 invited guest".
  final last = people.removeLast();
  final forWhom = 'Translates for ${people.isEmpty ? last : '${people.join(', ')} and $last'}';
  return open == 0 ? forWhom : '$forWhom, and at $tables without an owner';
}

/// Die Tür, die gilt, wenn keine genannt ist.
const discordDoor = 'discord';

/// Die zweite Tür, seit dem 22.11.2026.
const matrixDoor = 'matrix';

/// Die Anrede „vertraut", wie das Programm sie nennt.
const familiarFormality = 'familiar';

/// Die Anrede „respektvoll", wie das Programm sie nennt.
const respectfulFormality = 'respectful';

/// Die Türen, die diese Maske kennt, in der Reihenfolge, in der sie Felder
/// zeigt.
const knownDoors = [discordDoor, matrixDoor];

/// Wie eine Tür in der Maske heißt.
String doorLabel(String door) => switch (door) {
  discordDoor => 'Discord',
  matrixDoor => 'Matrix',
  _ => door,
};

/// Kann diese Tür Ende zu Ende verschlüsseln? Discord kann es nicht.
bool doorCanEncrypt(String door) => door == matrixDoor;

/// Über welche Türen ein Raum läuft, und was das für die Verschlüsselung
/// heißt: etwa "Discord ⇄ Discord · not end-to-end encrypted".
///
/// Entschieden von Nemesus am 21.11.2026: man muss sehen, ob eine Verbindung
/// Matrix mit Discord, Discord mit Discord oder Matrix mit Matrix ist, weil
/// Matrix verschlüsseln kann und Discord nicht.
///
/// Ein Raum ist so offen wie sein offenster Tisch: was an einem
/// verschlüsselten Tisch geschrieben wird, erscheint übersetzt auch an den
/// anderen. Und der KI-Dienst liest in jedem Fall, was er übersetzt; das
/// sagt der Hinweis an der Karte dazu.
String connectionText(Room room) {
  final doors = [for (final seat in room.seats) seat.door];
  final route = doors.map(doorLabel).join(' ⇄ ');
  final open = {
    for (final door in doors)
      if (!doorCanEncrypt(door)) doorLabel(door),
  };
  if (open.isEmpty) {
    return '$route · end-to-end encrypted up to the fish';
  }
  if (open.length == doors.toSet().length) {
    return '$route · not end-to-end encrypted';
  }
  return '$route · not end-to-end encrypted on ${open.join(' and ')}';
}

/// Was die Zeile über die Verbindung verschweigen würde. Steht hinter dem ⓘ
/// an ihrem Ende.
const connectionHint =
    'Whoever runs a door that does not encrypt end to end can read what is written there. '
    'The AI service reads every message it translates, whatever the door.';

/// Wohin der Fisch schickt, was er übersetzen lässt. Das Programm urteilt
/// nach der Adresse, die wirklich benutzt wird; die Maske zeigt es nur.
///
/// Entschieden von Nemesus am 21.11.2026: die Karte sagt, wohin die Sätze
/// gehen.
class AiReach {
  const AiReach({required this.service, required this.place});

  /// Der Dienst, wie auch die Bitte um Zustimmung ihn nennt: der Rechnername
  /// aus der Adresse.
  final String service;

  /// "this_machine", "own_network" oder "outside".
  final String place;

  factory AiReach.fromJson(Map<String, dynamic> json) =>
      AiReach(service: '${json['service'] ?? ''}', place: '${json['place'] ?? ''}');
}

/// Wohin der Fisch sendet, was er übersetzen lässt, als eine Zeile für die
/// Karte. Bei einem Ort, den diese Maske nicht kennt, nichts: lieber keine
/// Aussage als eine geratene.
///
/// Bis zum 23.11.2026 stand hier „AI runs on this PC". Das behauptet mehr,
/// als der Fisch weiß: er kennt die Adresse, an die er sendet, nicht was
/// dahinter läuft. Entschieden von Nemesus an dem Tag: ehrlicher formulieren.
String? aiPlaceText(AiReach ai) => switch (ai.place) {
  'this_machine' => 'Sends to this PC · ${ai.service}',
  'own_network' => 'Sends to your own network · ${ai.service}',
  'outside' => 'Sends to an outside service · ${ai.service}',
  _ => null,
};

/// Was die Zeile nicht beweisen kann. Steht hinter dem ⓘ an ihrem Ende.
const aiPlaceHint =
    'This is where the fish sends what it translates. It goes by the address alone: '
    'what runs behind that address, and whether the model is open source, the fish cannot check.';

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

/// Eine Sprache, die das Programm beim Eintragen vorschlägt
/// (`languages --json`).
class Language {
  const Language({required this.name, this.also = const []});

  /// Das Wort, das ins Profil kommt: der englische Name.
  final String name;

  /// Andere Namen, unter denen jemand sie beim Tippen sucht.
  final List<String> also;

  factory Language.fromJson(Map<String, dynamic> json) {
    final also = json['also'];
    return Language(
      name: '${json['name'] ?? ''}'.trim(),
      also: [
        if (also is List)
          for (final other in also)
            if (other is String && other.trim().isNotEmpty) other.trim(),
      ],
    );
  }
}

/// Ein Vorschlag: die Sprache und, wenn sie über einen anderen Namen gefunden
/// wurde, dieser Name.
typedef LanguageHit = ({String name, String? via});

/// Die Sprachen, die zu dem Getippten passen. Zuerst die, deren Name so
/// beginnt, dann die mit einem anderen Namen, der so beginnt („Deutsch" führt
/// zu German), zuletzt die, in deren Namen ein Wort so beginnt.
///
/// Steht schon genau ein Name der Liste da, gibt es nichts vorzuschlagen.
List<LanguageHit> suggestLanguages(List<Language> all, String typed) {
  final exact = typed.trim();
  final start = exact.toLowerCase();
  if (start.isEmpty || all.any((language) => language.name == exact)) {
    return const [];
  }
  final byName = <LanguageHit>[];
  final byOther = <LanguageHit>[];
  final byWord = <LanguageHit>[];
  for (final language in all) {
    final name = language.name.toLowerCase();
    if (name.startsWith(start)) {
      byName.add((name: language.name, via: null));
      continue;
    }
    final other = language.also.where((also) => also.toLowerCase().startsWith(start));
    if (other.isNotEmpty) {
      byOther.add((name: language.name, via: other.first));
    } else if (name.split(RegExp(r'[\s(]+')).any((word) => word.startsWith(start))) {
      byWord.add((name: language.name, via: null));
    }
  }
  return [...byName, ...byOther, ...byWord];
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

  /// Der Mensch, auf den dieser Name oder diese Kennung zeigt, falls das
  /// Profil ihn nennt.
  Person? person(String? ref) {
    if (ref == null || ref.isEmpty) {
      return null;
    }
    for (final person in people) {
      if (person.answersTo(ref)) {
        return person;
      }
    }
    return null;
  }

  /// Die Türen, an denen dieses Profil Tische hat. Seit dem 23.11.2026
  /// können es mehrere sein, auch in einem Raum.
  Set<String> get doors => {
    for (final room in rooms)
      for (final seat in room.seats) seat.door,
  };

  /// Die erste Tür dieses Profils: die seines ersten Tisches.
  String get door {
    for (final room in rooms) {
      for (final seat in room.seats) {
        return seat.door;
      }
    }
    return discordDoor;
  }

  /// Ist dieser Mensch der Gastgeber? Das Profil nennt den Gastgeber mit
  /// seinem Namen oder mit seiner Kennung.
  bool isHost(Person person) => person.answersTo(host);

  /// Gehört dieser Tisch dem Gastgeber?
  bool ownedByHost(Seat seat) {
    final owner = seat.owner;
    if (owner == null || host.isEmpty) {
      return false;
    }
    if (owner == host) {
      return true;
    }
    final who = person(owner);
    return who != null && isHost(who);
  }

  /// Ein Tisch als kurzer Text für die Karte: für wen er gebaut ist und in
  /// welcher Sprache dort gelesen wird, etwa "Robin · Russian". Ohne Namen
  /// nur die Sprache.
  String tableText(Seat seat) {
    final name = person(seat.owner)?.name;
    final lang = languageAt(seat);
    return name == null ? lang : '$name · $lang';
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
    this.ai,
  });

  /// Wohin die Sätze gehen. Ein älteres Programm sagt es nicht; dann zeigt
  /// die Karte dazu nichts, statt etwas zu vermuten.
  final AiReach? ai;

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
    final ai = json['ai'];
    return ProfileEntry(
      ai: ai is Map ? AiReach.fromJson(_map(ai)) : null,
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

/// Ein gelöschtes Profil im Papierkorb des Programms.
class Trashed {
  const Trashed({
    required this.id,
    required this.name,
    required this.deleted,
    required this.state,
  });

  /// Woran das Programm den Eintrag erkennt: Name und Zeitpunkt.
  final String id;

  /// Wie das Profil hieß.
  final String name;

  /// Wann es gelöscht wurde, etwa „2026-11-20 07:01:02 UTC".
  final String deleted;

  /// Ob auch liegt, was der Fisch sich dazu gemerkt hat.
  final bool state;

  factory Trashed.fromJson(Map<String, dynamic> json) => Trashed(
    id: '${json['id'] ?? ''}',
    name: '${json['name'] ?? ''}',
    deleted: '${json['deleted'] ?? ''}',
    state: json['state'] == true,
  );
}

/// Der Papierkorb, wie das Programm ihn auflistet (`profile trash --json`):
/// das zuletzt Gelöschte zuerst.
class Trash {
  const Trash({required this.dir, required this.deleted});

  final String dir;
  final List<Trashed> deleted;

  factory Trash.fromJson(Map<String, dynamic> json) => Trash(
    dir: '${json['dir'] ?? ''}',
    deleted: _maps(json['deleted']).map(Trashed.fromJson).toList(),
  );
}

/// Der Eintrag der Ablage, unter dem der Token des Bots liegt. Die Schlüssel
/// der KI-Dienste liegen unter der Kennung ihres Dienstes ([Provider.id]).
const String botTokenEntry = 'discord';

/// So heißt der Token des Bots in seinem Fenster.
const String botTokenLabel = 'Discord bot token';

/// So heißt die Anzeige des Tokens in der Titelleiste: nach der Tür, zu der
/// er gehört, wie „Matrix" daneben. Bis zum 23.11.2026 stand dort „Bot
/// token"; seit es zwei Türen gibt, sagte das nicht mehr, wozu er gehört
/// (Nemesus an dem Tag: „Discord: stored ist gut").
const String discordChipLabel = 'Discord';

/// Der Eintrag der Ablage, unter dem der Zugang zum Matrix-Konto des Fischs
/// liegt. Er entsteht beim Anmelden und wird nie eingefügt.
const String matrixEntry = 'matrix';

/// Ob und als wer der Fisch bei Matrix angemeldet ist (`matrix status --json`).
/// Die Kennung des Kontos und die Adresse des Servers sind keine Geheimnisse.
class MatrixStatus {
  const MatrixStatus({
    required this.loggedIn,
    this.user,
    this.homeserver,
    this.device,
    this.deviceNote,
  });

  final bool loggedIn;
  final String? user;
  final String? homeserver;

  /// Was der Server über das Gerät des Fischs zeigt, wenn er gefragt wurde
  /// (`--ask`): [deviceVerified], [deviceUnverified], [deviceRefused] oder
  /// [deviceUnknown]. Ohne Frage, oder von einem älteren Programm: nichts.
  final String? device;

  /// Warum es sich nicht klären ließ, im Wortlaut des Programms.
  final String? deviceNote;

  factory MatrixStatus.fromJson(Map<String, dynamic> json) => MatrixStatus(
    loggedIn: json['logged_in'] == true,
    user: _text(json['user']),
    homeserver: _text(json['homeserver']),
    device: _text(json['device']),
    deviceNote: _text(json['device_note']),
  );
}

/// Das Konto hat das Gerät des Fischs gegengezeichnet.
const String deviceVerified = 'verified';

/// Der Server zeigt das Gerät ohne gültige Unterschrift des Kontos.
const String deviceUnverified = 'unverified';

/// Der Server nimmt den gespeicherten Zugang nicht mehr an.
const String deviceRefused = 'refused';

/// Der Server ließ sich nicht fragen.
const String deviceUnknown = 'unknown';

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

  /// Ob der Fisch an seinem Matrix-Konto angemeldet ist. Ein älteres Programm
  /// nennt den Eintrag nicht: dann gilt er als nicht angemeldet.
  bool get matrix => has(matrixEntry);

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
