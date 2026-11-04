# Eight-score audio corpus against alphaTab 1.8.4 (2026-11-04)

Issue #203 asks for same-input evidence across rests, overlapping voices, tempo changes, dynamics,
pedal, ornaments and instrument changes, with import losses kept apart from synthesis failures.
Both renderers used the same score files, alphaTab 1.8.4's bundled `sonivox.sf2` (SHA-256
`4978cf37e7164206f2d22dc4fdf4020ffe040787210bf781e31bdc79961f59ef`), 48 kHz and stereo output.
Acorde used `audio-render --full-sf2 --realize-ornaments`; alphaTab used its public exporter via
`scripts/export_alphatab_audio.mjs`. `scripts/compare_audio_corpus.py` measured both against
the same `acorde playback-report` schedule. No WAV or float file is committed; hashes are below.

## Inputs and coverage

| Score | Events | Onsets | Import diagnostics | Exercises |
|---|---:|---:|---:|---|
| `simple` | 5 | 5 | 0 | single line |
| `multipart` | 4 | 3 | 0 | 2 parts |
| `openscore_lieder_aloha_oe` | 644 | 157 | 73 | overlapping voices, rests ≥300 ms, dynamics, programs 0,52,68, 4 parts |
| `openscore_lieder_just_for_today` | 579 | 151 | 32 | overlapping voices, rests ≥300 ms, dynamics, accent/fermata, programs 0,68, 2 parts |
| `openscore_lieder_quilter_prelude` | 181 | 69 | 103 | overlapping voices, dynamics, accent |
| `openscore_lieder_webern_so_ich_traurig_bin` | 132 | 67 | 22 | overlapping voices, tempo changes, dynamics, pedal, accent/staccato/tenuto, programs 0,68, 2 parts |
| `issue_311_trill` | 1 | 1 | 0 | trill |
| `issue_203_instrument_change` | 2 | 2 | 0 | rests ≥300 ms, programs 0,40 |

Import diagnostics are input losses reported by `acorde report`; they are not synthesis failures.
Every score rendered completely in both renderers; none failed synthesis.

## Onsets

Measured attack minus scheduled onset, in ms, from 5 ms log-energy rises (windows clamped to
halfway to neighbouring onsets). The paired column is alphaTab minus Acorde for the same onset,
which cancels the shared SF2 attack shape.

| Score | Acorde median abs / p95 (missed) | alphaTab median abs / p95 (missed) | Paired median / median abs (n) |
|---|---|---|---|
| `simple` | 5.0 / 5.0 (0) | 5.0 / 5.0 (0) | 0.0 / 0.0 (5) |
| `multipart` | 160.0 / 245.0 (0) | 160.0 / 245.0 (0) | 0.0 / 0.0 (3) |
| `openscore_lieder_aloha_oe` | 30.0 / 175.0 (2) | 30.0 / 180.0 (9) | 0.0 / 57.5 (146) |
| `openscore_lieder_just_for_today` | 37.4 / 185.2 (8) | 52.1 / 223.0 (9) | 7.5 / 72.5 (134) |
| `openscore_lieder_quilter_prelude` | 49.6 / 190.7 (2) | 15.1 / 194.6 (1) | 0.0 / 25.0 (66) |
| `openscore_lieder_webern_so_ich_traurig_bin` | 31.9 / 205.0 (3) | 33.6 / 240.5 (6) | 0.0 / 50.0 (59) |
| `issue_311_trill` | 5.0 / 5.0 (0) | 190.0 / 190.0 (0) | 185.0 / 185.0 (1) |
| `issue_203_instrument_change` | 2.5 / 5.0 (0) | 2.5 / 5.0 (0) | 0.0 / 0.0 (2) |

## Level, release and end of buffer

| Score | Acorde peak / full-scale | alphaTab peak / ≥1.0 / non-finite | Level 250 ms into rests, Acorde / alphaTab (dB) | Buffer beyond scheduled end, Acorde / alphaTab (s) | Level at buffer end, alphaTab (dB) |
|---|---|---|---|---|---|
| `simple` | 0.075897 / 0 | 0.592075 / 0 / 0 | — / — | 1.0 / 1.0 | -180.0 |
| `multipart` | 0.082001 / 0 | 0.591311 / 0 / 0 | — / — | 1.0 / 0.001 | -19.1 |
| `openscore_lieder_aloha_oe` | 0.553558 / 0 | 1.244791 / 76 / 0 | -49.4 / -25.0 | 1.0 / 1.876 | -180.0 |
| `openscore_lieder_just_for_today` | 0.220276 / 0 | 0.572022 / 0 / 0 | -49.3 / -19.0 | 1.0 / -4.89 | -23.0 |
| `openscore_lieder_quilter_prelude` | 0.182343 / 0 | 0.484158 / 0 / 0 | — / — | 1.0 / 0.0 | -38.2 |
| `openscore_lieder_webern_so_ich_traurig_bin` | 0.158081 / 0 | 0.303484 / 0 / 0 | — / — | 1.0 / -0.594 | -180.0 |
| `issue_311_trill` | 0.064087 / 0 | 0.719842 / 0 / 0 | — / — | 1.0 / 0.001 | -21.4 |
| `issue_203_instrument_change` | 0.047882 / 0 | 0.34503 / 0 / 0 | -38.8 / -38.0 | 1.0 / 1.0 | -180.0 |

## Findings

- **Onset timing does not separate the renderers on dense scores.** On the four OpenScore lieder the
  paired median difference is 0–7.5 ms, but the paired median absolute difference is 25–72.5 ms
  and Acorde's own measured error against its own schedule is 30–50 ms. That spread is the
  detector's resolution in polyphony, not a timing difference. alphaTab is better on Quilter's
  median, Acorde on Just for Today's. No onset claim is made either way. The known 749 ms split-bar
  lag in Aloha Oe is a local event and does not move these medians.
- **alphaTab truncates sounding output.** *Just for Today* ends 4.89 s before the scheduled end
  with −23 dB still sounding; `multipart` and the trill end at −19 and −21 dB. Acorde renders a
  1 s tail past the scheduled end on every score and ends in silence.
- **Release into rests differs.** 250 ms into a scheduled rest, Acorde is at about −49 dB and
  alphaTab at −19 to −25 dB on the two lieder with rests. This measures decay policy, not
  which one sounds better; it is a candidate for the listening check.
- **Headroom.** alphaTab's Aloha Oe export has 76 samples at or beyond full scale; Acorde has none
  on any score. Acorde's default gain is much lower (peak 0.05–0.55); `--normalize-peak-dbfs`
  levels it when needed.
- **Ornaments.** With `--realize-ornaments` Acorde plays the trill as 4 events; alphaTab's single
  attack is 185 ms later than Acorde's on that note.
- **Instrument changes.** This corpus exposed that Acorde ignored a mid-part MusicXML
  `<sound><midi-instrument>` program change. It is now imported as a part program change, so
  Acorde plays bar 2 of `issue_203_instrument_change.musicxml` with program 41 (violin). Whether
  alphaTab switches is not measured by these metrics. MusicXML export still reports mid-part
  program changes as unsupported.

No general audio-quality superiority is claimed. The blinded listening check is separate and
requires human listeners.

## Output hashes

| Score | Acorde WAV SHA-256 | alphaTab float32 SHA-256 |
|---|---|---|
| `simple` | `5924323107446e40972f5eddd46ea1aae265bdb437353bda716c52886a2edd02` | `781fc384d6480e691ebe9de12d3ef0c45680d0dbd00257a833fb084d3e96b57c` |
| `multipart` | `820452a38f1b9135c222cd1a93049ca100e973a4504a8cf67d59723b45d4b97e` | `f8a558b7412fca3bf0bc90cbdfad752aa5bbf2f305c2ebb168fdb615108171a8` |
| `openscore_lieder_aloha_oe` | `5a89d3a38d5e97e534ada78938b246c0f6074606724aa2cddbf3d0c26e8e53d6` | `6c91393e3f60723766ec3676a57f74bcbf1bda132a0412c2020800919217d3a1` |
| `openscore_lieder_just_for_today` | `2be34cabd461d1463b5b45143a9e2bf5cba68b2eb844b9df2e3b55a08b1e846e` | `e35b715158b49ea6852e3844a2d69840d50badaf6ca2736e8ef86a645cdb0ef5` |
| `openscore_lieder_quilter_prelude` | `3a7bcd06480254f3b49bc50ddf6c01ced2e938fee8a037c424891a4b927a0d36` | `743bc801612aa699e476f3b61c91a49e997bf0ce67f5230a461281daf59e33a1` |
| `openscore_lieder_webern_so_ich_traurig_bin` | `9ec72f1052554f5c520d2dab94f8557020ad5d61d63734db052a6037d36872df` | `5d59d08a43e7a91e3d855340c3ec54a2303a853239996f84dbd19feea42b7c43` |
| `issue_311_trill` | `e0bf8cc02d8ea76ecf77cc4c2693c009eebaf49024e48795bd027d0f7b71406f` | `99f141fb0fb67d8794f093b9ed73374d77b2910b6e5163b199fb8ad80c29ab57` |
| `issue_203_instrument_change` | `7d68ad6e9fbfcca3aba6084a25569813595eb69c32cea9b7a13f805fcbc7277d` | `b6be32ee84ac38b004f821be0e7d7307ea493c4ec110da9d6576e2a8d817941d` |

## Reproduction

```sh
npm install --prefix /tmp/acorde-alphatab-1_8_4 --no-save --no-package-lock @coderline/alphatab@1.8.4
SF2=/tmp/acorde-alphatab-1_8_4/node_modules/@coderline/alphatab/dist/soundfont/sonivox.sf2
for f in simple.musicxml multipart.musicxml openscore_lieder_aloha_oe.mxl \
    openscore_lieder_just_for_today.mxl openscore_lieder_quilter_prelude.mxl \
    openscore_lieder_webern_so_ich_traurig_bin.mxl issue_311_trill.musicxml \
    issue_203_instrument_change.musicxml; do
  n=${f%.*}
  acorde audio-render tests/fixtures/$f "$SF2" /tmp/$n.wav --full-sf2 --realize-ornaments \
    --sample-rate 48000 --channels 2
  node scripts/export_alphatab_audio.mjs /tmp/acorde-alphatab-1_8_4 tests/fixtures/$f "$SF2" \
    /tmp/$n.f32le 48000
  acorde playback-report tests/fixtures/$f > /tmp/$n.json
  acorde report tests/fixtures/$f > /tmp/$n.report.json
  specs="$specs $n:/tmp/$n.json:/tmp/$n.wav:/tmp/$n.f32le:/tmp/$n.report.json"
done
python3 scripts/compare_audio_corpus.py /tmp/acorde-audio-corpus.json $specs
```
