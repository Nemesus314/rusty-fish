// Tests der Teile, die ohne Fenster auskommen.

import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart' show Brightness, ThemeMode;
import 'package:rusty_fish_desk/about.dart';
import 'package:rusty_fish_desk/cli.dart';
import 'package:rusty_fish_desk/desk_state.dart';
import 'package:rusty_fish_desk/log_file.dart';
import 'package:rusty_fish_desk/model.dart';
import 'package:rusty_fish_desk/theme_choice.dart';

void main() {
  group('locating the program', () {
    const own = r'D:\repo\desk\build\windows\x64\runner\Debug\rusty_fish_desk.exe';

    test('the environment variable wins when the file exists', () {
      final found = locateFish(
        environment: {'RUSTY_FISH_EXE': r'E:\tools\rusty-fish.exe'},
        ownExecutable: own,
        exists: (path) => path == r'E:\tools\rusty-fish.exe',
      );
      expect(found.found, r'E:\tools\rusty-fish.exe');
    });

    test('next to the own exe comes before the build folder', () {
      const beside = r'D:\repo\desk\build\windows\x64\runner\Debug\rusty-fish.exe';
      final found = locateFish(
        environment: const {},
        ownExecutable: own,
        exists: (path) => path == beside || path == r'D:\repo\target\debug\rusty-fish.exe',
      );
      expect(found.found, beside);
    });

    test('the search walks up to the build folder of the repository', () {
      final found = locateFish(
        environment: const {},
        ownExecutable: own,
        exists: (path) => path == r'D:\repo\target\debug\rusty-fish.exe',
      );
      expect(found.found, r'D:\repo\target\debug\rusty-fish.exe');
    });

    test('when nothing is found, every checked place is listed', () {
      final found = locateFish(
        environment: {'RUSTY_FISH_EXE': r'E:\gone\rusty-fish.exe'},
        ownExecutable: r'D:\a\b\desk.exe',
        exists: (_) => false,
      );
      expect(found.found, isNull);
      expect(found.checked, [
        r'E:\gone\rusty-fish.exe',
        r'D:\a\b\rusty-fish.exe',
        r'D:\a\b\target\debug\rusty-fish.exe',
        r'D:\a\target\debug\rusty-fish.exe',
        r'D:\target\debug\rusty-fish.exe',
      ]);
    });

    test('on other systems the file has no .exe ending', () {
      final found = locateFish(
        environment: const {},
        ownExecutable: '/opt/fish/desk',
        exists: (path) => path == '/opt/fish/rusty-fish',
        windows: false,
      );
      expect(found.found, '/opt/fish/rusty-fish');
    });
  });

  test('a log line is stamped with date and time, padded', () {
    expect(logStamp(DateTime(2026, 11, 21, 3, 2, 52)), '2026-11-21 03:02:52');
    expect(logStamp(DateTime(2027, 1, 5, 23, 59, 9)), '2027-01-05 23:59:09');
  });

  group('the log file', () {
    late Directory home;

    setUp(() {
      home = Directory.systemTemp.createTempSync('rf-log-');
      Directory('${home.path}${Platform.pathSeparator}profiles').createSync();
    });

    tearDown(() => home.deleteSync(recursive: true));

    String profiles() => '${home.path}${Platform.pathSeparator}profiles';

    test('lies beside the profiles and keeps what earlier runs wrote', () {
      final first = openLogFile(profiles(), 'study');
      first.write('one');
      final second = openLogFile(profiles(), 'study');
      second.write('two');

      final sep = Platform.pathSeparator;
      expect(second.path, '${home.path}${sep}logs${sep}study.log');
      expect(File(second.path).readAsStringSync(), 'one\ntwo\n');
      // Im Ordner der Profile selbst liegt nichts Neues.
      expect(Directory(profiles()).listSync(), isEmpty);
    });

    test('a file that grew too large is set aside, and only one old one stays', () {
      final sep = Platform.pathSeparator;
      final old = File('${home.path}${sep}logs${sep}study.old.log');

      openLogFile(profiles(), 'study', maxBytes: 5).write('first run');
      openLogFile(profiles(), 'study', maxBytes: 5).write('second run');
      expect(old.readAsStringSync(), 'first run\n');

      final third = openLogFile(profiles(), 'study', maxBytes: 5);
      third.write('third run');
      expect(old.readAsStringSync(), 'second run\n');
      expect(File(third.path).readAsStringSync(), 'third run\n');
    });
  });

  test('numbers are grouped the English way', () {
    expect(groupThousands(0), '0');
    expect(groupThousands(999), '999');
    expect(groupThousands(3231), '3,231');
    expect(groupThousands(100000), '100,000');
    expect(groupThousands(1234567), '1,234,567');
  });

  test('profile names follow the rule of the program', () {
    for (final good in ['tea', 'tea-time', 'a_b', '0', 'x' * 40]) {
      expect(profileNamePattern.hasMatch(good), isTrue, reason: good);
    }
    for (final bad in ['', 'Tea', 'tea time', 'tee.toml', 'чай', 'x' * 41]) {
      expect(profileNamePattern.hasMatch(bad), isFalse, reason: bad);
    }
  });

  test('a listed profile is read in the form the program sends', () {
    final list = ProfileList.fromJson(
      jsonDecode('''
      {"dir": "C:\\\\fish", "profiles": [
        {"name": "tea", "path": "C:\\\\fish\\\\tea.toml", "ok": true, "error": null,
         "model": "m", "base_url": null, "host": "100", "ai_names": ["Fish", "Фиш"],
         "limits": {"translation_tokens_per_day": 100000, "questions_per_guest_per_day": 10},
         "rooms": [{"pass_through": ["English"], "context_lines": 10,
                    "seats": [{"channel": "111", "lang": "German", "gender": "male"},
                              {"channel": "222", "lang": "Russian", "gender": null}]}],
         "people": [{"id": "200", "gender": "female"}],
         "usage": {"day": "2026-11-20", "translation_tokens": 3231, "allowed_today": 102500},
         "running": true},
        {"name": "old", "path": "C:\\\\fish\\\\old.toml", "ok": false,
         "error": "configuration: no `[[room]]` is defined", "usage": null, "running": false}
      ]}''')
          as Map<String, dynamic>,
    );
    expect(list.dir, r'C:\fish');
    final tea = list.profiles[0];
    expect(tea.ok, isTrue);
    expect(tea.running, isTrue);
    expect(tea.doc!.aiNames, ['Fish', 'Фиш']);
    expect(tea.doc!.rooms.single.seats[0].gender, Gender.male);
    expect(tea.doc!.rooms.single.seats[1].gender, isNull);
    expect(tea.doc!.people.single.gender, Gender.female);
    expect(tea.usage!.allowedToday, 102500);

    final old = list.profiles[1];
    expect(old.ok, isFalse);
    expect(old.doc, isNull);
    expect(old.error, contains('no `[[room]]`'));
  });

  test('a profile goes out in the form the program reads', () {
    const doc = ProfileDoc(
      name: 'tea',
      model: 'm',
      baseUrl: null,
      host: '100',
      aiNames: ['Fish'],
      limits: Limits(translationTokensPerDay: 5, questionsPerGuestPerDay: 2),
      rooms: [
        Room(
          passThrough: ['English'],
          contextLines: 3,
          seats: [
            Seat(channel: '111', lang: 'German', gender: Gender.male),
            Seat(channel: '222', lang: 'Russian'),
          ],
        ),
      ],
      people: [Person(id: '200', gender: Gender.female)],
    );
    final json = doc.toJson();
    expect(json.keys, [
      'name',
      'provider',
      'model',
      'base_url',
      'host',
      'ai_names',
      'limits',
      'rooms',
      'people',
    ]);
    expect(json['base_url'], isNull);
    // Ohne Angabe im Profil gilt der Dienst, den es vor der Auswahl gab.
    expect(json['provider'], 'openai');
    expect((json['rooms'] as List).single['seats'][1]['gender'], isNull);
    // Hin und zurück bleibt es dasselbe Profil.
    final again = ProfileDoc.fromJson(
      jsonDecode(jsonEncode(json)) as Map<String, dynamic>,
    );
    expect(again.toJson(), json);
  });

  group('AI services', () {
    test('the list of services is read in the form the program sends', () {
      final list = [
        for (final item
            in jsonDecode('''
          [{"id": "openai", "label": "ChatGPT (OpenAI)",
            "base_url": "https://api.openai.com/v1", "needs_key": true},
           {"id": "custom", "label": "Own model or other service",
            "base_url": null, "needs_key": false},
           {"id": "bare"}]''')
                as List)
          Provider.fromJson(item as Map<String, dynamic>),
      ];
      expect(list[0].id, 'openai');
      expect(list[0].label, 'ChatGPT (OpenAI)');
      expect(list[0].baseUrl, 'https://api.openai.com/v1');
      expect(list[0].needsKey, isTrue);
      expect(list[0].needsAddress, isFalse);

      expect(list[1].baseUrl, isNull);
      expect(list[1].needsKey, isFalse);
      expect(list[1].needsAddress, isTrue);

      // Fehlt eine Angabe, bleibt der Eintrag lesbar: die Kennung dient als
      // Name, und im Zweifel wird nach einem Schlüssel gefragt.
      expect(list[2].label, 'bare');
      expect(list[2].needsKey, isTrue);
    });

    test('a profile names its service, and none means the first there was', () {
      Map<String, dynamic> profile(String? provider) => {
        'name': 'p',
        'provider': ?provider,
        'model': 'm',
        'host': '1',
        'rooms': <Object>[],
      };
      expect(ProfileDoc.fromJson(profile('xai')).provider, 'xai');
      // Ein älteres Programm nennt den Dienst nicht.
      expect(ProfileDoc.fromJson(profile(null)).provider, 'openai');
      expect(ProfileDoc.fromJson(profile(' ')).provider, 'openai');
      expect(ProfileDoc.blank().provider, 'openai');
      expect(
        const ProfileDoc(
          name: 'a',
          provider: 'google',
          model: 'm',
          baseUrl: null,
          host: '1',
          aiNames: [],
          limits: Limits(translationTokensPerDay: 1, questionsPerGuestPerDay: 1),
          rooms: [],
          people: [],
        ).renamed('b').provider,
        'google',
      );
    });

    test('the key status holds one yes or no per entry of the store', () {
      final status = KeyStatus.fromJson(
        jsonDecode(
              '{"discord": true, "openai": true, "xai": false, "google": false, '
              '"anthropic": true, "custom": false}',
            )
            as Map<String, dynamic>,
      );
      expect(status.discord, isTrue);
      expect(status.has('openai'), isTrue);
      expect(status.has('xai'), isFalse);
      expect(status.has('anthropic'), isTrue);
      // Was das Programm nicht nennt, gilt als nicht abgelegt.
      expect(status.has('never-heard-of'), isFalse);
      // Ein älteres Programm nennt nur zwei Einträge.
      final old = KeyStatus.fromJson(const {'openai': true, 'discord': false});
      expect(old.has('openai'), isTrue);
      expect(old.has('xai'), isFalse);
    });
  });

  // Nemesus am 20.11.2026: „pro Discord-Nummer die Muttersprache und die
  // Neutralsprache festzulegen", und: „Es sollte auch die Funktion ‚Einladen'
  // geben. Und dann ein Haken, wer eingeladen wird."
  group('language at the person, tables and invitations', () {
    Map<String, dynamic> listed(String people, String seats) =>
        jsonDecode('''
        {"dir": "C:\\\\fish", "profiles": [
          {"name": "tea", "path": "C:\\\\fish\\\\tea.toml", "ok": true, "error": null,
           "provider": "openai", "model": "m", "base_url": null, "host": "100",
           "ai_names": [],
           "limits": {"translation_tokens_per_day": 1, "questions_per_guest_per_day": 1},
           "rooms": [{"pass_through": ["English"], "context_lines": 10, "seats": $seats}],
           "people": $people, "usage": null, "running": false}
        ]}''')
            as Map<String, dynamic>;

    test('the new fields are read and go back out unchanged', () {
      final doc = ProfileList.fromJson(
        listed(
          '[{"id": "100", "gender": "male", "lang": "German", "neutral": null, "invited": false},'
              ' {"id": "200", "gender": null, "lang": "Russian", "neutral": ["English", "French"], "invited": true},'
              ' {"id": "300", "gender": "female", "lang": null, "neutral": [], "invited": false}]',
          '[{"channel": "111", "lang": "German", "gender": null, "owner": "100", "served": true},'
              ' {"channel": "222", "lang": "Russian", "gender": null, "owner": "200", "served": true},'
              ' {"channel": "333", "lang": "Korean", "gender": null, "owner": "300", "served": false}]',
        ),
      ).profiles.single.doc!;

      final guest = doc.people[1];
      expect(guest.gender, isNull, reason: 'a form is optional now');
      expect(guest.lang, 'Russian');
      expect(guest.neutral, ['English', 'French']);
      expect(guest.invited, isTrue);
      expect(doc.people[0].neutral, isNull);
      // Eine leere Liste ist eine andere Angabe als keine: für diese Person
      // bleibt nichts unübersetzt.
      expect(doc.people[2].neutral, isEmpty);
      expect(doc.rooms.single.seats.map((s) => s.owner), ['100', '200', '300']);
      expect(doc.rooms.single.seats.map((s) => s.served), [true, true, false]);

      final json = doc.toJson();
      final people = json['people'] as List;
      expect(people[1], {
        'discord': '200',
        'gender': null,
        'lang': 'Russian',
        'neutral': ['English', 'French'],
        'invited': true,
      });
      expect((people[2] as Map)['neutral'], isEmpty);
      final seats = (json['rooms'] as List).single['seats'] as List;
      expect((seats[1] as Map)['owner'], '200');
      // Ob ein Tisch bedient wird, entscheidet das Programm. Die Maske schickt
      // es nicht zurück.
      expect((seats[1] as Map).containsKey('served'), isFalse);
      // Hin und zurück bleibt es dasselbe Profil.
      final again = ProfileDoc.fromJson(jsonDecode(jsonEncode(json)) as Map<String, dynamic>);
      expect(again.toJson(), json);
    });

    test('a document without the new fields reads as before', () {
      final doc = ProfileList.fromJson(
        listed(
          '[{"id": "200", "gender": "female"}]',
          '[{"channel": "111", "lang": "German", "gender": "male"},'
              ' {"channel": "222", "lang": "Russian", "gender": null}]',
        ),
      ).profiles.single.doc!;

      final person = doc.people.single;
      expect(person.gender, Gender.female);
      expect(person.lang, isNull);
      expect(person.neutral, isNull);
      expect(person.invited, isFalse);
      for (final seat in doc.rooms.single.seats) {
        expect(seat.owner, isNull);
        expect(seat.served, isTrue, reason: 'an older program serves every table');
      }
      // Was nicht gesetzt ist, geht auch nicht hinaus. Ein älteres Programm
      // lehnt Felder ab, die es nicht kennt.
      final json = doc.toJson();
      expect((json['people'] as List).single, {'discord': '200', 'gender': 'female'});
      expect(((json['rooms'] as List).single['seats'] as List).first, {
        'channel': '111',
        'lang': 'German',
        'gender': 'male',
      });
    });

    ProfileDoc tables(List<Person> people, List<Seat> seats) => ProfileDoc(
      name: 'p',
      model: 'm',
      baseUrl: null,
      host: '100',
      aiNames: const [],
      limits: const Limits(translationTokensPerDay: 1, questionsPerGuestPerDay: 1),
      rooms: [Room(passThrough: const [], contextLines: 1, seats: seats)],
      people: people,
    );

    test('the language of a table is the mother tongue of its owner', () {
      final doc = tables(
        const [Person(id: '200', lang: 'Russian'), Person(id: '300')],
        const [
          Seat(channel: '1', lang: 'German'),
          Seat(channel: '2', lang: 'Polish', owner: '200'),
          Seat(channel: '3', lang: 'Korean', owner: '300'),
          Seat(channel: '4', lang: 'Greek', owner: '999'),
        ],
      );
      final seats = doc.rooms.single.seats;
      expect(doc.languageAt(seats[0]), 'German');
      expect(doc.languageAt(seats[1]), 'Russian');
      // Kein Eintrag zur Muttersprache: dann gilt die Sprache des Platzes.
      expect(doc.languageAt(seats[2]), 'Korean');
      expect(doc.languageAt(seats[3]), 'Greek');
    });

    test('the card line says whom this fish translates for', () {
      // Ein Profil aus der Zeit vor den Tischen: niemand besitzt einen Platz.
      final old = tables(const [], const [
        Seat(channel: '1', lang: 'German'),
        Seat(channel: '2', lang: 'Russian'),
      ]);
      expect(servesText(old), 'Translates at every table');
      expect(readOnlyText(old), isNull);

      final hostAndGuest = tables(const [], const [
        Seat(channel: '1', lang: 'German', owner: '100'),
        Seat(channel: '2', lang: 'Russian', owner: '200'),
      ]);
      expect(servesText(hostAndGuest), 'Translates for you and 1 invited guest');
      expect(readOnlyText(hostAndGuest), isNull);

      final oneReadOnly = tables(const [], const [
        Seat(channel: '1', lang: 'German', owner: '100'),
        Seat(channel: '2', lang: 'Russian', owner: '200', served: false),
      ]);
      expect(servesText(oneReadOnly), 'Translates for you');
      expect(readOnlyText(oneReadOnly), '1 table is read only');

      final many = tables(const [], const [
        Seat(channel: '1', lang: 'German', owner: '100'),
        Seat(channel: '2', lang: 'Russian', owner: '200'),
        Seat(channel: '3', lang: 'Korean', owner: '300'),
        Seat(channel: '4', lang: 'Greek', owner: '400', served: false),
        Seat(channel: '5', lang: 'Polish', owner: '500', served: false),
        Seat(channel: '6', lang: 'French'),
      ]);
      expect(
        servesText(many),
        'Translates for you and 2 invited guests, and at 1 table without an owner',
      );
      expect(readOnlyText(many), '2 tables are read only');

      final guestsOnly = tables(const [], const [
        Seat(channel: '1', lang: 'German', owner: '200'),
        Seat(channel: '2', lang: 'Russian', owner: '300', served: false),
      ]);
      expect(servesText(guestsOnly), 'Translates for 1 invited guest');

      final nobody = tables(const [], const [
        Seat(channel: '1', lang: 'German', owner: '200', served: false),
        Seat(channel: '2', lang: 'Russian', owner: '300', served: false),
      ]);
      expect(servesText(nobody), 'Translates at no table');
      expect(readOnlyText(nobody), '2 tables are read only');
    });
  });

  group('names the host gives, and the door of a table', () {
    // Der Gastgeber heißt Nemo und hat eine Kennung, Robin hat noch keine,
    // und ein Dritter steht nur mit seiner Kennung da.
    ProfileDoc named({String host = 'Nemo', String hostTable = 'Nemo'}) => ProfileDoc(
      name: 'tea',
      provider: 'openai',
      model: 'm',
      baseUrl: null,
      host: host,
      aiNames: const [],
      limits: const Limits(translationTokensPerDay: 1, questionsPerGuestPerDay: 1),
      rooms: [
        Room(
          passThrough: const [],
          contextLines: 1,
          seats: [
            Seat(channel: '1', lang: 'German', owner: hostTable),
            const Seat(channel: '2', lang: 'English', owner: 'Robin'),
            const Seat(channel: '3', lang: 'Korean', owner: '300'),
          ],
        ),
      ],
      people: const [
        Person(id: '100', name: 'Nemo'),
        Person(name: 'Robin', lang: 'Russian', invited: true),
        Person(id: '300'),
      ],
    );

    test('a person is read with a name and an id, under the new and the old word', () {
      final robin = Person.fromJson({'name': 'Robin', 'discord': '7'});
      expect(robin.name, 'Robin');
      expect(robin.id, '7');
      expect(robin.ref, 'Robin', reason: 'tables point at a person by name');

      // So schrieb es das Programm bis zum 21.11.2026.
      final old = Person.fromJson({'id': '7', 'name': null});
      expect(old.name, isNull);
      expect(old.id, '7');
      expect(old.ref, '7');
    });

    test('a name goes out, an id that is not known yet does not', () {
      expect(const Person(name: 'Robin').toJson(), {'name': 'Robin', 'gender': null});
      expect(const Person(id: '7', name: 'Robin').toJson(), {
        'name': 'Robin',
        'discord': '7',
        'gender': null,
      });
    });

    test('the card says for whom a table is built', () {
      final doc = named();
      final seats = doc.rooms.single.seats;
      expect(doc.tableText(seats[0]), 'Nemo · German');
      // Die Muttersprache des Besitzers gilt, auch ohne Kennung.
      expect(doc.tableText(seats[1]), 'Robin · Russian');
      // Wer keinen Namen hat, erscheint nicht als Zahl.
      expect(doc.tableText(seats[2]), 'Korean');
    });

    test('a named guest is named, the others are counted', () {
      expect(servesText(named()), 'Translates for you, Robin and 1 invited guest');
    });

    test('the host is the same person by name and by id', () {
      // Das Profil nennt den Gastgeber mit der Kennung, sein Tisch mit dem Namen.
      expect(servesText(named(host: '100')), startsWith('Translates for you,'));
      // Und umgekehrt.
      expect(servesText(named(hostTable: '100')), startsWith('Translates for you,'));
      final doc = named();
      expect(doc.isHost(doc.people[0]), isTrue);
      expect(doc.isHost(doc.people[1]), isFalse);
    });

    test('a table names its door, and Discord is meant when none is named', () {
      expect(Seat.fromJson({'channel': '1', 'lang': 'German'}).door, 'discord');
      expect(Seat.fromJson({'door': 'matrix', 'channel': '1', 'lang': 'German'}).door, 'matrix');
      expect(const Seat(channel: '1', lang: 'German').toJson().containsKey('door'), isFalse);
      expect(const Seat(door: 'matrix', channel: '1', lang: 'German').toJson()['door'], 'matrix');
    });

    test('the connection says which doors, and whether the way is encrypted', () {
      Room over(List<String> doors) => Room(
        passThrough: const [],
        contextLines: 1,
        seats: [for (final door in doors) Seat(door: door, channel: '1', lang: 'German')],
      );
      expect(
        connectionText(over(['discord', 'discord'])),
        'Discord ⇄ Discord · not end-to-end encrypted',
      );
      // Ein Raum ist so offen wie sein offenster Tisch.
      expect(
        connectionText(over(['discord', 'matrix'])),
        'Discord ⇄ Matrix · not end-to-end encrypted on Discord',
      );
      // Auch dann liest der Fisch mit, und mit ihm der KI-Dienst.
      expect(
        connectionText(over(['matrix', 'matrix'])),
        'Matrix ⇄ Matrix · end-to-end encrypted up to the fish',
      );
      expect(connectionHint, contains('AI service reads every message'));
    });
  });

  group('the About window', () {
    test('it shows the version the window really has', () {
      // Die Zahl steht an zwei Stellen. Dieser Test hält sie gleich.
      final pubspec = File('pubspec.yaml').readAsStringSync();
      expect(pubspec, contains('\nversion: $deskVersion+'));
    });
  });

  group('lines of a fish that serves several profiles', () {
    test('a line says which profile it is about', () {
      expect(profileOfLine('rusty-fish: [probe] consent stored'), ('probe', 'consent stored'));
      expect(
        profileOfLine('rusty-fish: [tea-time] cannot start: OpenAI API key not stored'),
        ('tea-time', 'cannot start: OpenAI API key not stored'),
      );
    });

    test('a line about the whole fish names no profile', () {
      expect(profileOfLine('rusty-fish: connected to Discord as Rusty Fish'), isNull);
      expect(profileOfLine('rusty-fish: stopping'), isNull);
      // Eckige Klammern mitten im Text sind kein Profil.
      expect(profileOfLine('rusty-fish: click received on [x] y'), isNull);
      expect(profileOfLine('something else: [probe] x'), isNull);
    });
  });

  group('where the AI reads', () {
    test('the card says it in words, by what the program found', () {
      expect(
        aiPlaceText(const AiReach(service: 'api.openai.com', place: 'outside')),
        'AI reads at an outside service · api.openai.com',
      );
      expect(
        aiPlaceText(const AiReach(service: 'localhost:11434', place: 'this_machine')),
        'AI runs on this PC · localhost:11434',
      );
      expect(
        aiPlaceText(const AiReach(service: '192.168.1.20:8000', place: 'own_network')),
        'AI runs in your own network · 192.168.1.20:8000',
      );
      // Ein Ort, den diese Maske nicht kennt: lieber nichts als etwas Geratenes.
      expect(aiPlaceText(const AiReach(service: 'x', place: 'on the moon')), isNull);
      // Die Zeile verspricht nicht mehr, als der Fisch prüfen kann.
      expect(aiPlaceHint, contains('open source'));
      expect(aiPlaceHint, contains('cannot check'));
    });

    test('it is read from the list, and an older program simply does not say', () {
      Map<String, dynamic> entry(String extra) =>
          jsonDecode('''{"name": "tea", "path": "p", "ok": true, "model": "m", "host": "100",
            "rooms": [], "people": [], "usage": null, "running": false$extra}''')
              as Map<String, dynamic>;
      final told = ProfileEntry.fromJson(
        entry(', "ai": {"service": "localhost:11434", "place": "this_machine"}'),
      );
      expect(told.ai!.service, 'localhost:11434');
      expect(told.ai!.place, 'this_machine');
      expect(ProfileEntry.fromJson(entry('')).ai, isNull);
    });
  });

  group('light or dark', () {
    test('the choice is written as a word and read back', () {
      for (final mode in ThemeMode.values) {
        expect(themeModeFromText(themeModeToText(mode)), mode);
      }
      expect(themeModeFromText(null), ThemeMode.system);
      expect(themeModeFromText('purple'), ThemeMode.system);
    });

    test('it is kept in the folder of rusty-fish, by the rule of the program', () {
      expect(
        deskSettingsPath(environment: {'APPDATA': r'C:\Users\x\AppData\Roaming'}, windows: true),
        r'C:\Users\x\AppData\Roaming\rusty-fish\desk.json',
      );
      expect(
        deskSettingsPath(
          environment: {'RUSTY_FISH_HOME': r'D:\fish', 'APPDATA': r'C:\x'},
          windows: true,
        ),
        r'D:\fish\desk.json',
      );
      expect(deskSettingsPath(environment: {'RUSTY_FISH_HOME': '/fish'}, windows: false), '/fish/desk.json');
      // Ohne Ort gilt die Wahl nur, solange das Fenster offen ist.
      expect(deskSettingsPath(environment: {'HOME': '/home/x'}, windows: false), isNull);
    });

    test('the first click turns what is shown around, and the choice is remembered', () {
      final remembered = <ThemeMode>[];
      final choice = ThemeChoice(remember: remembered.add);
      expect(choice.value, ThemeMode.system);
      // Windows ist hell eingestellt: der erste Klick macht dunkel.
      choice.toggle(Brightness.light);
      expect(choice.value, ThemeMode.dark);
      choice.toggle(Brightness.dark);
      expect(choice.value, ThemeMode.light);
      expect(remembered, [ThemeMode.dark, ThemeMode.light]);
    });

    test('the choice survives closing the window, and a broken file does no harm', () {
      final dir = Directory.systemTemp.createTempSync('rusty-fish-theme');
      addTearDown(() => dir.deleteSync(recursive: true));
      // Der Ordner darf noch fehlen.
      final path = '${dir.path}${Platform.pathSeparator}new${Platform.pathSeparator}desk.json';
      expect(readThemeMode(path), ThemeMode.system, reason: 'no file yet');
      writeThemeMode(path, ThemeMode.dark);
      expect(readThemeMode(path), ThemeMode.dark);
      File(path).writeAsStringSync('not json');
      expect(readThemeMode(path), ThemeMode.system);
    });
  });
}
