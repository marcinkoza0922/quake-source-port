#!/usr/bin/env python3
"""Per-language translation files: make a template, or check one.

The port's own text is keyed by English in crates/qcore/lang/*.txt (French,
German, Italian, Spanish and Russian live there too). Every other language
has one file, crates/qcore/lang/languages/<code>.txt, with the same `@ key`
lines and one `<code> text` line after each (format: qcore::lang::catalog).

    tools/lang_check.py template <code>   write a skeleton (keys + comments)
    tools/lang_check.py check <code>...   check files and docs/MANUAL.<code>.md
    tools/lang_check.py sync <code>...    add new keys / drop stale ones

`check` needs no Rust build: keys, one line per key, `{}` arguments,
trailing `\\n`, and the manual's `##`/`###` structure against MANUAL.md.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LANG = os.path.join(ROOT, "crates", "qcore", "lang")
MAIN = ["menu.txt", "tooltips.txt", "ui.txt", "hud.txt", "console.txt"]


def main_catalog():
    """[(file, [comment lines], key)] in catalog order."""
    out = []
    for name in MAIN:
        comments = []
        with open(os.path.join(LANG, name), encoding="utf-8") as f:
            for line in f.read().split("\n"):
                if line.startswith("@ "):
                    out.append((name, comments, line[2:]))
                    comments = []
                elif line.startswith("#") and not line.startswith("# Translations of the port's own text"):
                    comments.append(line)
    return out


def args(s):
    return sorted(re.findall(r"\{[^}]*\}?", s))


def path_for(code):
    return os.path.join(LANG, "languages", f"{code}.txt")


def header(code):
    return [
        f"# {code}: the port's own text; English keys from crates/qcore/lang/*.txt.",
        "# Format in crates/qcore/src/lang/catalog.rs; check with tools/lang_check.py.",
    ]


def template(code, existing=None):
    existing = existing or {}
    lines = header(code)
    last = None
    for name, comments, key in main_catalog():
        if name != last:
            lines += ["", f"# ==== {name} ===="]
            last = name
        lines.append("")
        lines += comments
        lines.append(f"@ {key}")
        if key in existing:
            lines.append(f"{code} {existing[key]}")
    return "\n".join(lines) + "\n"


def parse(code, text):
    """({key: text}, [problems])."""
    got, problems, key = {}, [], None
    for n, line in enumerate(text.split("\n"), 1):
        if not line.strip() or line.startswith("#"):
            continue
        if line.startswith("@ "):
            key = line[2:]
            if key in got:
                problems.append(f"{n}: duplicate key {key!r}")
            got.setdefault(key, None)
            continue
        c, _, rest = line.partition(" ")
        if c != code:
            problems.append(f"{n}: unknown line {line!r}")
        elif key is None:
            problems.append(f"{n}: translation before the first @ key")
        elif got.get(key) is not None:
            problems.append(f"{n}: second {code} line for {key!r}")
        else:
            got[key] = rest
    return got, problems


def check_catalog(code):
    p = path_for(code)
    if not os.path.exists(p):
        return [f"{p}: missing"]
    got, problems = parse(code, open(p, encoding="utf-8").read())
    keys = [k for _, _, k in main_catalog()]
    want = set(keys)
    for k in keys:
        t = got.get(k)
        if k not in got:
            problems.append(f"missing key {k!r}")
        elif t is None or not t.strip():
            problems.append(f"untranslated {k!r}")
        else:
            if args(k) != args(t):
                problems.append(f"arguments differ: {k!r} -> {t!r}")
            if k.endswith("\\n") != t.endswith("\\n"):
                problems.append(f"trailing \\n differs: {k!r} -> {t!r}")
            if t != t.strip(" ") and k == k.strip(" "):
                problems.append(f"leading/trailing spaces: {t!r}")
    for k in got:
        if k not in want:
            problems.append(f"stale key {k!r}")
    return [f"{os.path.relpath(p, ROOT)}: {x}" for x in problems]


def manual_shape(path):
    secs, subs, problems, cur = 0, [], [], None
    for line in open(path, encoding="utf-8").read().split("\n"):
        t = line.strip()
        if t.startswith("## "):
            secs += 1
            subs.append(0)
            cur = 0
        elif t.startswith("### "):
            if subs:
                subs[-1] += 1
        elif t and cur is not None:
            cur += 1
    return secs, subs


def check_manual(code):
    p = os.path.join(ROOT, "docs", f"MANUAL.{code}.md")
    if not os.path.exists(p):
        return [f"{os.path.relpath(p, ROOT)}: missing"]
    want = manual_shape(os.path.join(ROOT, "docs", "MANUAL.md"))
    got = manual_shape(p)
    out = []
    if got[0] != want[0]:
        out.append(f"{got[0]} ## sections, English has {want[0]}")
    elif got[1] != want[1]:
        out.append(f"### headings per section {got[1]}, English {want[1]}")
    text = open(p, encoding="utf-8").read()
    blocks = re.split(r"^## .*$", text, flags=re.M)
    for i, b in enumerate(blocks[1:], 1):
        if not b.strip():
            out.append(f"section {i} is empty")
    return [f"{os.path.relpath(p, ROOT)}: {x}" for x in out]


def main(argv):
    if len(argv) < 3 or argv[1] not in ("template", "check", "sync"):
        print(__doc__)
        return 2
    cmd, codes = argv[1], argv[2:]
    bad = 0
    for code in codes:
        if cmd == "template":
            p = path_for(code)
            if os.path.exists(p):
                print(f"{p} exists; use sync")
                bad += 1
                continue
            os.makedirs(os.path.dirname(p), exist_ok=True)
            open(p, "w", encoding="utf-8").write(template(code))
            print(f"wrote {os.path.relpath(p, ROOT)}")
        elif cmd == "sync":
            p = path_for(code)
            got, _ = parse(code, open(p, encoding="utf-8").read())
            open(p, "w", encoding="utf-8").write(template(code, {k: v for k, v in got.items() if v is not None}))
            print(f"synced {os.path.relpath(p, ROOT)}")
        else:
            problems = check_catalog(code) + check_manual(code)
            for x in problems[:60]:
                print(x)
            if len(problems) > 60:
                print(f"... {len(problems) - 60} more")
            print(f"{code}: {'OK' if not problems else f'{len(problems)} problem(s)'}")
            bad += bool(problems)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
