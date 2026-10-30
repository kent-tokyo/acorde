#!/usr/bin/env python3
"""Compare short score-aligned PCM windows without claiming perceptual quality.

Usage: python3 scripts/compare_playback_audio.py ACORDE.wav ALPHATAB.f32le 0,500,1000
The alphaTab file is interleaved stereo float32 at the WAV sample rate.
"""

import array
import json
import math
import pathlib
import sys
import wave


def load_acorde(path):
    with wave.open(path, "rb") as source:
        if source.getsampwidth() != 2 or source.getnchannels() not in (1, 2):
            raise ValueError("Acorde WAV must be mono or stereo PCM16")
        rate = source.getframerate()
        channels = source.getnchannels()
        samples = array.array("h")
        samples.frombytes(source.readframes(source.getnframes()))
    if sys.byteorder != "little":
        samples.byteswap()
    mono = array.array("f")
    for i in range(0, len(samples), channels):
        mono.append(sum(samples[i : i + channels]) / (32768 * channels))
    return rate, mono


def load_alphatab(path):
    if pathlib.Path(path).stat().st_size % 8:
        raise ValueError("alphaTab data must be stereo float32 frames")
    samples = array.array("f")
    with open(path, "rb") as source:
        samples.frombytes(source.read())
    if sys.byteorder != "little":
        samples.byteswap()
    mono = array.array("f")
    nonfinite = 0
    first_nonfinite_frame = None
    for i in range(0, len(samples), 2):
        invalid = not math.isfinite(samples[i]) or not math.isfinite(samples[i + 1])
        if invalid:
            nonfinite += (not math.isfinite(samples[i])) + (not math.isfinite(samples[i + 1]))
            if first_nonfinite_frame is None:
                first_nonfinite_frame = i // 2
        mono.append((samples[i] + samples[i + 1]) * 0.5)
    return mono, nonfinite, first_nonfinite_frame


def rms(values):
    return math.sqrt(sum(value * value for value in values) / len(values)) if values else 0.0


def window_alignment(acorde, alphatab, start, size, max_shift=16):
    x = acorde[start : start + size]
    if len(x) != size:
        return None
    power_x = sum(value * value for value in x)
    if power_x == 0:
        return None
    best = None
    for shift in range(-max_shift, max_shift + 1):
        at = start + shift
        if at < 0:
            continue
        y = alphatab[at : at + size]
        if len(y) != size or not all(map(math.isfinite, y)):
            continue
        power_y = sum(value * value for value in y)
        if power_y == 0:
            continue
        cosine = sum(a * b for a, b in zip(x, y)) / math.sqrt(power_x * power_y)
        if best is None or cosine > best["cosine"]:
            best = {"alpha_array_shift_frames": shift, "cosine": cosine}
    if best:
        best["cosine"] = round(best["cosine"], 6)
    return best


def main():
    if len(sys.argv) != 4:
        raise SystemExit(__doc__)
    rate, acorde = load_acorde(sys.argv[1])
    alphatab, nonfinite, first_nonfinite_frame = load_alphatab(sys.argv[2])
    onsets = [float(value) for value in sys.argv[3].split(",")]
    if not onsets or any(not math.isfinite(value) or value < 0 for value in onsets):
        raise ValueError("onsets must be nonnegative finite milliseconds")
    size = round(rate * 0.1)
    windows = []
    for onset in onsets:
        start = round((onset + 100) * rate / 1000)
        windows.append({"onset_ms": onset,
                        "alignment": window_alignment(acorde, alphatab, start, size)})
    print(json.dumps({
        "sample_rate": rate,
        "acorde_frames": len(acorde),
        "alphatab_frames": len(alphatab),
        "acorde_peak": round(max(map(abs, acorde), default=0), 6),
        "alphatab_peak": None if nonfinite else round(max(map(abs, alphatab), default=0), 6),
        "acorde_rms": round(rms(acorde), 6),
        "alphatab_rms": None if nonfinite else round(rms(alphatab), 6),
        "alphatab_nonfinite_samples": nonfinite,
        "alphatab_first_nonfinite_frame": first_nonfinite_frame,
        "windows": windows,
    }, indent=2))


if __name__ == "__main__":
    main()
