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

import 'private_folder.dart';

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
/// derselben Regel wie das Programm (`home` in `rusty-fish/src/profiles.rs`):
/// `RUSTY_FISH_HOME`, sonst unter Windows `%APPDATA%\rusty-fish`, unter macOS
/// `~/Library/Application Support/rusty-fish`, sonst
/// `$XDG_CONFIG_HOME/rusty-fish` oder `~/.config/rusty-fish`.
///
/// Bis zum 25.11.2026 kannte die Maske nur Windows. Nemesus an dem Tag auf
/// die Frage, ob sie sich Hell und Dunkel auch unter Linux merken soll: „Ja,
/// bauen". Gemessen war bis dahin im Linux Mint 22.3 seiner virtuellen
/// Maschine: die Maske baut und startet dort unverändert, alle 176 Tests
/// bestehen, nur diese Wahl galt bloß, solange das Fenster offen war.
///
/// Benannte Grenzen: lässt sich der Ort nicht bestimmen (weder `HOME` noch
/// eine der anderen Angaben), gilt die Wahl nur, solange das Fenster offen
/// ist. Die Regel für macOS ist die des Programms; gebaut und gemessen wurde
/// die Maske dort nie.
String? deskSettingsPath({
  required Map<String, String> environment,
  required bool windows,
  bool macOS = false,
}) {
  final separator = windows ? r'\' : '/';
  // Leer gesetzt zählt als nicht gesetzt, wie im Programm.
  String set(String key) => environment[key]?.trim() ?? '';
  final home = set('RUSTY_FISH_HOME');
  if (home.isNotEmpty) {
    return '$home${separator}desk.json';
  }
  if (windows) {
    final appData = set('APPDATA');
    return appData.isEmpty ? null : '$appData${separator}rusty-fish${separator}desk.json';
  }
  final user = set('HOME');
  if (macOS) {
    return user.isEmpty ? null : '$user/Library/Application Support/rusty-fish/desk.json';
  }
  final config = set('XDG_CONFIG_HOME');
  if (config.isNotEmpty) {
    return '$config/rusty-fish/desk.json';
  }
  return user.isEmpty ? null : '$user/.config/rusty-fish/desk.json';
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
    createPrivate(file.parent);
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
