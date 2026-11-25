// Rusty Fish, die Maske.
//
// Nemesus am 20.11.2026: „Du öffnest das Win-Programm und bekommst eine Maske
// mit allen Profilen. Jedes kannst du editieren und danach speichern.
// Gespeicherte Profile kannst du starten. Damit haben Anwender auch gleich die
// Übersicht, in welchen Chat der Kram geht und für wen er gedacht ist."
//
// Die Maske ist nur die Vorderseite. Ein Profil bleibt eine Datei des
// Programms `rusty-fish`, und dasselbe Profil läuft auf einem Server ohne
// Bildschirm ohne diese Maske.

import 'dart:io';

import 'package:material_ui/material_ui.dart';

import 'cli.dart';
import 'log_file.dart';
import 'overview.dart';
import 'start.dart';
import 'theme_choice.dart';

export 'start.dart';
export 'theme_choice.dart';

void main(List<String> args) {
  WidgetsFlutterBinding.ensureInitialized();
  final location = locateFish(
    environment: Platform.environment,
    ownExecutable: Platform.resolvedExecutable,
    exists: (path) => File(path).existsSync(),
    windows: Platform.isWindows,
  );
  final exe = location.found;
  // Hell oder dunkel: was beim letzten Mal gewählt wurde, sonst wie Windows.
  final settings = deskSettingsPath(
    environment: Platform.environment,
    windows: Platform.isWindows,
    macOS: Platform.isMacOS,
  );
  runApp(
    DeskApp(
      cli: exe == null ? null : ProcessFishCli(exe),
      location: location,
      openLog: openLogFile,
      startWith: StartRequest.fromArgs(args),
      // Nur unter Windows bittet der Fisch das System, wach zu bleiben
      // (`rusty-fish/src/awake.rs`).
      keepsAwake: Platform.isWindows,
      theme: ThemeChoice(
        initial: settings == null ? ThemeMode.system : readThemeMode(settings),
        remember: settings == null ? null : (mode) => writeThemeMode(settings, mode),
      ),
    ),
  );
}

class DeskApp extends StatefulWidget {
  const DeskApp({
    super.key,
    required this.cli,
    this.location,
    this.refreshEvery = const Duration(seconds: 4),
    this.today,
    this.now,
    this.openLog,
    this.startWith,
    this.theme,
    this.keepsAwake = true,
  });

  /// Die Wahl zwischen hell und dunkel. Ohne Angabe folgt die Maske dem
  /// System und merkt sich einen Wechsel nur für diesen Lauf.
  final ThemeChoice? theme;

  /// Ob der Fisch auf diesem System den Rechner wach hält, solange er an
  /// ist. Danach richtet sich, was der Hinweis in der Übersicht sagt.
  final bool keepsAwake;

  /// Die Uhr für die Zeitstempel im Protokoll. In Tests fest.
  final DateTime Function()? now;

  /// Öffnet die Protokolldatei eines Profils. In Tests keine.
  final OpenLog Function(String profilesDir, String profile)? openLog;

  /// Womit das Fenster aufgehen soll. Ohne Angabe mit der Übersicht.
  final StartRequest? startWith;

  /// Der Zugang zum Programm. Fehlt er, wurde das Programm nicht gefunden.
  final FishCli? cli;

  /// Wo das Programm gesucht wurde.
  final FishLocation? location;

  /// Der Takt, in dem sich die Übersicht erneuert. In Tests aus.
  final Duration? refreshEvery;

  /// Das heutige Datum, wie der Fisch es zählt. In Tests fest.
  final String Function()? today;

  @override
  State<DeskApp> createState() => _DeskAppState();
}

class _DeskAppState extends State<DeskApp> {
  late final ThemeChoice _theme = widget.theme ?? ThemeChoice();

  /// Dieselbe Grundfarbe für beide Fassungen, damit hell und dunkel als
  /// dieselbe Maske zu erkennen sind.
  static const _seed = Color(0xFF1F6F8B);

  @override
  Widget build(BuildContext context) {
    final cli = widget.cli;
    return ValueListenableBuilder<ThemeMode>(
      valueListenable: _theme,
      builder: (context, mode, _) => MaterialApp(
        title: 'Rusty Fish',
        debugShowCheckedModeBanner: false,
        themeMode: mode,
        theme: ThemeData(colorScheme: ColorScheme.fromSeed(seedColor: _seed)),
        darkTheme: ThemeData(
          colorScheme: ColorScheme.fromSeed(seedColor: _seed, brightness: Brightness.dark),
        ),
        home: cli == null
            ? MissingFishPage(checked: widget.location?.checked ?? const [])
            : OverviewPage(
                cli: cli,
                refreshEvery: widget.refreshEvery,
                today: widget.today ?? utcToday,
                now: widget.now,
                openLog: widget.openLog,
                startWith: widget.startWith,
                theme: _theme,
                keepsAwake: widget.keepsAwake,
              ),
      ),
    );
  }
}

/// Der Fisch zählt seine Tage in Weltzeit. Die Maske muss genauso zählen,
/// sonst zeigt sie um Mitternacht den Verbrauch von gestern als heutigen.
String utcToday() {
  final now = DateTime.now().toUtc();
  String two(int n) => n.toString().padLeft(2, '0');
  return '${now.year}-${two(now.month)}-${two(now.day)}';
}

/// Das Programm `rusty-fish` fehlt. Die Maske stürzt dann nicht ab, sondern
/// sagt, wo sie gesucht hat.
class MissingFishPage extends StatelessWidget {
  const MissingFishPage({super.key, required this.checked});

  final List<String> checked;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(title: const Text('Rusty Fish')),
      body: Padding(
        padding: const EdgeInsets.all(24),
        child: SelectionArea(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'The program rusty-fish was not found.',
                style: theme.textTheme.titleLarge,
              ),
              const SizedBox(height: 12),
              const Text(
                'This window only shows and edits profiles. The work is done '
                'by the program, so it has to be reachable. These places were '
                'checked, in this order:',
              ),
              const SizedBox(height: 12),
              for (final path in checked)
                Padding(
                  padding: const EdgeInsets.only(bottom: 4),
                  child: Text(
                    path,
                    style: const TextStyle(fontFamily: 'Consolas'),
                  ),
                ),
              const SizedBox(height: 12),
              const Text(
                'Put the program next to this one, or set the environment '
                'variable RUSTY_FISH_EXE to its full path, then start this '
                'window again.',
              ),
            ],
          ),
        ),
      ),
    );
  }
}
