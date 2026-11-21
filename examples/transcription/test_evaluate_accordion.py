import json
import tempfile
import unittest
from pathlib import Path

from evaluate_accordion import compare, read_events


class AccordionEvaluationTest(unittest.TestCase):
    def test_pitch_root_quality_and_duplicate_matching(self):
        reference = {
            'right': [{'time': 1, 'pitch': 64}, {'time': 2, 'pitch': 67}],
            'bass': [{'time': 1, 'root': 7}],
            'chord': [{'time': 1.5, 'root': 7, 'quality': 'major'}],
        }
        estimate = {
            'right': [{'time': 1.02, 'pitch': 64}, {'time': 1.04, 'pitch': 64},
                      {'time': 2.01, 'pitch': 79}],
            'bass': [{'time': 1.05, 'root': 7}],
            'chord': [{'time': 1.52, 'root': 7, 'quality': 'minor'}],
        }
        with tempfile.TemporaryDirectory() as directory:
            ref = Path(directory) / 'reference.json'
            est = Path(directory) / 'estimate.json'
            ref.write_text(json.dumps(reference))
            est.write_text(json.dumps(estimate))
            result = compare(read_events(ref), read_events(est), 0.1)
        self.assertEqual(result['right']['matched'], 1)
        self.assertEqual(result['right']['false_positive'], 2)
        self.assertEqual(result['right']['missed'], 1)
        self.assertEqual(result['bass']['matched'], 1)
        self.assertEqual(result['chord']['matched'], 0)

    def test_rejects_invalid_annotation(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'bad.json'
            path.write_text(json.dumps({'right': [], 'bass': [], 'chord': [
                {'time': 1, 'root': 12, 'quality': 'major'}]}))
            with self.assertRaises(ValueError):
                read_events(path)


if __name__ == '__main__':
    unittest.main()
