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

## PianoVAM independent performance excerpts

Three previously unexamined compositions were selected before inference from the test split
of [PianoVAM](https://huggingface.co/datasets/PianoVAM/PianoVAM_v1) revision `7aa7368`.
The associated [paper](https://arxiv.org/abs/2509.08800) describes amateur performances
recorded with a dedicated microphone and Disklavier MIDI, refined through audio/MIDI
alignment. This is a single recording setup, not a smartphone test. The source's MIDI is
used as the key-on and key-release reference. Model training-set overlap is unaudited.
The dataset card at the selected revision states CC BY-NC-SA 4.0; source recordings and
MIDI files are not included here.

The first 30 seconds of each recording were selected before model output was inspected.
The `Last Carnival` excerpt includes approximately 10.75 seconds before the first reference
onset. Each WAV was read as 16-bit PCM and the same excerpt was sent to the two models.
Onsets were matched within 50 ms and 50 cents. End matching additionally uses the greater
of 50 ms or 20% of reference note duration. Pedal-extended acoustical sound may persist
after the reference key release.

| Test performance | Reference onsets | Earlier model onset P / R / F1 | Transkun onset P / R / F1 | Earlier / Transkun onset-and-end F1 |
| --- | ---: | --- | --- | --- |
| Tsuru, Last Carnival | 80 | 0.878 / 0.988 / 0.929 | 1.000 / 1.000 / 1.000 | 0.047 / 0.325 |
| Bishop, Home Sweet Home | 35 | 0.972 / 1.000 / 0.986 | 1.000 / 1.000 / 1.000 | 0.986 / 0.829 |
| Haydn, Surprise Symphony | 43 | 0.977 / 1.000 / 0.989 | 1.000 / 1.000 / 1.000 | 0.989 / 1.000 |

Transkun matched all 158 reference onsets and produced no unmatched onsets in these three
short excerpts, whereas the earlier model matched 157 and produced 13 unmatched onsets.
The end metric for Home Sweet Home was lower with Transkun, and neither model scored well
on note endings in Last Carnival. These excerpts do not establish general performance.
`pianovam-comparison.json` fixes source audio, reference MIDI and model output hashes for
repeating the onset comparison. The source metadata SHA-256 is
`5be4f26ffd1119128ad956dae46898be6abb955a39c12a7e26ef8934fbba5a8e`.

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

The browser now marks a bass or chord candidate when each pitch class in its group also
appears in an earlier note still sounding at that time. The mark asks the listener to
check for a repeated attack and does not delete the candidate. In a browser check using
the already-inspected C major and C-sharp seventh outputs with a 0.4 s grouping window,
the later C-sharp seventh candidates were marked, while the C-major source's second
take at 3.36 s remained unmarked. This is a functional check selected from development
recordings. The source labels do not supply note-level timing, and the mark can also occur
on a genuine repeated key or button. Its accuracy on two-hand performances is unmeasured.

An additional recording was chosen after implementing the hint, without inspecting its
model output: [C-sharp minor by juskiddink](https://freesound.org/people/juskiddink/sounds/120693/),
CC BY 4.0. This is a separate recording on the same instrument by the same player. The
public MP3 preview was converted to 22.05 kHz mono WAV and processed by Basic Pitch 0.4.0
ONNX at default settings. All estimated notes were manually assigned to the left hand and
the 0.4 s grouping window was selected. The browser displayed C♯m at 0.05 s, followed by
three further C♯m and four E bass candidates carrying the continuation hint. The source
describes a single chord but gives no aligned note or button events. Thus this checks
the source's chord name and presentation of candidate overlap; it does not establish
note-level precision, recall, attack accuracy or two-hand performance.

The browser's evaluation export was checked with this project after manually assigning all
notes to the left hand. With 0.4 s grouping it wrote four bass and four C♯ minor candidates.
The JSON was accepted by `evaluate_accordion.py`. Comparing the file with itself matched all
eight events; this is a format and one-to-one matching check, not an accuracy result. The
exported candidate JSON SHA-256 is
`8e5811ec25a19b6b28a670515daa7879cc2a5fa52bbc18866853a7baed043dcd`.

The preview, derived WAV and model MIDI SHA-256 values respectively are
`46373ab56997d013eada463cb50e783d368b620fbb093a374bd3764770d2115a`,
`cf9196b83088b7b1fa1387b6409aef7844ce84212a60497db5b18f9a419abe73` and
`b775def074ceb92c75257ac7bf28a63da6dd783b863e1c396ab787a8e174696e`.

## iPhone piano recordings

### Transkun comparison on previously examined phone recordings

Transkun 2.0.1 with its bundled 2.0 model was run on CPU on the nine full recordings
below. Its checkpoint SHA-256 is
`50a80010effc2a59ffcd068a95cd2b29bd7f23a27a3515bc3ccd209c89a3d44c` and its config is
`d3d989214eb148230ee5df476d994dcde6af595904d3f968f1221d2e3bea5ac6`.
The comparison model is Piano Transcription Inference 0.0.6, as described above.
These recordings were already evaluated during filter development. The model's training
set overlap with these recordings has not been audited. The model was chosen after the
previous results were examined. Reported onset scores use the dataset's 50 ms alignment
limitation and do not assess reference offsets.

| Room, piece, device | Reference / Transkun notes | Transkun P / R / F1 | Earlier piano model P / R / F1 |
| --- | ---: | --- | --- |
| 1, Beethoven 1, iPhone | 212 / 211 | 0.972 / 0.967 / 0.969 | 0.727 / 0.967 / 0.830 |
| 1, chords, iPhone | 54 / 55 | 0.982 / 1.000 / 0.991 | 0.828 / 0.981 / 0.898 |
| 1, scale, iPhone | 88 / 85 | 0.800 / 0.773 / 0.786 | 0.305 / 0.727 / 0.430 |
| 2, Bartók, iPhone | 70 / 70 | 1.000 / 1.000 / 1.000 | 0.886 / 1.000 / 0.940 |
| 3, Mozart, Redmi | 131 / 136 | 0.926 / 0.962 / 0.944 | 0.438 / 0.939 / 0.597 |
| 4, Joplin, LG | 546 / 506 | 0.945 / 0.875 / 0.909 | 0.603 / 0.850 / 0.706 |
| 6, swing, iPhone | 176 / 179 | 0.983 / 1.000 / 0.992 | 0.872 / 0.972 / 0.919 |
| 5, Beethoven 2, iPhone | 272 / 277 | 0.931 / 0.949 / 0.940 | 0.826 / 0.945 / 0.882 |
| 7, Pachelbel, Redmi | 175 / 174 | 0.994 / 0.989 / 0.991 | 0.876 / 0.971 / 0.921 |

The new range API was also tested on Beethoven 1 from 10 to 20 seconds with two seconds'
context, yielding 45 events and onset P / R / F1 of 0.978 / 0.978 / 0.978 against 45
reference events. The response SHA-256 is
`03e86b9c4e2e4ca13d9094b04b1a1993026a9544686280c40caedb1b24fc4ea0`.
The recording has already been inspected, so this is a functional development check.

### Additional room8 phone performances

After selecting Transkun, four room8 performances were fixed before either model was run:
Bartók and swing on iPhone, Beethoven 2 on LG, and chords on Redmi. Room8 contains a
grand piano and a larger room than the prior development samples. These are separate
recorded performances, but their pieces were previously examined in other rooms. Therefore
this comparison checks a new recording environment, not a composition-disjoint test set.
The relationship of either model's training data to the recordings is unknown. Both models
processed the entire recordings and were compared to the corresponding room8 aligned MIDI
using the same 50 ms onset and 50-cent pitch tolerances. As elsewhere for IDMT, note-end
accuracy is not evaluated.

| Performance | Reference | Earlier model P / R / F1 | Transkun P / R / F1 |
| --- | ---: | --- | --- |
| Bartók, iPhone | 70 | 0.959 / 1.000 / 0.979 | 1.000 / 1.000 / 1.000 |
| Beethoven 2, LG | 272 | 0.967 / 0.982 / 0.974 | 0.974 / 0.982 / 0.978 |
| Chords, Redmi | 54 | 0.877 / 0.926 / 0.901 | 0.982 / 1.000 / 0.991 |
| Swing, iPhone | 176 | 0.903 / 0.955 / 0.928 | 0.989 / 1.000 / 0.994 |

Across these four performances the earlier model produced 37 false positives and matched
555 of 572 reference notes. Transkun produced 10 false positives and matched 567 reference
notes. That is a 73.0% reduction in false positives, with recall increasing from 97.0% to
99.1%; recall did not decline on any of the four recordings. This does not establish the
target on composition-disjoint mobile recordings. `idmt-room8-comparison.json` fixes the
four audio files, reference MIDIs and model output hashes for `compare_corpus.py`.

### MAPS real-piano comparison after model selection

After choosing Transkun, the first three alphabetically sorted `MUS` recordings in the
[MAPS ENSTDkCl archive](https://zenodo.org/records/18160555) were selected before their
outputs were examined. Each model received the first 30 seconds of the original recording.
These are Disklavier recordings generated from MIDI, captured with a close microphone,
not phone recordings. The model training sets have not been audited for these files.
MAPS is distributed under CC BY-NC-SA 2.0 France; its source files are not included here.
The original high-resolution MIDI was rescaled from 32767 to 960 ticks per beat by rounding
absolute ticks before scoring, because `pretty_midi` rejects its large maximum tick. Events
were evaluated by onset in the half-open 0–30 s window with the same pitch and onset criteria.

| Recording | Reference notes | Earlier model P / R / F1 (false positives) | Transkun P / R / F1 (false positives) |
| --- | ---: | --- | --- |
| alb_se2 | 173 | 0.908 / 0.971 / 0.939 (17) | 0.944 / 0.983 / 0.963 (10) |
| bk_xmas1 | 236 | 0.910 / 0.945 / 0.927 (22) | 0.957 / 0.932 / 0.944 (10) |
| bk_xmas4 | 212 | 0.786 / 0.986 / 0.875 (57) | 0.977 / 0.986 / 0.981 (5) |

Across the three excerpts, false positives fell from 96 to 25 and matched reference notes
fell from 600 to 599 out of 621. The aggregate reduction is 74.0% and recall fell 0.16
percentage points. On bk_xmas1 alone recall fell 1.27 percentage points, exceeding the
proposed one-point guard for that excerpt. These numbers do not establish the separate
mobile-recording target or performance on unseen acoustic instruments.

| Recording | Source WAV SHA-256 | Source MIDI SHA-256 | Transkun MIDI SHA-256 |
| --- | --- | --- | --- |
| alb_se2 | `68de7c0b9182ad77df8707c91523b7541f0da22653a82d1f9e641b2d79d2cd67` | `333d5f0751ae66f3cb9f02ddc46c5b81a8770870659343236dbedb88a7e3a1cd` | `0e6560dc5d9f2b57e61eac36b614eb9acd14761ee992de70c3adaffafe92ce2b` |
| bk_xmas1 | `02b6be62c39b48f40352d3eaccd1c492f55592fa116f0b4448d26e1e41a4a8d1` | `9616860640c8553af2e1e84d92f405488b82c843ee58a97d0c6d318402e244aa` | `f1aac0002a0808ca5235d669092e2eebc44780078bf8073474281d0d6e510fd3` |
| bk_xmas4 | `c3659b98b0dba1e981d9e45ac5e24007f3d0902cf1ba22102be4c200534aad5b` | `1d18e5cc88adc18f85cf58a5230a74a3b68a91fcc2bd0b97c99e86bf67734c62` | `64535f7b7f79dea2f1dcc3390ac3efc38ebe7d8ea8f2c05d0b9b8ed2ef506469` |

### Development investigation of short events

Using the full recordings and the same matching criteria below, the scale recording's
piano-model output has 64 matched notes, 24 missed notes and 146 unmatched estimates.
Of the unmatched estimates, 13 start within 50 ms of a reference note exactly one octave
away (within 50 cents). None starts within 100 ms of a matched estimate at the same pitch.
These proximity counts describe the output and do not establish the causes of the errors.

Removing estimates shorter than 50 ms from these already-inspected outputs gives the
following development results. This threshold was selected after examining the scale
output and has not been evaluated on held-out recordings. The matched reference counts
are unchanged on these three recordings. Onsets alone are evaluated here because this
dataset's reference offsets are not reliable.

| Recording | Before P / R / F1 | After P / R / F1 | Estimates before / after |
| --- | --- | --- | ---: |
| Scale | 0.305 / 0.727 / 0.430 | 0.427 / 0.727 / 0.538 | 210 / 150 |
| Chords | 0.828 / 0.981 / 0.898 | 0.855 / 0.981 / 0.914 | 64 / 62 |
| Beethoven 1 | 0.727 / 0.967 / 0.830 | 0.756 / 0.967 / 0.849 | 282 / 271 |

This is an offline diagnostic of raw MIDI outputs, not a change to the browser or an
evaluation of short staccato notes on independent performances.

### Independent check of the 50 ms filter

Before running the model, four other pieces were selected from separate rooms and three
different phone types in IDMT-PIANO-MM. The same fixed 50 ms duration filter was applied
to the full-recording piano-model output. These recordings were not used to choose the
threshold. Onset matching retains the 50 ms tolerance and the dataset's alignment limit.

| Room, piece, device | Reference / estimates before / after | Before P / R / F1 | After P / R / F1 |
| --- | ---: | --- | --- |
| room2, Bartók, iPhone | 70 / 79 / 76 | 0.886 / 1.000 / 0.940 | 0.921 / 1.000 / 0.959 |
| room3, Mozart, Redmi | 131 / 281 / 266 | 0.438 / 0.939 / 0.597 | 0.462 / 0.939 / 0.620 |
| room4, Joplin, LG | 546 / 769 / 633 | 0.603 / 0.850 / 0.706 | 0.652 / 0.756 / 0.701 |
| room6, swing, iPhone | 176 / 196 / 184 | 0.872 / 0.972 / 0.919 | 0.880 / 0.920 / 0.900 |

The filter removes matched events in the Joplin and swing recordings and fails the
proposed recall gate. It is not applied to transcription. The evaluation excludes
reference offsets, which are not reliable in this dataset.

| Piece | Audio SHA-256 | Reference MIDI SHA-256 | Model output MIDI SHA-256 |
| --- | --- | --- | --- |
| Bartók | `12d4fdf3985e47ddfde98ee6187fc5ff37d072314790db4ec3451f752424acec` | `c3bc072ef3308d13576721ac2f527e8db850f8707a8c28bd0a912cc9649d3690` | `373cb56b87a0e00643bd189007d343a53107fa96341a294da9ae985187764dcf` |
| Mozart | `18d308d0fb2cb03d2f3e18aa8ff7cd7229e41fcba5da7d222d65b97a144020a5` | `f29aef7d0735f03ba82165a26807914ce1d3fb1bcc6a3268101b00f0b7e49501` | `fdca8c6b6289a398f28089655478e45565cbd6b4b33d6f0370ef50ce5bdc76bd` |
| Joplin | `a17aae5f2a249b035a8c8d5a833d87e6d9183429fdda86fdd9910e2dc7fd9eb6` | `3b6cc6c88bc44b546431ab4889d9980f6f4210fad8243c2cf4ba7745db6d1b77` | `6a8e5b85ebf01991d5f8a09001b63150583a826c3a9572089e8bc94b96dc712a` |
| Swing | `4cce130a45659fd470924791c45311212ee5a578ca73e233ba5978c6f9d8f81c` | `71eda87e865e3c4a7df14f2fcdea8881cf9700119b1feedb1114ad8b5d534e99` | `c6674a58e756a67c0e105b8a54e1199b2d411835141a3788d03c0882145ff7d5` |

### Short and low-velocity diagnostic

After inspecting the seven recordings above, a second filter was selected for investigation:
remove estimated events shorter than 50 ms whose model-predicted MIDI velocity is below 40.
On those seven recordings it removed 84 unmatched events and no onset-matched events. These
recordings are now development data for this filter, including the previous independent set.

Before testing this second filter, two further recordings were chosen: room5 `beethoven2`
on iPhone and room7 `pachelbel` on Redmi. `beethoven2` is a different excerpt of the same
composition as the earlier `beethoven1`, so only Pachelbel is a previously unseen piece.
Neither recording was used to choose this filter. The model predicted three events satisfying
the filter on Beethoven and none on Pachelbel. Their reference / predicted / matched counts
were 272 / 311 / 257 and 175 / 194 / 170, respectively. After filtering, Beethoven onset
precision / recall / F1 changed from 0.826 / 0.945 / 0.882 to 0.834 / 0.945 / 0.886.
Pachelbel remained 0.876 / 0.971 / 0.921. Combined false positives fell from 78 to 75,
well short of the proposed 20% reduction. This filter has not been integrated.

The SHA-256 values for Beethoven audio, reference, and model output, in that order, are
`1f9a37753a0c52e043bc888c41cd91f8d0babd58ba5cb2959cd59f5ec0bb8c4b`,
`1c9fc6eb3111f6baccc2cc6a345f79b09bb1c77cab88401940ec01f12bcdec0b`, and
`ac4adadea6c6049bade5d12e8cca31155b80f77a12c7e7fbb5ca24c4ad088884`.
For Pachelbel they are
`161e2ccf2b9ae93df8d3e83c786a2029d99c64119340578bea7dbbefff34a1f9`,
`8ba39f7e09c5a0e5d20a678c65bd734594866b78d7b5fed77a4c0a1467fe81af`, and
`4eae8c9e8657e98bdb9159b643e9d0545a6610018541ec4fe141bccecf8799a0`.

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

### Thesis-linked two-hand recording

The dissertation links an audio example titled `First Waltz All 120BPM` from its discussion
of three opening measures. Its text describes the right-hand sequence E–G–E and G bass and
G-major chord components in the spectrogram. The linked SoundCloud MP3 was retrieved for
local analysis; it is not included in this repository. Its SHA-256 is
`8e1f3437075c45de274cc9504d5e6868e2d6ee14677c337a0f2acc9961821d67`. The first eight
seconds were converted to 22.05 kHz mono WAV, SHA-256
`020448312519fd0478870931fa92edb5c4aefc1919d6ff90eb6c31e7ed1d8f15`. Basic Pitch 0.4.0
ONNX, default settings, produced MIDI SHA-256
`d656b64a388fa82048d9c2eceeaaaf456a9e8d3ce5c3a4a9ef188e1559e327da`.

In that output, MIDI 64 spans 1.811–2.730 s, MIDI 67 spans 3.252–3.705 s, and MIDI 64
appears again at 5.065 s, consistent in pitch class with the stated E–G–E sequence. The
model also returned MIDI 48 near 1.881, 3.264 and 4.728 s, and combinations of 55, 60 and
64 near 2.73, 4.23 and 5.73 s. These suggest C bass and C-major pitch classes, whereas
the dissertation text describes G bass and G major. As an additional signal observation,
200 ms Hann-window FFTs centered at 2.85 and 5.85 s show strong peaks near 329, 391 and
521 Hz, compatible with E4, G4 and C5 or their harmonics. The windows were chosen after
model inspection, so they are not independent note annotations. Register combinations,
soundtrack alignment and the apparent difference from the written G labels require further
checking. The source does not specify note-level onset or release times in the excerpt,
and no button or key annotations have been verified. Do not use this output as ground truth
or derive note-level accuracy from it.

The first three written measures can also be checked against page 17 of the source method,
[Mário Mascarenhas's accordion method](https://www.cursosprofec.com.br/escola/apostilas/basico-em-acordeon-pfc-4.pdf).
The method's opening specifies right-hand E4, G4, E4, and each of the three bars has C bass
followed by two C-major button indications. The accessible PDF SHA-256 is
`811b41aabf1d0f8c27bdb0464be43a45793c9c3d86c1101c666108c454f886bf`; the score is not
included here. The written bass and chord agree with the C pitch-class evidence above, while
the dissertation's prose says G. This is a source discrepancy, not proof of which physical
buttons were pressed in the linked audio.

As a timing screen, librosa's `onset_detect` on the 22.05 kHz excerpt with a 128-sample hop
reported transients near 1.840, 2.305, 2.728, 3.175, 3.709, 4.197, 4.644, 5.195 and
5.695 seconds. These nine times resemble three groups of three beats at the stated tempo.
They were selected after the recording had already been inspected. Spectral transients need
not coincide with button presses, and the source provides no aligned event times. Treat the
proposed association with the written bars as a hypothesis requiring independent listening
or key/button observation, not as a reference suitable for note-level scoring.

The provisional nine-beat alignment was used as a diagnostic of the current default
workflow, not as a measured accuracy result. The Basic Pitch output for the first eight
seconds was filtered to note onsets before 6.15 s, with notes below MIDI 60 assigned to
the left hand and all others to the right, matching the editor's initial split. The browser
exported candidates using its default 0.2 s chord grouping. When compared within 100 ms to
the score-derived E4/G4/E4, three C basses and six C-major chord-button hypotheses, the
export contained 32 right-hand candidates, seven bass candidates and no chord candidates.
Two of the three hypothesized right-hand onsets and all three hypothesized bass onsets
matched. None of the six chord hypotheses could match. The right-hand output contains
many additional events, including octaves, and is not a reliable count of played keys.

This diagnostic exposes a limitation of the initial split: the detected notes for the
C-major pitch classes cross the fixed MIDI-60 boundary, so they cannot appear together in
the left-hand grouping without reassignment. The comparison does not validate those
hypothesized onset times or confirm the physical buttons. The provisional input and browser
output SHA-256 values were `8fc805d899031f21d4bf1fd026266f3d9dc7d0d66039dff28f166266e8df8ba3`
and `ab9dc03c8e75269d7d20fb027c9cdf5b0d2011edde91bc65b639c15c4ce7a867` respectively.

An optional candidate source now permits grouping all estimated notes across the initial
hand split. On the same provisional example with the same 0.2 s window, this produced four
chord candidates, two of which matched the six hypothesized chord events within 100 ms.
However, none of the three hypothesized C bass events matched the five bass candidates:
grouping the melody with the bass can prevent a single-pitch bass candidate from appearing.
This mode is a review aid and does not assign notes to hands or infer a pressed button. It
is not an established improvement in automatic two-hand transcription. The exported
all-notes candidate JSON SHA-256 is
`4ae7e463b234efb87c28f713cbd85bc89ea4c6d5b6061f2ba26c8e640b8428bf`.

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

The opening treble notes in the lesson 8 PDF are G4, A4, G4, F4, E4. As a second, non-model
inspection, 8192-sample Hann-window FFTs at 11.025 kHz were taken from the lesson 8 audio at
695.90, 696.42, 696.80, 697.75 and 698.20 seconds. Each window contained a strong peak near,
respectively, 196.2, 219.7, 195.5, 174.3 and 164.5 Hz: approximately G3, A3, G3, F3 and E3.
The analogous windows at 706.02, 706.88, 707.28, 708.20 and 708.55 seconds showed the same
pitch-class sequence. These spectral observations support, but do not prove, correspondence
with the printed opening, one sounding octave lower. The windows were selected after model
inspection, several harmonics and other notes were present, and no independent onset or
left-hand button annotation was made. Human listening and annotation are still required before
using this passage as a note-level reference.

A second lesson 8 candidate was screened at source seconds 760–790. Its 22.05 kHz mono WAV
SHA-256 is `1b2b77d8349c7392681bf3d218f9f0dc04ff0d8ec582429bb8bf1a5b5b411a88`, and its
default Basic Pitch response SHA-256 is
`5aaf68bc200e0285a4ec0782ceb570aa3ec1da01edcfe5d827b9c5206fa6c408`. Model detections beginning
at 765.99 seconds include G3, A3, G3, F3 and E3, compatible in pitch class with the first printed
treble phrase. A 4096-sample Hann FFT at 766.7 seconds has peaks around 220 Hz (A3), 261 Hz (C4)
and 523 Hz (C5); a window at 767.1 seconds also contains a peak near 326 Hz (approximately E4).
The written accompaniment at this point includes C major. These peaks are compatible with, but
do not prove, simultaneous melody and accompaniment: they may include harmonics, residual sound
or other sources. The window and observations were selected after inspecting model output. No
independent identification of button presses, note onsets, voice overlap or speech has been made,
so this remains a candidate for listening and annotation, not a two-hand accuracy result.

An additional check used shorter 2048-sample Hann windows at 11.025 kHz, centered every 0.1 s
from 766.7 through 767.2 seconds. The approximately 220 Hz component persisted, while peaks
near C and E were intermittent and varied in strength. In particular, the evidence is weaker
than a continuously sounding C-major chord: the longer FFT windows above can mix neighboring
events. This does not identify whether a bass or chord button was played, and the passage still
requires human listening and independent annotation.

## Overlap export check

### Four-bar MusicXML playback comparison

The two candidate excerpts from `listening-study.md` were generated by the piano model and
exported with the 3/8 score settings documented there. The same IDMT reference, exported
MIDI and MusicXML playback events were aligned to recording time. The score settings were
chosen from the reference and published score, and the excerpts have previously been
inspected, so these are development observations. No participant correction was included.

| Excerpt | Reference / MIDI / MusicXML events in window | MIDI onset P / R / F1 | MusicXML onset P / R / F1 |
| --- | ---: | --- | --- |
| A, 9.5–15.5 s | 27 / 32 / 31 | 0.844 / 1.000 / 0.915 | 0.871 / 1.000 / 0.931 |
| B, 15.5–21.5 s | 28 / 30 / 30 | 0.867 / 0.929 / 0.897 | 0.833 / 0.893 / 0.862 |

Taking the exported MIDI as reference to isolate conversion differences, A has one unmatched
MIDI onset and B has three unmatched onsets on each side under the 50 ms matcher. A's
MusicXML playback has two onset-matched notes whose endings differ by 106 ms from MIDI;
B has two with ending differences of +85 ms and −81 ms. These are conversion differences,
not comparisons to verified key-release annotations. Quantization can also move an onset
across the selected scoring boundary. Musical notation and its readability were not rated.
In A, MIDI A4 at 15.483 s was rounded to the next bar at 15.500 s and therefore fell
outside the half-open scoring window. In B, D5 at 19.808 s, E5 at 20.063 s and C3 at
20.075 s moved to 19.750 s, 20.000 s and 20.000 s respectively. The MIDI importer's
sixteenth-note grid is 250 ms at the selected 60 quarter-note BPM, so those offsets are
consistent with grid snapping. This does not establish whether the resulting notation is
musically preferable, because the source performance itself may deviate from the beat.

The MIDI and MusicXML SHA-256 values for A are
`0326f5a92f3eaab217ff9e5a3a0ac3a791da08cd1f7a7937d12b2fa2553affe8` and
`7b26c639856a68d644f1cb227bed5f8a368e051e4c21f6babdaf35d5dd7c18f3`.
For B they are `c81f2d2bad374bc7db17d473d31fb3f2a07645d39132a1151f34a2d406cc6f20`
and `20de204d4da1164832b5cee1a2e19eb5e43e41ce93828adbf4d357230dfb3444`.

The same two excerpts were reanalyzed through the browser's range API using Transkun 2.0.1,
up to two seconds of context and the same hand split and score settings. The returned
start-boundary review candidate for B was not added. These excerpts were previously
inspected and used during development. The reference is the same aligned IDMT MIDI,
whose onset tolerance is approximately 50 ms and whose releases are not verified.

| Excerpt | Reference notes | Stage | Estimated onsets | Onset P / R / F1 |
| --- | ---: | --- | ---: | --- |
| A | 27 | API | 28 | 0.964 / 1.000 / 0.982 |
| A | 27 | MIDI | 28 | 0.964 / 1.000 / 0.982 |
| A | 27 | MusicXML playback | 27 | 1.000 / 1.000 / 1.000 |
| B | 28 | API | 27 | 0.963 / 0.929 / 0.945 |
| B | 28 | MIDI | 27 | 0.963 / 0.929 / 0.945 |
| B | 28 | MusicXML playback | 27 | 0.926 / 0.893 / 0.909 |

Comparing MusicXML playback to the exported MIDI, A has one unmatched MIDI onset at
15.492 s that was rounded to the next bar outside the 15.5 s scoring boundary. B has three
unmatched events in each direction: D5 at 19.806 s, E5 at 20.065 s and C3 at 20.077 s
became 19.750, 20.000 and 20.000 s respectively. Five onset-matched note endings differ
from MIDI by more than the evaluator's duration-relative tolerance in each excerpt. These
are differences from model-produced MIDI releases, not validated key-release errors. The
0.25 s grid at the chosen tempo explains the three B onset changes, while the score alone
does not prove which timing is musically preferable. No participant corrected these outputs.

| Excerpt | API response SHA-256 | MIDI SHA-256 | MusicXML SHA-256 |
| --- | --- | --- | --- |
| A | `45b77856ca8dcd35b5362330af2507738f63bf495c3a3602e210dc01dd0c38de` | `305b9a15c6448b67fe8a1e9ce88952b6ed94fee7398d79d43df89cdc8eaa7d83` | `459c5b80b5ecbb72cb0f61af2c67483066bbdeb6b2a33ec424d1487e3ae7212b` |
| B | `e579ff645673651160a9877942c68d2d20037b3eec607581fdbb6afcd57083e5` | `ccd787d773837b1917cb629078b926568c4ace63e00e828751b5c59e0e48359b` | `5ecf0d1204ca39f2c820cd03cb269aee99916d72dc6b5b7770cc5432eef39b2d` |

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

## Phone recording of digital piano: Sight to Sound MIDI test set

The [Sight to Sound MIDI test set](https://www.robots.ox.ac.uk/~vgg/research/sighttosound/)
contains eight videos of a single amateur playing a digital piano. The authors describe a
phone-camera recording and a MIDI reference captured by the instrument, aligned to the
video audio. This is a separate performer and recording setup from the preceding development
and evaluation data. The eight numbered files are all included; their individual piece
names are not supplied, so composition-level separation has not been verified. Model
training-set overlap is also unknown. The source is by A. S. Koepke, O. Wiles, Y. Moses and
A. Zisserman, ICASSP 2020, and is licensed CC BY 4.0. Neither videos nor MIDI are included
here. The downloaded `MIDItest.zip` had SHA-256
`0e440b140af7fb5d53d8ec34a17adff26479b2fada537688c36e5386d25a8a1b`.

Before inspecting either model's output, all eight videos were fixed and each video's first
30 seconds were decoded with ffmpeg to mono 16 kHz PCM16; both models used the same audio.
Initial comparisons produced almost no matches because the provided MIDI is approximately
0.2–0.4 seconds later than the decoded audio. After discovering this, an alignment procedure
was added: `align_phone_reference.py` scores a grid of constant shifts from -0.8 to 0.2 s
in 5 ms steps against the 16 kHz audio's spectral-flux onset envelope and reference MIDI
onsets between seconds 1 and 9. The best shift is applied to the reference MIDI; no model
output enters this calculation. Librosa 0.11.0, NumPy 2.4.6, and pretty_midi 0.2.11.post0
were used. The offsets in file order below were -0.375, -0.355, -0.205, -0.225, -0.270,
-0.390, -0.300 and -0.180 seconds. The following results use only onsets in seconds 10–30,
outside the alignment window. Because the alignment procedure was introduced after seeing
the initial comparisons, this is an exploratory evaluation, not a preregistered test.

The earlier piano model and Transkun were run with the same versions and settings as the
PianoVAM evaluation. Matching uses 50 ms onset tolerance and 50-cent pitch tolerance;
end matching also uses a tolerance of 20% of reference duration or 50 ms, whichever is
larger. MIDI key releases need not coincide with the ends of sustained sound.

| Video | Reference | Earlier matched / extra | Transkun matched / extra | Earlier / Transkun onset-and-end F1 |
| --- | ---: | ---: | ---: | ---: |
| 10 | 60 | 60 / 9 | 60 / 0 | 0.667 / 0.933 |
| 21 | 43 | 42 / 11 | 43 / 0 | 0.792 / 0.814 |
| 23 | 72 | 71 / 0 | 72 / 0 | 0.923 / 0.944 |
| 24 | 80 | 80 / 1 | 80 / 0 | 0.870 / 0.963 |
| 25 | 37 | 37 / 3 | 37 / 0 | 0.727 / 0.946 |
| 26 | 49 | 48 / 5 | 49 / 0 | 0.549 / 0.939 |
| 27 | 52 | 52 / 5 | 52 / 0 | 0.771 / 0.904 |
| 5 | 83 | 83 / 10 | 83 / 0 | 0.795 / 0.952 |
| Total | 476 | 473 / 44 | 476 / 0 | — |

On this small sample the candidate has 44 fewer unmatched notes and matches three more
reference onsets. It does not establish performance on acoustic pianos recorded by phone,
other performers, independently identified compositions, or other recording devices.
`sighttosound-comparison.json` fixes the excerpts and hashes for original video, original
and aligned reference, decoded audio and both outputs. To reproduce, unpack the archive as
`examples/transcription/sighttosound/`, use ffmpeg's `-t 30 -ac 1 -ar 16000 -c:a pcm_s16le`
to write each `.excerpt.wav`, generate `.aligned.mid` with `align_phone_reference.py`, run
the two models into the manifest's paths, then pass the manifest to `compare_corpus.py`.
