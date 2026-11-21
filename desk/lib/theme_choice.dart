// Hell oder dunkel.
//
// Vorgemerkt von Nemesus am 21.11.2026: ein Umschalter zwischen Dark und
// Light. Ohne Wahl folgt die Maske dem, was in Windows eingestellt ist. Wer
// umschaltet, findet seine Wahl beim nächsten Öffnen wieder vor.
//
// Die Wahl gehört der Maske, nicht dem Fisch: sie steht in einer eigenen
// kleinen Datei `desk.json` im Ordner von rusty-fish, nicht in einem Profil.

import 'dart:convert';
import 'dart:io';

import 'package:material_ui/material_ui.dart';

/// Die Wahl zwischen hell und dunkel, auf die die ganze Maske hört.
class ThemeChoice extends ValueNotifier<ThemeMode> {
  ThemeChoice({ThemeMode initial = ThemeMode.system, this.remember}) : super(initial);

  /// Merkt sich die Wahl über das Schließen des Fensters hinaus. In Tests
  /// ohne Angabe: dann gilt sie nur für diesen Lauf.
  final void Function(ThemeMode mode)? remember;

  /// Schaltet um, ausgehend von dem, was gerade zu sehen ist. So macht der
  /// erste Klick auch dann etwas Sichtbares, wenn die Maske noch Windows folgt.
  void toggle(Brightness shown) {
    value = shown == Brightness.dark ? ThemeMode.light : ThemeMode.dark;
    remember?.call(value);
  }
}

/// Wo die Maske ihre eigene Einstellung ablegt: im Ordner von rusty-fish, nach
/// derselben Regel wie das Programm (`RUSTY_FISH_HOME`, sonst unter Windows
/// `%APPDATA%\rusty-fish`).
///
/// Benannte Grenze: auf anderen Systemen gibt es ohne `RUSTY_FISH_HOME`
/// keinen Ort. Dann gilt die Wahl nur, solange das Fenster offen ist.
String? deskSettingsPath({required Map<String, String> environment, required bool windows}) {
  final separator = windows ? r'\' : '/';
  final home = environment['RUSTY_FISH_HOME']?.trim() ?? '';
  if (home.isNotEmpty) {
    return '$home${separator}desk.json';
  }
  final appData = environment['APPDATA']?.trim() ?? '';
  if (windows && appData.isNotEmpty) {
    return '$appData${separator}rusty-fish${separator}desk.json';
  }
  return null;
}

/// Liest die gemerkte Wahl. Fehlt die Datei oder ist sie nicht zu lesen,
/// folgt die Maske Windows: wegen einer Farbe scheitert kein Start.
ThemeMode readThemeMode(String path) {
  try {
    final json = jsonDecode(File(path).readAsStringSync());
    return themeModeFromText(json is Map ? json['theme'] : null);
  } on Object {
    return ThemeMode.system;
  }
}

/// Merkt sich die Wahl. Gelingt es nicht, bleibt sie für diesen Lauf gültig.
void writeThemeMode(String path, ThemeMode mode) {
  try {
    final file = File(path);
    file.parent.createSync(recursive: true);
    file.writeAsStringSync('${jsonEncode({'theme': themeModeToText(mode)})}\n');
  } on Object {
    // Die Maske läuft weiter; nur das Merken entfällt.
  }
}

ThemeMode themeModeFromText(Object? text) => switch (text) {
  'light' => ThemeMode.light,
  'dark' => ThemeMode.dark,
  _ => ThemeMode.system,
};

String themeModeToText(ThemeMode mode) => switch (mode) {
  ThemeMode.light => 'light',
  ThemeMode.dark => 'dark',
  ThemeMode.system => 'system',
};
