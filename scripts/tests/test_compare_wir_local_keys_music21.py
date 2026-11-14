import unittest

import music21

from compare_wir_local_keys_music21 import (align_music21_estimates, counts,
                                            music21_window_grid, score_rows)


class LocalKeyComparisonTests(unittest.TestCase):
    def test_blank_measure_keeps_following_music21_address(self):
        aligned = align_music21_estimates(["raw0", None, "raw2"],
                                          ["C major", "G major"], 3)
        self.assertEqual(aligned, {0: "C major", 2: "G major"})
        gold = [{"measure": 2, "measure_index": 1, "beat": 0.0,
                 "gold_local_key": "G major"},
                {"measure": 3, "measure_index": 2, "beat": 0.0,
                 "gold_local_key": "G major"}]
        rows = score_rows(gold, {1: "G major", 2: "G major"}, aligned)
        self.assertEqual(counts(rows)["music21_available"], 1)
        self.assertEqual(counts(rows)["music21_exact"], 1)
        self.assertEqual(counts(rows)["acorde_exact"], 2)

    def test_length_mismatch_is_an_error(self):
        with self.assertRaisesRegex(ValueError, "do not align"):
            align_music21_estimates(["raw0", None], ["C major", "G major"], 2)

    def test_window_order_does_not_change_native_results(self):
        score = music21.converter.parse("tinyNotation: c1 e1 g1 c1 d-4 d-4 d-4 d-4")
        windows = (0, 1, 2, 4)
        measure_count, forward = music21_window_grid(score, windows)
        _, backward = music21_window_grid(score, tuple(reversed(windows)))
        self.assertEqual(forward, backward)
        native = music21.analysis.floatingKey.KeyAnalyzer(score)
        native.windowSize = 4
        smoothed = native.run()
        self.assertEqual(forward[4], align_music21_estimates(
            native.rawKeyByMeasure, smoothed, measure_count))


if __name__ == "__main__":
    unittest.main()
