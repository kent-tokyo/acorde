import io
import json
import os
import subprocess
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

import mido
from server import midi_bytes


class MidiExportTest(unittest.TestCase):
    def test_preserves_time_meter_and_hands(self):
        data = {'tempo': 120, 'bar_start': 1.0, 'numerator': 3, 'denominator': 4,
                'notes': [{'start': 1.5, 'end': 2.0, 'pitch': 60, 'hand': 'right'},
                          {'start': 1.5, 'end': 2.5, 'pitch': 48, 'hand': 'left'}]}
        midi = mido.MidiFile(file=io.BytesIO(midi_bytes(data)))
        self.assertEqual(len(midi.tracks), 3)
        self.assertEqual(midi.tracks[0][1].numerator, 3)
        self.assertEqual(midi.tracks[1][3].time, 480)
        self.assertEqual(midi.tracks[1][3].time, 480)
        self.assertEqual(midi.tracks[2][2].note, 48)
        self.assertEqual(midi.tracks[2][3].time, 960)

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
