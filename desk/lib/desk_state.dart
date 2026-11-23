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

  /// Ob der Fisch an ist: ob dieses Fenster ihn gestartet hat und er läuft.
  ///
  /// Es gibt einen Schalter für alle Profile, keinen je Profil. Nemesus am
  /// 21.11.2026, nachdem er zwei Profile auf zwei Servern laufen hatte und der
  /// Bot auch dort grün aussah, wo gerade nichts übersetzt wurde: „Wir bauen
  /// die Schalter für die einzelnen Profile ab. Stattdessen bauen wir nur
  /// einen Button. An und Aus. Schaltet alle Profile ein oder aus. Damit löst
  /// sich auch gleich das Problem, wenn man 20 Profile hat. Und es ist nicht
  /// mehr missverständlich."
  bool get on => _run != null;

  /// Die Profile, die der Fisch gerade bedient. Das sagt er selbst, in seiner
  /// Ausgabe; die Maske glaubt nur ihm.
  final Set<String> running = {};

  /// Die Profile, von denen der Fisch noch nicht gesagt hat, ob er sie
  /// bedient oder warum nicht.
  final Set<String> _starting = {};

  bool starting(String name) => _starting.contains(name);

  /// Warum der Fisch nicht gestartet werden konnte oder von selbst geendet
  /// hat. Das betrifft alle Profile, deshalb steht es nicht an einer Karte.
  String? fishError;

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

  /// Warum ein einzelnes Profil nicht läuft, während der Fisch an ist.
  final Map<String, String> _runErrors = {};

  /// Die letzte Meldung im Wortlaut des Fischs, ohne Zeitstempel.
  String? _lastLine;

  List<String> logOf(String name) => _logs[name] ?? const [];

  String? logPathOf(String name) => _files[name]?.path;

  String? runErrorOf(String name) => _runErrors[name];

  static const int logLimit = 200;

  FishRun? _run;
  bool _switching = false;
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
      if (!freshKeys.matrix) {
        matrixDevice = null;
        _deviceAsked = false;
      } else if (!_deviceAsked) {
        _deviceAsked = true;
        unawaited(_askDevice());
      }
    } on CliException catch (e) {
      loadError = e.message;
    } finally {
      _refreshing = false;
      _changed();
    }
  }

  /// Was der Server über das Gerät des Fischs bei Matrix zeigt
  /// ([deviceVerified] und die anderen); nichts, solange er nicht gefragt
  /// wurde oder nicht geantwortet hat.
  String? matrixDevice;
  bool _deviceAsked = false;

  /// Fragt den Server einmal je Anmeldung, nicht im Takt der Übersicht: die
  /// Frage geht ins Netz, und die Antwort ändert sich nur, wenn sich am Konto
  /// etwas ändert.
  Future<void> _askDevice() async {
    try {
      final status = await cli.matrixStatus(ask: true);
      matrixDevice = status.device;
    } on CliException {
      matrixDevice = null;
    }
    if (!_disposed) {
      _changed();
    }
  }

  /// Nach dem Fenster des Matrix-Kontos: beim nächsten Erneuern wird der
  /// Server wieder gefragt.
  void askDeviceAgain() => _deviceAsked = false;

  /// Schaltet den Fisch an: er bedient alle Profile, die sich starten lassen.
  /// Eines, das sich nicht starten lässt, sagt an seiner Karte, warum, und
  /// hält die anderen nicht auf.
  Future<void> switchOn() async {
    if (_run != null || _switching) {
      return;
    }
    _switching = true;
    fishError = null;
    _runErrors.clear();
    _logs.clear();
    _files.clear();
    running.clear();
    _lastLine = null;
    // Erwartet wird eine Antwort zu jedem Profil, das das Programm für gültig
    // hält. Bis sie da ist, zeigt die Karte „Starting".
    _starting
      ..clear()
      ..addAll([
        for (final entry in list?.profiles ?? const <ProfileEntry>[])
          if (entry.ok) entry.name,
      ]);
    _changed();
    final FishRun run;
    try {
      run = await cli.runAll();
    } on CliException catch (e) {
      fishError = e.message;
      _starting.clear();
      _changed();
      return;
    } finally {
      _switching = false;
    }
    _run = run;
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
        running.clear();
        _starting.clear();
        _files.clear();
        if (!stoppedByUs) {
          // Der Fisch ist von selbst gegangen. Der Grund steht in seinen
          // letzten Zeilen; die letzte wird hervorgehoben.
          fishError = _lastLine ?? 'The fish ended with code $code and gave no reason.';
        }
        _changed();
        if (!_disposed) {
          await refresh();
        }
      }),
    );
  }

  bool _stopping = false;

  /// Schaltet den Fisch aus, mit allen Profilen.
  void switchOff() {
    final run = _run;
    if (run == null) {
      return;
    }
    _stopping = true;
    run.stop();
  }

  /// Ein Profil wurde gespeichert, während der Fisch an ist: er nimmt es neu
  /// auf, ohne die anderen zu stören. Ein neues kommt dazu, ein geändertes
  /// wird neu geladen.
  void saved(String name) {
    final run = _run;
    if (run == null) {
      return;
    }
    _runErrors.remove(name);
    _starting.add(name);
    run.say(running.contains(name) ? 'restart $name' : 'start $name');
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
      if (said == 'serving') {
        running.add(name);
        _starting.remove(name);
        _runErrors.remove(name);
      } else if (said.startsWith(_cannotStart)) {
        // Der Fisch konnte dieses Profil nicht aufnehmen. Die anderen laufen
        // weiter; der Grund steht an dieser Karte.
        running.remove(name);
        _starting.remove(name);
        _runErrors[name] = said.substring(_cannotStart.length);
      } else if (said.startsWith(_cannotRestart)) {
        // Das geänderte Profil ließ sich nicht laden. Das bisherige läuft
        // weiter: lieber der alte Stand als keiner.
        _starting.remove(name);
        _runErrors[name] =
            'Still running with the settings from before: ${said.substring(_cannotRestart.length)}';
      } else if (said == 'no longer serving') {
        running.remove(name);
      }
    }
    // Eine Zeile über ein Profil gehört zu diesem. Eine über den ganzen Fisch
    // (verbunden, hält an) gehört zu allen, die laufen oder gerade starten.
    final owners = about != null ? [about.$1] : {...running, ..._starting}.toList();
    final stamped = '${logStamp(_now())}  $line';
    for (final name in owners) {
      final log = _logOf(name);
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

  /// Das Protokoll eines Profils in diesem Lauf. Beim ersten Mal wird auch
  /// seine Datei geöffnet. Eine Datei, die sich nicht öffnen lässt, hält den
  /// Fisch nicht auf: das Fenster sagt es, und das Protokoll steht nur dort.
  List<String> _logOf(String name) {
    final known = _logs[name];
    if (known != null) {
      return known;
    }
    final log = _logs[name] = [];
    final open = openLog;
    final dir = list?.dir;
    if (open != null && dir != null) {
      try {
        _files[name] = open(dir, name);
      } on Exception catch (e) {
        log.add('${logStamp(_now())}  desk: cannot open the log file: $e');
      }
    }
    return log;
  }

  static const _cannotStart = 'cannot start: ';
  static const _cannotRestart = 'cannot restart: ';

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
