Rusty Fish {version} for Windows (64-bit)
==========================================

Everyone writes in their own language, and an AI translates in passing.
Rusty Fish is a translator bot for Discord channels and Matrix rooms that
you run yourself.


Starting it
-----------

1. Unpack this folder to any place you like. Keep its files together.
2. Start rusty_fish_desk.exe. That is the window. The file rusty-fish.exe
   is the fish itself; the window starts it for you.
3. The first time, Windows may show a blue warning, "Windows protected
   your PC", because this program carries no paid signature. If it does,
   click "More info", then "Run anyway". Do that only if you got this
   folder from the address below or from someone you trust.

What you need: Windows 11 (other versions have not been tested), a Discord
bot or a Matrix account for the fish, and an API key of an AI service. The
file GUIDE.md tells how to set all of it up. It is written so that an AI
assistant can walk you through it: give it to an assistant you trust and
ask in your own language. The window can copy the same text for you.


Where your things are kept
--------------------------

Nothing of yours is stored in this folder. Your profiles lie in
%APPDATA%\rusty-fish, your keys and tokens in the Windows Credential
Manager. Deleting this folder removes the program and leaves both.


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
license. Their notices are in THIRD-PARTY-NOTICES.txt (the fish) and in
THIRD-PARTY-NOTICES-window.txt (the window).

The files msvcp140.dll, vcruntime140.dll and vcruntime140_1.dll are
Microsoft's Visual C++ runtime. They are passed on unchanged under
Microsoft's terms and are not covered by the license above.
