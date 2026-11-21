// Das Protokoll eines Fischs als Datei, damit es das Fenster überlebt.
//
// Nemesus am 21.11.2026 auf die Frage, ob das Protokoll zusätzlich in eine
// Datei geschrieben werden soll: „Ja, bauen". Bis dahin stand es nur im
// Fenster und war beim nächsten Start fort.
//
// Die Dateien liegen im Ordner `logs` neben dem Ordner der Profile, eine je
// Profil. Dort stehen dieselben Zeilen wie im Fenster: Kennungen und Gründe,
// nie der Text einer Nachricht.

import 'dart:io';

/// Eine geöffnete Protokolldatei: wo sie liegt und wie man eine Zeile anhängt.
class OpenLog {
  const OpenLog(this.path, this.write);

  final String path;
  final void Function(String line) write;
}

/// Öffnet die Protokolldatei eines Profils. Neue Zeilen kommen ans Ende; was
/// frühere Läufe geschrieben haben, bleibt stehen.
///
/// Ist die Datei größer als [maxBytes], wird sie vorher zu `NAME.old.log`, und
/// eine ältere dieses Namens verschwindet. So bleiben höchstens zwei Dateien
/// je Profil, und keine wächst ohne Ende.
OpenLog openLogFile(String profilesDir, String profile, {int maxBytes = 1 << 20}) {
  final sep = Platform.pathSeparator;
  final dir = Directory('${Directory(profilesDir).parent.path}${sep}logs');
  dir.createSync(recursive: true);
  final file = File('${dir.path}$sep$profile.log');
  if (file.existsSync() && file.lengthSync() > maxBytes) {
    final old = File('${dir.path}$sep$profile.old.log');
    if (old.existsSync()) {
      old.deleteSync();
    }
    file.renameSync(old.path);
  }
  return OpenLog(
    file.path,
    // Jede Zeile sofort auf die Platte: nach einem Absturz soll gerade die
    // letzte Zeile noch da sein.
    (line) => file.writeAsStringSync('$line\n', mode: FileMode.append, flush: true),
  );
}
