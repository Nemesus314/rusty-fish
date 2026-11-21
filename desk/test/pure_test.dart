// Tests der Teile, die ohne Fenster auskommen.

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:rusty_fish_desk/cli.dart';
import 'package:rusty_fish_desk/desk_state.dart';
import 'package:rusty_fish_desk/model.dart';

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
        'id': '200',
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
      expect((json['people'] as List).single, {'id': '200', 'gender': 'female'});
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
}
