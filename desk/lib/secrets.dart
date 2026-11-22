// Die Geheimnisse: der Token des Bots, das Matrix-Konto des Fischs und je ein
// Schlüssel pro KI-Dienst.
//
// Die Maske zeigt nur, ob sie liegen. Ein Wert geht vom Eingabefeld über die
// Standardeingabe an das Programm und von dort in die Ablage des Systems.
// Zurück kommt er nie: es gibt keinen Aufruf, der ihn ausliest.
//
// Bis zum 20.11.2026 gab es genau einen KI-Schlüssel. Nemesus am selben Tag:
// „Die Möglichkeit, die KIs im Profil zu wechseln sollte jedoch erhalten
// bleiben." Seitdem hat jeder Dienst seinen eigenen Schlüssel, und der Dialog
// zeigt eine Zeile je Dienst, den das Programm nennt.
//
// Seit dem 22.11.2026 steht hier auch das Matrix-Konto des Fischs. Es ist
// kein Geheimnis, das man einfügt: man meldet den Fisch an. Das Passwort geht
// wie jedes Geheimnis über die Standardeingabe an das Programm, wird dort
// einmal gebraucht und nicht aufbewahrt. In der Ablage liegt danach der
// Zugang dieses einen Geräts.

import 'package:flutter/services.dart' show Clipboard, ClipboardData;
import 'package:material_ui/material_ui.dart';

import 'cli.dart';
import 'model.dart';

/// Eine Zeile des Dialogs.
class _Entry {
  const _Entry({
    required this.entry,
    required this.title,
    required this.forgetTitle,
    required this.forgetBody,
    this.note,
  });

  /// Der Name des Eintrags in der Ablage.
  final String entry;
  final String title;
  final String forgetTitle;
  final String forgetBody;

  /// Ein Satz unter dem Feld, wo er etwas klärt.
  final String? note;
}

class SecretsDialog extends StatefulWidget {
  const SecretsDialog({
    super.key,
    required this.cli,
    required this.providers,
    required this.onChanged,
  });

  final FishCli cli;

  /// Die Dienste, die das Programm nennt. Jeder bekommt eine Zeile.
  final List<Provider> providers;

  /// Die Übersicht dahinter soll sofort den neuen Stand zeigen.
  final Future<void> Function() onChanged;

  @override
  State<SecretsDialog> createState() => _SecretsDialogState();
}

class _SecretsDialogState extends State<SecretsDialog> {
  late final List<_Entry> _entries = [
    const _Entry(
      entry: botTokenEntry,
      title: botTokenLabel,
      forgetTitle: 'Forget the bot token?',
      forgetBody:
          'Without it the fish cannot connect to Discord. You can store it again any time.',
      note: 'From the Discord Developer Portal: your application, Bot, Reset Token.',
    ),
    for (final provider in widget.providers)
      _Entry(
        entry: provider.id,
        title: provider.label,
        forgetTitle: 'Forget the key for ${provider.label}?',
        forgetBody: provider.needsKey
            ? 'Without it, profiles that use ${provider.label} cannot translate. '
                  'You can store it again any time.'
            : 'Profiles that use ${provider.label} then run without a key. '
                  'You can store it again any time.',
        // Ein eigenes Modell auf dem eigenen Rechner verlangt oft keinen
        // Schlüssel. Das muss dastehen, sonst sucht man einen, den es nicht gibt.
        note: provider.needsKey
            ? null
            : 'A key is optional here. Leave it out if your own model asks for none.',
      ),
  ];
  late final Map<String, TextEditingController> _fields = {
    for (final entry in _entries) entry.entry: TextEditingController(),
  };
  KeyStatus? _status;
  String? _error;
  bool _busy = false;

  final _matrixUser = TextEditingController();
  final _matrixPassword = TextEditingController();
  final _matrixRecovery = TextEditingController();

  /// Ob und als wer der Fisch bei Matrix angemeldet ist.
  MatrixStatus? _matrix;

  /// Das Programm kennt Matrix nicht (ein älterer Stand).
  bool _matrixUnknown = false;

  /// Was das Programm nach dem An- oder Abmelden angemerkt hat.
  String? _matrixNote;

  /// Was nach dem Klick auf den Link für den Bot zu sagen ist.
  String? _inviteNote;

  /// Legt den Link in die Zwischenablage, mit dem dieser Bot auf einen Server
  /// kommt.
  ///
  /// Gemessen am 22.11.2026 im ersten Lauf mit zwei Fischen: bringt jeder
  /// seinen eigenen Fisch mit, muss der Bot des einen auf den Server des
  /// anderen. Der Link dafür ließ sich bis dahin nur von Hand zusammensetzen.
  Future<void> _copyInvite() async {
    String note;
    try {
      final link = await widget.cli.inviteLink();
      await Clipboard.setData(ClipboardData(text: link));
      note =
          'Link copied. Send it to whoever owns the server. '
          'They open it, choose the server and see the four rights before they agree.';
    } on CliException catch (e) {
      note = 'Cannot make the link: ${e.message}';
    }
    if (mounted) {
      setState(() => _inviteNote = note);
    }
  }

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
    _matrixPassword.clear();
    _matrixPassword.dispose();
    _matrixRecovery.clear();
    _matrixRecovery.dispose();
    _matrixUser.dispose();
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
    try {
      final matrix = await widget.cli.matrixStatus();
      if (mounted) {
        setState(() {
          _matrix = matrix;
          _matrixUnknown = false;
        });
      }
    } on CliException {
      // Ein älteres Programm kennt den Aufruf nicht. Das ist kein Fehler des
      // Fensters; der Block sagt es an seiner Stelle.
      if (mounted) {
        setState(() => _matrixUnknown = true);
      }
    }
  }

  /// Was das Programm nach dem Anmelden anmerkt, steht in Zeilen mit „Note:".
  String? _notesOf(String said) {
    final notes = [
      for (final line in said.split('\n'))
        if (line.trim().startsWith('Note: ')) line.trim().substring('Note: '.length),
    ];
    return notes.isEmpty ? null : notes.join(' ');
  }

  Future<void> _matrixLogin() async {
    final user = _matrixUser.text.trim();
    // Ein Passwort darf mit einem Leerzeichen enden; abgeschnitten wird nichts.
    final password = _matrixPassword.text;
    if (user.isEmpty || password.isEmpty) {
      setState(() => _error = 'Enter the user ID of the fish and its password.');
      return;
    }
    final recovery = _matrixRecovery.text.trim();
    // Die Felder werden geleert, bevor irgendetwas anderes geschieht. Danach
    // stehen Passwort und Wiederherstellungsschlüssel nirgends mehr im Fenster.
    _matrixPassword.clear();
    _matrixRecovery.clear();
    _matrixNote = null;
    await _act(() async {
      _matrixNote = _notesOf(
        await widget.cli.matrixLogin(user, password, recovery: recovery),
      );
      _matrixUser.clear();
    });
  }

  Future<void> _matrixLogout() async {
    final sure = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Log the fish out of Matrix?'),
        content: const Text(
          'Its device keys on this PC are removed. After a new login the fish is a new '
          'device: in encrypted rooms it cannot read what was written before.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Log out'),
          ),
        ],
      ),
    );
    if (sure == true) {
      _matrixNote = null;
      await _act(() async {
        _matrixNote = _notesOf(await widget.cli.matrixLogout());
      });
    }
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

  Future<void> _store(_Entry entry) async {
    final field = _fields[entry.entry]!;
    final value = field.text.trim();
    if (value.isEmpty) {
      setState(() => _error = 'Nothing entered, nothing stored.');
      return;
    }
    // Das Feld wird geleert, bevor irgendetwas anderes geschieht. Danach
    // steht der Wert nirgends mehr im Fenster.
    field.clear();
    await _act(() => widget.cli.setKey(entry.entry, value));
  }

  Future<void> _forget(_Entry entry) async {
    final sure = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(entry.forgetTitle),
        content: Text(entry.forgetBody),
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
      await _act(() => widget.cli.forgetKey(entry.entry));
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final error = _error;
    return AlertDialog(
      title: const Text('Secrets'),
      content: SizedBox(
        // Rechts bleibt Platz für die Bildlaufleiste. Gesehen von Nemesus am
        // 22.11.2026: ohne ihn schwebten die Knöpfe „Forget" über der Leiste,
        // sobald das Fenster rollte.
        width: 644,
        child: SingleChildScrollView(
          padding: const EdgeInsets.only(right: 24),
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
              for (final (index, entry) in _entries.indexed) ...[
                // Der Token des Bots ist etwas anderes als die Schlüssel der
                // Dienste. Dazwischen steht das Konto für die zweite Tür, und
                // die Überschrift trennt die Schlüssel ab.
                if (index == 1) ...[
                  _matrixBlock(theme),
                  Padding(
                    padding: const EdgeInsets.only(top: 24),
                    child: Text(
                      'AI keys, one per service',
                      style: theme.textTheme.titleSmall,
                    ),
                  ),
                ],
                _block(entry, theme),
              ],
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

  /// Das Matrix-Konto des Fischs: anmelden, oder sehen, als wer er angemeldet
  /// ist, und abmelden.
  Widget _matrixBlock(ThemeData theme) {
    final status = _matrix;
    final loggedIn = _matrixUnknown ? null : status?.loggedIn;
    final String state;
    if (_matrixUnknown) {
      state = 'this program does not know Matrix';
    } else if (status == null) {
      state = 'checking';
    } else if (status.loggedIn) {
      state = 'logged in as ${status.user ?? 'the fish'}';
    } else {
      state = 'not logged in';
    }
    return Padding(
      key: const ValueKey('secret-row-matrix'),
      padding: const EdgeInsets.only(top: 16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Text('Matrix account', style: theme.textTheme.titleMedium),
              const SizedBox(width: 12),
              Flexible(
                child: Text(
                  state,
                  key: const ValueKey('matrix-state'),
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: loggedIn == true
                        ? theme.colorScheme.primary
                        : theme.colorScheme.onSurfaceVariant,
                  ),
                ),
              ),
            ],
          ),
          if (loggedIn == false)
            Row(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Expanded(
                  flex: 3,
                  child: TextField(
                    key: const ValueKey('matrix-user'),
                    controller: _matrixUser,
                    enableSuggestions: false,
                    autocorrect: false,
                    decoration: const InputDecoration(
                      labelText: 'User ID of the fish',
                      hintText: '@name:server',
                    ),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  flex: 2,
                  child: TextField(
                    key: const ValueKey('matrix-password'),
                    controller: _matrixPassword,
                    obscureText: true,
                    enableSuggestions: false,
                    autocorrect: false,
                    decoration: const InputDecoration(labelText: 'Its password'),
                  ),
                ),
                const SizedBox(width: 12),
                FilledButton(
                  key: const ValueKey('matrix-login'),
                  onPressed: _busy ? null : _matrixLogin,
                  child: const Text('Log in'),
                ),
                // Derselbe Platz wie neben den anderen Zeilen, damit die
                // Felder untereinander stehen.
                const SizedBox(width: 116),
              ],
            ),
          // Gewünscht von Nemesus am 22.11.2026, nachdem ein Matrix-Programm
          // das Gerät des Fischs als unsicher gezeigt hatte: „wäre es schön,
          // wenn auch der Fishbot sicher wäre“.
          if (loggedIn == false)
            Padding(
              padding: const EdgeInsets.only(right: 116),
              child: TextField(
                key: const ValueKey('matrix-recovery'),
                controller: _matrixRecovery,
                obscureText: true,
                enableSuggestions: false,
                autocorrect: false,
                decoration: const InputDecoration(
                  labelText: 'Recovery key of the account (optional)',
                  helperText:
                      'Only if the account has one, from a Matrix app. It makes the fish a '
                      'verified device. Used once, not kept.',
                  helperMaxLines: 2,
                ),
              ),
            ),
          if (loggedIn == true)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: OutlinedButton(
                key: const ValueKey('matrix-logout'),
                onPressed: _busy ? null : _matrixLogout,
                child: const Text('Log out'),
              ),
            ),
          Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Text(
              'An ordinary Matrix account of its own for the fish, not yours. The password is '
              'used once to log in and is not kept; what is stored is the access of this one '
              'device. Switch the fish off before you log in or out.',
              style: theme.textTheme.bodySmall,
            ),
          ),
          if (_matrixNote case final note?)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Text(note, style: theme.textTheme.bodySmall),
            ),
        ],
      ),
    );
  }

  Widget _block(_Entry entry, ThemeData theme) {
    final status = _status;
    final stored = status?.has(entry.entry);
    return Padding(
      key: ValueKey('secret-row-${entry.entry}'),
      padding: const EdgeInsets.only(top: 16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Text(entry.title, style: theme.textTheme.titleMedium),
              const SizedBox(width: 12),
              Text(
                stored == null ? 'checking' : (stored ? 'stored' : 'not stored'),
                style: TextStyle(
                  color: stored == true
                      ? theme.colorScheme.primary
                      : theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
          Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Expanded(
                child: TextField(
                  key: ValueKey('secret-${entry.entry}'),
                  controller: _fields[entry.entry],
                  obscureText: true,
                  enableSuggestions: false,
                  autocorrect: false,
                  decoration: InputDecoration(
                    labelText: stored == true
                        ? 'Replace it with a new one'
                        : 'Paste it here',
                  ),
                ),
              ),
              const SizedBox(width: 12),
              FilledButton(
                key: ValueKey('store-${entry.entry}'),
                onPressed: _busy ? null : () => _store(entry),
                child: const Text('Store'),
              ),
              const SizedBox(width: 8),
              // Der Platz bleibt auch ohne Knopf belegt, damit die Felder
              // aller Zeilen gleich breit untereinander stehen. Im echten
              // Fenster angesehen am 20.11.2026: bei 84 Punkten Breite brach
              // „Forget" in zwei Zeilen um.
              SizedBox(
                width: 108,
                child: stored == true
                    ? OutlinedButton(
                        key: ValueKey('forget-${entry.entry}'),
                        onPressed: _busy ? null : () => _forget(entry),
                        child: const Text('Forget', softWrap: false),
                      )
                    : null,
              ),
            ],
          ),
          // Der Hinweis steht unter der Zeile und nicht als Hilfetext im Feld.
          // Als Hilfetext schob er die Knöpfe daneben nach unten, und die
          // Zeilen mit und ohne Hinweis sahen verschieden aus.
          if (entry.note != null)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Text(entry.note!, style: theme.textTheme.bodySmall),
            ),
          // Liegt das Token, kennt das Programm die Kennung des Bots und kann
          // den Link nennen, mit dem er auf einen Server kommt. Der Link
          // enthält kein Geheimnis.
          if (entry.entry == botTokenEntry && stored == true) ...[
            Align(
              alignment: Alignment.centerLeft,
              child: TextButton.icon(
                key: const ValueKey('copy-invite'),
                onPressed: _copyInvite,
                icon: const Icon(Icons.link, size: 18),
                label: const Text('Copy the link that brings this bot onto a server'),
              ),
            ),
            if (_inviteNote case final note?)
              Text(note, style: theme.textTheme.bodySmall),
          ],
        ],
      ),
    );
  }
}
