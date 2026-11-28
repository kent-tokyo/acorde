#!/usr/bin/env python3
"""Inventory public structs and enums and propose a 3.0 extensibility decision.

Reads rustdoc JSON for the library crates and writes one JSON row per publicly
reachable struct or enum (#496). Generate the input with a stable toolchain:

    for crate in acorde-core acorde-layout acorde-io acorde-render-svg acorde-pdf \
                 acorde-analysis acorde-soundfont acorde-omr; do
      RUSTC_BOOTSTRAP=1 cargo rustdoc -p "$crate" --all-features --lib --locked \
        -- -Z unstable-options --output-format json
    done
    python3 -B scripts/api_extensibility_inventory.py target/doc/acorde_*.json \
      --output docs/api-extensibility.json

The decision is a proposal from fixed rules, listed in docs/api-extensibility.md.
Reviewers change the rules or the CLOSED_ENUMS list, then regenerate. Rows are
not hand-edited.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path

# Enums whose variant set is fixed by music theory or by a two-sided relation.
# Everything else is presumed to grow.
CLOSED_ENUMS = {
    "acorde_core::model::pitch::Step",
    "acorde_core::model::interval::IntervalQuality",
    "acorde_core::model::score::HarpPedalPosition",
    "acorde_core::model::validate::SpannerEndpoint",
    "acorde_core::model::notation::HairpinKind",
}

# acorde-core modules that implement opt-in arrangement features. They are
# candidates for the experimental tier proposed in #497.
ARRANGEMENT_MODULE_PREFIXES = (
    "accordion_", "arrange", "bass_line", "chord_plan", "chord_proposals",
    "chordal_pad", "countermelody", "cross_plan_form_proposals",
    "developed_form_proposals", "ensemble_", "free_bass", "harmony_arrangement",
    "harmony_comping", "harmony_expansion", "harmony_groove_", "harmony_voicing",
    "inner_voices", "instrument_plan", "integrated_arrangement_proposals",
    "legacy_accompaniment", "melodic_insert", "performer_edition",
    "phrase_harmony", "phrase_melody", "profiled_chord_plan", "rhythm_engraving",
    "rhythm_section", "standard_bass",
)


def has_non_exhaustive(item: dict) -> bool:
    for attr in item.get("attrs", []):
        text = attr if isinstance(attr, str) else json.dumps(attr)
        if "non_exhaustive" in text:
            return True
    return False


def trait_impls(index: dict, item_impls: list) -> set[str]:
    names = set()
    for impl_id in item_impls:
        impl = index.get(str(impl_id), {}).get("inner", {}).get("impl")
        if impl and impl.get("trait"):
            trait = impl["trait"]
            names.add(trait.get("path") or trait.get("name") or "")
    return {name.rsplit("::", 1)[-1] for name in names}


def has_constructor(index: dict, item_impls: list) -> bool:
    for impl_id in item_impls:
        impl = index.get(str(impl_id), {}).get("inner", {}).get("impl")
        if not impl or impl.get("trait"):
            continue
        for member in impl.get("items", []):
            fn = index.get(str(member), {})
            if "function" in fn.get("inner", {}) and fn.get("visibility") == "public":
                output = fn["inner"]["function"]["sig"].get("output")
                if output and "generic" in output and output["generic"] == "Self":
                    return True
                if fn.get("name") in {"new", "builder"}:
                    return True
    return False


def tier(path: list[str]) -> str:
    if len(path) > 2 and path[0] == "acorde_core" and path[1] == "model":
        if path[2].startswith(ARRANGEMENT_MODULE_PREFIXES):
            return "experimental-candidate"
    return "stable"


def classify(index: dict, item: dict, kind: str, full_path: str) -> tuple[str, dict]:
    facts: dict = {}
    if has_non_exhaustive(item):
        return "already-non-exhaustive", facts
    if kind == "enum":
        facts["variants"] = len(item["inner"]["enum"]["variants"])
        if full_path in CLOSED_ENUMS:
            return "closed", facts
        return "non-exhaustive", facts
    struct = item["inner"]["struct"]
    struct_kind = struct["kind"]
    impls = struct.get("impls", [])
    if struct_kind == "unit":
        return "no-action", facts
    if "plain" in struct_kind:
        fields = struct_kind["plain"]["fields"]
        stripped = struct_kind["plain"]["has_stripped_fields"]
    else:
        fields = [f for f in struct_kind["tuple"] if f is not None]
        stripped = any(f is None for f in struct_kind["tuple"])
    facts["public_fields"] = len(fields)
    if stripped or not fields:
        return "no-action", facts
    traits = trait_impls(index, impls)
    facts["default"] = "Default" in traits
    facts["constructor"] = has_constructor(index, impls)
    if facts["default"] or facts["constructor"]:
        return "non-exhaustive", facts
    return "non-exhaustive+constructor", facts


def inventory(paths: list[Path]) -> list[dict]:
    rows = []
    for path in paths:
        doc = json.loads(path.read_text(encoding="utf-8"))
        index = doc["index"]
        for item_id, summary in doc["paths"].items():
            if summary.get("crate_id") != 0 or summary.get("kind") not in {"struct", "enum"}:
                continue
            item = index.get(item_id)
            if item is None or item.get("visibility") != "public":
                continue
            full_path = "::".join(summary["path"])
            decision, facts = classify(index, item, summary["kind"], full_path)
            span = item.get("span") or {}
            filename = span.get("filename", "")
            if "/crates/" in filename:
                filename = "crates/" + filename.split("/crates/", 1)[1]
            rows.append({
                "path": full_path,
                "kind": summary["kind"],
                "tier": tier(summary["path"]),
                "decision": decision,
                "source": f"{filename}:{span.get('begin', [0])[0]}" if filename else None,
                **facts,
            })
    rows.sort(key=lambda row: row["path"])
    return rows


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("rustdoc_json", nargs="+", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    rows = inventory(args.rustdoc_json)
    summary = Counter((row["tier"], row["kind"], row["decision"]) for row in rows)
    report = {
        "schema_version": 1,
        "closed_enums": sorted(CLOSED_ENUMS),
        "summary": [
            {"tier": t, "kind": k, "decision": d, "count": n}
            for (t, k, d), n in sorted(summary.items())
        ],
        "items": rows,
    }
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"{len(rows)} public types written to {args.output}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
