#!/bin/bash
# Baut das Linux-Paket zum Herunterladen: den Fisch als eine Datei, die vom
# System nichts verlangt, mit Lizenz, Leitfaden und Hinweisen, als .tar.gz.
#
# Nemesus am 25.11.2026, nachdem das Windows-Paket draußen war: „danach können
# wir darüber reden, was wir für das Linux Paket tun können", und zum Weg:
# „Statisch, hier bauen".
#
# Aufruf, unter Linux aus dem Ordner des Repositorys:
#
#     bash packaging/linux.sh
#
# Gebraucht werden: Rust mit dem Ziel x86_64-unknown-linux-musl
# (`rustup target add x86_64-unknown-linux-musl`), der C-Compiler dazu
# (`musl-gcc`, unter Debian und Ubuntu das Paket musl-tools), python3 und git.
#
# Warum statisch: ein gewöhnlich gebautes Programm verlangt die C-Bibliothek
# des Systems, auf dem es gebaut wurde, in mindestens dessen Fassung. Gebaut
# auf einem neuen Ubuntu, startet es auf einem älteren Server nicht. Statisch
# gebaut bringt es alles mit. Gemessen am 25.11.2026: der Bau ging auf Anhieb,
# auch mit den Bausteinen in C (aws-lc, SQLite); das Programm verlangt keine
# Bibliothek, erreicht das Netz verschlüsselt und hält auf SIGTERM sauber an.
#
# Wie im Windows-Paket werden die Pfade der Quelldateien ersetzt, die der
# Compiler sonst ins Programm schreibt, und das Skript bricht ab, wenn der
# Ordner dieses Nutzers noch darin steht.
#
# Benannte Grenzen:
# - Auf einem anderen Linux ist das Programm nicht gestartet worden. Dass es
#   dort läuft, folgt daraus, dass es nichts verlangt; gesehen ist es nicht.
# - Nur x86-64.
# - Der Bau-Ordner liegt unter `target/package-build`. Liegt das Repository auf
#   einem langsamen oder mit Windows geteilten Laufwerk (WSL: /mnt/…), setzt
#   man PACKAGE_BUILD_DIR auf einen Ordner im Linux-Dateisystem.

set -euo pipefail

TARGET=x86_64-unknown-linux-musl
ADDRESS=https://github.com/Nemesus314/rusty-fish

fail() {
    echo "packaging: $*" >&2
    exit 1
}
say() { echo "== $*"; }

[ -f Cargo.toml ] && [ -d packaging ] || fail "run this from the folder of the repository: bash packaging/linux.sh"
for tool in cargo musl-gcc python3 git tar sha256sum readelf; do
    command -v "$tool" > /dev/null || fail "missing: $tool"
done

REPO="$PWD"
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)"
[ -n "$VERSION" ] || fail "cannot read the version from Cargo.toml"
# Ein Laufwerk, das Windows gehört, erscheint unter WSL als das eines anderen
# Besitzers; für diese Aufrufe gilt das Repository trotzdem.
gitc() { git -c safe.directory="$REPO" "$@"; }
COMMIT="$(gitc rev-parse HEAD)"
# Das Paket nennt den Commit, aus dem es gebaut ist. Mit offenen Änderungen
# stimmte diese Angabe nicht.
OPEN="$(gitc status --porcelain)"
if [ -n "$OPEN" ] && [ "${1:-}" != "--with-open-changes" ]; then
    fail "there are changes that are not committed; the package names its commit.
$OPEN"
fi

BUILD="${PACKAGE_BUILD_DIR:-$REPO/target/package-build}"
NAME="RustyFish-$VERSION-linux-x64"
PACKAGES="$REPO/target/package"
# Der Ordner wird im Linux-Dateisystem zusammengesetzt, nicht im Repository.
# Gemessen am 25.11.2026: lag er auf einem Laufwerk von Windows (WSL, /mnt/…),
# trug im Archiv jede Datei die Rechte 0777, für alle schreibbar und
# ausführbar, weil ein solches Laufwerk keine Rechte von Linux kennt.
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
OUT="$STAGE/$NAME"

say "building the fish"
# Mit dem Trennzeichen 0x1f, damit ein Pfad mit Leerzeichen heil bleibt. Für
# die Bausteine in C derselbe Gedanke mit dem Schalter des C-Compilers.
CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=$HOME=~"$'\x1f'"--remap-path-prefix=$REPO=rusty-fish" \
CFLAGS="-ffile-prefix-map=$HOME=~ -ffile-prefix-map=$REPO=rusty-fish" \
CARGO_PROFILE_RELEASE_STRIP="${PACKAGE_STRIP:-symbols}" \
    cargo build --release --locked -p rusty-fish --target "$TARGET" --target-dir "$BUILD" 2>&1 | tail -n 1
FISH="$BUILD/$TARGET/release/rusty-fish"
[ -x "$FISH" ] || fail "the build left no program at $FISH"

# Statisch heißt: das Programm nennt keine Bibliothek, die es braucht.
if readelf -d "$FISH" 2> /dev/null | grep -q NEEDED; then
    fail "the program is not statically linked: $(readelf -d "$FISH" | grep NEEDED | tr -s ' ' | tr '\n' ' ')"
fi
if grep -a -q -- "$HOME/" "$FISH" || grep -a -q -- "$REPO/" "$FISH"; then
    fail "the program still carries paths of this machine ($HOME or $REPO)"
fi

say "putting the folder together"
mkdir -m 755 "$OUT"
mkdir -p "$PACKAGES"
install -m 755 "$FISH" "$OUT/rusty-fish"
install -m 644 LICENSE GUIDE.md packaging/rusty-fish.service "$OUT/"
sed -e "s/{version}/$VERSION/g" -e "s/{commit}/$COMMIT/g" packaging/README-linux.txt > "$OUT/README.txt"

say "collecting the notices of the parts"
python3 packaging/notices.py "$TARGET" "$VERSION" "$COMMIT" rusty-fish > "$OUT/THIRD-PARTY-NOTICES.txt"
chmod 644 "$OUT/README.txt" "$OUT/THIRD-PARTY-NOTICES.txt"

# Ohne den Namen und die Nummer dieses Nutzers im Archiv.
tar -C "$STAGE" --owner=0 --group=0 --numeric-owner -czf "$PACKAGES/$NAME.tar.gz" "$NAME"
# Nur das Programm ist ausführbar, und nichts ist für andere schreibbar.
LISTED="$(tar -tvzf "$PACKAGES/$NAME.tar.gz")"
WRONG="$(printf '%s\n' "$LISTED" | grep -v -e '^drwxr-xr-x ' -e '^-rw-r--r-- ' -e '^-rwxr-xr-x .*/rusty-fish$' || true)"
[ -z "$WRONG" ] || fail "the archive carries rights it should not:
$WRONG"

say "done"
echo "file:    $PACKAGES/$NAME.tar.gz ($(stat -c %s "$PACKAGES/$NAME.tar.gz") bytes)"
echo "sha256:  $(sha256sum "$PACKAGES/$NAME.tar.gz" | cut -d ' ' -f 1)"
echo "version: $VERSION, commit $COMMIT"
echo "program: $("$OUT/rusty-fish" --version), $(stat -c %s "$OUT/rusty-fish") bytes, statically linked"
echo "source:  $ADDRESS"
