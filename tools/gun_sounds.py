#!/usr/bin/env python3
"""Cut and process the add-on guns' sounds from CC0 recordings (offline).

    python3 tools/gun_sounds.py SRC_DIR [--preview DIR]

SRC_DIR holds the downloads listed in assets/gun-sounds/README.md. Every cue
is one or more cuts of a recording (one channel, so the two microphones of a
stereo take don't comb), equalised, compressed and saturated towards stock
Quake weapon sounds (measured on id1's shotgn2, sgun1, spike2, rocket1i: full
scale peaks, RMS 0.5-0.65 over the first 0.1 s, energy mostly under 2 kHz),
then written as 22050 Hz 16 bit mono WAVs to assets/gun-sounds/.
Needs numpy, scipy and ffmpeg.
"""
import argparse
from pathlib import Path
import subprocess
import wave

import numpy as np
from scipy.ndimage import gaussian_filter1d
from scipy.signal import butter, resample_poly, sosfilt

ROOT = Path(__file__).resolve().parent.parent
SR, OUT = 44100, 22050
LIB = 'Prepared SFX Library/'
P_MID, P_NEAR = LIB + '1911/A_34P.wav', LIB + '1911/A_42P.wav'
R_MID, R_NEAR = LIB + 'AR-15/D_24P.wav', LIB + 'AR-15/D_32P.wav'
HAND, AIR, RIFLE = 'reload.wav', 'gunreload1.wav', 'assaultriflereload1_0.wav'


def load(path, channel):
    raw = subprocess.run(['ffmpeg', '-v', 'error', '-i', str(path), '-f', 'f32le', '-ac', '2', '-ar', str(SR), '-'],
                         capture_output=True, check=True).stdout
    return np.frombuffer(raw, '<f4').reshape(-1, 2)[:, channel].astype(np.float64)


def filt(x, kind, hz, order=2):
    return sosfilt(butter(order, hz, kind, fs=SR, output='sos'), x)


def env(x, attack, release):
    """Peak envelope follower (seconds)."""
    a, r = np.exp(-1 / (attack * SR)), np.exp(-1 / (release * SR))
    out, e = np.empty(len(x)), 0.0
    for i, v in enumerate(np.abs(x)):
        k = a if v > e else r
        e = k * e + (1 - k) * v
        out[i] = e
    return out


def compress(x, threshold, ratio, attack=0.0008, release=0.09):
    e = np.maximum(env(x, attack, release), 1e-6)
    gain = np.where(e > threshold, (e / threshold) ** (1 / ratio - 1), 1.0)
    return x * gain


def onset(x, t):
    """The sample the shot near `t` seconds starts at."""
    a = int((t - 0.08) * SR)
    y = np.abs(x[a:a + int(0.3 * SR)])
    return a + int(np.argmax(y > 0.15 * y.max()))


def shot(near, mid, length, body, bright, pitch, hold=0.05, decay=9.0, drive=0.95):
    """A shot: the close take for its crack, the distant take's echo for a
    tail, squashed together."""
    (nx, nt), (mx, mt) = near, mid
    n = int(length * SR)
    pre = int(0.0015 * SR)
    a, b = onset(nx, nt) - pre, onset(mx, mt) - pre
    close, far = nx[a:a + n].copy(), mx[b:b + n].copy()
    close, far = (v / np.abs(v).max() for v in (close, far))
    x = filt(close, 'high', 35) + 0.8 * filt(far, 'high', 60) * np.minimum(np.arange(n) / (0.02 * SR), 1.0)
    # even out the spike and the body, then give them a stock weapon's decay
    t = np.arange(n) / SR
    level = np.sqrt(gaussian_filter1d(x * x, 0.005 * SR))
    shape = np.where(t < hold, 1.0, np.exp(-(t - hold) * decay))
    x = x / np.maximum(level, 0.003 * level.max()) * shape
    # a chest thump under the crack and a duller top, as in 1996
    x = x + body * filt(x, 'low', 170) + bright * filt(x, 'high', 2500, 1)
    x = filt(x, 'low', 4200)
    # saturate against the body's level, not the spike's
    x = np.tanh(x / np.sqrt((x[:int(0.1 * SR)] ** 2).mean()) * drive)
    x = x * 0.99 / np.abs(x).max()
    if pitch != 1.0:
        x = resample_poly(x, 1000, int(round(1000 * pitch)))
    return x


def work(x, start, end, lead=0.0, level=0.8, low=4500, thick=0.8):
    """Handling noise: a cut with `lead` seconds of silence before it."""
    y = x[int(start * SR):int(end * SR)].copy()
    fade = int(0.004 * SR)
    y[:fade] *= np.linspace(0, 1, fade)
    y = filt(y, 'high', 70)
    y = y + thick * filt(y, 'low', 500)
    y = filt(y, 'low', low, 4)
    y /= np.abs(y).max()
    y = compress(y, 0.05, 3.0, 0.0006, 0.04)
    y = np.tanh(y / np.sort(np.abs(y))[int(len(y) * 0.985)] * 1.1)
    return np.concatenate([np.zeros(int(lead * SR)), y * level / np.abs(y).max()])


def save(path, x):
    x = resample_poly(x, OUT, SR)
    tail = int(0.03 * OUT)
    x[-tail:] *= np.linspace(1, 0, tail) ** 2
    x[:8] *= np.linspace(0, 1, 8)
    peak = np.abs(x).max()
    if peak > 0.99:
        x = x * 0.99 / peak
    with wave.open(str(path), 'wb') as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(OUT)
        w.writeframes((x * 32767).round().astype('<i2').tobytes())
    return x


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('src', type=Path)
    ap.add_argument('--out', type=Path, default=ROOT / 'assets/gun-sounds')
    o = ap.parse_args()
    find = lambda name: next(p for p in o.src.rglob(Path(name).name) if str(p).endswith(name))
    cache = {}

    def src(name, channel=0):
        if (name, channel) not in cache:
            cache[name, channel] = load(find(name), channel)
        return cache[name, channel]

    cues = {
        # two takes per pistol side, so a pair never sounds like one gun
        'pistol_shot_l': shot((src(P_NEAR), 0.94), (src(P_MID), 1.54), 0.62, 0.9, 0.25, 1.0),
        'pistol_shot_r': shot((src(P_NEAR), 5.0), (src(P_MID), 6.65), 0.62, 0.9, 0.25, 1.05),
        'pistol_dry': work(src(HAND), 0.652, 0.70, level=0.7),
        'pistol_magout': work(src(AIR), 0.055, 0.30, level=0.65),
        'pistol_magin': work(src(HAND), 0.573, 0.79, level=0.8),
        'pistol_slide': work(src(HAND), 1.03, 1.37, level=0.8, low=3800, thick=1.5),
        # alternating takes keep full-auto fire from sounding looped
        'rifle_shot_a': shot((src(R_NEAR), 0.7), (src(R_MID), 0.54), 0.5, 0.7, 0.35, 1.0, decay=11.0),
        'rifle_shot_b': shot((src(R_NEAR), 5.64), (src(R_MID), 3.91), 0.5, 0.7, 0.35, 1.02, decay=11.0),
        'rifle_shot_c': shot((src(R_NEAR, 1), 0.7), (src(R_MID), 3.91), 0.5, 0.7, 0.35, 0.98, decay=11.0),
        'rifle_dry': work(src(RIFLE), 1.055, 1.11, level=0.7),
        'rifle_magout': work(src(RIFLE), 0.14, 0.37, level=0.65),
        'rifle_magin': work(src(RIFLE), 1.035, 1.185, lead=0.085, level=0.8),
        'rifle_bolt': work(src(RIFLE), 1.19, 1.50, lead=0.09, level=0.8, low=4000),
    }
    o.out.mkdir(parents=True, exist_ok=True)
    for name, x in cues.items():
        x = save(o.out / f'{name}.wav', x)
        n = len(x[:OUT // 10])
        s = np.abs(np.fft.rfft(x)) ** 2
        f = np.fft.rfftfreq(len(x), 1 / OUT)
        roll = f[np.searchsorted(np.cumsum(s) / s.sum(), 0.95)]
        print(f'{name:14} {len(x) / OUT:.2f}s peak {np.abs(x).max():.2f} rms100 {np.sqrt((x[:n] ** 2).mean()):.3f} '
              f'rms {np.sqrt((x ** 2).mean()):.3f} centroid {(s * f).sum() / s.sum():.0f} roll95 {roll:.0f}')


main()
