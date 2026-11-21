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
   alignment before interpreting the error counts.
5. With multiple participants, alternate which passage uses each condition and the order of the
   conditions. Report each time and precision/recall separately, along with the corrections made
   and the participant's observations. Compare time only for outputs of comparable accuracy.

The `IDMT-PIANO-MM` iPhone recording used in `evaluation.md` is available in the temporary local
work area for a pilot. The dataset is CC BY-NC-ND 4.0, and its manually aligned MIDI claims onset
accuracy of about 50 ms but does not provide reliable note offsets. Keep its source files outside
the repository and disregard offset scoring. It covers one recording room and one device in the
current evaluation. Select and document passages before starting the timed comparison.
