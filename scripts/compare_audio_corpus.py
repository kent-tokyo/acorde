#!/usr/bin/env python3
"""Measure Acorde and alphaTab PCM against the same score schedule (Issue #203).

Usage:
  python3 scripts/compare_audio_corpus.py OUT.json NAME:SCHEDULE.json:ACORDE.wav:ALPHATAB.f32le[:REPORT.json] ...

SCHEDULE.json is `acorde playback-report SCORE`; ACORDE.wav is PCM16 from
`acorde audio-render --full-sf2`; ALPHATAB.f32le is interleaved stereo float32
from `scripts/export_alphatab_audio.mjs` at the same sample rate. The optional
REPORT.json is `acorde report SCORE`; its import diagnostics are recorded
separately so unsupported input is not mistaken for a synthesis failure.

One gain policy for both renderers: level figures (peak, full-scale and
non-finite samples) use the raw output; timing figures use each signal
normalized to its own peak, so gain does not affect them.

Onsets are measured, not assumed. Around each distinct scheduled onset, the
strongest 5 ms log-energy rise of at least 3 dB is the heard attack. The search
window is [-80, +250] ms, clamped to halfway to the neighbouring onsets so a
later attack is not mistaken for this one. Both renderers share the SF2, so the
attack shape of a patch biases both alike: `paired_*` compares alphaTab minus
Acorde per onset where both were detected, which cancels that bias.

Releases: for each scheduled silence of at least 300 ms (no note sounding), the
level 250 ms into it; and at the end, buffer length beyond the scheduled end,
the residual level at the buffer end, and the time to fall 60 dB below peak.

These are screening metrics. They do not replace listening, and a better number
on this corpus is not a general quality claim.
"""

import collections
import json
import math
import pathlib
import statistics
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from compare_playback_audio import load_acorde, load_alphatab, sha256  # noqa: E402

HOP = 0.005
WINDOW = (-0.080, 0.250)
MIN_RISE_DB = 3.0
GAP = 0.300


def envelope_db(mono, rate):
    hop = max(1, int(rate * HOP))
    peak = max((abs(v) for v in mono), default=0.0) or 1.0
    env = []
    for i in range(0, len(mono), hop):
        block = mono[i : i + hop]
        mean = sum((v / peak) ** 2 for v in block) / len(block)
        env.append(10 * math.log10(max(mean, 1e-18)))
    return env


def level_at(env, seconds):
    i = int(seconds / HOP)
    return round(env[i], 1) if 0 <= i < len(env) else None


def attacks(env, onsets):
    found = []
    for k, onset in enumerate(onsets):
        lo_t = onset + WINDOW[0]
        hi_t = onset + WINDOW[1]
        if k:
            lo_t = max(lo_t, (onsets[k - 1] + onset) / 2)
        if k + 1 < len(onsets):
            hi_t = min(hi_t, (onset + onsets[k + 1]) / 2)
        best, best_rise = None, MIN_RISE_DB
        for i in range(max(1, int(lo_t / HOP)), min(len(env) - 1, int(hi_t / HOP)) + 1):
            rise = env[i] - env[i - 1]
            if rise > best_rise:
                best, best_rise = i, rise
        found.append(None if best is None else (best * HOP - onset) * 1000)
    return found


def summary(values):
    absolute = sorted(abs(v) for v in values)
    if not values:
        return {"median_ms": None, "median_abs_ms": None, "p95_abs_ms": None}
    return {
        "median_ms": round(statistics.median(values), 1),
        "median_abs_ms": round(statistics.median(absolute), 1),
        "p95_abs_ms": round(absolute[math.ceil(0.95 * len(absolute)) - 1], 1),
    }


def features(events):
    by_part = {}
    for e in events:
        by_part.setdefault(e["part_index"], []).append(e)
    overlapping = any(
        a["source_voice_number"] != b["source_voice_number"]
        and a["time_secs"] < b["time_secs"] + b["duration_secs"]
        and b["time_secs"] < a["time_secs"] + a["duration_secs"]
        for part in by_part.values()
        for i, a in enumerate(part)
        for b in part[i + 1 : i + 40]
    )
    ratios = {round(e["time_secs"] / e["time_beats"], 4) for e in events if e["time_beats"] > 0}
    return {
        "parts": len(by_part),
        "overlapping_voices": overlapping,
        "tempo_changes": len(ratios) > 1,
        "dynamics": len({e["velocity"] for e in events}) > 1,
        "pedal": any(e["pedal"] for e in events),
        "articulations": sorted({a for e in events for a in e["articulations"]}),
        "programs": sorted({e["program"] for e in events}),
    }


def silences(events, end):
    spans = sorted((e["time_secs"], e["time_secs"] + e["duration_secs"]) for e in events)
    gaps, sounding_until = [], 0.0
    for start, stop in spans:
        if start - sounding_until >= GAP:
            gaps.append(sounding_until)
        sounding_until = max(sounding_until, stop)
    return [g for g in gaps if 0 < g < end]


def measure(mono, rate, onsets, gaps, end):
    env = envelope_db(mono, rate)
    found = attacks(env, onsets)
    detected = [v for v in found if v is not None]
    gap_levels = [level_at(env, g + 0.25) for g in gaps]
    gap_levels = [v for v in gap_levels if v is not None]
    seconds = len(mono) / rate
    to_minus60 = next(
        (round(i * HOP - end, 3) for i in range(int(end / HOP), len(env)) if env[i] < -60),
        None,
    )
    return found, {
        "seconds": round(seconds, 3),
        "detected_onsets": len(detected),
        "undetected_onsets": len(found) - len(detected),
        **{f"onset_{k}": v for k, v in summary(detected).items()},
        "median_level_250ms_into_silence_db": round(statistics.median(gap_levels), 1)
        if gap_levels
        else None,
        "buffer_beyond_scheduled_end_seconds": round(seconds - end, 3),
        "level_at_buffer_end_db": round(env[-1], 1) if env else None,
        "tail_to_minus60db_seconds": to_minus60,
    }


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    results = []
    for spec in sys.argv[2:]:
        name, schedule_path, acorde_path, alphatab_path, *report = spec.split(":")
        events = [
            e
            for e in json.loads(pathlib.Path(schedule_path).read_text())
            if not e.get("is_metronome")
        ]
        onsets = sorted({round(e["time_secs"], 4) for e in events})
        end = max(e["time_secs"] + e["duration_secs"] for e in events)
        gaps = silences(events, end)
        rate, acorde, _, acorde_peak, acorde_rail = load_acorde(acorde_path)
        alphatab, nonfinite, _, alphatab_peak, over_unit = load_alphatab(alphatab_path)
        row = {
            "score": name,
            "schedule_sha256": sha256(schedule_path),
            "scheduled_events": len(events),
            "scheduled_onsets": len(onsets),
            "scheduled_silences": len(gaps),
            "scheduled_end_seconds": round(end, 3),
            "sample_rate": rate,
            "features": features(events),
        }
        if report:
            codes = collections.Counter(
                d["code"] for d in json.loads(pathlib.Path(report[0]).read_text())["diagnostics"]
            )
            row["import_diagnostics"] = {"total": sum(codes.values()), "by_code": dict(sorted(codes.items()))}
        acorde_found, row["acorde"] = measure(acorde, rate, onsets, gaps, end)
        alphatab_found, row["alphatab"] = measure(alphatab, rate, onsets, gaps, end)
        row["acorde"].update(sha256=sha256(acorde_path), peak=round(acorde_peak, 6),
                             full_scale_samples=acorde_rail, nonfinite_samples=0)
        row["alphatab"].update(sha256=sha256(alphatab_path), peak=round(alphatab_peak, 6),
                               full_scale_samples=over_unit, nonfinite_samples=nonfinite)
        paired = [t - a for a, t in zip(acorde_found, alphatab_found) if a is not None and t is not None]
        row["paired_alphatab_minus_acorde"] = {"onsets": len(paired), **summary(paired)}
        results.append(row)
    pathlib.Path(sys.argv[1]).write_text(json.dumps(results, indent=2) + "\n")
    for row in results:
        a, t, p = row["acorde"], row["alphatab"], row["paired_alphatab_minus_acorde"]
        print(
            f"{row['score']}: onsets {row['scheduled_onsets']} silences {row['scheduled_silences']}\n"
            f"  Acorde   abs med/p95 {a['onset_median_abs_ms']}/{a['onset_p95_abs_ms']} signed med {a['onset_median_ms']} "
            f"missed {a['undetected_onsets']} silence {a['median_level_250ms_into_silence_db']} dB "
            f"beyond-end {a['buffer_beyond_scheduled_end_seconds']} s end {a['level_at_buffer_end_db']} dB "
            f"peak {a['peak']} fullscale {a['full_scale_samples']}\n"
            f"  alphaTab abs med/p95 {t['onset_median_abs_ms']}/{t['onset_p95_abs_ms']} signed med {t['onset_median_ms']} "
            f"missed {t['undetected_onsets']} silence {t['median_level_250ms_into_silence_db']} dB "
            f"beyond-end {t['buffer_beyond_scheduled_end_seconds']} s end {t['level_at_buffer_end_db']} dB "
            f"peak {t['peak']} fullscale {t['full_scale_samples']} nonfinite {t['nonfinite_samples']}\n"
            f"  paired alphaTab-Acorde n={p['onsets']} med {p['median_ms']} abs med/p95 {p['median_abs_ms']}/{p['p95_abs_ms']} ms\n"
            f"  features {row['features']} import diagnostics {row.get('import_diagnostics', {}).get('total')}"
        )


if __name__ == "__main__":
    main()
