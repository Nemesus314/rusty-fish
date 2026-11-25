Rusty Fish {version} for Linux with a desktop (x86-64)
========================================================

Everyone writes in their own language, and an AI translates in passing.
Rusty Fish is a translator bot for Discord channels and Matrix rooms that
you run yourself.

This folder holds the window and the fish, for a PC that runs Linux. For a
server without a desktop there is a smaller download with the fish alone.


Starting it
-----------

1. Unpack this folder to any place you like. Keep its files together.
2. Start rusty_fish_desk. That is the window. In a terminal, from this
   folder:

       ./rusty_fish_desk

   The file rusty-fish is the fish itself; the window starts it for you.

What you need: a 64-bit Linux with a desktop, GTK 3 and the C library
glibc {glibc} or newer (Linux Mint 21, Ubuntu 22.04, Debian 12 and later
have both), a Discord bot or a Matrix account for the fish, and an API key
of an AI service. The file GUIDE.md tells how to set all of it up. It is
written so that an AI assistant can walk you through it: give it to an
assistant you trust and ask in your own language. The window can copy the
same text for you.

The fish runs while the window is open and the PC is awake. On Linux it
does not keep the PC awake: if yours goes to sleep by itself, switch that
off in the power settings while the fish should run.


Where your things are kept
--------------------------

Nothing of yours is stored in this folder. Your profiles lie in
~/.config/rusty-fish, in folders that only your account can open. Your keys
and tokens go to the keyring of the desktop (the Secret Service; under
Cinnamon and GNOME that is GNOME Keyring), which opens when you log in.
Deleting this folder removes the program and leaves both.


What has been measured
----------------------

Window and fish of this folder were built and run on Linux Mint 22.3
(Cinnamon) in a virtual machine, on 25 November 2026: the window opens,
finds the fish beside it, switches it on and off, and closing the window
stops the fish. The fish kept a made-up secret in the keyring of the
desktop, found it in a new process and deleted it again. All tests of the
window passed there.

Seen the same day with real accounts, with the download of version 0.4.3:
bot token and API key stored through the window, a profile switched on, the
fish at the Discord door a second later, translating in both directions,
and taken offline by "Switch off" and by closing the window.

Not measured: the Matrix door from this window on Linux; other desktops
than Cinnamon, and Wayland; other Linux systems; a keyring that is locked,
or a login without a password.

There is no entry in the start menu and no icon of its own yet.


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
license. Their notices are in THIRD-PARTY-NOTICES.txt (the fish) and
THIRD-PARTY-NOTICES-window.txt (the window).
