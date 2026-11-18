#!/usr/bin/env python3
"""Structural diff of two Quake version-5 savegames (ours, ironwail, vkQuake).

Compares the header (skill, map, time, light styles, spawn parms), the
globals block and every edict field by field. Floats and vectors match within
a tolerance; `// #n` comments (ironwail) are ignored, as are the comment line
and any trailing data after the edicts (QSPADDONS header, ironwail extras).

  savediff.py A.sav B.sav [--tol 0.01] [--ignore field,...] [--max N] [--json]

Exit 1 when they differ.
"""
import argparse
import json
import re
import sys

TOKEN = re.compile(r'"([^"]*)"|(//[^\n]*)|([{}])|(\S+)')


def parse(path):
    text = open(path, "rb").read().decode("latin-1")
    if text.startswith("QSPADDONS"):
        # customized (.qsa) save: header block, then the vanilla save
        text = text.split("\n", 1)[1]
        while text and not text.split("\n", 1)[0].strip().isdigit():
            text = text.split("\n", 1)[1]
    lines = text.split("\n")
    if lines[0].strip() == "6":
        # 2021 re-release (KEX): a game-dir line after the version
        lines = [lines[0]] + lines[2:]
    hdr = {"version": lines[0].strip(), "comment": lines[1]}
    hdr["parms"] = [float(x) for x in lines[2:18]]
    hdr["skill"] = lines[18].strip()
    hdr["map"] = lines[19].strip()
    hdr["time"] = float(lines[20])
    hdr["styles"] = [x.strip() for x in lines[21:85]]
    rest = "\n".join(lines[85:])
    blocks, cur, key, extra = [], None, None, ""
    pos = 0
    for m in TOKEN.finditer(rest):
        s, comment, brace, bare = m.groups()
        if comment is not None:
            continue
        if brace == "{":
            cur, key = {}, None
            continue
        if brace == "}":
            if cur is not None:
                blocks.append(cur)
            cur = None
            pos = m.end()
            continue
        if cur is None:
            extra = rest[m.start():]
            break
        tok = s if s is not None else bare
        if key is None:
            key = tok
        else:
            cur[key] = tok
            key = None
    hdr["extra"] = extra.strip()[:200]
    return hdr, blocks[0] if blocks else {}, blocks[1:]


def same(a, b, tol):
    if a == b:
        return True
    try:
        fa = [float(x) for x in a.split()]
        fb = [float(x) for x in b.split()]
    except ValueError:
        return False
    return len(fa) == len(fb) and all(abs(x - y) <= tol * max(1.0, abs(x), abs(y)) for x, y in zip(fa, fb))


def diff_dict(a, b, tol, ignore):
    out = []
    for k in sorted(set(a) | set(b)):
        if k in ignore:
            continue
        va, vb = a.get(k), b.get(k)
        # a missing field is a zero value (savegames omit zero fields)
        if va is None and vb is not None and same("0", vb, tol) or vb is None and va is not None and same(va, "0", tol):
            continue
        if va is None or vb is None or not same(va, vb, tol):
            out.append((k, va, vb))
    return out


def compare(pa, pb, tol=0.01, ignore=()):
    ha, ga, ea = parse(pa)
    hb, gb, eb = parse(pb)
    rep = {"header": [], "globals": diff_dict(ga, gb, tol, ignore), "edicts": [], "counts": (len(ea), len(eb))}
    for k in ("version", "skill", "map"):
        if ha[k] != hb[k]:
            rep["header"].append((k, ha[k], hb[k]))
    if not same(str(ha["time"]), str(hb["time"]), 1e-4):
        rep["header"].append(("time", ha["time"], hb["time"]))
    for i, (x, y) in enumerate(zip(ha["styles"], hb["styles"])):
        if x != y:
            rep["header"].append((f"lightstyle{i}", x, y))
    for i, (x, y) in enumerate(zip(ha["parms"], hb["parms"])):
        if abs(x - y) > 1e-4:
            rep["header"].append((f"parm{i + 1}", x, y))
    for n in range(max(len(ea), len(eb))):
        a = ea[n] if n < len(ea) else None
        b = eb[n] if n < len(eb) else None
        if a is None or b is None:
            rep["edicts"].append((n, (a if a is not None else b).get("classname", "?"), [("<edict>", "present" if a is not None else None, "present" if b is not None else None)]))
            continue
        d = diff_dict(a, b, tol, ignore)
        if d:
            rep["edicts"].append((n, a.get("classname") or b.get("classname") or "(free)", d))
    return rep


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("a")
    ap.add_argument("b")
    ap.add_argument("--tol", type=float, default=0.01)
    ap.add_argument("--ignore", default="")
    ap.add_argument("--max", type=int, default=40)
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()
    rep = compare(a.a, a.b, a.tol, set(filter(None, a.ignore.split(","))))
    if a.json:
        json.dump(rep, sys.stdout, indent=1)
        print()
    else:
        print(f"edicts: {rep['counts'][0]} vs {rep['counts'][1]}")
        for k, x, y in rep["header"]:
            print(f"header {k}: {x!r} vs {y!r}")
        for k, x, y in rep["globals"]:
            print(f"global {k}: {x!r} vs {y!r}")
        for n, cls, d in rep["edicts"][:a.max]:
            print(f"#{n} {cls}: " + "; ".join(f"{k}: {x!r} vs {y!r}" for k, x, y in d))
        if len(rep["edicts"]) > a.max:
            print(f"... {len(rep['edicts']) - a.max} more edicts differ")
    sys.exit(1 if rep["header"] or rep["globals"] or rep["edicts"] else 0)


if __name__ == "__main__":
    main()
