"""Check that the BCMH comparator detects written differences at equal MIDI pitch."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compare_bcmh_keys import same_measure_events, same_measure_spelling_and_ties


class WrittenSemanticsTests(unittest.TestCase):
    def test_enharmonic_spelling_and_tie_differences(self) -> None:
        c_sharp = [[[(0.0, "pitched", (61,), 1.0,
                      (("C", 4, 1.0, True, False),))]]]
        d_flat = [[[(0.0, "pitched", (61,), 1.0,
                    (("D", 4, -1.0, True, False),))]]]
        no_tie = [[[(0.0, "pitched", (61,), 1.0,
                     (("C", 4, 1.0, False, False),))]]]

        self.assertTrue(same_measure_events(c_sharp, d_flat, "pitched", with_onset=True))
        self.assertFalse(same_measure_spelling_and_ties(
            c_sharp, d_flat, include_ties=False
        ))
        self.assertFalse(same_measure_spelling_and_ties(
            c_sharp, no_tie, include_ties=True
        ))
        self.assertTrue(same_measure_spelling_and_ties(
            c_sharp, no_tie, include_ties=False
        ))


if __name__ == "__main__":
    unittest.main()
