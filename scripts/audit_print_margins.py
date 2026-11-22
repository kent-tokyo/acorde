#!/usr/bin/env python3
"""Classify A4 margin crossings in measured Acorde development SVGs.

Each input directory must contain render-report.json and its recorded page SVGs.
The output contains every candidate; it does not classify printer-safe margins or
replace physical inspection. Requires headless Google Chrome.
"""

import argparse
import collections
import hashlib
import html as html_module
import json
import pathlib
import re
import subprocess

SCRIPT = r"""
let result=[];
for (const [i,svg] of [...document.querySelectorAll('body > div > svg')].entries()) {
  const page=svg.getBoundingClientRect(), mm=210/page.width;
  for (const el of svg.querySelectorAll('path,use,line,polyline,polygon,rect,ellipse,circle,text')) {
    if (el.closest('defs,symbol')) continue;
    const r=el.getBoundingClientRect(), b=[(r.left-page.left)*mm,(r.top-page.top)*mm,(r.right-page.left)*mm,(r.bottom-page.top)*mm];
    if (b[2]-b[0]<=0.01 && b[3]-b[1]<=0.01) continue;
    if (b[0]<-0.1 || b[1]<-0.1 || b[2]>210.1 || b[3]>297.1) continue;
    if (!(b[0]<13.5 || b[2]>196.5 || b[1]<15.5 || b[3]>281.5)) continue;
    const group=el.closest('[class]'), semantic=el.closest('[data-acorde-kind]'), text=el.closest('[data-text]');
    const fontSize=text?.getAttribute('data-font-size');
    const fontMm=fontSize ? Number(fontSize) * text.getScreenCTM().d * mm : null;
    result.push({page:i+1,box:b.map(x=>Math.round(x*100)/100),tag:el.tagName,
      className:group?.getAttribute('class')||'',kind:semantic?.getAttribute('data-acorde-kind')||'',
      part:semantic?.getAttribute('data-part'),staff:semantic?.getAttribute('data-staff'),
      measure:semantic?.getAttribute('data-measure'),text:text?.getAttribute('data-text')||el.textContent||'',
      font_mm:fontMm === null ? null : Math.round(fontMm*100)/100});
  }
}
document.getElementById('out').textContent=JSON.stringify(result);
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=pathlib.Path)
    parser.add_argument("directories", nargs="+", type=pathlib.Path)
    parser.add_argument("--chrome", default="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
    args = parser.parse_args()
    rows = []
    for directory in args.directories:
        report = json.loads((directory / "render-report.json").read_text())
        for row in report["rows"]:
            name = row["score"]
            pages = sorted(directory.glob(name + "-acorde-page-*.svg"),
                           key=lambda path: int(path.stem.rsplit("-", 1)[1]))
            if len(pages) != row["acorde"]["pages"] or not pages:
                raise ValueError(f"missing page SVGs for {name}")
            for page in pages:
                actual = hashlib.sha256(page.read_bytes()).hexdigest()
                if row["acorde"]["svg_sha256"].get(page.name) != actual:
                    raise ValueError(f"page SVG differs from render receipt: {page}")
            body = "".join("<div>" + re.sub(r"<\?xml[^>]*>", "", page.read_text()) + "</div>"
                           for page in pages)
            review = directory / (name + "-margin-audit.html")
            review.write_text("<!doctype html><style>body{margin:0}div{width:840px}"
                              "div>svg{width:840px;height:1188px;display:block}</style>"
                              + body + '<pre id="out"></pre><script>' + SCRIPT + "</script>")
            run = subprocess.run([args.chrome, "--headless=new", "--disable-gpu",
                                  "--virtual-time-budget=10000", "--dump-dom", review.resolve().as_uri()],
                                 capture_output=True, text=True, timeout=120, check=True)
            match = re.search(r'<pre id="out">(.*?)</pre>', run.stdout, re.S)
            if match is None:
                raise ValueError(f"missing margin results for {name}")
            items = json.loads(html_module.unescape(match.group(1)))
            classes = dict(collections.Counter(item["className"] or item["kind"] or item["tag"]
                                               for item in items))
            rows.append({"score": name, "count": len(items), "classes": classes, "items": items})
            args.output.write_text(json.dumps(rows, indent=2) + "\n")
            print(name, len(items), classes, flush=True)


if __name__ == "__main__":
    main()
