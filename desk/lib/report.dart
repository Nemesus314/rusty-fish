// Was jemand einer KI gibt, um einen Fehler an einem Profil zu finden: der
// Stand des Profils und sein Protokoll, als Text für die Zwischenablage.
//
// Nemesus am 23.11.2026, nach dem ersten echten Lauf eines gemischten Raums:
// „eine echt coole Hilfe wäre ein Button, wo man den aktuellen Text von
// bridge, probe usw in die Zwischenablage tun kann, um es mit KI zu teilen."
// Und auf die Frage, was er mitgeben soll: „zwei Buttons. Mit und ohne. Damit
// der SysOp die Wahl hat, was er mit KI teilt."
//
// Ohne: der Stand, die Räume nach Tür und Sprache, wohin die KI sendet, und
// das Protokoll. Keine Menschen: weder die Namen, die der Gastgeber ihnen
// gibt, noch ihre Kennungen, noch der Gastgeber. Bis dahin galt, dass diese
// Namen das Profil nie verlassen; das bleibt die Vorgabe.
//
// Mit: dazu das ganze Profil, so wie die Maske es an das Programm schickt.
//
// In keinem Fall dabei: ein Schlüssel, ein Token, der Text einer Nachricht.
// Das Programm schreibt in sein Protokoll nur Kennungen und Gründe.

import 'dart:convert';

import 'model.dart';

/// Die Kennung eines Menschen bei Matrix, wie sie in einer Zeile des
/// Protokolls stehen kann, etwa bei einer Nachricht, die sich nicht
/// entschlüsseln ließ.
/// Ein Satzzeichen hinter der Kennung gehört nicht zu ihr.
final _matrixUser = RegExp(r'@[^\s:@]+:[^\s,;)]*[^\s,;):.]');

/// Der Text für die Zwischenablage.
///
/// - `status`: wie die Karte den Stand nennt, etwa „Running".
/// - `service`: der KI-Dienst, wie die Karte ihn nennt, wenn sie ihn kennt.
/// - `log`: die Zeilen des Protokolls an der Karte.
/// - `runError`: warum das Profil in diesem Lauf nicht läuft, wenn es das
///   nicht tut.
/// - `withPeople`: mit dem ganzen Profil, also auch mit den Menschen.
String reportForAi({
  required ProfileEntry entry,
  required String status,
  required List<String> log,
  required bool withPeople,
  String? service,
  String? runError,
}) {
  final out = StringBuffer()
    ..writeln('Rusty Fish: profile "${entry.name}", copied to find a fault.')
    ..writeln(
      withPeople
          ? 'With the full profile: it holds the names the host gives people and their '
                'user ids.'
          : 'Left out: the people of this profile, their user ids and the host.',
    )
    ..writeln('Never in here: a key, a token, the text of a message.')
    ..writeln()
    ..writeln('Status: $status');
  final doc = entry.doc;
  if (doc == null) {
    out.writeln('The program refuses this profile: ${entry.error ?? 'it gave no reason'}');
  } else {
    out.writeln('Rooms:');
    for (final (index, room) in doc.rooms.indexed) {
      final seats = [
        for (final seat in room.seats)
          '${doorLabel(seat.door)}, ${doc.languageAt(seat)}${seat.served ? '' : ' (read only)'}',
      ];
      final neutral = room.passThrough.isEmpty ? 'none' : room.passThrough.join(', ');
      out.writeln('- Room ${index + 1}: ${seats.join(' ⇄ ')} · neutral: $neutral');
      out.writeln('  ${connectionText(room)}');
    }
    out.writeln('AI: ${service ?? doc.provider} · ${doc.model}');
    if (entry.ai case final ai?) {
      if (aiPlaceText(ai) case final where?) {
        out.writeln(where);
      }
    }
    out.writeln(
      doc.aiNames.isEmpty ? 'Translator only.' : 'The AI answers to: ${doc.aiNames.join(', ')}',
    );
  }
  if (runError != null) {
    out.writeln('Why it is not running: $runError');
  }
  out
    ..writeln()
    ..writeln(
      log.isEmpty
          ? 'Log: empty. The fish has said nothing about this profile since the window opened.'
          : 'Log (${log.length} ${log.length == 1 ? 'line' : 'lines'}; the fish writes ids and '
                'reasons, never the text of a message):',
    );
  for (final line in log) {
    // Ohne Menschen heißt auch: ohne ihre Kennungen im Protokoll.
    out.writeln(withPeople ? line : line.replaceAll(_matrixUser, '@…'));
  }
  if (withPeople && doc != null) {
    out
      ..writeln()
      ..writeln('The profile, as the window sends it to the program:')
      ..writeln(const JsonEncoder.withIndent('  ').convert(doc.toJson()));
  }
  return out.toString();
}
