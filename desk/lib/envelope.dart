// Export und Import: alle Profile samt Bot-Token und KI-Schlüsseln in einer
// Datei mit Kennwort, für den Umzug auf einen anderen Rechner und als
// Sicherung.
//
// Nemesus am 25.11.2026, nachdem „Share rooms" nur ein halbes Profil vom PC
// nach Linux Mint gebracht hatte: „Die Import / Export Funktion scheint mir
// essentiell. Das sollten wir gleich in Angriff nehmen." Entschieden von ihm
// am selben Tag: die Knöpfe heißen „Export…" und „Import…", die Datei endet
// auf `.rustyfish`, der Vorschlag für ihren Namen trägt das Datum, und die
// Maske fragt mit dem Dialog des Systems, wohin sie soll („Ja, mit
// Dateidialog"). In der Maske gibt es nur diesen einen Weg; der Export ohne
// Geheimnisse bleibt auf der Kommandozeile.
//
// Die Maske fragt nur und zeigt, was das Programm meldet. Was in der Datei
// steht, wie sie verschlossen ist, ob ein Kennwort genügt und was beim
// Einlesen abgelehnt wird, entscheidet das Programm (`rusty-fish envelope`).
//
// Benannte Grenze: das Kennwort steht in zwei Feldern dieses Fensters, bis
// der Aufruf beginnt; dann werden sie geleert. Im Speicher der Maske löscht
// es niemand eigens.

import 'package:file_selector/file_selector.dart' as system;
import 'package:material_ui/material_ui.dart';

import 'cli.dart';
import 'model.dart';

/// Die Endung, die der Vorschlag für den Namen der Datei trägt.
const envelopeEnding = 'rustyfish';

/// Ab dieser Länge nimmt das Programm ein Kennwort an, gezählt in Zeichen.
/// Die Maske prüft das nur, um es zu sagen, bevor der Dialog des Systems
/// aufgeht; entscheiden tut das Programm.
const envelopePasswordChars = 12;

/// Der Vorschlag für den Namen der Datei: mit dem Datum des Tages am Ort des
/// Menschen, damit man zwei Umschläge auseinanderhält, ohne sie zu öffnen.
String suggestedEnvelopeName(DateTime now) {
  String two(int n) => n.toString().padLeft(2, '0');
  return 'rusty-fish-${now.year}-${two(now.month)}-${two(now.day)}.$envelopeEnding';
}

/// Der Name, unter dem geschrieben wird. Wer im Dialog einen Namen ohne
/// Endung tippt, bekommt die des Vorschlags; eine andere Endung bleibt.
String envelopePath(String chosen) {
  final name = chosen.split(RegExp(r'[\\/]')).last;
  return name.contains('.') ? chosen : '$chosen.$envelopeEnding';
}

/// Wie ein Geheimnis in der Maske heißt: der Token des Bots oder der
/// Schlüssel eines Dienstes. Ein Eintrag, den die Liste der Dienste nicht
/// nennt, steht mit seinem Namen da.
String secretLabel(String entry, List<Provider> providers) {
  if (entry == botTokenEntry) {
    return botTokenLabel;
  }
  for (final provider in providers) {
    if (provider.id == entry) {
      return '${provider.label} API key';
    }
  }
  return entry;
}

/// Fragt den Menschen nach einer Datei. Als eigene Stelle, damit die Tests
/// keinen Dialog des Systems öffnen.
abstract class FileChooser {
  /// Wohin geschrieben wird, mit [suggestedName] als Vorschlag. `null`, wenn
  /// der Mensch abbricht.
  Future<String?> saveAs(String suggestedName);

  /// Welche Datei gelesen wird. `null`, wenn der Mensch abbricht.
  Future<String?> open();
}

/// Der Dialog des Systems.
class SystemFileChooser implements FileChooser {
  const SystemFileChooser();

  static const _types = [
    system.XTypeGroup(label: 'Rusty Fish export', extensions: [envelopeEnding]),
  ];

  @override
  Future<String?> saveAs(String suggestedName) async {
    final location = await system.getSaveLocation(
      acceptedTypeGroups: _types,
      suggestedName: suggestedName,
    );
    return location?.path;
  }

  @override
  Future<String?> open() async {
    // Ohne Filter auf die Endung: wer die Datei umbenannt hat, soll sie
    // trotzdem wählen können. Ob es ein Umschlag ist, sagt das Programm.
    final file = await system.openFile(
      acceptedTypeGroups: const [
        ..._types,
        system.XTypeGroup(label: 'All files'),
      ],
    );
    return file?.path;
  }
}

/// Das Fenster von „Export…". Schließt mit `true`, wenn geschrieben wurde.
class ExportDialog extends StatefulWidget {
  const ExportDialog({
    super.key,
    required this.cli,
    required this.files,
    required this.providers,
    required this.now,
  });

  final FishCli cli;
  final FileChooser files;
  final List<Provider> providers;

  /// Die Uhr für den Vorschlag des Namens.
  final DateTime Function() now;

  @override
  State<ExportDialog> createState() => _ExportDialogState();
}

class _ExportDialogState extends State<ExportDialog> {
  final _password = TextEditingController();
  final _again = TextEditingController();
  String? _error;
  bool _busy = false;
  EnvelopeWritten? _written;
  String? _path;

  @override
  void dispose() {
    _password.dispose();
    _again.dispose();
    super.dispose();
  }

  Future<void> _export() async {
    final password = _password.text;
    final again = _again.text;
    // Vor dem Dialog des Systems: wer dort schon einen Ort gewählt hat, soll
    // nicht danach erfahren, dass das Kennwort nicht genügt.
    if (password != again) {
      setState(() => _error = 'The two passwords differ.');
      return;
    }
    if (password.runes.length < envelopePasswordChars) {
      setState(() => _error = 'The password needs at least $envelopePasswordChars characters.');
      return;
    }
    setState(() {
      _error = null;
      _busy = true;
    });
    final chosen = await widget.files.saveAs(suggestedEnvelopeName(widget.now()));
    if (chosen == null) {
      if (mounted) {
        setState(() => _busy = false);
      }
      return;
    }
    final path = envelopePath(chosen);
    try {
      final written = await widget.cli.writeEnvelope(path, password, again);
      // Erst jetzt: scheitert das Schreiben, etwa weil es die Datei schon
      // gibt, muss niemand das Kennwort noch einmal tippen.
      _password.clear();
      _again.clear();
      if (mounted) {
        setState(() {
          _written = written;
          _path = path;
        });
      }
    } on CliException catch (e) {
      if (mounted) {
        setState(() => _error = e.message);
      }
    }
    if (mounted) {
      setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final written = _written;
    final error = _error;
    return AlertDialog(
      title: const Text('Export'),
      content: SizedBox(
        width: 620,
        child: SingleChildScrollView(
          padding: const EdgeInsets.only(right: 24),
          child: written != null
              ? _done(theme, written)
              : Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text(
                      'Writes everything into one file, locked with a password: all your '
                      'profiles, the bot token and the AI keys. For moving to another PC, or as '
                      'a copy to keep. It is for you alone: to let someone join a room with a '
                      'fish of their own, use "Share rooms" on a card.',
                    ),
                    const SizedBox(height: 8),
                    Text(
                      // Nemesus am 25.11.2026: „Bei dem Hinweis, dass Matrix nicht
                      // übernommen wird, ein kleiner Nebensatz, der begründet,
                      // warum nicht."
                      'Not in the file: the Matrix account of the fish, because its login is a '
                      'device with keys of its own, and one device cannot run in two places. Log '
                      'the fish in again where you import the file.',
                      style: theme.textTheme.bodySmall,
                    ),
                    const SizedBox(height: 16),
                    TextField(
                      key: const ValueKey('export-password'),
                      controller: _password,
                      obscureText: true,
                      enabled: !_busy,
                      onChanged: (_) => setState(() {}),
                      decoration: const InputDecoration(
                        labelText: 'Password for the file',
                        helperText: 'At least $envelopePasswordChars characters, of any kind.',
                        border: OutlineInputBorder(),
                      ),
                    ),
                    const SizedBox(height: 12),
                    TextField(
                      key: const ValueKey('export-password-again'),
                      controller: _again,
                      obscureText: true,
                      enabled: !_busy,
                      onChanged: (_) => setState(() {}),
                      decoration: const InputDecoration(
                        labelText: 'The same password once more',
                        border: OutlineInputBorder(),
                      ),
                    ),
                    const SizedBox(height: 12),
                    Text(
                      'Only the password opens the file, and nothing brings a forgotten one '
                      'back.',
                      style: theme.textTheme.bodySmall,
                    ),
                    if (error != null)
                      Padding(
                        padding: const EdgeInsets.only(top: 12),
                        child: Text(error, style: TextStyle(color: theme.colorScheme.error)),
                      ),
                  ],
                ),
        ),
      ),
      actions: written != null
          ? [
              FilledButton(
                onPressed: () => Navigator.of(context).pop(true),
                child: const Text('Close'),
              ),
            ]
          : [
              TextButton(
                onPressed: _busy ? null : () => Navigator.of(context).pop(false),
                child: const Text('Cancel'),
              ),
              FilledButton(
                key: const ValueKey('export-choose'),
                onPressed: _busy || _password.text.isEmpty || _again.text.isEmpty ? null : _export,
                child: const Text('Choose where to save…'),
              ),
            ],
    );
  }

  Widget _done(ThemeData theme, EnvelopeWritten written) {
    final profiles = written.profiles == 1 ? '1 profile' : '${written.profiles} profiles';
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text('Written to $_path', style: theme.textTheme.titleSmall),
        const SizedBox(height: 8),
        Text('In it: $profiles'),
        for (final entry in written.secrets) Text('In it: ${secretLabel(entry, widget.providers)}'),
        const SizedBox(height: 12),
        const Text(
          'Keep the password where you will find it again. Without it the file cannot be '
          'opened, by nobody.',
        ),
        if (written.matrixLeftOut)
          const Padding(
            padding: EdgeInsets.only(top: 8),
            child: Text(
              'The Matrix account of the fish is not in it, because its login is a device with '
              'keys of its own, and one device cannot run in two places. Log the fish in again '
              'where you import the file, under "Matrix".',
            ),
          ),
        if (written.running)
          const Padding(
            padding: EdgeInsets.only(top: 8),
            child: Text(
              'The fish is on. What it learns from now on (who agreed, what was read) is not '
              'in the file: to move, switch it off first and export again.',
            ),
          ),
      ],
    );
  }
}

/// Das Fenster von „Import…". Schließt mit dem, was eingelesen wurde, oder
/// mit `null`, wenn nichts geschah.
class ImportDialog extends StatefulWidget {
  const ImportDialog({
    super.key,
    required this.cli,
    required this.files,
    required this.providers,
  });

  final FishCli cli;
  final FileChooser files;
  final List<Provider> providers;

  @override
  State<ImportDialog> createState() => _ImportDialogState();
}

class _ImportDialogState extends State<ImportDialog> {
  final _password = TextEditingController();
  String? _path;
  String? _error;
  bool _busy = false;
  EnvelopeRead? _read;

  @override
  void dispose() {
    _password.dispose();
    super.dispose();
  }

  Future<void> _choose() async {
    final chosen = await widget.files.open();
    if (chosen != null && mounted) {
      setState(() {
        _path = chosen;
        _error = null;
      });
    }
  }

  Future<void> _import() async {
    final path = _path;
    if (path == null) {
      return;
    }
    setState(() {
      _error = null;
      _busy = true;
    });
    try {
      final read = await widget.cli.readEnvelope(path, _password.text);
      _password.clear();
      if (mounted) {
        setState(() => _read = read);
      }
    } on CliException catch (e) {
      if (mounted) {
        setState(() => _error = e.message);
      }
    }
    if (mounted) {
      setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final read = _read;
    final error = _error;
    final path = _path;
    return AlertDialog(
      title: const Text('Import'),
      content: SizedBox(
        width: 620,
        child: SingleChildScrollView(
          padding: const EdgeInsets.only(right: 24),
          child: read != null
              ? _done(theme, read)
              : Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text(
                      'Opens a file written with "Export…" and takes what it holds: profiles, '
                      'the bot token and the AI keys. Nothing that is here already is replaced: '
                      'a profile or a key of the same name is left as it is, and you are told.',
                    ),
                    const SizedBox(height: 16),
                    Row(
                      children: [
                        OutlinedButton(
                          key: const ValueKey('import-choose'),
                          onPressed: _busy ? null : _choose,
                          child: const Text('Choose file…'),
                        ),
                        const SizedBox(width: 12),
                        Expanded(
                          child: Text(
                            path ?? 'No file chosen yet.',
                            style: theme.textTheme.bodySmall,
                            overflow: TextOverflow.ellipsis,
                            maxLines: 2,
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 16),
                    TextField(
                      key: const ValueKey('import-password'),
                      controller: _password,
                      obscureText: true,
                      enabled: !_busy,
                      onChanged: (_) => setState(() {}),
                      decoration: const InputDecoration(
                        labelText: 'Password of the file',
                        border: OutlineInputBorder(),
                      ),
                    ),
                    if (error != null)
                      Padding(
                        padding: const EdgeInsets.only(top: 12),
                        child: Text(error, style: TextStyle(color: theme.colorScheme.error)),
                      ),
                  ],
                ),
        ),
      ),
      actions: read != null
          ? [
              FilledButton(
                onPressed: () => Navigator.of(context).pop(read),
                child: const Text('Close'),
              ),
            ]
          : [
              TextButton(
                onPressed: _busy ? null : () => Navigator.of(context).pop(),
                child: const Text('Cancel'),
              ),
              FilledButton(
                key: const ValueKey('import-read'),
                onPressed: _busy || path == null || _password.text.isEmpty ? null : _import,
                child: const Text('Import'),
              ),
            ],
    );
  }

  Widget _done(ThemeData theme, EnvelopeRead read) {
    String label(String entry) => secretLabel(entry, widget.providers);
    final nothing =
        read.created.isEmpty &&
        read.stored.isEmpty &&
        read.same.isEmpty &&
        read.refused.isEmpty &&
        read.secretsRefused.isEmpty;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (read.created.isNotEmpty) Text('Profiles taken: ${read.created.join(', ')}'),
        for (final entry in read.stored) Text('Stored: ${label(entry)}'),
        for (final entry in read.same) Text('Here already, the same: ${label(entry)}'),
        if (nothing) const Text('The file held nothing to take.'),
        if (read.refused.isNotEmpty || read.secretsRefused.isNotEmpty) ...[
          const SizedBox(height: 12),
          Text('Left as it was', style: theme.textTheme.titleSmall),
          for (final refused in read.refused) Text('Profile ${refused.name}: ${refused.why}'),
          for (final refused in read.secretsRefused) Text('${label(refused.name)}: ${refused.why}'),
        ],
        const SizedBox(height: 12),
        const Text(
          'The Matrix account of the fish is never in the file, because its login is a device '
          'with keys of its own, and one device cannot run in two places. If a profile has a '
          'table at Matrix, log the fish in here under "Matrix".',
        ),
        const SizedBox(height: 8),
        const Text(
          'One bot runs in one place only. Switch the fish off where the file came from '
          'before you switch it on here, or everything is translated twice.',
        ),
      ],
    );
  }
}
