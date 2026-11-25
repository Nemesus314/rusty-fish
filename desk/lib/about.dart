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
// - Ein Bild gibt es noch nicht.
//
// Den Text über das Programm hat Fischlein auf sein Wort geschrieben, der
// Satz über ihn ist sein eigener (beides 24.11.2026).
//
// Seit dem 24.11.2026 ist Rusty Fish veröffentlicht, unter der AGPL 3 oder
// später (beides von ihm an diesem Tag entschieden). Bis dahin standen hier
// sein Profil auf GitHub und der Satz, dass der Fisch dort noch nicht liegt.

import 'package:flutter/services.dart' show Clipboard, ClipboardData;
import 'package:material_ui/material_ui.dart';

/// Die Version der Maske, wie sie auch in `pubspec.yaml` steht. Ein Test hält
/// die beiden gleich.
const deskVersion = '0.4.1';

/// Wer das Programm gebaut hat.
const madeBy = 'Nemesus (Christian Ermisch)';

/// Seine Webseite.
const website = 'christianermisch.com';

/// Wo der Quelltext von Rusty Fish liegt. Wer einen veränderten Fisch
/// betreibt, schuldet seinen Gästen dessen Quelltext; diese Adresse ist die
/// des unveränderten.
const github = 'github.com/Nemesus314/rusty-fish';

/// Die Lizenz, in einem Satz. Verbindlich ist der Wortlaut in `LICENSE`.
const licenseLine =
    'Rusty Fish is free software under the GNU Affero General Public License, '
    'version 3 or later, and comes without any warranty.';

/// Wofür das Programm gebaut ist.
///
/// Nemesus am 24.11.2026: „den About Fenster Text, bitte schreibe du ihn. Du
/// weißt mehr über das Ding als ich." Bis dahin stand hier ein Entwurf, der
/// nur Discord-Kanäle und den eigenen PC kannte; seither gibt es Matrix-Räume
/// und den Server. Der Text sagt, was der Fisch tut, und wirbt für nichts.
const purpose =
    'Rusty Fish lets people talk to each other, each in their own language. '
    'Everyone writes at the table of their language, in a Discord channel or a Matrix room, '
    'and an AI translates in passing, '
    'so that nobody has to meet in a language that belongs to neither of them.';

const principles =
    'You run it yourself, on your own PC or server, with an AI service and a key you choose. '
    'It asks everyone once before it lets an AI read their words, '
    'and it says where those words go.';

/// Wer er ist, in seinen eigenen Worten vom 24.11.2026, ins Englische und in
/// die dritte Person gesetzt; derselbe Satz wie im Leitfaden.
const aboutHim =
    'He codes, writes and makes videos. What drives him is the wish for '
    'meaningful connections between people, worldwide.';

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
                const SizedBox(height: 4),
                const Text(aboutHim),
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
                const Text(licenseLine),
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
