#!/usr/bin/env python3
"""Generate the fixed 30–60 second Sight to Sound continuation comparisons."""

import argparse
from pathlib import Path
import subprocess

import librosa
import pretty_midi
from piano_transcription_inference import PianoTranscription


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('dataset', type=Path, help='unpacked MIDItest directory')
    parser.add_argument('output', type=Path)
    parser.add_argument('--piano-checkpoint', type=Path, required=True)
    parser.add_argument('--transkun', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    model = PianoTranscription(checkpoint_path=str(args.piano_checkpoint), device='cpu')
    for ident in ('10', '21', '23', '24', '25', '26', '27', '5'):
        video = args.dataset / 'miditest_videos' / f'{ident}.mp4'
        wave = args.output / f'{ident}.wav'
        subprocess.run(['ffmpeg', '-nostdin', '-v', 'error', '-y', '-i', str(video),
                        '-ss', '30', '-t', '30', '-ac', '1', '-ar', '16000',
                        '-c:a', 'pcm_s16le', str(wave)], check=True)
        audio, _ = librosa.load(wave, sr=16000, mono=True)
        piano = args.output / f'{ident}.piano.raw.mid'
        transkun = args.output / f'{ident}.transkun.raw.mid'
        model.transcribe(audio, str(piano))
        subprocess.run([str(args.transkun), str(wave), str(transkun), '--device', 'cpu'],
                       check=True)
        for source, label in ((piano, 'piano'), (transkun, 'transkun')):
            midi = pretty_midi.PrettyMIDI(str(source))
            for instrument in midi.instruments:
                for note in instrument.notes:
                    note.start += 30
                    note.end += 30
                for event in instrument.control_changes:
                    event.time += 30
                for event in instrument.pitch_bends:
                    event.time += 30
            midi.write(str(args.output / f'{ident}.{label}.mid'))


if __name__ == '__main__':
    main()
