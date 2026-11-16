#!/usr/bin/env python3
"""Checks that the Quaddicted source behind Map Packs > Get more maps still works.

The engine (src/quaddicted.rs) reads the archive from the Quaddicted/quaddicted-data
GitHub repo as one zip, because quaddicted_database.xml is frozen and the "new
Quaddicted API" was not live yet (2026-11). This script:

  * downloads that zip and checks the release JSON still has the fields the
    engine reads (sha256, urls, tags title=/release_date=/game_mode=, install);
  * checks that a release download URL still answers;
  * reports when the new API at /api/v1/ starts returning data, or the XML
    feed comes back to life, so the engine can move to the official route.

Exit status: 0 = fine (or offline: a warning only), 1 = the route is broken.
--daily skips the run when it passed in the last 24 h (used by the pre-commit
hook, .githooks/pre-commit). Standard library only.
"""

import io
import json
import os
import sys
import time
import urllib.request
import zipfile

INDEX_URL = "https://codeload.github.com/Quaddicted/quaddicted-data/zip/refs/heads/main"
API_URL = "https://www.quaddicted.com/api/v1/"
XML_URL = "https://www.quaddicted.com/reviews/quaddicted_database.xml"
STAMP = os.path.join(os.path.dirname(__file__), "..", "target", "quaddicted_check.stamp")
UA = {"User-Agent": "quake-source-port-check"}


def get(url, limit=None):
    req = urllib.request.Request(url, headers=UA)
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.read(limit) if limit else r.read()


def check_index():
    """Returns (problems, sample download url)."""
    z = zipfile.ZipFile(io.BytesIO(get(INDEX_URL)))
    names = [n for n in z.namelist() if "json/by-sha256/" in n and n.endswith(".json")]
    if len(names) < 1000:
        return [f"index has only {len(names)} release files (layout changed?)"], None
    problems, counts, sample = [], {"title": 0, "date": 0, "sp": 0, "install": 0}, None
    for n in names:
        try:
            d = json.loads(z.read(n))
        except ValueError:
            problems.append(f"{n}: not JSON")
            continue
        if not isinstance(d.get("sha256"), str) or not d.get("urls"):
            problems.append(f"{n}: no sha256/urls")
            continue
        tags = [t for t in d.get("tags", []) if isinstance(t, str)]
        counts["title"] += any(t.startswith("title=") for t in tags)
        counts["date"] += any(t.startswith("release_date=") for t in tags)
        counts["sp"] += "game_mode=singleplayer" in tags
        inst = d.get("install") or {}
        counts["install"] += isinstance(inst, dict) and ("extract" in inst or "extractmapping" in inst)
        if sample is None and d.get("bytes", 1 << 40) < 2_000_000:
            sample = d["urls"][0]
    if len(problems) > len(names) // 20:
        problems = problems[:5] + [f"... {len(problems)} bad release files"]
    else:
        problems = []
    for k, v in counts.items():
        if v < len(names) * 0.8:
            problems.append(f"only {v}/{len(names)} releases have '{k}' (tag/field renamed?)")
    print(f"index: {len(names)} releases, {counts}")
    return problems, sample


def notices():
    out = []
    try:
        body = get(API_URL, 4096).strip()
        if body not in (b"[]", b""):
            out.append(f"{API_URL} now returns data: the new Quaddicted API may be live")
    except Exception as e:  # noqa: BLE001 - informational only
        out.append(f"{API_URL}: {e}")
    try:
        head = get(XML_URL, 4096)
        if b"not updated anymore" not in head:
            out.append("quaddicted_database.xml no longer carries the deprecation notice")
    except Exception as e:  # noqa: BLE001
        out.append(f"{XML_URL}: {e}")
    return out


def main():
    daily = "--daily" in sys.argv
    if daily and os.path.exists(STAMP) and time.time() - os.path.getmtime(STAMP) < 86400:
        return 0
    try:
        problems, sample = check_index()
        if sample:
            try:
                get(sample, 1024)
            except Exception as e:  # noqa: BLE001
                problems.append(f"release download {sample}: {e}")
    except (OSError, zipfile.BadZipFile) as e:
        if isinstance(e, zipfile.BadZipFile) or getattr(e, "code", None):
            problems = [f"index {INDEX_URL}: {e}"]
        else:
            print(f"quaddicted check: offline? ({e}); skipped", file=sys.stderr)
            return 0
    for n in notices():
        print(f"NOTICE: {n}", file=sys.stderr)
    if problems:
        print("quaddicted check FAILED (Map Packs > Get more maps, src/quaddicted.rs):", file=sys.stderr)
        for p in problems:
            print(f"  - {p}", file=sys.stderr)
        return 1
    os.makedirs(os.path.dirname(STAMP), exist_ok=True)
    open(STAMP, "w").close()
    print("quaddicted check: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
