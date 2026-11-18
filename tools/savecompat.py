#!/usr/bin/env python3
"""Cross-engine savegame round trips: ours and ironwail load the same save.

For every savegame in the corpus, each engine runs as a one-slot dedicated
server, `load`s a copy, `save`s again at once (no frame runs in between) and
quits. Then three comparisons with `savediff.py`:

  original  vs ours     our loader + writer keep everything
  original  vs ironwail ironwail reads what the save says
  ours      vs ironwail both engines understand the save the same way

Corpus: our own saves (user dir, default ~/.local/share/quake-source-port),
saves other engines left in the game data (vkQuake writes `<game>/*.sav`),
and `--extra GAME:PATH`. Originals are only read: copies go into the
spawnparity sandbox (`target/compat/sandbox/<game>` for ironwail, the sandbox
XDG_DATA_HOME for ours). ironwail is the local `qsp-diag` build (see
spawnparity.py).

  savecompat.py [-b BASEDIR] [--userdir DIR] [--extra GAME:PATH ...] [-j N] [--out DIR] [game ...]

Writes OUT/report.md and OUT/report.json (default target/compat/saves).
"""
import argparse
import concurrent.futures as cf
import json
import os
import shutil
import sys

sys.dont_write_bytecode = True  # a tools/__pycache__ dir would break the cargo workspace glob
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import savediff  # noqa: E402
import spawnparity  # noqa: E402
from qdata import base_dir  # noqa: E402


def corpus(base, userdir, extra):
    out = []
    if os.path.isdir(userdir):
        for g in sorted(os.listdir(userdir)):
            d = os.path.join(userdir, g)
            if not os.path.isdir(d):
                continue
            for f in sorted(os.listdir(d)):
                if f.lower().endswith((".sav", ".qsa")):
                    out.append(("ours", g, os.path.join(d, f)))
    for g in sorted(os.listdir(base)):
        d = os.path.join(base, g)
        if os.path.isdir(d):
            for f in sorted(os.listdir(d)):
                if f.lower().endswith(".sav"):
                    out.append(("gamedata", g, os.path.join(d, f)))
    for e in extra:
        g, p = e.split(":", 1)
        out.append(("extra", g, p))
    return out


def real_game(base, g):
    """Our user dirs are lowercase; find the game dir's real name."""
    for e in os.listdir(base):
        if e.lower() == g.lower():
            return e
    return g


def first_line(path):
    with open(path, "rb") as f:
        return f.readline().decode("latin-1").strip()


def one(a, sandbox, userdata, origin, game, path, slot):
    stem = f"qspsc_{origin}_{os.path.splitext(os.path.basename(path))[0]}"
    ext = os.path.splitext(path)[1].lower()
    res = {"origin": origin, "game": game, "file": path, "version": first_line(path)}
    # our user dirs are the lowercased game dir name
    ours_dir = os.path.join(userdata, "quake-source-port", game.lower())
    iw_dir = os.path.join(sandbox, game)
    os.makedirs(ours_dir, exist_ok=True)
    shutil.copyfile(path, os.path.join(ours_dir, stem + ext))
    shutil.copyfile(path, os.path.join(iw_dir, stem + ".sav"))
    out_ours = os.path.join(ours_dir, stem + "_rt.sav")
    out_iw = os.path.join(iw_dir, stem + "_rt.sav")
    for f in (out_ours, out_iw):
        if os.path.exists(f):
            os.remove(f)
    ga = spawnparity.game_args(game)
    port = ["-port", str(27900 + slot % 64)]
    common = ["-dedicated", "1", "+sv_autosave", "0"]
    env = dict(os.environ, XDG_DATA_HOME=userdata)
    _, o1 = spawnparity.run([a.ours, "-basedir", sandbox] + port + ga + common + ["+load", stem + ext, "+save", stem + "_rt", "+quit"], env, sandbox)
    _, o2 = spawnparity.run([a.ironwail, "-basedir", "."] + port + ga + common + ["+load", stem, "+save", stem + "_rt", "+quit"], env, sandbox)
    res["ours_loaded"] = os.path.exists(out_ours)
    res["iw_loaded"] = os.path.exists(out_iw)
    if not res["ours_loaded"]:
        res["ours_tail"] = [l for l in o1.strip().splitlines() if l.strip()][-4:]
    if not res["iw_loaded"]:
        res["iw_tail"] = [l for l in o2.strip().splitlines() if l.strip()][-4:]
    pairs = {"orig_vs_ours": (path, out_ours), "orig_vs_iw": (path, out_iw), "ours_vs_iw": (out_ours, out_iw)}
    for k, (x, y) in pairs.items():
        if os.path.exists(x) and os.path.exists(y):
            try:
                r = savediff.compare(x, y, a.tol)
                res[k] = {"n": len(r["header"]) + len(r["globals"]) + len(r["edicts"]), "counts": r["counts"],
                          "header": r["header"][:8], "globals": r["globals"][:8], "edicts": r["edicts"][:10]}
            except Exception as e:  # noqa: BLE001 - report unparseable saves
                res[k] = {"n": -1, "error": f"{type(e).__name__}: {e}"}
    return res


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("-b", "--basedir")
    ap.add_argument("--userdir", default=os.path.expanduser("~/.local/share/quake-source-port"))
    ap.add_argument("--ironwail", default="ironwail/Quake/ironwail")
    ap.add_argument("--ours", default="target/release/quake-source-port")
    ap.add_argument("--extra", action="append", default=[])
    ap.add_argument("-j", type=int, default=6)
    ap.add_argument("--tol", type=float, default=0.01)
    ap.add_argument("--out", default="target/compat/saves")
    ap.add_argument("games", nargs="*")
    a = ap.parse_args()
    a.ironwail, a.ours = os.path.abspath(a.ironwail), os.path.abspath(a.ours)
    base = base_dir(a.basedir)
    sandbox = spawnparity.build_sandbox(base, os.path.abspath("target/compat/sandbox"))
    userdata = os.path.abspath("target/compat/userdata")
    work = []
    for origin, g, p in corpus(base, a.userdir, a.extra):
        g = real_game(base, g)
        if a.games and g.lower() not in [x.lower() for x in a.games]:
            continue
        if not os.path.isdir(os.path.join(sandbox, g)) or "bugreports" in p:
            continue
        work.append((origin, g, p))
    print(f"savecompat: {len(work)} saves", file=sys.stderr)
    results = []
    with cf.ThreadPoolExecutor(a.j) as ex:
        futs = [ex.submit(one, a, sandbox, userdata, o, g, p, i) for i, (o, g, p) in enumerate(work)]
        for f in cf.as_completed(futs):
            r = f.result()
            results.append(r)
            print(f"{r['game']}/{os.path.basename(r['file'])}: ours={r['ours_loaded']} iw={r['iw_loaded']} "
                  + " ".join(f"{k}={r[k]['n']}" for k in ("orig_vs_ours", "orig_vs_iw", "ours_vs_iw") if k in r), file=sys.stderr)
    results.sort(key=lambda r: (r["game"], r["file"]))
    os.makedirs(a.out, exist_ok=True)
    with open(os.path.join(a.out, "report.json"), "w") as f:
        json.dump(results, f, indent=1, default=str)
    lines = ["# Savegame round trips (ours, ironwail)", "",
             "| game | save | from | ver | ours loads | ironwail loads | orig→ours | orig→iw | ours↔iw |", "|---|---|---|---|---|---|---|---|---|"]
    for r in results:
        c = [str(r.get(k, {}).get("n", "-")) for k in ("orig_vs_ours", "orig_vs_iw", "ours_vs_iw")]
        lines.append(f"| {r['game']} | {os.path.basename(r['file'])} | {r['origin']} | {r['version'][:10]} | {r['ours_loaded']} | {r['iw_loaded']} | " + " | ".join(c) + " |")
    lines += ["", "## Differences"]
    for r in results:
        for k in ("orig_vs_ours", "orig_vs_iw", "ours_vs_iw"):
            d = r.get(k)
            if d and d["n"]:
                lines.append(f"### {r['game']}/{os.path.basename(r['file'])} {k}: {d['n']} (edicts {d.get('counts')})")
                for x in d.get("header", []) + d.get("globals", []):
                    lines.append(f"- {x[0]}: {x[1]!r} / {x[2]!r}")
                for n, cls, dd in d.get("edicts", []):
                    lines.append(f"- #{n} {cls}: " + "; ".join(f"{q}: {u!r} / {v!r}" for q, u, v in dd[:5]))
        for k in ("ours_tail", "iw_tail"):
            if r.get(k):
                lines.append(f"- {r['game']}/{os.path.basename(r['file'])} {k}: {r[k]}")
    with open(os.path.join(a.out, "report.md"), "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"savecompat: wrote {a.out}/report.md", file=sys.stderr)


if __name__ == "__main__":
    main()
