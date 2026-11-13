#!/usr/bin/env python3
"""Inventory installed maps using qinspect, then optionally capture portal views.
Game data is read in place; reports and screenshots go under target/portal-survey.
"""
import argparse
import json
import html
import math
from pathlib import Path
import re
import struct
import subprocess
import tempfile
import zipfile


ARCHIVE_ERRORS = []


def maps_in(directory):
    maps = {}
    for p in directory.rglob('*'):
        if not p.is_file() or p.suffix.lower() != '.bsp':
            continue
        rel = p.relative_to(directory).as_posix()
        if rel.lower().startswith('maps/'):
            maps[rel.lower()] = ('loose', str(p), None)
    files = {p.name.lower(): p for p in directory.iterdir()}
    i = 0
    while (pak := files.get(f'pak{i}.pak')) is not None:
        with pak.open('rb') as f:
            magic, offset, size = struct.unpack('<4sii', f.read(12))
            if magic != b'PACK':
                break
            if offset < 0 or size < 0 or offset + size > pak.stat().st_size:
                message = f'truncated archive {pak}: directory extends beyond file size'
                ARCHIVE_ERRORS.append(dict(game=directory.name, archive=str(pak), error=message))
                print('WARNING: ' + message, flush=True)
                i += 1
                continue
            f.seek(offset)
            for _ in range(size // 64):
                name, start, length = struct.unpack('<56sii', f.read(64))
                name = name.split(b'\0')[0].decode('latin1').lower()
                if name.startswith('maps/') and name.endswith('.bsp'):
                    maps[name] = ('pak', str(pak), (start, length))
        i += 1
    for archive in sorted(p for p in directory.iterdir() if p.suffix.lower() == '.pk3'):
        with zipfile.ZipFile(archive) as z:
            for name in z.namelist():
                if name.lower().startswith('maps/') and name.lower().endswith('.bsp'):
                    maps[name.lower()] = ('zip', str(archive), name)
    return maps


def read_source(source):
    kind, path, entry = source
    if kind == 'loose':
        return Path(path).read_bytes()
    if kind == 'zip':
        with zipfile.ZipFile(path) as z:
            return z.read(entry)
    with open(path, 'rb') as f:
        f.seek(entry[0])
        return f.read(entry[1])


def capture(args, row):
    game, mapname = row['game'], row['map']
    folder = (args.output / game / mapname).resolve()
    folder.mkdir(parents=True, exist_ok=True)
    commands = ['delay 0.6 god', 'delay 0.7 noclip']
    for number, portal in enumerate(row['portals']):
        center, normal = portal['center'], portal['normal']
        position = [center[k] + normal[k] * max(10, portal.get('camera_distance', 110)) for k in range(3)]
        position[2] -= 22
        yaw = math.degrees(math.atan2(-normal[1], -normal[0]))
        at = 2.0 + number * args.capture_interval
        shot = folder / f"portal-{portal['index']:03}.png"
        portal['screenshot'] = str(shot)
        portal['camera_obstructed'] = portal.get('camera_distance', 110) < 0
        commands += [f'delay {at:.2f} setpos {position[0]} {position[1]} {position[2]} 0 {yaw} 0',
                     f'delay {at + 0.4:.2f} screenshot {shot}']
    command = [args.engine, '-basedir', str(args.basedir), '-headless', '-width', '800', '-height', '600',
               '-autoquit', str(4 + len(row['portals']) * args.capture_interval), '-game', game,
               '+r_teleportwindows', '1', '+map', mapname]
    for item in commands:
        command.append('+' + item)
    try:
        result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=max(180, 60 + len(row['portals']) * args.capture_interval * 10))
        (folder / 'capture.log').write_text(result.stdout)
        row['capture_exit'] = result.returncode
    except subprocess.TimeoutExpired:
        row['capture_error'] = 'capture process exceeded timeout'
    for portal in row['portals']:
        portal['captured'] = Path(portal['screenshot']).is_file()


def write_gallery(output, report):
    content = ['<!doctype html><meta charset="utf-8"><title>Portal survey</title>',
               '<style>body{background:#171717;color:#eee;font:16px sans-serif}img{width:400px;max-width:100%}article{display:inline-block;vertical-align:top;padding:12px}summary{padding:8px}</style>',
               '<h1>Portal survey</h1><p>Exact renderer matches. Custom teleport entity classes and inspection errors are recorded in inventory.json. Camera views are automatic and can be obstructed by nearby geometry.</p>']
    for row in report:
        label = html.escape(row['game'] + '/' + row['map'])
        portals = row.get('portals', [])
        content.append(f'<details{ " open" if portals else ""}><summary>{label}: {len(portals)} surfaces</summary>')
        custom = {key: value for key, value in row.get('teleport_entity_classes', {}).items() if key not in ('trigger_teleport', 'info_teleport_destination')}
        if custom:
            content.append('<p>Other teleport entities (not renderer matches): ' + html.escape(str(custom)) + '</p>')
        if row.get('error'):
            content.append('<pre>' + html.escape(row['error']) + '</pre>')
        for portal in portals:
            shot = Path(portal.get('screenshot', ''))
            if portal.get('captured'):
                relative = shot.relative_to(output.resolve()).as_posix()
                center = ', '.join(f'{v:.1f}' for v in portal['center'])
                status = ' — camera obstructed in BSP' if portal.get('camera_obstructed') else ''
                obstruction = '; camera ray intersects solid geometry' if portal.get('camera_distance', 110) < 0 else ''
                content.append(f'<article><a href="{relative}"><img loading="lazy" src="{relative}"></a><p>Surface {portal["index"]}; center {center}{status}</p></article>')
        content.append('</details>')
    (output / 'index.html').write_text('\n'.join(content))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--basedir', type=Path, required=True)
    p.add_argument('--game', action='append')
    p.add_argument('--exclude-game', action='append', default=[])
    p.add_argument('--output', type=Path, default=Path('target/portal-survey/mods'))
    p.add_argument('--capture', action='store_true')
    p.add_argument('--capture-interval', type=float, default=0.7)
    p.add_argument('--engine', default='target/release/quake-source-port')
    p.add_argument('--inspect', default='target/release/qinspect')
    args = p.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    games = args.game or [d.name for d in sorted(args.basedir.iterdir()) if d.is_dir() and d.name != 'id1']
    report = []
    previous_path = args.output / 'inventory.json'
    previous = {(r['game'], r['map']): r for r in json.loads(previous_path.read_text())} if previous_path.exists() else {}
    with tempfile.TemporaryDirectory(prefix='portal-survey-') as temp:
        bsp = Path(temp) / 'map.bsp'
        for game in games:
            if game in args.exclude_game:
                continue
            for name, source in sorted(maps_in(args.basedir / game).items()):
                old = previous.get((game, name[5:-4]))
                if old and (not args.capture or all(p.get('captured') for p in old.get('portals', []))):
                    report.append(old)
                    continue
                row = dict(game=game, map=name[5:-4], source=source)
                try:
                    bsp.write_bytes(read_source(source))
                    proc = subprocess.run([args.inspect, str(bsp)], capture_output=True, text=True)
                    row['portals'] = []
                    for line in proc.stdout.splitlines():
                        values = list(map(float, line.split()))
                        row['portals'].append(dict(index=int(values[0]), center=values[1:4], normal=values[4:7], yaw=values[7], camera_distance=values[8] if len(values) > 8 else 110))
                    data = bsp.read_bytes()
                    offset, length = struct.unpack_from('<ii', data, 4)
                    entities = data[offset:offset + length].decode('latin1')
                    classes = re.findall(r'"classname"\s*"([^"]*tele[^"]*)"', entities)
                    row['teleport_entity_classes'] = {c: classes.count(c) for c in sorted(set(classes))}
                    if args.capture and row['portals']:
                        capture(args, row)
                    if proc.returncode:
                        row['error'] = proc.stderr[-2000:]
                except Exception as error:
                    row['error'] = str(error)
                report.append(row)
            (args.output / 'archive-errors.json').write_text(json.dumps(ARCHIVE_ERRORS, indent=2))
            (args.output / 'inventory.json').write_text(json.dumps(report, indent=2))
            write_gallery(args.output, report)
            print(game, len([r for r in report if r['game'] == game]), flush=True)


if __name__ == '__main__':
    main()
