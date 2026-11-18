#!/usr/bin/env python3
"""Static compatibility inventory of installed Quake game dirs (read-only).

For each game dir it lists what the content relies on, so engine gaps can be
found without playing: file types by directory, console commands/cvars in
shipped configs and in progs strings (stuffcmd/localcmd/cvar text), the
builtins and fields each progs declares, BSP formats and BSPX lumps, special
texture names, worldspawn and entity keys, model/sprite/sound formats and
companion files (.lit/.ent/.vis/...).

  compat_audit.py [-b BASEDIR] [-n NAMES_DIR] [-o OUT_DIR] [game ...]

NAMES_DIR may hold `<engine>_all.txt` files (one lowercase console name per
line: ours, ironwail, vkQuake); every console word found in the content is
then tagged with the engines that know it. Writes OUT_DIR/inventory.json and
OUT_DIR/inventory.md (default target/compat).
"""
import argparse
import json
import os
import re
import struct
import sys
import zipfile
from collections import Counter, defaultdict

sys.dont_write_bytecode = True  # a tools/__pycache__ dir would break the cargo workspace glob
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from qdata import Pak, Pk3, base_dir, ci_child, parse_entities  # noqa: E402

BSP_LUMPS29 = 15
BSPX_MAGIC = b"BSPX"
WORD = re.compile(r"[A-Za-z_+\-][A-Za-z0-9_+\-.]*")
NOT_CFG_DIRS = ("particles/", "effectinfo", "mapsrc/", "src/", "source/", "docs/", "_src/", "qcc/", "keepsrc/")


def own_files(gdir):
    """Yield (relpath_lower, source, read) for every file a game dir supplies."""
    for name in sorted(os.listdir(gdir)):
        p = os.path.join(gdir, name)
        low = name.lower()
        pack = None
        try:
            if re.fullmatch(r"pak\d+\.pak", low):
                pack = Pak(p)
            elif low.endswith(".pk3") or low.endswith(".pk4"):
                pack = Pk3(p)
        except (OSError, ValueError, struct.error, zipfile.BadZipFile) as e:
            print(f"compat_audit: {p}: unreadable ({e})", file=sys.stderr)
            continue
        if pack is not None:
            for n in sorted(pack.names()):
                yield n, name, (lambda n=n, pack=pack: pack.read(n))
    for root, _dirs, files in os.walk(gdir, followlinks=True):
        for f in files:
            full = os.path.join(root, f)
            rel = os.path.relpath(full, gdir).replace(os.sep, "/")
            if "/" not in rel and (re.fullmatch(r"pak\d+\.pak", rel.lower()) or rel.lower().endswith((".pk3", ".pk4"))):
                continue
            yield rel.lower(), "loose", (lambda full=full: open(full, "rb").read())


def cfg_commands(text):
    """First word of every console command in a cfg (handles ; and quotes)."""
    out = []
    for line in text.splitlines():
        cmd, q = "", False
        i = 0
        while i < len(line):
            c = line[i]
            if c == '"':
                q = not q
            if not q and line.startswith("//", i):
                break
            if not q and c == ";":
                out.append(cmd)
                cmd = ""
            else:
                cmd += c
            i += 1
        out.append(cmd)
    words = []
    for c in out:
        t = c.strip().split(None, 2)
        if not t:
            continue
        w = t[0].strip('"').lower()
        words.append(w)
        # nested: alias x "a; b", bind k "cmd"
        if w in ("alias", "bind") and len(t) > 2:
            words += [x for x in cfg_commands(t[2].strip('"')) if x]
        if w in ("set", "seta", "toggle", "cycle", "inc", "reset") and len(t) > 1:
            words.append(t[1].strip('"').lower())
    return [w for w in words if WORD.fullmatch(w)]


def parse_progs(data):
    if len(data) < 60:
        return None
    h = struct.unpack("<15i", data[:60])
    version, crc = h[0], h[1]
    ofs_st, _ns, ofs_gd, n_gd, ofs_fd, n_fd, ofs_fn, n_fn, ofs_str, n_str = h[2:12]
    if version not in (6, 7):
        return {"version": version, "crc": crc}
    strs = data[ofs_str:ofs_str + n_str]

    def s(o):
        if o < 0 or o >= len(strs):
            return ""
        return strs[o:strs.find(b"\0", o)].decode("latin-1")

    builtins = {}
    for i in range(n_fn):
        b = data[ofs_fn + i * 36:ofs_fn + (i + 1) * 36]
        first, _parm, _locals, _prof, name, _file, _np = struct.unpack("<iiiiiii", b[:28])
        if first < 0:
            builtins[s(name)] = -first
        elif first == 0 and s(name):
            builtins.setdefault(s(name), 0)
    fields = []
    for i in range(n_fd):
        _t, _o, name = struct.unpack("<HHi", data[ofs_fd + i * 8:ofs_fd + i * 8 + 8])
        fields.append(s(name))
    gdefs = []
    for i in range(n_gd):
        _t, _o, name = struct.unpack("<HHi", data[ofs_gd + i * 8:ofs_gd + i * 8 + 8])
        gdefs.append(s(name))
    strings = [x.decode("latin-1") for x in strs.split(b"\0") if x]
    # Builtins the code actually calls: OP_CALL0..8 (51..59) name a global
    # that holds a function index; a header full of unused declarations
    # (dpextensions.qc) is harmless.
    ofs_gl, n_gl = h[12], h[13]
    glob = struct.unpack(f"<{n_gl}i", data[ofs_gl:ofs_gl + n_gl * 4])
    fnames, fnum = [], []
    for i in range(n_fn):
        b = data[ofs_fn + i * 36:ofs_fn + i * 36 + 20]
        first, _p, _l, _pr, name = struct.unpack("<iiiii", b)
        fnames.append(s(name))
        fnum.append(first)
    called = set()
    for i in range(_ns):
        op, a = struct.unpack("<Hh", data[ofs_st + i * 8:ofs_st + i * 8 + 4])
        if 51 <= op <= 59 and 0 <= (a & 0xFFFF) < n_gl:
            f = glob[a & 0xFFFF]
            if 0 < f < n_fn and fnum[f] <= 0:
                called.add(fnames[f])
    return {"version": version, "crc": crc, "builtins": builtins, "called_builtins": sorted(called), "fields": fields,
            "globals": gdefs, "strings": strings}


def bsp_info(data):
    ident = data[:4]
    kind = {b"BSP2": "BSP2", b"2PSB": "2PSB", b" 46Q": "Q64"}.get(ident)
    if kind is None:
        v = struct.unpack("<i", ident)[0]
        kind = "BSP29" if v == 29 else f"ver{v}"
    lumps = [struct.unpack("<ii", data[4 + i * 8:12 + i * 8]) for i in range(BSP_LUMPS29)]
    info = {"format": kind, "size": len(data)}
    end = max(o + ln for o, ln in lumps)
    p = (end + 3) & ~3
    bspx = []
    if data[p:p + 4] == BSPX_MAGIC:
        n = struct.unpack("<i", data[p + 4:p + 8])[0]
        for i in range(min(n, 64)):
            e = data[p + 8 + i * 32:p + 8 + (i + 1) * 32]
            bspx.append(e[:24].split(b"\0", 1)[0].decode("latin-1"))
    info["bspx"] = bspx
    # textures (lump 2)
    to, tl = lumps[2]
    texs = []
    if tl >= 4:
        n = struct.unpack("<i", data[to:to + 4])[0]
        for i in range(max(0, min(n, 4096))):
            o = struct.unpack("<i", data[to + 4 + i * 4:to + 8 + i * 4])[0]
            if o < 0:
                continue
            b = data[to + o:to + o + 40]
            if len(b) < 40:
                continue
            name = b[:16].split(b"\0", 1)[0].decode("latin-1").lower()
            w, hgt, m0 = struct.unpack("<IIi", b[16:28])
            texs.append((name, w, hgt, m0))
    info["textures"] = texs
    info["lighting_bytes"] = lumps[8][1]
    info["vis_bytes"] = lumps[4][1]
    model_size = 64 if kind in ("BSP29", "Q64", "BSP2", "2PSB") else 64
    info["models"] = lumps[14][1] // model_size
    face_size = 20 if kind in ("BSP29", "Q64") else 28
    info["faces"] = lumps[7][1] // face_size
    eo, el = lumps[0]
    info["entities"] = parse_entities(data[eo:eo + el].split(b"\0", 1)[0].decode("latin-1"))
    return info


def tex_class(name):
    for pre, cls in (("sky", "sky"), ("*", "liquid"), ("+", "animated"), ("{", "fence")):
        if name.startswith(pre):
            return cls
    return None


def wav_info(d):
    if d[:4] != b"RIFF" or d[8:12] != b"WAVE":
        return "not-riff"
    p, fmt, extra = 12, None, []
    while p + 8 <= len(d):
        cid, ln = d[p:p + 4], struct.unpack("<I", d[p + 4:p + 8])[0]
        if cid == b"fmt ":
            tag, ch, rate, _br, _al, bits = struct.unpack("<HHIIHH", d[p + 8:p + 24])
            fmt = f"tag{tag}/{ch}ch/{rate}/{bits}b"
        elif cid in (b"cue ", b"LIST", b"smpl"):
            extra.append(cid.decode().strip())
        p += 8 + ln + (ln & 1)
    return (fmt or "nofmt") + ("+" + "+".join(sorted(set(extra))) if extra else "")


def model_info(rel, d):
    m = d[:4]
    if m == b"IDPO":
        ver = struct.unpack("<i", d[4:8])[0]
        sw, sh = struct.unpack("<ii", d[52:60])
        nv, nt, nf, _st, flags = struct.unpack("<iiiii", d[60:80])
        return {"kind": f"mdl{ver}", "flags": flags, "skin": f"{sw}x{sh}", "verts": nv, "tris": nt, "frames": nf}
    if m == b"MD16":
        return {"kind": "md16"}
    if m == b"IDP3":
        return {"kind": "md3"}
    if m == b"IDSP":
        ver = struct.unpack("<i", d[4:8])[0]
        typ = struct.unpack("<i", d[8:12])[0]
        return {"kind": f"spr{ver}", "type": typ}
    if d[:16] == b"INTERQUAKEMODEL\0":
        return {"kind": "iqm"}
    if m == b"IDBS" or m == b"BSP2" or m == b"2PSB" or struct.unpack("<i", m)[0] == 29:
        return {"kind": "bspmodel"}
    if rel.endswith(".md5mesh") or rel.endswith(".md5anim"):
        return {"kind": "md5"}
    return {"kind": "other:" + rel.rsplit(".", 1)[-1]}


def audit_game(base, game, names):
    gdir = ci_child(base, game)
    r = {"game": game, "ext": Counter(), "dirs": Counter(), "cfg": {}, "progs": {}, "bsp": {}, "models": Counter(),
         "model_flags": Counter(), "big_skins": [], "wav": Counter(), "companions": Counter(), "misc_files": []}
    files = list(own_files(gdir))
    seen = set()
    for rel, src, read in files:
        ext = rel.rsplit(".", 1)[-1] if "." in rel.rsplit("/", 1)[-1] else ""
        top = rel.split("/", 1)[0] if "/" in rel else "."
        r["ext"][ext] += 1
        r["dirs"][f"{top}/*.{ext}"] += 1
        if rel in seen:
            continue
        seen.add(rel)
        if any(rel.startswith(x) or f"/{x}" in rel for x in NOT_CFG_DIRS):
            if ext in ("cfg", "txt") and ("particles/" in rel or "effectinfo" in rel):
                r["misc_files"].append(rel)
            if not rel.endswith(".dat"):
                continue
        try:
            if ext in ("cfg", "rc"):
                r["cfg"][f"{src}:{rel}"] = sorted(set(cfg_commands(read().decode("latin-1"))))
            elif ext == "dat" and rel.rsplit("/", 1)[-1] in ("progs.dat", "qwprogs.dat", "csprogs.dat", "menu.dat") or rel.startswith("progs/") and ext == "dat":
                p = parse_progs(read())
                if p:
                    r["progs"][f"{src}:{rel}"] = p
            elif ext == "bsp" and rel.startswith("maps/") and rel.count("/") == 1:
                r["bsp"][rel[5:-4]] = bsp_info(read())
            elif ext in ("mdl", "md3", "spr", "spr32", "iqm", "md5mesh", "md5anim") or (ext == "bsp" and not rel.startswith("maps/")):
                mi = model_info(rel, read())
                r["models"][mi["kind"]] += 1
                if "flags" in mi and mi["flags"]:
                    r["model_flags"][hex(mi["flags"])] += 1
                if mi.get("skin") and any(int(x) > 512 for x in mi["skin"].split("x")):
                    r["big_skins"].append(f"{rel} {mi['skin']}")
            elif ext == "wav":
                r["wav"][wav_info(read()[:4096])] += 1
            elif ext in ("lit", "lux", "ent", "vis", "loc", "rtlights", "lit2"):
                r["companions"][ext] += 1
            elif ext in ("skybox", "shader", "skin", "mat", "material", "effect", "lst", "fnt", "font", "kfont", "json", "ttf", "otf", "glsl", "def"):
                r["misc_files"].append(rel)
        except (struct.error, ValueError, IndexError) as e:
            r.setdefault("parse_errors", []).append(f"{rel}: {e}")
    # console names in progs strings
    allnames = set().union(*names.values()) if names else set()
    qc_words, stuff = Counter(), []
    for pk, p in r["progs"].items():
        for st in p.get("strings", []):
            if "\n" in st or ";" in st:
                stuff.append(st.strip())
            for w in WORD.findall(st):
                if w.lower() in allnames and len(w) > 3:
                    qc_words[w.lower()] += 1
    r["qc_console_words"] = qc_words
    r["qc_stuff_strings"] = sorted(set(x for x in stuff if len(x) < 200 and WORD.match(x)))[:400]
    return r


def summarize(r, names):
    lines = [f"## {r['game']}", ""]
    lines.append("Files: " + ", ".join(f"{k or '(none)'}:{v}" for k, v in r["ext"].most_common(25)))
    lines.append("")
    lines.append("Dirs: " + ", ".join(f"{k}:{v}" for k, v in r["dirs"].most_common(40)))
    lines.append("")

    def tag(w):
        if not names:
            return w
        have = [e for e, s in names.items() if w in s]
        if "ours" in have:
            return None
        return f"{w}[{','.join(have) or 'none'}]"

    cfgw = set()
    for k, ws in r["cfg"].items():
        cfgw.update(ws)
    tagged = [t for t in (tag(w) for w in sorted(cfgw)) if t]
    lines.append(f"Configs: {', '.join(r['cfg']) or 'none'}")
    lines.append(f"Config words unknown to ours: {', '.join(tagged) or 'none'}")
    qtag = [t for t in (tag(w) for w in sorted(r['qc_console_words'])) if t]
    lines.append(f"Progs console words unknown to ours: {', '.join(qtag) or 'none'}")
    for pk, p in r["progs"].items():
        b = p.get("builtins", {})
        ext = sorted(f"{n}#{v}" for n, v in b.items() if v == 0 or v > 85)
        lines.append(f"Progs {pk}: v{p['version']} crc {p['crc']}, {len(b)} builtins, {len(p.get('fields', []))} fields; extension builtins: {', '.join(ext) or 'none'}")
    fmts = Counter(i["format"] for i in r["bsp"].values())
    bspx = Counter(x for i in r["bsp"].values() for x in i["bspx"])
    lines.append(f"BSP: {dict(fmts)}; BSPX lumps: {dict(bspx) or 'none'}; companions: {dict(r['companions']) or 'none'}")
    ws_keys, ent_keys, classes = Counter(), Counter(), Counter()
    texcls, bigtex, external = Counter(), [], 0
    for m, i in r["bsp"].items():
        for name, w, h, m0 in i["textures"]:
            c = tex_class(name)
            if c:
                texcls[c] += 1
            if w > 1024 or h > 1024:
                bigtex.append(f"{m}:{name} {w}x{h}")
            if m0 == 0:
                external += 1
        for e in i["entities"]:
            d = dict(e)
            classes[d.get("classname", "?")] += 1
            for k, _v in e:
                (ws_keys if d.get("classname") == "worldspawn" else ent_keys)[k] += 1
    lines.append(f"Texture classes: {dict(texcls)}; textures without embedded pixels: {external}; >1024px: {bigtex[:8]}")
    lines.append(f"Worldspawn keys: {', '.join(f'{k}:{v}' for k, v in sorted(ws_keys.items()))}")
    lines.append(f"Entity keys ({len(ent_keys)}): {', '.join(f'{k}:{v}' for k, v in ent_keys.most_common(120))}")
    lines.append(f"Classnames ({len(classes)}): {', '.join(f'{k}:{v}' for k, v in classes.most_common(60))}")
    big = sorted(((len(i['entities']), i['models'], i['faces'], m) for m, i in r['bsp'].items()), reverse=True)[:5]
    lines.append(f"Largest maps (ents, models, faces): {big}")
    lines.append(f"Models: {dict(r['models'])}; nonzero flags: {dict(r['model_flags'].most_common(12))}; big skins: {r['big_skins'][:6]}")
    lines.append(f"WAV formats: {dict(r['wav'].most_common(12))}")
    lines.append(f"Other notable files: {sorted(set(r['misc_files']))[:40]}")
    if r.get("parse_errors"):
        lines.append(f"Parse errors: {r['parse_errors'][:10]}")
    if r["qc_stuff_strings"]:
        lines.append("QC stuff/console strings (sample):")
        lines += [f"    {json.dumps(s)}" for s in r["qc_stuff_strings"][:60]]
    lines.append("")
    return "\n".join(lines)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("-b", "--basedir")
    ap.add_argument("-n", "--names")
    ap.add_argument("-o", "--out", default="target/compat")
    ap.add_argument("games", nargs="*")
    a = ap.parse_args()
    base = base_dir(a.basedir)
    names = {}
    if a.names:
        for f in sorted(os.listdir(a.names)):
            if f.endswith("_all.txt"):
                with open(os.path.join(a.names, f)) as fh:
                    names[f[:-8]] = {x.strip() for x in fh if x.strip()}
    games = a.games or sorted(e for e in os.listdir(base) if os.path.isdir(os.path.join(base, e)) and e != "quake-source-port")
    os.makedirs(a.out, exist_ok=True)
    md, js = ["# Compatibility inventory", ""], {}
    for g in games:
        print(f"compat_audit: {g}", file=sys.stderr)
        r = audit_game(base, g, names)
        md.append(summarize(r, names))
        for p in r["progs"].values():
            p.pop("strings", None)
        for i in r["bsp"].values():
            i["textures"] = len(i["textures"])
            i["worldspawn"] = dict(i["entities"][0]) if i["entities"] else {}
            i["entities"] = len(i["entities"])
        js[g] = r
    with open(os.path.join(a.out, "inventory.md"), "w") as f:
        f.write("\n".join(md))
    with open(os.path.join(a.out, "inventory.json"), "w") as f:
        json.dump(js, f, indent=1, default=lambda o: dict(o) if isinstance(o, Counter) else str(o))
    print(f"compat_audit: wrote {a.out}/inventory.md and inventory.json", file=sys.stderr)


if __name__ == "__main__":
    main()
