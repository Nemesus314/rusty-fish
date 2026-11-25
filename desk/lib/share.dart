// Räume weitergeben: als Text, den man jemandem schickt, der seinen eigenen
// Fisch mitbringt.
//
// Gemessen am 22.11.2026 im ersten Lauf mit zwei Fischen in einem Raum: beide
// müssen denselben Raum eintragen, zwei Kanal-Kennungen, zwei
// Benutzer-Kennungen, wer welchen Tisch hat. Seit dem 22.11.2026 gilt
// dasselbe für Räume bei Matrix; die Tür jedes Tisches steht im Text. Ein Zahlendreher, und nichts
// passt zusammen, ohne dass irgendetwas es sagt. Nemesus dazu: „Ja, bauen."
//
// Weitergegeben wird, was beide gleich eintragen müssen: die Tische mit ihren
// Kanälen und Sprachen, wem sie gehören, und was über diese Menschen
// eingetragen ist. Nicht weitergegeben wird, was jedem Fisch selbst gehört:
// wer der Gastgeber ist, wer eingeladen ist, der KI-Dienst, das Modell, die
// Grenzen, die Rufnamen. Schlüssel und Token liegen ohnehin nicht im Profil.
//
// Der Text ist lesbar, damit man sieht, was man aus der Hand gibt. Die Form
// der Räume und Menschen darin ist dieselbe, in der das Programm ein Profil
// ausgibt.
//
// Benannte Grenzen:
// - Die Namen, die der Gastgeber Menschen gegeben hat, gehen mit. Wer einen
//   Raum weitergibt, gibt auch diese Etiketten weiter.
// - Ob der Text stimmt, entscheidet erst das Programm beim Speichern, wie bei
//   jedem Profil. Hier wird nur gelesen, was dasteht.
// - Geht der Text durch ein Programm, das Anführungszeichen verschönert, ist
//   er danach nicht mehr lesbar.

import 'dart:convert';

import 'model.dart';

const _marker = 'rusty_fish_rooms';

/// Wie viele frühere Nachrichten die KI sieht, wenn nichts anderes gewählt
/// ist. Das kostet bei jeder Übersetzung mit und gehört deshalb jedem Fisch
/// selbst; es wird nicht weitergegeben.
const _defaultContextLines = 10;

/// Was jemand weitergegeben hat: die Räume und die Menschen an ihren Tischen.
class SharedRooms {
  const SharedRooms({required this.rooms, required this.people});

  final List<Room> rooms;
  final List<Person> people;
}

/// Der Text, den man jemandem gibt, der seinen eigenen Fisch mitbringt.
String shareRoomsText(ProfileDoc doc) {
  final json = {
    _marker: 1,
    'rooms': [
      for (final room in doc.rooms)
        {
          'pass_through': room.passThrough,
          // Ob ein Tisch bedient wird, entscheidet jeder Fisch für sich.
          'seats': [for (final seat in room.seats) seat.toJson()],
        },
    ],
    'people': [
      for (final person in doc.people)
        // Wen der Gastgeber eingeladen hat, ist seine Sache und geht nicht mit.
        Person(
          id: person.id,
          matrix: person.matrix,
          name: person.name,
          gender: person.gender,
          // Wie jemand angesprochen wird, gehört zum Raum wie seine Form.
          formality: person.formality,
          lang: person.lang,
          neutral: person.neutral,
        ).toJson(),
    ],
  };
  return const JsonEncoder.withIndent('  ').convert(json);
}

/// Liest einen weitergegebenen Text. `null`, wenn es keiner ist.
///
/// Aus einem Chat kopiert steht manchmal etwas davor oder dahinter, etwa eine
/// Anrede oder die drei Zeichen eines Code-Blocks. Gelesen wird deshalb von
/// der ersten bis zur letzten geschweiften Klammer.
SharedRooms? readSharedRooms(String text) {
  final start = text.indexOf('{');
  final end = text.lastIndexOf('}');
  if (start < 0 || end <= start) {
    return null;
  }
  final Object? json;
  try {
    json = jsonDecode(text.substring(start, end + 1));
  } on FormatException {
    return null;
  }
  if (json is! Map || json[_marker] != 1) {
    return null;
  }
  final rawRooms = json['rooms'];
  if (rawRooms is! List || rawRooms.isEmpty) {
    return null;
  }
  final rooms = <Room>[];
  for (final raw in rawRooms) {
    if (raw is! Map) {
      return null;
    }
    final room = Room.fromJson(raw.map((key, value) => MapEntry('$key', value)));
    // Weniger als zwei Tische ergeben kein Gespräch.
    if (room.seats.length < 2) {
      return null;
    }
    rooms.add(
      Room(
        passThrough: room.passThrough,
        contextLines: _defaultContextLines,
        seats: [
          for (final seat in room.seats)
            Seat(
              door: seat.door,
              channel: seat.channel,
              lang: seat.lang,
              gender: seat.gender,
              owner: seat.owner,
            ),
        ],
      ),
    );
  }
  final rawPeople = json['people'];
  final people = <Person>[
    if (rawPeople is List)
      for (final raw in rawPeople)
        if (raw is Map)
          _uninvited(Person.fromJson(raw.map((key, value) => MapEntry('$key', value)))),
  ];
  return SharedRooms(rooms: rooms, people: people);
}

/// Wer der Einfügende sein kann: womit das neue Profil auf ihn als Gastgeber
/// zeigt. Das sind die genannten Menschen mit einer Kennung, und dazu jede
/// Kennung, der ein Tisch gehört, ohne dass ein Mensch zu ihr eingetragen ist.
///
/// Das Zweite ist der Fall eines Gastgebers, der nur mit seiner Kennung im
/// Profil steht und dem die Tische selbst gehören. Gemessen am 25.11.2026 in
/// Linux Mint mit einem Profil an der Matrix-Tür: die Auswahl bot niemanden
/// an, obwohl beide Tische im Text dieselbe Kennung trugen. Nemesus tippte
/// seine Kennung von Hand, und sie passte zu keinem seiner zwei Konten. Das
/// Profil lief trotzdem: der Fisch behandelte beide Konten als Gäste und bat
/// beide um Zustimmung (20:41:12 und 20:41:59), und nichts sagte, dass der
/// Gastgeber nicht erkannt war. Nemesus dazu: „Ja, beheben."
///
/// Ob eine solche Kennung stimmt, entscheidet wie bei allem hier erst das
/// Programm beim Speichern.
List<String> hostCandidates(SharedRooms shared) {
  final refs = <String>[
    for (final person in shared.people)
      if (person.id.isNotEmpty || person.matrix.isNotEmpty) person.ref,
  ];
  for (final room in shared.rooms) {
    for (final seat in room.seats) {
      final owner = seat.owner;
      if (owner == null || owner.isEmpty || refs.contains(owner)) {
        continue;
      }
      // Gehört der Tisch einem genannten Menschen, steht der schon da, oder
      // er hat noch keine Kennung und kann darum kein Gastgeber sein.
      if (shared.people.any((person) => person.answersTo(owner))) {
        continue;
      }
      refs.add(owner);
    }
  }
  return refs;
}

/// Derselbe Mensch, aber nicht eingeladen: wen ein Fisch einlädt, entscheidet
/// sein Gastgeber, nicht der Text eines anderen.
Person _uninvited(Person person) => Person(
  id: person.id,
  matrix: person.matrix,
  name: person.name,
  gender: person.gender,
  formality: person.formality,
  lang: person.lang,
  neutral: person.neutral,
);
