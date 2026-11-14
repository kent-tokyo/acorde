"""Check the pinned cadence split without reading the annotations."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from freeze_mozart_cadence import freeze


class FreezeMozartCadenceTests(unittest.TestCase):
    def test_complete_sonatas_never_cross_splits(self):
        entries = []
        for sonata in range(279, 291):
            for movement in (1, 2, 3):
                entries.append((f"scores/kv{sonata}_{movement}.musicxml", "a" * 40))
                entries.append((f"score_parts_annotated/kv{sonata}_{movement}_spart_cadence.csv", "b" * 40))
        result = freeze("c" * 40, entries)
        self.assertEqual(36, len(result["cases"]))
        self.assertEqual(18, sum(case["split"] == "development" for case in result["cases"]))
        self.assertEqual(18, sum(case["split"] == "holdout" for case in result["cases"]))
        for sonata in range(279, 291):
            splits = {case["split"] for case in result["cases"] if case["id"].startswith(f"K{sonata}-")}
            self.assertEqual(1, len(splits))

    def test_missing_annotation_rejected(self):
        with self.assertRaisesRegex(ValueError, "no cadence annotation"):
            freeze("c" * 40, [("scores/kv279_1.musicxml", "a" * 40)])


if __name__ == "__main__":
    unittest.main()
