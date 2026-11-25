#!/usr/bin/env python3
# Die Hinweise aller fremden Bausteine, aus denen der Fisch für ein Ziel gebaut
# ist, auf die Standardausgabe.
#
#     python3 packaging/notices.py ZIEL VERSION COMMIT PROGRAMM
#
# Für das Linux-Paket (packaging/linux.sh). Dieselben Regeln wie in
# packaging/windows.dart, dort in Dart, weil es unter Windows mit Flutter schon
# da ist; unter Linux ist es Python. Wer eine der beiden Fassungen ändert,
# ändert die andere mit.
#
# - Gezählt wird, was Cargo für das Ziel wirklich ins Programm baut: von
#   rusty-fish aus über die gewöhnlichen Abhängigkeiten, ohne die, die nur beim
#   Bauen oder nur in Tests gebraucht werden. Die Liste ist eher zu lang als zu
#   kurz.
# - Die Texte stammen aus den Dateien, die jeder Baustein selbst mitbringt
#   (LICENSE, COPYING, NOTICE und ähnliche). Gleiche Texte stehen einmal da.
#   Bringt ein Baustein keine mit, steht er mit seiner Lizenzangabe in einer
#   eigenen Liste.

import json
import os
import re
import subprocess
import sys


def main():
    if len(sys.argv) != 5:
        sys.exit("usage: notices.py TARGET VERSION COMMIT PROGRAM")
    target, version, commit, program = sys.argv[1:]
    metadata = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target],
            check=True,
            capture_output=True,
        ).stdout
    )
    by_id = {package["id"]: package for package in metadata["packages"]}
    members = set(metadata["workspace_members"])
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}

    start = next((i for i in members if by_id[i]["name"] == "rusty-fish"), None)
    if start is None:
        sys.exit("notices: cannot find the package rusty-fish")
    seen = {start}
    queue = [start]
    while queue:
        for dep in nodes[queue.pop()]["deps"]:
            # `kind` ist bei einem gewöhnlichen Baustein leer; „build" und
            # „dev" kommen nicht ins fertige Programm.
            built = any(kind["kind"] is None for kind in dep["dep_kinds"])
            if built and dep["pkg"] not in seen:
                seen.add(dep["pkg"])
                queue.append(dep["pkg"])

    foreign = sorted(
        (by_id[i] for i in seen if i not in members),
        key=lambda p: f"{p['name']} {p['version']}".lower(),
    )

    is_notice = re.compile(r"^(licen[sc]e|copying|notice|unlicense|copyright)", re.IGNORECASE)
    texts = {}  # Text -> wer ihn trägt, in der Reihenfolge des ersten Vorkommens
    without = []
    for package in foreign:
        label = f"{package['name']} {package['version']}"
        folder = os.path.dirname(package["manifest_path"])
        found = False
        for name in sorted(os.listdir(folder)):
            path = os.path.join(folder, name)
            if not is_notice.match(name) or not os.path.isfile(path) or os.path.getsize(path) >= 300 * 1024:
                continue
            try:
                with open(path, encoding="utf-8") as file:
                    text = file.read().replace("\r\n", "\n").strip()
            except UnicodeDecodeError:
                continue  # kein Text
            if not text:
                continue
            texts.setdefault(text, []).append(f"{label} ({name})")
            found = True
        if not found:
            without.append(f"{label}: {package.get('license') or 'no license named'}")

    out = [
        f"Notices of the parts the fish ({program}) is built from",
        f"Rusty Fish {version}, commit {commit}",
        "",
        f"{program} is built from the {len(foreign)} packages listed below, written by",
        "others, each under the license named next to it. Their license texts follow",
        "the list; a text that several packages share is printed once. This file was",
        f"made by packaging/notices.py from what `cargo metadata` names for {target}.",
        "The list errs on the side of too many: it names a few packages that Cargo",
        "resolves for this program without building them into it.",
        "",
        "It also contains the Rust standard library, under MIT OR Apache-2.0,",
        "copyright The Rust Project contributors: https://www.rust-lang.org/policies/licenses",
        "",
        "PACKAGES",
        "",
    ]
    for package in foreign:
        line = f"{package['name']} {package['version']}  |  {package.get('license') or 'see its files'}"
        if package.get("repository"):
            line += f"  |  {package['repository']}"
        out.append(line)
    if without:
        out += [
            "",
            "PACKAGES THAT BRING NO LICENSE FILE OF THEIR OWN",
            "For these the license named here applies; its text is among those below.",
            "",
        ]
        out += without
    out += ["", "LICENSE TEXTS"]
    for text, used_by in texts.items():
        out += ["", "=" * 76, "Used by: " + ", ".join(used_by), "=" * 76, "", text]

    sys.stdout.write("\n".join(out) + "\n")
    sys.stderr.write(
        f"parts:   {len(foreign)} packages, {len(texts)} different license texts, "
        f"{len(without)} packages without a license file of their own\n"
    )


if __name__ == "__main__":
    main()
