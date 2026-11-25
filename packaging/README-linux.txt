Rusty Fish {version} for Linux (x86-64)
========================================

Everyone writes in their own language, and an AI translates in passing.
Rusty Fish is a translator bot for Discord channels and Matrix rooms that
you run yourself.

This folder holds the fish itself, for a server without a desktop;
everything is done on the command line. For a PC with Linux and a desktop
there is another download, with a window to manage the fish.


What is in it
-------------

rusty-fish           the program, one file. It is statically linked: it
                     asks nothing of the system it runs on, no library and
                     no particular version of Linux.
rusty-fish.service   a file for systemd, to run the fish as a service
GUIDE.md             how to set everything up; see "A Linux server" there


Starting it
-----------

    ./rusty-fish --version

GUIDE.md leads through the rest, step by step: an account of its own for
the fish, the secrets (`./rusty-fish key store file`, then `key set`), the
login to Matrix, bringing profiles over from a PC (`profile import`), and
`./rusty-fish run --all`. It is written so that an AI assistant can walk
you through it: give it to an assistant you trust and ask in your own
language. The program prints the same text with `./rusty-fish guide`.

The fish keeps its things in ~/.config/rusty-fish, in folders that only its
account can open. On a server its secrets lie there in files, not
encrypted; the guide says what that means.


What has been measured
----------------------

With real accounts the fish has run on Linux at both doors, in its ordinary
build, on Ubuntu inside Windows (WSL). The statically linked file in this
folder is built on Linux Mint 22.3 in a virtual machine. Built that way it
passed all tests there. With made-up accounts it reached an AI service over
an encrypted connection on Mint, and on Ubuntu (WSL) it kept a secret in
files, served a profile and stopped cleanly on SIGTERM. With real accounts
it has run at both doors on Mint, started from the window of the download
for a desktop.
That it asks nothing of the system was read off the file. It has not been
started on a real server yet; if it does not run on yours, the fish can be
built from the source on the server itself.


License
-------

Copyright (C) 2026 Christian Ermisch (Nemesus)

Rusty Fish is free software: you can redistribute it and/or modify it under
the terms of the GNU Affero General Public License as published by the Free
Software Foundation, either version 3 of the License, or (at your option)
any later version. It is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. The full text is in
the file LICENSE.

The source of exactly this version:

    https://github.com/Nemesus314/rusty-fish
    version {version}, commit {commit}

Rusty Fish is built from many parts written by others, each under its own
license. Their notices are in THIRD-PARTY-NOTICES.txt.
