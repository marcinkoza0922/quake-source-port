#!/usr/bin/env python3
"""Read-only Quake game data lookup (PAK / PK3 / loose files, BSP entity lumps).

Mirrors the engine's search order (crates/qcore/src/fs.rs): later game dirs
win; inside a dir, pk3s (reverse-sorted) beat pakN..pak0, which beat loose
files; names match case-insensitively. A `maps/<map>.ent` found by the same
search replaces the BSP's entity lump, as in the engine (only from the same
or a higher-priority game dir than the map).

  qdata.py mods
  qdata.py maps   [-g GAME]
  qdata.py which  PATH [-g GAME]
  qdata.py ents   MAP  [-g GAME] [-c CLASS] [-k KEY=SUBSTR | -k KEY==EXACT] [--all] [--json]

GAME may be `ad`, `hipnotic`, ...; prefix base packs with `+`, e.g.
`-g +hipnotic,mymod` mounts id1, hipnotic, mymod (like `-hipnotic -game mymod`).
Base dir: -b, else $QUAKE_BASEDIR, else `./vkQuake directory with maps`.
"""
import argparse
import json
import os
import struct
import sys
import zipfile


def base_dir(arg):
    d = arg or os.environ.get("QUAKE_BASEDIR") or "vkQuake directory with maps"
    if not os.path.isdir(os.path.join(d, "id1")):
        sys.exit(f"qdata: no id1 in {d!r} (set QUAKE_BASEDIR or -b)")
    return d


def ci_child(d, name):
    """Case-insensitive lookup of one path component."""
    try:
        for e in os.listdir(d):
            if e.lower() == name.lower():
                return os.path.join(d, e)
    except OSError:
        pass
    return None


def ci_path(d, rel):
    for part in rel.split("/"):
        d = ci_child(d, part)
        if d is None:
            return None
    return d if os.path.isfile(d) else None


class Pak:
    def __init__(self, path):
        self.path = path
        with open(path, "rb") as f:
            hdr = f.read(12)
            if hdr[:4] != b"PACK":
                raise ValueError("not a PAK")
            ofs, ln = struct.unpack("<ii", hdr[4:])
            f.seek(ofs)
            raw = f.read(ln)
        self.files = {}
        for i in range(ln // 64):
            e = raw[i * 64:(i + 1) * 64]
            name = e[:56].split(b"\0", 1)[0].decode("latin-1").lower()
            self.files[name] = struct.unpack("<ii", e[56:64])

    def names(self):
        return self.files.keys()

    def read(self, name):
        pos, ln = self.files[name.lower()]
        with open(self.path, "rb") as f:
            f.seek(pos)
            return f.read(ln)


class Pk3:
    def __init__(self, path):
        self.path = path
        self.z = zipfile.ZipFile(path)
        self.files = {i.filename.lower(): i.filename for i in self.z.infolist() if not i.is_dir()}

    def names(self):
        return self.files.keys()

    def read(self, name):
        return self.z.read(self.files[name.lower()])


def search_path(base, game):
    """[(gamedir, source)] highest priority first; source is a pack or a dir."""
    dirs = ["id1"]
    for g in (game or "id1").split(","):
        g = g.lstrip("+")
        if g and g.lower() != "id1":
            dirs.append(g)
    out = []
    for g in reversed(dirs):
        d = ci_child(base, g)
        if d is None:
            print(f"qdata: no game dir {g!r} in {base}", file=sys.stderr)
            continue
        packs = []
        i = 0
        while (p := ci_child(d, f"pak{i}.pak")) is not None:
            try:
                packs.append(Pak(p))
            except (OSError, ValueError) as e:
                print(f"qdata: {p}: {e}", file=sys.stderr)
            i += 1
        pk3s = sorted(e for e in os.listdir(d) if e.lower().endswith(".pk3"))
        for name in reversed(pk3s):
            try:
                out.append((g, Pk3(os.path.join(d, name))))
            except (OSError, zipfile.BadZipFile) as e:
                print(f"qdata: {name}: {e}", file=sys.stderr)
        out += [(g, p) for p in reversed(packs)]
        out.append((g, d))
    return out


def find(sp, rel):
    """(rank, gamedir, where, bytes) of the first match, or None."""
    rel = rel.replace("\\", "/").lower()
    for rank, (g, src) in enumerate(sp):
        if isinstance(src, str):
            p = ci_path(src, rel)
            if p:
                with open(p, "rb") as f:
                    return rank, g, p, f.read()
        elif rel in src.files:
            return rank, g, f"{src.path}:{rel}", src.read(rel)
    return None


def entity_text(bsp):
    ident = bsp[:4]
    # BSP29, BSP2, 2PSB and Quake64 (" 46Q") all keep the entity lump first.
    if ident not in (b"BSP2", b"2PSB", b" 46Q") and struct.unpack("<i", ident)[0] != 29:
        print(f"qdata: unusual BSP ident {ident!r}; reading lump 0 anyway", file=sys.stderr)
    ofs, ln = struct.unpack("<ii", bsp[4:12])
    return bsp[ofs:ofs + ln].split(b"\0", 1)[0].decode("latin-1")


def parse_entities(text):
    ents, cur, toks, i, n = [], None, [], 0, len(text)
    while i < n:
        c = text[i]
        if c.isspace():
            i += 1
        elif text.startswith("//", i):
            j = text.find("\n", i)
            i = n if j < 0 else j
        elif c in "{}":
            toks.append(c)
            i += 1
        elif c == '"':
            j = text.find('"', i + 1)
            j = n if j < 0 else j
            toks.append(("s", text[i + 1:j]))
            i = j + 1
        else:
            j = i
            while j < n and not text[j].isspace() and text[j] not in '{}"':
                j += 1
            toks.append(("s", text[i:j]))
            i = j
    key = None
    for t in toks:
        if t == "{":
            cur, key = [], None
        elif t == "}":
            if cur is not None:
                ents.append(cur)
            cur = None
        elif cur is not None:
            if key is None:
                key = t[1]
            else:
                cur.append((key, t[1]))
                key = None
    return ents


def cmd_mods(a):
    base = base_dir(a.basedir)
    for e in sorted(os.listdir(base), key=str.lower):
        d = os.path.join(base, e)
        if not os.path.isdir(d):
            continue
        ls = [x.lower() for x in os.listdir(d)]
        kinds = [k for k, ok in (
            ("pak", any(x.startswith("pak") and x.endswith(".pak") for x in ls)),
            ("pk3", any(x.endswith(".pk3") for x in ls)),
            ("progs", "progs.dat" in ls or "progs" in ls),
            ("maps", "maps" in ls),
        ) if ok]
        if kinds:
            print(f"{e:32} {' '.join(kinds)}")


def cmd_maps(a):
    base = base_dir(a.basedir)
    sp = search_path(base, a.game)
    own = (a.game or "id1").split(",")[-1].lstrip("+").lower()
    seen = {}
    for g, src in sp:
        if isinstance(src, str):
            md = ci_child(src, "maps")
            names = [f"maps/{x.lower()}" for x in os.listdir(md)] if md else []
            where = src
        else:
            names, where = src.names(), src.path
        for n in names:
            if n.startswith("maps/") and n.endswith(".bsp") and n.count("/") == 1 and n not in seen:
                seen[n] = (g, where)
    for n, (g, where) in sorted(seen.items()):
        if a.all or g.lower() == own:
            print(f"{n[5:-4]:28} {g:12} {where}")


def cmd_which(a):
    base = base_dir(a.basedir)
    r = find(search_path(base, a.game), a.path)
    if not r:
        sys.exit(f"qdata: {a.path} not found")
    _, g, where, data = r
    print(f"{a.path}: {where} ({g}, {len(data)} bytes)")


def cmd_ents(a):
    base = base_dir(a.basedir)
    sp = search_path(base, a.game)
    m = a.map[:-4] if a.map.lower().endswith(".bsp") else a.map
    hit = find(sp, f"maps/{m}.bsp")
    if not hit:
        sys.exit(f"qdata: maps/{m}.bsp not found in {a.game or 'id1'}")
    _, g, where, bsp = hit
    text, src = entity_text(bsp), where
    ent = find(sp, f"maps/{m}.ent")
    # Game dir priority (0 = highest): an .ent counts from the map's own dir or above.
    prio = {}
    for gd, _ in sp:
        prio.setdefault(gd, len(prio))
    if ent and prio[ent[1]] <= prio[g]:
        text, src = ent[3].decode("latin-1"), ent[2]
    ents = parse_entities(text)
    print(f"# {len(ents)} entities from {src}", file=sys.stderr)
    filt = []  # (key, value, exact)
    for f in a.key or []:
        k, _, v = f.partition("=")
        exact = v.startswith("=")
        filt.append((k.lower(), v[1:].lower() if exact else v.lower(), exact))

    def match(kv, k, v, exact):
        return any(k2.lower() == k and (v2.lower() == v if exact else v in v2.lower()) for k2, v2 in kv)
    rows = []
    for i, kv in enumerate(ents):
        d = {}
        for k, v in kv:
            d.setdefault(k, v)
        cls = d.get("classname", "")
        if a.classname and a.classname.lower() not in cls.lower():
            continue
        if not all(match(kv, k, v, exact) for k, v, exact in filt):
            continue
        rows.append((i, cls, kv))
    if a.json:
        print(json.dumps([{"index": i, "pairs": kv} for i, _, kv in rows], indent=1))
        return
    brief = ("origin", "targetname", "target", "killtarget", "model", "spawnflags", "angle", "mangle", "message")
    for i, cls, kv in rows:
        shown = [(k, v) for k, v in kv if a.all or k in brief]
        if not a.all:
            shown.sort(key=lambda p: brief.index(p[0]))
        print(f"{i:4} {cls:26} " + "  ".join(f'{k}="{v}"' for k, v in shown if k != "classname"))


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("-b", "--basedir")
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("mods", help="list game dirs")
    s = sub.add_parser("maps", help="list maps a game supplies")
    s.add_argument("-g", "--game")
    s.add_argument("--all", action="store_true", help="include maps from base dirs")
    s = sub.add_parser("which", help="which archive supplies a path")
    s.add_argument("path")
    s.add_argument("-g", "--game")
    s = sub.add_parser("ents", help="dump a map's entities")
    s.add_argument("map")
    s.add_argument("-g", "--game")
    s.add_argument("-c", "--classname", help="substring of classname")
    s.add_argument("-k", "--key", action="append", help="KEY=SUBSTR or KEY==EXACT filter (repeatable)")
    s.add_argument("--all", action="store_true", help="print every key, not just the usual ones")
    s.add_argument("--json", action="store_true")
    a = p.parse_args()
    {"mods": cmd_mods, "maps": cmd_maps, "which": cmd_which, "ents": cmd_ents}[a.cmd](a)


if __name__ == "__main__":
    main()
