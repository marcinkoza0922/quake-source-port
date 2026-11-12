#!/usr/bin/env python3
"""Read-only audit of local mod inventory fields, constants, and HUD art (no game data copied)."""
import argparse, json, re, struct, zipfile, sys
from pathlib import Path

def files(root):
    out = {}
    for pak in sorted(root.glob('*.pak')):
        data = pak.read_bytes()
        if data[:4] != b'PACK': continue
        off, size = struct.unpack_from('<ii', data, 4)
        if off < 12 or off + size > len(data):
            print(f'WARNING: truncated PAK directory: {pak}', file=sys.stderr)
            continue
        for pos in range(off, off + size, 64):
            name = data[pos:pos+56].split(b'\0')[0].decode('latin1')
            start, length = struct.unpack_from('<ii', data, pos+56)
            if name in ('progs.dat','gfx.wad') or name.endswith('.qc') or name.startswith('gfx/'):
                out[name] = data[start:start+length]
    for pak in sorted(root.glob('*.pk3')):
        with zipfile.ZipFile(pak) as z:
            for name in z.namelist():
                if name in ('progs.dat','gfx.wad') or name.endswith('.qc') or name.startswith('gfx/'):
                    out[name] = z.read(name)
    for p in root.rglob('*'):
        if p.is_file() and (p.name in ('progs.dat', 'gfx.wad') or p.suffix == '.qc' or p.relative_to(root).parts[0] == 'gfx'):
            out[p.relative_to(root).as_posix()] = p.read_bytes()
    return out

def audit(root):
    data = files(root)
    progs = data.get('progs.dat', b'')
    fields, constants = [], {}
    if len(progs) >= 60:
        h = struct.unpack_from('<15i', progs)
        strings = progs[h[10]:h[10]+h[11]]
        def name(n): return strings[n:].split(b'\0')[0].decode('latin1')
        for p in range(h[6], h[6]+h[7]*8, 8):
            ty, ofs, n = struct.unpack_from('<HHi', progs, p)
            key = name(n)
            if re.search(r'item|key|weapon|upgrade|worldtype|perms', key, re.I): fields.append(key)
        for p in range(h[4], h[4]+h[5]*8, 8):
            ty, ofs, n = struct.unpack_from('<HHi', progs, p)
            key = name(n)
            if key.startswith(('IT_', 'IT2_', 'HIT_', 'RIT_')) and ty & 0x7fff == 2:
                constants[key] = struct.unpack_from('<f', progs, h[12]+ofs*4)[0]
    art = list(data)
    wad = data.get('gfx.wad', b'')
    if wad[:4] in (b'WAD2', b'WAD3'):
        count, off = struct.unpack_from('<ii', wad, 4)
        art += [wad[p+16:p+32].split(b'\0')[0].decode('latin1') for p in range(off,off+count*32,32)]
    icons = sorted(n for n in art if re.search(r'(^inv_|^r_|^sb_.*key|^gfx/.*(key|adw_|axe|wrench|inv_|weapons/))', n, re.I))
    sources = {n: [line.strip() for line in text.decode('latin1').splitlines() if re.search(r'IT_CKEY|IT_UPGRADE|weaponnames|weaponlabel|keytype|ckeyskin|keycount',line)][:24] for n,text in data.items() if n.endswith('.qc')}
    return dict(mod=root.name, has_progs=bool(progs), fields=fields, constants=constants, icons=icons, source_hints={k:v for k,v in sources.items() if v})

if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('basedir',type=Path)
    parser.add_argument('mods',nargs='*')
    args=parser.parse_args()
    roots=[args.basedir/n for n in args.mods] if args.mods else sorted(p for p in args.basedir.iterdir() if p.is_dir())
    print(json.dumps([audit(p) for p in roots], indent=2))
