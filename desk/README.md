# Rusty Fish desk

A window for the `rusty-fish` program: it shows every profile, lets you edit and
save it, and starts or stops it.

The window never reads or writes a profile file itself. It calls `rusty-fish`
and exchanges JSON, so the program stays the one place that decides what a valid
profile is. The same profiles run on a server without any window.

The program is looked for in this order:

1. the environment variable `RUSTY_FISH_EXE`
2. next to this window's own executable
3. upwards from there in `target/debug` (the development case)

Known limit: the fish runs while this window is open. Closing the window stops
it. If the window is killed the hard way, the fish keeps running until someone
stops it by hand.

    flutter test
    flutter build windows
