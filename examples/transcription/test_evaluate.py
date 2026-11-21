import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import mido

from server import midi_bytes


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

    def test_pickup_export_matches_absolute_reference_after_shift(self):
        common = {'tempo': 120, 'numerator': 4, 'denominator': 4,
                  'notes': [{'start': 9.75, 'end': 10.25, 'pitch': 60, 'hand': 'right'}]}
        with tempfile.TemporaryDirectory() as directory:
            reference = Path(directory) / 'reference.mid'
            estimate = Path(directory) / 'estimate.mid'
            reference.write_bytes(midi_bytes({**common, 'bar_start': 0}))
            estimate.write_bytes(midi_bytes({**common, 'bar_start': 10, 'pickup_steps': 4}))
            result = subprocess.run(
                [sys.executable, str(Path(__file__).with_name('evaluate.py')),
                 str(reference), str(estimate), '--estimate-offset', '9.5',
                 '--start', '9.5', '--end', '10.5'],
                check=True, capture_output=True, text=True,
            )
            report = json.loads(result.stdout)
            self.assertEqual(report['onset']['f1'], 1)
            self.assertEqual(report['onset_offset']['f1'], 1)

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
