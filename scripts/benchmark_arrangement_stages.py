#!/usr/bin/env python3
"""Measure unchanged legacy generation and stage-4/5 APIs on matched native builds.

Requires existing clean baseline and candidate source checkouts and cached Rust
dependencies. Never edits either checkout. Writes builds, logs and a report only
to a new output directory; the caller may then pin report.json in docs.
"""
import argparse
import datetime
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import time


def command(args, cwd=None, env=None):
    return subprocess.check_output(args, cwd=cwd, env=env, text=True).strip()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--build-cache", type=Path, help="Optional shared Cargo target directory")
    parser.add_argument("--toolchain", default="1.100.0")
    parser.add_argument("--warmups", type=int, default=5)
    parser.add_argument("--samples", type=int, default=30)
    args = parser.parse_args()
    if not 1 <= args.warmups <= 100 or not 1 <= args.samples <= 1000:
        parser.error("warmups must be 1..100 and samples 1..1000")
    roots = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    harness = Path(__file__).with_name("arrangement_stage45_benchmark.rs").resolve()
    fixture = "tests/fixtures/arrangement/form_development_v2.rs"
    if (roots["baseline"] / fixture).read_bytes() != (roots["candidate"] / fixture).read_bytes():
        raise RuntimeError("legacy fixture differs between revisions")
    revisions = {}
    for label, root in roots.items():
        changed = command(["git", "status", "--porcelain", "--untracked-files=all"], cwd=root)
        # Harness/receipt edits do not affect the measured library. Refuse any
        # tracked or untracked product-code changes and any dirty baseline.
        if (label == "baseline" and changed) or any(
            line[3:].startswith(("crates/", "tests/"))
            or line[3:] in {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml"}
            for line in changed.splitlines()
        ):
            raise RuntimeError(f"uncommitted benchmark input changes in {label}: {changed}")
        revisions[label] = {
            "head": command(["git", "rev-parse", "HEAD"], cwd=root),
            "core_tree": command(["git", "rev-parse", "HEAD:crates/core"], cwd=root),
            "checkout_status": changed.splitlines(),
        }
    env = os.environ.copy()
    target = args.build_cache.resolve() if args.build_cache else output / "target"
    env.update(CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="2",
               CARGO_TARGET_DIR=str(target), CARGO_PROFILE_RELEASE_DEBUG="0")
    # Do not inherit caller compiler flags that would invalidate a matched run.
    for key in ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER"]:
        env.pop(key, None)
    binaries = {}
    lock_hashes = {}
    for label, root in roots.items():
        project = output / label
        project.mkdir()
        modules = [
            '// Both original audition fixture graphs are intentional; the rhythm fixture privately embeds harmony.',
            '#![allow(clippy::duplicate_mod)]',
            f'#[allow(dead_code)]\n#[path = {json.dumps(str(root / fixture), ensure_ascii=False)}]\nmod legacy;',
        ]
        if label == "candidate":
            for module, name in [("harmony", "harmony_targets_listening.rs"),
                                 ("rhythm", "rhythm_section_listening.rs")]:
                path = root / "tests/fixtures/arrangement" / name
                modules.append(f'#[allow(dead_code)]\n#[path = {json.dumps(str(path), ensure_ascii=False)}]\nmod {module};')
        modules.extend([
            f'#[path = {json.dumps(str(harness), ensure_ascii=False)}]\nmod benchmark;',
            'fn main() -> Result<(), Box<dyn std::error::Error>> { benchmark::run() }',
        ])
        (project / "main.rs").write_text("\n".join(modules) + "\n")
        (project / "Cargo.toml").write_text(
            '[package]\nname = "acorde-stage45-benchmark"\nversion = "0.0.0"\nedition = "2024"\n'
            '[workspace]\n[[bin]]\nname = "acorde-stage45-benchmark"\npath = "main.rs"\n'
            '[features]\nstages45 = []\n[dependencies]\n'
            f'acorde-core = {{ path = {json.dumps(str(root / "crates/core"), ensure_ascii=False)} }}\n'
            'serde = { version = "1", features = ["derive"] }\nserde_json = "1"\n'
        )
        shutil.copyfile(roots["candidate"] / "Cargo.lock", project / "Cargo.lock")
        build = ["cargo", f"+{args.toolchain}", "build", "--offline", "--release"]
        if label == "candidate":
            build += ["--features", "stages45"]
        print(f"Building {label} {revisions[label]['head'][:8]}", flush=True)
        with (output / f"{label}-build.log").open("w") as log:
            subprocess.run(build, cwd=project, env=env, stdout=log, stderr=log, check=True)
        lock_hashes[label] = digest((project / "Cargo.lock").read_bytes())
        suffix = ".exe" if os.name == "nt" else ""
        binary = output / f"{label}-benchmark{suffix}"
        shutil.copyfile(target / "release" / f"acorde-stage45-benchmark{suffix}", binary)
        binary.chmod(0o755)
        binaries[label] = binary
    if lock_hashes["baseline"] != lock_hashes["candidate"]:
        raise RuntimeError("dependency lockfiles differ; refusing an unmatched comparison")
    started = datetime.datetime.now(datetime.timezone.utc).isoformat()
    load_before = os.getloadavg() if hasattr(os, "getloadavg") else None
    runs = []
    # Balanced order limits simple warm-machine/order bias without claiming
    # statistical independence of calls from the same host/process.
    for label in ["baseline", "candidate", "candidate", "baseline"]:
        mode = "all" if label == "candidate" else "legacy"
        print(f"Measuring {label} ({mode})", flush=True)
        start = time.monotonic()
        raw = command([str(binaries[label]), str(args.warmups), str(args.samples), mode])
        path = output / f"run-{len(runs) + 1}-{label}.json"
        path.write_text(raw + "\n")
        runs.append({"backend": label, "wall_seconds": time.monotonic() - start,
                     "raw_sha256": digest(path.read_bytes()), "data": json.loads(raw)})
    grouped = {}
    parity_sources = {}
    parity_cases = {}
    for run in runs:
        for case in run["data"]["cases"]:
            key = (run["backend"], case["id"])
            observed = case["observed"]
            record = {
                "backend": key[0], "id": key[1], "input_sha256": digest(canonical(case["input"])),
                "playback_sha256": digest(canonical(observed["playback"])), "work": observed["work"],
            }
            if key[0] == "candidate" and key[1] != "legacy-form-sixteen":
                source_key = digest(canonical(case["input"]["score"]))
                parity_sources[source_key] = case["input"]["score"]
                parity = {"id":key[1], "source":source_key,
                          "request":case["input"]["request"],
                          "playback":observed["playback"], "contracts":observed["contracts"]}
                if key[1] in parity_cases and parity_cases[key[1]] != parity:
                    raise RuntimeError(f"native parity fixture drift: {key[1]}")
                parity_cases[key[1]] = parity
            if key not in grouped:
                grouped[key] = {**record, "samples_ns": []}
            elif any(grouped[key][k] != v for k, v in record.items()):
                raise RuntimeError(f"input/output/counter drift across runs: {key}")
            grouped[key]["samples_ns"].extend(case["sample_ns"])
    for record in grouped.values():
        samples = sorted(record["samples_ns"])
        record["median_ms"] = statistics.median(samples) / 1e6
        record["p95_ms"] = samples[math.ceil(len(samples) * 0.95) - 1] / 1e6
        record["max_ms"] = samples[-1] / 1e6
    old = grouped[("baseline", "legacy-form-sixteen")]
    new = grouped[("candidate", "legacy-form-sixteen")]
    for key in ["input_sha256", "playback_sha256", "work"]:
        if old[key] != new[key]:
            raise RuntimeError(f"legacy comparison changed {key}")
    report = {
        "schema_version": 1, "started_utc": started,
        "scope": "Native release API-call generation cost; excludes setup, serialization, playback verification, rendering and output destruction. Fixed development inputs, not a browser latency or general performance claim.",
        "baseline_scope": "Stage-0 timing was not originally recorded. Rebuilt its pinned revision beside the candidate with the same harness, legacy input, dependencies and toolchain. Stage 4/5 APIs did not exist in stage 0; their absolute costs are reported separately, without a cross-API speed ratio.",
        "environment": {"platform": platform.platform(), "machine": platform.machine(),
            "rustc": command(["rustc", f"+{args.toolchain}", "-Vv"]),
            "profile": "release; incremental=0; debug=0; build_jobs=2; no custom RUSTFLAGS",
            "load_before": load_before, "load_after": os.getloadavg() if hasattr(os, "getloadavg") else None},
        "revisions": revisions, "dependency_lock_sha256": lock_hashes["baseline"],
        "harness_sha256": digest(harness.read_bytes()), "runner_sha256": digest(Path(__file__).read_bytes()),
        "fixture_sha256": {name: digest((roots["candidate"] / "tests/fixtures/arrangement" / name).read_bytes())
            for name in ["form_development_v2.rs", "phrase_harmony_targets.rs", "harmony_targets_listening.rs",
                         "harmony-targets-listening.json", "rhythm_section.rs", "rhythm_section_listening.rs",
                         "rhythm-section-listening.json"]},
        "protocol": {"order": [r["backend"] for r in runs], "warmups_per_case_per_run": args.warmups,
            "samples_per_case_per_run": args.samples, "p95": "nearest rank; pooled two runs",
            "validation": "Each warmup and measured call must preserve source parts and match playback/work counters. Baseline/candidate legacy input, playback and counters must match."},
        "legacy_comparison": {"same_input_and_playback": True,
            "median_ratio_candidate_over_baseline": new["median_ms"] / old["median_ms"],
            "p95_ratio_candidate_over_baseline": new["p95_ms"] / old["p95_ms"]},
        "runs": [{k: v for k, v in r.items() if k != "data"} for r in runs],
        "cases": list(grouped.values()),
        "limitations": ["One native host and fixed development scores; no universal latency guarantee.",
            "Selected-candidate voicing-state counts omit rejected candidate internals; realization counts cover all attempted plans.",
            "Rhythm generation is deterministic construction and has no candidate-search counter.",
            "No historical timing or listening acceptance is inferred from these measurements."],
    }
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    parity = {"schema_version":1, "native_commit":revisions["candidate"]["head"],
              "core_tree":revisions["candidate"]["core_tree"],
              "harness_sha256":report["harness_sha256"], "runner_sha256":report["runner_sha256"],
              "rustc":report["environment"]["rustc"], "sources":parity_sources,
              "cases":list(parity_cases.values())}
    (output / "native-parity.json").write_bytes(canonical(parity) + b"\n")
    print(json.dumps({"report": str(output / "report.json"), "cases": len(grouped),
                      "legacy_comparison": report["legacy_comparison"]}), flush=True)


if __name__ == "__main__":
    main()
