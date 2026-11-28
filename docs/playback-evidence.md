# Playback and tablature evidence

These dated records separate score/event timing, MIDI conversion, and rendered audio. They
are fixture-specific observations, not general playback or audio-quality rankings.

- On the pinned OpenScore *Just for Today* input, the recorded comparison with alphaTab 1.8.4
  matched 579 written-pitch/beat attacks; fermata duration differences remained. It did not
  compare sound quality or velocities.
- A synthetic piano PCM comparison used different channel and gain settings. Its alignment
  shifts are signal-alignment values, not independent onset-error measurements.
- Full-SF2 WAV, pedal, grace-note, and tablature observations each have their own scope and
  limitations in the records below. Browser audio timing and broader corpus equivalence remain
  unproven.

[Local gate and CLI reproduction](archive/playback-synthesis-and-swing.md#local-gate) explain
how to run the basic checks. The [40-score manifest](playback-corpus-2026-10-30.json) contains
its own pinned selection and results. Full commands, revisions, hashes, and observations are
preserved in the records below.

## Local gate

[Full record](archive/playback-synthesis-and-swing.md#local-gate).

## CLI reproduction

[Full record](archive/playback-synthesis-and-swing.md#cli-reproduction).

## Swing pair timing (2026-10-30)

[Full record](archive/playback-synthesis-and-swing.md#swing-pair-timing-2026-10-30).

## Native SF2 WAV output (2026-10-31)

[Full record](archive/playback-synthesis-and-swing.md#native-sf2-wav-output-2026-10-31).

## Same-input PCM baseline against alphaTab 1.8.4 (2026-10-31)

[Full record](archive/playback-synthesis-and-swing.md#same-input-pcm-baseline-against-alphatab-184-2026-10-31).

## Full-SF2 GM render and fixed-rate comparison (2026-10-31)

[Full record](archive/playback-synthesis-and-swing.md#full-sf2-gm-render-and-fixed-rate-comparison-2026-10-31).

### Explicit PCM16 peak leveling and channel-level audit (2026-10-31)

[Full record](archive/playback-synthesis-and-swing.md#explicit-pcm16-peak-leveling-and-channel-level-audit-2026-10-31).

### Authored program changes in full-SF2 audio (2026-10-31)

[Full record](archive/playback-synthesis-and-swing.md#authored-program-changes-in-full-sf2-audio-2026-10-31).

## alphaTab 1.8.4 / independent CC0 score (2026-10-30)

[Full record](archive/playback-alphatab-and-midi.md#alphatab-184--independent-cc0-score-2026-10-30).

## alphaTab 1.8.4 / split-measure CC0 score (2026-10-30)

[Full record](archive/playback-alphatab-and-midi.md#alphatab-184--split-measure-cc0-score-2026-10-30).

### Same-score PCM lag at the split bar (2026-10-31)

[Full record](archive/playback-alphatab-and-midi.md#same-score-pcm-lag-at-the-split-bar-2026-10-31).

### Part mix in stereo on the same score (2026-10-31)

[Full record](archive/playback-alphatab-and-midi.md#part-mix-in-stereo-on-the-same-score-2026-10-31).

## MIDI export timing parity (2026-10-30)

[Full record](archive/playback-alphatab-and-midi.md#midi-export-timing-parity-2026-10-30).

### Overlapping unison duration in MIDI export (2026-10-31)

[Full record](archive/playback-alphatab-and-midi.md#overlapping-unison-duration-in-midi-export-2026-10-31).

## Pinned OpenScore Lieder timing sample (2026-10-30)

[Full record](archive/playback-corpus-and-controls.md#pinned-openscore-lieder-timing-sample-2026-10-30).

## Written grace timing against the same-score MIDI (2026-10-31)

[Full record](archive/playback-corpus-and-controls.md#written-grace-timing-against-the-same-score-midi-2026-10-31).

### Opt-in ornament WAV on the same score

[Full record](archive/playback-corpus-and-controls.md#opt-in-ornament-wav-on-the-same-score).

## Webern tenuto and same-score audio export (2026-10-31)

[Full record](archive/playback-corpus-and-controls.md#webern-tenuto-and-same-score-audio-export-2026-10-31).

## Webern same-cursor pedal retake (2026-10-31)

[Full record](archive/playback-corpus-and-controls.md#webern-same-cursor-pedal-retake-2026-10-31).

## Eight-score audio corpus against alphaTab 1.8.4 (2026-11-04)

[Full record](archive/playback-audio-corpus.md).

## Eight-score audio observation against alphaTab 1.9.0 (2026-11-07)

[Full record](archive/playback-audio-corpus-alpha19.md). The attack comparison uses
Acorde's schedule and is a screening result, not an independent timing verdict.

## Written note onsets against alphaTab 1.9.0 (2026-11-07)

[Full record](archive/playback-alpha19-source-onsets.md). The comparison uses
written beat positions with fermata stretching disabled; the record also
documents a corrected whole-measure-rest fermata hold in default playback.

## Evidence boundary

[Full record](archive/playback-corpus-and-controls.md#evidence-boundary).

## Explicit shared-unison playback and rest pedal controls

[Full record](archive/playback-corpus-and-controls.md#explicit-shared-unison-playback-and-rest-pedal-controls).

## Roadmap measurement notes (moved 2026-11-28)

These paragraphs were moved verbatim from the [roadmap](../ROADMAP.md) so that it can list
only current status, next steps, and completion conditions. They are dated observations.

The [eight-score 1.9.0 observation in PR #401](https://github.com/kent-tokyo/acorde/pull/401)
uses the same scores, soundfont, sample rate and channel count. Its timing
reference comes from Acorde. The independent written-onset audit in
[PR #407](https://github.com/kent-tokyo/acorde/pull/407) covers four source
scores: Acorde matched 1,536/1,536 pitch/beat attacks and alphaTab 1.9.0
matched 1,093/1,536; the 443 differences are split-bar timing in one score.
The audit also corrects a whole-measure-rest fermata hold. The
[written note-span follow-up](archive/playback-alpha19-source-onsets.md)
checks source-derived note ends for the same scores. After excluding ambiguous
same-pitch overlaps and expressive markings, Acorde matches 838/838 written
ends, and alphaTab matches 597/623 onset-matched written ends; all 26
differences are in the split-bar score. Expand the source
oracle beyond these simple forms. The
[local-pedal follow-up](archive/playback-alpha19-local-pedal.md) narrows
pedal release caveats to written active intervals and propagates chord-wide
articulations: Acorde matches 824/824 strict written ends and alphaTab
586/612 under the revised eligibility rule. These totals cannot be compared
as a trend with the preceding rule. A
[lined-pedal repair](archive/playback-lined-pedal-controls.md) keeps
MusicXML offsets, barline releases and the Webern fixture's `resume` in
offline playback and MusicXML roundtrips.
