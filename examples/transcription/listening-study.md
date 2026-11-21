# Comparing manual entry and candidate correction

This protocol is prepared for a later listening session. No participant results are recorded yet.
The purpose is to compare the time needed to produce a similarly accurate four-bar transcription.

Use two four-bar passages of comparable note density from a piano recording with a known reference.
Fix the passage boundaries, tempo, meter, pitch range, playback speed controls, and reference
before the session. Do not show the reference to the participant during transcription. Record
which passages and recordings are used, their checksums, model version and thresholds, and the
participant's familiarity with the music and the editor. The participant may listen as often as
needed in both conditions.

1. In one passage, begin with an empty editor and enter the notes by ear.
2. In the other passage, generate candidates first and correct them by ear. Record inference time
   separately from the editing time, and also report their sum.
3. Start the timer when the passage is first played and stop when the participant judges the
   transcription ready. Record interruptions and any later changes.
4. Export MIDI and compare pitch and onset with the same aligned reference using `evaluate.py`.
   Save the participant's project JSON and MIDI alongside the timing record. Check the audio
   alignment before interpreting the error counts. The exported MIDI starts at the first pickup
   boundary, so pass `--estimate-offset` with the recording time of that boundary. This equals
   `barStart - pickupSteps * 15 / tempo` seconds. Pass the same `--start` and `--end` recording
   times for both conditions. The start is inclusive and the end is exclusive; the evaluator
   selects notes by onset and retains their original end times.
5. With multiple participants, alternate which passage uses each condition and the order of the
   conditions. Report each time and precision/recall separately, along with the corrections made
   and the participant's observations. Compare time only for outputs of comparable accuracy.

The `IDMT-PIANO-MM` iPhone recording used in `evaluation.md` is available in the temporary local
work area for a pilot. The dataset is CC BY-NC-ND 4.0, and its manually aligned MIDI claims onset
accuracy of about 50 ms but does not provide reliable note offsets. Keep its source files outside
the repository and disregard offset scoring. It covers one recording room and one device in the
current evaluation. Select and document passages before starting the timed comparison.

## Candidate passages for the pilot

Two non-overlapping six-second candidates have been identified in
`ROOM_room1_MUS_beethoven1_DEV_iphone.wav`, using the reference
`ROOM_room1_MUS_beethoven1.mid` from the same dataset. The recording SHA-256 is
`f8267f9416bd39304d7920ce90fa80f13932ed927fc5b70c31cd7ee9d529e45f`; the reference MIDI
SHA-256 is `8b24a67aea7acf2dde747c29fa082fcd7d8f42431f4e4382a7f8cd764e2edbaa`.

| Candidate | Recording seconds | Reference onsets | Reference pitch range |
| --- | --- | ---: | --- |
| A | 9.5 inclusive to 15.5 exclusive | 27 | MIDI 40–76 |
| B | 15.5 inclusive to 21.5 exclusive | 28 | MIDI 40–77 |

No reference note strictly crosses the boundaries at 9.5, 15.5 or 21.5 seconds. Similar counts and
pitch ranges do not establish equal listening or editing difficulty. The dataset's
[README](https://zenodo.org/records/7544248) identifies `beethoven1` as a section of Beethoven's
*Für Elise* and lists 120 BPM. The aligned MIDI contains no time-signature event. A published
[score](https://s9.imslp.org/files/imglnks/usimg/1/10/IMSLP926971-PMLP14377-Beethoven_Clavierst%C3%BCck_F%C3%BCrElise-WoO59.pdf)
is in 3/8. The first attacks and subsequent pitch sequences in the aligned MIDI match measures
85–92 of the [OpenScore MusicXML edition](https://github.com/sightreader/sheet-music/blob/master/Beethoven%20-%20Bagatelle%20No.%2025%20%28Fur%20Elise%29.musicxml)
(downloaded XML SHA-256 `f665fc5d0c8ad5c96e7e099a4bf927756d1087ea5a58d0d85816c2e8c3530f5c`).
The corresponding reference onsets place measure 85 at 9.5 seconds and measure 89 at 15.5
seconds; measure 93 begins around 21.5 seconds. This supports four bars per candidate, although
the dataset's stated 50 ms onset tolerance limits alignment precision. A 3/8 meter at 60 quarter
notes per minute gives 1.5 seconds per bar, consistent with these boundaries. This is an
export-grid setting; the dataset's nominal 120 BPM does not specify its beat unit in the README.
Set the first bar at 9.5 or 15.5 seconds and pickup
to zero for the respective candidate. Check the audio before the timed session. If the boundaries
or passages change, recompute the counts and record the new conditions. These
development candidates were chosen after prior model testing and are not an independent test set.
Assign condition order and candidate allocation before starting; do not use a participant's
result to choose the easier condition. The user's participation is deferred, and no timing or
accuracy result has yet been collected.

In a development check with the piano model, both ranges produced candidates that could be
exported as MIDI and MusicXML with the settings above. For B, the model also proposed A4 at
15.484 seconds as a boundary-review candidate, just before the score start. Adding that candidate
without editing its start makes export reject it as preceding the score. Compare it with the
recording and adjust its start or the score start if appropriate. This check is not a participant
trial or a measurement of correction time. Inference and boundary results may vary by model and
environment.
