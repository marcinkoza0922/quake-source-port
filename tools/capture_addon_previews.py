#!/usr/bin/env python3
"""Capture silent, four-second add-on gameplay loops using the headless renderer.

Requires an existing release binary, installed Quake data and Pillow. Does not
modify game data or config.cfg. Raw screenshots/logs stay under target/.
Usage: python3 tools/capture_addon_previews.py [--engine PATH] [--basedir DIR]
QAV1 is a small independent-frame RGB/zlib container, not a new codec dependency.
"""
import argparse
from pathlib import Path
import struct
import subprocess
import zlib

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
WIDTH, HEIGHT, FRAMES, FPS = 320, 92, 48, 12
IDS = ["portalgun", "boomerang", "sticky", "hookshot", "railgun", "goggles"]


def capture(engine, basedir, addon):
    work = ROOT / "target" / "addon-previews" / addon
    work.mkdir(parents=True, exist_ok=True)
    settings = {
        "addons_enable": 1, "skill": 2, "scr_centertime": 0,
        "con_notifytime": 0, "scr_showfps": 0, "scr_hudstyle": 1,
        "hud_health": 0, "hud_armor": 0, "hud_ammo": 0, "hud_ammolist": 0,
        "hud_weapons": 0, "hud_items": 0, "hud_face": 0, "crosshair": 1,
        "host_maxfps": 120, "r_drawviewmodel": 1, "v_flashscale": 0,
        "sv_autosave_keep": 0, "volume": 0, "bgmvolume": 0,
    }
    args = [str(engine), "-headless", "-width", "960", "-height", "276",
            "-autoquit", "7.4"]
    if basedir:
        args += ["-basedir", basedir]
    for name, value in settings.items():
        args += [f"+{name}", str(value)]
    args += ["+map", "e1m1"]

    def at(t, cmd):
        args.append(f"+delay {t:.4f} {cmd}")

    for cmd in ["god", "notarget", "impulse 9", "setpos 440 72 40 4 0 0"]:
        at(1.0, cmd)
    if addon != "goggles":
        for i in range(IDS.index(addon) + 1):
            at(1.2 + i * 0.2, "+toy")
            at(1.3 + i * 0.2, "-toy")
    if addon == "portalgun":
        at(2.5, "setpos 470 160 40 0 80 0")
        at(3.1, "+attack")
        at(3.2, "-attack")
        at(3.3, "portal_set 0 480 80 48 0 1 0 0 0 1")
        at(4.0, "+attack2")
        at(4.1, "-attack2")
        at(4.2, "portal_set 1 480 330 48 0 -1 0 0 0 1")
    elif addon == "goggles":
        at(3.9, "goggles nv")
        at(5.2, "goggles thermal")
    else:
        at(3.1, "+attack")
        at(4.0 if addon == "boomerang" else 3.5, "-attack")
        if addon in ["sticky", "railgun"]:
            at(4.4, "+attack")
            at(4.8, "-attack")
        if addon == "sticky":
            at(5.6, "+attack2")
            at(5.7, "-attack2")
        if addon == "hookshot":
            at(5.1, "+attack")
            at(5.2, "-attack")
    for frame in range(FRAMES):
        at(2.8 + frame / FPS, f"screenshot {work / f'{frame:03}.png'}")
    with (work / "capture.log").open("w") as log:
        subprocess.run(args, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT,
                       check=True, timeout=120)
    output = bytearray(struct.pack("<4sHHHH", b"QAV1", WIDTH, HEIGHT, FRAMES, FPS))
    for frame in range(FRAMES):
        with Image.open(work / f"{frame:03}.png") as image:
            rgb = image.convert("RGB").resize((WIDTH, HEIGHT), Image.Resampling.LANCZOS)
            compressed = zlib.compress(rgb.tobytes(), 9)
        output += struct.pack("<I", len(compressed)) + compressed
    dest = ROOT / "assets" / "addon-previews" / f"{addon}.qav"
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(output)
    print(f"{addon}: {FRAMES} gameplay frames, {len(output):,} bytes", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", type=Path, default=ROOT / "target/release/quake-source-port")
    parser.add_argument("--basedir")
    parser.add_argument("--addon", choices=IDS)
    opts = parser.parse_args()
    for addon in [opts.addon] if opts.addon else IDS:
        capture(opts.engine.resolve(), opts.basedir, addon)


if __name__ == "__main__":
    main()
