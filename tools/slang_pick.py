import json, os, re, sys, shutil
sys.path.insert(0, os.path.dirname(__file__))
root, out = sys.argv[1], sys.argv[2]
r = json.load(open(sys.argv[3]))
keep_dirs = ('crt/', 'scanlines/', 'ntsc/', 'pal/', 'gpu/3dfx_4x1', 'gpu/powervr2', 'sharpen/', 'cel/', 'deblur/',
  'edge-smoothing/fsr/', 'edge-smoothing/nis/', 'anti-aliasing/advanced-aa', 'blurs/gaussian_blur.slangp',
  'dithering/bayer_4x4', 'dithering/blue_noise.slangp', 'misc/color-mangler', 'misc/image-adjustment', 'misc/grade-no-LUT',
  'misc/deband', 'motionblur/motionblur-simple', 'misc/relief', 'denoisers/fast-bilateral')
drop = ('crt/shaders/', '-tate', 'newpixie-crt', 'pal-r57shell', 'guest-advanced-fastest')
sel = [k for k, v in sorted(r.items()) if not v and k.startswith(keep_dirs) and not any(d in k for d in drop)]
src=open(os.path.join(os.path.dirname(__file__), 'slang_vet.py')).read()
exec(src[src.index('def lic'):src.index('res = {}')], globals())
lic_of=lambda f: lic(open(f, errors='ignore').read()[:6000])
files = set()
for k in sel:
    s = set(); deps(os.path.join(root, k), s); files |= {f for f in s if os.path.exists(f)}
for f in files:
    d = os.path.join(out, os.path.relpath(f, root)); os.makedirs(os.path.dirname(d), exist_ok=True); shutil.copy(f, d)
lics = {}
for f in sorted(files):
    if f.endswith(('.slang','.inc','.h','.glsl')):
        lics[os.path.relpath(f, root)] = lic_of(f)
json.dump({'presets': sel, 'licenses': lics}, open(os.path.join(out, 'manifest.json'), 'w'), indent=1)
print(len(sel), 'presets', len(files), 'files')
