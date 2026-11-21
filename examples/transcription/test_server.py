import io
import json
import os
import subprocess
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

import mido
from server import midi_bytes, recording_notes


class RecordingWindowTest(unittest.TestCase):
    def test_selected_attacks_keep_releases_beyond_selection(self):
        events = [(0.4, 1.5, 60), (1.5, 3.2, 64), (2.5, 5.0, 67),
                  (3.1, 3.5, 69), (1.7, 2.0, 30)]
        notes, limited, boundary = recording_notes(events, 2.0, 6.0, 10.0, 3.0, 5.0, 48, 80)
        self.assertEqual([(n['pitch'], n['start'], n['end']) for n in notes],
                         [(64, 3.5, 5.2), (67, 4.5, 6.0)])
        self.assertEqual(limited, 1)

    def test_recording_end_is_not_a_context_cut(self):
        notes, limited, boundary = recording_notes([(0.5, 3.0, 60)], 0.0, 3.0, 3.0, 0.0, 2.0, 0, 127)
        self.assertEqual(notes[0]['end'], 3.0)
        self.assertEqual(limited, 0)
        self.assertEqual(boundary, [])

    def test_near_start_notes_are_reviewed_not_inserted(self):
        events = [(1.94, 2.4, 60), (1.96, 2.5, 62), (2.0, 2.7, 64), (4.0, 4.5, 65)]
        notes, limited, boundary = recording_notes(events, 0, 6, 10, 2, 4, 0, 127)
        self.assertEqual([note['pitch'] for note in notes], [64])
        self.assertEqual([note['pitch'] for note in boundary], [62])
        self.assertEqual(boundary[0]['start'], 1.96)
        self.assertEqual(limited, 0)


class MidiExportTest(unittest.TestCase):
    def test_preserves_time_meter_and_hands(self):
        data = {'tempo': 120, 'bar_start': 1.0, 'numerator': 3, 'denominator': 4,
                'notes': [{'start': 1.5, 'end': 2.0, 'pitch': 60, 'hand': 'right'},
                          {'start': 1.5, 'end': 2.5, 'pitch': 48, 'hand': 'left'}]}
        midi = mido.MidiFile(file=io.BytesIO(midi_bytes(data)))
        self.assertEqual(len(midi.tracks), 3)
        self.assertEqual(midi.tracks[0][1].numerator, 3)
        self.assertEqual(midi.tracks[1][2].time, 480)
        self.assertEqual(midi.tracks[1][3].time, 480)
        self.assertEqual(midi.tracks[2][2].note, 48)
        self.assertEqual(midi.tracks[2][3].time, 960)

    def test_rejects_fractional_and_boolean_musical_integers(self):
        data = {'tempo': 120, 'bar_start': 1, 'numerator': 4, 'denominator': 4,
                'pickup_steps': 0,
                'notes': [{'start': 1, 'end': 2, 'pitch': 60, 'hand': 'right'}]}
        for field, invalid in [('tempo', 120.5), ('numerator', 3.5),
                               ('denominator', 4.5), ('pickup_steps', 1.5),
                               ('pickup_steps', True)]:
            with self.subTest(field=field, invalid=invalid), self.assertRaises(ValueError):
                midi_bytes({**data, field: invalid})
        with self.assertRaises(ValueError):
            midi_bytes({**data, 'notes': [{**data['notes'][0], 'pitch': 60.5}]})

    def test_overlapping_same_pitch_uses_separate_hand_channels(self):
        data = {'tempo': 120, 'bar_start': 0, 'numerator': 4, 'denominator': 4,
                'instrument': 'accordion',
                'notes': [{'start': 0, 'end': 1, 'pitch': 60, 'hand': 'right'},
                          {'start': 0.5, 'end': 1.5, 'pitch': 60, 'hand': 'left'}]}
        midi = mido.MidiFile(file=io.BytesIO(midi_bytes(data)))
        for track, expected in zip(midi.tracks[1:], [0, 1]):
            messages = [message for message in track if not message.is_meta]
            self.assertEqual([message.channel for message in messages], [expected] * 3)
            self.assertEqual(messages[0].program, 21)
        self.assertEqual(midi.tracks[1][2].time, 0)
        self.assertEqual(midi.tracks[2][2].time, 480)

    def test_rejects_note_before_bar_start(self):
        data = {'tempo': 120, 'bar_start': 1, 'numerator': 4, 'denominator': 4,
                'notes': [{'start': 0.5, 'end': 2, 'pitch': 60, 'hand': 'right'}]}
        with self.assertRaises(ValueError):
            midi_bytes(data)

    def test_rejects_more_than_10000_measures(self):
        data = {'tempo': 400, 'bar_start': 0, 'numerator': 1, 'denominator': 4,
                'notes': [{'start': 2000, 'end': 2001, 'pitch': 60, 'hand': 'right'}]}
        with self.assertRaisesRegex(ValueError, '10,000'):
            midi_bytes(data)

    def test_repeated_note_off_precedes_next_note_on(self):
        data = {'tempo': 120, 'bar_start': 0, 'numerator': 4, 'denominator': 4,
                'notes': [{'start': 0, 'end': 1, 'pitch': 60, 'hand': 'right'},
                          {'start': 1, 'end': 2, 'pitch': 60, 'hand': 'right'}]}
        track = mido.MidiFile(file=io.BytesIO(midi_bytes(data))).tracks[1]
        self.assertEqual([message.type for message in track[2:5]], ['note_on', 'note_off', 'note_on'])
        self.assertEqual(track[4].time, 0)

    def test_rejects_overlapping_same_pitch_in_one_hand(self):
        data = {'tempo': 120, 'bar_start': 0, 'numerator': 4, 'denominator': 4,
                'notes': [{'start': 0, 'end': 1, 'pitch': 60, 'hand': 'right'},
                          {'start': 0.5, 'end': 1.5, 'pitch': 60, 'hand': 'right'}]}
        with self.assertRaisesRegex(ValueError, '右手のMIDI 60'):
            midi_bytes(data)
        data['notes'].reverse()
        with self.assertRaisesRegex(ValueError, '右手のMIDI 60'):
            midi_bytes(data)

    def test_rejects_overlap_created_by_tick_rounding(self):
        data = {'tempo': 120, 'bar_start': 0, 'numerator': 4, 'denominator': 4,
                'notes': [{'start': 0, 'end': 0.0001, 'pitch': 60, 'hand': 'left'},
                          {'start': 0.0002, 'end': 0.5, 'pitch': 60, 'hand': 'left'}]}
        with self.assertRaisesRegex(ValueError, '左手のMIDI 60'):
            midi_bytes(data)


    @unittest.skipUnless(os.environ.get('ACORDE_TRANSCRIPTION_CONVERTER'), 'converter not built')
    def test_musicxml_contains_two_staves_and_accordion_instrument(self):
        data = {'tempo': 120, 'bar_start': 0, 'numerator': 3, 'denominator': 4,
                'instrument': 'accordion',
                'notes': [{'start': 0, 'end': 0.5, 'pitch': 60, 'hand': 'right'},
                          {'start': 0, 'end': 0.5, 'pitch': 48, 'hand': 'left'}]}
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'input.mid'
            output = Path(directory) / 'output.musicxml'
            source.write_bytes(midi_bytes(data))
            subprocess.run([os.environ['ACORDE_TRANSCRIPTION_CONVERTER'], source, output, '3', '4', '0'], check=True)
            root = ET.parse(output).getroot()
            self.assertEqual(root.findtext('./part-list/score-part/part-name'), 'Accordion')
            self.assertEqual(root.findtext('./part/measure/attributes/staves'), '2')
            self.assertEqual(root.findtext('./part/measure/attributes/clef[@number="2"]/sign'), 'F')
            pitches = {(n.findtext('pitch/step'), n.findtext('pitch/octave'), n.findtext('staff'))
                       for n in root.findall('./part/measure/note') if n.find('pitch') is not None}
            self.assertIn(('C', '4', '1'), pitches)
            self.assertIn(('C', '3', '2'), pitches)


    @unittest.skipUnless(os.environ.get('ACORDE_TRANSCRIPTION_CONVERTER'), 'converter not built')
    def test_musicxml_preserves_overlapping_unison_on_both_staves(self):
        data = {'tempo': 120, 'bar_start': 0, 'numerator': 4, 'denominator': 4,
                'instrument': 'accordion',
                'notes': [{'start': 0, 'end': 1, 'pitch': 60, 'hand': 'right'},
                          {'start': 0.5, 'end': 1.5, 'pitch': 60, 'hand': 'left'}]}
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'input.mid'
            output = Path(directory) / 'output.musicxml'
            source.write_bytes(midi_bytes(data))
            subprocess.run([os.environ['ACORDE_TRANSCRIPTION_CONVERTER'], source, output,
                            '4', '4', '0'], check=True)
            root = ET.parse(output).getroot()
            notes = [note for note in root.findall('./part/measure/note')
                     if note.findtext('pitch/step') == 'C' and note.findtext('pitch/octave') == '4']
            self.assertEqual(sorted(note.findtext('staff') for note in notes), ['1', '2'])
            if os.environ.get('ACORDE_TEST_BIN'):
                result = subprocess.run([os.environ['ACORDE_TEST_BIN'], 'playback-report', output],
                                        check=True, capture_output=True, text=True)
                events = json.loads(result.stdout)
                self.assertEqual([(event['pitch_midi'], event['time_secs'], event['duration_secs'])
                                  for event in events], [(60, 0, 1), (60, 0.5, 1)])

    @unittest.skipUnless(os.environ.get('ACORDE_TRANSCRIPTION_CONVERTER'), 'converter not built')
    def test_pickup_and_tie_crossing_first_full_bar(self):
        data = {'tempo': 120, 'bar_start': 1, 'pickup_steps': 4,
                'numerator': 4, 'denominator': 4,
                'notes': [{'start': 0.75, 'end': 1.25, 'pitch': 60, 'hand': 'right'}]}
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'input.mid'
            output = Path(directory) / 'output.musicxml'
            source.write_bytes(midi_bytes(data))
            midi = mido.MidiFile(source)
            self.assertEqual(midi.tracks[0][1].numerator, 4)
            self.assertEqual(midi.tracks[0][1].denominator, 16)
            self.assertEqual(midi.tracks[0][2].time, 480)
            subprocess.run([os.environ['ACORDE_TRANSCRIPTION_CONVERTER'], source, output,
                            '4', '4', '4'], check=True)
            root = ET.parse(output).getroot()
            measures = root.findall('./part/measure')
            self.assertEqual(measures[0].get('implicit'), 'yes')
            self.assertEqual(measures[0].findtext('./attributes/time/beats'), '4')
            self.assertEqual(measures[0].findtext('./attributes/time/beat-type'), '4')
            self.assertIsNotNone(measures[0].find('./note/tie[@type="start"]'))
            self.assertIsNotNone(measures[1].find('./note/tie[@type="stop"]'))
            if os.environ.get('ACORDE_TEST_BIN'):
                result = subprocess.run([os.environ['ACORDE_TEST_BIN'], 'playback-report', output],
                                        check=True, capture_output=True, text=True)
                events = json.loads(result.stdout)
                self.assertEqual(len(events), 1)
                self.assertEqual(events[0]['pitch_midi'], 60)
                self.assertAlmostEqual(events[0]['time_secs'], 0.25)
                self.assertAlmostEqual(events[0]['duration_secs'], 0.5)


if __name__ == '__main__':
    unittest.main()
