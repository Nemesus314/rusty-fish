// Die Übersicht: eine Karte je Profil. Sie zeigt, in welchen Chat ein Profil
// geht und für wen es gedacht ist, und lässt es starten und anhalten.

import 'dart:async';
import 'dart:ui' show AppExitResponse;

import 'package:flutter/services.dart' show Clipboard, ClipboardData;
import 'package:material_ui/material_ui.dart';

import 'about.dart';
import 'cli.dart';
import 'desk_state.dart';
import 'editor.dart';
import 'log_file.dart';
import 'model.dart';
import 'report.dart';
import 'secrets.dart';
import 'share.dart';
import 'start.dart';
import 'theme_choice.dart';

class OverviewPage extends StatefulWidget {
  const OverviewPage({
    super.key,
    required this.cli,
    required this.refreshEvery,
    required this.today,
    this.now,
    this.openLog,
    this.startWith,
    this.theme,
  });

  /// Die Wahl zwischen hell und dunkel. Ohne Angabe gibt es keinen Umschalter.
  final ThemeChoice? theme;

  final FishCli cli;
  final Duration? refreshEvery;
  final String Function() today;

  /// Die Uhr für die Zeitstempel im Protokoll. Ohne Angabe die des Rechners.
  final DateTime Function()? now;

  /// Öffnet die Protokolldatei eines Profils. Ohne Angabe gibt es keine.
  final OpenLog Function(String profilesDir, String profile)? openLog;
  final StartRequest? startWith;

  @override
  State<OverviewPage> createState() => _OverviewPageState();
}

class _OverviewPageState extends State<OverviewPage> {
  late final DeskState _state = DeskState(
    widget.cli,
    now: widget.now,
    openLog: widget.openLog,
  );
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
      await _edit(profile, scrollTo: request.section);
    } else if (request.newProfile) {
      await _openEditor(ProfileDoc.blank(), isNew: true);
    } else if (request.secrets) {
      await _secrets(SecretsPart.aiKeys);
    }
  }

  /// Legt die Räume eines Profils als Text in die Zwischenablage.
  Future<void> _shareRooms(ProfileEntry entry) async {
    final doc = entry.doc;
    if (doc == null) {
      return;
    }
    await Clipboard.setData(ClipboardData(text: shareRoomsText(doc)));
    _say(
      'Rooms copied. The text holds the channels or rooms, the user ids and the names from '
      'this profile. No key, no token, nothing about your AI.',
    );
  }

  /// Legt Stand und Protokoll eines Profils als Text in die Zwischenablage,
  /// für eine KI, die bei der Fehlersuche hilft.
  Future<void> _copyForAi(
    ProfileEntry entry,
    String status, {
    required bool withPeople,
  }) async {
    final doc = entry.doc;
    final text = reportForAi(
      entry: entry,
      status: status,
      log: _state.logOf(entry.name),
      runError: _state.runErrorOf(entry.name),
      service: doc == null ? null : _state.providerOf(doc.provider).label,
      withPeople: withPeople,
    );
    await Clipboard.setData(ClipboardData(text: text));
    _say(
      withPeople
          ? 'Copied with the full profile: it holds the names you give people and their user '
                'ids. No key, no token, no message text.'
          : 'Copied: status, rooms and log of this profile. No people, no key, no token, no '
                'message text.',
    );
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

  Future<void> _openEditor(ProfileDoc doc, {required bool isNew, String? scrollTo}) async {
    final saved = await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        builder: (_) => ProfileEditorPage(
          cli: widget.cli,
          initial: doc,
          isNew: isNew,
          existingNames: _names,
          providers: _state.providers,
          providersListed: _state.providersListed,
          scrollTo: scrollTo,
        ),
      ),
    );
    if (saved == true) {
      final before = _names.toSet();
      await _state.refresh();
      // Ist der Fisch an, nimmt er das gespeicherte Profil gleich neu auf:
      // ein geändertes wird neu geladen, ein neues kommt dazu. Die anderen
      // laufen dabei ungestört weiter.
      if (isNew) {
        _names.toSet().difference(before).forEach(_state.saved);
      } else {
        _state.saved(doc.name);
      }
    }
  }

  Future<void> _edit(String name, {String? scrollTo}) async {
    try {
      final doc = await widget.cli.showProfile(name);
      await _openEditor(doc, isNew: false, scrollTo: scrollTo);
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

  /// Legt die Beschreibung des Programms in die Zwischenablage. Wer sie einer
  /// KI seines Vertrauens gibt, kann sich die Einrichtung erklären lassen.
  Future<void> _copyGuide() async {
    try {
      final text = await widget.cli.guide();
      await Clipboard.setData(ClipboardData(text: text));
      _say('Guide copied. Paste it into a chat with an AI assistant you trust.');
    } on CliException catch (e) {
      _say('Cannot copy the guide: ${e.message}');
    }
  }

  Future<void> _secrets(SecretsPart part) async {
    await showDialog<void>(
      context: context,
      builder: (_) => SecretsDialog(
        part: part,
        cli: widget.cli,
        providers: _state.providers,
        onChanged: _state.refresh,
      ),
    );
    if (part == SecretsPart.matrix) {
      _state.askDeviceAgain();
    }
    await _state.refresh();
  }

  /// Die Dienste, die ohne Schlüssel nicht antworten. Ein eigenes Modell
  /// zählt nicht mit, sonst stünde dort dauerhaft „einer fehlt".
  List<Provider> get _needingKey => [
    for (final provider in _state.providers)
      if (provider.needsKey) provider,
  ];

  /// Steht ein gültiges Profil an der Matrix-Tür? Nur dann fehlt etwas,
  /// solange der Fisch dort nicht angemeldet ist.
  bool _usesMatrix(ProfileList? list) => [
    for (final entry in list?.profiles ?? const <ProfileEntry>[])
      if (entry.doc?.doors.contains(matrixDoor) ?? false) entry,
  ].isNotEmpty;

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
              // Wofür das Programm gebaut ist und wer es gebaut hat.
              Padding(
                padding: const EdgeInsets.only(right: 4),
                child: TextButton.icon(
                  key: const ValueKey('about'),
                  onPressed: () => showAbout(context),
                  icon: const Icon(Icons.info_outline, size: 18),
                  label: const Text('About'),
                ),
              ),
              Padding(
                padding: const EdgeInsets.only(right: 16),
                child: Tooltip(
                  message:
                      'Copies a description of Rusty Fish. An AI assistant can '
                      'use it to explain the setup in any language.',
                  child: TextButton.icon(
                    key: const ValueKey('copy-guide'),
                    onPressed: _copyGuide,
                    icon: const Icon(Icons.copy, size: 18),
                    label: const Text('Copy guide for an AI'),
                  ),
                ),
              ),
              Padding(
                padding: const EdgeInsets.only(right: 8),
                child: _SecretIndicator(
                  key: const ValueKey('chip-ai-keys'),
                  text: _aiKeysText(keys),
                  good: _aiKeysGood(keys, list),
                  onPressed: () => _secrets(SecretsPart.aiKeys),
                ),
              ),
              Padding(
                padding: const EdgeInsets.only(right: 8),
                child: _SecretIndicator(
                  key: const ValueKey('chip-bot-token'),
                  text: '$discordChipLabel: ${keys == null ? 'checking' : (keys.discord ? 'stored' : 'not stored')}',
                  good: keys?.discord,
                  onPressed: () => _secrets(SecretsPart.discord),
                ),
              ),
              // Das Matrix-Konto des Fischs. Seit jede Anzeige ihr eigenes
              // Fenster öffnet (23.11.2026), steht diese immer da: sie ist der
              // Weg zum Anmelden. Solange niemand Matrix benutzt, ist sie
              // kurz und fehlt nichts; rot wird sie erst, wenn ein Profil an
              // der Matrix-Tür steht und der Fisch dort nicht angemeldet ist.
              // Gemessen am 22.11.2026: mit der langen Fassung für alle
              // reichte die Breite der Titelleiste im Test nicht.
              Padding(
                padding: const EdgeInsets.only(right: 8),
                child: _SecretIndicator(
                  key: const ValueKey('chip-matrix'),
                  // Der Haken dafür, dass das Konto das Gerät des Fischs
                  // gegengezeichnet hat (Nemesus, 23.11.2026): „verified"
                  // statt „logged in". Nimmt der Server den Zugang nicht mehr
                  // an, steht auch das hier, statt eines falschen Hakens.
                  text: switch (keys) {
                    null => 'Matrix',
                    final keys when keys.matrix => switch (_state.matrixDevice) {
                      deviceVerified => 'Matrix: verified',
                      deviceRefused => 'Matrix: refused',
                      _ => 'Matrix: logged in',
                    },
                    _ when _usesMatrix(list) => 'Matrix: not logged in',
                    _ => 'Matrix',
                  },
                  good: switch (keys) {
                    null => null,
                    final keys when keys.matrix => _state.matrixDevice != deviceRefused,
                    _ when _usesMatrix(list) => false,
                    _ => null,
                  },
                  verified: keys?.matrix == true && _state.matrixDevice == deviceVerified,
                  onPressed: () => _secrets(SecretsPart.matrix),
                ),
              ),
              // Hell oder dunkel. Das Zeichen zeigt, wohin der Klick führt.
              if (widget.theme case final choice?)
                Builder(
                  builder: (context) {
                    final dark = Theme.of(context).brightness == Brightness.dark;
                    return IconButton(
                      key: const ValueKey('toggle-theme'),
                      tooltip: dark ? 'Switch to light' : 'Switch to dark',
                      onPressed: () => choice.toggle(Theme.of(context).brightness),
                      icon: Icon(dark ? Icons.light_mode : Icons.dark_mode),
                    );
                  },
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
                          // Wer von unterwegs über die Discord-App schreibt,
                          // braucht zu Hause einen wachen Rechner. Dass ein
                          // schlafender PC den Fisch anhält, sieht man dem
                          // Fenster sonst nicht an.
                          const Tooltip(
                            message:
                                'Writing from a phone works too, through the Discord app, as long '
                                'as this PC stays on and awake. While the fish is on, it keeps '
                                'Windows from going to sleep by itself. Closing a laptop lid '
                                'still stops it. A screen that turns off is fine.',
                            child: Text(
                              'The fish runs while this window is open and this PC is awake.',
                            ),
                          ),
                          if (list != null)
                            Text(
                              'Profiles are kept in ${list.dir}',
                              style: theme.textTheme.bodySmall,
                            ),
                        ],
                      ),
                    ),
                    _FishSwitch(state: _state),
                    const SizedBox(width: 12),
                    FilledButton.tonalIcon(
                      onPressed: () => _openEditor(ProfileDoc.blank(), isNew: true),
                      icon: const Icon(Icons.add),
                      label: const Text('New profile'),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                // Was den ganzen Fisch betrifft, steht hier und nicht an
                // einer Karte: warum er nicht anging oder von selbst ausging.
                if (_state.fishError case final why?)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 12),
                    child: _ErrorBox(text: why),
                  ),
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
                                onSecrets: () => _secrets(SecretsPart.aiKeys),
                                onEdit: () => _edit(entry.name),
                                onDuplicate: () => _duplicate(entry.name),
                                onShare: () => _shareRooms(entry),
                                onCopyForAi: (status, {required withPeople}) =>
                                    _copyForAi(entry, status, withPeople: withPeople),
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

/// „Discord: stored". Zeigt nur, ob ein Geheimnis liegt, nie den Wert.
class _SecretIndicator extends StatelessWidget {
  const _SecretIndicator({
    super.key,
    required this.text,
    required this.good,
    required this.onPressed,
    this.verified = false,
  });

  final String text;

  /// Mehr als „da": von anderer Seite bestätigt. Dann steht statt des Kreises
  /// mit Haken das Siegel mit Haken.
  final bool verified;

  /// Ob alles da ist, was gebraucht wird. Ohne Angabe wird noch nachgesehen,
  /// oder es wird nichts gebraucht: dann fehlt auch nichts.
  final bool? good;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return ActionChip(
      avatar: Icon(
        switch (good) {
          true when verified => Icons.verified_outlined,
          true => Icons.check_circle_outline,
          false => Icons.error_outline,
          null => Icons.radio_button_unchecked,
        },
        size: 18,
        color: switch (good) {
          true => scheme.primary,
          false => scheme.error,
          null => scheme.onSurfaceVariant,
        },
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
    required this.onShare,
    required this.onCopyForAi,
    required this.onDelete,
  });

  /// Legt den Stand und das Protokoll dieses Profils als Text in die
  /// Zwischenablage, für eine KI, die bei der Fehlersuche hilft: mit dem
  /// Stand, wie die Karte ihn nennt, und mit oder ohne die Menschen.
  final void Function(String status, {required bool withPeople}) onCopyForAi;

  /// Legt die Räume dieses Profils als Text in die Zwischenablage, für
  /// jemanden, der seinen eigenen Fisch mitbringt.
  final VoidCallback onShare;

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
    final runningHere = state.running.contains(entry.name);
    final starting = state.starting(entry.name);
    // Ein Fisch, den dieses Fenster nicht gestartet hat, etwa aus einem
    // Terminal. Er hält die Sperre des Bots: solange er läuft, kann dieses
    // Fenster keinen eigenen starten.
    final runningElsewhere = entry.running && !runningHere && !state.on;
    final running = runningHere || runningElsewhere;
    final log = state.logOf(entry.name);
    final runError = state.runErrorOf(entry.name);
    final provider = doc == null ? null : state.providerOf(doc.provider);
    final keys = state.keys;
    // Fehlt der Schlüssel des Dienstes, würde der Start sicher scheitern. Das
    // soll man sehen, bevor man drückt. Solange noch nachgesehen wird, gilt
    // er nicht als fehlend.
    final keyMissing =
        provider != null && keys != null && provider.needsKey && !keys.has(provider.id);

    final status = !entry.ok
        ? 'Invalid'
        : starting
        ? 'Starting'
        : (running ? 'Running' : 'Stopped');

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
                  text: status,
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
              for (final room in doc.rooms) ...[
                Text(
                  // Für wen ein Tisch gebaut ist, und in welcher Sprache dort
                  // gelesen wird: die Muttersprache seines Besitzers, sonst
                  // die des Platzes.
                  room.seats.map(doc.tableText).join(' ⇄ '),
                  style: theme.textTheme.titleMedium,
                ),
                // Über welche Türen der Raum läuft. Daran hängt, ob das
                // Gespräch unterwegs verschlüsselt ist.
                _HintLine(
                  key: ValueKey('hint-door-${entry.name}-${doc.rooms.indexOf(room)}'),
                  text: connectionText(room),
                  hint: connectionHint,
                ),
              ],
              const SizedBox(height: 4),
              // Welcher Dienst mit welchem Modell übersetzt. Das ist seit dem
              // 20.11.2026 je Profil wählbar und gehört deshalb auf die Karte.
              Text('${provider!.label} · ${doc.model}'),
              // Wohin die Sätze gehen: auf diesen Rechner, ins eigene Netz
              // oder zu einem fremden Dienst.
              if (entry.ai case final ai?)
                if (aiPlaceText(ai) case final where?)
                  _HintLine(
                    key: ValueKey('hint-ai-${entry.name}'),
                    text: where,
                    hint: aiPlaceHint,
                  ),
              const SizedBox(height: 4),
              // Für wen dieser Fisch übersetzt. Seit dem 20.11.2026 schreibt
              // ein Fisch nur noch an die Tische, die er bedient: den seines
              // Gastgebers und die der Eingeladenen. An den anderen liest er
              // nur mit; das soll man sehen, ohne das Profil zu öffnen.
              Wrap(
                spacing: 24,
                runSpacing: 4,
                children: [
                  Text(servesText(doc)),
                  if (readOnlyText(doc) case final readOnly?) Text(readOnly),
                ],
              ),
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
                // Kein Start und kein Stop je Profil: der Fisch hat einen
                // Schalter für alle, oben im Fenster. Ein Profil, das
                // gespeichert wird, während er an ist, nimmt er neu auf.
                if (doc != null) ...[
                  OutlinedButton(onPressed: onEdit, child: const Text('Edit')),
                  OutlinedButton(
                    onPressed: onDuplicate,
                    child: const Text('Duplicate'),
                  ),
                  Tooltip(
                    message:
                        'Copies the rooms of this profile as text, for someone who brings '
                        'a fish of their own. They paste it into a new profile.',
                    child: OutlinedButton(
                      key: ValueKey('share-${entry.name}'),
                      onPressed: onShare,
                      child: const Text('Share rooms'),
                    ),
                  ),
                ],
                // Zwei Knöpfe, damit der Gastgeber wählt, was er einer KI
                // gibt (Nemesus am 23.11.2026). Der erste lässt die Menschen
                // weg; der zweite gibt das ganze Profil mit. Ein Profil, das
                // das Programm ablehnt, hat nur seinen Grund und sein
                // Protokoll: dort gibt es den zweiten nicht.
                OutlinedButton(
                  key: ValueKey('copy-ai-${entry.name}'),
                  onPressed: () => onCopyForAi(status, withPeople: false),
                  child: const Text('Copy for an AI'),
                ),
                if (doc != null)
                  OutlinedButton(
                    key: ValueKey('copy-ai-people-${entry.name}'),
                    onPressed: () => onCopyForAi(status, withPeople: true),
                    child: const Text('Copy for an AI, with people'),
                  ),
                Tooltip(
                  // Einem laufenden Fisch die Dateien wegzunehmen hieße, dass
                  // er seinen Zustand an einen Ort schreibt, den es nicht
                  // mehr gibt.
                  message: state.on || running
                      ? 'Switch the fish off to delete a profile.'
                      : 'Move this profile to the trash folder',
                  child: OutlinedButton(
                    onPressed: state.on || running ? null : onDelete,
                    child: const Text('Delete'),
                  ),
                ),
              ],
            ),
            // Jede Karte zeigt, was ihr Profil betrifft, und was den ganzen
            // Fisch betrifft, solange ihr Profil läuft.
            if (runningHere || starting || log.isNotEmpty)
              _LogPanel(lines: log, path: state.logPathOf(entry.name)),
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

/// Eine Zeile der Karte mit einem ⓘ am Ende. Was die Zeile nicht sagen
/// kann, erscheint erst nach einem Klick darauf, unter der Zeile, und
/// verschwindet mit dem nächsten.
///
/// Bis zum 23.11.2026 ging der Text auf, sobald die Maus über der Zeile
/// stand. Nemesus an dem Tag, beim Überfliegen der Karten: „Das finde ich
/// sehr störend."
class _HintLine extends StatefulWidget {
  const _HintLine({super.key, required this.text, required this.hint});

  final String text;
  final String hint;

  @override
  State<_HintLine> createState() => _HintLineState();
}

class _HintLineState extends State<_HintLine> {
  bool _open = false;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Flexible(child: Text(widget.text, style: theme.textTheme.bodySmall)),
            const SizedBox(width: 4),
            InkWell(
              borderRadius: BorderRadius.circular(12),
              onTap: () => setState(() => _open = !_open),
              child: Padding(
                padding: const EdgeInsets.all(2),
                child: Icon(
                  Icons.info_outline,
                  size: 16,
                  color: theme.colorScheme.onSurfaceVariant,
                  semanticLabel: 'What this line cannot say',
                ),
              ),
            ),
          ],
        ),
        if (_open)
          Padding(
            padding: const EdgeInsets.only(top: 2, bottom: 6),
            child: Text(
              widget.hint,
              style: theme.textTheme.bodySmall?.copyWith(fontStyle: FontStyle.italic),
            ),
          ),
      ],
    );
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

/// Der eine Schalter des Fischs: an oder aus, für alle Profile zugleich.
///
/// Entschieden von Nemesus am 21.11.2026. Ein Bot hat in Discord einen
/// einzigen Zustand für alle Server. Mit einem Schalter je Profil sah er auch
/// dort grün aus, wo gerade nichts übersetzt wurde. Mit einem Schalter für
/// alle sagt der grüne Punkt wieder, was er zeigt.
class _FishSwitch extends StatelessWidget {
  const _FishSwitch({required this.state});

  final DeskState state;

  @override
  Widget build(BuildContext context) {
    final profiles = state.list?.profiles ?? const <ProfileEntry>[];
    final startable = profiles.where((entry) => entry.ok).length;
    // Ein Fisch, den dieses Fenster nicht gestartet hat, etwa aus einem
    // Terminal. Er hält die Sperre des Bots.
    final elsewhere = !state.on && profiles.any((entry) => entry.running);

    if (state.on) {
      final serving = state.running.length;
      return Tooltip(
        message: 'Switches the fish off for all profiles. The bot goes offline.',
        child: FilledButton.tonalIcon(
          key: const ValueKey('fish-switch'),
          onPressed: state.switchOff,
          icon: const Icon(Icons.power_settings_new),
          label: Text('Switch off · $serving of ${profiles.length} running'),
        ),
      );
    }
    return Tooltip(
      message: elsewhere
          ? 'A fish started outside this window is running. Stop it there first.'
          : startable == 0
          ? 'There is no profile to start yet.'
          : 'Switches the fish on for all profiles.',
      child: FilledButton.icon(
        key: const ValueKey('fish-switch'),
        onPressed: elsewhere || startable == 0 ? null : state.switchOn,
        icon: const Icon(Icons.power_settings_new),
        label: const Text('Switch on'),
      ),
    );
  }
}

/// Die letzten Meldungen des Fischs. Er schreibt nur Kennungen und Gründe,
/// nie den Text einer Nachricht.
class _LogPanel extends StatelessWidget {
  const _LogPanel({required this.lines, required this.path});

  final List<String> lines;

  /// Die Datei, in der dieselben Zeilen stehen, wenn es eine gibt.
  final String? path;

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
        if (path != null)
          Align(
            alignment: Alignment.centerLeft,
            child: Padding(
              padding: const EdgeInsets.only(top: 4, bottom: 8),
              child: SelectableText(
                'Also written to $path',
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ),
          ),
      ],
    );
  }
}
