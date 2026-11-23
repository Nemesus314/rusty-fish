// Die einzige Stelle, an der die Oberfläche mit dem Programm `rusty-fish`
// spricht. Die Oberfläche liest und schreibt keine Profildatei selbst; sie
// ruft das Programm auf und tauscht JSON aus. Dieselben Profile laufen später
// auf einem Server ohne Bildschirm, deshalb bleibt das Programm die eine
// Wahrheit darüber, was ein gültiges Profil ist.

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'model.dart';

/// Was das Programm als Grund nennt, wenn es etwas ablehnt. Der Text wird der
/// Person wörtlich gezeigt.
class CliException implements Exception {
  CliException(this.message);

  final String message;

  @override
  String toString() => message;
}

/// Ein laufender Fisch.
abstract class FishRun {
  /// Seine Meldungen, Zeile für Zeile. Nur Kennungen und Gründe, nie der Text
  /// einer Nachricht: so schreibt der Fisch sie.
  Stream<String> get lines;

  /// Endet, wenn der Fisch endet, auch ohne dass ihn jemand anhält.
  Future<int> get exitCode;

  /// Sagt dem laufenden Fisch eine Zeile: `start NAME` nimmt ein weiteres
  /// Profil dazu, `stop NAME` nimmt eines weg. Die anderen laufen weiter.
  void say(String line);

  /// Hält den ganzen Fisch an.
  void stop();
}

abstract class FishCli {
  Future<ProfileList> listProfiles();

  Future<ProfileDoc> showProfile(String name);

  Future<void> saveProfile(ProfileDoc doc);

  /// Verschiebt das Profil in den Papierkorb des Programms.
  Future<void> deleteProfile(String name);

  Future<KeyStatus> keyStatus();

  /// Die KI-Dienste, die das Programm kennt. Die Maske zeigt genau diese
  /// Liste. Ein älteres Programm kennt den Aufruf nicht und lehnt ihn ab.
  Future<List<Provider>> providers();

  /// [entry] ist der Name des Eintrags in der Ablage: [botTokenEntry] oder
  /// die Kennung eines Dienstes.
  Future<void> setKey(String entry, String value);

  Future<void> forgetKey(String entry);

  /// Die Modelle, die der abgelegte Schlüssel bei diesem Dienst sieht.
  ///
  /// Ohne [provider] fragt die Maske so, wie sie es vor der Auswahl tat. Das
  /// braucht sie nur gegenüber einem älteren Programm, das keine Dienste nennt.
  Future<List<String>> models({String? provider, String? baseUrl});

  /// Der Link, mit dem der eigene Bot auf einen Discord-Server kommt. Er
  /// enthält die Kennung des Bots und kein Geheimnis. Ein älteres Programm
  /// kennt den Aufruf nicht und lehnt ihn ab.
  Future<String> inviteLink();

  /// Ob und als wer der Fisch bei Matrix angemeldet ist. Ein älteres
  /// Programm kennt den Aufruf nicht und lehnt ihn ab.
  ///
  /// Mit [ask] fragt das Programm dazu den Server, ob das Gerät des Fischs
  /// vom Konto gegengezeichnet ist und ob der Zugang noch gilt. Das braucht
  /// das Netz und kann einige Sekunden dauern.
  Future<MatrixStatus> matrixStatus({bool ask = false});

  /// Meldet den Fisch an seinem Matrix-Konto an. Das Passwort geht über die
  /// Standardeingabe hinein und wird vom Programm nicht aufbewahrt. Zurück
  /// kommt, was das Programm dazu sagt.
  ///
  /// [recovery] ist der Wiederherstellungsschlüssel des Kontos, wenn es einen
  /// hat: mit ihm weist sich das Gerät des Fischs aus. Er geht denselben Weg
  /// wie das Passwort und wird ebenso wenig aufbewahrt.
  Future<String> matrixLogin(String user, String password, {String recovery = ''});

  /// Meldet den Fisch bei Matrix ab und leert seine Geräteschlüssel.
  Future<String> matrixLogout();

  /// Startet den Fisch mit allen gespeicherten Profilen. Eines, das sich
  /// nicht starten lässt, sagt in der Ausgabe, warum, und bleibt weg.
  Future<FishRun> runAll();

  /// Die Beschreibung des Programms, die man einer KI zum Erklären gibt. Sie
  /// kommt aus dem Programm selbst, damit sie zu dessen Stand gehört. Ein
  /// älteres Programm kennt den Aufruf nicht und lehnt ihn ab.
  Future<String> guide();
}

/// Die echte Umsetzung: ruft das Programm auf.
///
/// Geheimnisse und Profile gehen über die Standardeingabe hinein, nie als
/// Argument. Argumente stehen in der Prozessliste des Systems und wären dort
/// für andere Programme lesbar.
class ProcessFishCli implements FishCli {
  ProcessFishCli(this.exe);

  final String exe;

  @override
  Future<ProfileList> listProfiles() async {
    final out = await _call(['profile', 'list', '--json']);
    return ProfileList.fromJson(_object(out));
  }

  @override
  Future<ProfileDoc> showProfile(String name) async {
    final out = await _call(['profile', 'show', name, '--json']);
    return ProfileDoc.fromJson(_object(out));
  }

  @override
  Future<void> saveProfile(ProfileDoc doc) async {
    await _call(['profile', 'save', doc.name], input: jsonEncode(doc.toJson()));
  }

  @override
  Future<void> deleteProfile(String name) async {
    await _call(['profile', 'delete', name]);
  }

  @override
  Future<KeyStatus> keyStatus() async {
    final out = await _call(['key', 'status', '--json']);
    return KeyStatus.fromJson(_object(out));
  }

  @override
  Future<List<Provider>> providers() async {
    final out = await _call(['providers', '--json']);
    final decoded = _decode(out);
    if (decoded is! List) {
      throw CliException('rusty-fish answered in an unexpected form.');
    }
    final list = [
      for (final item in decoded)
        if (item is Map)
          Provider.fromJson(item.map((key, value) => MapEntry('$key', value))),
    ];
    if (list.isEmpty) {
      throw CliException('rusty-fish named no AI service.');
    }
    return list;
  }

  @override
  Future<void> setKey(String entry, String value) async {
    await _call(['key', 'set', entry, '--stdin'], input: '$value\n');
  }

  @override
  Future<void> forgetKey(String entry) async {
    await _call(['key', 'forget', entry]);
  }

  @override
  Future<List<String>> models({String? provider, String? baseUrl}) async {
    final url = baseUrl?.trim() ?? '';
    final out = await _call([
      'models',
      if (provider != null) ...['--provider', provider],
      if (url.isNotEmpty) ...['--base-url', url],
      '--json',
    ]);
    final decoded = _decode(out);
    if (decoded is! List) {
      throw CliException('rusty-fish answered in an unexpected form.');
    }
    return [for (final item in decoded) '$item'];
  }

  @override
  Future<FishRun> runAll() async {
    final Process process;
    try {
      process = await Process.start(exe, ['run', '--all']);
    } on ProcessException catch (e) {
      throw CliException('Cannot start rusty-fish: ${e.message}');
    }
    return _ProcessRun(process);
  }

  @override
  Future<MatrixStatus> matrixStatus({bool ask = false}) async {
    if (ask) {
      try {
        final out = await _call(['matrix', 'status', '--json', '--ask']);
        return MatrixStatus.fromJson(_object(out));
      } on CliException {
        // Ein Programm vom Stand vor dem 23.11.2026 kennt die Frage an den
        // Server nicht. Dann bleibt, was es ohne sie sagt.
      }
    }
    final out = await _call(['matrix', 'status', '--json']);
    return MatrixStatus.fromJson(_object(out));
  }

  // Die Kennung des Kontos ist kein Geheimnis und darf ein Argument sein.
  // Das Passwort nicht: es geht über die Standardeingabe.
  @override
  Future<String> matrixLogin(String user, String password, {String recovery = ''}) => _call([
    'matrix',
    'login',
    '--user',
    user,
    '--stdin',
  ], input: '$password\n$recovery\n');

  @override
  Future<String> matrixLogout() => _call(['matrix', 'logout']);

  @override
  Future<String> guide() => _call(['guide']);

  @override
  Future<String> inviteLink() async => (await _call(['invite'])).trim();

  Future<String> _call(List<String> args, {String? input}) async {
    final Process process;
    try {
      process = await Process.start(exe, args);
    } on ProcessException catch (e) {
      throw CliException('Cannot start rusty-fish: ${e.message}');
    }
    // Erst zuhören, dann schreiben: sonst kann ein Programm, das viel
    // ausgibt, auf uns warten, während wir auf es warten.
    final stdoutText = process.stdout.transform(utf8.decoder).join();
    final stderrText = process.stderr.transform(utf8.decoder).join();
    if (input != null) {
      process.stdin.add(utf8.encode(input));
    }
    try {
      await process.stdin.close();
    } on IOException {
      // Das Programm hat seine Eingabe schon geschlossen. Was es dazu sagt,
      // steht in seinem Fehlertext.
    }
    final code = await process.exitCode;
    final out = await stdoutText;
    final err = await stderrText;
    if (code != 0) {
      throw CliException(_reason(err, out, code));
    }
    return out;
  }

  Map<String, dynamic> _object(String text) {
    final decoded = _decode(text);
    if (decoded is! Map) {
      throw CliException('rusty-fish answered in an unexpected form.');
    }
    return decoded.map((key, value) => MapEntry('$key', value));
  }

  Object? _decode(String text) {
    try {
      return jsonDecode(text);
    } on FormatException {
      throw CliException('rusty-fish answered with something that is not JSON.');
    }
  }
}

/// Der Grund, so wie das Programm ihn nennt, ohne seinen Vorsatz.
String _reason(String err, String out, int code) {
  var text = err.trim();
  if (text.isEmpty) {
    text = out.trim();
  }
  if (text.isEmpty) {
    return 'rusty-fish ended with code $code and gave no reason.';
  }
  const prefix = 'rusty-fish: ';
  return text.startsWith(prefix) ? text.substring(prefix.length) : text;
}

class _ProcessRun implements FishRun {
  _ProcessRun(this._process) {
    // Der Fisch schreibt seine Meldungen auf die Fehlerausgabe. Die
    // Standardausgabe wird mitgelesen, damit nichts verloren geht und der
    // Fisch nie an einer vollen Leitung hängt.
    _listen(_process.stderr);
    _listen(_process.stdout);
    _process.exitCode.then((_) async {
      await Future.wait(_done);
      await _lines.close();
    });
  }

  final Process _process;
  final _lines = StreamController<String>.broadcast();
  final _done = <Future<void>>[];

  void _listen(Stream<List<int>> source) {
    final finished = Completer<void>();
    _done.add(finished.future);
    source
        .transform(const Utf8Decoder(allowMalformed: true))
        .transform(const LineSplitter())
        .listen(
          _lines.add,
          onError: (Object _) {},
          onDone: finished.complete,
          cancelOnError: false,
        );
  }

  @override
  Stream<String> get lines => _lines.stream;

  @override
  Future<int> get exitCode => _process.exitCode;

  /// Was dem Fisch gesagt wird, geht nacheinander hinaus: die Eingabe nimmt
  /// keine neue Zeile an, solange die vorige noch unterwegs ist.
  Future<void> _sending = Future.value();

  @override
  void say(String line) {
    _sending = _sending.then((_) async {
      try {
        _process.stdin.writeln(line);
        await _process.stdin.flush();
      } on Object {
        // Die Eingabe ist schon zu: der Fisch ist am Ende oder schon fort.
      }
    });
  }

  @override
  void stop() {
    // Erst bitten, dann beenden. Gemessen von Nemesus am 20.11.2026: nach
    // einem harten Ende stand der Bot in Discord noch eine Weile als online
    // da. Auf die Zeile „stop" meldet sich der Fisch bei Discord ab und endet
    // von selbst. Tut er das nicht binnen acht Sekunden, wird er beendet.
    // Bis zum 23.11.2026 waren es vier. Seither sagt der Fisch in seinen
    // Matrix-Räumen, dass er geht, und nimmt sich dafür bis zu fünf.
    say('stop');
    Timer(const Duration(seconds: 8), _process.kill);
  }
}

/// Wo das Programm gesucht wurde und ob es gefunden ist.
class FishLocation {
  const FishLocation({required this.found, required this.checked});

  final String? found;

  /// Alle geprüften Orte, in der Reihenfolge der Suche. Wird gezeigt, wenn
  /// nichts gefunden wurde, damit die Person weiß, wohin das Programm gehört.
  final List<String> checked;
}

/// Sucht das Programm `rusty-fish`.
///
/// 1. Die Umgebungsvariable RUSTY_FISH_EXE, falls gesetzt.
/// 2. Neben der eigenen exe: so wird das Paar später ausgeliefert.
/// 3. Von der eigenen exe aufwärts nach `target/debug`: der Entwicklungsfall,
///    in dem die Oberfläche tief im Bauordner des Repositorys liegt.
FishLocation locateFish({
  required Map<String, String> environment,
  required String ownExecutable,
  required bool Function(String path) exists,
  bool windows = true,
}) {
  final file = windows ? 'rusty-fish.exe' : 'rusty-fish';
  final sep = windows ? r'\' : '/';
  final checked = <String>[];

  bool hit(String path) {
    checked.add(path);
    return exists(path);
  }

  final fromEnv = environment['RUSTY_FISH_EXE']?.trim() ?? '';
  if (fromEnv.isNotEmpty && hit(fromEnv)) {
    return FishLocation(found: fromEnv, checked: checked);
  }

  var dir = _parent(ownExecutable, sep);
  if (dir != null) {
    final beside = '$dir$sep$file';
    if (hit(beside)) {
      return FishLocation(found: beside, checked: checked);
    }
  }
  while (dir != null) {
    final candidate = '$dir${sep}target${sep}debug$sep$file';
    if (hit(candidate)) {
      return FishLocation(found: candidate, checked: checked);
    }
    dir = _parent(dir, sep);
  }
  return FishLocation(found: null, checked: checked);
}

String? _parent(String path, String sep) {
  final cut = path.lastIndexOf(sep);
  if (cut <= 0) {
    return null;
  }
  return path.substring(0, cut);
}
