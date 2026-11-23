// Über das Programm: wofür es gebaut ist und wer es gebaut hat.
//
// Nemesus am 21.11.2026: „Langsam wäre es an der Zeit, für ein About-Button.
// Dann sollte sich ein zweites Fenster öffnen. Darin könnte dann stehen, wofür
// das Ding gebaut ist, wer ich bin, sowie Webseite und Github-Adresse?
// Eventuell mit einem hübschen Bild?"
//
// Entschieden von ihm am 20.11.2026: der Name steht mit beidem da, „Nemesus"
// und dem bürgerlichen Namen, und als Adresse nur die Webseite.
//
// Benannte Lücken, bis er sie füllt:
// - Der Text über ihn und das Programm ist ein Entwurf von Fischlein.
// - Rusty Fish ist noch nicht veröffentlicht. Hier steht deshalb sein Profil
//   auf GitHub und der Satz, dass der Fisch dort noch nicht liegt.
// - Ein Bild gibt es noch nicht.

import 'package:flutter/services.dart' show Clipboard, ClipboardData;
import 'package:material_ui/material_ui.dart';

/// Die Version der Maske, wie sie auch in `pubspec.yaml` steht. Ein Test hält
/// die beiden gleich.
const deskVersion = '0.3.0';

/// Wer das Programm gebaut hat.
const madeBy = 'Nemesus (Christian Ermisch)';

/// Seine Webseite.
const website = 'christianermisch.com';

/// Sein Profil auf GitHub. Dort liegen Cluster One, Prometheus und Lamarr;
/// Rusty Fish selbst ist dort noch nicht veröffentlicht.
const github = 'github.com/Nemesus314';

/// Wofür das Programm gebaut ist. Entwurf, bis Nemesus seinen Text liefert.
const purpose =
    'Rusty Fish lets people write to each other, each in their own language. '
    'Everyone types in the channel of their language, and an AI translates in passing, '
    'so that nobody has to meet in a language that belongs to neither of them.';

const principles =
    'It runs on your own PC, with an AI service and a key you choose yourself. '
    'It asks everyone once before it lets the AI read their words, '
    'and it says where those words go.';

/// Öffnet das Fenster über das Programm.
Future<void> showAbout(BuildContext context) {
  return showDialog<void>(context: context, builder: (_) => const AboutFish());
}

class AboutFish extends StatelessWidget {
  const AboutFish({super.key});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final small = theme.textTheme.bodySmall;
    return Dialog(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 560),
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: SelectionArea(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Icon(Icons.translate, size: 40, color: theme.colorScheme.primary),
                    const SizedBox(width: 16),
                    Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text('Rusty Fish', style: theme.textTheme.headlineSmall),
                        Text('Version $deskVersion', style: small),
                      ],
                    ),
                  ],
                ),
                const SizedBox(height: 20),
                Text('What it is for', style: theme.textTheme.titleMedium),
                const SizedBox(height: 6),
                const Text(purpose),
                const SizedBox(height: 8),
                const Text(principles),
                const SizedBox(height: 20),
                Text('Who made it', style: theme.textTheme.titleMedium),
                const SizedBox(height: 6),
                const Text(madeBy),
                Row(
                  children: [
                    const Text(website),
                    // Ohne eigenes Paket kann die Maske keinen Browser öffnen.
                    // Kopieren geht immer.
                    IconButton(
                      key: const ValueKey('copy-website'),
                      tooltip: 'Copy the address',
                      iconSize: 18,
                      onPressed: () => Clipboard.setData(const ClipboardData(text: website)),
                      icon: const Icon(Icons.copy),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                Text('Source code', style: theme.textTheme.titleMedium),
                const SizedBox(height: 6),
                Row(
                  children: [
                    const Text(github),
                    IconButton(
                      key: const ValueKey('copy-github'),
                      tooltip: 'Copy the address',
                      iconSize: 18,
                      onPressed: () => Clipboard.setData(const ClipboardData(text: github)),
                      icon: const Icon(Icons.copy),
                    ),
                  ],
                ),
                const Text(
                  'Rusty Fish itself is not published there yet. It is planned as free '
                  'software under the GPL 3 or the AGPL 3.',
                ),
                const SizedBox(height: 20),
                Align(
                  alignment: Alignment.centerRight,
                  child: FilledButton(
                    onPressed: () => Navigator.of(context).pop(),
                    child: const Text('Close'),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
