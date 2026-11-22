#!/usr/bin/env python3
"""Screen measured-print default/A4 and matched-size/Letter configurations.

Usage: check_print_variants.py COMPARISON.json SHARED_DIR CONFIG_DIR OUT_DIR OUT.json
The comparison's shared input hashes are checked before rendering. No sources are
downloaded, and this script does not assign a visual-review or parity pass.
"""
import hashlib
import json
import pathlib
import re
import sys

import compare_print_corpus as c


def main():
    comparison, shared_dir, configs, work, output = map(pathlib.Path, sys.argv[1:])
    work = work.resolve()
    work.mkdir(parents=True, exist_ok=True)
    results = []
    for row in json.loads(comparison.read_text()):
        source = c.ROOT / row["source"]
        shared = source if source.suffix in (".mxl", ".musicxml") else shared_dir / f'{row["score"]}.musicxml'
        if hashlib.sha256(shared.read_bytes()).hexdigest() != row["shared_input_sha256"]:
            raise ValueError(f"changed shared input: {shared}")
        for variant, paper, width, height, capacity in [
            ("default", "A4", 210, 297, 4), ("letter", "Letter", 215.9, 279.4, 8)
        ]:
            name = row["score"] + "-" + variant
            pdf = work / f"{name}-acorde.pdf"
            record = {"score": name, "source_score": row["score"], "variant": variant,
                      "paper_size_mm": [width, height], "shared_input_sha256": row["shared_input_sha256"],
                      "review": "pending", "configuration": json.loads((configs / f"{variant}.json").read_text())}
            try:
                log = c.run(c.PRINT_MXL, shared, c.FONT, pdf, capacity, "measured", configs / f"{variant}.json").stderr
                c.run("node", c.ROOT / "scripts/export_verovio_pages.mjs", c.VEROVIO_NPM_PREFIX,
                      shared, work / f"{name}-verovio", "6.4.1", paper)
                for engine in ("acorde", "verovio"):
                    svgs = sorted(work.glob(f"{name}-{engine}*.svg"),
                                  key=lambda p: int(p.stem.rsplit("-", 1)[1]))
                    record[engine] = c.measure(svgs, c.SELECTORS[engine], work / f"{name}-{engine}.html", width, height)
                record["acorde"]["systems"] = sum(len(re.findall(r"\(\[", line)) for line in log.splitlines() if line.startswith("page "))
                record["status"] = "rendered"
            except Exception as error:
                record["status"] = "failed"
                record["error"] = str(error)
                record["stderr"] = getattr(error, "stderr", None)
            results.append(record)
            output.write_text(json.dumps(results, indent=2) + "\n")
            print(name, record["status"], flush=True)


if __name__ == "__main__":
    main()
