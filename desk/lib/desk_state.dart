// Was die Maske weiß: die Liste der Profile, ob die Geheimnisse liegen, und
// den einen Fisch, den dieses Fenster gestartet hat.

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'cli.dart';
import 'model.dart';

class DeskState extends ChangeNotifier {
  DeskState(this.cli, {DateTime Function()? now}) : _now = now ?? DateTime.now;

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

  /// Das Profil, das dieses Fenster gestartet hat. Höchstens eines: ein Bot
  /// kann nur einmal laufen, sonst käme jede Nachricht doppelt an.
  String? runningName;

  /// Die letzten Meldungen des laufenden oder zuletzt gelaufenen Fischs, jede
  /// mit der Uhrzeit, zu der sie hier ankam.
  ///
  /// Nemesus am 21.11.2026: „Die LogFiles haben bis jetzt noch keinen
  /// Zeitstempel." Gestempelt wird hier und nicht im Fisch: die Maske kennt
  /// die Ortszeit des Rechners, der Fisch ohne weitere Bibliothek nur Weltzeit.
  /// Benannte Grenze: ein Fisch, der ohne Maske läuft, stempelt nichts.
  final List<String> log = [];

  /// Die letzte Meldung im Wortlaut des Fischs, ohne Zeitstempel.
  String? _lastLine;

  /// Zu welchem Profil [log] gehört.
  String? logOwner;

  /// Warum ein Start gescheitert ist oder der Fisch von selbst geendet hat.
  String? runError;

  static const int logLimit = 200;

  FishRun? _run;
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

  Future<void> start(String name) async {
    if (_run != null) {
      return;
    }
    runError = null;
    log.clear();
    _lastLine = null;
    logOwner = name;
    final FishRun run;
    try {
      run = await cli.run(name);
    } on CliException catch (e) {
      runError = e.message;
      _changed();
      return;
    }
    _run = run;
    runningName = name;
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
        runningName = null;
        if (!stoppedByUs) {
          // Der Fisch ist von selbst gegangen. Der Grund steht in seinen
          // letzten Zeilen; die letzte wird hervorgehoben.
          runError =
              _lastLine ?? 'The fish ended with code $code and gave no reason.';
        }
        _changed();
        if (!_disposed) {
          await refresh();
        }
      }),
    );
  }

  bool _stopping = false;

  void stop() {
    final run = _run;
    if (run == null) {
      return;
    }
    _stopping = true;
    run.stop();
  }

  /// Beim Schließen des Fensters: den Fisch sofort beenden.
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
    log.add('${logStamp(_now())}  $line');
    if (log.length > logLimit) {
      log.removeRange(0, log.length - logLimit);
    }
    _changed();
  }

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

/// Der Zeitstempel einer Protokollzeile: „2026-11-21 03:02:52", in der Zeit,
/// die [at] trägt. Mit Datum, weil ein Fisch über Mitternacht laufen kann.
String logStamp(DateTime at) {
  String two(int n) => n.toString().padLeft(2, '0');
  return '${at.year.toString().padLeft(4, '0')}-${two(at.month)}-${two(at.day)} '
      '${two(at.hour)}:${two(at.minute)}:${two(at.second)}';
}
