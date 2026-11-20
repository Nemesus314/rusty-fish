// Die Übersicht: eine Karte je Profil. Sie zeigt, in welchen Chat ein Profil
// geht und für wen es gedacht ist, und lässt es starten und anhalten.

import 'dart:async';
import 'dart:ui' show AppExitResponse;

import 'package:material_ui/material_ui.dart';

import 'cli.dart';
import 'desk_state.dart';
import 'editor.dart';
import 'model.dart';
import 'secrets.dart';
import 'start.dart';

class OverviewPage extends StatefulWidget {
  const OverviewPage({
    super.key,
    required this.cli,
    required this.refreshEvery,
    required this.today,
    this.startWith,
  });

  final FishCli cli;
  final Duration? refreshEvery;
  final String Function() today;
  final StartRequest? startWith;

  @override
  State<OverviewPage> createState() => _OverviewPageState();
}

class _OverviewPageState extends State<OverviewPage> {
  late final DeskState _state = DeskState(widget.cli);
  late final AppLifecycleListener _lifecycle;
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    unawaited(_state.refresh().then((_) => _openStartRequest()));
    final every = widget.refreshEvery;
    if (every != null) {
      // Der Verbrauch und der Zustand „läuft" ändern sich, ohne dass hier
      // jemand klickt. Deshalb fragt die Übersicht im Takt nach.
      _timer = Timer.periodic(every, (_) => unawaited(_state.refresh()));
    }
    // Schließt man das Fenster, wird der Fisch beendet. Windows beendet
    // Kindprozesse nicht von selbst; ohne das liefe er unsichtbar weiter.
    _lifecycle = AppLifecycleListener(
      onExitRequested: () async {
        await _state.stopAndWait();
        return AppExitResponse.exit;
      },
    );
  }

  @override
  void dispose() {
    _timer?.cancel();
    _lifecycle.dispose();
    _state.dispose();
    super.dispose();
  }

  /// Einmal, nach dem ersten Laden: die Ansicht öffnen, mit der das Fenster
  /// aufgehen sollte.
  Future<void> _openStartRequest() async {
    final request = widget.startWith;
    if (request == null || !mounted) {
      return;
    }
    final profile = request.profile;
    if (profile != null) {
      await _edit(profile);
    } else if (request.newProfile) {
      await _openEditor(ProfileDoc.blank(), isNew: true);
    } else if (request.secrets) {
      await _secrets();
    }
  }

  void _say(String message) {
    if (!mounted) {
      return;
    }
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(message)));
  }

  Set<String> get _names => {
    for (final entry in _state.list?.profiles ?? const <ProfileEntry>[]) entry.name,
  };

  Future<void> _openEditor(ProfileDoc doc, {required bool isNew}) async {
    final saved = await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        builder: (_) => ProfileEditorPage(
          cli: widget.cli,
          initial: doc,
          isNew: isNew,
          existingNames: _names,
          providers: _state.providers,
          providersListed: _state.providersListed,
        ),
      ),
    );
    if (saved == true) {
      await _state.refresh();
    }
  }

  Future<void> _edit(String name) async {
    try {
      final doc = await widget.cli.showProfile(name);
      await _openEditor(doc, isNew: false);
    } on CliException catch (e) {
      _say(e.message);
    }
  }

  Future<void> _duplicate(String name) async {
    try {
      final doc = await widget.cli.showProfile(name);
      // Der erste freie Name, damit die Kopie kein vorhandenes Profil ersetzt.
      final taken = _names;
      var copy = '$name-copy';
      for (var n = 2; taken.contains(copy); n++) {
        copy = '$name-copy-$n';
      }
      await _openEditor(doc.renamed(copy), isNew: true);
    } on CliException catch (e) {
      _say(e.message);
    }
  }

  Future<void> _delete(String name) async {
    final sure = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text('Delete $name?'),
        content: const Text(
          'The profile is moved to the trash folder of rusty-fish. '
          'It is not erased, and you can put it back by hand.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Move to trash'),
          ),
        ],
      ),
    );
    if (sure != true) {
      return;
    }
    try {
      await widget.cli.deleteProfile(name);
    } on CliException catch (e) {
      _say(e.message);
    }
    await _state.refresh();
  }

  Future<void> _secrets() async {
    await showDialog<void>(
      context: context,
      builder: (_) => SecretsDialog(
        cli: widget.cli,
        providers: _state.providers,
        onChanged: _state.refresh,
      ),
    );
    await _state.refresh();
  }

  /// Die Dienste, die ohne Schlüssel nicht antworten. Ein eigenes Modell
  /// zählt nicht mit, sonst stünde dort dauerhaft „einer fehlt".
  List<Provider> get _needingKey => [
    for (final provider in _state.providers)
      if (provider.needsKey) provider,
  ];

  /// „AI keys: 1 of 4 stored". Seit jeder Dienst seinen eigenen Schlüssel
  /// hat, passt nicht mehr jeder einzeln in die Titelleiste.
  String _aiKeysText(KeyStatus? keys) {
    if (keys == null) {
      return 'AI keys: checking';
    }
    final needing = _needingKey;
    final stored = needing.where((provider) => keys.has(provider.id)).length;
    return 'AI keys: $stored of ${needing.length} stored';
  }

  /// In Ordnung ist es, wenn jedes gültige Profil den Schlüssel hat, den sein
  /// Dienst braucht. Dass andere Dienste keinen haben, ist kein Mangel.
  bool? _aiKeysGood(KeyStatus? keys, ProfileList? list) {
    if (keys == null || list == null) {
      return null;
    }
    final used = [
      for (final entry in list.profiles)
        if (entry.doc != null) _state.providerOf(entry.doc!.provider),
    ];
    if (used.isEmpty) {
      // Noch kein Profil: dann zählt, ob überhaupt ein Schlüssel liegt.
      return _needingKey.any((provider) => keys.has(provider.id));
    }
    return used.every((provider) => !provider.needsKey || keys.has(provider.id));
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: _state,
      builder: (context, _) {
        final theme = Theme.of(context);
        final list = _state.list;
        final keys = _state.keys;
        final loadError = _state.loadError;
        final providersError = _state.providersError;
        return Scaffold(
          appBar: AppBar(
            title: const Text('Rusty Fish'),
            actions: [
              Padding(
                padding: const EdgeInsets.only(right: 8),
                child: _SecretIndicator(
                  key: const ValueKey('chip-ai-keys'),
                  text: _aiKeysText(keys),
                  good: _aiKeysGood(keys, list),
                  onPressed: _secrets,
                ),
              ),
              Padding(
                padding: const EdgeInsets.only(right: 8),
                child: _SecretIndicator(
                  key: const ValueKey('chip-bot-token'),
                  text: '$botTokenLabel: ${keys == null ? 'checking' : (keys.discord ? 'stored' : 'not stored')}',
                  good: keys?.discord,
                  onPressed: _secrets,
                ),
              ),
              const SizedBox(width: 8),
            ],
          ),
          body: Padding(
            padding: const EdgeInsets.fromLTRB(24, 8, 24, 0),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          const Text('The fish runs while this window is open.'),
                          if (list != null)
                            Text(
                              'Profiles are kept in ${list.dir}',
                              style: theme.textTheme.bodySmall,
                            ),
                        ],
                      ),
                    ),
                    FilledButton.icon(
                      onPressed: () => _openEditor(ProfileDoc.blank(), isNew: true),
                      icon: const Icon(Icons.add),
                      label: const Text('New profile'),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                if (loadError != null) _ErrorBox(text: loadError),
                // Ein älteres Programm kennt die Auswahl des Dienstes noch
                // nicht. Die Maske bleibt benutzbar und sagt, was das Programm
                // dazu gemeldet hat.
                if (providersError != null)
                  _ErrorBox(
                    key: const ValueKey('providers-error'),
                    // Die Meldung des Programms im Wortlaut, nur ohne seine
                    // Leerzeilen: im echten Fenster angesehen, nahm sie sonst
                    // ein Viertel der Übersicht ein.
                    text:
                        'This rusty-fish cannot list its AI services, so only '
                        '${fallbackProviders.first.label} can be chosen. It said:\n'
                        '${providersError.split('\n').where((line) => line.trim().isNotEmpty).join('\n')}',
                  ),
                Expanded(
                  child: list == null
                      ? const SizedBox.shrink()
                      : list.profiles.isEmpty
                      ? const Center(
                          child: Text('No profile yet. Create the first one.'),
                        )
                      : ListView(
                          padding: const EdgeInsets.only(bottom: 24),
                          children: [
                            for (final entry in list.profiles)
                              _ProfileCard(
                                entry: entry,
                                state: _state,
                                today: widget.today(),
                                onSecrets: _secrets,
                                onEdit: () => _edit(entry.name),
                                onDuplicate: () => _duplicate(entry.name),
                                onDelete: () => _delete(entry.name),
                              ),
                          ],
                        ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}

/// „Bot token: stored". Zeigt nur, ob ein Geheimnis liegt, nie den Wert.
class _SecretIndicator extends StatelessWidget {
  const _SecretIndicator({
    super.key,
    required this.text,
    required this.good,
    required this.onPressed,
  });

  final String text;

  /// Ob alles da ist, was gebraucht wird. Ohne Angabe wird noch nachgesehen.
  final bool? good;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return ActionChip(
      avatar: Icon(
        good == true ? Icons.check_circle_outline : Icons.error_outline,
        size: 18,
        color: good == true ? scheme.primary : scheme.error,
      ),
      label: Text(text),
      onPressed: onPressed,
    );
  }
}

class _ErrorBox extends StatelessWidget {
  const _ErrorBox({super.key, required this.text, this.mono = false});

  final String text;

  /// Für Gründe, die das Programm mit Zeilen und Zeigern setzt (etwa beim
  /// Lesen einer Profildatei). In einer Schrift mit festen Breiten zeigt der
  /// Zeiger auf die richtige Stelle.
  final bool mono;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      width: double.infinity,
      margin: const EdgeInsets.only(bottom: 12),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: scheme.errorContainer,
        borderRadius: BorderRadius.circular(8),
      ),
      child: SelectionArea(
        child: Text(
          text.trimRight(),
          style: TextStyle(
            color: scheme.onErrorContainer,
            fontFamily: mono ? 'Consolas' : null,
          ),
        ),
      ),
    );
  }
}

class _ProfileCard extends StatelessWidget {
  const _ProfileCard({
    required this.entry,
    required this.state,
    required this.today,
    required this.onSecrets,
    required this.onEdit,
    required this.onDuplicate,
    required this.onDelete,
  });

  final ProfileEntry entry;
  final DeskState state;
  final String today;

  /// Öffnet die Geheimnisse. Der Hinweis auf einen fehlenden Schlüssel führt
  /// dorthin, wo man ihn ablegt.
  final VoidCallback onSecrets;
  final VoidCallback onEdit;
  final VoidCallback onDuplicate;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final doc = entry.doc;
    final runningHere = state.runningName == entry.name;
    final runningElsewhere = entry.running && !runningHere;
    final running = runningHere || runningElsewhere;
    final ownsLog = state.logOwner == entry.name;
    final runError = ownsLog ? state.runError : null;
    final provider = doc == null ? null : state.providerOf(doc.provider);
    final keys = state.keys;
    // Fehlt der Schlüssel des Dienstes, würde der Start sicher scheitern. Das
    // soll man sehen, bevor man drückt. Solange noch nachgesehen wird, gilt
    // er nicht als fehlend.
    final keyMissing =
        provider != null && keys != null && provider.needsKey && !keys.has(provider.id);

    return Card(
      key: ValueKey('profile-${entry.name}'),
      margin: const EdgeInsets.only(bottom: 12),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(entry.name, style: theme.textTheme.titleLarge),
                ),
                _StatusLabel(
                  text: !entry.ok ? 'Invalid' : (running ? 'Running' : 'Stopped'),
                  kind: !entry.ok
                      ? _StatusKind.invalid
                      : (running ? _StatusKind.running : _StatusKind.stopped),
                ),
              ],
            ),
            const SizedBox(height: 8),
            if (doc == null) ...[
              _ErrorBox(
                text: entry.error ?? 'The program gave no reason.',
                mono: true,
              ),
              Text(entry.path, style: theme.textTheme.bodySmall),
            ] else ...[
              for (final room in doc.rooms)
                Text(
                  room.seats.map((seat) => seat.lang).join(' ⇄ '),
                  style: theme.textTheme.titleMedium,
                ),
              const SizedBox(height: 4),
              // Welcher Dienst mit welchem Modell übersetzt. Das ist seit dem
              // 20.11.2026 je Profil wählbar und gehört deshalb auf die Karte.
              Text('${provider!.label} · ${doc.model}'),
              const SizedBox(height: 4),
              Text(
                'Channels ${[for (final room in doc.rooms) for (final seat in room.seats) seat.channel].join(', ')}',
                style: theme.textTheme.bodySmall,
              ),
              const SizedBox(height: 8),
              Wrap(
                spacing: 24,
                runSpacing: 4,
                children: [
                  Text(_people(doc.people.length)),
                  Text(
                    doc.aiNames.isEmpty
                        ? 'translator only'
                        : 'AI answers to ${doc.aiNames.join(', ')}',
                  ),
                  Text(_usage(entry.usage, today)),
                  // Nemesus am 20.11.2026: in der Übersicht war nicht zu
                  // sehen, welche Sprache unübersetzt bleibt.
                  Text(_neutral(doc.rooms)),
                ],
              ),
            ],
            if (keyMissing)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: TextButton.icon(
                    onPressed: onSecrets,
                    style: TextButton.styleFrom(
                      foregroundColor: theme.colorScheme.error,
                    ),
                    icon: const Icon(Icons.error_outline, size: 18),
                    label: Text('No key stored for ${provider.label}'),
                  ),
                ),
              ),
            if (runningElsewhere)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(
                  'Started outside this window. Stop it where it was started.',
                  style: theme.textTheme.bodySmall,
                ),
              ),
            if (runError != null)
              Padding(
                padding: const EdgeInsets.only(top: 12),
                child: _ErrorBox(text: runError),
              ),
            const SizedBox(height: 12),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                if (doc != null && runningHere)
                  FilledButton.tonal(
                    onPressed: state.stop,
                    child: const Text('Stop'),
                  ),
                if (doc != null && !running)
                  Tooltip(
                    message: keyMissing
                        ? 'Store the key for ${provider.label} first.'
                        : state.runningName == null
                        ? 'Start this profile'
                        : 'Stop ${state.runningName} first. One bot can only run once.',
                    child: FilledButton(
                      onPressed: state.runningName == null && !keyMissing
                          ? () => state.start(entry.name)
                          : null,
                      child: const Text('Start'),
                    ),
                  ),
                if (doc != null) ...[
                  OutlinedButton(onPressed: onEdit, child: const Text('Edit')),
                  OutlinedButton(
                    onPressed: onDuplicate,
                    child: const Text('Duplicate'),
                  ),
                ],
                OutlinedButton(
                  onPressed: running ? null : onDelete,
                  child: const Text('Delete'),
                ),
              ],
            ),
            if (ownsLog && (runningHere || state.log.isNotEmpty))
              _LogPanel(lines: state.log),
          ],
        ),
      ),
    );
  }

  static String _people(int count) => switch (count) {
    0 => 'no people listed',
    1 => '1 person listed',
    _ => '$count people listed',
  };

  /// Welche Sprachen unübersetzt durchgehen, über alle Räume des Profils.
  static String _neutral(List<Room> rooms) {
    final langs = <String>[];
    for (final room in rooms) {
      for (final lang in room.passThrough) {
        if (!langs.contains(lang)) langs.add(lang);
      }
    }
    return switch (langs.length) {
      0 => 'no neutral language',
      1 => '${langs.first} stays untranslated',
      _ => '${langs.join(', ')} stay untranslated',
    };
  }

  static String _usage(Usage? usage, String today) {
    // Ein gespeicherter Verbrauch von einem früheren Tag zählt heute nicht.
    if (usage == null || usage.day != today || usage.translationTokens == 0) {
      return 'not used today';
    }
    return '${groupThousands(usage.translationTokens)} of '
        '${groupThousands(usage.allowedToday)} tokens today';
  }
}

enum _StatusKind { running, stopped, invalid }

class _StatusLabel extends StatelessWidget {
  const _StatusLabel({required this.text, required this.kind});

  final String text;
  final _StatusKind kind;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final (background, foreground) = switch (kind) {
      _StatusKind.running => (scheme.primaryContainer, scheme.onPrimaryContainer),
      _StatusKind.stopped => (scheme.surfaceContainerHighest, scheme.onSurfaceVariant),
      _StatusKind.invalid => (scheme.errorContainer, scheme.onErrorContainer),
    };
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
      decoration: BoxDecoration(
        color: background,
        borderRadius: BorderRadius.circular(16),
      ),
      child: Text(text, style: TextStyle(color: foreground)),
    );
  }
}

/// Die letzten Meldungen des Fischs. Er schreibt nur Kennungen und Gründe,
/// nie den Text einer Nachricht.
class _LogPanel extends StatelessWidget {
  const _LogPanel({required this.lines});

  final List<String> lines;

  @override
  Widget build(BuildContext context) {
    final count = lines.length;
    return ExpansionTile(
      tilePadding: EdgeInsets.zero,
      title: Text('Log ($count ${count == 1 ? 'line' : 'lines'})'),
      children: [
        Container(
          width: double.infinity,
          constraints: const BoxConstraints(maxHeight: 240),
          child: SingleChildScrollView(
            reverse: true,
            child: SelectionArea(
              child: Text(
                lines.join('\n'),
                style: const TextStyle(fontFamily: 'Consolas', fontSize: 13),
              ),
            ),
          ),
        ),
      ],
    );
  }
}
