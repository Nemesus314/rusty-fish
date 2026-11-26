# Rusty Fish

Everyone writes in their own language, and an AI translates in passing.

Rusty Fish is a translator bot for Discord channels and Matrix rooms. One
conversation has a table per language: what is written at one table arrives
at the others, translated. A Discord channel and a Matrix room can belong to
the same conversation.

- You run it yourself: your own Discord bot, your own Matrix account for the
  fish, your own API key. There is no server of ours in between.
- It talks to ChatGPT (OpenAI), Claude (Anthropic), Grok (xAI), Gemini
  (Google) or a model you host yourself.
- A guest is asked once before their words go to an AI service, and is told
  which one.
- Secrets are kept in the credential store of the system, or, on a Linux
  server, in files that only the fish's account can read.
- Everything moves in one file: "Export…" writes all profiles, the bot token
  and the API keys into a file locked with a password, and "Import…" takes
  them in on another machine. It is also a copy to keep.

## Where it stands

Version 0.4.5. It runs on Windows 11, with a window to manage it. Window
and fish also run on Linux with a desktop; that has been measured in one
place only, Linux Mint 22.3 in a virtual machine. The program itself also
runs on Linux without a desktop; measured in Ubuntu inside Windows (WSL),
not on a real server. macOS has not been tested.

Ready to download, under
[Releases](https://github.com/Nemesus314/rusty-fish/releases):

- For Windows a folder with the window and the fish: unpack it and start
  `rusty_fish_desk.exe`. It carries no paid signature, so Windows may warn
  at the first start.
- For Linux with a desktop a folder with the window and the fish: unpack it
  and start `rusty_fish_desk`. The window asks for GTK 3 and glibc 2.34 or
  newer. There is no entry in the start menu yet.
- For a Linux server the fish itself as one file, statically linked, that
  asks nothing of the system it runs on. It has not been started on a real
  server yet.

What has been measured and what has not is written down, in the guide under
"Limits" and at the end of each section.

## The guide

[GUIDE.md](GUIDE.md) describes the program exactly enough for an AI assistant
to walk someone through using it or setting it up. Give it to an assistant
you trust and ask in your own language. The program prints the same text
with `rusty-fish guide`.

## Building

You need Rust (edition 2024; built with 1.100) and a C compiler.

    cargo build --release -p rusty-fish
    cargo test --workspace

The window is a Flutter program in `desk/`:

    cd desk
    flutter build windows               (or: flutter build linux)

The downloads offered under Releases are each put together by one script,
from a clean checkout:

    dart run packaging/windows.dart        (on Windows)
    bash packaging/linux.sh                (on Linux; the fish for servers)
    bash packaging/linux-desktop.sh        (on Linux, after the one before:
                                            it takes the fish from there)

`rusty-fish.example.toml` shows a profile with every setting explained.

## The source

Code and tests are in English. The comments are in German: they carry the
reasons behind each decision, the sentence that asked for it and the numbers
of each measurement.

## License

Copyright (C) 2026 Christian Ermisch (Nemesus)

Rusty Fish is free software: you can redistribute it and/or modify it under
the terms of the GNU Affero General Public License as published by the Free
Software Foundation, either version 3 of the License, or (at your option)
any later version.

Rusty Fish is distributed in the hope that it will be useful, but WITHOUT
ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License
for more details. Its text is in the file [LICENSE](LICENSE).

## Who made this

Rusty Fish is made by Nemesus. He codes, writes and makes videos. What
drives him is the wish for meaningful connections between people,
worldwide.

- His site: https://christianermisch.com
- The source of Rusty Fish: https://github.com/Nemesus314/rusty-fish
- Videos in German: https://www.youtube.com/@NemesusDE
- Videos in English: https://www.youtube.com/@NemesusEN
- Experiments, in German: https://www.youtube.com/@Nemesus314
