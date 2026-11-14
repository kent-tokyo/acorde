import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from fetch_global_key_holdout_scores import git_blob_sha1
from score_global_key_holdout import (chorale_index_labels, gold_for_work,
                                      normalize_label, opening_key)


class GlobalKeyLabelTest(unittest.TestCase):
    def test_case_encodes_mode_and_accidental(self):
        self.assertEqual(normalize_label("Bb"), "Bb major")
        self.assertEqual(normalize_label("f#"), "F# minor")
        self.assertEqual(normalize_label("e-"), "Eb minor")
        with self.assertRaises(ValueError):
            normalize_label("C major")

    def test_opening_key_ignores_header_and_later_modulation(self):
        analysis = "Title: G: not a key\nm0 b3 g: i\nm2 C: I\n"
        self.assertEqual(opening_key(analysis), "G minor")

    def test_index_marks_ambiguous_or_transposed_row(self):
        filler = "<tr><td>x</td>" + "<td></td>" * 10 + "</tr>"
        target = ("<tr><td>x</td><td></td><td></td><td>201*</td>"
                  "<td></td><td>0</td><td><i>G?</i></td><td>major</td>"
                  "<td></td><td></td><td></td></tr>")
        page = ("<table id='sortable'>" + filler * 300 + target + "</table>").encode()
        rows = chorale_index_labels(page, ["201"])
        self.assertEqual(rows["201"], [{"tonic": "G?", "mode": "major",
                                        "starred": True,
                                        "ambiguous_markup": True}])

    def test_chorale_gold_requires_independent_opening_key_agreement(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "analysis.txt"
            data = b"m1 G: I\n"
            path.write_bytes(data)
            work = {"group": "chorale", "piece": "200",
                    "corroboration_path": "analysis.txt",
                    "corroboration_git_blob": git_blob_sha1(data)}
            index = {"200": [{"tonic": "G", "mode": "major",
                               "starred": False,
                               "ambiguous_markup": False}]}
            self.assertEqual(gold_for_work(work, index, root, {}),
                             ("G major", None))
            index["200"][0]["tonic"] = "D"
            self.assertEqual(gold_for_work(work, index, root, {})[1],
                             "chorale_reference_disagreement")


if __name__ == "__main__":
    unittest.main()
