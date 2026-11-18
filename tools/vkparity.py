#!/usr/bin/env python3
"""Server-side content parity against vkQuake: every playable map of a game.

Each map runs in vkQuake (windowed in a headless gamescope, one run at a
time) and in ours (headless) for the same number of server frames, then both
dump every edict (`edicts`). The report compares edict counts per classname
and the engine the mod detected (AD prints "Advanced Engine" or "Fitz
Compatible" with `developer 1`), so it shows content a mod turns on only for
one engine (extension checks such as checkextension("FTE_STRINGS")).

vkQuake needs a trick to be scripted: its server stuffs `cmd pext` during
signon and waits for the reply, which would sit behind the script's `wait`s;
so the script sends `cmd pext` itself right after `map` (the server takes it
any time before the player spawns, and ignores the late duplicate).

vkQuake is never killed (a killed vkQuake once left the GPU needing a reset):
when a run doesn't quit by itself within --timeout, the sweep stops and
reports the PID. Both engines run on the compat sandbox (no engine configs)
with an overlay game dir holding a quake.rc with `stuffcmds` (AD's has none)
and the scripts; ours gets XDG_DATA_HOME in the work dir. Nothing is written
to the game data or the user's configs and saves.

  vkparity.py [--game ad] [--frames 720] [--vkquake BIN] [--ours BIN]
              [--timeout S] [--out DIR] [map ...]

Writes OUT/report.md, OUT/report.json and OUT/runs/<map>_{vk,ours}.txt
(default target/compat/vkparity/<game>).
"""
import argparse
import collections
import json
import os
import re
import subprocess
import sys
import time

sys.dont_write_bytecode = True  # a tools/__pycache__ dir would break the cargo workspace glob
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from qdata import base_dir  # noqa: E402
from spawnparity import build_sandbox, playable_maps  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OVERLAY = "qspcmp"
ENGINE_LINES = re.compile(r"Fitz Compatible|Advanced Engine|Engine does not support [a-z]+")
ERRORS = re.compile(r"host_error|server error|sys_error|panicked|couldn't spawn|cannot find map|bad version", re.I)
SIZE = (640, 480)


def setup(work, sandbox, game):
    """work/{id1,<game>} link to the sandbox; work/OVERLAY is the scratch dir."""
    os.makedirs(os.path.join(work, OVERLAY), exist_ok=True)
    for g in {"id1", game}:
        link = os.path.join(work, g)
        if not os.path.lexists(link):
            os.symlink(os.path.join(sandbox, g), link)
    with open(os.path.join(work, OVERLAY, "quake.rc"), "w") as f:
        f.write("exec default.cfg\nexec config.cfg\nexec autoexec.cfg\nstuffcmds\n")


def script(work, name, mapname, frames, pext):
    head = "skill 1\ndeveloper 1\nmap %s\n" % mapname
    # vkQuake: answer the signon's protocol-extension query ourselves
    lead = "wait\nwait\ncmd pext\n" if pext else "wait\nwait\n"
    body = lead + "wait\n" * (frames - 2) + "edictcount\nedicts\nwait\nquit\n"
    with open(os.path.join(work, OVERLAY, name), "w") as f:
        f.write(head + body)
    return name


def parse(text):
    classes = collections.Counter()
    in_edict = False
    for line in text.splitlines():
        if line.startswith("EDICT "):
            in_edict = True
        elif in_edict and line.startswith("classname"):
            classes[line.split(None, 1)[1].strip() if len(line.split(None, 1)) > 1 else ""] += 1
            in_edict = False
    num = re.search(r"num_edicts\s*:\s*(\d+)", text)
    return {
        "engine": sorted(set(ENGINE_LINES.findall(text))),
        "num_edicts": int(num.group(1)) if num else None,
        "classes": classes,
        "errors": sorted({m.group(0).lower() for m in ERRORS.finditer(text)}),
    }


def run_vk(a, work, game, mapname, log):
    cfg = script(work, f"vk_{mapname}.cfg", mapname, a.frames, True)
    w, h = SIZE
    cmd = ["gamescope", "--backend", "headless", "-W", str(w), "-H", str(h), "-w", str(w), "-h", str(h), "--",
           "stdbuf", "-oL", a.vkquake, "-basedir", ".", "-game", game, "-game", OVERLAY,
           "-window", "-width", str(w), "-height", str(h), "+exec", cfg]
    with open(log, "w") as out:
        p = subprocess.Popen(cmd, cwd=work, stdin=subprocess.DEVNULL, stdout=out, stderr=subprocess.STDOUT)
        start = time.time()
        while p.poll() is None:
            if time.time() - start > a.timeout:
                return p.pid
            time.sleep(0.5)
    return None


def run_ours(a, work, game, mapname, log):
    cfg = script(work, f"ours_{mapname}.cfg", mapname, a.frames, False)
    w, h = SIZE
    cmd = [a.ours, "-basedir", ".", "-game", game, "-game", OVERLAY, "-headless", "-nosound",
           "-width", str(w), "-height", str(h), "+host_maxfps", "72", "+sv_autosave", "0", "+exec", cfg]
    env = dict(os.environ, XDG_DATA_HOME=os.path.join(work, "userdata"))
    with open(log, "w") as out:
        try:
            subprocess.run(cmd, cwd=work, env=env, stdin=subprocess.DEVNULL, stdout=out, stderr=subprocess.STDOUT,
                           timeout=a.timeout)
        except subprocess.TimeoutExpired:
            out.write("\nvkparity: timeout\n")


def compare(vk, ours):
    names = set(vk["classes"]) | set(ours["classes"])
    diff = {n: (vk["classes"].get(n, 0), ours["classes"].get(n, 0)) for n in names
            if vk["classes"].get(n, 0) != ours["classes"].get(n, 0)}
    return dict(sorted(diff.items(), key=lambda kv: -abs(kv[1][0] - kv[1][1])))


def write_report(out, game, frames, results):
    with open(os.path.join(out, "report.json"), "w") as f:
        json.dump(results, f, indent=1)
    lines = [f"# vkQuake content parity: {game}", "",
             f"{len(results)} maps, {frames} server frames (1/72 s) after `map`, skill 1. "
             "Classes differing by more than one edict (or present in only one engine) are listed; "
             "single-edict differences are usually timing (temporary entities).", "",
             "| map | engine (vk / ours) | edicts vk | ours | differing classes |", "|---|---|---|---|---|"]
    for r in results:
        sig = {n: d for n, d in r["diff"].items() if abs(d[0] - d[1]) > 1 or 0 in d}
        cls = ", ".join(f"{n} {d[0]}/{d[1]}" for n, d in list(sig.items())[:8]) or "-"
        eng = f"{'+'.join(r['vk']['engine']) or '?'} / {'+'.join(r['ours']['engine']) or '?'}"
        err = " **errors: " + ", ".join(r["vk"]["errors"] + r["ours"]["errors"]) + "**" if r["vk"]["errors"] or r["ours"]["errors"] else ""
        lines.append(f"| {r['map']} | {eng} | {r['vk']['num_edicts']} | {r['ours']['num_edicts']} | {cls}{err} |")
    with open(os.path.join(out, "report.md"), "w") as f:
        f.write("\n".join(lines) + "\n")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("maps", nargs="*")
    ap.add_argument("--game", default="ad")
    ap.add_argument("-b", "--basedir")
    ap.add_argument("--frames", type=int, default=720)
    ap.add_argument("--vkquake")
    ap.add_argument("--ours", default=os.path.join(ROOT, "target/release/quake-source-port"))
    ap.add_argument("--timeout", type=int, default=240)
    ap.add_argument("--out")
    a = ap.parse_args()
    base = base_dir(a.basedir)
    a.vkquake = os.path.abspath(a.vkquake or os.path.join(base, "vkquake.AppImage"))
    a.ours = os.path.abspath(a.ours)
    out = os.path.abspath(a.out or os.path.join(ROOT, "target/compat/vkparity", a.game))
    os.makedirs(os.path.join(out, "runs"), exist_ok=True)
    sandbox = build_sandbox(base, os.path.join(ROOT, "target/compat/sandbox"))
    work = os.path.join(ROOT, "target/compat/vkwork")
    setup(work, sandbox, a.game)
    maps = a.maps or playable_maps(sandbox, a.game)
    print(f"vkparity: {len(maps)} maps", flush=True)
    results = []
    for i, m in enumerate(maps, 1):
        vk_log = os.path.join(out, "runs", f"{m}_vk.txt")
        ours_log = os.path.join(out, "runs", f"{m}_ours.txt")
        hung = run_vk(a, work, a.game, m, vk_log)
        if hung:
            print(f"vkparity: vkQuake on {m} didn't quit within {a.timeout}s (pid {hung}); stopping, NOT killing it",
                  flush=True)
            break
        run_ours(a, work, a.game, m, ours_log)
        vk = parse(open(vk_log, encoding="latin-1").read())
        ours = parse(open(ours_log, encoding="latin-1").read())
        diff = compare(vk, ours)
        sig = sum(1 for d in diff.values() if abs(d[0] - d[1]) > 1 or 0 in d)
        results.append({"map": m, "vk": {**vk, "classes": dict(vk["classes"])},
                        "ours": {**ours, "classes": dict(ours["classes"])}, "diff": diff})
        print(f"[{i}/{len(maps)}] {m}: edicts {vk['num_edicts']}/{ours['num_edicts']}, "
              f"{sig} classes differ, engine {vk['engine']}/{ours['engine']}", flush=True)
        write_report(out, a.game, a.frames, results)
    write_report(out, a.game, a.frames, results)
    print(f"vkparity: wrote {out}/report.md")


if __name__ == "__main__":
    main()
