// Baut das Windows-Paket zum Herunterladen: einen Ordner, den man entpackt und
// startet, und daraus eine ZIP-Datei.
//
// Nemesus am 24.11.2026: „Wenn sich jetzt jemand den Fisch für Windows laden
// möchte, gibt es dann eine Möglichkeit, das ohne Bauen zu tun?" Die erste
// Leserin des Repositorys ist keine Entwicklerin.
//
// Aufruf, aus dem Ordner des Repositorys:
//
//     dart run packaging/windows.dart
//
// Geschrieben in Dart, weil Dart mit Flutter schon da ist: wer die Maske
// bauen kann, kann auch das Paket bauen, ohne ein weiteres Werkzeug.
//
// Was im Paket liegt, und warum:
// - Die Maske, wie `flutter build windows --release` sie baut, und
//   `rusty-fish.exe` daneben; dort sucht die Maske den Fisch.
// - Drei Dateien von Microsofts C++-Laufzeit. Sie kommen aus dem Ordner, den
//   Microsoft den Bauwerkzeugen zum Weitergeben beilegt, nicht aus dem
//   Windows dieses Rechners.
// - `LICENSE`, `GUIDE.md` und eine Liesmich-Datei mit der Adresse des
//   Quelltexts und dem Commit, aus dem gebaut wurde: wer das Programm
//   weitergibt, nennt, wo dessen Quelltext liegt.
// - Die Hinweise der fremden Bausteine. Fast alle verlangen, dass ihr
//   Lizenztext dem fertigen Programm beiliegt.
//
// Benannte Grenzen:
// - Das Programm ist nicht signiert; Windows warnt beim ersten Start. Die
//   Liesmich-Datei sagt es.
// - Die Hinweise der Bausteine des Fischs stammen aus den Dateien, die jeder
//   Baustein selbst mitbringt (LICENSE, COPYING, NOTICE und ähnliche). Bringt
//   einer keine mit, steht er mit seiner Lizenzangabe in einer eigenen Liste.
//   Die Rust-Standardbibliothek ist genannt, ihre Bausteine nicht einzeln.
// - Welche Dateien der Laufzeit die Maske braucht, ist nicht abgeleitet: ein
//   Ordner mit diesen dreien lief am 22.11.2026 auf einem Laptop ohne
//   Entwicklerwerkzeug. Ob dort Microsofts Laufzeit ohnehin installiert war,
//   ist nicht gemessen; auf einem Windows ganz ohne sie ist das Paket nicht
//   gelaufen.

import 'dart:convert';
import 'dart:io';

const address = 'https://github.com/Nemesus314/rusty-fish';

/// Die Dateien von Microsofts Laufzeit, die mitgehen.
const runtime = ['msvcp140.dll', 'vcruntime140.dll', 'vcruntime140_1.dll'];

Future<void> main(List<String> arguments) async {
  final repo = Directory.current;
  if (!File('${repo.path}/Cargo.toml').existsSync() || !Directory('${repo.path}/desk').existsSync()) {
    fail('run this from the folder of the repository: dart run packaging/windows.dart');
  }
  final version = RegExp(r'^version = "([^"]+)"', multiLine: true)
      .firstMatch(File('${repo.path}/Cargo.toml').readAsStringSync())
      ?.group(1);
  if (version == null) fail('cannot read the version from Cargo.toml');

  // Das Paket nennt den Commit, aus dem es gebaut ist. Mit offenen
  // Änderungen stimmte diese Angabe nicht.
  final commit = (await run('git', ['rev-parse', 'HEAD'])).trim();
  final open = (await run('git', ['status', '--porcelain'])).trim();
  if (open.isNotEmpty && !arguments.contains('--with-open-changes')) {
    fail('there are changes that are not committed; the package names its commit.\n$open');
  }

  // Der Compiler schreibt die Pfade der Quelldateien ins Programm, für seine
  // Fehlermeldungen. Gemessen am 25.11.2026: im gewöhnlichen Bau stand so
  // der Windows-Benutzername dieses Rechners 578-mal in `rusty-fish.exe`. Für
  // das Paket werden die Pfade deshalb ersetzt. Das ändert die Schalter des
  // Compilers und damit jeden Baustein; ein eigener Bau-Ordner hält den
  // gewöhnlichen Bau davon frei.
  final home = Platform.environment['USERPROFILE'];
  if (home == null || home.isEmpty) fail('cannot tell the folder of this user (USERPROFILE)');
  final building = '${repo.path}\\target\\package-build';
  say('building the fish');
  await run(
    'cargo',
    ['build', '--release', '--locked', '-p', 'rusty-fish', '--target-dir', building],
    environment: {
      // Mit dem Trennzeichen 0x1f, damit ein Pfad mit Leerzeichen heil bleibt.
      'CARGO_ENCODED_RUSTFLAGS': ['--remap-path-prefix=$home=~', '--remap-path-prefix=${repo.path}=rusty-fish'].join('\x1f'),
      // Dasselbe für die Bausteine in C: ihre Fehlermeldungen tragen den Pfad
      // der Quelldatei, und der Schalter oben gilt nur für Rust. Gemessen am
      // 25.11.2026: ohne dies blieben die Pfade von aws-lc-sys im Programm.
      // Der Schalter ist einer von Microsofts Compiler; er schneidet den
      // Anfang des Pfads ab.
      'CFLAGS': '/d1trimfile:$home\\',
    },
    show: true,
  );
  final fish = File('$building\\release\\rusty-fish.exe');
  final user = home.split(RegExp(r'[\\/]')).last;
  if (latin1.decode(fish.readAsBytesSync()).contains(user)) {
    fail('rusty-fish.exe still carries the name of this user ($user) in its paths');
  }
  say('building the window');
  await run('flutter', ['build', 'windows', '--release'], directory: '${repo.path}/desk', show: true);

  final name = 'RustyFish-$version-windows-x64';
  final packages = Directory('${repo.path}/target/package');
  final out = Directory('${packages.path}/$name');
  if (out.existsSync()) out.deleteSync(recursive: true);
  out.createSync(recursive: true);

  say('putting the folder together');
  copyTree(Directory('${repo.path}/desk/build/windows/x64/runner/Release'), out);
  copy(fish.path, '${out.path}/rusty-fish.exe');
  final from = runtimeFolder();
  for (final file in runtime) {
    copy('${from.path}/$file', '${out.path}/$file');
  }
  copy('${repo.path}/LICENSE', '${out.path}/LICENSE');
  copy('${repo.path}/GUIDE.md', '${out.path}/GUIDE.md');
  File('${out.path}/README.txt').writeAsStringSync(
    File('${repo.path}/packaging/README-windows.txt')
        .readAsStringSync()
        .replaceAll('{version}', version)
        .replaceAll('{commit}', commit),
  );

  say('collecting the notices of the parts');
  final notices = await noticesOfTheFish(version, commit);
  File('${out.path}/THIRD-PARTY-NOTICES.txt').writeAsStringSync(notices.text);
  final window = File('${out.path}/data/flutter_assets/NOTICES.Z');
  if (!window.existsSync()) fail('the window has no notices (${window.path})');
  File('${out.path}/THIRD-PARTY-NOTICES-window.txt').writeAsStringSync(
    'Notices of the parts the window (rusty_fish_desk.exe) is built from,\n'
    'as Flutter collects them. Rusty Fish $version.\n\n'
    '${utf8.decode(gzip.decode(window.readAsBytesSync()))}',
  );

  // Windows bringt ein tar mit, das ZIP-Dateien schreibt.
  final zip = File('${packages.path}/$name.zip');
  if (zip.existsSync()) zip.deleteSync();
  await run(r'C:\Windows\System32\tar.exe', ['-a', '-cf', '$name.zip', name], directory: packages.path);
  final sum = RegExp(r'^[0-9a-f]{64}$', multiLine: true)
      .firstMatch(await run('certutil', ['-hashfile', zip.path, 'SHA256']))
      ?.group(0);

  say('done');
  stdout.writeln('folder:  ${out.path}');
  stdout.writeln('file:    ${zip.path} (${zip.lengthSync()} bytes)');
  stdout.writeln('sha256:  $sum');
  stdout.writeln('version: $version, commit $commit');
  stdout.writeln('runtime: ${from.path}');
  stdout.writeln(
    'parts:   ${notices.packages} packages, ${notices.texts} different license texts, '
    '${notices.without} packages without a license file of their own',
  );
}

/// Der Ordner, aus dem Microsofts Laufzeit kommt: der, den Microsoft den
/// Bauwerkzeugen zum Weitergeben beilegt. Gibt es mehrere, der neueste.
Directory runtimeFolder() {
  final found = <Directory>[];
  for (final base in [r'C:\Program Files\Microsoft Visual Studio', r'C:\Program Files (x86)\Microsoft Visual Studio']) {
    final studio = Directory(base);
    if (!studio.existsSync()) continue;
    for (final year in studio.listSync().whereType<Directory>()) {
      for (final edition in year.listSync().whereType<Directory>()) {
        final redist = Directory('${edition.path}\\VC\\Redist\\MSVC');
        if (!redist.existsSync()) continue;
        for (final toolset in redist.listSync().whereType<Directory>()) {
          final x64 = Directory('${toolset.path}\\x64');
          if (!x64.existsSync()) continue;
          for (final crt in x64.listSync().whereType<Directory>()) {
            final isCrt = crt.path.toUpperCase().endsWith('.CRT');
            if (isCrt && runtime.every((file) => File('${crt.path}\\$file').existsSync())) {
              found.add(crt);
            }
          }
        }
      }
    }
  }
  if (found.isEmpty) {
    fail(
      'cannot find the Visual C++ runtime that Microsoft ships for redistribution '
      r'(…\VC\Redist\MSVC\<version>\x64\Microsoft.VC143.CRT)',
    );
  }
  found.sort((a, b) => a.path.compareTo(b.path));
  return found.last;
}

class Notices {
  Notices(this.text, this.packages, this.texts, this.without);
  final String text;
  final int packages;
  final int texts;
  final int without;
}

/// Die Hinweise aller fremden Bausteine, aus denen `rusty-fish.exe` gebaut
/// ist: was Cargo für Windows wirklich hineinbaut, ohne die Bausteine, die
/// nur beim Bauen oder nur in Tests gebraucht werden.
Future<Notices> noticesOfTheFish(String version, String commit) async {
  final metadata =
      jsonDecode(
            await run('cargo', [
              'metadata',
              '--format-version',
              '1',
              '--locked',
              '--filter-platform',
              'x86_64-pc-windows-msvc',
            ]),
          )
          as Map<String, dynamic>;
  final byId = {for (final p in metadata['packages'] as List) (p as Map)['id'] as String: p};
  final members = (metadata['workspace_members'] as List).cast<String>().toSet();
  final nodes = {
    for (final n in (metadata['resolve'] as Map)['nodes'] as List) (n as Map)['id'] as String: n,
  };

  final start = members.firstWhere(
    (id) => byId[id]!['name'] == 'rusty-fish',
    orElse: () => fail('cannot find the package rusty-fish'),
  );
  final seen = <String>{start};
  final queue = [start];
  while (queue.isNotEmpty) {
    final node = nodes[queue.removeLast()]!;
    for (final dep in node['deps'] as List) {
      // `kind` fehlt bei einem gewöhnlichen Baustein; „build" und „dev"
      // kommen nicht ins fertige Programm.
      final built = (dep['dep_kinds'] as List).any((kind) => (kind as Map)['kind'] == null);
      final id = dep['pkg'] as String;
      if (built && seen.add(id)) queue.add(id);
    }
  }

  final foreign =
      seen.where((id) => !members.contains(id)).map((id) => byId[id]!).toList()..sort(
        (a, b) =>
            '${a['name']} ${a['version']}'.toLowerCase().compareTo('${b['name']} ${b['version']}'.toLowerCase()),
      );

  // Gleiche Texte stehen einmal da, mit allen Bausteinen, die sie tragen.
  final texts = <String, List<String>>{};
  final without = <String>[];
  final isNotice = RegExp(r'^(licen[sc]e|copying|notice|unlicense|copyright)', caseSensitive: false);
  for (final package in foreign) {
    final label = '${package['name']} ${package['version']}';
    final folder = File(package['manifest_path'] as String).parent;
    final files =
        folder.listSync().whereType<File>().where((file) {
          final name = file.uri.pathSegments.last;
          return isNotice.hasMatch(name) && file.lengthSync() < 300 * 1024;
        }).toList()..sort((a, b) => a.path.compareTo(b.path));
    var any = false;
    for (final file in files) {
      final String text;
      try {
        text = file.readAsStringSync().replaceAll('\r\n', '\n').trim();
      } on FileSystemException {
        continue; // kein Text
      }
      if (text.isEmpty) continue;
      texts.putIfAbsent(text, () => []).add('$label (${file.uri.pathSegments.last})');
      any = true;
    }
    if (!any) without.add('$label: ${package['license'] ?? 'no license named'}');
  }

  final out = StringBuffer()
    ..writeln('Notices of the parts the fish (rusty-fish.exe) is built from')
    ..writeln('Rusty Fish $version, commit $commit')
    ..writeln()
    ..writeln('rusty-fish.exe is built from the ${foreign.length} packages listed below, written by')
    ..writeln('others, each under the license named next to it. Their license texts follow')
    ..writeln('the list; a text that several packages share is printed once. This file was')
    ..writeln('made by packaging/windows.dart from what `cargo metadata` names for Windows.')
    ..writeln('The list errs on the side of too many: it names a few packages that Cargo')
    ..writeln('resolves for this program without building them into it.')
    ..writeln()
    ..writeln('It also contains the Rust standard library, under MIT OR Apache-2.0,')
    ..writeln('copyright The Rust Project contributors: https://www.rust-lang.org/policies/licenses')
    ..writeln()
    ..writeln('PACKAGES')
    ..writeln();
  for (final package in foreign) {
    out.writeln(
      '${package['name']} ${package['version']}  |  ${package['license'] ?? 'see its files'}'
      '${package['repository'] == null ? '' : '  |  ${package['repository']}'}',
    );
  }
  if (without.isNotEmpty) {
    out
      ..writeln()
      ..writeln('PACKAGES THAT BRING NO LICENSE FILE OF THEIR OWN')
      ..writeln('For these the license named here applies; its text is among those below.')
      ..writeln();
    without.forEach(out.writeln);
  }
  out
    ..writeln()
    ..writeln('LICENSE TEXTS');
  for (final entry in texts.entries) {
    out
      ..writeln()
      ..writeln('=' * 76)
      ..writeln('Used by: ${entry.value.join(', ')}')
      ..writeln('=' * 76)
      ..writeln()
      ..writeln(entry.key);
  }
  return Notices(out.toString(), foreign.length, texts.length, without.length);
}

/// Führt einen Befehl aus und liefert seine Ausgabe. Scheitert er, endet das
/// Skript mit seiner Meldung.
Future<String> run(
  String program,
  List<String> arguments, {
  String? directory,
  Map<String, String>? environment,
  bool show = false,
}) async {
  // Über die Shell, weil `flutter` und `cargo` unter Windows keine .exe im
  // engeren Sinn sein müssen (flutter.bat).
  // Die Ausgabe als Bytes, nachsichtig gelesen: ein deutsches Windows mischt
  // in die Meldungen der Werkzeuge Zeichen, die kein UTF-8 sind, und daran
  // soll das Paket nicht scheitern.
  final result = await Process.run(
    program,
    arguments,
    workingDirectory: directory,
    environment: environment,
    runInShell: true,
    stdoutEncoding: null,
    stderrEncoding: null,
  );
  final said = utf8.decode(result.stdout as List<int>, allowMalformed: true);
  final complained = utf8.decode(result.stderr as List<int>, allowMalformed: true);
  if (result.exitCode != 0) {
    fail('$program ${arguments.join(' ')} failed (${result.exitCode}):\n$said\n$complained');
  }
  if (show) {
    final lines = '$said$complained'.trim().split('\n');
    stdout.writeln('  ${lines.last.trim()}');
  }
  return said;
}

void copy(String from, String to) {
  final file = File(from);
  if (!file.existsSync()) fail('missing: $from');
  file.copySync(to);
}

void copyTree(Directory from, Directory to) {
  if (!from.existsSync()) fail('missing: ${from.path}');
  for (final entry in from.listSync(recursive: true)) {
    final relative = entry.path.substring(from.path.length);
    if (entry is Directory) {
      Directory('${to.path}$relative').createSync(recursive: true);
    } else if (entry is File) {
      File('${to.path}$relative').parent.createSync(recursive: true);
      entry.copySync('${to.path}$relative');
    }
  }
}

void say(String what) => stdout.writeln('== $what');

Never fail(String why) {
  stderr.writeln('packaging: $why');
  exit(1);
}
