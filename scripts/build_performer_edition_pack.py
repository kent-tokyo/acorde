"""Build the development audition and print pack from local Acorde binaries."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures"
EDITIONS = FIXTURES / "arrangement/editions"
FONT = ROOT / "crates/pdf/tests/fixtures/Tuffy.ttf"
PIANO = FIXTURES / "UprightPianoKW-small-20190703.sf2"


def run(*args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_source(report, source):
    """Read this monophonic fixture independently, including its cross-bar tie."""
    position = 0
    expected = []
    scale = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
    bars = ET.parse(source).findall("part/measure")
    assert len(bars) == 16 and bars[0].findtext("attributes/divisions") == "2"
    for bar in bars:
        for note in bar.findall("note"):
            length = int(note.findtext("duration"))
            pitch = note.find("pitch")
            if pitch is not None:
                midi = 12 * (int(pitch.findtext("octave")) + 1) + scale[pitch.findtext("step")]
                if note.find('tie[@type="stop"]') is not None:
                    assert expected[-1][0] == midi and sum(expected[-1][1:3]) == position
                    expected[-1][2] += length
                else:
                    expected.append([midi, position, length, 64])
            position += length
    actual = [[e["pitch_midi"], e["time_beats"] * 2, e["duration_beats"] * 2, e["velocity"]] for e in report]
    assert expected == actual and position == 120


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bin_dir", type=Path, help="directory containing acorde and examples/print_mxl")
    parser.add_argument("out", type=Path, help="new output directory")
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    cli = args.bin_dir.resolve() / "acorde"
    printer = cli.parent / "examples/print_mxl"
    source = EDITIONS / "lantern-16.musicxml"
    assets = {}

    def render(name, score, audio=True):
        pdf = out / (name + ".pdf")
        run(printer, score, FONT, pdf, "4", "density")
        events = json.loads(run(cli, "playback-report", score, capture_output=True).stdout)
        (out / (name + "-events.json")).write_text(json.dumps(events, indent=2) + "\n")
        if audio:
            # Keep the published score's instruments. This temporary listening copy
            # uses the same piano on each part to isolate note and rhythm differences.
            tree = ET.parse(score)
            for program in tree.findall(".//midi-program"):
                program.text = "1"
            with tempfile.TemporaryDirectory() as temp:
                audition = Path(temp) / "playback.musicxml"
                tree.write(audition, encoding="utf-8", xml_declaration=True)
                run(cli, "audio-render", audition, PIANO, out / (name + ".wav"),
                    "--sample-rate", "24000", "--channels", "1")
        assets[name] = {"attacks": len(events), "pages": len(list(out.glob(name + "-page-*.svg")))}
        return sorted((e["part_index"], e["time_beats"], e["duration_beats"], e["pitch_midi_cents"], e["velocity"]) for e in events)

    shutil.copyfile(source, out / "source.musicxml")
    render("source", out / "source.musicxml")
    check_source(json.loads((out / "source-events.json").read_text()), source)
    for mode in ("solo", "duet"):
        score = out / (mode + ".musicxml")
        run(cli, "performer-edition", source, EDITIONS / (mode + ".json"), score)
        before = render(mode, score)
        engraved = out / (mode + "-engraved.musicxml")
        run(cli, "engrave-rhythm", score, engraved)
        assert render(mode + "-engraved", engraved, audio=False) == before
        if mode == "duet":
            for i in range(2):
                name = "duet-player-" + str(i + 1)
                part = out / (name + ".musicxml")
                run(cli, "extract", engraved, part, "--part", str(i))
                extracted = render(name, part, audio=False)
                assert [e[1:] for e in extracted] == [e[1:] for e in before if e[0] == i]
    result = {"source_sha256": digest(source), "font_sha256": digest(FONT),
              "soundfont_sha256": digest(PIANO), "outputs": assets,
              "files": {p.name: digest(p) for p in sorted(out.iterdir())}}
    (out / "manifest.json").write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
