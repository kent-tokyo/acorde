#!/usr/bin/env python3
"""Render every comparison page for assistant review; this does not assign a pass.

Usage: render_print_review.py COMPARISON.json WORKDIR REVIEWDIR
Chrome converts Verovio's full SVG pages to A4 PDF; Poppler rasterizes both PDFs.
"""

import concurrent.futures
import hashlib
import json
import pathlib
import re
import subprocess
import sys
import tempfile

CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"


def run(*args):
    return subprocess.run([str(a) for a in args], check=True, capture_output=True, text=True)


def render(row, work, review):
    name = row["score"]
    svgs = sorted(work.glob(f"{name}-verovio-*.svg"), key=lambda p: int(p.stem.rsplit("-", 1)[1]))
    if len(svgs) != row["verovio"]["pages"]:
        raise ValueError(f"{name}: Verovio page count mismatch")
    html = review / f"{name}-verovio.html"
    width, height = row.get("paper_size_mm", [210, 297])
    html.write_text("<!doctype html><meta charset='utf-8'><style>"
                    f"@page{{size:{width}mm {height}mm;margin:0}}html,body{{margin:0;padding:0}}"
                    f"section{{width:{width}mm;height:{height}mm;break-after:page}}"
                    f"section:last-child{{break-after:auto}}svg{{display:block;width:{width}mm;height:{height}mm}}"
                    "</style>" + "".join("<section>" + re.sub(r"<\?xml[^>]*>", "", p.read_text()) + "</section>" for p in svgs))
    verovio_pdf = review / f"{name}-verovio.pdf"
    with tempfile.TemporaryDirectory(prefix="acorde-review-chrome-") as profile:
        run(CHROME, "--headless=new", "--disable-gpu", "--no-pdf-header-footer",
            f"--user-data-dir={profile}", f"--print-to-pdf={verovio_pdf}", html.as_uri())
    records = []
    for engine, pdf in [("acorde", work / f"{name}-acorde.pdf"), ("verovio", verovio_pdf)]:
        info = run("pdfinfo", pdf).stdout
        pages = int(re.search(r"^Pages:\s+(\d+)", info, re.M).group(1))
        if pages != row[engine]["pages"]:
            raise ValueError(f"{name}/{engine}: PDF page count {pages} differs from SVG")
        prefix = review / f"{name}-{engine}"
        run("pdftoppm", "-scale-to", "1800", "-png", pdf, prefix)
        images = sorted(review.glob(f"{name}-{engine}-*.png"))
        records.append({"score": name, "engine": engine, "pdf_sha256": hashlib.sha256(pdf.read_bytes()).hexdigest(),
                        "pages": [{"file": p.name, "sha256": hashlib.sha256(p.read_bytes()).hexdigest(), "review": "pending"} for p in images]})
    return records


def main():
    comparison, work, review = map(pathlib.Path, sys.argv[1:])
    work, review = work.resolve(), review.resolve()
    review.mkdir(parents=True, exist_ok=True)
    rows = json.loads(comparison.read_text())
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        batches = list(pool.map(lambda row: render(row, work, review), rows))
    records = [record for batch in batches for record in batch]
    (review / "manifest.json").write_text(json.dumps(records, indent=2) + "\n")
    print(json.dumps({"scores": len(rows), "pages": sum(len(r["pages"]) for r in records), "review": "pending"}))


if __name__ == "__main__":
    main()
