"""Prepare two counterbalanced player review packs; keep the key with the coordinator."""
import argparse
import json
from pathlib import Path
import secrets
import tempfile
from zipfile import ZipFile, ZIP_DEFLATED
from xml.etree import ElementTree as ET

from build_performer_edition_pack import EDITIONS, FONT, PIANO, digest, run

FORM = """個別試奏用　Orchard Gate

同じ編成のAとBを、ほかの奏者と相談する前に比べてください。
音源はすべて同じテンポ・ピアノ音色です。譜面はA4、倍率100%で印刷します。
各項目で A／B／同程度／判断できず のいずれかと、理由・小節番号を記入してください。

独奏　旋律が追いやすい方：
独奏　実際に両手を動かして無理なく弾ける方：
二重奏　旋律と句末の応答が自然に聞こえる方：
二重奏　担当の受け渡しが弾きやすい方：
清書　拍と声部を読み取りやすい方：
音符・記号の欠落、演奏できない箇所、読み違えた箇所：
使用楽器・担当パート・A4実寸印刷の有無・試奏した範囲：
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bin_dir", type=Path)
    parser.add_argument("baseline_printer", type=Path)
    parser.add_argument("out", type=Path)
    args = parser.parse_args()
    cli = args.bin_dir.resolve() / "acorde"
    printer = cli.parent / "examples/print_mxl"
    original = EDITIONS / "blind-orchard.musicxml"
    if digest(original) != "d6bc812d4a04658c1e85cf2f8212348e9acf81d3aab3db191dc623ce26f9c575":
        raise ValueError("review source differs from reserved fixture")
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    assignments = {}
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        materials = {}
        for ensemble in ["solo", "duet"]:
            for treatment in ["reference", "candidate"]:
                name = ensemble + "-" + treatment
                cfg = json.loads((EDITIONS / (ensemble + ".json")).read_text())
                if ensemble == "solo":
                    cfg["edition"]["max_right_hand_span"] = 0 if treatment == "reference" else 7
                else:
                    # The reference leaves the melody with one player throughout,
                    # accompanied by the same profiled bass/chord pulse.
                    cfg["edition"]["partner_measures"] = list(range(8)) if treatment == "reference" else [4, 5]
                    cfg["edition"]["answer_measures"] = [] if treatment == "reference" else [3]
                config = root / (name + ".json")
                config.write_text(json.dumps(cfg))
                score = root / (name + ".musicxml")
                run(cli, "performer-edition", original, config, score)
                piano = root / (name + "-audio.musicxml")
                tree = ET.parse(score)
                for program in tree.findall(".//midi-program"):
                    program.text = "1"
                tree.write(piano, encoding="utf-8", xml_declaration=True)
                wav = root / (name + ".wav")
                run(cli, "audio-render", piano, PIANO, wav, "--sample-rate", "24000", "--channels", "1")
                pdf = root / (name + ".pdf")
                run(printer, score, FONT, pdf, "4", "density")
                files = [("score.pdf", pdf), ("listen.wav", wav)]
                if ensemble == "duet":
                    for i in (0, 1):
                        player = root / (name + f"-player-{i+1}.musicxml")
                        run(cli, "extract", score, player, "--part", str(i))
                        part_pdf = player.with_suffix(".pdf")
                        run(printer, player, FONT, part_pdf, "4", "density")
                        files.append((f"player-{i+1}.pdf", part_pdf))
                materials[ensemble, treatment] = files

        # Freeze the music across both print comparisons.
        duet = root / "duet-candidate.musicxml"
        clean = root / "engraved.musicxml"
        run(cli, "engrave-rhythm", duet, clean)
        def event_key(score):
            ev = json.loads(run(cli, "playback-report", score, capture_output=True).stdout)
            return sorted((e["part_index"], e["time_beats"], e["duration_beats"], e["pitch_midi_cents"], e["velocity"]) for e in ev)
        if event_key(duet) != event_key(clean):
            raise ValueError("print pair has different music")
        for treatment, binary, score in [("reference", args.baseline_printer, duet), ("candidate", printer, clean)]:
            pdf = root / ("reading-" + treatment + ".pdf")
            run(binary, score, FONT, pdf, "4", "density")
            materials["reading", treatment] = [("score.pdf", pdf)]

        first = {task: secrets.randbelow(2) for task in ("solo", "duet", "reading")}
        for reader in (1, 2):
            key = {}
            with ZipFile(out / f"player-{reader}.zip", "x", compression=ZIP_DEFLATED) as pack:
                pack.writestr("回答.txt", FORM)
                for task, initial in first.items():
                    for index, treatment in enumerate(("reference", "candidate")):
                        label = "AB"[index ^ initial ^ (reader - 1)]
                        key[f"{task}/{label}"] = treatment
                        for dest, src in materials[task, treatment]:
                            pack.write(src, arcname=f"{task}/{label}/{dest}")
            assignments[str(reader)] = key
        (out / "coordinator.json").write_text(json.dumps({
            "source_sha256": digest(original), "assignments": assignments,
            "archives": {p.name: digest(p) for p in out.glob("*.zip")},
        }, indent=2) + "\n")


if __name__ == "__main__":
    main()
