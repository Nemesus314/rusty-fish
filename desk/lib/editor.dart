// Ein Profil bearbeiten. Die Maske sammelt nur ein; ob das Ergebnis gültig
// ist, entscheidet das Programm beim Speichern. Lehnt es ab, steht sein Grund
// wörtlich über dem Formular, und nichts von dem Getippten geht verloren.

import 'package:material_ui/material_ui.dart';

import 'cli.dart';
import 'model.dart';

class ProfileEditorPage extends StatefulWidget {
  const ProfileEditorPage({
    super.key,
    required this.cli,
    required this.initial,
    required this.isNew,
    required this.existingNames,
    this.providers = fallbackProviders,
    this.providersListed = false,
  });

  final FishCli cli;
  final ProfileDoc initial;

  /// Die KI-Dienste, die das Programm nennt. Die Auswahl zeigt genau diese.
  final List<Provider> providers;

  /// Ob die Liste wirklich vom Programm stammt. Ein älteres Programm kennt
  /// die Angabe des Dienstes bei der Frage nach den Modellen nicht.
  final bool providersListed;

  /// Neu oder Kopie: der Name ist dann noch frei wählbar.
  final bool isNew;

  /// Die Namen, die es schon gibt. Ein neues Profil darf keines ersetzen.
  final Set<String> existingNames;

  @override
  State<ProfileEditorPage> createState() => _ProfileEditorPageState();
}

/// Die Auswahl im Formular. „Keine Angabe" braucht einen eigenen Wert, weil
/// eine Auswahlliste mit leerem Wert nichts anzeigt.
enum _Form { none, female, male }

_Form _formOf(Gender? gender) => switch (gender) {
  Gender.female => _Form.female,
  Gender.male => _Form.male,
  null => _Form.none,
};

Gender? _genderOf(_Form form) => switch (form) {
  _Form.female => Gender.female,
  _Form.male => Gender.male,
  _Form.none => null,
};

class _SeatDraft {
  _SeatDraft(Seat seat)
    : channel = TextEditingController(text: seat.channel),
      lang = TextEditingController(text: seat.lang),
      form = _formOf(seat.gender);

  final TextEditingController channel;
  final TextEditingController lang;
  _Form form;

  void dispose() {
    channel.dispose();
    lang.dispose();
  }
}

class _RoomDraft {
  _RoomDraft(Room room)
    : passThrough = TextEditingController(text: room.passThrough.join(', ')),
      contextLines = TextEditingController(text: '${room.contextLines}'),
      seats = [for (final seat in room.seats) _SeatDraft(seat)];

  final TextEditingController passThrough;
  final TextEditingController contextLines;
  final List<_SeatDraft> seats;

  void dispose() {
    passThrough.dispose();
    contextLines.dispose();
    for (final seat in seats) {
      seat.dispose();
    }
  }
}

class _PersonDraft {
  _PersonDraft(Person? person)
    : id = TextEditingController(text: person?.id ?? ''),
      gender = person?.gender ?? Gender.female;

  final TextEditingController id;
  Gender gender;

  void dispose() => id.dispose();
}

class _ProfileEditorPageState extends State<ProfileEditorPage> {
  late final TextEditingController _name;
  late final TextEditingController _model;
  late final TextEditingController _baseUrl;
  late final TextEditingController _host;
  late final TextEditingController _tokens;
  late final TextEditingController _questions;
  late final List<TextEditingController> _aiNames;
  late final List<_RoomDraft> _rooms;
  late final List<_PersonDraft> _people;

  String? _error;
  String? _modelsError;
  bool _saving = false;
  bool _loadingModels = false;

  /// Die Kennung des gewählten KI-Dienstes.
  late String _provider;

  /// Der Dienst wurde gewechselt, das Modell stammt aber noch vom alten. Dann
  /// steht ein Hinweis am Modellfeld, bis dort etwas Neues eingetragen ist.
  bool _modelFromOtherService = false;

  /// Die Auswahl im Formular: die Dienste des Programms und, falls das Profil
  /// einen nennt, den das Programm nicht (mehr) aufführt, auch dieser. Sonst
  /// würde ein Profil beim bloßen Öffnen und Speichern still umgestellt.
  late final List<Provider> _choices;

  Provider get _service => _choices.firstWhere((p) => p.id == _provider);

  @override
  void initState() {
    super.initState();
    final doc = widget.initial;
    _provider = doc.provider;
    _choices = [
      ...widget.providers,
      if (!widget.providers.any((p) => p.id == doc.provider))
        Provider(id: doc.provider, label: doc.provider, baseUrl: null, needsKey: true),
    ];
    _name = TextEditingController(text: doc.name);
    _model = TextEditingController(text: doc.model);
    _baseUrl = TextEditingController(text: doc.baseUrl ?? '');
    _host = TextEditingController(text: doc.host);
    _tokens = TextEditingController(text: '${doc.limits.translationTokensPerDay}');
    _questions = TextEditingController(text: '${doc.limits.questionsPerGuestPerDay}');
    _aiNames = [for (final name in doc.aiNames) TextEditingController(text: name)];
    _rooms = [for (final room in doc.rooms) _RoomDraft(room)];
    _people = [for (final person in doc.people) _PersonDraft(person)];
    // Der Hinweis zum Namen erscheint schon beim Tippen.
    _name.addListener(_redraw);
    // Wer nach einem Wechsel des Dienstes das Modell anfasst, hat den Hinweis
    // dazu gesehen. Er verschwindet dann.
    _model.addListener(_modelTouched);
  }

  void _modelTouched() {
    if (_modelFromOtherService && mounted) {
      setState(() => _modelFromOtherService = false);
    }
  }

  void _chooseService(String id) {
    if (id == _provider) {
      return;
    }
    setState(() {
      _provider = id;
      // Das Modell bleibt stehen, damit nichts still verschwindet. Bei dem
      // neuen Dienst heißt es aber fast sicher anders.
      _modelFromOtherService = _model.text.trim().isNotEmpty;
      _modelsError = null;
      // Eine abweichende Adresse gehörte zum alten Dienst. Bliebe sie stehen,
      // ginge das Profil mit dem Schlüssel des neuen Dienstes an die Adresse
      // des alten.
      _baseUrl.clear();
    });
  }

  void _redraw() {
    if (mounted) {
      setState(() {});
    }
  }

  @override
  void dispose() {
    _name.dispose();
    _model.dispose();
    _baseUrl.dispose();
    _host.dispose();
    _tokens.dispose();
    _questions.dispose();
    for (final name in _aiNames) {
      name.dispose();
    }
    for (final room in _rooms) {
      room.dispose();
    }
    for (final person in _people) {
      person.dispose();
    }
    super.dispose();
  }

  /// Der Hinweis zum Namen, solange er noch frei wählbar ist.
  String? get _nameProblem {
    if (!widget.isNew) {
      return null;
    }
    final name = _name.text;
    if (name.isEmpty) {
      return null;
    }
    if (!profileNamePattern.hasMatch(name)) {
      return 'Only a-z, 0-9, - and _ are possible, up to 40 characters.';
    }
    if (widget.existingNames.contains(name)) {
      return 'A profile named $name already exists.';
    }
    return null;
  }

  int? _number(TextEditingController field, String label) {
    final value = int.tryParse(field.text.trim());
    if (value == null || value < 0) {
      setState(() => _error = '"$label" must be a whole number.');
    }
    return (value == null || value < 0) ? null : value;
  }

  /// Baut aus dem Formular das Profil, so wie es an das Programm geht.
  ProfileDoc? _collect() {
    final tokens = _number(_tokens, 'Translation tokens per day');
    if (tokens == null) {
      return null;
    }
    final questions = _number(_questions, 'Questions to the AI per guest per day');
    if (questions == null) {
      return null;
    }
    final rooms = <Room>[];
    for (final (index, room) in _rooms.indexed) {
      final context = _number(
        room.contextLines,
        'Earlier messages for context (room ${index + 1})',
      );
      if (context == null) {
        return null;
      }
      rooms.add(
        Room(
          passThrough: [
            for (final lang in room.passThrough.text.split(','))
              if (lang.trim().isNotEmpty) lang.trim(),
          ],
          contextLines: context,
          seats: [
            for (final seat in room.seats)
              Seat(
                channel: seat.channel.text.trim(),
                lang: seat.lang.text.trim(),
                gender: _genderOf(seat.form),
              ),
          ],
        ),
      );
    }
    final baseUrl = _baseUrl.text.trim();
    return ProfileDoc(
      name: _name.text.trim(),
      provider: _provider,
      model: _model.text.trim(),
      baseUrl: baseUrl.isEmpty ? null : baseUrl,
      host: _host.text.trim(),
      // Eine Zeile, in die nichts getippt wurde, ist keine Angabe.
      aiNames: [
        for (final name in _aiNames)
          if (name.text.trim().isNotEmpty) name.text.trim(),
      ],
      limits: Limits(
        translationTokensPerDay: tokens,
        questionsPerGuestPerDay: questions,
      ),
      rooms: rooms,
      people: [
        for (final person in _people)
          if (person.id.text.trim().isNotEmpty)
            Person(id: person.id.text.trim(), gender: person.gender),
      ],
    );
  }

  Future<void> _save() async {
    if (_saving) {
      return;
    }
    final problem = _nameProblem;
    if (problem != null) {
      setState(() => _error = problem);
      return;
    }
    final doc = _collect();
    if (doc == null) {
      return;
    }
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      await widget.cli.saveProfile(doc);
    } on CliException catch (e) {
      if (mounted) {
        setState(() {
          _saving = false;
          _error = e.message;
        });
      }
      return;
    }
    if (mounted) {
      Navigator.of(context).pop(true);
    }
  }

  Future<void> _loadModels() async {
    setState(() {
      _loadingModels = true;
      _modelsError = null;
    });
    final List<String> models;
    try {
      models = await widget.cli.models(
        // Ein älteres Programm kennt diese Angabe nicht; dort fragt die
        // Maske so, wie sie es vor der Auswahl tat.
        provider: widget.providersListed ? _provider : null,
        baseUrl: _baseUrl.text,
      );
    } on CliException catch (e) {
      if (mounted) {
        setState(() {
          _loadingModels = false;
          _modelsError = e.message;
        });
      }
      return;
    }
    if (!mounted) {
      return;
    }
    setState(() => _loadingModels = false);
    final picked = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text('Models at ${_service.label} (${models.length})'),
        content: SizedBox(
          width: 420,
          height: 460,
          child: ListView(
            children: [
              for (final id in models)
                ListTile(
                  dense: true,
                  title: Text(id),
                  onTap: () => Navigator.of(context).pop(id),
                ),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Cancel'),
          ),
        ],
      ),
    );
    if (picked != null && mounted) {
      setState(() => _model.text = picked);
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final error = _error;
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.isNew ? 'New profile' : 'Edit ${widget.initial.name}'),
        actions: [
          TextButton(
            onPressed: _saving ? null : () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          const SizedBox(width: 8),
          FilledButton(
            onPressed: _saving ? null : _save,
            child: const Text('Save'),
          ),
          const SizedBox(width: 16),
        ],
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.fromLTRB(24, 8, 24, 32),
        child: Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 920),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                if (error != null)
                  Container(
                    key: const ValueKey('editor-error'),
                    width: double.infinity,
                    margin: const EdgeInsets.only(bottom: 16),
                    padding: const EdgeInsets.all(16),
                    decoration: BoxDecoration(
                      color: theme.colorScheme.errorContainer,
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: SelectionArea(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            'Not saved',
                            style: theme.textTheme.titleMedium?.copyWith(
                              color: theme.colorScheme.onErrorContainer,
                            ),
                          ),
                          const SizedBox(height: 4),
                          Text(
                            error,
                            style: TextStyle(
                              color: theme.colorScheme.onErrorContainer,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                _general(),
                _ai(),
                _limits(),
                _roomsSection(),
                _peopleSection(),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _general() {
    final modelsError = _modelsError;
    final service = _service;
    return _Section(
      title: 'General',
      children: [
        TextField(
          key: const ValueKey('field-name'),
          controller: _name,
          enabled: widget.isNew,
          decoration: InputDecoration(
            labelText: 'Name',
            helperText: widget.isNew
                ? 'Only a-z, 0-9, - and _. Up to 40 characters.'
                : 'The name of a saved profile is fixed. Duplicate it to get another name.',
            errorText: _nameProblem,
          ),
        ),
        const SizedBox(height: 16),
        // Nemesus am 20.11.2026: „Die Möglichkeit, die KIs im Profil zu
        // wechseln sollte jedoch erhalten bleiben." Bis dahin stand hier nur
        // ein Adressfeld („Service address (optional)"), in das man die
        // Adresse eines anderen Dienstes selbst eintippen musste.
        DropdownButtonFormField<String>(
          key: const ValueKey('field-provider'),
          initialValue: _provider,
          isExpanded: true,
          decoration: InputDecoration(
            labelText: 'AI service',
            helperText: service.needsKey
                ? 'Who translates. Its key is kept under Secrets.'
                : 'Who translates. A key is optional for this one.',
          ),
          items: [
            for (final provider in _choices)
              DropdownMenuItem(value: provider.id, child: Text(provider.label)),
          ],
          onChanged: (id) {
            if (id != null) {
              _chooseService(id);
            }
          },
        ),
        const SizedBox(height: 16),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: TextField(
                key: const ValueKey('field-model'),
                controller: _model,
                decoration: InputDecoration(
                  labelText: 'Model',
                  helperText: _modelFromOtherService
                      ? 'Model names differ between services. Check that this one exists at ${service.label}.'
                      : 'The AI model that translates.',
                  helperMaxLines: 2,
                  errorText: modelsError,
                  errorMaxLines: 4,
                ),
              ),
            ),
            const SizedBox(width: 12),
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: OutlinedButton(
                onPressed: _loadingModels ? null : _loadModels,
                child: const Text('Load models'),
              ),
            ),
          ],
        ),
        const SizedBox(height: 16),
        if (service.needsAddress)
          // Ohne übliche Adresse muss die Person sagen, wo ihr Modell zu
          // erreichen ist. Ob sie es getan hat, prüft das Programm.
          TextField(
            key: const ValueKey('field-base-url'),
            controller: _baseUrl,
            decoration: const InputDecoration(
              labelText: 'Address of your model',
              hintText: 'http://localhost:11434/v1',
              helperText:
                  'Needed. For example http://localhost:11434/v1. '
                  'The service must speak the OpenAI chat format.',
              helperMaxLines: 2,
            ),
          )
        else
          // Die übliche Adresse kennt das Programm. Eine andere braucht nur,
          // wer den Dienst über einen Umweg erreicht; deshalb eingeklappt.
          Theme(
            data: Theme.of(context).copyWith(dividerColor: Colors.transparent),
            child: ExpansionTile(
              // Je Dienst ein eigener Zustand: nach einem Wechsel ist das
              // Feld wieder zu, denn die Adresse des alten Dienstes ist fort.
              key: ValueKey('address-$_provider'),
              tilePadding: EdgeInsets.zero,
              childrenPadding: const EdgeInsets.only(bottom: 8),
              initiallyExpanded: _baseUrl.text.trim().isNotEmpty,
              title: const Text('Different address (optional)'),
              children: [
                TextField(
                  key: const ValueKey('field-base-url'),
                  controller: _baseUrl,
                  decoration: InputDecoration(
                    labelText: 'Address',
                    helperText: service.baseUrl == null
                        ? 'Leave empty for the usual address of ${service.label}.'
                        : 'Leave empty for the usual address of ${service.label}: ${service.baseUrl}',
                    helperMaxLines: 2,
                  ),
                ),
              ],
            ),
          ),
        const SizedBox(height: 16),
        TextField(
          key: const ValueKey('field-host'),
          controller: _host,
          decoration: const InputDecoration(
            labelText: 'Your Discord user ID',
            helperText:
                'In Discord: right-click your own name, then Copy User ID. '
                'Needs Developer Mode (Settings, Advanced).',
            helperMaxLines: 2,
          ),
        ),
      ],
    );
  }

  Widget _ai() {
    return _Section(
      title: 'AI',
      children: [
        Text(
          _aiNames.isEmpty
              ? 'No name: the AI only translates.'
              : 'People talk to the AI by starting a message with one of these names.',
        ),
        const SizedBox(height: 8),
        for (final (index, name) in _aiNames.indexed)
          Padding(
            padding: const EdgeInsets.only(bottom: 8),
            child: Row(
              children: [
                Expanded(
                  child: TextField(
                    key: ValueKey('ai-name-$index'),
                    controller: name,
                    decoration: InputDecoration(
                      labelText: index == 0
                          ? 'Name the AI appears under'
                          : 'Also answers to',
                    ),
                  ),
                ),
                IconButton(
                  tooltip: 'Remove this name',
                  onPressed: () => setState(() => _aiNames.removeAt(index).dispose()),
                  icon: const Icon(Icons.close),
                ),
              ],
            ),
          ),
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton.icon(
            onPressed: () => setState(() => _aiNames.add(TextEditingController())),
            icon: const Icon(Icons.add),
            label: const Text('Add name'),
          ),
        ),
      ],
    );
  }

  Widget _limits() {
    return _Section(
      title: 'Limits',
      children: [
        const Text('Everything runs on your AI key, so these are yours to set.'),
        const SizedBox(height: 8),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: TextField(
                key: const ValueKey('field-tokens'),
                controller: _tokens,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(
                  labelText: 'Translation tokens per day',
                  helperText: 'For everyone together. When used up, messages still arrive, untranslated.',
                  helperMaxLines: 2,
                ),
              ),
            ),
            const SizedBox(width: 16),
            Expanded(
              child: TextField(
                key: const ValueKey('field-questions'),
                controller: _questions,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(
                  labelText: 'Questions to the AI per guest per day',
                  helperText: 'You as the host have no limit.',
                ),
              ),
            ),
          ],
        ),
      ],
    );
  }

  Widget _roomsSection() {
    return _Section(
      title: 'Rooms',
      children: [
        const Text(
          'A room is one conversation. Every seat is a channel with a language. '
          'What is written at one seat appears at all others in their language.',
        ),
        const SizedBox(height: 4),
        Text(
          'Channel ID: in Discord, right-click the channel, then Copy Channel ID. '
          'Language as a word, for example German.',
          style: Theme.of(context).textTheme.bodySmall,
        ),
        const SizedBox(height: 12),
        for (final (r, room) in _rooms.indexed) _room(r, room),
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton.icon(
            onPressed: () => setState(
              () => _rooms.add(
                _RoomDraft(
                  const Room(
                    // Wie beim neuen Profil: Englisch ist vorbelegt.
                    passThrough: ['English'],
                    contextLines: 10,
                    seats: [
                      Seat(channel: '', lang: ''),
                      Seat(channel: '', lang: ''),
                    ],
                  ),
                ),
              ),
            ),
            icon: const Icon(Icons.add),
            label: const Text('Add room'),
          ),
        ),
      ],
    );
  }

  Widget _room(int r, _RoomDraft room) {
    final theme = Theme.of(context);
    return Container(
      margin: const EdgeInsets.only(bottom: 12),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        border: Border.all(color: theme.colorScheme.outlineVariant),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: Text('Room ${r + 1}', style: theme.textTheme.titleSmall),
              ),
              // Ein Profil ohne Raum hätte nichts zu tun.
              if (_rooms.length > 1)
                TextButton(
                  onPressed: () => setState(() => _rooms.removeAt(r).dispose()),
                  child: const Text('Remove room'),
                ),
            ],
          ),
          const SizedBox(height: 8),
          for (final (s, seat) in room.seats.indexed)
            Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.end,
                children: [
                  Expanded(
                    flex: 3,
                    child: TextField(
                      key: ValueKey('seat-$r-$s-channel'),
                      controller: seat.channel,
                      decoration: const InputDecoration(labelText: 'Channel ID'),
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    flex: 2,
                    child: TextField(
                      key: ValueKey('seat-$r-$s-lang'),
                      controller: seat.lang,
                      decoration: const InputDecoration(labelText: 'Language'),
                    ),
                  ),
                  const SizedBox(width: 12),
                  Tooltip(
                    message:
                        'Grammatical form of whoever usually sits here. Used only for grammar.',
                    child: DropdownButton<_Form>(
                      key: ValueKey('seat-$r-$s-form'),
                      value: seat.form,
                      onChanged: (form) => setState(() => seat.form = form ?? _Form.none),
                      items: const [
                        DropdownMenuItem(value: _Form.none, child: Text('no form given')),
                        DropdownMenuItem(value: _Form.female, child: Text('female')),
                        DropdownMenuItem(value: _Form.male, child: Text('male')),
                      ],
                    ),
                  ),
                  // Weniger als zwei Plätze ergeben kein Gespräch.
                  if (room.seats.length > 2)
                    IconButton(
                      tooltip: 'Remove this seat',
                      onPressed: () => setState(() => room.seats.removeAt(s).dispose()),
                      icon: const Icon(Icons.close),
                    ),
                ],
              ),
            ),
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton.icon(
              onPressed: () => setState(
                () => room.seats.add(_SeatDraft(const Seat(channel: '', lang: ''))),
              ),
              icon: const Icon(Icons.add),
              label: const Text('Add seat'),
            ),
          ),
          const SizedBox(height: 8),
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                flex: 3,
                child: TextField(
                  key: ValueKey('room-$r-pass'),
                  controller: room.passThrough,
                  decoration: const InputDecoration(
                    // Bis zum 20.11.2026 hieß das Feld „Languages that stay
                    // untranslated". Nemesus suchte nach der „Neutral-Sprache"
                    // und fand es nicht.
                    labelText: 'Neutral language (stays untranslated)',
                    helperText:
                        'For example English: a fallback everyone reads. Separate several with commas.',
                  ),
                ),
              ),
              const SizedBox(width: 16),
              Expanded(
                flex: 2,
                child: TextField(
                  key: ValueKey('room-$r-context'),
                  controller: room.contextLines,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(
                    labelText: 'Earlier messages for context',
                    helperText: 'Each one costs with every translation.',
                  ),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _peopleSection() {
    return _Section(
      title: 'People',
      children: [
        const Text(
          'Whoever is listed here keeps their own grammatical form in every channel. '
          'Everyone else gets the form of the seat they write at.',
        ),
        const SizedBox(height: 8),
        for (final (index, person) in _people.indexed)
          Padding(
            padding: const EdgeInsets.only(bottom: 8),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Expanded(
                  child: TextField(
                    key: ValueKey('person-$index-id'),
                    controller: person.id,
                    decoration: const InputDecoration(
                      labelText: 'Discord user ID',
                    ),
                  ),
                ),
                const SizedBox(width: 12),
                DropdownButton<Gender>(
                  key: ValueKey('person-$index-form'),
                  value: person.gender,
                  onChanged: (gender) =>
                      setState(() => person.gender = gender ?? person.gender),
                  items: const [
                    DropdownMenuItem(value: Gender.female, child: Text('female')),
                    DropdownMenuItem(value: Gender.male, child: Text('male')),
                  ],
                ),
                IconButton(
                  tooltip: 'Remove this person',
                  onPressed: () => setState(() => _people.removeAt(index).dispose()),
                  icon: const Icon(Icons.close),
                ),
              ],
            ),
          ),
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton.icon(
            onPressed: () => setState(() => _people.add(_PersonDraft(null))),
            icon: const Icon(Icons.add),
            label: const Text('Add person'),
          ),
        ),
      ],
    );
  }
}

class _Section extends StatelessWidget {
  const _Section({required this.title, required this.children});

  final String title;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: const EdgeInsets.only(bottom: 16),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(title, style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 12),
            ...children,
          ],
        ),
      ),
    );
  }
}
