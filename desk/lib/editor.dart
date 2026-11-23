// Ein Profil bearbeiten. Die Maske sammelt nur ein; ob das Ergebnis gültig
// ist, entscheidet das Programm beim Speichern. Lehnt es ab, steht sein Grund
// wörtlich über dem Formular, und nichts von dem Getippten geht verloren.

import 'package:material_ui/material_ui.dart';

import 'cli.dart';
import 'model.dart';
import 'paste_rooms.dart';

class ProfileEditorPage extends StatefulWidget {
  const ProfileEditorPage({
    super.key,
    required this.cli,
    required this.initial,
    required this.isNew,
    required this.existingNames,
    this.providers = fallbackProviders,
    this.providersListed = false,
    this.scrollTo,
  });

  final FishCli cli;
  final ProfileDoc initial;

  /// Der Abschnitt, bei dem das Formular aufgehen soll: „rooms" oder
  /// „people". Ohne Angabe ganz oben.
  final String? scrollTo;

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
    : door = seat.door,
      channel = TextEditingController(text: seat.channel),
      lang = TextEditingController(text: seat.lang),
      form = _formOf(seat.gender),
      owner = seat.owner ?? '';

  /// Die Tür, an der dieser Tisch steht. Seit dem 23.11.2026 wählt sie jeder
  /// Tisch für sich: ein Raum kann Tische an Discord und an Matrix haben.
  String door;
  final TextEditingController channel;
  final TextEditingController lang;
  _Form form;

  /// Wem der Tisch gehört: der Name des Menschen, sonst seine Kennung (siehe
  /// [_PersonDraft.ref]). Leer: niemandem im Besonderen.
  String owner;

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
  _PersonDraft(Person? person, this._doorsInUse)
    : name = TextEditingController(text: person?.name ?? ''),
      ids = {
        for (final door in knownDoors) door: TextEditingController(text: person?.idAt(door) ?? ''),
      },
      lang = TextEditingController(text: person?.lang ?? ''),
      neutral = TextEditingController(text: person?.neutral?.join(', ') ?? ''),
      // Bis zum 20.11.2026 begann ein neuer Mensch mit der weiblichen Form,
      // weil eine Form Pflicht war. Seitdem ist sie freiwillig.
      form = _formOf(person?.gender),
      formality = person?.formality ?? '',
      invited = person?.invited ?? false,
      _nothingNeutral = person?.neutral?.isEmpty ?? false {
    lastRef = ref;
  }

  /// Der Name, den der Gastgeber diesem Menschen gibt.
  final TextEditingController name;

  /// Die Türen, an denen gerade Tische stehen. Der Entwurf fragt das
  /// Formular: mit jedem Tisch, der die Tür wechselt, kann es sich ändern.
  final List<String> Function() _doorsInUse;

  /// Womit Tische auf diesen Menschen zeigen: mit seinem Namen, sonst mit
  /// seiner Kennung an einer Tür, an der Tische stehen. So schreibt es auch
  /// das Programm ins Profil.
  String get ref {
    if (name.text.trim().isNotEmpty) {
      return name.text.trim();
    }
    for (final door in _doorsInUse()) {
      if (idAt(door).isNotEmpty) {
        return idAt(door);
      }
    }
    // Eine Kennung nur an einer Tür ohne Tisch: der Mensch bleibt stehen, und
    // das Programm sagt beim Speichern, was ihm fehlt.
    for (final door in knownDoors) {
      if (idAt(door).isNotEmpty) {
        return idAt(door);
      }
    }
    return '';
  }

  /// Worauf die Tische zuletzt zeigten. Ändert sich [ref], weil jemand den
  /// Namen tippt, ziehen die Tische dieses Menschen mit.
  String lastRef = '';

  /// Die Kennung je Tür. Ein Feld bekommt jede Tür, an der Tische stehen; die
  /// Kennung an einer anderen steht in keinem Feld, bleibt aber erhalten: das
  /// bloße Öffnen und Speichern darf keine verlieren.
  final Map<String, TextEditingController> ids;

  /// Die Kennung dieses Menschen an dieser Tür, wie sie gerade dasteht.
  String idAt(String door) => ids[door]?.text.trim() ?? '';

  /// Zeigt diese Angabe (ein Name oder eine Kennung) auf diesen Menschen?
  bool answersTo(String what) =>
      what.isNotEmpty && (name.text.trim() == what || knownDoors.any((door) => idAt(door) == what));

  final TextEditingController lang;
  final TextEditingController neutral;
  _Form form;

  /// Wie dieser Mensch angesprochen wird: leer (wie geschrieben), vertraut
  /// oder respektvoll, in den Worten des Programms.
  String formality;
  bool invited;

  /// Das Profil sagte ausdrücklich: für diesen Menschen bleibt nichts
  /// unübersetzt (eine leere Liste). Im Feld sieht das aus wie „keine Angabe".
  /// Damit das bloße Öffnen und Speichern daraus nicht „wie im Raum" macht,
  /// merkt sich der Entwurf den Unterschied.
  final bool _nothingNeutral;

  /// Die neutralen Sprachen, so wie sie ins Profil gehen. Ein leeres Feld
  /// heißt: es gilt, was im Raum steht.
  List<String>? get neutralLangs {
    final langs = [
      for (final lang in neutral.text.split(','))
        if (lang.trim().isNotEmpty) lang.trim(),
    ];
    if (langs.isEmpty) {
      return _nothingNeutral ? const [] : null;
    }
    return langs;
  }

  void dispose() {
    name.dispose();
    for (final id in ids.values) {
      id.dispose();
    }
    lang.dispose();
    neutral.dispose();
  }
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

  /// Die Türen, an denen gerade Tische stehen, Discord vor Matrix. Danach
  /// richten sich die Felder: je Tür ein Kennungsfeld bei den Menschen, und
  /// die Erklärungen nennen, was es an diesen Türen braucht.
  ///
  /// Bis zum 23.11.2026 wählte ein Profil seine Tür einmal, unter General.
  /// Entschieden von Nemesus an dem Tag: die Tür wird je Tisch gewählt.
  List<String> _doorsInUse() {
    final used = {for (final seat in _rooms.expand((room) => room.seats)) seat.door};
    return [
      for (final door in knownDoors)
        if (used.contains(door)) door,
      // Eine Tür, die diese Maske nicht kennt, zählt auch.
      for (final door in used)
        if (!knownDoors.contains(door)) door,
    ];
  }

  /// Die eine Tür, an der alle Tische stehen, oder `null` in einem Profil
  /// mit Tischen an mehreren.
  String? get _onlyDoor {
    final doors = _doorsInUse();
    return doors.length == 1 ? doors.single : null;
  }

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
    _people = [for (final person in doc.people) _watched(_PersonDraft(person, _doorsInUse))];
    _pointSeatsAtPeople();
    // Der Hinweis zum Namen erscheint schon beim Tippen.
    _name.addListener(_redraw);
    // Die Auswahl „Whose table" nennt den Gastgeber mit seiner Kennung, und
    // der Haken „Invited" hängt davon ab, wer der Gastgeber ist.
    _host.addListener(_redraw);
    final section = widget.scrollTo;
    if (section != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) => _show(section));
    }
    // Wer nach einem Wechsel des Dienstes das Modell anfasst, hat den Hinweis
    // dazu gesehen. Er verschwindet dann.
    _model.addListener(_modelTouched);
  }

  void _modelTouched() {
    if (_modelFromOtherService && mounted) {
      setState(() => _modelFromOtherService = false);
    }
  }

  /// Ein Profil kann mit dem Namen oder mit der Kennung auf einen Menschen
  /// zeigen. Im Formular zeigt jeder Tisch einheitlich mit [_PersonDraft.ref]
  /// auf seinen Besitzer, damit die Auswahl ihn findet.
  void _pointSeatsAtPeople() {
    for (final seat in _rooms.expand((room) => room.seats)) {
      for (final person in _people) {
        if (person.answersTo(seat.owner)) {
          seat.owner = person.ref;
          break;
        }
      }
    }
  }

  /// Setzt Räume ein, die jemand weitergegeben hat, samt den Menschen an
  /// ihren Tischen. Was bis dahin im Formular an Räumen und Menschen stand,
  /// wird ersetzt: zwei Fische in einem Raum müssen denselben Raum eintragen.
  ///
  /// Niemand ist danach eingeladen, und wer der Gastgeber ist, hat der
  /// Einfügende im Dialog selbst gesagt.
  Future<void> _pasteRooms() async {
    final pasted = await showDialog<PastedRooms>(
      context: context,
      builder: (_) => const PasteRoomsDialog(),
    );
    if (pasted == null || !mounted) {
      return;
    }
    setState(() {
      for (final room in _rooms) {
        room.dispose();
      }
      for (final person in _people) {
        person.dispose();
      }
      // Die Türen kommen mit den Räumen: jeder Tisch bringt seine mit.
      _rooms
        ..clear()
        ..addAll([for (final room in pasted.shared.rooms) _RoomDraft(room)]);
      _people
        ..clear()
        ..addAll([
          for (final person in pasted.shared.people) _watched(_PersonDraft(person, _doorsInUse)),
        ]);
      _pointSeatsAtPeople();
      final me = pasted.me;
      if (me != null) {
        _host.text = me;
      }
    });
  }

  final _roomsKey = GlobalKey();
  final _peopleKey = GlobalKey();

  /// Rollt das Formular zu einem Abschnitt.
  void _show(String section) {
    final target = switch (section) {
      'rooms' => _roomsKey.currentContext,
      'people' => _peopleKey.currentContext,
      _ => null,
    };
    if (target != null) {
      Scrollable.ensureVisible(target);
    }
  }

  /// Kennung und Muttersprache eines Menschen stehen auch in der Auswahl
  /// „Whose table". Ändert sich dort etwas, muss die Auswahl es zeigen.
  _PersonDraft _watched(_PersonDraft person) {
    void followed() => _followRef(person);
    person.name.addListener(followed);
    for (final id in person.ids.values) {
      id.addListener(followed);
    }
    person.lang.addListener(_redraw);
    return person;
  }

  /// Der Name oder die Kennung eines Menschen hat sich geändert. Die Tische,
  /// die auf ihn zeigten, zeigen weiter auf ihn.
  void _followRef(_PersonDraft person) {
    final before = person.lastRef;
    final now = person.ref;
    // Solange ein Mensch für einen Augenblick weder Namen noch Kennung hat,
    // etwa weil jemand erst die Kennung leert und dann den Namen tippt,
    // bleiben seine Tische, wo sie sind, und ziehen mit, sobald wieder etwas
    // dasteht. Gemessen von Nemesus am 21.11.2026: bis dahin verlor der Tisch
    // in dieser Reihenfolge seinen Besitzer.
    if (now.isNotEmpty && before != now) {
      // Nur wenn kein anderer Mensch dieselbe Angabe trägt: sonst wäre nicht
      // zu sagen, wessen Tische mitziehen sollen.
      final shared = _people.any((other) => other != person && other.ref == before);
      if (before.isNotEmpty && !shared) {
        for (final seat in _rooms.expand((room) => room.seats)) {
          if (seat.owner == before) {
            seat.owner = now;
          }
        }
      }
      person.lastRef = now;
    }
    _redraw();
  }

  /// Der Gastgeber, wie er im Feld steht: seine Kennung oder sein Name.
  String get _hostId => _host.text.trim();

  /// Ist dieser Mensch der Gastgeber?
  bool _isHost(_PersonDraft person) => person.answersTo(_hostId);

  /// Der Mensch, auf den ein Tisch zeigt, so wie er gerade im Formular steht.
  _PersonDraft? _personAt(String ref) {
    if (ref.isEmpty) {
      return null;
    }
    for (final person in _people) {
      if (person.ref == ref) {
        return person;
      }
    }
    return null;
  }

  /// Die Muttersprache eines Menschen, so wie sie gerade im Formular steht.
  String? _motherTongue(String ref) {
    final lang = _personAt(ref)?.lang.text.trim() ?? '';
    return lang.isEmpty ? null : lang;
  }

  /// Wer als Besitzer eines Tisches zur Wahl steht: niemand, der Gastgeber und
  /// die Menschen aus „People". Gehört der Tisch jemandem, den das Formular
  /// nicht (mehr) nennt, bleibt er als eigener Eintrag stehen. Sonst würde ein
  /// Profil beim bloßen Öffnen und Speichern still umgestellt.
  List<DropdownMenuItem<String>> _owners(String current) {
    String withTongue(String label, String id) {
      final tongue = _motherTongue(id);
      return tongue == null ? label : '$label · $tongue';
    }

    final host = _hostId;
    // Jede Angabe einmal: eine Auswahl verträgt keinen Wert doppelt, auch
    // nicht für den Augenblick, in dem zwei Menschen gleich heißen.
    final choices = <String, String>{};
    // Der Gastgeber zuerst. Steht er nicht unter den Menschen, zeigt ein
    // Tisch mit seiner Kennung auf ihn, wie vor dem 21.11.2026.
    for (final person in _people.where(_isHost)) {
      if (person.ref.isNotEmpty) {
        choices.putIfAbsent(person.ref, () => 'You (host) · ${person.ref}');
      }
    }
    if (choices.isEmpty && host.isNotEmpty) {
      choices[host] = 'You (host) · $host';
    }
    for (final person in _people) {
      if (person.ref.isNotEmpty) {
        choices.putIfAbsent(person.ref, () => person.ref);
      }
    }
    return [
      const DropdownMenuItem(value: '', child: Text('nobody in particular')),
      for (final MapEntry(key: ref, value: label) in choices.entries)
        DropdownMenuItem(
          value: ref,
          child: Text(withTongue(label, ref), overflow: TextOverflow.ellipsis),
        ),
      if (current.isNotEmpty && !choices.containsKey(current))
        DropdownMenuItem(
          value: current,
          child: Text('$current (not listed under People)', overflow: TextOverflow.ellipsis),
        ),
    ];
  }

  /// Ein Tisch wechselt die Tür. Seine Kennung bleibt stehen; sie gilt an der
  /// neuen Tür nicht und ist neu einzutragen. Bei den Menschen erscheint ein
  /// Kennungsfeld für jede Tür, an der jetzt Tische stehen.
  void _chooseSeatDoor(_SeatDraft seat, String door) {
    if (door == seat.door) {
      return;
    }
    setState(() {
      seat.door = door;
      // Wer ohne Namen eingetragen ist, heißt nach seiner Kennung an einer
      // benutzten Tür. Ändert sich das, ziehen seine Tische mit.
      for (final person in _people) {
        _followRef(person);
      }
    });
  }

  /// Die Tür, an der ein neuer Tisch beginnt: die des Tisches davor.
  String _doorForNewSeat(_RoomDraft? room) {
    if (room != null && room.seats.isNotEmpty) {
      return room.seats.last.door;
    }
    final seats = _rooms.expand((room) => room.seats);
    return seats.isEmpty ? discordDoor : seats.last.door;
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
                door: seat.door,
                channel: seat.channel.text.trim(),
                lang: seat.lang.text.trim(),
                gender: _genderOf(seat.form),
                owner: seat.owner.isEmpty ? null : seat.owner,
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
        // Eine Zeile ohne Namen und ohne Kennung ist über niemanden.
        for (final person in _people)
          if (person.ref.isNotEmpty)
            Person(
              id: person.idAt(discordDoor),
              matrix: person.idAt(matrixDoor),
              name: person.name.text.trim().isEmpty ? null : person.name.text.trim(),
              gender: _genderOf(person.form),
              formality: person.formality.isEmpty ? null : person.formality,
              lang: person.lang.text.trim().isEmpty ? null : person.lang.text.trim(),
              neutral: person.neutralLangs,
              invited: person.invited,
            ),
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
          decoration: InputDecoration(
            // Stehen alle Tische an einer Tür, heißt das Feld nach ihr, wie
            // bisher. In einem Profil mit Tischen an beiden genügt die
            // Kennung an einer.
            labelText: switch (_onlyDoor) {
              final door? => 'Your ${doorLabel(door)} user ID',
              null => 'Your user ID',
            },
            helperText: switch (_onlyDoor) {
              matrixDoor =>
                'Looks like @name:server. Your Matrix app shows it in your own profile.',
              null =>
                'Your Discord user ID (digits) or your Matrix user ID (@name:server). '
                    'One of the two is enough; the other can stand under People.',
              _ =>
                'In Discord: right-click your own name, then Copy User ID. '
                    'Needs Developer Mode (Settings, Advanced).',
            },
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
          // Gemessen von Nemesus am 22.11.2026: ein Fisch ohne Rufnamen
          // schwieg auf jede Ansprache, und nichts sagte, warum.
          _aiNames.isEmpty
              ? 'No call name: the AI only translates and answers nobody. '
                    'Add a name if you want to talk to it.'
              : 'You and the people you invited talk to the AI by starting a message '
                    'with one of these names. Someone who brought a fish of their own '
                    'talks to their own AI; give two fish in one room different names.',
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
      key: _roomsKey,
      title: 'Rooms',
      children: [
        // Bis zum 20.11.2026: „Every seat is a channel with a language." Seit
        // ein Platz der Tisch eines Menschen sein kann, steht das Wort dabei.
        Text(switch (_onlyDoor) {
          matrixDoor =>
            'A room is one conversation. Every seat is a table: a Matrix room in one language. '
                'What is written at one table appears at all others in their language.',
          null =>
            'A room is one conversation. Every seat is a table: a Discord channel or a Matrix '
                'room in one language. What is written at one table appears at all others in '
                'their language, whichever door they stand at.',
          _ =>
            'A room is one conversation. Every seat is a table: a channel in one language. '
                'What is written at one table appears at all others in their language.',
        }),
        const SizedBox(height: 4),
        Text(
          'Door: where the table stands, a channel on Discord or a room on Matrix. '
          'The tables of one room may stand at different doors.',
          style: Theme.of(context).textTheme.bodySmall,
        ),
        const SizedBox(height: 4),
        if (_doorsInUse().contains(discordDoor))
          Text(
            'Channel ID: in Discord, right-click the channel, then Copy Channel ID. '
            'Language as a word, for example German.',
            style: Theme.of(context).textTheme.bodySmall,
          ),
        if (_doorsInUse().contains(matrixDoor))
          Text(
            'Room ID: the internal ID of the Matrix room, which starts with ! (in Element: '
            'room settings, Advanced). Invite the account of the fish into each room. '
            'Language as a word, for example German.',
            style: Theme.of(context).textTheme.bodySmall,
          ),
        const SizedBox(height: 4),
        Text(
          'Whose table: the person who reads there. Their mother tongue then decides the '
          'language. Your fish reads at every table, but writes only at yours and at '
          'those of people you invited.',
          style: Theme.of(context).textTheme.bodySmall,
        ),
        // Wer mit dem eigenen Fisch in den Raum eines anderen kommt, muss
        // denselben Raum eintragen. Abtippen heißt: ein Zahlendreher, und
        // nichts passt, ohne dass etwas es sagt. Deshalb einfügen.
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton.icon(
            key: const ValueKey('paste-rooms'),
            onPressed: _pasteRooms,
            icon: const Icon(Icons.content_paste, size: 18),
            label: const Text('Paste rooms someone shared with you'),
          ),
        ),
        const SizedBox(height: 4),
        for (final (r, room) in _rooms.indexed) _room(r, room),
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton.icon(
            onPressed: () => setState(
              () => _rooms.add(
                _RoomDraft(
                  Room(
                    // Wie beim neuen Profil: Englisch ist vorbelegt.
                    passThrough: const ['English'],
                    contextLines: 10,
                    seats: [
                      Seat(door: _doorForNewSeat(null), channel: '', lang: ''),
                      Seat(door: _doorForNewSeat(null), channel: '', lang: ''),
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
          for (final (s, seat) in room.seats.indexed) _seat(r, room, s, seat),
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton.icon(
              onPressed: () => setState(
                () => room.seats.add(
                  _SeatDraft(Seat(door: _doorForNewSeat(room), channel: '', lang: '')),
                ),
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

  /// Die Auswahl der grammatischen Form, in derselben Gestalt wie die Felder
  /// daneben, damit eine Zeile aus Feldern und Auswahlen auf einer Linie steht.
  Widget _formChoice({
    required Key key,
    required _Form value,
    required String tooltip,
    required ValueChanged<_Form> onChanged,
  }) {
    return SizedBox(
      width: 150,
      child: Tooltip(
        message: tooltip,
        child: InputDecorator(
          decoration: const InputDecoration(labelText: 'Form'),
          child: DropdownButtonHideUnderline(
            child: DropdownButton<_Form>(
              key: key,
              value: value,
              isDense: true,
              isExpanded: true,
              onChanged: (form) => onChanged(form ?? _Form.none),
              items: const [
                DropdownMenuItem(value: _Form.none, child: Text('no form given')),
                DropdownMenuItem(value: _Form.female, child: Text('female')),
                DropdownMenuItem(value: _Form.male, child: Text('male')),
              ],
            ),
          ),
        ),
      ),
    );
  }

  /// Ein Platz: Kanal, Sprache, wessen Tisch es ist, und die Form.
  Widget _seat(int r, _RoomDraft room, int s, _SeatDraft seat) {
    final tongue = _motherTongue(seat.owner);
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: Row(
        // Oben bündig: unter dem Sprachfeld kann ein Hinweis stehen, und die
        // Felder daneben sollen dann nicht nach unten rutschen.
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 120,
            child: InputDecorator(
              decoration: const InputDecoration(labelText: 'Door'),
              child: DropdownButtonHideUnderline(
                child: DropdownButton<String>(
                  key: ValueKey('seat-$r-$s-door'),
                  value: seat.door,
                  isDense: true,
                  isExpanded: true,
                  onChanged: (door) => _chooseSeatDoor(seat, door ?? seat.door),
                  items: [
                    const DropdownMenuItem(value: discordDoor, child: Text('Discord')),
                    const DropdownMenuItem(value: matrixDoor, child: Text('Matrix')),
                    // Eine Tür, die diese Maske nicht kennt, bleibt stehen:
                    // sonst würde ein Profil beim bloßen Öffnen und Speichern
                    // umgestellt.
                    if (!knownDoors.contains(seat.door))
                      DropdownMenuItem(value: seat.door, child: Text(doorLabel(seat.door))),
                  ],
                ),
              ),
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            flex: 3,
            child: TextField(
              key: ValueKey('seat-$r-$s-channel'),
              controller: seat.channel,
              decoration: InputDecoration(
                labelText: seat.door == matrixDoor ? 'Matrix room ID' : 'Discord channel ID',
              ),
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            flex: 2,
            child: TextField(
              key: ValueKey('seat-$r-$s-lang'),
              controller: seat.lang,
              decoration: InputDecoration(
                labelText: 'Language',
                // Welche Sprache gilt, entscheidet das Programm: die
                // Muttersprache des Besitzers, sonst die des Platzes. Die
                // Maske sagt es nur, damit niemand am falschen Feld dreht.
                helperText: tongue == null
                    ? null
                    : 'The mother tongue of the owner applies: $tongue',
                helperMaxLines: 2,
              ),
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            flex: 3,
            child: InputDecorator(
              decoration: const InputDecoration(labelText: 'Whose table'),
              child: DropdownButtonHideUnderline(
                child: DropdownButton<String>(
                  key: ValueKey('seat-$r-$s-owner'),
                  value: seat.owner,
                  isDense: true,
                  isExpanded: true,
                  onChanged: (id) => setState(() => seat.owner = id ?? ''),
                  items: _owners(seat.owner),
                ),
              ),
            ),
          ),
          const SizedBox(width: 12),
          _formChoice(
            key: ValueKey('seat-$r-$s-form'),
            value: seat.form,
            tooltip: 'Grammatical form of whoever usually sits here. Used only for grammar.',
            onChanged: (form) => setState(() => seat.form = form),
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
    );
  }

  Widget _peopleSection() {
    return _Section(
      key: _peopleKey,
      title: 'People',
      children: [
        // Bis zum 20.11.2026 stand hier nur die grammatische Form. Nemesus am
        // selben Tag: „ob es nicht sinnvoller wäre, pro Discord-Nummer die
        // Muttersprache und die Neutralsprache festzulegen."
        const Text(
          'Everyone listed here keeps their own language and grammatical form at every table. '
          'Everyone else gets what the seat says where they write.',
        ),
        const SizedBox(height: 4),
        Text(switch (_onlyDoor) {
          final door? =>
            'Name: your own label for this person, so you can see whose table is whose. '
                'It stays in this profile; neither ${doorLabel(door)} nor the AI gets it. '
                'The ${doorLabel(door)} user ID can follow later.',
          null =>
            'Name: your own label for this person, so you can see whose table is whose. '
                'It stays in this profile; neither Discord nor Matrix nor the AI gets it. '
                'The user IDs can follow later; a person needs one only where they write.',
        }, style: Theme.of(context).textTheme.bodySmall),
        const SizedBox(height: 4),
        Text(
          'Formality: how this person is addressed in what they read at their table, for '
          'languages that tell friends from elders, such as du and Sie or the levels of Korean '
          'and Japanese. "as written" follows what the writer wrote.',
          style: Theme.of(context).textTheme.bodySmall,
        ),
        const SizedBox(height: 4),
        Text(
          'Invited: your fish translates for this person, on your key. '
          'You as the host need no tick.',
          style: Theme.of(context).textTheme.bodySmall,
        ),
        const SizedBox(height: 12),
        for (final (index, person) in _people.indexed)
          _person(index, person, isHost: _isHost(person)),
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton.icon(
            onPressed: () => setState(() => _people.add(_watched(_PersonDraft(null, _doorsInUse)))),
            icon: const Icon(Icons.add),
            label: const Text('Add person'),
          ),
        ),
      ],
    );
  }

  /// Ein Mensch: Kennung, Muttersprache, neutrale Sprachen, Form, Einladung.
  Widget _person(int index, _PersonDraft person, {required bool isHost}) {
    // Je Tür, an der Tische stehen, ein Kennungsfeld. Das Feld heißt nach
    // seiner Tür.
    final ids = [
      for (final door in _doorsInUse())
        if (person.ids[door] case final id?)
          Expanded(
            flex: 3,
            child: TextField(
              key: ValueKey('person-$index-$door'),
              controller: id,
              decoration: InputDecoration(labelText: '${doorLabel(door)} user ID'),
            ),
          ),
    ];
    // Zwei Zeilen je Mensch. Oben, wer er ist: Name, Muttersprache, neutrale
    // Sprachen, Form, Einladung. Darunter, wie man ihn erreicht und anspricht:
    // seine Kennungen und die Anrede.
    //
    // Bis zum 23.11.2026 stand die Kennung in der oberen Zeile. Mit zwei
    // Kennungen (gemischter Raum) wurden dort bei 1280 Punkten Fensterbreite
    // die Beschriftungen abgeschnitten, und mit der Anrede stünde bei einer
    // Kennung ein einzelnes Feld allein in der zweiten Zeile.
    final row = Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(
          flex: 2,
          child: TextField(
            key: ValueKey('person-$index-name'),
            controller: person.name,
            decoration: InputDecoration(
              labelText: 'Name',
              hintText: 'Robin',
              helperText: isHost ? 'This is you, the host.' : null,
              helperMaxLines: 2,
            ),
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          flex: 2,
          child: TextField(
            key: ValueKey('person-$index-lang'),
            controller: person.lang,
            decoration: const InputDecoration(labelText: 'Mother tongue', hintText: 'Russian'),
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          flex: 3,
          child: TextField(
            key: ValueKey('person-$index-neutral'),
            controller: person.neutral,
            decoration: const InputDecoration(
              labelText: 'Neutral languages',
              helperText: 'Stay untranslated for this person. Empty: as in the room.',
              helperMaxLines: 3,
            ),
          ),
        ),
        const SizedBox(width: 12),
        _formChoice(
          key: ValueKey('person-$index-form'),
          value: person.form,
          tooltip: 'Grammatical form of this person. Used only for grammar.',
          onChanged: (form) => setState(() => person.form = form),
        ),
        const SizedBox(width: 4),
        Tooltip(
          message: isHost
              ? 'Your fish always translates for you.'
              : 'Your fish translates for whoever you invite, on your key.',
          child: Padding(
            // Auf die Höhe der Feldzeile gesetzt, nicht auf die des Etiketts.
            padding: const EdgeInsets.only(top: 8),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Checkbox(
                  key: ValueKey('person-$index-invited'),
                  // Der Gastgeber braucht keinen Haken: sein Fisch übersetzt
                  // ohnehin für ihn. Gezeigt wird er gesetzt und fest; im
                  // Profil ändert das nichts.
                  value: isHost || person.invited,
                  onChanged: isHost
                      ? null
                      : (ticked) => setState(() => person.invited = ticked ?? false),
                ),
                const Text('Invited'),
              ],
            ),
          ),
        ),
        IconButton(
          tooltip: 'Remove this person',
          onPressed: () => setState(() => _people.removeAt(index).dispose()),
          icon: const Icon(Icons.close),
        ),
      ],
    );
    // Wie dieser Mensch angesprochen wird.
    final formality = SizedBox(
      width: 190,
      child: InputDecorator(
        decoration: const InputDecoration(labelText: 'Formality'),
        child: DropdownButtonHideUnderline(
          child: DropdownButton<String>(
            key: ValueKey('person-$index-formality'),
            value: person.formality,
            isDense: true,
            isExpanded: true,
            onChanged: (how) => setState(() => person.formality = how ?? ''),
            items: [
              const DropdownMenuItem(value: '', child: Text('as written')),
              const DropdownMenuItem(value: familiarFormality, child: Text('familiar')),
              const DropdownMenuItem(value: respectfulFormality, child: Text('respectful')),
              // Ein Wert, den diese Maske nicht kennt, bleibt stehen: sonst
              // würde ein Profil beim bloßen Öffnen und Speichern umgestellt.
              if (!const ['', familiarFormality, respectfulFormality].contains(person.formality))
                DropdownMenuItem(value: person.formality, child: Text(person.formality)),
            ],
          ),
        ),
      ),
    );
    return Padding(
      // Mit zwei Zeilen je Mensch braucht es mehr Abstand zum nächsten.
      padding: const EdgeInsets.only(bottom: 24),
      child: Column(
        children: [
          row,
          const SizedBox(height: 8),
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              for (final id in ids) ...[id, const SizedBox(width: 12)],
              formality,
            ],
          ),
        ],
      ),
    );
  }
}

class _Section extends StatelessWidget {
  const _Section({super.key, required this.title, required this.children});

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
