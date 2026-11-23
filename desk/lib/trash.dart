// Der Papierkorb: gelöschte Profile ansehen, zurückholen oder endgültig
// löschen.
//
// Nemesus am 23.11.2026: „Im Hilfstext steht, es wandert in den Papierkorb.
// Aber gibt es auch einen Weg, das gelöschte Profil vom Papierkorb zurück zu
// bringen oder endgültig zu löschen?" Bis dahin ging beides nur von Hand im
// Ordner.
//
// Die Maske zeigt nur und fragt nach. Was ein Eintrag ist, ob ein Name frei
// ist und was verschoben oder gelöscht wird, entscheidet das Programm.

import 'package:material_ui/material_ui.dart';

import 'cli.dart';
import 'model.dart';

/// Das Fenster des Papierkorbs. Schließt mit den Namen der Profile, die
/// zurückgeholt wurden: ist der Fisch an, nimmt er sie gleich auf.
class TrashDialog extends StatefulWidget {
  const TrashDialog({super.key, required this.cli, required this.existingNames});

  final FishCli cli;

  /// Die Namen, die es schon gibt. Ein zurückgeholtes Profil ersetzt keines.
  final Set<String> existingNames;

  @override
  State<TrashDialog> createState() => _TrashDialogState();
}

class _TrashDialogState extends State<TrashDialog> {
  Trash? _trash;
  String? _error;
  final _restored = <String>[];

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final trash = await widget.cli.trash();
      if (mounted) {
        setState(() => _trash = trash);
      }
    } on CliException catch (e) {
      if (mounted) {
        setState(() => _error = e.message);
      }
    }
  }

  /// Die Namen, die jetzt vergeben sind: die beim Öffnen und die inzwischen
  /// zurückgeholten.
  Set<String> get _taken => {...widget.existingNames, ..._restored};

  /// Fragt nach einem anderen Namen, weil der alte wieder vergeben ist. Der
  /// Vorschlag ist der erste freie.
  Future<String?> _otherName(String taken) {
    var free = '$taken-restored';
    for (var n = 2; _taken.contains(free); n++) {
      free = '$taken-restored-$n';
    }
    return showDialog<String>(
      context: context,
      builder: (_) => _OtherNameDialog(taken: taken, suggestion: free),
    );
  }

  Future<void> _putBack(Trashed entry) async {
    var name = entry.name;
    var asked = false;
    if (_taken.contains(name)) {
      final chosen = await _otherName(name);
      if (chosen == null || !mounted) {
        return;
      }
      name = chosen;
      asked = true;
    }
    while (true) {
      try {
        final back = await widget.cli.restoreProfile(
          entry.id,
          name: name == entry.name ? null : name,
        );
        _restored.add(back);
        if (mounted) {
          setState(() => _error = null);
        }
        break;
      } on CliException catch (e) {
        if (!mounted) {
          return;
        }
        // Ob ein Name frei ist, weiß das Programm, nicht die Maske: lehnt es
        // den alten ab, weil es ihn inzwischen wieder gibt, fragt die Maske
        // einmal nach einem anderen.
        if (!asked && e.message.contains('already exists')) {
          asked = true;
          final chosen = await _otherName(name);
          if (chosen == null || !mounted) {
            return;
          }
          name = chosen;
          continue;
        }
        setState(() => _error = e.message);
        break;
      }
    }
    await _load();
  }

  Future<void> _erase(Trashed entry) async {
    final sure = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text('Erase ${entry.name} for good?'),
        content: Text(
          'Its settings${entry.state ? ' and what the fish remembered for it (who agreed, how '
                        'far it has read, today\'s use)' : ''} are erased. This cannot be undone.\n\n'
          'The log file of this name stays in the logs folder.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            key: const ValueKey('erase-confirm'),
            style: FilledButton.styleFrom(
              backgroundColor: Theme.of(context).colorScheme.error,
              foregroundColor: Theme.of(context).colorScheme.onError,
            ),
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Erase for good'),
          ),
        ],
      ),
    );
    if (sure != true || !mounted) {
      return;
    }
    try {
      await widget.cli.eraseProfile(entry.id);
      if (mounted) {
        setState(() => _error = null);
      }
    } on CliException catch (e) {
      if (mounted) {
        setState(() => _error = e.message);
      }
    }
    await _load();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final trash = _trash;
    return AlertDialog(
      title: const Text('Trash'),
      content: SizedBox(
        width: 620,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text(
              'Deleted profiles are kept here until you erase them. Put one back to use it '
              'again, with everything the fish remembered for it.',
            ),
            const SizedBox(height: 12),
            if (_error case final why?)
              Padding(
                padding: const EdgeInsets.only(bottom: 12),
                child: Text(why, style: TextStyle(color: theme.colorScheme.error)),
              ),
            if (trash == null && _error == null)
              const Padding(
                padding: EdgeInsets.symmetric(vertical: 12),
                child: Text('Looking…'),
              )
            else if (trash != null && trash.deleted.isEmpty)
              const Padding(
                padding: EdgeInsets.symmetric(vertical: 12),
                child: Text('The trash is empty.'),
              )
            else if (trash != null)
              Flexible(
                child: SingleChildScrollView(
                  child: Column(
                    children: [for (final entry in trash.deleted) _row(theme, entry)],
                  ),
                ),
              ),
            if (trash != null && trash.dir.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(top: 12),
                child: SelectableText('Kept in ${trash.dir}', style: theme.textTheme.bodySmall),
              ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(_restored),
          child: const Text('Close'),
        ),
      ],
    );
  }

  Widget _row(ThemeData theme, Trashed entry) {
    return Padding(
      key: ValueKey('trash-${entry.id}'),
      padding: const EdgeInsets.only(bottom: 8),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(entry.name, style: theme.textTheme.titleMedium),
                Text(
                  'deleted ${entry.deleted}'
                  '${entry.state ? '' : ' · settings only, the fish had remembered nothing'}',
                  style: theme.textTheme.bodySmall,
                ),
              ],
            ),
          ),
          const SizedBox(width: 12),
          OutlinedButton(
            key: ValueKey('put-back-${entry.id}'),
            onPressed: () => _putBack(entry),
            child: const Text('Put back'),
          ),
          const SizedBox(width: 8),
          OutlinedButton(
            key: ValueKey('erase-${entry.id}'),
            onPressed: () => _erase(entry),
            child: const Text('Erase'),
          ),
        ],
      ),
    );
  }
}

/// Fragt nach einem anderen Namen, wenn der alte wieder vergeben ist.
class _OtherNameDialog extends StatefulWidget {
  const _OtherNameDialog({required this.taken, required this.suggestion});

  final String taken;
  final String suggestion;

  @override
  State<_OtherNameDialog> createState() => _OtherNameDialogState();
}

class _OtherNameDialogState extends State<_OtherNameDialog> {
  late final _name = TextEditingController(text: widget.suggestion);

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Put back under another name'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('There is a profile named "${widget.taken}" again. It is not replaced.'),
          const SizedBox(height: 12),
          TextField(
            key: const ValueKey('put-back-name'),
            controller: _name,
            autofocus: true,
            decoration: const InputDecoration(
              labelText: 'Name',
              helperText: 'a-z, 0-9, - and _',
            ),
          ),
        ],
      ),
      actions: [
        TextButton(onPressed: () => Navigator.of(context).pop(), child: const Text('Cancel')),
        FilledButton(
          key: const ValueKey('put-back-confirm'),
          onPressed: () => Navigator.of(context).pop(_name.text.trim()),
          child: const Text('Put back'),
        ),
      ],
    );
  }
}
