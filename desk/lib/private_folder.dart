// Ordner, die die Maske selbst anlegt, nur für das eigene Konto.
//
// Der Fisch legt sein Heim und die Ordner darin unter Unix mit 0700 an
// (`create_private` in `rusty-fish/src/profiles.rs`): in Profilen, Zustand und
// Protokollen stehen Namen und Kennungen von Menschen. Die Maske legt zwei
// Dinge selbst an, den Ordner `logs` und, für `desk.json`, notfalls das Heim.
//
// Gemessen am 25.11.2026 im Linux Mint 22.3 von Nemesus' virtueller Maschine:
// `logs` entstand mit 0775 und die Protokolldatei mit 0664, neben `profiles`,
// `secrets` und `matrix` mit 0700. Nemesus an dem Tag auf die Frage, ob die
// Maske es wie der Fisch halten soll: „Ja, angleichen".

import 'dart:io';

/// Legt einen Ordner an, samt allem darüber, was noch fehlt. Unter Linux und
/// macOS nur für das eigene Konto (0700), wie der Fisch seine Ordner anlegt.
///
/// Benannte Grenzen:
/// - Ein Ordner, den es schon gibt, bleibt, wie er ist; wie beim Fisch.
/// - Dart kann Rechte nicht selbst setzen. Der Ordner entsteht mit den
///   Rechten des Systems und wird gleich danach mit `chmod` geschlossen; für
///   diesen Augenblick ist er leer. Fehlt `chmod`, bleibt er, wie er entstand.
/// - Unter Windows gilt, was der Ordner des Nutzers ohnehin vorgibt.
void createPrivate(Directory dir) {
  if (Platform.isWindows) {
    dir.createSync(recursive: true);
    return;
  }
  // Welche Ordner neu sind, lässt sich nur vor dem Anlegen sagen.
  final fresh = <String>[];
  var at = dir.absolute;
  while (!at.existsSync()) {
    fresh.add(at.path);
    final above = at.parent;
    if (above.path == at.path) {
      break;
    }
    at = above;
  }
  dir.createSync(recursive: true);
  if (fresh.isEmpty) {
    return;
  }
  try {
    Process.runSync('chmod', ['700', ...fresh]);
  } on Object {
    // Die Maske läuft weiter; der Ordner bleibt, wie das System ihn anlegte.
  }
}
