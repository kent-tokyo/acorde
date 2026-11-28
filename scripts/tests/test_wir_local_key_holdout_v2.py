import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import adapt_wir_local_key_report as adapter  # noqa: E402
import freeze_wir_local_key_holdout_v2 as freeze  # noqa: E402


def pair(work):
    return [(work + "/score.mxl", "s" + work[-1]), (work + "/analysis.txt", "a" + work[-1])]


class SelectionTest(unittest.TestCase):
    def test_excludes_prior_works_and_their_collections(self):
        quartets = "Corpus/Quartets/Beethoven,_Ludwig_van/"
        bach = "Corpus/Keyboard_Other/Bach,_Johann_Sebastian/WTC/"
        mozart = "Corpus/Piano_Sonatas/Mozart,_Wolfgang_Amadeus/"
        songs = "Corpus/OpenScore-LiederCorpus/"
        entries = []
        for number in range(1, 7):
            entries += pair(f"{quartets}Op{number}/1") + pair(f"{quartets}Op{number}/2")
            entries += pair(f"{mozart}K{number}/1")
            entries += pair(f"{songs}Composer{number}/Cycle/{number}")
        for number in range(1, 8):
            entries += pair(f"{bach}{number}")
        entries.append((f"{mozart}K6/2/score.mxl", "lonely"))
        previous = [{"works": [{"work": f"{quartets}Op1/1"}, {"work": f"{bach}1"},
                               {"work": f"{mozart}K1/1"}, {"work": f"{songs}Composer1/Cycle/1"}]}]
        works = freeze.select(entries, previous)
        names = [work["work"] for work in works]
        self.assertEqual(len(works), 16)
        self.assertFalse(any(name.startswith(f"{quartets}Op1/") for name in names))
        self.assertNotIn(f"{bach}1", names)
        self.assertNotIn(f"{mozart}K6/2", names)
        quartet_sets = {name.rsplit("/", 1)[0] for name in names if name.startswith(quartets)}
        self.assertEqual(len(quartet_sets), 4)
        composers = {freeze.composer(name) for name in names if name.startswith(songs)}
        self.assertEqual(len(composers), 4)
        self.assertEqual(names, [work["work"] for work in freeze.select(entries, previous)])


class AdapterTest(unittest.TestCase):
    def test_rows_must_match_their_recorded_hash(self):
        with tempfile.TemporaryDirectory() as tmp:
            rows = Path(tmp) / "report-rows.jsonl"
            rows.write_text(json.dumps({"work": "w", "measure_index": 0, "beat": 0.5,
                                        "gold": "C major", "acorde": "C major",
                                        "music21": None}) + "\n")
            report = {"rows_file": rows.name, "rows_sha256": adapter.sha256(rows)}
            grouped = adapter.rows_for(Path(tmp) / "report.json", report)
            case = adapter.case("w", "digest", "holdout", grouped["w"])
            self.assertEqual(case["gold"][0]["at"]["beat"], "1/2")
            self.assertEqual(case["systems"]["music21"], [])
            self.assertEqual(case["systems"]["acorde"][0]["label"], "C major")
            report["rows_sha256"] = "0" * 64
            with self.assertRaises(ValueError):
                adapter.rows_for(Path(tmp) / "report.json", report)


if __name__ == "__main__":
    unittest.main()
