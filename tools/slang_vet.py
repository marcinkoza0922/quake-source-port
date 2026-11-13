import os, re, sys, json
root = sys.argv[1]

def lic(text):
    t = re.sub(r'\s+', ' ', text.lower())
    if 'version 3' in t and 'gnu' in t and 'version 2' not in t: return 'GPL3'
    if 'gplv3' in t or 'gpl-3' in t or 'gpl 3' in t: return 'GPL3'
    if 'creative commons' in t or 'cc by' in t or 'cc-by' in t:
        return 'CC0' if 'cc0' in t else 'CC'
    if 'version 2' in t and ('gnu' in t or 'gpl' in t): return 'GPL2+' if 'any later' in t or 'or later' in t else 'GPL2'
    if 'gnu general public' in t or 'gpl' in t: return 'GPL?'
    if 'permission is hereby granted, free of charge' in t or 'mit license' in t: return 'MIT'
    if 'redistribution and use in source and binary' in t: return 'BSD'
    if 'public domain' in t or 'unlicense' in t or 'cc0' in t: return 'PD'
    if 'zlib' in t or 'altered source versions must be plainly marked' in t: return 'zlib'
    if 'copyright' in t or 'all rights reserved' in t: return 'COPYRIGHT'
    return None
OK = {'GPL2+','GPL?','MIT','BSD','PD','zlib','CC0','GPL2'}
cache = {}
def deps(path, seen):
    if path in seen: return
    seen.add(path)
    if not os.path.exists(path): seen.add('MISSING:'+path); return
    if path.endswith(('.png','.bmp','.jpg','.tga')): return
    txt = open(path, errors='ignore').read()
    for m in re.finditer(r'#include\s+"([^"]+)"', txt):
        deps(os.path.normpath(os.path.join(os.path.dirname(path), m.group(1))), seen)
    if path.endswith('.slangp'):
        for m in re.finditer(r'^\s*#reference\s+"?([^"\n]+)"?', txt, re.M):
            deps(os.path.normpath(os.path.join(os.path.dirname(path), m.group(1).strip())), seen)
        for m in re.finditer(r'^\s*(\w+)\s*=\s*"?([^"\n;]+\.(?:slang|png|bmp|jpg|tga|slangp))"?', txt, re.M|re.I):
            deps(os.path.normpath(os.path.join(os.path.dirname(path), m.group(2).strip())), seen)
res = {}
for dp, dn, fn in os.walk(root):
    for f in fn:
        if not f.endswith('.slangp'): continue
        p = os.path.join(dp, f); seen = set(); deps(p, seen)
        bad = []
        for s in sorted(seen):
            if s.startswith('MISSING:'): bad.append(s); continue
            if not os.path.exists(s): continue
            if s.endswith(('.slangp','.png','.bmp','.jpg','.tga')): continue
            l = lic(open(s, errors='ignore').read()[:6000])
            if l not in OK: bad.append(f'{os.path.relpath(s,root)}:{l}')
        res[os.path.relpath(p, root)] = bad
json.dump(res, open(sys.argv[2],'w'), indent=1)
good = [k for k,v in res.items() if not v]
print(len(res), 'presets;', len(good), 'clean')
from collections import Counter
print(Counter(b.split(':')[-1] for v in res.values() for b in v).most_common())
