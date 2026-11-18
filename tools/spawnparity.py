#!/usr/bin/env python3
"""Spawn-state parity against ironwail: every playable map, both engines.

Each engine runs as a one-slot dedicated server (`-dedicated 1`), loads the
map (SV_SpawnServer: spawn functions plus two 0.1 s settle frames, independent
of frame rate), saves at once and quits. The two savegames are compared field
by field with `savediff.py`, so a difference is a difference in spawn
behaviour (spawn functions, builtins, droptofloor, setmodel sizes, precache,
physics) or in the save format itself.

ironwail must be the local diagnostic build (branch `qsp-diag` in ironwail/,
`make DO_USERDIRS=0`): `qsp_rng 1` gives QC `random()` and movetogoal our LCG
and per-map seed, and DO_USERDIRS=0 keeps its saves in the sandbox. Ours runs
with XDG_DATA_HOME in the sandbox. Nothing is written to the game data or the
user's configs and saves.

  spawnparity.py [-b BASEDIR] [--ironwail BIN] [--ours BIN] [-j N] [--skill N] [--frames N]
                 [--out DIR] [game[:map] ...]

Extra maps (e.g. the AD and Alkaline dev-kit test maps) can be unpacked into
target/compat/sandbox/<game>/maps; they are swept too.

Writes OUT/report.md and OUT/report.json (default target/compat/spawn).
"""
import argparse
import concurrent.futures as cf
import json
import os
import re
import subprocess
import sys

sys.dont_write_bytecode = True  # a tools/__pycache__ dir would break the cargo workspace glob
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import compat_audit  # noqa: E402
import savediff  # noqa: E402
from qdata import base_dir  # noqa: E402

ENGINE_CONFIGS = {"config.cfg", "vkquake.cfg", "ironwail.cfg", "autoexec.cfg"}
PACK_FLAGS = {"hipnotic": ["-hipnotic"], "rogue": ["-rogue"], "quoth": ["-quoth"]}


def packs_readable(gdir):
    """False when a pakN.pak's directory lies outside the file (truncated)."""
    for e in os.listdir(gdir):
        if re.fullmatch(r"pak\d+\.pak", e.lower()):
            p = os.path.join(gdir, e)
            with open(p, "rb") as f:
                hdr = f.read(12)
            if len(hdr) < 12 or hdr[:4] != b"PACK":
                return False
            ofs, ln = int.from_bytes(hdr[4:8], "little"), int.from_bytes(hdr[8:12], "little")
            if ofs + ln > os.path.getsize(p):
                return False
    return True


def build_sandbox(base, root):
    """root/<game>/ = real dirs holding a symlink per game data entry, minus
    engine configs and saves, so engines start from defaults and write here."""
    os.makedirs(root, exist_ok=True)
    for g in sorted(os.listdir(base)):
        src = os.path.join(base, g)
        if not os.path.isdir(src) or os.path.islink(src) and not os.path.isdir(os.path.realpath(src)):
            continue
        if not packs_readable(src):
            print(f"spawnparity: leaving {g} out of the sandbox (unreadable pak; ironwail aborts on it)", file=sys.stderr)
            continue
        dst = os.path.join(root, g)
        os.makedirs(dst, exist_ok=True)
        for e in os.listdir(src):
            low = e.lower()
            if low in ENGINE_CONFIGS or low.endswith((".sav", ".qsa", ".png", ".tga", ".jpg")):
                continue
            link = os.path.join(dst, e)
            if not os.path.lexists(link):
                os.symlink(os.path.abspath(os.path.join(src, e)), link)
    return root


def playable_maps(base, game):
    out = []
    d = os.path.join(base, game)
    for rel, _src, read in compat_audit.own_files(d):
        if rel.startswith("maps/") and rel.endswith(".bsp") and rel.count("/") == 1:
            name = rel[5:-4]
            if name.startswith("b_"):
                continue
            data = read()
            try:
                ents = compat_audit.bsp_info(data)["entities"]
            except Exception:  # noqa: BLE001 - report unreadable maps as such
                out.append(name)
                continue
            if any(dict(e).get("classname", "").startswith("info_player_") for e in ents):
                out.append(name)
    return sorted(set(out))


def game_args(game):
    if game.lower() in PACK_FLAGS:
        return PACK_FLAGS[game.lower()]
    return [] if game.lower() == "id1" else ["-game", game]


ERR = re.compile(r"host_error|server error|object error|sys_error|panicked|segmentation|couldn't spawn|cannot find map|bad version", re.I)


def run(cmd, env, cwd, timeout=60):
    try:
        p = subprocess.run(cmd, cwd=cwd, env=env, stdin=subprocess.DEVNULL, capture_output=True, timeout=timeout)
        out = (p.stdout + p.stderr).decode("latin-1")
        return p.returncode, out
    except subprocess.TimeoutExpired as e:
        return "timeout", ((e.stdout or b"") + (e.stderr or b"")).decode("latin-1")


def one(a, sandbox, userdata, game, mapname, slot=0):
    tag = f"qspspawn_{mapname}"
    ga = game_args(game)
    base = ["-dedicated", "1", "+sv_autosave", "0", "+skill", str(a.skill)]
    common = base + ["+map", mapname, "+save", tag, "+quit"]
    common_ours = common
    if a.frames:
        # N server frames of 1/72 s, then save. Both engines run the same
        # script from a cfg (ironwail keeps only 256 chars of command line);
        # both run a server frame in the frame that executes `map`.
        def script(engine, waits):
            name = f"qsprun_{mapname}_{engine}.cfg"
            with open(os.path.join(sandbox, game, name), "w") as f:
                f.write(f"host_framerate {1 / 72}\nmap {mapname}\n" + "wait\n" * waits + f"save {tag}\nquit\n")
            return base + ["+exec", name]
        common_ours = script("ours", a.frames)
        common = script("iw", a.frames)
    env = dict(os.environ, XDG_DATA_HOME=userdata)
    ours_sav = os.path.join(userdata, "quake-source-port", game.lower(), tag + ".sav")  # our user dirs are lowercase
    iw_sav = os.path.join(sandbox, game, tag + ".sav")
    for f in (ours_sav, iw_sav):
        if os.path.exists(f):
            os.remove(f)
    port = ["-port", str(27600 + slot % 64)]  # parallel dedicated servers each need a UDP port
    # sv_fteparticles 0: advertise the same extensions as ironwail, so mods
    # that detect the engine (AD) take the same path in both
    rc1, out1 = run([a.ours, "-basedir", sandbox] + port + ga + ["+sv_fteparticles", "0"] + common_ours, env, sandbox)
    # r_enhancedmodels 0: ironwail would take setmodel bounds from a .md5mesh
    # replacement's bind pose; ours and vkQuake keep the .mdl's
    rc2, out2 = run([a.ironwail, "-basedir", "."] + port + ga + ["+qsp_rng", "1", "+r_enhancedmodels", "0"] + common, env, sandbox)
    res = {"game": game, "map": mapname, "ours_rc": str(rc1), "iw_rc": str(rc2),
           "ours_errors": sorted(set(l.strip() for l in out1.splitlines() if ERR.search(l)))[:5],
           "iw_errors": sorted(set(l.strip() for l in out2.splitlines() if ERR.search(l)))[:5]}
    if not os.path.exists(ours_sav) or not os.path.exists(iw_sav):
        res["status"] = f"nosave ours={os.path.exists(ours_sav)} iw={os.path.exists(iw_sav)}"
        res["ours_tail"] = out1.strip().splitlines()[-3:]
        res["iw_tail"] = out2.strip().splitlines()[-3:]
        return res
    rep = savediff.compare(ours_sav, iw_sav, a.tol)
    res["counts"] = rep["counts"]
    res["header"] = rep["header"]
    res["globals"] = rep["globals"]
    res["edicts"] = rep["edicts"][:60]
    res["n_edicts_diff"] = len(rep["edicts"])
    res["status"] = "same" if not (rep["header"] or rep["globals"] or rep["edicts"]) else "differ"
    os.remove(ours_sav)
    os.remove(iw_sav)
    return res


def write_report(out, results):
    os.makedirs(out, exist_ok=True)
    with open(os.path.join(out, "report.json"), "w") as f:
        json.dump(results, f, indent=1)
    from collections import Counter
    st = Counter(r["status"].split()[0] for r in results)
    lines = ["# Spawn-state parity (ours vs ironwail)", "", f"{len(results)} maps: {dict(st)}", ""]
    fields = Counter()
    for r in results:
        for _n, cls, d in r.get("edicts", []):
            for k, _x, _y in d:
                fields[(cls, k)] += 1
        for k, _x, _y in r.get("globals", []):
            fields[("<global>", k)] += 1
    lines.append("## Most frequent differing (classname, field)")
    lines += [f"- {c} `{k}`: {n}" for (c, k), n in fields.most_common(60)]
    lines.append("")
    lines.append("## Per map")
    for r in results:
        if r["status"] == "same":
            continue
        lines.append(f"### {r['game']}/{r['map']}: {r['status']}" + (f" ({r.get('n_edicts_diff', 0)} edicts, counts {r.get('counts')})" if "counts" in r else ""))
        for k in ("ours_errors", "iw_errors", "ours_tail", "iw_tail"):
            if r.get(k):
                lines.append(f"- {k}: {r[k]}")
        for k, x, y in r.get("header", []) + r.get("globals", []):
            lines.append(f"- {k}: ours {x!r} / ironwail {y!r}")
        for n, cls, d in r.get("edicts", [])[:12]:
            lines.append(f"- #{n} {cls}: " + "; ".join(f"{k}: {x!r} / {y!r}" for k, x, y in d[:6]))
        lines.append("")
    with open(os.path.join(out, "report.md"), "w") as f:
        f.write("\n".join(lines))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("-b", "--basedir")
    ap.add_argument("--ironwail", default="ironwail/Quake/ironwail")
    ap.add_argument("--ours", default="target/release/quake-source-port")
    ap.add_argument("-j", type=int, default=8)
    ap.add_argument("--skill", type=int, default=1)
    ap.add_argument("--frames", type=int, default=0, help="also run N server frames (1/72 s) before saving")
    ap.add_argument("--tol", type=float, default=0.01)
    ap.add_argument("--out", default="target/compat/spawn")
    ap.add_argument("filters", nargs="*")
    a = ap.parse_args()
    a.ironwail = os.path.abspath(a.ironwail)
    a.ours = os.path.abspath(a.ours)
    base = base_dir(a.basedir)
    sandbox = build_sandbox(base, os.path.abspath("target/compat/sandbox"))
    userdata = os.path.abspath("target/compat/userdata")
    work = []
    # maps come from the sandbox, so extra test maps unpacked into
    # target/compat/sandbox/<game>/maps (AD/Alkaline dev kits) are included
    for g in sorted(os.listdir(sandbox)):
        if not os.path.isdir(os.path.join(sandbox, g)) or g == "quake-source-port":
            continue
        want = [f.split(":", 1)[1] if ":" in f else "" for f in a.filters if f.split(":", 1)[0].lower() == g.lower()]
        if a.filters and not want:
            continue
        for m in playable_maps(sandbox, g):
            if not want or "" in want or m in want:
                work.append((g, m))
    print(f"spawnparity: {len(work)} maps", file=sys.stderr)
    results = []
    with cf.ThreadPoolExecutor(a.j) as ex:
        futs = {ex.submit(one, a, sandbox, userdata, g, m, i): (g, m) for i, (g, m) in enumerate(work)}
        for i, f in enumerate(cf.as_completed(futs)):
            try:
                r = f.result()
            except Exception as e:  # noqa: BLE001 - one bad map must not lose the sweep
                g, m = futs[f]
                r = {"game": g, "map": m, "status": f"tool-error {type(e).__name__}: {e}"}
            results.append(r)
            print(f"[{i + 1}/{len(work)}] {r['game']}/{r['map']}: {r['status']}" + (f" ({r.get('n_edicts_diff')} edicts)" if r.get("n_edicts_diff") else ""), file=sys.stderr)
    results.sort(key=lambda r: (r["game"], r["map"]))
    write_report(a.out, results)
    print(f"spawnparity: wrote {a.out}/report.md", file=sys.stderr)


if __name__ == "__main__":
    main()
