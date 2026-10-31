#!/usr/bin/env python3
"""Measure local PCM alignment for a same-score, same-SF2 export pair.

Requires NumPy and SciPy. This reports relative waveform lag, not which
renderer follows the score; compare the result with an independent onset oracle.
"""

import argparse
import hashlib
import json
import math
import wave
from pathlib import Path

import numpy as np
from scipy.signal import correlate


def file_sha256(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def load_acorde(path):
    with wave.open(str(path), "rb") as source:
        if source.getsampwidth() != 2 or source.getnchannels() not in (1, 2):
            raise ValueError("Acorde WAV must be mono or stereo PCM16")
        rate = source.getframerate()
        channels = source.getnchannels()
        frames = source.getnframes()
        samples = np.frombuffer(source.readframes(frames), dtype="<i2")
    if len(samples) != frames * channels:
        raise ValueError("truncated Acorde WAV")
    return rate, samples.reshape(-1, channels).astype(np.float64).mean(axis=1) / 32768


def load_alphatab(path):
    if path.stat().st_size % 8:
        raise ValueError("alphaTab export must contain stereo float32 frames")
    samples = np.memmap(path, dtype="<f4", mode="r").reshape(-1, 2)
    if not np.isfinite(samples).all():
        raise ValueError("alphaTab export contains non-finite samples")
    return samples.astype(np.float64).mean(axis=1)


def best_lag(acorde, alphatab, rate, start_seconds, window_seconds, search_seconds):
    start = round(start_seconds * rate)
    size = round(window_seconds * rate)
    radius = round(search_seconds * rate)
    first = max(0, start - radius)
    last = min(len(alphatab), start + size + radius)
    reference = acorde[start : start + size]
    candidates = alphatab[first:last]
    if len(reference) != size or len(candidates) < size:
        raise ValueError(f"window at {start_seconds} s exceeds an audio file")
    reference_power = float(np.dot(reference, reference))
    if reference_power <= 1e-12:
        raise ValueError(f"window at {start_seconds} s has no Acorde signal")
    products = correlate(candidates, reference, mode="valid", method="fft")
    energy = np.concatenate(([0.0], np.cumsum(candidates * candidates)))
    candidate_power = energy[size:] - energy[:-size]
    similarity = products / np.sqrt(np.maximum(candidate_power * reference_power, 1e-24))
    index = int(np.argmax(similarity))
    return {
        "start_seconds": start_seconds,
        "lag_ms": round((first + index - start) * 1000 / rate, 3),
        "cosine": round(float(similarity[index]), 6),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("acorde_wav", type=Path)
    parser.add_argument("alphatab_f32le", type=Path)
    parser.add_argument("--sample-rate-hz", type=int, required=True,
                        help="sample rate used for the headerless alphaTab export")
    parser.add_argument("--windows", required=True, help="comma-separated score seconds")
    parser.add_argument("--window-ms", type=float, default=500)
    parser.add_argument("--search-ms", type=float, default=1000)
    args = parser.parse_args()
    starts = [float(value) for value in args.windows.split(",")]
    if not starts or any(not math.isfinite(value) or value < 0 for value in starts):
        parser.error("windows must be nonnegative finite seconds")
    if not 0 < args.window_ms <= 5000 or not 0 < args.search_ms <= 5000:
        parser.error("window and search sizes must be within 0–5000 ms")
    rate, acorde = load_acorde(args.acorde_wav)
    if rate != args.sample_rate_hz:
        parser.error("alphaTab sample rate must equal the Acorde WAV sample rate")
    alphatab = load_alphatab(args.alphatab_f32le)
    windows = [
        best_lag(acorde, alphatab, rate, start, args.window_ms / 1000, args.search_ms / 1000)
        for start in starts
    ]
    print(json.dumps({
        "sample_rate_hz": rate,
        "acorde_frames": len(acorde),
        "alphatab_frames": len(alphatab),
        "acorde_sha256": file_sha256(args.acorde_wav),
        "alphatab_sha256": file_sha256(args.alphatab_f32le),
        "window_ms": args.window_ms,
        "search_ms": args.search_ms,
        "windows": windows,
    }, indent=2))


if __name__ == "__main__":
    main()
