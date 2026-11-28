#!/usr/bin/env python3
"""Benchmark a tagged Acorde release against a pinned Verovio on the same MusicXML.

Builds the release WASM package from a git tag in a temporary worktree,
installs the pinned Verovio npm package, and runs scripts/bench_print_parity.mjs
in one isolated Node process per engine and input. Engine order alternates by
input. The JSON report records the tag, commit, toolchain, CPU label, artifact
hashes and per-run samples (median, p95, peak RSS) (#500).

    python3 -B scripts/bench_release_print.py --tag v2.0.1 --cpu-label "Apple M4" \
      --output docs/benchmarks/print-release-v2.0.1-<host>.json

Requirements: git, cargo, wasm-pack 0.15.0, the wasm32-unknown-unknown target,
Node.js and npm. wasm-pack uses a `wasm-opt` on PATH when one exists, otherwise it
downloads binaryen; the report records the version used. Timings are host-dependent. Compare only runs from the same
host, CPU label and settings. A result is not a quality comparison.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
DEFAULT_INPUTS = (
    "aloha=tests/fixtures/openscore_lieder_aloha_oe.mxl",
    "just-for-today=tests/fixtures/openscore_lieder_just_for_today.mxl",
    "quilter=tests/fixtures/openscore_lieder_quilter_prelude.mxl",
    "webern=tests/fixtures/openscore_lieder_webern_so_ich_traurig_bin.mxl",
)
FONT = "crates/pdf/tests/fixtures/Tuffy.ttf"
# Matches the comparison protocol: A4 and at most four measures per system.
CONFIG = {"measures_per_system": 4}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(command: list[str], **kwargs) -> str:
    result = subprocess.run(command, check=True, text=True, capture_output=True, **kwargs)
    return result.stdout


def tool_version(command: list[str]) -> str:
    try:
        return run(command).strip().splitlines()[0]
    except (OSError, subprocess.CalledProcessError):
        return "unavailable"


def extract_musicxml(source: Path, destination: Path) -> None:
    """Write plain MusicXML; .mxl archives are unpacked through META-INF/container.xml."""
    if source.suffix.lower() != ".mxl":
        shutil.copyfile(source, destination)
        return
    with zipfile.ZipFile(source) as archive:
        container = ET.fromstring(archive.read("META-INF/container.xml"))
        rootfile = next(e for e in container.iter() if e.tag.endswith("rootfile"))
        destination.write_bytes(archive.read(rootfile.attrib["full-path"]))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--tag", required=True, help="git tag to build, e.g. v2.0.1")
    parser.add_argument("--verovio", default="6.4.1", help="pinned Verovio npm version")
    parser.add_argument("--cpu-label", required=True, help="CPU model recorded in the report")
    parser.add_argument("--rounds", type=int, default=9)
    parser.add_argument("--input", action="append", metavar="ID=PATH",
                        help="repository-relative MusicXML or MXL at the tag (default: four OpenScore lieder)")
    parser.add_argument("--work-dir", type=Path, help="reuse a build directory instead of a temporary one")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)

    inputs = [item.split("=", 1) for item in (args.input or DEFAULT_INPUTS)]
    work = args.work_dir or Path(tempfile.mkdtemp(prefix="acorde-release-bench-"))
    work.mkdir(parents=True, exist_ok=True)
    source = work / "src"
    if not source.exists():
        run(["git", "-C", str(REPO), "worktree", "add", "--detach", str(source), args.tag])
    commit = run(["git", "-C", str(source), "rev-parse", "HEAD"]).strip()

    package = work / "pkg"
    if not (package / "acorde_wasm.js").exists():
        subprocess.run(["wasm-pack", "build", str(source / "crates/wasm"), "--release", "--target", "nodejs",
                        "--out-dir", str(package), "--features", "measured-print"], check=True)
    verovio_prefix = work / "verovio"
    if not (verovio_prefix / "node_modules/verovio/package.json").exists():
        subprocess.run(["npm", "install", "--prefix", str(verovio_prefix), f"verovio@{args.verovio}",
                        "--ignore-scripts", "--no-audit", "--no-fund"], check=True)

    config = work / "config.json"
    config.write_text(json.dumps(CONFIG), encoding="utf-8")
    font = source / FONT
    bench = REPO / "scripts/bench_print_parity.mjs"
    env = {**os.environ, "VEROVIO_VERSION": args.verovio}

    results = []
    input_hashes = {}
    for position, (score_id, relative) in enumerate(inputs):
        plain = work / f"{score_id}.musicxml"
        extract_musicxml(source / relative, plain)
        input_hashes[relative] = sha256(source / relative)
        order = ["acorde", "verovio"] if position % 2 == 0 else ["verovio", "acorde"]
        for engine in order:
            module = package / "acorde_wasm.js" if engine == "acorde" else verovio_prefix
            output = run(["node", str(bench), engine, str(module), str(plain), str(font), str(config),
                          str(args.rounds)], env=env)
            row = json.loads(output)
            row.update({"score": score_id, "source": relative, "order": order})
            results.append(row)
            print(f"{score_id} {engine}: median {row['median_ms']:.1f} ms, "
                  f"max RSS {row['process_max_rss_kib'] / 1024:.0f} MiB", file=sys.stderr)

    report = {
        "schema_version": 1,
        "classification": "same-host release WASM measurement; not a quality or general superiority claim",
        "tag": args.tag,
        "commit": commit,
        "verovio": args.verovio,
        "cpu_label": args.cpu_label,
        "host": platform.platform(),
        "toolchain": {
            "rustc": tool_version(["rustc", "--version"]),
            "wasm_pack": tool_version(["wasm-pack", "--version"]),
            "wasm_opt": tool_version(["wasm-opt", "--version"]),
            "node": tool_version(["node", "--version"]),
        },
        "execution": (f"isolated Node process per engine and input; engine order alternates by input; "
                      f"2 warmups and {args.rounds} measured parse/render operations"),
        "config": CONFIG,
        "artifact_hashes": {
            "acorde_wasm_bg.wasm": sha256(package / "acorde_wasm_bg.wasm"),
            FONT: sha256(font),
            "scripts/bench_print_parity.mjs": sha256(bench),
            **input_hashes,
        },
        "results": results,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {args.output}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
