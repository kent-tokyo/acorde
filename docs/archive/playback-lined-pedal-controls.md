# Exact-time lined MusicXML pedals (2026-11-07)

MusicXML pedal lines can begin or end between note onsets. Previously Acorde
attached `line="yes"` starts and stops only to nearby notes. A direction at
beat 1.5 could therefore press the pedal at beat 1; a `resume` was ignored.
The imported score now keeps both the drawn line's note anchors and independent
pedal control events at the written beats. Offline playback suppresses the
redundant anchor controls. MusicXML export combines each paired line and timed
control into one direction with the correct offset.

The focused 4/4 test places a start at beat 1.5 and a stop at beat 2.5. At
120 BPM and 16 kHz, the offline events are one down at frame 12,000 and one
up at frame 20,000. Tests also cover a release at the next barline, two
presses in one measure, and `change` as release then repress at the same beat.
The [CC0 Webern fixture](../../tests/fixtures/manifest.json) has eight pedal
directions on its second part's second staff, including a lined `resume`.
All eight are now present in its imported score and offline manifest. This is
a source/event check, not an acoustic sustain or listening result.

The corrected MusicXML line and control roundtrips preserve the tested beat
positions and number of directions. This does not yet verify every pedal
glyph's printed position, half-pedal values, controller-channel routing, or
the way another application's player interprets the export. Those remain
separate print and audio gates.

Validation: 122 `acorde-io` library tests, 624 `acorde-core` library tests,
and ten MusicXML pedal integration tests passed in the release profile.
All-feature Clippy passed for core and I/O. The same-base optimized macOS WASM
grew from 7,062,665 to 7,082,331 bytes (+19,666). The Linux CI artifact and
browser tests must be checked on the PR before merging.
