#!/bin/bash
# Baut das Paket für Linux mit Desktop: die Maske und den Fisch in einem
# Ordner, mit Lizenz, Leitfaden und Hinweisen, als .tar.gz.
#
# Nemesus am 25.11.2026, nachdem die Pakete für Windows und für Server draußen
# waren, schlug vor, sein Labor um ein Linux Mint in einer virtuellen Maschine
# zu erweitern, für eine Fassung für Leute, die Linux auf ihrem PC nutzen. Zur
# Form an dem Tag: „Eigenes Desktop-Paket", neben dem für Server.
#
# Aufruf, unter Linux aus dem Ordner des Repositorys, eins nach dem anderen:
#
#     bash packaging/linux.sh
#     bash packaging/linux-desktop.sh
#
# Der Fisch und seine Lizenzhinweise kommen aus dem Paket für Server, das
# `packaging/linux.sh` aus demselben Commit gebaut hat: derselbe statisch
# gebundene Fisch, nicht ein zweiter Bau. Liegt es nicht in `target/package`,
# nennt PACKAGE_SERVER die Datei. Die Maske baut dieses Skript selbst.
#
# Gebraucht werden: Flutter mit den Werkzeugen für Linux (clang, cmake,
# ninja-build, pkg-config, libgtk-3-dev), python3 und git; für den Fisch, was
# `packaging/linux.sh` nennt.
#
# Auf welchem Rechner: auf einem älteren Linux. Die Maske ist kein statisches
# Programm. Sie verlangt GTK 3 und die C-Bibliothek des Systems, in mindestens
# der Fassung des Rechners, auf dem sie gebaut wurde; gebaut auf einem älteren
# Linux startet sie auch auf neueren. Gemessen am 25.11.2026: gebaut unter
# Linux Mint 22.3 (Basis Ubuntu 24.04, glibc 2.39) verlangt sie glibc ab 2.34.
#
# Der Fisch verlangt nichts und dürfte woanders gebaut sein; geprüft werden
# nur seine Fassung und sein Commit. Am 25.11.2026 kam er zuerst aus WSL
# (Ubuntu 26.04) und startete unter Mint. Nemesus danach: „Ich glaube, das
# sollten wir als nächsten nachholen", das Mint sei auch sein Labor unter
# Linux mit Desktop. Seither entstehen beide Pakete an einem Ort. Gemessen
# dort, mit einem Prozessor: der statische Bau 57 Minuten, die Maske 25
# Sekunden; der so gebaute Fisch startete auch unter Ubuntu 26.04 (WSL).
#
# Wie in den anderen Paketen bricht das Skript ab, wenn der Ordner oder der
# Name dieses Nutzers in einer Datei der Maske steht. Gemessen am 25.11.2026
# mittags: Flutter schreibt sie nicht hinein, anders als der Rust-Compiler.
# Am Abend desselben Tages, mit dem ersten Baustein mit eigenem Maschinencode
# (dem Dateidialog für „Export…" und „Import…"), schrieb es doch zwei hinein;
# seither wird die Maske in einer Kopie unter /tmp/rusty-fish-build gebaut,
# und die Suchpfade der Bibliotheken werden geprüft.
#
# Benannte Grenzen:
# - Gestartet wurde die Maske nur unter Linux Mint 22.3 (Cinnamon, X11), in
#   einer virtuellen Maschine. Andere Systeme und Wayland: nicht gesehen.
# - Nur x86-64.
# - Kein Eintrag im Startmenü und kein eigenes Symbol: man startet die Datei
#   `rusty_fish_desk` im entpackten Ordner.

set -euo pipefail

ADDRESS=https://github.com/Nemesus314/rusty-fish

fail() {
    echo "packaging: $*" >&2
    exit 1
}
say() { echo "== $*"; }

[ -f Cargo.toml ] && [ -d packaging ] && [ -d desk ] || fail "run this from the folder of the repository: bash packaging/linux-desktop.sh"
for tool in flutter python3 git tar sha256sum readelf objdump; do
    command -v "$tool" > /dev/null || fail "missing: $tool"
done

REPO="$PWD"
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)"
[ -n "$VERSION" ] || fail "cannot read the version from Cargo.toml"
gitc() { git -c safe.directory="$REPO" "$@"; }
COMMIT="$(gitc rev-parse HEAD)"
# Das Paket nennt den Commit, aus dem es gebaut ist. Mit offenen Änderungen
# stimmte diese Angabe nicht.
OPEN="$(gitc status --porcelain)"
if [ -n "$OPEN" ] && [ "${1:-}" != "--with-open-changes" ]; then
    fail "there are changes that are not committed; the package names its commit.
$OPEN"
fi

NAME="RustyFish-$VERSION-linux-desktop-x64"
PACKAGES="$REPO/target/package"
SERVER="${PACKAGE_SERVER:-$PACKAGES/RustyFish-$VERSION-linux-x64.tar.gz}"
SERVER_NAME="RustyFish-$VERSION-linux-x64"
[ -f "$SERVER" ] || fail "the package for servers is missing: $SERVER
build it first with: bash packaging/linux.sh (or name the file in PACKAGE_SERVER)"

# Zusammengesetzt wird im Linux-Dateisystem, nicht im Repository; siehe
# `packaging/linux.sh`.
STAGE="$(mktemp -d)"
# Wo die Maske gebaut wird; siehe unten.
BUILD=/tmp/rusty-fish-build
trap 'rm -rf "$STAGE" "$BUILD"' EXIT
OUT="$STAGE/$NAME"

say "taking the fish from the package for servers"
mkdir "$STAGE/server"
tar -C "$STAGE/server" -xzf "$SERVER" "$SERVER_NAME/rusty-fish" "$SERVER_NAME/README.txt" "$SERVER_NAME/THIRD-PARTY-NOTICES.txt"
FROM="$STAGE/server/$SERVER_NAME"
# Dieselbe Fassung und derselbe Commit: sonst trüge das Paket einen Fisch, den
# seine Liesmich-Datei nicht nennt.
grep -q "version $VERSION, commit $COMMIT\$" "$FROM/README.txt" || fail "the package for servers is not from this commit:
$(grep 'version .*, commit ' "$FROM/README.txt" || echo '(it names none)')
wanted: version $VERSION, commit $COMMIT"
FISH="$FROM/rusty-fish"
if readelf -d "$FISH" 2> /dev/null | grep -q NEEDED; then
    fail "the fish is not statically linked"
fi
[ "$("$FISH" --version)" = "rusty-fish $VERSION" ] || fail "the fish calls itself $("$FISH" --version), not $VERSION"

say "building the window"
# In einer Kopie an einem Ort, der nichts über diesen Rechner sagt, und damit
# ohne Reste eines früheren Baus. Seit die Maske einen Baustein mit eigenem
# Maschinencode trägt (`file_selector`, 25.11.2026), schreibt Flutter den Ort
# einer Datei hinein, die es beim Bauen erzeugt
# (`…/desk/.dart_tool/flutter_build/dart_plugin_registrant.dart`). Gemessen an
# dem Tag: gebaut im Repository stand so der Ordner des Nutzers in `libapp.so`,
# und dieses Skript brach an der Prüfung unten ab.
# Kopiert wird, was git kennt oder kennen würde: keine Bau-Ausgaben.
rm -rf "$BUILD"
mkdir -m 700 "$BUILD"
gitc ls-files -co --exclude-standard -z -- desk | tar -c --null -T - | tar -x -C "$BUILD"
(cd "$BUILD/desk" && FLUTTER_SUPPRESS_ANALYTICS=true flutter build linux --release 2>&1 | tail -n 2)
BUNDLE="$BUILD/desk/build/linux/x64/release/bundle"
[ -x "$BUNDLE/rusty_fish_desk" ] || fail "the build left no window at $BUNDLE"

# Weder der Ordner noch der Name dieses Nutzers in einer Datei der Maske.
TRACES="$(grep -a -r -l -e "$HOME/" -e "$REPO/" -e "/$(id -un)/" "$BUNDLE" || true)"
[ -z "$TRACES" ] || fail "the window still carries paths of this machine:
$TRACES"
# Und keine Datei sucht ihre Bibliotheken an einem festen Ort: nur neben sich.
for file in "$BUNDLE/rusty_fish_desk" "$BUNDLE"/lib/*.so; do
    SEARCH="$(readelf -d "$file" | sed -n 's/.*(R\(UN\)\{0,1\}PATH).*\[\(.*\)\]/\2/p')"
    case "$SEARCH" in
        '' | '$ORIGIN' | '$ORIGIN/lib') ;;
        *) fail "$(basename "$file") looks for its libraries in a fixed place: $SEARCH" ;;
    esac
done

say "putting the folder together"
mkdir -m 755 "$OUT"
mkdir -p "$PACKAGES"
cp -r "$BUNDLE/." "$OUT/"
install -m 755 "$FISH" "$OUT/rusty-fish"
install -m 644 LICENSE GUIDE.md "$OUT/"
install -m 644 "$FROM/THIRD-PARTY-NOTICES.txt" "$OUT/THIRD-PARTY-NOTICES.txt"
GLIBC="$(objdump -T "$OUT/rusty_fish_desk" "$OUT"/lib/*.so | grep -o 'GLIBC_[0-9.]*' | sort -V | tail -n 1 | cut -d _ -f 2)"
[ -n "$GLIBC" ] || fail "cannot tell which C library the window asks for"
sed -e "s/{version}/$VERSION/g" -e "s/{commit}/$COMMIT/g" -e "s/{glibc}/$GLIBC/g" \
    packaging/README-linux-desktop.txt > "$OUT/README.txt"

say "collecting the notices of the parts"
[ -f "$OUT/data/flutter_assets/NOTICES.Z" ] || fail "the window has no notices"
{
    echo "Notices of the parts the window (rusty_fish_desk) is built from,"
    echo "as Flutter collects them. Rusty Fish $VERSION."
    echo
    python3 -c 'import gzip, sys; sys.stdout.buffer.write(gzip.open(sys.argv[1]).read())' \
        "$OUT/data/flutter_assets/NOTICES.Z"
} > "$OUT/THIRD-PARTY-NOTICES-window.txt"

# Die Rechte fest, gleich mit welchen Voreinstellungen dieser Rechner anlegt:
# Ordner offen zum Lesen, Dateien nur lesbar, ausführbar nur die zwei Programme.
find "$OUT" -type d -exec chmod 755 {} +
find "$OUT" -type f -exec chmod 644 {} +
chmod 755 "$OUT/rusty_fish_desk" "$OUT/rusty-fish"

# Ohne den Namen und die Nummer dieses Nutzers im Archiv.
tar -C "$STAGE" --owner=0 --group=0 --numeric-owner -czf "$PACKAGES/$NAME.tar.gz" "$NAME"
LISTED="$(tar -tvzf "$PACKAGES/$NAME.tar.gz")"
WRONG="$(printf '%s\n' "$LISTED" | grep -v -e '^drwxr-xr-x ' -e '^-rw-r--r-- ' \
    -e "^-rwxr-xr-x .* $NAME/rusty-fish\$" -e "^-rwxr-xr-x .* $NAME/rusty_fish_desk\$" || true)"
[ -z "$WRONG" ] || fail "the archive carries rights it should not:
$WRONG"

say "done"
echo "file:    $PACKAGES/$NAME.tar.gz ($(stat -c %s "$PACKAGES/$NAME.tar.gz") bytes)"
echo "sha256:  $(sha256sum "$PACKAGES/$NAME.tar.gz" | cut -d ' ' -f 1)"
echo "version: $VERSION, commit $COMMIT"
echo "window:  asks for GTK 3 and glibc $GLIBC or newer"
echo "fish:    $("$OUT/rusty-fish" --version), $(stat -c %s "$OUT/rusty-fish") bytes, statically linked"
echo "source:  $ADDRESS"
