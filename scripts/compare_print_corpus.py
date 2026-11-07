#!/usr/bin/env python3
"""Measure Acorde print pages against Verovio on the same MusicXML (Issue #183).

Usage:
  cargo build --release -p acorde-cli -p acorde-pdf --example print_mxl
  python3 scripts/compare_print_corpus.py OUT.json WORKDIR NAME:SOURCE[:MEASURES] ...

Needs Verovio 6.3.0 on PATH or VEROVIO_NPM_PREFIX with verovio@6.4.1,
plus Google Chrome at the macOS default path. Set CARGO_TARGET_DIR when the
release binaries are outside the checkout. ACORDE_BINARY and PRINT_MXL_BINARY
can select existing binaries from another build profile for geometry checks.

Each SOURCE is a licensed fixture. MuseScore sources are first converted by
`acorde convert` so both renderers read one shared MusicXML file. Acorde uses
`print_mxl` (A4, notation-density breaks, at most MEASURES per system, default
4) and Verovio uses A4 with 14 mm side and 16 mm top/bottom margins.

Every measurement is taken the same way for both renderers: each page SVG is
inlined into one HTML page and measured in headless Chrome with
getBoundingClientRect, converted to millimetres from the A4 page width.

- marks: symbol counts in the SVG against counts in the shared MusicXML. A
  shortfall is a candidate missing mark; spans split across systems can
  legitimately exceed the source count.
- clipping: drawn elements extending past the page edge, and past the margins.
- collisions: text boxes overlapping noteheads, accidentals, rests, or other
  text by more than 0.5 mm^2. Text boxes include font ascent and descent, so
  this is a screening count, not a defect list.
- legibility: median staff space, and median/minimum rendered font size in mm
  (CSS font size times the element's screen scale).

Import diagnostics (`acorde report`) are recorded separately. These are
screening numbers on a small corpus; they are not a quality ranking.
"""

import collections
import hashlib
import json
import os
import pathlib
import re
import statistics
import subprocess
import sys
import xml.etree.ElementTree as ET
import zipfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
TARGET = pathlib.Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
ACORDE = pathlib.Path(os.environ.get("ACORDE_BINARY", TARGET / "release/acorde"))
PRINT_MXL = pathlib.Path(os.environ.get("PRINT_MXL_BINARY", TARGET / "release/examples/print_mxl"))
FONT = ROOT / "crates/pdf/tests/fixtures/Tuffy.ttf"
CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
VEROVIO = ["verovio", "--page-width", "2100", "--page-height", "2970",
           "--page-margin-top", "160", "--page-margin-bottom", "160",
           "--page-margin-left", "140", "--page-margin-right", "140"]
VEROVIO_NPM_PREFIX = os.environ.get("VEROVIO_NPM_PREFIX")

SELECTORS = {
    "acorde": {
        "noteheads": ".acorde-notehead", "rests": ".acorde-rest:not(.acorde-hidden)",
        "slurs": ".acorde-slur", "ties": ".acorde-tie", "lyrics": ".acorde-lyric, .acorde-lyric-verse",
        "dynamics": ".acorde-dynamic", "hairpins": ".acorde-hairpin", "articulations": ".acorde-articulation",
        "notation": ".acorde-notehead, .acorde-accidental, .acorde-rest:not(.acorde-hidden)",
        "staff_lines": ".acorde-staff > line", "staff": ".acorde-staff", "system": None,
    },
    "verovio": {
        "noteheads": ".notehead", "rests": ".rest, .mRest",
        "slurs": ".slur", "ties": ".tie", "lyrics": ".syl",
        "dynamics": ".dynam", "hairpins": ".hairpin:has(path, line, polyline)", "articulations": ".artic",
        "notation": ".notehead, .accid, .rest, .mRest",
        "staff_lines": ":scope > path", "staff": ".staff", "system": ".system",
    },
}

MEASURE_JS = r"""
const SEL = %s;
const out = {pages: []};
for (const svg of document.querySelectorAll('body > div > svg')) {
  const page = svg.getBoundingClientRect();
  const mm = 210 / page.width;
  const box = el => { const r = el.getBoundingClientRect();
    return {x0: (r.left - page.left) * mm, y0: (r.top - page.top) * mm,
            x1: (r.right - page.left) * mm, y1: (r.bottom - page.top) * mm}; };
  const real = b => b.x1 - b.x0 > 0.01 || b.y1 - b.y0 > 0.01;
  const counts = {};
  for (const k of ['noteheads','rests','slurs','ties','lyrics','dynamics','hairpins','articulations'])
    counts[k] = svg.querySelectorAll(SEL[k]).length;
  const H = 297, W = 210;
  let pastEdge = 0, pastMargin = 0;
  for (const el of svg.querySelectorAll('path, use, line, polyline, polygon, rect, ellipse, circle, text')) {
    if (el.closest('defs, symbol')) continue;
    const b = box(el); if (!real(b)) continue;
    if (b.x0 < -0.1 || b.y0 < -0.1 || b.x1 > W + 0.1 || b.y1 > H + 0.1) pastEdge++;
    else if (b.x0 < 14 - 0.5 || b.x1 > W - 14 + 0.5 || b.y0 < 16 - 0.5 || b.y1 > H - 16 + 0.5) pastMargin++;
  }
  const texts = [...svg.querySelectorAll('text')].filter(t => t.textContent.trim() && !t.closest('defs'))
    .map(t => ({b: box(t), label: t.textContent.trim(), className: t.getAttribute('class') || '',
                lyric: !!t.closest(SEL.lyrics),
                font: Math.max(...[t, ...t.querySelectorAll('tspan')].map(e =>
                  parseFloat(getComputedStyle(e).fontSize) * e.getScreenCTM().d * mm))})).filter(t => real(t.b));
  const marks = [...svg.querySelectorAll(SEL.notation)]
    .map(el => ({b: box(el), className: el.getAttribute('class') || ''})).filter(m => real(m.b));
  const area = (a, b) => Math.max(0, Math.min(a.x1, b.x1) - Math.max(a.x0, b.x0)) *
                         Math.max(0, Math.min(a.y1, b.y1) - Math.max(a.y0, b.y0));
  let textMark = 0, textText = 0;
  const textMarkCandidates = [];
  for (let i = 0; i < texts.length; i++) {
    for (const m of marks) if (area(texts[i].b, m.b) > 0.5) {
      textMark++;
      textMarkCandidates.push({text: texts[i].label, textClass: texts[i].className,
        markClass: m.className, overlap_mm2: area(texts[i].b, m.b), textBox: texts[i].b, markBox: m.b});
    }
    for (let j = i + 1; j < texts.length; j++) if (area(texts[i].b, texts[j].b) > 0.5) textText++;
  }
  const spaces = [];
  for (const staff of svg.querySelectorAll(SEL.staff)) {
    const ys = [...staff.querySelectorAll(SEL.staff_lines)].map(box).filter(b => b.x1 - b.x0 > 5)
      .map(b => (b.y0 + b.y1) / 2).sort((a, b) => a - b);
    if (ys.length === 5) spaces.push((ys[4] - ys[0]) / 4);
  }
  const med = a => { if (!a.length) return null; const s = [...a].sort((x, y) => x - y); return s[Math.floor(s.length / 2)]; };
  out.pages.push({counts, past_page_edge: pastEdge, past_margins: pastMargin,
    text_over_notation: textMark, text_over_text: textText,
    text_notation_candidates: textMarkCandidates,
    systems: SEL.system ? svg.querySelectorAll(SEL.system).length : null,
    staff_space_mm: med(spaces), lyric_font_mm: med(texts.filter(t => t.lyric).map(t => t.font)),
    text_font_mm: med(texts.map(t => t.font)), min_text_font_mm: texts.length ? Math.min(...texts.map(t => t.font)) : null});
}
document.getElementById('out').textContent = JSON.stringify(out);
"""


def run(*args, **kw):
    return subprocess.run([str(a) for a in args], check=True, capture_output=True, text=True, **kw)


def source_root(path):
    if path.suffix != ".mxl":
        return ET.parse(path).getroot()
    with zipfile.ZipFile(path) as archive:
        container = ET.fromstring(archive.read("META-INF/container.xml"))
        rootfile = next(e for e in container.iter() if e.tag.endswith("rootfile")).get("full-path")
        return ET.fromstring(archive.read(rootfile))


def expected_marks(path):
    root = source_root(path)
    counts = collections.Counter()
    for note in root.iter("note"):
        if note.find("rest") is not None:
            counts["rests"] += note.get("print-object") != "no"
        elif note.get("print-object") != "no":
            counts["noteheads"] += 1
        counts["lyrics"] += sum(1 for l in note.findall("lyric") if (l.findtext("text") or "").strip())
        for n in note.iter("notations"):
            counts["slurs"] += sum(1 for s in n.findall("slur") if s.get("type") == "start")
            counts["ties"] += sum(1 for t in n.findall("tied") if t.get("type") == "start")
            counts["articulations"] += sum(len(a) for a in n.findall("articulations"))
    for d in root.iter("dynamics"):
        # <other-dynamics> carries arbitrary text; compare it as a direction,
        # not as a standard dynamic glyph.
        counts["dynamics"] += sum(mark.tag != "other-dynamics" for mark in d)
    for w in root.iter("wedge"):
        counts["hairpins"] += w.get("type") in ("crescendo", "diminuendo")
    return dict(counts)


def measure(svgs, selectors, html_path):
    body = "".join(f"<div>{re.sub(r'<\\?xml[^>]*>', '', p.read_text())}</div>" for p in svgs)
    html_path.write_text(
        "<!doctype html><html><head><style>body{margin:0}div{width:840px}div>svg{width:840px;height:1188px;display:block}"
        f"</style></head><body>{body}<pre id='out'></pre><script>{MEASURE_JS % json.dumps(selectors)}</script></body></html>")
    dom = run(CHROME, "--headless=new", "--disable-gpu", "--virtual-time-budget=10000",
              "--dump-dom", html_path.as_uri()).stdout
    pages = json.loads(re.search(r"<pre id=\"out\">(.*?)</pre>", dom, re.S).group(1).replace("&amp;", "&"))["pages"]
    total = collections.Counter()
    for p in pages:
        total.update(p["counts"])
    summed = lambda k: sum(p[k] for p in pages)
    values = lambda k: [p[k] for p in pages if p[k] is not None]
    return {
        "pages": len(pages),
        "marks": dict(total),
        "elements_past_page_edge": summed("past_page_edge"),
        "elements_past_margins": summed("past_margins"),
        "text_over_notation": summed("text_over_notation"),
        "text_over_text": summed("text_over_text"),
        "text_notation_candidates": [dict(page=index + 1, **candidate)
                                     for index, page in enumerate(pages)
                                     for candidate in page["text_notation_candidates"]],
        "staff_space_mm": round(statistics.median(values("staff_space_mm")), 2) if values("staff_space_mm") else None,
        "lyric_font_mm": round(statistics.median(values("lyric_font_mm")), 2) if values("lyric_font_mm") else None,
        "text_font_mm": round(statistics.median(values("text_font_mm")), 2) if values("text_font_mm") else None,
        "min_text_font_mm": round(min(values("min_text_font_mm")), 2) if values("min_text_font_mm") else None,
        "systems": summed("systems") if pages and pages[0]["systems"] is not None else None,
    }


def main():
    if len(sys.argv) < 4:
        sys.exit(__doc__)
    out, work = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    work.mkdir(parents=True, exist_ok=True)
    rows = []
    for spec in sys.argv[3:]:
        name, source, *rest = spec.split(":")
        measures = rest[0] if rest else "4"
        src = ROOT / source
        shared = src if src.suffix in (".mxl", ".musicxml") else work / f"{name}.musicxml"
        if shared != src:
            run(ACORDE, "convert", src, shared)
        report = json.loads(run(ACORDE, "report", shared).stdout)
        codes = collections.Counter(d["code"] for d in report["diagnostics"])
        pdf = work / f"{name}-acorde.pdf"
        for old in work.glob(f"{name}-acorde-page-*.svg"):
            old.unlink()
        log = run(PRINT_MXL, shared, FONT, pdf, measures, "density").stderr
        acorde_svgs = sorted(work.glob(f"{name}-acorde-page-*.svg"), key=lambda p: int(p.stem.rsplit("-", 1)[1]))
        for old in work.glob(f"{name}-verovio*.svg"):
            old.unlink()
        if VEROVIO_NPM_PREFIX:
            run("node", ROOT / "scripts/export_verovio_pages.mjs", VEROVIO_NPM_PREFIX,
                shared, work / f"{name}-verovio")
        else:
            run(*VEROVIO, "-a", "-o", work / f"{name}-verovio.svg", shared)
        verovio_svgs = sorted(work.glob(f"{name}-verovio*.svg"))
        acorde = measure(acorde_svgs, SELECTORS["acorde"], work / f"{name}-acorde.html")
        acorde["systems"] = sum(len(re.findall(r"\(\[", line)) for line in log.splitlines() if line.startswith("page "))
        verovio = measure(verovio_svgs, SELECTORS["verovio"], work / f"{name}-verovio.html")
        expected = expected_marks(shared)
        rows.append({
            "score": name, "source": source, "measures_per_system": int(measures),
            "verovio_version": "6.4.1" if VEROVIO_NPM_PREFIX else "6.3.0",
            "source_sha256": hashlib.sha256(src.read_bytes()).hexdigest(),
            "shared_input_sha256": hashlib.sha256(shared.read_bytes()).hexdigest(),
            "import_diagnostics": {"total": sum(codes.values()), "by_code": dict(sorted(codes.items()))},
            "source_marks": expected, "acorde": acorde, "verovio": verovio,
            "shortfalls": {r: {k: v - d[r]["marks"].get(k, 0) for k, v in expected.items() if d[r]["marks"].get(k, 0) < v}
                           for d in [{"acorde": acorde, "verovio": verovio}] for r in d},
        })
    out.write_text(json.dumps(rows, indent=2) + "\n")
    for r in rows:
        a, v = r["acorde"], r["verovio"]
        print(f"{r['score']}: import diagnostics {r['import_diagnostics']['total']}")
        for label, m in (("Acorde ", a), ("Verovio", v)):
            print(f"  {label} pages {m['pages']} systems {m['systems']} edge {m['elements_past_page_edge']} "
                  f"margin {m['elements_past_margins']} text/notation {m['text_over_notation']} "
                  f"text/text {m['text_over_text']} staff {m['staff_space_mm']} lyric {m['lyric_font_mm']} "
                  f"text {m['text_font_mm']} min-text {m['min_text_font_mm']} shortfall {r['shortfalls'][label.strip().lower()]}")


if __name__ == "__main__":
    main()
