#!/usr/bin/env python3
"""Make the bundled fonts for scripts that need text shaping.

Arabic (Arabic, Persian, Urdu), Hebrew, Devanagari (Hindi, Marathi),
Bengali, Gurmukhi (Punjabi), Gujarati, Tamil, Telugu, Thai, Khmer and
Ethiopic (Amharic) come from Noto Sans (SIL OFL). The engine shapes them
with rustybuzz, so each subset keeps its script's whole Unicode blocks and
every OpenType layout feature (GSUB/GPOS/GDEF), but drops Latin (Inter draws
it), hinting (ab_glyph ignores it) and glyph names:
assets/fonts/NotoSans<Script>-Subset.ttf.

    python3 -m venv /tmp/v && /tmp/v/bin/pip install fonttools
    /tmp/v/bin/python3 tools/script_fonts.py [SRC_DIR]

SRC_DIR holds NotoSans<Script>-Regular.ttf (from
https://github.com/notofonts/notofonts.github.io/tree/main/fonts); missing
files are downloaded there (default: target/script-fonts).
"""
import os
import sys
import urllib.request

from fontTools import subset
from fontTools.ttLib import TTFont

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
URL = "https://raw.githubusercontent.com/notofonts/notofonts.github.io/main/fonts/NotoSans{s}/hinted/ttf/NotoSans{s}-Regular.ttf"


def r(a, b):
    return set(range(a, b + 1))


# Shared by every script: spaces, joiners and directional marks, dotted
# circle (shapers insert it before stray marks), common punctuation.
COMMON = (
    {0x20, 0xA0, 0x2D, 0x2E, 0x2C, 0x3A, 0x3B, 0x21, 0x3F, 0x28, 0x29, 0x25CC, 0x2026}
    | r(0x200B, 0x200F)
    | r(0x2010, 0x2015)
    | r(0x2018, 0x201F)
    | r(0x2060, 0x2064)
    | r(0x0030, 0x0039)
)
# Indic scripts share the dandas (U+0964/5) and the Vedic signs.
INDIC = {0x0964, 0x0965} | r(0x1CD0, 0x1CFF) | r(0xA8E0, 0xA8FF)

SCRIPTS = {
    "Arabic": r(0x0600, 0x06FF) | r(0x0750, 0x077F) | r(0x0870, 0x08FF) | r(0xFB50, 0xFDFF) | r(0xFE70, 0xFEFF),
    "Hebrew": r(0x0590, 0x05FF) | r(0xFB1D, 0xFB4F),
    "Devanagari": r(0x0900, 0x097F) | r(0xA8E0, 0xA8FF) | INDIC,
    "Bengali": r(0x0980, 0x09FF) | INDIC,
    "Gurmukhi": r(0x0A00, 0x0A7F) | INDIC,
    "Gujarati": r(0x0A80, 0x0AFF) | INDIC,
    "Tamil": r(0x0B80, 0x0BFF) | r(0x11FC0, 0x11FFF) | INDIC,
    "Telugu": r(0x0C00, 0x0C7F) | INDIC,
    "Thai": r(0x0E00, 0x0E7F),
    "Khmer": r(0x1780, 0x17FF) | r(0x19E0, 0x19FF),
    "Ethiopic": r(0x1200, 0x139F) | r(0x2D80, 0x2DDF) | r(0xAB00, 0xAB2F) | r(0x1E7E0, 0x1E7FF),
}


def main(argv):
    src = argv[1] if len(argv) > 1 else os.path.join(ROOT, "target", "script-fonts")
    os.makedirs(src, exist_ok=True)
    out_dir = os.path.join(ROOT, "assets", "fonts")
    for script, chars in SCRIPTS.items():
        path = os.path.join(src, f"NotoSans{script}-Regular.ttf")
        if not os.path.exists(path):
            print(f"downloading {script}")
            urllib.request.urlretrieve(URL.format(s=script), path)
        font = TTFont(path)
        have = set(font.getBestCmap())
        opts = subset.Options()
        opts.layout_features = ["*"]
        opts.layout_scripts = ["*"]
        opts.hinting = False
        opts.glyph_names = False
        opts.notdef_outline = True
        opts.name_IDs = ["*"]
        opts.name_languages = ["*"]
        sub = subset.Subsetter(opts)
        sub.populate(unicodes=sorted((chars | COMMON) & have))
        sub.subset(font)
        out = os.path.join(out_dir, f"NotoSans{script}-Subset.ttf")
        font.save(out)
        print(f"{os.path.relpath(out, ROOT)}: {os.path.getsize(out) // 1024} KiB")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
