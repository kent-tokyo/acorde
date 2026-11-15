#!/usr/bin/env python3
"""Evaluate every prepared reserved input, retaining both engine failures.

Usage: evaluate_print_holdout.py PREPARATION FREEZE OUTPUT_DIR
Uses pinned artifacts from the freeze. Outputs are screening evidence, not an
automatic visual or source-fidelity pass. No downloads or source modifications.
"""
import collections
import hashlib
import json
import pathlib
import re
import sys

import compare_print_corpus as c
from audit_print_musicxml import xml_events, score_events


def sha(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def main():
    preparation_path, freeze_path, output = map(pathlib.Path, sys.argv[1:])
    preparation = json.loads(preparation_path.read_text())
    freeze = json.loads(freeze_path.read_text())
    if preparation["freeze_sha256"] != sha(freeze_path):
        raise ValueError("candidate freeze differs from source preparation")
    for path, digest in freeze["artifacts_sha256"].items():
        if sha(path) != digest:
            raise ValueError(f"frozen artifact changed: {path}")
    artifact = lambda suffix: next(path for path in freeze["artifacts_sha256"] if path.endswith(suffix))
    printer = artifact("/examples/print_mxl")
    native = artifact("/examples/measured_print")
    font = artifact("/fixtures/Tuffy.ttf")
    audit_cli = freeze["native_cli_audit_tool"]["path"]
    if sha(audit_cli) != freeze["native_cli_audit_tool"]["sha256"]:
        raise ValueError("audit CLI changed")
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    records = []
    for prepared in preparation["rows"]:
        name = prepared["score"]
        row = {"score": name, "group": prepared["group"], "review": "pending",
               "freeze_sha256": sha(freeze_path), "preparation_status": prepared["status"]}
        if prepared["status"] != "prepared":
            row.update(status="failed", stage="source-preparation", error=prepared.get("error"))
        else:
            source = pathlib.Path(prepared["shared_input"])
            if sha(source) != prepared["shared_input_sha256"]:
                raise ValueError(f"prepared input changed: {source}")
            row.update(source=str(source), shared_input_sha256=sha(source),
                       measures_per_system=8, acorde_print_strategy="measured", verovio_version="6.4.1")
            row["source_marks"] = c.expected_marks(source)
            try:
                report = json.loads(c.run(audit_cli, "report", source).stdout)
                expected, actual = xml_events(c.source_root(source)), score_events(report["score"])
                row["pitched_event_audit"] = {"source": sum(expected.values()), "imported": sum(actual.values()),
                    "missing": [{"event": k, "count": v} for k, v in (expected-actual).items()],
                    "extra": [{"event": k, "count": v} for k, v in (actual-expected).items()]}
                row["import_diagnostics"] = dict(collections.Counter(x["code"] for x in report["diagnostics"]))
            except Exception as error:
                row["import_error"] = str(error)
            for engine in ("acorde", "verovio"):
                try:
                    if engine == "acorde":
                        log = c.run(printer, source, font, output / f"{name}-acorde.pdf", "8", "measured").stderr
                        (output / f"{name}-acorde.log").write_text(log)
                        c.run(native, source, font, output / f"{name}-native.json")
                    else:
                        result = c.run("node", c.ROOT / "scripts/export_verovio_pages.mjs", c.VEROVIO_NPM_PREFIX,
                                       source, output / f"{name}-verovio", "6.4.1")
                        (output / f"{name}-verovio.log").write_text(result.stderr)
                    svgs = sorted(output.glob(f"{name}-{engine}*.svg"), key=lambda p: int(p.stem.rsplit("-", 1)[1]))
                    measurement = c.measure(svgs, c.SELECTORS[engine], output / f"{name}-{engine}.html")
                    if engine == "acorde":
                        measurement["systems"] = sum(len(re.findall(r"\(\[", line)) for line in log.splitlines() if line.startswith("page "))
                    measurement["status"] = "rendered"
                    measurement["svg_sha256"] = {p.name: sha(p) for p in svgs}
                    row[engine] = measurement
                except Exception as error:
                    row[engine] = {"status": "failed", "error": str(error), "stderr": getattr(error, "stderr", None)}
            row["status"] = "rendered" if all(row[e]["status"] == "rendered" for e in ("acorde", "verovio")) else "failed"
        records.append(row)
        (output / "evaluation.json").write_text(json.dumps(records, indent=2) + "\n")
        print(name, row["status"], flush=True)


if __name__ == "__main__":
    main()
