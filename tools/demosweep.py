#!/usr/bin/env python3
"""Play demos in our engine, ironwail and vkQuake and compare the results.

Each demo is `timedemo`ed once per engine: ours headless, the references in a
headless gamescope (no window on the desktop). Per run it records the frame
count from the "N frames S seconds F fps" line and console lines that point at
a protocol or content problem. Demos whose frame counts disagree by more than
`--slack`, or that print errors in one engine only, are listed first.

Demos: every `.dem` the game data ships (from paks or loose files), plus any
`--extra GAME:PATH` files (copied into the sandbox game dir, never into the
game data). Engines run in the spawnparity sandbox (`target/compat/sandbox`)
with XDG_DATA_HOME there, so no user config or save is touched.

  demosweep.py [-b BASEDIR] [--engines ours,ironwail,vkquake] [-j N]
               [--extra GAME:PATH ...] [--out DIR] [game[:demo] ...]

Writes OUT/report.md and OUT/report.json (default target/compat/demos).
"""
import argparse
import concurrent.futures as cf
import json
import os
import re
import shutil
import subprocess
import sys
import time

sys.dont_write_bytecode = True  # a tools/__pycache__ dir would break the cargo workspace glob
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import compat_audit  # noqa: E402
import spawnparity  # noqa: E402
from qdata import base_dir  # noqa: E402

FRAMES = re.compile(r"(\d+) frames\s+([\d.]+) seconds")
PROBLEM = re.compile(r"host_error|illegible|bad server message|cl_parseservermessage|svc_|sys_error|panicked|"
                     r"couldn't load|not found|unknown command|demo.*(error|bad)|protocol|segmentation|"
                     r"overflow|exceeds|invalid", re.I)
NOISE = re.compile(r"autoexec\.cfg|config\.cfg|GL api|gamescope|xkbcomp|vkGetPhysicalDevice|external vis|"
                   r"\.lit not found|exceeds standard limit|Buffer detailed info", re.I)


def shipped_demos(base, game):
    out = []
    d = os.path.join(base, game)
    for rel, _src, _read in compat_audit.own_files(d):
        if rel.endswith(".dem"):
            out.append(rel[:-4])
    return sorted(set(out))


def run_engine(engine, a, sandbox, game, demo, slot):
    ga = spawnparity.game_args(game)
    env = dict(os.environ, XDG_DATA_HOME=a.userdata)
    if engine == "ours":
        cmd = [a.ours, "-basedir", sandbox, "-headless", "-nosound", "-autoquit", str(a.timeout - 5)] + ga + \
              ["+sv_autosave", "0", "+timedemo", demo]
    else:
        binary = a.ironwail if engine == "ironwail" else a.vkquake
        env["HOME"] = a.refhome  # vkQuake keeps nothing there, but be safe
        # stdbuf -oL: through a pipe the references block-buffer stdout, so
        # the timedemo result line would only show up at exit
        cmd = ["gamescope", "--backend", "headless", "-W", "640", "-H", "480", "--", "stdbuf", "-oL", binary, "-basedir", ".",
               "-window", "-width", "640", "-height", "480", "-nosound", "-port", str(27800 + slot % 100)] + ga + \
              ["+vid_vsync", "0", "+host_maxfps", "0", "+timedemo", demo]
    log = os.path.join(a.out, "runs", f"{game}_{demo.replace('/', '_')}_{engine}.txt")
    os.makedirs(os.path.dirname(log), exist_ok=True)
    start = time.time()
    with open(log, "wb") as f:
        p = subprocess.Popen(cmd, cwd=sandbox, env=env, stdin=subprocess.DEVNULL, stdout=f, stderr=subprocess.STDOUT,
                             start_new_session=True)
        frames = None
        hung = False
        while time.time() - start < a.timeout:
            time.sleep(0.5)
            text = open(log, "rb").read().decode("latin-1")
            m = FRAMES.search(text)
            if m:
                frames = (int(m.group(1)), float(m.group(2)))
                break
            if p.poll() is not None:
                break
        else:
            hung = True
        if hung and engine != "ours":
            # Killing a reference that hung inside gamescope left the NVIDIA
            # driver needing a GPU reset (Xid 51, 2026-11-17): leave it running
            # and stop the sweep instead.
            raise SystemExit(f"demosweep: {engine} hung on {game}/{demo} (pid {p.pid}); not killing it, stopping")
        # SIGKILL the whole group: the references would write a config on a normal quit
        try:
            os.killpg(p.pid, 9)
        except ProcessLookupError:
            pass
        p.wait()
    text = open(log, "rb").read().decode("latin-1")
    problems = sorted(set(l.strip()[:160] for l in text.splitlines() if PROBLEM.search(l) and not NOISE.search(l)))
    return {"frames": frames[0] if frames else None, "seconds": frames[1] if frames else None,
            "problems": problems[:12], "wall": round(time.time() - start, 1), "log": log}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("-b", "--basedir")
    ap.add_argument("--engines", default="ours,ironwail,vkquake")
    ap.add_argument("--ours", default="target/release/quake-source-port")
    ap.add_argument("--ironwail", default="ironwail/Quake/ironwail")
    ap.add_argument("--vkquake")
    ap.add_argument("-j", type=int, default=3)
    ap.add_argument("--slack", type=int, default=2)
    ap.add_argument("--timeout", type=int, default=240, help="per run; a reference that hangs stops the sweep")
    ap.add_argument("--extra", action="append", default=[])
    ap.add_argument("--out", default="target/compat/demos")
    ap.add_argument("filters", nargs="*")
    a = ap.parse_args()
    base = base_dir(a.basedir)
    a.ours = os.path.abspath(a.ours)
    a.ironwail = os.path.abspath(a.ironwail)
    a.vkquake = os.path.abspath(a.vkquake or os.path.join(base, "vkquake.AppImage"))
    a.out = os.path.abspath(a.out)
    a.userdata = os.path.abspath("target/compat/userdata")
    a.refhome = os.path.abspath("target/compat/refhome")
    os.makedirs(a.refhome, exist_ok=True)
    sandbox = spawnparity.build_sandbox(base, os.path.abspath("target/compat/sandbox"))
    work = []
    for g in sorted(os.listdir(base)):
        if not os.path.isdir(os.path.join(sandbox, g)):
            continue
        for d in shipped_demos(base, g):
            work.append((g, d))
    for e in a.extra:
        g, path = e.split(":", 1)
        dst = os.path.join(sandbox, g, os.path.basename(path))
        shutil.copyfile(path, dst)
        work.append((g, os.path.basename(path)[:-4]))
    if a.filters:
        work = [(g, d) for g, d in work if any(f == g or f == f"{g}:{d}" for f in a.filters)]
    engines = a.engines.split(",")
    print(f"demosweep: {len(work)} demos x {len(engines)} engines", file=sys.stderr)
    results = []
    # Ours runs headless in parallel; the references one at a time (several
    # headless gamescope sessions at once hung vkQuake on 2026-11-17).
    jobs = [(g, d, e, i) for i, (g, d) in enumerate(work) for e in engines if e == "ours"]
    refs = [(g, d, e, i) for i, (g, d) in enumerate(work) for e in engines if e != "ours"]
    per = {}
    for g, d, e, i in refs:
        r = run_engine(e, a, sandbox, g, d, i * 4 + engines.index(e))
        per.setdefault((g, d), {})[e] = r
        print(f"{g}/{d} {e}: {r.get('frames')} frames", file=sys.stderr)
    with cf.ThreadPoolExecutor(a.j) as ex:
        futs = {ex.submit(run_engine, e, a, sandbox, g, d, i * 4 + engines.index(e)): (g, d, e) for g, d, e, i in jobs}
        for f in cf.as_completed(futs):
            g, d, e = futs[f]
            try:
                r = f.result()
            except Exception as ex_:  # noqa: BLE001 - keep the sweep going
                r = {"frames": None, "problems": [f"tool error: {ex_}"]}
            per.setdefault((g, d), {})[e] = r
            print(f"{g}/{d} {e}: {r.get('frames')} frames", file=sys.stderr)
    for (g, d), rs in sorted(per.items()):
        fr = [rs[e]["frames"] for e in engines if e in rs and rs[e]["frames"] is not None]
        ok = len(fr) == len(engines) and max(fr) - min(fr) <= a.slack
        only = {e: rs[e]["problems"] for e in engines if e in rs and rs[e]["problems"]}
        results.append({"game": g, "demo": d, "ok": ok, "engines": rs, "problem_engines": sorted(only)})
    os.makedirs(a.out, exist_ok=True)
    with open(os.path.join(a.out, "report.json"), "w") as f:
        json.dump(results, f, indent=1)
    lines = ["# Demo sweep", "", f"{len(results)} demos; engines {', '.join(engines)}; frame slack {a.slack}", "",
             "| game | demo | " + " | ".join(engines) + " | verdict |", "|---|---|" + "---|" * len(engines) + "---|"]
    for r in sorted(results, key=lambda r: (r["ok"], r["game"], r["demo"])):
        cells = [str(r["engines"].get(e, {}).get("frames")) for e in engines]
        lines.append(f"| {r['game']} | {r['demo']} | " + " | ".join(cells) + f" | {'ok' if r['ok'] else 'DIFF'} |")
    lines.append("")
    lines.append("## Console problems")
    for r in results:
        for e in engines:
            pr = r["engines"].get(e, {}).get("problems")
            if pr:
                lines.append(f"- {r['game']}/{r['demo']} [{e}]: " + " | ".join(pr[:6]))
    with open(os.path.join(a.out, "report.md"), "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"demosweep: wrote {a.out}/report.md", file=sys.stderr)


if __name__ == "__main__":
    main()
