import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from diagnose_mozart_cadence_transition_rules import plain_roman, transitions


def sonority(measure, beat, roman):
    return {"measure": measure, "beat": beat,
            "label": {"roman_numeral": roman,
                      "address": {"part": 0, "staff": 0, "measure": measure,
                                  "voice": 0, "note": int(beat)}}}


class TransitionHypothesisTest(unittest.TestCase):
    def test_secondary_or_sixth_degree_is_not_plain_dominant(self):
        self.assertTrue(plain_roman("V7", "V"))
        self.assertFalse(plain_roman("V7/ii", "V"))
        self.assertFalse(plain_roman("VI", "V"))

    def test_half_and_deceptive_transitions(self):
        score = [sonority(0, 0, "I64"), sonority(0, 1, "V"),
                 sonority(1, 0, "vi")]
        self.assertEqual([item["kind"] for item in transitions(score)],
                         ["Half", "Deceptive"])

    def test_distant_transition_is_not_used(self):
        score = [sonority(0, 0, "I64"), sonority(2, 0, "V")]
        self.assertEqual(transitions(score), [])


if __name__ == "__main__":
    unittest.main()
