// Ein Feld für eine Sprache, das beim Tippen vorschlägt.
//
// Nemesus am 24.11.2026: „Im Moment muss man die Sprachen manuell eingeben.
// Das könnte Fehler verursachen. Besser wäre, wenn Vorschläge kommen würden,
// je nachdem, was ich tippe. Beispielsweise ich tippe Ko und ergänzt wird
// Koreanisch."
//
// Die Liste nennt das Programm (`languages --json`); die Maske erfindet keine
// eigene. Das Feld bleibt ein freies Feld: wer etwas tippt, das nicht auf der
// Liste steht, behält es. Kennt das Programm den Aufruf nicht (älterer
// Stand), ist die Liste leer und das Feld verhält sich wie zuvor.

import 'package:material_ui/material_ui.dart';

import 'model.dart';

/// Ein Vorschlag und der Text, der nach der Wahl im Feld steht.
typedef _Option = ({String name, String? via, String result});

class LanguageField extends StatefulWidget {
  const LanguageField({
    super.key,
    required this.fieldKey,
    required this.controller,
    required this.languages,
    required this.decoration,
    this.many = false,
  });

  /// Der Schlüssel des Textfelds selbst, unter dem es schon immer zu finden
  /// war.
  final Key fieldKey;
  final TextEditingController controller;
  final List<Language> languages;
  final InputDecoration decoration;

  /// Mehrere Sprachen, durch Kommas getrennt: vorgeschlagen wird für die
  /// letzte, die davor bleiben stehen.
  final bool many;

  @override
  State<LanguageField> createState() => _LanguageFieldState();
}

class _LanguageFieldState extends State<LanguageField> {
  final _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    // Der Hinweis unter dem Feld hängt daran, was darin steht und ob gerade
    // jemand darin tippt.
    _focus.addListener(_changed);
    widget.controller.addListener(_changed);
  }

  @override
  void didUpdateWidget(LanguageField old) {
    super.didUpdateWidget(old);
    if (old.controller != widget.controller) {
      old.controller.removeListener(_changed);
      widget.controller.addListener(_changed);
    }
  }

  @override
  void dispose() {
    widget.controller.removeListener(_changed);
    _focus.dispose();
    super.dispose();
  }

  void _changed() {
    if (mounted) {
      setState(() {});
    }
  }

  /// Steht ein Wort im Feld, das nicht auf der Liste ist, sagt die Maske es
  /// darunter. Nicht während des Tippens: da ist jeder Anfang eines Namens
  /// noch kein Name, und die Vorschläge stehen ohnehin offen.
  String? get _hint => _focus.hasFocus
      ? null
      : languageHint(widget.languages, widget.controller.text, many: widget.many);

  List<_Option> _options(String text) {
    // Bei mehreren Sprachen zählt nur, was hinter dem letzten Komma steht.
    final cut = widget.many ? text.lastIndexOf(',') + 1 : 0;
    final head = text.substring(0, cut);
    return [
      for (final hit in suggestLanguages(widget.languages, text.substring(cut)))
        (name: hit.name, via: hit.via, result: head.isEmpty ? hit.name : '$head ${hit.name}'),
    ];
  }

  @override
  Widget build(BuildContext context) {
    return RawAutocomplete<_Option>(
      textEditingController: widget.controller,
      focusNode: _focus,
      displayStringForOption: (option) => option.result,
      optionsBuilder: (value) => _options(value.text),
      fieldViewBuilder: (context, controller, focus, onSubmitted) {
        final hint = _hint;
        final theme = Theme.of(context);
        return Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            TextField(
              key: widget.fieldKey,
              controller: controller,
              focusNode: focus,
              decoration: widget.decoration,
              // Eingabetaste: der hervorgehobene Vorschlag, wenn einer offen ist.
              onSubmitted: (_) => onSubmitted(),
            ),
            // Unter dem Feld und seinem eigenen Hilfstext, nicht an dessen
            // Stelle: der sagt, wofür das Feld ist, dieser, was darin steht.
            if (hint != null)
              Padding(
                padding: const EdgeInsets.only(top: 4),
                child: Text(
                  hint,
                  key: ValueKey('language-hint-${(widget.fieldKey as ValueKey).value}'),
                  style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.tertiary),
                ),
              ),
          ],
        );
      },
      optionsViewBuilder: (context, onSelected, options) {
        // Nach einer Wahl behält Flutter die alten Vorschläge und zeigte sie
        // wieder, sobald das Feld erneut den Fokus bekommt. Gezeigt wird
        // deshalb nur, wozu der jetzige Text noch Vorschläge hat.
        if (_options(widget.controller.text).isEmpty) {
          return const SizedBox.shrink();
        }
        final list = options.toList();
        return Align(
          alignment: Alignment.topLeft,
          child: Material(
            elevation: 4,
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 232),
              child: ListView.builder(
                padding: EdgeInsets.zero,
                shrinkWrap: true,
                itemCount: list.length,
                itemBuilder: (context, index) {
                  final option = list[index];
                  final via = option.via;
                  final theme = Theme.of(context);
                  return InkWell(
                    key: ValueKey('suggest-${option.name}'),
                    onTap: () => onSelected(option),
                    child: Container(
                      width: double.infinity,
                      color: AutocompleteHighlightedOption.of(context) == index
                          ? theme.focusColor
                          : null,
                      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(option.name),
                          // Gefunden über einen anderen Namen: der steht
                          // klein darunter, damit klar ist, warum.
                          if (via != null)
                            Text(
                              via,
                              style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor),
                            ),
                        ],
                      ),
                    ),
                  );
                },
              ),
            ),
          ),
        );
      },
    );
  }
}
