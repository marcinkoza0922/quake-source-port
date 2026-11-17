#!/usr/bin/env python3
"""Subset Noto Sans CJK (JP, KR, SC, TC) to the characters the port ships.

The full regional fonts are 4-8 MB each; the engine only needs the
characters of its Japanese, Korean and Chinese catalogs and manuals (plus
kana, Hangul jamo, CJK punctuation, full-width forms and every language's
own name), so it bundles subsets: assets/fonts/NotoSans<R>-Subset.otf.
Re-run after changing those catalogs or manuals, or new text shows as
empty boxes.

    python3 -m venv /tmp/v && /tmp/v/bin/pip install fonttools
    /tmp/v/bin/python3 tools/cjk_fonts.py [SRC_DIR]

SRC_DIR holds NotoSans<R>-Regular.otf (from
https://github.com/notofonts/noto-cjk/tree/main/Sans/SubsetOTF, SIL OFL);
missing files are downloaded there (default: target/cjk-fonts).
"""
import os
import re
import sys
import urllib.request

from fontTools import subset

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
URL = "https://github.com/notofonts/noto-cjk/raw/main/Sans/SubsetOTF/{r}/NotoSans{r}-Regular.otf"
# region -> catalog/manual codes whose text it draws first
REGIONS = {"JP": ["ja"], "KR": ["ko"], "SC": ["zh-CN"], "TC": ["zh-TW", "yue"]}

BASE = set(range(0x20, 0x7F)) | set(range(0x3000, 0x3040)) | set(range(0xFF00, 0xFFF0)) | {0x2026, 0x2014, 0x2013, 0x00B7, 0x2018, 0x2019, 0x201C, 0x201D, 0x2192, 0x2190}
EXTRA = {
    "JP": set(range(0x3040, 0x3100)) | set(range(0x31F0, 0x3200)),
    "KR": set(range(0x3130, 0x3190)),
    "SC": set(),
    "TC": set(),
}


def read(path):
    try:
        with open(path, encoding="utf-8") as f:
            return f.read()
    except FileNotFoundError:
        print(f"warning: {os.path.relpath(path, ROOT)} missing")
        return ""


def chars_for(region):
    text = ""
    for code in REGIONS[region]:
        text += read(os.path.join(ROOT, "crates", "qcore", "lang", "languages", f"{code}.txt"))
        text += read(os.path.join(ROOT, "docs", f"MANUAL.{code}.md"))
    # every language's own name (the Language row shows them all)
    lang = read(os.path.join(ROOT, "crates", "qcore", "src", "lang.rs"))
    text += "".join(re.findall(r'\(Lang::\w+, "[^"]*", "[^"]*", "([^"]*)"\)', lang))
    return {ord(c) for c in text if ord(c) >= 0x80} | BASE | EXTRA[region]


def main(argv):
    src = argv[1] if len(argv) > 1 else os.path.join(ROOT, "target", "cjk-fonts")
    os.makedirs(src, exist_ok=True)
    for region in REGIONS:
        path = os.path.join(src, f"NotoSans{region}-Regular.otf")
        if not os.path.exists(path):
            print(f"downloading {URL.format(r=region)}")
            urllib.request.urlretrieve(URL.format(r=region), path)
        out = os.path.join(ROOT, "assets", "fonts", f"NotoSans{region}-Subset.otf")
        opts = subset.Options()
        opts.layout_features = []
        opts.hinting = False
        opts.name_IDs = ["*"]
        opts.name_languages = ["*"]
        opts.notdef_outline = True
        font = subset.load_font(path, opts)
        s = subset.Subsetter(opts)
        cps = chars_for(region)
        s.populate(unicodes=cps)
        s.subset(font)
        subset.save_font(font, out, opts)
        print(f"{os.path.relpath(out, ROOT)}: {len(cps)} code points, {os.path.getsize(out) // 1024} KiB")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
