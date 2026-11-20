// Was die Maske weiß: die Liste der Profile, ob die Geheimnisse liegen, und
// den einen Fisch, den dieses Fenster gestartet hat.

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'cli.dart';
import 'model.dart';

class DeskState extends ChangeNotifier {
  DeskState(this.cli);

  final FishCli cli;

  ProfileList? list;
  KeyStatus? keys;

  /// Warum die Übersicht nicht geladen werden konnte, im Wortlaut des Programms.
  String? loadError;

  /// Das Profil, das dieses Fenster gestartet hat. Höchstens eines: ein Bot
  /// kann nur einmal laufen, sonst käme jede Nachricht doppelt an.
  String? runningName;

  /// Die letzten Meldungen des laufenden oder zuletzt gelaufenen Fischs.
  final List<String> log = [];

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
          runError = log.isNotEmpty
              ? log.last
              : 'The fish ended with code $code and gave no reason.';
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

  void _addLine(String line) {
    log.add(line);
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
