import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import mido


class EvaluationAlignmentTest(unittest.TestCase):
    def test_shift_and_range_score_recording_time(self):
        with tempfile.TemporaryDirectory() as directory:
            reference = Path(directory) / 'reference.mid'
            estimate = Path(directory) / 'estimate.mid'
            for path, delay in ((reference, 960), (estimate, 0)):
                midi = mido.MidiFile(ticks_per_beat=480)
                track = mido.MidiTrack()
                midi.tracks.append(track)
                track.append(mido.Message('note_on', note=60, velocity=80, time=delay))
                track.append(mido.Message('note_off', note=60, time=480))
                track.append(mido.Message('note_on', note=64, velocity=80, time=480))
                track.append(mido.Message('note_off', note=64, time=480))
                midi.save(path)
            result = subprocess.run(
                [sys.executable, str(Path(__file__).with_name('evaluate.py')),
                 str(reference), str(estimate), '--estimate-offset', '1', '--start', '0.9', '--end', '1.6'],
                check=True, capture_output=True, text=True,
            )
            report = json.loads(result.stdout)
            self.assertEqual(report['reference_notes'], 1)
            self.assertEqual(report['estimated_notes'], 1)
            self.assertEqual(report['estimate_offset_seconds'], 1)
            self.assertEqual(report['onset']['f1'], 1)

    def test_empty_estimate_counts_as_miss(self):
        with tempfile.TemporaryDirectory() as directory:
            reference = Path(directory) / 'reference.mid'
            estimate = Path(directory) / 'estimate.mid'
            for path, delay in ((reference, 960), (estimate, 0)):
                midi = mido.MidiFile(ticks_per_beat=480)
                track = mido.MidiTrack()
                midi.tracks.append(track)
                track.append(mido.Message('note_on', note=60, velocity=80, time=delay))
                track.append(mido.Message('note_off', note=60, time=480))
                midi.save(path)
            result = subprocess.run(
                [sys.executable, str(Path(__file__).with_name('evaluate.py')),
                 str(reference), str(estimate), '--start', '0.9', '--end', '1.6'],
                check=True, capture_output=True, text=True,
            )
            report = json.loads(result.stdout)
            self.assertEqual(report['estimated_notes'], 0)
            self.assertEqual(report['onset']['f1'], 0)


if __name__ == '__main__':
    unittest.main()
