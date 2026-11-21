import json
import tempfile
import unittest
from pathlib import Path

import mido

from compare_corpus import compare
from rescale_reference import rescale


class CorpusComparisonTest(unittest.TestCase):
    def test_reference_rescale_rounds_absolute_ticks(self):
        with tempfile.TemporaryDirectory() as directory:
            source, output = Path(directory) / 'source.mid', Path(directory) / 'output.mid'
            midi = mido.MidiFile(ticks_per_beat=1000)
            track = mido.MidiTrack()
            midi.tracks.append(track)
            track.append(mido.Message('note_on', note=60, velocity=80, time=1))
            track.append(mido.Message('note_off', note=60, time=1000))
            midi.save(source)
            rescale(source, output, 500)
            result = mido.MidiFile(output)
            self.assertEqual(result.ticks_per_beat, 500)
            self.assertEqual([message.time for message in result.tracks[0][:2]], [0, 500])

    def test_aggregate_retains_per_excerpt_regressions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            def write_midi(name, pitches):
                midi = mido.MidiFile(ticks_per_beat=480)
                track = mido.MidiTrack()
                midi.tracks.append(track)
                for pitch in pitches:
                    track.append(mido.Message('note_on', note=pitch, velocity=80, time=0))
                    track.append(mido.Message('note_off', note=pitch, time=480))
                midi.save(root / name)

            write_midi('reference.mid', [60, 62])
            write_midi('baseline.mid', [60, 62])
            write_midi('candidate.mid', [60, 63])
            (root / 'audio.wav').write_bytes(b'example recording')
            manifest = root / 'manifest.json'
            manifest.write_text(json.dumps([{
                'id': 'sample', 'reference': 'reference.mid', 'baseline': 'baseline.mid',
                'candidate': 'candidate.mid', 'audio': 'audio.wav', 'start': 0, 'end': 2,
            }]))
            result = compare(manifest)
            excerpt = result['excerpts'][0]
            self.assertEqual(excerpt['baseline']['recall'], 1.0)
            self.assertEqual(excerpt['candidate']['recall'], 0.5)
            self.assertEqual(excerpt['candidate']['false_positive'], 1)
            self.assertEqual(excerpt['candidate']['missed'], 1)
            self.assertEqual(result['aggregate']['candidate']['matched'], 1)
            self.assertEqual(len(excerpt['sha256']['reference']), 64)
            self.assertEqual(len(excerpt['sha256']['audio']), 64)
            item = json.loads(manifest.read_text())[0]
            item['sha256'] = {'reference': 'incorrect'}
            manifest.write_text(json.dumps([item]))
            with self.assertRaisesRegex(ValueError, 'hash does not match'):
                compare(manifest)

    def test_rejects_duplicate_excerpt_ids(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'manifest.json'
            item = {'id': 'duplicate', 'reference': 'r', 'baseline': 'b', 'candidate': 'c'}
            path.write_text(json.dumps([item, item]))
            # The first item's files must exist before the duplicate is encountered.
            for name in ('r', 'b', 'c'):
                midi = mido.MidiFile()
                midi.tracks.append(mido.MidiTrack())
                midi.save(Path(directory) / name)
            with self.assertRaisesRegex(ValueError, 'unique'):
                compare(path)


if __name__ == '__main__':
    unittest.main()
