// Die beiden Geheimnisse: der Schlüssel der KI und der Token des Bots.
//
// Die Maske zeigt nur, ob sie liegen. Ein Wert geht vom Eingabefeld über die
// Standardeingabe an das Programm und von dort in die Ablage des Systems.
// Zurück kommt er nie: es gibt keinen Aufruf, der ihn ausliest.

import 'package:material_ui/material_ui.dart';

import 'cli.dart';
import 'model.dart';

class SecretsDialog extends StatefulWidget {
  const SecretsDialog({super.key, required this.cli, required this.onChanged});

  final FishCli cli;

  /// Die Übersicht dahinter soll sofort den neuen Stand zeigen.
  final Future<void> Function() onChanged;

  @override
  State<SecretsDialog> createState() => _SecretsDialogState();
}

class _SecretsDialogState extends State<SecretsDialog> {
  final Map<SecretKind, TextEditingController> _fields = {
    for (final kind in SecretKind.values) kind: TextEditingController(),
  };
  KeyStatus? _status;
  String? _error;
  bool _busy = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void dispose() {
    for (final field in _fields.values) {
      // Was noch im Feld steht, soll nicht im Speicher herumliegen.
      field.clear();
      field.dispose();
    }
    super.dispose();
  }

  Future<void> _load() async {
    try {
      final status = await widget.cli.keyStatus();
      if (mounted) {
        setState(() => _status = status);
      }
    } on CliException catch (e) {
      if (mounted) {
        setState(() => _error = e.message);
      }
    }
  }

  bool? _stored(SecretKind kind) {
    final status = _status;
    if (status == null) {
      return null;
    }
    return kind == SecretKind.openai ? status.openai : status.discord;
  }

  Future<void> _act(Future<void> Function() action) async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await action();
    } on CliException catch (e) {
      _error = e.message;
    }
    await _load();
    await widget.onChanged();
    if (mounted) {
      setState(() => _busy = false);
    }
  }

  Future<void> _store(SecretKind kind) async {
    final field = _fields[kind]!;
    final value = field.text.trim();
    if (value.isEmpty) {
      setState(() => _error = 'Nothing entered, nothing stored.');
      return;
    }
    // Das Feld wird geleert, bevor irgendetwas anderes geschieht. Danach
    // steht der Wert nirgends mehr im Fenster.
    field.clear();
    await _act(() => widget.cli.setKey(kind, value));
  }

  Future<void> _forget(SecretKind kind) async {
    final sure = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text('Forget the ${kind.label}?'),
        content: Text(
          kind == SecretKind.openai
              ? 'Without it the fish cannot translate. You can store it again any time.'
              : 'Without it the fish cannot connect to Discord. You can store it again any time.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Forget it'),
          ),
        ],
      ),
    );
    if (sure == true) {
      await _act(() => widget.cli.forgetKey(kind));
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final error = _error;
    return AlertDialog(
      title: const Text('Secrets'),
      content: SizedBox(
        width: 520,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Secrets go straight into the credential store of this system. '
                'This window only knows whether they are there and never shows them.',
              ),
              if (error != null)
                Padding(
                  padding: const EdgeInsets.only(top: 12),
                  child: Text(error, style: TextStyle(color: theme.colorScheme.error)),
                ),
              for (final kind in SecretKind.values) _block(kind, theme),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Close'),
        ),
      ],
    );
  }

  Widget _block(SecretKind kind, ThemeData theme) {
    final stored = _stored(kind);
    return Padding(
      padding: const EdgeInsets.only(top: 20),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Text(kind.label, style: theme.textTheme.titleMedium),
              const SizedBox(width: 12),
              Text(
                stored == null ? 'checking' : (stored ? 'stored' : 'not stored'),
                style: TextStyle(
                  color: stored == true
                      ? theme.colorScheme.primary
                      : theme.colorScheme.error,
                ),
              ),
            ],
          ),
          const SizedBox(height: 4),
          TextField(
            key: ValueKey('secret-${kind.cliName}'),
            controller: _fields[kind],
            obscureText: true,
            enableSuggestions: false,
            autocorrect: false,
            decoration: InputDecoration(
              labelText: stored == true ? 'Replace it with a new one' : 'Paste it here',
              helperText: kind == SecretKind.openai
                  ? 'The API key of the service that translates.'
                  : 'From the Discord Developer Portal: your application, Bot, Reset Token.',
            ),
          ),
          const SizedBox(height: 8),
          Row(
            children: [
              FilledButton(
                key: ValueKey('store-${kind.cliName}'),
                onPressed: _busy ? null : () => _store(kind),
                child: const Text('Store'),
              ),
              const SizedBox(width: 8),
              if (stored == true)
                OutlinedButton(
                  key: ValueKey('forget-${kind.cliName}'),
                  onPressed: _busy ? null : () => _forget(kind),
                  child: const Text('Forget'),
                ),
            ],
          ),
        ],
      ),
    );
  }
}
