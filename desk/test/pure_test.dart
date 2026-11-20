// Tests der Teile, die ohne Fenster auskommen.

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:rusty_fish_desk/cli.dart';
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
      'model',
      'base_url',
      'host',
      'ai_names',
      'limits',
      'rooms',
      'people',
    ]);
    expect(json['base_url'], isNull);
    expect((json['rooms'] as List).single['seats'][1]['gender'], isNull);
    // Hin und zurück bleibt es dasselbe Profil.
    final again = ProfileDoc.fromJson(
      jsonDecode(jsonEncode(json)) as Map<String, dynamic>,
    );
    expect(again.toJson(), json);
  });
}
