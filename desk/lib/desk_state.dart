// Was die Maske weiß: die Liste der Profile, ob die Geheimnisse liegen, und
// den einen Fisch, den dieses Fenster gestartet hat, samt den Profilen, die
// er gerade bedient.

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'cli.dart';
import 'log_file.dart';
import 'model.dart';

class DeskState extends ChangeNotifier {
  DeskState(this.cli, {DateTime Function()? now, this.openLog})
    : _now = now ?? DateTime.now;

  /// Öffnet die Protokolldatei eines Profils im Ordner neben den Profilen.
  /// Ohne Angabe (in Tests) steht das Protokoll nur im Fenster.
  final OpenLog Function(String profilesDir, String profile)? openLog;

  /// Die Uhr für die Zeitstempel im Protokoll. In Tests fest.
  final DateTime Function() _now;

  final FishCli cli;

  ProfileList? list;
  KeyStatus? keys;

  /// Die KI-Dienste, die das Programm nennt. Solange es das nicht kann, steht
  /// hier der eine Dienst, den es vor der Auswahl gab.
  List<Provider> providers = fallbackProviders;

  /// Ob [providers] wirklich vom Programm stammt. Nur dann fragt die Maske
  /// nach den Modellen eines bestimmten Dienstes; ein älteres Programm kennt
  /// diese Angabe nicht.
  bool providersListed = false;

  /// Warum das Programm seine Dienste nicht nennen konnte, in seinem Wortlaut.
  /// Die Maske bleibt dann benutzbar, nur ohne Auswahl.
  String? providersError;

  /// Warum die Übersicht nicht geladen werden konnte, im Wortlaut des Programms.
  String? loadError;

  /// Der Dienst zu einer Kennung. Eine Kennung, die das Programm nicht (mehr)
  /// nennt, bleibt sichtbar, statt still zu etwas anderem zu werden.
  Provider providerOf(String id) {
    for (final provider in providers) {
      if (provider.id == id) {
        return provider;
      }
    }
    return Provider(id: id, label: id, baseUrl: null, needsKey: true);
  }

  /// Die Profile, die dieses Fenster gestartet hat und die gerade laufen.
  ///
  /// Bis zum 21.11.2026 höchstens eines. Nemesus an dem Tag: „Wieso kann ich
  /// eigentlich nicht alle Profile gleichzeitig laufen lassen, sondern muss
  /// mich für einen entscheiden?" Seitdem bedient ein Fisch mehrere Profile
  /// über eine Verbindung: das erste startet ihn, jedes weitere wird ihm
  /// gesagt, ohne die laufenden zu stören.
  final Set<String> running = {};

  /// Ob dieses Fenster gerade einen Fisch laufen hat.
  bool get hasFish => _run != null;

  /// Die letzten Meldungen des Fischs je Profil, jede mit der Uhrzeit, zu der
  /// sie hier ankam. Eine Zeile, die ein Profil nennt (`[tea-time]`), steht
  /// bei diesem; eine, die den ganzen Fisch betrifft, bei allen laufenden.
  ///
  /// Nemesus am 21.11.2026: „Die LogFiles haben bis jetzt noch keinen
  /// Zeitstempel." Gestempelt wird hier und nicht im Fisch: die Maske kennt
  /// die Ortszeit des Rechners, der Fisch ohne weitere Bibliothek nur Weltzeit.
  /// Benannte Grenze: ein Fisch, der ohne Maske läuft, stempelt nichts.
  final Map<String, List<String>> _logs = {};

  /// Wohin das Protokoll eines Profils zusätzlich geschrieben wird.
  final Map<String, OpenLog> _files = {};

  /// Warum ein Start gescheitert ist oder der Fisch von selbst geendet hat,
  /// je Profil.
  final Map<String, String> _runErrors = {};

  /// Die letzte Meldung im Wortlaut des Fischs, ohne Zeitstempel.
  String? _lastLine;

  List<String> logOf(String name) => _logs[name] ?? const [];

  String? logPathOf(String name) => _files[name]?.path;

  String? runErrorOf(String name) => _runErrors[name];

  static const int logLimit = 200;

  FishRun? _run;
  Future<void>? _spawning;
  StreamSubscription<String>? _lines;
  bool _disposed = false;
  bool _refreshing = false;

  Future<void> refresh() async {
    // Die Übersicht erneuert sich im Takt. Dauert ein Aufruf länger als der
    // Takt, sollen sich die Aufrufe nicht stapeln.
    if (_refreshing) {
      return;
    }
    _refreshing = true;
    // Die Dienste ändern sich nicht, während das Fenster offen ist. Gefragt
    // wird deshalb nur, bis es einmal geklappt hat. Scheitert es (ein älteres
    // Programm), bleibt der Rest der Maske davon unberührt.
    if (!providersListed) {
      try {
        providers = await cli.providers();
        providersListed = true;
        providersError = null;
      } on CliException catch (e) {
        providersError = e.message;
      }
    }
    try {
      final fresh = await cli.listProfiles();
      final freshKeys = await cli.keyStatus();
      list = fresh;
      keys = freshKeys;
      loadError = null;
    } on CliException catch (e) {
      loadError = e.message;
    } finally {
      _refreshing = false;
      _changed();
    }
  }

  /// Startet ein Profil. Läuft noch kein Fisch, startet es ihn; läuft schon
  /// einer, bekommt er das Profil dazu.
  Future<void> start(String name) async {
    if (running.contains(name)) {
      return;
    }
    // Zwei Klicks kurz nacheinander auf zwei Karten: der zweite wartet, bis
    // der Fisch des ersten da ist, statt einen zweiten Fisch zu starten.
    final pending = _spawning;
    if (pending != null) {
      await pending;
      if (running.contains(name)) {
        return;
      }
    }
    _runErrors.remove(name);
    _logs[name] = [];
    _openLogFile(name);

    final already = _run;
    if (already != null) {
      // Ob es geklappt hat, sagt der Fisch in seiner Ausgabe (siehe
      // [_addLine]). Bis dahin gilt das Profil als laufend.
      running.add(name);
      already.say('start $name');
      _changed();
      return;
    }

    final spawned = Completer<void>();
    _spawning = spawned.future;
    final FishRun run;
    try {
      run = await cli.run(name);
    } on CliException catch (e) {
      _runErrors[name] = e.message;
      _changed();
      return;
    } finally {
      _spawning = null;
      spawned.complete();
    }
    _lastLine = null;
    _run = run;
    running.add(name);
    _lines = run.lines.listen(_addLine);
    _changed();

    unawaited(
      run.exitCode.then((code) async {
        if (!identical(_run, run)) {
          return;
        }
        final stoppedByUs = _stopping;
        _stopping = false;
        // Bewusst nicht abgewartet: es gibt danach nichts, was darauf
        // angewiesen wäre, und in den Tests der Maske käme das Warten nie
        // zurück (die Antwort liefe außerhalb der gestellten Uhr).
        unawaited(_lines?.cancel());
        _lines = null;
        _run = null;
        final ended = running.toList();
        running.clear();
        _files.clear();
        if (!stoppedByUs) {
          // Der Fisch ist von selbst gegangen. Der Grund steht in seinen
          // letzten Zeilen; die letzte wird hervorgehoben, an jedem Profil,
          // das dabei mit zu Ende ging.
          final why = _lastLine ?? 'The fish ended with code $code and gave no reason.';
          for (final name in ended) {
            _runErrors[name] = why;
          }
        }
        _changed();
        if (!_disposed) {
          await refresh();
        }
      }),
    );
  }

  /// Öffnet die Protokolldatei eines Profils. Eine Datei, die sich nicht
  /// öffnen lässt, hält den Fisch nicht auf. Das Fenster sagt es, und das
  /// Protokoll steht dann nur dort.
  void _openLogFile(String name) {
    _files.remove(name);
    final open = openLog;
    final dir = list?.dir;
    if (open == null || dir == null) {
      return;
    }
    try {
      _files[name] = open(dir, name);
    } on Exception catch (e) {
      _logs[name]!.add('${logStamp(_now())}  desk: cannot open the log file: $e');
    }
  }

  bool _stopping = false;

  /// Hält ein Profil an. Ist es das letzte, das läuft, hält der Fisch an; die
  /// anderen laufen sonst ungestört weiter.
  void stop(String name) {
    final run = _run;
    if (run == null || !running.contains(name)) {
      return;
    }
    if (running.length == 1) {
      _stopping = true;
      run.stop();
      return;
    }
    running.remove(name);
    _files.remove(name);
    run.say('stop $name');
    _changed();
  }

  /// Beim Schließen des Fensters: den Fisch sofort beenden, mit allen
  /// Profilen.
  ///
  /// Benannte Grenze: der Fisch läuft nur, solange dieses Fenster offen ist.
  /// Wird das Fenster hart beendet (Task-Manager, Absturz), bekommt die
  /// Oberfläche keine Gelegenheit dazu, und der Fisch läuft weiter, bis ihn
  /// jemand von Hand beendet.
  void stopNow() {
    _stopping = true;
    _run?.stop();
  }

  /// Hält den Fisch an und wartet, bis er wirklich zu Ende ist, höchstens
  /// sechs Sekunden. Gebraucht beim Schließen des Fensters: ohne das Warten
  /// wäre die Maske fort, bevor der Fisch sich bei Discord abmelden konnte.
  Future<void> stopAndWait() async {
    final run = _run;
    if (run == null) {
      return;
    }
    _stopping = true;
    run.stop();
    await run.exitCode.timeout(const Duration(seconds: 6), onTimeout: () => -1);
  }

  void _addLine(String line) {
    _lastLine = line;
    final about = profileOfLine(line);
    if (about != null) {
      final (name, said) = about;
      if (said.startsWith(_cannotStart)) {
        // Der laufende Fisch konnte dieses Profil nicht dazunehmen. Die
        // anderen laufen weiter; der Grund steht an dieser Karte.
        running.remove(name);
        _runErrors[name] = said.substring(_cannotStart.length);
      } else if (said == 'no longer serving') {
        running.remove(name);
      }
    }
    // Eine Zeile über ein Profil gehört zu diesem. Eine über den ganzen Fisch
    // (verbunden, hält an) gehört zu allen, die gerade laufen.
    final owners = about != null ? [about.$1] : running.toList();
    final stamped = '${logStamp(_now())}  $line';
    for (final name in owners) {
      final log = _logs.putIfAbsent(name, () => []);
      log.add(stamped);
      final file = _files[name];
      if (file != null) {
        try {
          file.write(stamped);
        } on Exception catch (e) {
          // Einmal sagen, dann nicht bei jeder Zeile wieder scheitern.
          _files.remove(name);
          log.add('${logStamp(_now())}  desk: cannot write the log file: $e');
        }
      }
      if (log.length > logLimit) {
        log.removeRange(0, log.length - logLimit);
      }
    }
    _changed();
  }

  static const _cannotStart = 'cannot start: ';

  void _changed() {
    if (!_disposed) {
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    stopNow();
    _lines?.cancel();
    super.dispose();
  }
}

final _tagged = RegExp(r'^rusty-fish: \[([a-z0-9_-]+)\] (.*)$');

/// Von welchem Profil eine Zeile des Fischs handelt, und was sie sagt:
/// aus `rusty-fish: [probe] consent stored` wird (`probe`, `consent stored`).
/// Eine Zeile über den ganzen Fisch nennt kein Profil.
(String, String)? profileOfLine(String line) {
  final match = _tagged.firstMatch(line);
  if (match == null) {
    return null;
  }
  return (match.group(1)!, match.group(2)!);
}

/// Der Zeitstempel einer Protokollzeile: „2026-11-21 03:02:52", in der Zeit,
/// die [at] trägt. Mit Datum, weil ein Fisch über Mitternacht laufen kann.
String logStamp(DateTime at) {
  String two(int n) => n.toString().padLeft(2, '0');
  return '${at.year.toString().padLeft(4, '0')}-${two(at.month)}-${two(at.day)} '
      '${two(at.hour)}:${two(at.minute)}:${two(at.second)}';
}
