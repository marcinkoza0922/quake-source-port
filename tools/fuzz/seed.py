#!/usr/bin/env python3
"""Seed corpora for tools/fuzz from the installed game data (target/fuzz/corpus,
never committed) and a sandboxed basedir (target/fuzz/base) for the targets
that run a server. Usage: seed.py BASEDIR"""
import os, struct, sys, zipfile, io
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / "target/fuzz"
base = Path(sys.argv[1]).resolve()

def put(target, name, data, limit=200_000):
    if len(data) <= limit:
        d = ROOT / "corpus" / target
        d.mkdir(parents=True, exist_ok=True)
        (d / name.replace("/", "_")).write_bytes(data)

def pak_files(p):
    with open(p, "rb") as f:
        magic, ofs, n = struct.unpack("<4sii", f.read(12))
        if magic != b"PACK":
            return
        f.seek(ofs)
        d = f.read(n)
        if len(d) < n:
            print(f'{p}: truncated: directory at {ofs}+{n}, file is {os.path.getsize(p)}')
            return
        for i in range(0, n - 63, 64):
            name, o, l = struct.unpack("<56sii", d[i:i + 64])
            f.seek(o)
            yield name.split(b"\0")[0].decode("latin1").lower(), f.read(l)

KIND = {".mdl": "0", ".md3": "1", ".iqm": "2", ".spr": "3", ".wad": "4", ".wav": "5", ".lmp": "6",
        ".pcx": "7", ".tga": "8", ".lit": "a"}
counts = {}
paks = sorted(base.glob("*/pak[0-9].pak"))
for pak in paks:
    for name, data in pak_files(pak):
        ext = os.path.splitext(name)[1]
        key = (pak.parent.name, ext)
        if ext == ".bsp":
            if len(data) < 60_000 and counts.get(key, 0) < 8:
                counts[key] = counts.get(key, 0) + 1
                put("bsp", name, data)
        elif name.endswith("progs.dat"):
            put("progs", pak.parent.name, data, 900_000)
        elif ext in KIND and len(data) < 40_000 and counts.get(key, 0) < 6:
            counts[key] = counts.get(key, 0) + 1
            put("formats", name, KIND[ext].encode() + data)
        elif ext == ".cfg" or ext == ".rc":
            put("text", name, b"0" + data)
# the rest of the loose particle scripts
for p in list(base.glob("*/particles/*.cfg"))[:10]:
    put("text", p.name, b"1" + p.read_bytes())
for p in list(base.glob("*/effectinfo*.txt"))[:4]:
    put("text", p.name, b"2" + p.read_bytes())
put("text", "json", b'3{"a":[1,2.5,{"b":null}],"c":"\\u0041"}')
pad = struct.pack("<4sii", b"PACK", 17, 64) + b"hello" + b"a.txt".ljust(56, b"\0") + struct.pack("<ii", 12, 5)
put("packs", "pak", b"0" + pad)
z = io.BytesIO()
with zipfile.ZipFile(z, "w") as f:
    f.writestr("a.txt", "hello" * 40, zipfile.ZIP_DEFLATED)
    f.writestr("b/c.txt", "stored", zipfile.ZIP_STORED)
put("packs", "pk3", b"1" + z.getvalue())
put("netchan", "ccreq", struct.pack("<H", 12) + b"\x80\0\0\x0c\x01QUAKE\0\x03" + b"\x0c\0\0\x01\0\x0c\0\0\0\0abcd")
put("netchan", "oob", struct.pack("<H", 9) + b"\xff\xff\xff\xffnabc\0")
for i, cmd in enumerate(["prespawn", "spawn", "begin", "god", "give h 100", "say hi", "name x", "color 1 2", "kill",
                         "pext 0x58455446 1 0x32455446 0xffff", "ping", "status", "tell x y", "fly", "noclip"]):
    put("server", f"cmd{i}", b"\0" + cmd.encode() + b"\n\6")
put("server", "session", b"\0prespawn\n\6\0spawn\n\6\0begin\n\6" + (b"\3" + b"\0" * 24 + b"\1\7\6\6") * 6)
for p in base.glob("id1/*.sav"):
    put("savegame", p.name, p.read_bytes(), 400_000)
for p in (Path(__file__).resolve().parents[2] / "bug-reports").glob("*/save.*"):
    put("savegame", p.parent.name + p.suffix, p.read_bytes(), 400_000)
b = ROOT / "base/id1"
b.mkdir(parents=True, exist_ok=True)
for p in (base / "id1").glob("*.pak"):
    if not (b / p.name).exists():
        (b / p.name).symlink_to(p)
for d in sorted((ROOT / "corpus").iterdir()):
    print(d.name, len(list(d.iterdir())))
