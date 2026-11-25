// Das Einfügen von Räumen, die jemand weitergegeben hat (siehe share.dart).
//
// Zwei Schritte in einem Fenster: erst den Text einfügen und lesen lassen,
// dann sagen, wer von den genannten Menschen man selbst ist. Der wird der
// Gastgeber des neuen Profils.

import 'package:material_ui/material_ui.dart';

import 'model.dart';
import 'share.dart';

/// Was das Fenster zurückgibt: die gelesenen Räume und wer der Einfügende ist.
class PastedRooms {
  const PastedRooms(this.shared, this.me);

  final SharedRooms shared;

  /// Womit das Profil auf den Gastgeber zeigt: sein Name unter den Menschen,
  /// sonst seine Kennung. `null`, wenn er keiner der genannten ist; dann trägt
  /// er seine eigene Kennung von Hand ein.
  final String? me;
}

class PasteRoomsDialog extends StatefulWidget {
  const PasteRoomsDialog({super.key});

  @override
  State<PasteRoomsDialog> createState() => _PasteRoomsDialogState();
}

class _PasteRoomsDialogState extends State<PasteRoomsDialog> {
  final _text = TextEditingController();
  SharedRooms? _read;
  String? _error;
  String _me = '';

  @override
  void dispose() {
    _text.dispose();
    super.dispose();
  }

  void _readText() {
    final read = readSharedRooms(_text.text);
    setState(() {
      _read = read;
      _me = '';
      _error = read == null
          ? 'This is not a room text from Rusty Fish. '
                'Copy the whole text, from the first { to the last }.'
          : null;
    });
  }

  /// Ein Tisch in einem Wort: für wen er ist und in welcher Sprache dort
  /// gelesen wird.
  String _table(SharedRooms shared, Seat seat) {
    Person? owner;
    for (final person in shared.people) {
      if (seat.owner != null && person.answersTo(seat.owner!)) {
        owner = person;
        break;
      }
    }
    final lang = owner?.lang ?? seat.lang;
    final name = owner?.name;
    return name == null ? lang : '$name · $lang';
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final read = _read;
    final error = _error;
    // Der Gastgeber braucht eine Kennung: an ihr erkennt der Fisch ihn. Zur
    // Wahl steht auch, wem ein Tisch gehört, ohne unter den Menschen zu stehen.
    final candidates = read == null ? const <String>[] : hostCandidates(read);
    return AlertDialog(
      title: const Text('Paste rooms'),
      content: SizedBox(
        // Rechts bleibt Platz für die Bildlaufleiste, wie im Fenster der
        // Geheimnisse.
        width: 644,
        child: SingleChildScrollView(
          padding: const EdgeInsets.only(right: 24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Someone who runs a fish gave you a text with "Share rooms". Paste it here. '
                'It replaces the rooms and people in this form, so that both fish '
                'know the same tables.',
              ),
              const SizedBox(height: 12),
              TextField(
                key: const ValueKey('shared-rooms-text'),
                controller: _text,
                minLines: 4,
                maxLines: 8,
                decoration: const InputDecoration(
                  labelText: 'The shared text',
                  border: OutlineInputBorder(),
                ),
              ),
              if (error != null)
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Text(error, style: TextStyle(color: theme.colorScheme.error)),
                ),
              if (read != null) ...[
                const SizedBox(height: 16),
                Text(
                  read.rooms.length == 1 ? 'Found one room:' : 'Found ${read.rooms.length} rooms:',
                  style: theme.textTheme.titleSmall,
                ),
                for (final room in read.rooms)
                  Text(room.seats.map((seat) => _table(read, seat)).join(' ⇄ ')),
                const SizedBox(height: 16),
                InputDecorator(
                  decoration: const InputDecoration(labelText: 'Which of these people are you?'),
                  child: DropdownButtonHideUnderline(
                    child: DropdownButton<String>(
                      key: const ValueKey('shared-me'),
                      value: _me,
                      isDense: true,
                      isExpanded: true,
                      onChanged: (ref) => setState(() => _me = ref ?? ''),
                      items: [
                        const DropdownMenuItem(
                          value: '',
                          child: Text('none of them; I will enter my own user ID'),
                        ),
                        for (final ref in candidates)
                          DropdownMenuItem(
                            value: ref,
                            child: Text(ref, overflow: TextOverflow.ellipsis),
                          ),
                      ],
                    ),
                  ),
                ),
                const SizedBox(height: 8),
                Text(
                  'You become the host of this profile. Nobody is invited: '
                  'with a fish each, everyone pays for what they read themselves.',
                  style: theme.textTheme.bodySmall,
                ),
              ],
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        if (read == null)
          FilledButton(
            key: const ValueKey('read-shared-rooms'),
            onPressed: _readText,
            child: const Text('Read'),
          )
        else
          FilledButton(
            key: const ValueKey('use-shared-rooms'),
            onPressed: () =>
                Navigator.of(context).pop(PastedRooms(read, _me.isEmpty ? null : _me)),
            child: const Text('Use these rooms'),
          ),
      ],
    );
  }
}
