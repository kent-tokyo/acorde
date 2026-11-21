#!/usr/bin/env python3
"""Local audio transcription workspace backed by Basic Pitch and Acorde."""

import argparse
import io
import json
import math
import subprocess
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit
import xml.etree.ElementTree as ET

import mido

ROOT = Path(__file__).resolve().parent
MAX_AUDIO = 64 * 1024 * 1024
MAX_JSON = 4 * 1024 * 1024


def whole_number(value, name):
    if isinstance(value, bool):
        raise ValueError(f'{name}は整数で入力してください')
    try:
        number = float(value)
    except (TypeError, ValueError, OverflowError) as error:
        raise ValueError(f'{name}は整数で入力してください') from error
    if not math.isfinite(number) or not number.is_integer():
        raise ValueError(f'{name}は整数で入力してください')
    return int(number)


def midi_bytes(data):
    tempo = whole_number(data['tempo'], 'テンポ')
    origin = float(data['bar_start'])
    numerator = whole_number(data['numerator'], '拍子の分子')
    denominator = whole_number(data['denominator'], '拍子の分母')
    notes = data['notes']
    pickup_steps = whole_number(data.get('pickup_steps', 0), '弱起の長さ')
    instrument = data.get('instrument', 'piano')
    if instrument not in ('piano', 'accordion'):
        raise ValueError('Invalid instrument')
    if not (20 <= tempo <= 400 and math.isfinite(origin) and origin >= 0 and 1 <= numerator <= 32
            and denominator in (1, 2, 4, 8, 16, 32) and len(notes) <= 10000):
        raise ValueError('Invalid tempo, meter, start, or note count')
    if not 0 <= pickup_steps <= 255 or pickup_steps * denominator >= 16 * numerator:
        raise ValueError('Pickup must be shorter than a full bar')
    origin -= pickup_steps * 15 / tempo
    if origin < -0.000001:
        raise ValueError('Pickup begins before the recording')
    origin = max(0, origin)
    midi = mido.MidiFile(type=1, ticks_per_beat=480)
    conductor = mido.MidiTrack()
    midi.tracks.append(conductor)
    conductor.append(mido.MetaMessage('set_tempo', tempo=mido.bpm2tempo(tempo)))
    if pickup_steps:
        conductor.append(mido.MetaMessage('time_signature', numerator=pickup_steps, denominator=16))
        conductor.append(mido.MetaMessage('time_signature', numerator=numerator,
                                         denominator=denominator, time=pickup_steps * 120))
    else:
        conductor.append(mido.MetaMessage('time_signature', numerator=numerator, denominator=denominator))
    tracks = {'right': [], 'left': []}
    for note in notes:
        pitch = whole_number(note['pitch'], '音高')
        start = float(note['start'])
        end = float(note['end'])
        hand = note['hand']
        if hand not in tracks or not (0 <= pitch <= 127 and math.isfinite(start) and math.isfinite(end)
                                      and 0 <= start < end <= 36000 and start >= origin):
            raise ValueError('Invalid note or note before the selected bar start')
        start_tick = round((start - origin) * tempo * 480 / 60)
        end_tick = max(start_tick + 1, round((end - origin) * tempo * 480 / 60))
        tracks[hand].extend([(start_tick, 1, pitch), (end_tick, 0, pitch)])
    max_ticks = pickup_steps * 120 + (10_000 - bool(pickup_steps)) * 480 * 4 * numerator / denominator
    if any(tick > max_ticks for events in tracks.values() for tick, _, _ in events):
        raise ValueError('The selected notes exceed Acorde’s 10,000-measure limit')
    for hand, events in tracks.items():
        if not events:
            continue
        track = mido.MidiTrack()
        midi.tracks.append(track)
        channel = 0 if hand == 'right' else 1
        track.append(mido.MetaMessage('track_name', name=hand))
        track.append(mido.Message('program_change', channel=channel,
                                  program=21 if instrument == 'accordion' else 0))
        last = 0
        for tick, is_on, pitch in sorted(events):
            track.append(mido.Message('note_on' if is_on else 'note_off', channel=channel, note=pitch,
                                      velocity=80 if is_on else 0, time=tick - last))
            last = tick
    if len(midi.tracks) == 1:
        raise ValueError('Add at least one note')
    output = io.BytesIO()
    midi.save(file=output)
    return output.getvalue()


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if urlsplit(self.path).path != '/':
            self.send_error(404)
            return
        self.respond(200, 'text/html; charset=utf-8', (ROOT / 'index.html').read_bytes())

    def respond(self, status, kind, body, extra_headers=None):
        self.send_response(status)
        self.send_header('Content-Type', kind)
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Cache-Control', 'no-store')
        for name, value in (extra_headers or {}).items():
            self.send_header(name, value)
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        try:
            origin = self.headers.get('Origin')
            if origin != f'http://127.0.0.1:{self.server.server_port}':
                raise ValueError('Invalid request origin')
            size = int(self.headers.get('Content-Length', '0'))
            limit = MAX_AUDIO if self.path == '/transcribe' else MAX_JSON
            if size <= 0 or size > limit:
                raise ValueError('Invalid request size')
            payload = self.rfile.read(size)
            if self.path == '/transcribe':
                mode = self.headers.get('X-Transcription-Model', 'basic')
                low = int(self.headers.get('X-Pitch-Low', '0'))
                high = int(self.headers.get('X-Pitch-High', '127'))
                if not 0 <= low <= high <= 127:
                    raise ValueError('Invalid pitch range')
                with self.server.inference_lock, tempfile.TemporaryDirectory() as directory:
                    path = Path(directory) / 'input.wav'
                    path.write_bytes(payload)
                    import soundfile as sf
                    info = sf.info(str(path))
                    start = float(self.headers.get('X-Range-Start', '0'))
                    end = float(self.headers.get('X-Range-End', str(info.duration)))
                    if not (math.isfinite(start) and math.isfinite(end)
                            and 0 <= start < end <= info.duration + 0.01 and start < info.duration):
                        raise ValueError('Invalid audio range')
                    end = min(end, info.duration)
                    if start > 0 or end < info.duration:
                        samples, rate = sf.read(str(path), start=round(start * info.samplerate),
                                                stop=round(end * info.samplerate), always_2d=True)
                        sf.write(str(path), samples, rate)
                    if mode == 'piano':
                        if (not self.server.piano_model or not Path(self.server.piano_model).is_file()
                                or Path(self.server.piano_model).stat().st_size < 160_000_000):
                            raise ValueError('Piano model is not configured on the server')
                        import librosa
                        from piano_transcription_inference import PianoTranscription
                        if self.server.piano is None:
                            self.server.piano = PianoTranscription(checkpoint_path=self.server.piano_model, device='cpu')
                        audio, _ = librosa.load(path, sr=16000, mono=True)
                        result = self.server.piano.transcribe(audio, None)
                        events = [(n['onset_time'], n['offset_time'], n['midi_note'])
                                  for n in result['est_note_events']]
                    elif mode == 'basic':
                        from basic_pitch import FilenameSuffix, build_icassp_2022_model_path
                        from basic_pitch.inference import Model, predict
                        if self.server.basic is None:
                            self.server.basic = Model(build_icassp_2022_model_path(FilenameSuffix.onnx))
                        _, _, raw = predict(
                            str(path), self.server.basic,
                            minimum_frequency=440 * 2 ** ((low - 69 - 0.5) / 12) if low else None,
                            maximum_frequency=440 * 2 ** ((high - 69 + 0.5) / 12) if high < 127 else None,
                        )
                        events = [(a, b, p) for a, b, p, _, _ in raw]
                    else:
                        raise ValueError('Unknown transcription model')
                notes = [{'start': float(a) + start, 'end': min(float(b) + start, end), 'pitch': int(p),
                          'hand': 'left' if p < 60 else 'right'} for a, b, p in events if a < end - start and b > a and low <= p <= high]
                self.respond(200, 'application/json', json.dumps({'notes': notes}).encode())
            elif self.path in ('/midi', '/musicxml', '/preview'):
                data = json.loads(payload)
                midi = midi_bytes(data)
                if self.path == '/midi':
                    self.respond(200, 'audio/midi', midi)
                else:
                    with tempfile.TemporaryDirectory() as directory:
                        source = Path(directory) / 'input.mid'
                        output = Path(directory) / 'output.musicxml'
                        source.write_bytes(midi)
                        result = subprocess.run([self.server.score_converter, str(source), str(output), str(whole_number(data['numerator'], '拍子の分子')),
                                                 str(whole_number(data['denominator'], '拍子の分母')),
                                                 str(whole_number(data.get('pickup_steps', 0), '弱起の長さ'))],
                                                capture_output=True, text=True, timeout=60)
                        if result.returncode:
                            raise ValueError(result.stderr.strip() or 'Acorde conversion failed')
                        if self.path == '/preview':
                            svg = Path(directory) / 'score.svg'
                            rendered = subprocess.run([self.server.acorde, 'render', str(output), str(svg),
                                                       '--measures-per-system', '4', '--staff-size', '14'],
                                                      capture_output=True, text=True, timeout=60)
                            if rendered.returncode:
                                raise ValueError(rendered.stderr.strip() or 'Acorde rendering failed')
                            root = ET.fromstring(output.read_bytes())
                            first_part = root.find('part')
                            count = len(first_part.findall('measure')) if first_part is not None else 0
                            self.respond(200, 'image/svg+xml', svg.read_bytes(),
                                         {'X-Measure-Count': str(count)})
                        else:
                            self.respond(200, 'application/vnd.recordare.musicxml+xml', output.read_bytes())
            else:
                self.send_error(404)
        except (ValueError, KeyError, TypeError, OSError, subprocess.SubprocessError) as error:
            self.respond(400, 'application/json', json.dumps({'error': str(error)}).encode())
        except Exception as error:
            self.respond(500, 'application/json', json.dumps({'error': str(error)}).encode())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', type=int, default=8765)
    parser.add_argument('--piano-model', help='path to the piano model checkpoint')
    parser.add_argument('--score-converter', default='target/debug/examples/transcription_score')
    parser.add_argument('--acorde', default='target/debug/acorde')
    args = parser.parse_args()
    server = ThreadingHTTPServer(('127.0.0.1', args.port), Handler)
    server.score_converter = args.score_converter
    server.acorde = args.acorde
    server.piano_model = args.piano_model
    server.inference_lock = threading.Lock()
    server.basic = None
    server.piano = None
    print(f'Open http://127.0.0.1:{server.server_port}/', flush=True)
    server.serve_forever()


if __name__ == '__main__':
    main()
