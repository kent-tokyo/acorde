# Source note ends with local pedal intervals (2026-11-07)

This reruns the [four-score alphaTab 1.9.0 source audit](playback-alpha19-source-onsets.md)
with the same licensed MXL files, Acorde schedules and alphaTab MIDI files.
Their per-score SHA-256 hashes are identical to the preceding run. The
[new report](playback-alpha19-local-pedal.json) changes only the independent
source oracle and its strict note-end eligibility rules.

Previously, one pedal mark excluded every note in its part. The oracle now
reads pedal start, stop, change, continue, resume and discontinue directions
at their written beats, including offsets and backup/forward positions. It
excludes a note end only when its note span touches an active pedal interval.
An orphan stop or continuation still excludes the whole part conservatively.
MusicXML usually writes a chord articulation on one member; the oracle now
applies release caveats from that marking to every sounding member of the
chord. Neither change modifies Acorde's playback engine.

| Score | Acorde strict written ends | alphaTab strict written ends |
| --- | ---: | ---: |
| Aloha Oe | 373/373 | 135/161 |
| Just for Today | 308/308 | 308/308 |
| Quilter Prelude | 106/106 | 106/106 |
| Webern, So ich traurig bin | 37/37 | 37/37 |
| **Total** | **824/824** | **586/612** |

Webern's pedal caveat falls from 89 notes under the whole-part rule to 39
under written intervals. Its strict sample rises from 29 to 37 after also
propagating chord markings. An intermediate run appeared to find six Acorde
note ends shortened by 0.25 beat; all six are members of two chords whose
first member has a staccato. Those are marked releases, so comparing their
performed note-offs to full written lengths would be a false defect. Across
the four scores, the refined chord caveat reduces the strict sample from the
previous 838 to 824. The old and new totals use different eligibility rules
and are not a performance trend.

The independent written attacks remain 1,536/1,536 for Acorde and 1,093/1,536
for alphaTab, with its 443 differences in *Aloha Oe*. The 26 alphaTab strict
note-end differences also remain there and coincide with its split-bar timing
differences; they have not been isolated as independent release defects. This
audit does not check pedal controller events, acoustic sustain, ornaments,
repeats, program changes, sample envelopes or perceived audio quality. Those
need separate source-level and listening tests before a playback parity claim.

## Reproduction

Use `acorde playback-report SOURCE.mxl --fermata-multiplier 1.0`,
`export_alphatab_midi.mjs` with pinned alphaTab 1.9.0, and
`compare_musicxml_onsets.py` as in the [preceding protocol](playback-alpha19-source-onsets.md#reproduction).
The Acorde CLI SHA-256 is
`77f1c779589b3b8c3442be1196847cb0031fe82703dd996f229470234ba412bf`;
the alphaTab package SHA-256 and every output file hash are in the JSON report.
