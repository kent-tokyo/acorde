# Eight-score audio observation against alphaTab 1.9.0 (2026-11-07)

This repeats the [earlier eight-score corpus](playback-audio-corpus.md) with
alphaTab 1.9.0. Acorde is `d5b47b447ceca1c5e4f286ef236bfccf643a6764`
(v1.3.10). All eight inputs are committed fixtures with provenance in
`tests/fixtures/manifest.json`. The renderers read the same score file and used
alphaTab's `sonivox.sf2` (SHA-256
`4978cf37e7164206f2d22dc4fdf4020ffe040787210bf781e31bdc79961f59ef`),
48 kHz and stereo. Acorde used `audio-render --full-sf2 --realize-ornaments`;
alphaTab used its public exporter through `scripts/export_alphatab_audio.mjs`.
The complete machine-readable measurements, including output hashes and import
diagnostic codes, are in [playback-audio-corpus-alpha19.json](playback-audio-corpus-alpha19.json).
Raw audio is not committed.

## Measured output

Attack figures are median absolute / 95th-percentile absolute milliseconds
against the `acorde playback-report` schedule. Parentheses give undetected
attacks. The detector uses 5 ms log-energy rises near distinct scheduled
onsets; dense chords and overlapping voices limit its accuracy.

| Score | Acorde attack ms (missed) | alphaTab attack ms (missed) | alphaTab buffer minus Acorde scheduled end | Import diagnostics |
| --- | ---: | ---: | ---: | ---: |
| simple | 5 / 5 (0) | 5 / 5 (0) | +1.000 s | 0 |
| multipart | 160 / 245 (0) | 160 / 245 (0) | +0.001 s | 0 |
| Aloha Oe | 30 / 175 (2) | 30 / 175 (8) | +1.876 s | 73 |
| Just for Today | 43.7 / 187.6 (6) | 52.1 / 223 (9) | -4.890 s | 32 |
| Quilter Prelude | 49.6 / 190.7 (2) | 15.1 / 194.6 (1) | 0 s | 104 |
| Webern, So ich traurig bin | 31.9 / 205 (3) | 33.6 / 235.5 (8) | -0.595 s | 23 |
| issue_311_trill | 5 / 5 (0) | 190 / 190 (0) | +0.001 s | 0 |
| issue_203_instrument_change | 2.5 / 5 (0) | 2.5 / 5 (0) | +1.000 s | 0 |

In Aloha Oe, alphaTab's float output peaks at 1.244791 and contains 76 samples
above unit amplitude; Acorde peaks at 0.553558 with none. Both outputs
contain no non-finite samples. For Just for Today, alphaTab's buffer stops
4.89 seconds before Acorde's scheduled end with a measured end level of -23 dB.
These are output observations. The source timing and import losses need an
independent audit before attributing the shorter buffer to missing music.

The schedule comes from Acorde, so the attack comparison cannot independently
validate Acorde's event timing. It also does not establish audible quality.
The paired difference only helps screen individual onsets for further review;
there is no general win on this corpus. [Issue #388](https://github.com/kent-tokyo/acorde/issues/388)
still requires at least two blind listeners for the pinned 1.8.4 corpus; a
separate blind review is needed for 1.9.0.

## Reproduction

Install `@coderline/alphatab@1.9.0` into a temporary prefix and use its bundled
soundfont. Build `acorde-cli` at the commit above. For each of the eight fixture
filenames in the [earlier corpus](playback-audio-corpus.md#inputs-and-coverage),
produce the WAV, alphaTab float file, playback report and import report with
the same commands shown in that document, passing `1.9.0` as the last argument
to `export_alphatab_audio.mjs`. Then run `compare_audio_corpus.py` over the eight
specifications. The version argument is mandatory for 1.9.0; omitting it keeps
the archived 1.8.4 check.
