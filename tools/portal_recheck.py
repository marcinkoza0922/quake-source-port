#!/usr/bin/env python3
"""Recapture every screenshot linked by problematic.md with source scenery hidden.
Preserves the original survey and emits paired gallery, commands and per-map logs.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import html
import json
import math
import os
from pathlib import Path
import re
import subprocess


def capture(job, args):
    game, mapname, portals = job
    folder = (args.output / game / mapname).resolve()
    folder.mkdir(parents=True, exist_ok=True)
    command = [str(args.engine.resolve()), '-basedir', str(args.basedir.resolve()),
               '-headless', '-width', '800', '-height', '600', '-game', game,
               '-autoquit', str(4 + len(portals) * 1.5),
               '+sv_autosave', '0', '+r_teleportwindows', '1', '+r_drawviewmodel', '0',
               '+viewsize', '120', '+con_notifytime', '0', '+crosshair', '0',
               '+r_motionblur', '0', '+scr_centertime', '0', '+v_flashscale', '0',
               '+hud_health', '0', '+hud_armor', '0', '+hud_ammo', '0',
               '+hud_ammolist', '0', '+hud_weapons', '0', '+hud_items', '0',
               '+hud_face', '0', '+map', mapname,
               '+delay 0.5 god', '+delay 0.6 noclip']
    results = []
    for n, (portal, original) in enumerate(portals):
        index = portal['index']
        # A fixed stand-off makes even inaccessible back faces readable after
        # isolation. Pitch is zero because renderer matches vertical sheets.
        position = [portal['center'][k] + portal['normal'][k] * 110 for k in range(3)]
        position[2] -= 22
        yaw = math.degrees(math.atan2(-portal['normal'][1], -portal['normal'][0]))
        at = 1.5 + n * 1.5
        baseline = folder / f'portal-{index:03}-baseline.png'
        shot = folder / f'portal-{index:03}-isolated.png'
        command += [f'+delay {at:.2f} r_visibility reset',
                    f'+delay {at:.2f} setpos {position[0]} {position[1]} {position[2]} 0 {yaw} 0',
                    f'+delay {at + .35:.2f} screenshot {baseline}',
                    f'+delay {at + .55:.2f} r_visibility isolate {index} 110',
                    f'+delay {at + 1:.2f} screenshot {shot}']
        for path in (baseline, shot):
            path.unlink(missing_ok=True)
        results.append(dict(game=game, map=mapname, index=index, original=original,
                            baseline=str(baseline), isolated=str(shot), camera_origin=position,
                            center=portal['center'], normal=portal['normal']))
    (folder / 'command.json').write_text(json.dumps(command, indent=2))
    try:
        proc = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                              text=True, timeout=120 + len(portals) * 3)
        (folder / 'capture.log').write_text(proc.stdout)
        for r in results:
            r['exit_code'] = proc.returncode
            r['isolation_confirmed'] = f"Isolated portal {r['index']}:" in proc.stdout
    except subprocess.TimeoutExpired as error:
        for r in results:
            r['error'] = str(error)
    for r in results:
        r['captured'] = Path(r['baseline']).is_file() and Path(r['isolated']).is_file()
        r['verified'] = r['captured'] and r.get('exit_code') == 0 and r.get('isolation_confirmed', False)
    print(f'{game}/{mapname}: {sum(r["captured"] for r in results)}/{len(results)}', flush=True)
    return results


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--survey', type=Path, default=Path('target/portal-survey'))
    p.add_argument('--output', type=Path, default=Path('target/portal-survey/recheck'))
    p.add_argument('--basedir', type=Path, default=Path(os.environ.get('QUAKE_BASEDIR', 'vkQuake directory with maps')))
    p.add_argument('--engine', type=Path, default=Path('target/release/quake-source-port'))
    p.add_argument('--workers', type=int, default=3)
    args = p.parse_args()
    links = set(re.findall(r'\]\(([^)]+/portal-\d+\.png)\)', (args.survey / 'problematic.md').read_text()))
    lookup = {}
    for inventory in args.survey.rglob('inventory.json'):
        if args.output.resolve() in inventory.resolve().parents:
            continue
        for row in json.loads(inventory.read_text()):
            for portal in row.get('portals', []):
                if portal.get('screenshot') in links:
                    lookup[portal['screenshot']] = (row['game'], row['map'], portal)
    missing = links - lookup.keys()
    if missing:
        raise SystemExit(f'Missing inventory entries: {sorted(missing)}')
    jobs = {}
    for original in sorted(links):
        game, mapname, portal = lookup[original]
        jobs.setdefault((game, mapname), []).append((portal, original))
    args.output.mkdir(parents=True, exist_ok=True)
    results = []
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        for rows in pool.map(lambda job: capture(job, args),
                             [(g, m, sorted(ps, key=lambda p: p[0]['index'])) for (g, m), ps in jobs.items()]):
            results.extend(rows)
            (args.output / 'inventory.json').write_text(json.dumps(results, indent=2))
    content = ['<!doctype html><meta charset="utf-8"><title>Portal obstruction recheck</title>',
               '<style>body{background:#181818;color:#eee;font:16px sans-serif}img{width:48%;max-width:800px}article{padding:12px;border-bottom:1px solid #555}a{color:#acd}</style>',
               '<h1>Portal obstruction recheck</h1><p>Left: requested-camera baseline. Right: isolated source sheet. Destination world and available brush/alias entities retained. Source geometry, entities and particles hidden. Fixed 110-unit diagnostic camera overrides mod player relocation; no gameplay geometry changed. Back sides may be inaccessible during normal play.</p>']
    for r in results:
        label = html.escape(f"{r['game']}/{r['map']} / {r['index']}")
        content.append(f'<article><h2>{label}</h2>')
        for key in ('baseline', 'isolated'):
            rel = Path(r[key]).relative_to(args.output.resolve()).as_posix()
            content.append(f'<a href="{rel}"><img loading="lazy" src="{rel}" alt="{key}"></a>')
        if not r.get('verified'):
            content.append('<p>Capture or isolation failed; inspect capture.log.</p>')
        content.append('</article>')
    (args.output / 'index.html').write_text('\n'.join(content))
    print(f'Total: {len(results)} views; {sum(r["captured"] for r in results)} captured', flush=True)
    if not all(r.get('verified') for r in results):
        raise SystemExit('Some views failed capture/isolation; inspect inventory.json and map logs.')


if __name__ == '__main__':
    main()
