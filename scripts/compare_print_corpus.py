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

Break policy (PRINT_BREAKS, recorded per row as `break_policy`): Acorde keeps
the source's `<print new-system/new-page>` breaks, while Verovio's default
layout ignores them, so raw page counts compare different layouts. The default
`stripped` removes those print breaks from the shared input before either
engine reads it. `source` keeps them, as earlier runs did.

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
from fractions import Fraction
import hashlib
import html
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
PRINT_STRATEGY = os.environ.get("ACORDE_PRINT_STRATEGY", "density")
if PRINT_STRATEGY not in ("density", "measured"):
    raise ValueError("ACORDE_PRINT_STRATEGY must be density or measured")
FONT = ROOT / "crates/pdf/tests/fixtures/Tuffy.ttf"
CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
VEROVIO = ["verovio", "--page-width", "2100", "--page-height", "2970",
           "--page-margin-top", "160", "--page-margin-bottom", "160",
           "--page-margin-left", "140", "--page-margin-right", "140"]
VEROVIO_NPM_PREFIX = os.environ.get("VEROVIO_NPM_PREFIX")
BREAK_POLICY = os.environ.get("PRINT_BREAKS", "stripped")
if BREAK_POLICY not in ("stripped", "source"):
    raise ValueError("PRINT_BREAKS must be stripped or source")

SELECTORS = {
    "acorde": {
        "noteheads": ".acorde-notehead", "rests": ".acorde-rest:not(.acorde-hidden)",
        "slurs": ".acorde-slur", "ties": ".acorde-tie", "lyrics": ".acorde-lyric, .acorde-lyric-verse",
        "dynamics": ".acorde-dynamic", "hairpins": ".acorde-hairpin", "articulations": ".acorde-articulation",
        "notation": ".acorde-notehead, .acorde-accidental, .acorde-rest:not(.acorde-hidden)",
        "staff_lines": ".acorde-staff > line", "staff": ".acorde-staff", "system": None,
        "curves": ".acorde-stem, .acorde-beam, .acorde-slur, .acorde-tie, .acorde-articulation, .acorde-arpeggio, .acorde-lyric-hyphen, .acorde-lyric-extender, .acorde-tuplet-bracket, .acorde-tuplet-number, .acorde-hairpin",
    },
    "verovio": {
        "noteheads": ".notehead", "rests": ".rest, .mRest",
        "slurs": ".slur", "ties": ".tie", "lyrics": ".syl",
        "dynamics": ".dynam", "hairpins": ".hairpin:has(path, line, polyline)", "articulations": ".artic",
        "notation": ".notehead, .accid, .rest, .mRest",
        "staff_lines": ":scope > path", "staff": ".staff", "system": ".system",
        "curves": ".stem, .beam, .slur, .tie, .artic, .arpeg, .tupletBracket, .tupletNum, .hairpin",
    },
}

MEASURE_JS = r"""
const SEL = %s;
const W = SEL.page_width_mm || 210, H = SEL.page_height_mm || 297;
const out = {pages: []};
for (const svg of document.querySelectorAll('body > div > svg')) {
  const page = svg.getBoundingClientRect();
  const mm = W / page.width;
  const box = el => { const r = el.getBoundingClientRect();
    return {x0: (r.left - page.left) * mm, y0: (r.top - page.top) * mm,
            x1: (r.right - page.left) * mm, y1: (r.bottom - page.top) * mm}; };
  const real = b => b.x1 - b.x0 > 0.01 || b.y1 - b.y0 > 0.01;
  const counts = {};
  for (const k of ['noteheads','rests','slurs','ties','lyrics','dynamics','hairpins','articulations'])
    counts[k] = svg.querySelectorAll(SEL[k]).length;
  let pastEdge = 0, pastMargin = 0;
  const edgeCandidates = [], marginCandidates = [];
  for (const el of svg.querySelectorAll('path, use, line, polyline, polygon, rect, ellipse, circle, text')) {
    if (el.closest('defs, symbol')) continue;
    const b = box(el); if (!real(b)) continue;
    if (b.x0 < -0.1 || b.y0 < -0.1 || b.x1 > W + 0.1 || b.y1 > H + 0.1) {
      pastEdge++;
      edgeCandidates.push({box:b, className:el.getAttribute('class') || el.parentElement?.getAttribute('class'), text:el.closest('[data-text]')?.getAttribute('data-text') || el.textContent});
    }
    else if (b.x0 < 14 - 0.5 || b.x1 > W - 14 + 0.5 || b.y0 < 16 - 0.5 || b.y1 > H - 16 + 0.5) {
      pastMargin++;
      marginCandidates.push({box:b, className:el.getAttribute('class') || el.parentElement?.getAttribute('class'), text:el.closest('[data-text]')?.getAttribute('data-text') || el.textContent});
    }
  }
  const texts = [...svg.querySelectorAll('text, [data-acorde-outlined-text]')].filter(t => (t.getAttribute('data-text') || t.textContent).trim() && !t.closest('defs'))
    .map(t => ({el: t, b: box(t), label: t.getAttribute('data-text') || t.textContent.trim(), className: t.getAttribute('class') || '',
                lyric: !!t.closest(SEL.lyrics),
                font: t.hasAttribute('data-font-size') ? parseFloat(t.getAttribute('data-font-size')) * t.getScreenCTM().d * mm : Math.max(...[t, ...t.querySelectorAll('tspan')].map(e =>
                  parseFloat(getComputedStyle(e).fontSize) * e.getScreenCTM().d * mm))})).filter(t => real(t.b));
  for (const el of svg.querySelectorAll('g.acorde-dynamic[data-baseline]')) {
    texts.push({el, b:box(el), label:el.getAttribute('data-text'), className:'acorde-dynamic', lyric:false, font:null});
  }
  const marks = [...svg.querySelectorAll(SEL.notation)]
    .map(el => ({b: box(el), className: el.getAttribute('class') || ''})).filter(m => real(m.b));
  const area = (a, b) => Math.max(0, Math.min(a.x1, b.x1) - Math.max(a.x0, b.x0)) *
                         Math.max(0, Math.min(a.y1, b.y1) - Math.max(a.y0, b.y0));
  let textMark = 0, textText = 0;
  const textMarkCandidates = [];
  const textTextCandidates = [];
  const staffLines = [...svg.querySelectorAll(SEL.staff)].flatMap(staff =>
    [...staff.querySelectorAll(SEL.staff_lines)]).map(box).filter(b => b.x1 - b.x0 > 5);
  const lyricStaffCandidates = [];
  const dynamicStaffCandidates = [];
  const curveCandidates = [];
  const curves = [...new Set([...svg.querySelectorAll(SEL.curves)].flatMap(el =>
    el instanceof SVGGeometryElement ? [el] : [...el.querySelectorAll('path,line,polyline,polygon,circle,ellipse')]))]
    .map(el => ({el, b:box(el), owner:el.closest(SEL.curves)?.getAttribute('class') || ''}));
  for (let i = 0; i < texts.length; i++) {
    const t = texts[i];
    for (const curve of curves) {
      // An outlined text ornament owns its own paths; those are not collisions.
      if (t.el.contains(curve.el)) continue;
      const b = t.b, c = curve.b;
      if (Math.min(b.x1,c.x1) < Math.max(b.x0,c.x0) || Math.min(b.y1,c.y1) < Math.max(b.y0,c.y0)) continue;
      const length = curve.el.getTotalLength(), matrix = curve.el.getScreenCTM();
      const scale = Math.hypot(matrix.a,matrix.b) * mm;
      const steps = Math.max(1, Math.min(20000, Math.ceil(length*scale/0.15)));
      const hit = Array.from({length:steps+1}, (_,j) => {
        const p = curve.el.getPointAtLength(length*j/steps).matrixTransform(matrix);
        const x = (p.x-page.left)*mm, y = (p.y-page.top)*mm;
        if (!(b.x0+0.04 < x && x < b.x1-0.04 && b.y0+0.04 < y && y < b.y1-0.04)) return false;
        if (!t.el.hasAttribute('data-acorde-outlined-text') && !t.el.matches('g.acorde-dynamic')) return true;
        return [...t.el.querySelectorAll('path')].some(path =>
          { const point = new DOMPoint(p.x,p.y).matrixTransform(path.getScreenCTM().inverse());
            return path.isPointInFill(point) || path.isPointInStroke(point); });
      }).some(Boolean);
      if (hit) curveCandidates.push({text:t.label,textBox:b,markClass:curve.owner,markBox:c});
    }
    for (const m of marks) if (area(texts[i].b, m.b) > 0.5) {
      textMark++;
      textMarkCandidates.push({text: texts[i].label, textClass: texts[i].className,
        markClass: m.className, overlap_mm2: area(texts[i].b, m.b), textBox: texts[i].b, markBox: m.b});
    }
    for (let j = i + 1; j < texts.length; j++) if (area(texts[i].b, texts[j].b) > 0.5) {
      textText++;
      textTextCandidates.push({first:t.label, second:texts[j].label, firstBox:t.b, secondBox:texts[j].b});
    }
    if (texts[i].lyric) for (const line of staffLines) {
      const y = (line.y0 + line.y1) / 2, b = texts[i].b;
      if (b.y0 + 0.05 < y && y < b.y1 - 0.05 && Math.min(b.x1,line.x1)-Math.max(b.x0,line.x0) > 0.2)
        lyricStaffCandidates.push({text:texts[i].label,textBox:b,lineBox:line});
    }
    if (t.el.matches('g.acorde-dynamic')) for (const line of staffLines) {
      const y = (line.y0 + line.y1) / 2, b = t.b;
      if (b.y0 + 0.05 < y && y < b.y1 - 0.05 && Math.min(b.x1,line.x1)-Math.max(b.x0,line.x0) > 0.2)
        dynamicStaffCandidates.push({text:t.label,textBox:b,lineBox:line});
    }
  }
  // For Acorde's outlined first verse, compare baselines only within one
  // measure and voice. Cross-measure/system and additional-verse alignment
  // require separate ownership information and are outside this screen.
  const lyricLanes = new Map(), lyricBaselineCandidates = [];
  for (const t of texts) {
    if (!t.el.matches('g.acorde-lyric[data-acorde-outlined-text]')) continue;
    const owner = t.el.closest('[data-print-note-addr]');
    const path = t.el.querySelector('path');
    const address = owner?.getAttribute('data-print-note-addr')?.split(':');
    if (!path || !address || address.length !== 5) continue;
    const key = address.slice(0, 4).join(':');
    const origin = new DOMPoint(0, 0).matrixTransform(path.getScreenCTM());
    const lane = lyricLanes.get(key) || [];
    lane.push({text:t.label, note:address[4], baseline_mm:(origin.y-page.top)*mm});
    lyricLanes.set(key, lane);
  }
  for (const [address, lane] of lyricLanes) {
    const ys = lane.map(item => item.baseline_mm);
    if (ys.length > 1 && Math.max(...ys) - Math.min(...ys) > 0.15)
      lyricBaselineCandidates.push({address, syllables:lane});
  }
  const spaces = [];
  for (const staff of svg.querySelectorAll(SEL.staff)) {
    const ys = [...staff.querySelectorAll(SEL.staff_lines)].map(box).filter(b => b.x1 - b.x0 > 5)
      .map(b => (b.y0 + b.y1) / 2).sort((a, b) => a - b);
    if (ys.length === 5) spaces.push((ys[4] - ys[0]) / 4);
  }
  const med = a => { if (!a.length) return null; const s = [...a].sort((x, y) => x - y); return s[Math.floor(s.length / 2)]; };
  out.pages.push({counts, past_page_edge: pastEdge, past_margins: pastMargin,
    edge_candidates: edgeCandidates, margin_candidates: marginCandidates, text_text_candidates: textTextCandidates,
    text_over_notation: textMark, text_over_text: textText,
    text_notation_candidates: textMarkCandidates,
    lyric_staff_candidates: lyricStaffCandidates,
    lyric_baseline_candidates: lyricBaselineCandidates,
    dynamic_staff_candidates: dynamicStaffCandidates,
    text_curve_candidates: curveCandidates,
    systems: SEL.system ? svg.querySelectorAll(SEL.system).length : null,
    staff_space_mm: med(spaces), staff_space_samples_mm: spaces,
    lyric_font_mm: med(texts.filter(t => t.lyric).map(t => t.font)),
    text_font_mm: med(texts.map(t => t.font).filter(f => f !== null)), min_text_font_mm: texts.some(t => t.font !== null) ? Math.min(...texts.map(t => t.font).filter(f => f !== null)) : null});
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


def expected_marks(root):
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
        # Both renderers draw one <dynamics> element as one dynamic, even when
        # it lists several marks such as <p/> and <pp/>. <other-dynamics>
        # carries arbitrary text; compare it as a direction, not as a glyph.
        counts["dynamics"] += any(mark.tag != "other-dynamics" for mark in d)
    for w in root.iter("wedge"):
        counts["hairpins"] += w.get("type") in ("crescendo", "diminuendo")
    return dict(counts)


def strip_print_breaks(root):
    """Remove encoded system and page breaks; return how many were removed."""
    removed = 0
    for element in root.iter("print"):
        for attribute in ("new-system", "new-page"):
            if element.attrib.pop(attribute, None) == "yes":
                removed += 1
    return removed


def wedge_endpoints(root):
    """Inventory written endpoints without guessing where an unclosed wedge ends."""
    endpoints = []
    for part in root.findall("part"):
        divisions = 1
        for measure in part.findall("measure"):
            cursor = Fraction(0)
            for item in measure:
                if item.tag == "attributes" and item.find("divisions") is not None:
                    divisions = Fraction(item.findtext("divisions"))
                elif item.tag in ("note", "backup", "forward"):
                    if item.find("chord") is None and item.find("grace") is None:
                        length = Fraction(item.findtext("duration", "0")) / divisions
                        cursor += -length if item.tag == "backup" else length
                elif item.tag == "direction":
                    for wedge in item.findall("direction-type/wedge"):
                        beat = cursor + Fraction(item.findtext("offset", "0")) / divisions
                        endpoints.append({
                            "part": part.get("id"), "measure": measure.get("number"),
                            "quarter_offset": str(beat), "staff": item.findtext("staff", "1"),
                            "number": wedge.get("number", "1"), "type": wedge.get("type"),
                            "placement": item.get("placement"),
                        })
    return endpoints


def measure(svgs, selectors, html_path, page_width_mm=210, page_height_mm=297):
    if not (0 < page_width_mm < 2000 and 0 < page_height_mm < 2000):
        raise ValueError("invalid physical page size")
    selectors = dict(selectors, page_width_mm=page_width_mm, page_height_mm=page_height_mm)
    body = "".join(f"<div>{re.sub(r'<\\?xml[^>]*>', '', p.read_text())}</div>" for p in svgs)
    html_path.write_text(
        "<!doctype html><html><head><meta charset='utf-8'><style>body{margin:0}"
        f"div{{width:{page_width_mm*4}px}}div>svg{{width:{page_width_mm*4}px;height:{page_height_mm*4}px;display:block}}"
        f"</style></head><body>{body}<pre id='out'></pre><script>{MEASURE_JS % json.dumps(selectors)}</script></body></html>")
    dom = run(CHROME, "--headless=new", "--disable-gpu", "--virtual-time-budget=10000",
              "--dump-dom", html_path.as_uri()).stdout
    pages = json.loads(html.unescape(re.search(r"<pre id=\"out\">(.*?)</pre>", dom, re.S).group(1)))["pages"]
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
        "edge_candidates": [dict(page=index + 1, **candidate)
                            for index, page in enumerate(pages)
                            for candidate in page["edge_candidates"]],
        "margin_candidates": [dict(page=index + 1, **candidate)
                              for index, page in enumerate(pages)
                              for candidate in page["margin_candidates"]],
        "text_text_candidates": [dict(page=index + 1, **candidate)
                                 for index, page in enumerate(pages)
                                 for candidate in page["text_text_candidates"]],
        "lyric_staff_candidates": [dict(page=index + 1, **candidate)
                                     for index, page in enumerate(pages)
                                     for candidate in page["lyric_staff_candidates"]],
        "lyric_baseline_candidates": [dict(page=index + 1, **candidate)
                                        for index, page in enumerate(pages)
                                        for candidate in page["lyric_baseline_candidates"]],
        "dynamic_staff_candidates": [dict(page=index + 1, **candidate)
                                     for index, page in enumerate(pages)
                                     for candidate in page["dynamic_staff_candidates"]],
        "text_curve_candidates": [dict(page=index + 1, **candidate)
                                   for index, page in enumerate(pages)
                                   for candidate in page["text_curve_candidates"]],
        "text_notation_candidates": [dict(page=index + 1, **candidate)
                                     for index, page in enumerate(pages)
                                     for candidate in page["text_notation_candidates"]],
        "staff_space_mm": round(statistics.median(values("staff_space_mm")), 2) if values("staff_space_mm") else None,
        "staff_space_samples_mm": [{"page": index + 1, "staff_ordinal": ordinal,
                                    "space_mm": round(space, 5)}
                                   for index, page in enumerate(pages)
                                   for ordinal, space in enumerate(page["staff_space_samples_mm"])],
        "min_staff_space_mm": round(min(space for page in pages for space in page["staff_space_samples_mm"]), 5)
                              if any(page["staff_space_samples_mm"] for page in pages) else None,
        "max_staff_space_mm": round(max(space for page in pages for space in page["staff_space_samples_mm"]), 5)
                              if any(page["staff_space_samples_mm"] for page in pages) else None,
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
        removed_breaks = 0
        if BREAK_POLICY == "stripped":
            root = source_root(shared)
            removed_breaks = strip_print_breaks(root)
            shared = work / f"{name}-shared.musicxml"
            ET.ElementTree(root).write(shared, encoding="UTF-8", xml_declaration=True)
        report = json.loads(run(ACORDE, "report", shared).stdout)
        codes = collections.Counter(d["code"] for d in report["diagnostics"])
        pdf = work / f"{name}-acorde.pdf"
        for old in work.glob(f"{name}-acorde-page-*.svg"):
            old.unlink()
        try:
            log = run(PRINT_MXL, shared, FONT, pdf, measures, PRINT_STRATEGY).stderr
        except subprocess.CalledProcessError as error:
            rows.append({"score": name, "source": source, "status": "failed",
                         "stage": "acorde-print", "error": error.stderr,
                         "acorde_print_strategy": PRINT_STRATEGY,
                         "source_sha256": hashlib.sha256(src.read_bytes()).hexdigest(),
                         "shared_input_sha256": hashlib.sha256(shared.read_bytes()).hexdigest()})
            out.write_text(json.dumps(rows, indent=2) + "\n")
            continue
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
        root = source_root(shared)
        expected = expected_marks(root)
        rows.append({
            "score": name, "source": source, "measures_per_system": int(measures),
            "acorde_print_strategy": PRINT_STRATEGY,
            "break_policy": BREAK_POLICY, "removed_print_breaks": removed_breaks,
            "verovio_version": "6.4.1" if VEROVIO_NPM_PREFIX else "6.3.0",
            "source_sha256": hashlib.sha256(src.read_bytes()).hexdigest(),
            "shared_input_sha256": hashlib.sha256(shared.read_bytes()).hexdigest(),
            "import_diagnostics": {"total": sum(codes.values()), "by_code": dict(sorted(codes.items()))},
            "source_marks": expected, "source_wedge_endpoints": wedge_endpoints(root),
            "acorde": acorde, "verovio": verovio,
            "shortfalls": {r: {k: v - d[r]["marks"].get(k, 0) for k, v in expected.items() if d[r]["marks"].get(k, 0) < v}
                           for d in [{"acorde": acorde, "verovio": verovio}] for r in d},
        })
        out.write_text(json.dumps(rows, indent=2) + "\n")
    out.write_text(json.dumps(rows, indent=2) + "\n")
    for r in rows:
        if r.get("status") == "failed":
            print(f"{r['score']}: FAILED {r['stage']}: {r['error'].strip()}")
            continue
        a, v = r["acorde"], r["verovio"]
        print(f"{r['score']}: import diagnostics {r['import_diagnostics']['total']}")
        for label, m in (("Acorde ", a), ("Verovio", v)):
            print(f"  {label} pages {m['pages']} systems {m['systems']} edge {m['elements_past_page_edge']} "
                  f"margin {m['elements_past_margins']} text/notation {m['text_over_notation']} "
                  f"text/text {m['text_over_text']} staff {m['staff_space_mm']} lyric {m['lyric_font_mm']} "
                  f"text {m['text_font_mm']} min-text {m['min_text_font_mm']} shortfall {r['shortfalls'][label.strip().lower()]}")
    if any(r.get("status") == "failed" for r in rows):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
