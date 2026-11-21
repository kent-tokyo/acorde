import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import mido
import numpy as np
import pretty_midi

from evaluate import error_details, extract_playback_report
from server import midi_bytes


class EvaluationAlignmentTest(unittest.TestCase):
    def test_playback_report_offset_and_range(self):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory) / 'report.json'
            report.write_text(json.dumps([
                {'time_secs': 0.5, 'duration_secs': 0.25, 'pitch_midi': 60},
                {'time_secs': 1.5, 'duration_secs': 0.25, 'pitch_midi': 62},
                {'time_secs': 0.5, 'duration_secs': 0.25, 'pitch_midi': 90, 'is_metronome': True},
            ]))
            intervals, pitches = extract_playback_report(report, shift=10, start=10, end=11)
            np.testing.assert_allclose(intervals, [[10.5, 10.75]])
            np.testing.assert_allclose(pitches, [pretty_midi.note_number_to_hz(60)])

    def test_error_details_identifies_unmatched_and_offset_errors(self):
        hz = lambda pitches: np.array([pretty_midi.note_number_to_hz(p) for p in pitches])
        reference = (np.array([[1., 2.], [3., 4.]]), hz([60, 62]))
        estimate = (np.array([[1., 1.5], [1.02, 2.], [1.03, 2.]]), hz([60, 60, 72]))
        errors = error_details(reference, estimate)
        self.assertEqual([n['pitch'] for n in errors['missed']], [62])
        self.assertEqual(len(errors['extra']), 2)
        self.assertTrue(next(n for n in errors['extra'] if n['pitch'] == 60)['near_matched_same_pitch'])
        self.assertTrue(next(n for n in errors['extra'] if n['pitch'] == 72)['near_reference_octave'])
        self.assertEqual(len(errors['offset_errors']), 1)
        self.assertAlmostEqual(errors['offset_errors'][0]['offset_error_seconds'], -0.5)

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
