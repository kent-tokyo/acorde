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

Two non-overlapping eight-second candidates have been identified in
`ROOM_room1_MUS_beethoven1_DEV_iphone.wav`, using the reference
`ROOM_room1_MUS_beethoven1.mid` from the same dataset. The recording SHA-256 is
`f8267f9416bd39304d7920ce90fa80f13932ed927fc5b70c31cd7ee9d529e45f`; the reference MIDI
SHA-256 is `8b24a67aea7acf2dde747c29fa082fcd7d8f42431f4e4382a7f8cd764e2edbaa`.

| Candidate | Recording seconds | Reference onsets | Reference pitch range |
| --- | --- | ---: | --- |
| A | 8 inclusive to 16 exclusive | 36 | MIDI 40–76 |
| B | 16 inclusive to 24 exclusive | 37 | MIDI 40–77 |

No reference note strictly crosses the boundaries at 8, 16 or 24 seconds. Similar counts and
pitch ranges do not establish equal listening or editing difficulty. The aligned MIDI contains
no time-signature event; its 120 BPM tempo must not be treated as verified musical bar alignment.
Before the timed session, check the recording and an appropriate score to establish whether these
are four-bar passages, and fix the first-bar time, tempo, meter and any pickup in advance. If the
boundaries or passages change, recompute the counts and record the new conditions. These
development candidates were chosen after prior model testing and are not an independent test set.
Assign condition order and candidate allocation before starting; do not use a participant's
result to choose the easier condition. The user's participation is deferred, and no timing or
accuracy result has yet been collected.
