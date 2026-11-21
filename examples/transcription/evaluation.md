# Initial transcription observation

One development recording was evaluated with two models under the same conditions. This is a
single training-split recording and does not establish accuracy on unseen performances.

- Dataset: MAESTRO v3.0.0, train split, 2011 recording of J. S. Bach, Prelude and Fugue in G Minor,
  WTC II, BWV 885, `MIDI-Unprocessed_22_R1_2011_MID--AUDIO_R1-D8_12_Track12_wav`.
- Source: https://magenta.tensorflow.org/datasets/maestro, licensed CC BY-NC-SA 4.0. The audio and
  reference MIDI are not included in this repository.
- Audio duration: 45.169 seconds. Audio SHA-256:
  `e57fe6d33ac8026359fc16888ec47e7e26c254dc73a237adc98a2bd23d7d71bd`.
- Reference MIDI SHA-256: `e2ba82596c757411e06ee2ca182567c02dd2e7ebe2ef7f1b18dc8e40044428eb`.
- Basic Pitch 0.4.0, ONNX model, default thresholds. Output SHA-256:
  `5a39adf450c1c4140feedbc8b0d4df47d9c7d8442bf5447616627ba1c7e48977`.
- Piano Transcription Inference 0.0.6, default thresholds, CPU. Checkpoint SHA-256:
  `c3fa9730725bf4a762f1c14bc80cd5986eacda01b026f5a4a2525cd607876141`.
  Output MIDI SHA-256: `a18403ce70ffbc4db9890bfcd8209189e69c332b59bcc25f41a2cdb5123f619c`.
- `evaluate.py`, mir_eval 0.8.2, matches note pitch within 50 cents and onset within 50 ms.
  Offset matching uses 20% of reference note duration or 50 ms, whichever is greater. The
  reference uses MIDI key release times; pedal-extended acoustic durations can differ.

| Model | Reference / estimated notes | Onset precision | Onset recall | Onset F1 | Onset + offset F1 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Basic Pitch | 129 / 182 | 0.599 | 0.845 | 0.701 | 0.206 |
| Piano model | 129 / 124 | 1.000 | 0.961 | 0.980 | 0.656 |

No independent-source or phone recording, accordion performance, or correction-time study is
included in this observation. A separate held-out corpus is needed to evaluate generalization.

A second observation uses MAESTRO v3.0.0 test split, Domenico Scarlatti, Sonata K. 525,
2008 `MIDI-Unprocessed_09_R3_2008_01-07_ORIG_MID--AUDIO_09_R3_2008_wav--2`.
This recording's relationship to the training split used by each model has not been audited.
Audio SHA-256 `8037f5a09bf1b1108893d664d25e82b9900aff805f3dfee3b075afaaf35474b0`.
Reference MIDI SHA-256 `fbff7b81e8eb9159e52f0a9f46f3fef1cbe14cb0737dca137d3b52498a2b58cc`.
The settings and metric are the same as above.

| Model | Reference / estimated notes | Onset precision | Onset recall | Onset F1 | Onset + offset F1 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Basic Pitch | 882 / 604 | 0.798 | 0.546 | 0.649 | 0.040 |
| Piano model | 882 / 854 | 0.982 | 0.951 | 0.967 | 0.757 |

Basic Pitch output SHA-256 `b17040acb12a8970f93ad25f15e0b108e269f83b75d16119480adb02e0b3118a`.
Piano output SHA-256 `c2e87d355bdf99b60d23c02a976db564b7d87c56bd77535f8cf3ed206d80565f`.

## Accordion single-note screening

Eight natural accordion samples from [TinySOL 5.0](https://zenodo.org/records/3685331) were
selected in archive order. TinySOL is licensed CC BY 4.0 and its creators are Carmine-Emanuele
Cella, Daniele Ghisi, Vincent Lostanlen, Fabien Lévy, Joshua Fineberg and Yan Maresz. The source
samples are not included here. This is a convenience sample of isolated sounds and does not
measure melody, left-hand chords, register changes, or performance in a room.

With Basic Pitch 0.4.0 and its default settings, all eight contained the labeled pitch. Across
the eight samples the model returned 47 note events, of which 37 had a different pitch from
the sample label. Setting its minimum frequency to 100 Hz returned 15 events, including 10
at the labeled pitches and five at an octave above E3. Repeated events at the same correct
pitch are counted separately. The 100 Hz setting is suitable for this specific set of notes;
it excludes actual notes below that frequency.

| Sample | SHA-256 |
| --- | --- |
| Acc-ord-D4-pp-alt2-N.wav | `68422e6e1ea69c19bb59ae312953f9d3d8ba40a77ac26f985dfb843e816ce3e3` |
| Acc-ord-E3-mf-alt4-N.wav | `9e1b9f57d24d68260084edda167a3497029a9fbd54d70aab272fd567885fcf1b` |
| Acc-ord-F#4-mf-alt2-N.wav | `0282cab26aadef15c8e84fd88e2189bae376a88c5e65d35a4a7823ae1f00daf6` |
| Acc-ord-F#4-pp-N-N.wav | `108a6f334ab3fed1d985ab9900f6d7e74b2b582bcd00175cfcb59b28ed36a058` |
| Acc-ord-F#6-pp-alt1-N.wav | `f4603be530abd52ed7556e7c0b8b4c2360fb353e7b7e3903192c42a06360aef6` |
| Acc-ord-F5-mf-alt1-N.wav | `c62afdceef87bfc4573c5193cf11d7256a6b93a0f6336055c73d695dcbae29e1` |
| Acc-ord-G#3-mf-N-N.wav | `ae2cb9159c007652842202e815ba9786652046dfa61c5d75f1b4ba2cc0244119` |
| Acc-ord-G#5-mf-alt4-N.wav | `6e2eee17e577cca98b36b6c02bd2d5f1619903cce2c7791214d6a09f99b0f161` |

Three TinySOL samples (E3, G♯3 and D4, listed above) were level-normalized and mixed with
roughly aligned attacks as a synthetic E7 voicing without a fifth. This is a constructed
mixture, not a recorded chord or a two-hand accordion performance. With a 100 Hz lower
frequency limit, Basic Pitch returned pitches 52, 56 and 62. Their detected starts were
0.06 s, 0.05 s and 0.18 s. The generated WAV SHA-256 was
`45b8cd4d846b3ccae1691bb44c33008c9478b3a60306f2355c31d6ffc2402205`.
The 0.2 s grouping and omitted-fifth dominant-seventh candidate were added after inspecting
this mixture, so this example is development evidence.

A real solo accordion recording, [accordion melody 17 by PhonosUPF](https://freesound.org/people/PhonosUPF/sounds/485810/),
was used for a functional check. The source is CC0. The published high-quality MP3 preview was
converted to 22.05 kHz mono WAV and truncated to the first 10 seconds. The MP3 SHA-256 is
`db184feacd93899b9beab2451bc3d5df5f6351927aa277359fbb1cea7bb8fe16` and the derived WAV
SHA-256 is `f4f58c4c9b52184b1d57d98d2450a8c8068638bfe6863af1d93909f7f8e98105`.
Basic Pitch returned 11 events through the local API when limited to MIDI pitches 48–96. No
aligned reference or register annotation was supplied, so neither accuracy nor right/left
assignment was measured for this recording.

The actual recording [Accordion C minor chord.wav by juskiddink](https://freesound.org/people/juskiddink/sounds/120690/)
was also checked. The creator describes one C minor chord played on a Paolo Soprani 80-bass
accordion. The sample is CC BY 4.0. Its public MP3 preview was converted to 22.05 kHz mono WAV.
The MP3 SHA-256 is `058311306d9839c5bd6a186f68828fcb0545660c445917c69789cb9828bdadbc`,
and the WAV SHA-256 is `4c6fdfaaf54289d3a7870d687632cdd9b006310fab0427973cbc4fa1926991fc`.
Basic Pitch returned MIDI pitches 67, 55, 67, 63 and 60. The user-assigned left-hand range
produced a Cm candidate in the browser. The source provides a chord label, not note-level
onset, offset, or register annotations.

Three additional public MP3 previews by juskiddink, all CC BY 4.0, were converted to 22.05 kHz
mono WAV and analyzed with default Basic Pitch settings. All detected notes were manually
assigned to the left hand before grouping. The chord labels below are the source labels, not
independent note-by-note annotations.

| Source | Preview SHA-256 | Browser observation |
| --- | --- | --- |
| [C major](https://freesound.org/people/juskiddink/sounds/120689/) | `6d6d72fe4f4d37955adcb9a9eaca6421a02fcbf628cfc1f44303066f8b070e1a` | C at 0.01 and 3.36 s, corresponding to the two takes described by the source |
| [C diminished](https://freesound.org/people/juskiddink/sounds/120688/) | `4310dc02800e30a49885e7cdc3356806ef1dae0967667d1ccef9ad630cb84178` | Cdim at 0.02 s |
| [C-sharp seventh](https://freesound.org/people/juskiddink/sounds/120691/) | `d9ef886e52d145294f99dd2e7bc1287074fc4a8623de7f176702bb9f62b96447` | C♯7 at 0.09 s with 0.4 s grouping, plus later C♯ and C♯7 candidates during the held chord |

The grouping window was made adjustable after inspecting the C-sharp seventh recording.
The additional candidates show that repeated false attacks still need review. These three
recordings use the same instrument and recording source as the C minor sample.

## iPhone piano recordings

Three room 1 recordings from [IDMT-PIANO-MM 1.0.0](https://zenodo.org/records/7544248)
were recorded using an iPhone 6S Plus. The dataset is by Jakob Abeßer, Franca Bittner,
Maike Richter, Marcel Gonzalez and Hanna Lukashevich, Fraunhofer IDMT, CC BY-NC-ND 4.0.
The source audio and MIDI remain outside this repository. The authors state MIDI onsets have
approximately 50 ms alignment tolerance and offsets were not checked for accuracy. Results
below use pitch and onset only, with 50 ms onset and 50 cent pitch tolerances. They cover one
room and one device and do not establish performance across phones or recording conditions.

| Recording | Reference notes | Basic Pitch P / R / F1 | Piano model P / R / F1 |
| --- | ---: | --- | --- |
| Scale | 88 | 0.793 / 0.739 / 0.765 | 0.305 / 0.727 / 0.430 |
| Chords | 54 | 0.619 / 0.722 / 0.667 | 0.828 / 0.981 / 0.898 |
| Beethoven 1 | 212 | 0.596 / 0.821 / 0.690 | 0.727 / 0.967 / 0.830 |

The piano model emitted 210 events on the 88-note scale reference. It is not uniformly better
on these recordings. No onset-offset metric is reported for this dataset.

| Recording | Source WAV SHA-256 | Reference MIDI SHA-256 |
| --- | --- | --- |
| Scale | `bb84f851a1393492679757819e13da4f8db7bdf730253a4174e3a709a7a7df79` | `9319dc19ef3ad9705db194f0846b2a062636416cc590ed1d2b975be66b06b1b1` |
| Chords | `d1239204bec41a273300608b372284a9a7586b5c615b9824950c8a3e43e4d6fb` | `d4b92faf03fc4d14bd2f8158a40b23add017afe9b2d2aa569ce5e74c1fb83356` |
| Beethoven 1 | `f8267f9416bd39304d7920ce90fa80f13932ed927fc5b70c31cd7ee9d529e45f` | `8b24a67aea7acf2dde747c29fa082fcd7d8f42431f4e4382a7f8cd764e2edbaa` |

The source names are `ROOM_room1_MUS_{scale,chords,beethoven1}_DEV_iphone.wav` with matching
`ROOM_room1_MUS_{scale,chords,beethoven1}.mid` files. Both models used the versions and default
settings stated above. Outputs can be evaluated with `evaluate.py`; its offset statistics must
be disregarded for this dataset.

The 10–20 second ranges of the scale and Beethoven recordings were also submitted to the local
range API with pitch range 0–127 and up to two seconds of audio context on either side. The
raw notes returned by the API were compared with reference note onsets in the same range;
boundary review candidates were not added. These are development observations on portions of
the recordings already evaluated above, with the same 50 ms alignment limitation. Offset
accuracy was not measured.

| Recording | Model | Reference / estimated notes | Onset precision / recall / F1 | Boundary candidates |
| --- | --- | ---: | --- | ---: |
| Scale | Basic Pitch | 27 / 30 | 0.833 / 0.926 / 0.877 | 0 |
| Scale | Piano | 27 / 30 | 0.800 / 0.889 / 0.842 | 0 |
| Beethoven 1 | Basic Pitch | 45 / 68 | 0.618 / 0.933 / 0.743 | 1 |
| Beethoven 1 | Piano | 45 / 51 | 0.863 / 0.978 / 0.917 | 0 |

No context-limited releases were reported. The response JSON SHA-256 values in table order are
`4b7f189552d6cccff7c468709c7b13e74812b0d4902a0f79c10e317b261347f0`,
`74782361ce370bf1def9ace517ed882b512044e8d6d7d7cc649335ee4ba3a4ad`,
`879bb42611c146a3bc93be7d45396f177856d6a3ad9a169ae0cfd9b0ca2f386f`, and
`5618fccef7f50af4bc3b128b3f543576aee5051e9d016a1e3522c15ffb275682`.
These short ranges cannot establish overall phone-recording accuracy or correction time. In
particular, the piano model's result on the selected scale range is substantially different
from its result on the complete scale recording above.

## Same melody across synthesized accordion registers

A 16-note, four-bar melody at 120 BPM was rendered twice from the same MIDI with Acorde
`audio-render --full-sf2`, 22.05 kHz mono. The source was Ziya Mete Demircan's
[HedSound Accordion 5 Choir](https://hedsounds.blogspot.com/2021/01/hedsound-accordion-5-choir.html),
whose included license states free personal use. It uses synthesized waveforms and is not a
recording of a physical accordion. The SoundFont's `Clarinet M` bank 15 and `Musette-Auth. mMm`
bank 18 presets were separately copied to bank 0, program 21 for the renderer. Both variants use
the same MIDI and rendering options. The source SoundFont and audio are not distributed here.

The source SoundFont SHA-256 is `589dfd239500983cf6ad4c5372d4186c11a45e21b7d72a8c81b00e8e6f2127c9`.
The input MIDI SHA-256 is `17037fd1697310b805b203c4f024f798803afa6e74302e43750125fa33391e5a`.
The WAV hashes are `f1ebf3236e0070df6fa0519ae33979bbf79da991644bda08f89b094c2790369c`
for Clarinet and `98eeb55df6f193382d4464abf0b562d489b5c7b21cf5f322700ddef6038bd858` for Musette.
Basic Pitch 0.4.0 used a 200 Hz minimum frequency, below every melody note. Both outputs contained
16 events. Onset and pitch precision/recall/F1 were 1.000 for Clarinet and 0.938 for Musette.
Onset-plus-offset F1 was 0.688 and 0.813 respectively. The Musette mismatch was one onset outside
the 50 ms tolerance. This fixed synthesized phrase does not establish performance on real
register changes, expressive playing, or left-hand accompaniment.

The same synthesized melody was then rendered with alternating C bass and C major chord
button events every half-second. The SoundFont's Clarinet M, Bass1 and major chord presets were
mapped to bank 0 programs 21, 0 and 1 for this rendering. The input had 16 melody events, eight
bass-button events and eight chord-button events. MIDI SHA-256 is
`3dbd6b0c339a01ef9137859ec03b91f1f3ea9f5afe298bc68ec73b9149b2c47a` and WAV SHA-256 is
`67a865ea8e0b10aacf5b91f2fe342dd5e6080ba1605bbbbc53fc723beb80fccf`.
Basic Pitch returned 49 pitched events. Around the first chord it returned pitches 62, 64, 67
and 72, where 62 is the melody. In the browser, manually selecting MIDI 64–72 and starts
0.4–0.95 s for the left hand produced a C chord candidate at 0.50 s. The initial pitch split assigned the bass-region events to the left hand. After also assigning
MIDI 0–59 at 0–0.4 s, the browser displayed C bass candidates at all eight expected attacks
in this constructed phrase. Bass overtones also produced
multiple pitch events. This is a constructed mixture using a synthesized SoundFont. It does
not demonstrate automatic hand separation or accuracy for an acoustic two-hand performance.

For future acoustic two-hand and register evaluation, Lucas Mrowskovsky Paim's 2024
[dissertation](https://www.ppgia.pucpr.br/pt/arquivos/mestrado/dissertacoes/2024/Dissertacao_Lucas_Mrowskovsky_Paim.pdf)
describes FoleDataset, including paired audio/MIDI and some repeated performances with different
registers and isolated hands. A public download of the dataset itself was not identified in this
investigation. It was not used for any of the results above.

## Real accordion register screening

The public-domain recording [Accordion registers.ogg](https://commons.wikimedia.org/wiki/File:Accordion_registers.ogg)
by Necz0r is described by its creator as the same octave played through five accordion keyboard
registers. The original Ogg SHA-256 is
`e4aa5b2e824bd16f21b01a4a74dc94699b5a3795d32eb336b26f616695649130`; it was converted with
FFmpeg to 22.05 kHz mono WAV, SHA-256
`0465ffb56cd73c9ca3a25fe06151bd7e3143770ff59728f2dedabe70f2cd8418`. Basic Pitch 0.4.0
with default thresholds and pitch range 0–127 processed the 0–26 second range through the local
API. The raw response SHA-256 is `eee6ca53522a02ee9bb3a777c01a72e84abf0bdca5a34ad4556648aad4da2267`.

The following time windows were chosen after observing the output and separate the five
sequences. The source supplies neither note-level annotations nor names for the registers.

| Time window (s) | Detected events | Detected MIDI pitches in onset order |
| --- | ---: | --- |
| 0–5.5 | 8 | 60 62 64 65 67 69 71 72 |
| 5.5–10.5 | 8 | 60 62 64 65 67 69 71 72 |
| 10.5–16 | 16 | 60/48 62/50 64/52 65/53 67/55 69/57 71/59 72/60 |
| 16–21.3 | 15 | 60/48 62/50 64/52 65/53 55 57/69 59/71 72/60 |
| 21.3–26 | 8 | 48 50 52 53 55 57 59 60 |

Slashes group nearby events for readability; they do not assert exact simultaneous onset. This
is an acoustic recording, but detected sounding pitches can include multiple reed octaves and
are not independent labels for keys pressed. It provides a real-register functional comparison,
not a note-accuracy measurement, register identification, or evaluation of an accompanied
two-hand performance. Reference pitches and timing would need independent annotation.

### Named registers with a common score

Henry Doktorski's [Accordion Registrations](https://newmusicusa.org/nmbx/accordion-registrations/)
includes a score for its audio demonstrations and describes the instruments and reed combinations.
The article's original audio links currently redirect to unavailable pages. Archived copies of
the [clarinet](https://web.archive.org/web/20060213220215id_/http://www.newmusicbox.org:80/70/audio/Clarinet.mp3),
[bandoneon](https://web.archive.org/web/20060213220102id_/http://www.newmusicbox.org:80/70/audio/Bandoneon.mp3),
and [master](https://web.archive.org/web/20060213220155id_/http://www.newmusicbox.org:80/70/audio/Master.mp3)
examples were converted to 22.05 kHz mono WAV and processed from 0 to 20 seconds using Basic Pitch
0.4.0, default thresholds and pitch range 0–127. Source recordings and score are not included in
this repository.

| Register | Model events | Source MP3 SHA-256 | API response SHA-256 |
| --- | ---: | --- | --- |
| Clarinet | 65 | `2ccc93990f761665116df0b3dc7bcaee288884799f7dab469b4f82625f55031b` | `02adbed80b4d7ed88b8843eba689ec94ab1b8707bb4dd300ddd89af5b6ee9be2` |
| Bandoneon | 67 | `b6bf8cab94e1377fafe8180fa688f004bfc5a37d3beba2dd70890f9b56111530` | `7b10eac30cccd714cbcf3e75d8bc958a344c5d8d0893e3aa75aa0d3874fb7cdf` |
| Master | 100 | `d430b28e13ac3f7549b621e9f387d0c9d84b98531954ed33aad209e9dd02d2dd` | `0812f8b9ab05ff2a59e1d35b0bfab54e0d687a2d9f242bdcf1e196da2b209623` |

The [score image](https://newmusicusa.org/wp-content/uploads/nmbx/assets/70/images/accregistration_525x79.jpg)
SHA-256 is `845c69185e0c930bacda63a26e3b5b0ecd380e01133dec0385935ad74f935d83`.
The common score is useful context, but it is not an independently time-aligned reference, and
event counts do not distinguish extra octave reeds, repeated detections, missed keys, or actual
performance differences. This observation does not measure note accuracy or two-hand separation.

The first written note in the score is F3 (MIDI 53). Looking only at the first detected sustained
pitch group, the clarinet response contains 53 at 1.881 seconds; the bandoneon response contains
53 at 1.080 and 41 at 1.126 seconds; the master response contains 65 at 1.347, 53 at 1.382,
and 41 at 1.451 seconds. These are model onset estimates, not independently annotated onset times.
The article identifies the three stops as 8′, 16′+8′, and 16′+8′+8′+4′, respectively, which is
consistent with the observed octave pattern for this written note. It illustrates why all detected
octaves cannot automatically be treated as separate key presses; it does not establish that every
additional detected octave elsewhere in the recording is an intended reed tone.

As a browser functional check, the master response was loaded into the editor as an accordion
project. Selecting MIDI 53 at 1.382 seconds displayed MIDI 41 and 65 in the octave review list;
selecting MIDI 41 from that list moved the editor selection to that note. This uses the same
development example that motivated the 120 ms grouping window and does not establish accuracy,
ease of correction, or performance on other recordings.

### Candidate source for acoustic two-hand evaluation

[Duane Schnur's free accordion lessons](https://duaneschnur.com/accordion/lessons.htm) provide
downloadable lesson PDFs and narrated MP3s. The PDF in lesson 8 contains a two-staff arrangement
of “Carnival in Venice” with written right-hand notes and left-hand bass/chord symbols; lesson 12
contains a similar arrangement of “Come to the Sea”. The lesson 8 archive SHA-256 is
`4c78d82cafc1592dfc3fdc5bd4f29ddabb35fee728d1a3aaea4c7b480eac9762`, its MP3 SHA-256 is
`94cd7d11cf0357d4af289d5dcf7b3775d1d8879012f257ba2c311d77b7c047fb`, and PDF SHA-256 is
`74e9fb966bb5834fbd7675cf373b056b1334a199c758f2dbae38c33c37703736`. The lesson 12 archive
SHA-256 is `a358099ce78e591e79eb55933a0c81370bab4c3d2892ba445935fb65c11a0b9c`, MP3 SHA-256
`9aa0d6e67659000a92ea3abc979f466ec0792d9fba860877eb7c33b64d2eb52a`, and PDF SHA-256
`5f8ebea2e864c54ae7f2f06f30776123ac07667dafd64d11d2982326fab1fae8`.

These materials remain outside the repository. The MP3s are long narrated lessons, not aligned
reference recordings. A specific solo demonstration must first be located, checked against the
matching score and annotated independently; left-hand chord symbols alone do not specify all
sounding reed octaves. No transcription score or two-hand result is claimed from this source yet.

For lesson 8, a preliminary signal screen ranked 696–708 seconds highly for concentrated
spectral energy. A 35-second excerpt starting at 690 seconds was decoded from the MP3 to 22.05 kHz
mono WAV (SHA-256 `045e1ebebd33cb4415cc6814a47dd6be9b3fd7c80e632eb7cfd173cc49675a3d`) and
processed with default Basic Pitch over the entire excerpt. The JSON response SHA-256 is
`2821ab1efdf756facd0d3c7a0c1319452223a604e2dc1fa2cafe006b6bc09d7c`. Among its detections,
MIDI pitches 55, 57, 55, 53, 52 occur around source seconds 695.8–698.0 and again around
705.9–708.4. This suggests a repeated pitched passage worth checking against the score; both
the passage selection and this observation derive from model output and are not independent
annotation. Speech, exact performance boundaries, the played hands and score correspondence
remain unverified. Do not use the candidate as ground truth or compute accuracy from it yet.

## Overlap export check

The same-hand, same-pitch overlap guard was checked against five existing model outputs. Each
MIDI was loaded with `pretty_midi`; its note times and pitches were passed to `midi_bytes` at
120 BPM, 4/4, with a zero bar start and the editor's initial hand split at MIDI 60. All five
were accepted, including the guard's MIDI-tick rounding check. This checks the reconstructed
note data, not the complete browser workflow or the accuracy of the resulting score. Different
hand assignments, edits, tempo, or other recordings can produce an overlap that needs correction.

| Existing model output | Notes | Output MIDI SHA-256 |
| --- | ---: | --- |
| MAESTRO development, Basic Pitch | 182 | `5a39adf450c1c4140feedbc8b0d4df47d9c7d8442bf5447616627ba1c7e48977` |
| MAESTRO development, piano model | 124 | `a18403ce70ffbc4db9890bfcd8209189e69c332b59bcc25f41a2cdb5123f619c` |
| MAESTRO test, piano model | 854 | `c2e87d355bdf99b60d23c02a976db564b7d87c56bd77535f8cf3ed206d80565f` |
| Synthesized accordion with accompaniment, Basic Pitch | 49 | `faf676919c7a8aa73bd57c210bb89cd398ec9ed06ea7ed2058e01e6d85cd0471` |
| Recorded C minor accordion chord, Basic Pitch | 5 | `3e7489ff6683e10c980dd2a5c5b192f410b56e95736a052c59f52bf236d27cdb` |

## Range analysis on a piano recording

Two adjacent selections of the MAESTRO test recording described above were submitted to the
local `/transcribe` API with the piano model and pitch range 0–127. The server supplied up to
two seconds of context on each side. The table compares raw returned notes with the same
reference MIDI and, separately, the previous full-recording piano-model MIDI filtered to the
same onset range. Metrics and tolerances are the same as in the first section. The reference
and full-recording MIDI hashes and model checkpoint are given there. Boundary review candidates
were not added to the window results, and note releases were not truncated at the selection end.

| Selection (s) | Source of estimates | Reference / estimated notes | Onset P / R / F1 | Onset + offset F1 |
| --- | --- | ---: | --- | ---: |
| 10–20 | Range API | 137 / 136 | 0.963 / 0.956 / 0.960 | 0.725 |
| 10–20 | Full recording | 137 / 135 | 0.970 / 0.956 / 0.963 | 0.735 |
| 20–30 | Range API | 136 / 134 | 0.970 / 0.956 / 0.963 | 0.667 |
| 20–30 | Full recording | 136 / 135 | 0.963 / 0.956 / 0.959 | 0.664 |

The first response contained no boundary review candidates, and the second contained one;
neither reported a context-limited release. The JSON response SHA-256 values were
`65edd679cdf81b0ece033af6bd0391c808b573d0ec7cb4a6c82d418d7c25a7dd` and
`35da4ef1251f83e8427c8ee779f035dbbb7101506f2c233fa414770bc4af7733`, respectively.
This is one recording and does not measure correction time, score quality, or the effect of
manually reviewing boundary candidates. The first range was not selected independently of
the previously inspected recording.

For the 10–20 second range, its 136 raw API notes were also sent through `/midi` and
`/musicxml`. The export settings were 120 BPM, 4/4, no pickup, and a first bar at recording
time 10 seconds. These are fixed test settings, not verified beat annotations. MIDI times were
shifted back by 10 seconds for evaluation. MusicXML was read using Acorde's `playback-report`;
its playback events were shifted by the same amount. All 136 notes appeared in that report,
but one onset moved outside the 10–20 second scoring window after quantization.

| Output | Estimated onsets in window | Onset P / R / F1 | Onset + offset F1 |
| --- | ---: | --- | ---: |
| MIDI | 136 | 0.963 / 0.956 / 0.960 | 0.725 |
| MusicXML playback events | 135 | 0.963 / 0.949 / 0.956 | 0.669 |

The MIDI SHA-256 is `31183611b5d4ec226ef88df16046aa5405d898ce9aee9f1c92333a008ccc24e2`;
the MusicXML SHA-256 is `979e966c1fdf65145fb979ec3f0dec0167394a591f8c2935c5a04962f80dc829`.
This evaluates playback events on one excerpt with a chosen tempo, not notation readability,
correct beat alignment, or general MusicXML conversion accuracy.
