# External interoperability evidence

This is a checksum-pinned Phase 17 observation log, not a compatibility, engraving-quality, or
playback-quality score. SVG element counts are structural diagnostics only; they are not visual
similarity measurements.

## Verovio / MEI / SVG

On 2026-09-22, Verovio 6.3.0 ran the default command below against
`tests/fixtures/interchange_subset.mei` (SHA-256
`992c79629f7f44af59a43f7169c5829b96d0a3f6802a07f81dec371651d1defe`).

```bash
verovio -o /private/tmp/acorde-phase17b-verovio.svg tests/fixtures/interchange_subset.mei
```

Verovio produced a 25,226-byte SVG (SHA-256
`6baa8a4ce0841eb450e389a63ef8ab8cf14f79eecf3439135da1f777a5a15982`) and warned that the MEI `accid="qs"` value was unsupported
and that its tie could not be matched. The output had two nested `svg` elements, 36 `g` elements,
22 `path` elements, two `text` elements, and six `use` elements. It contains the rehearsal mark
`A`, but not the fixture's literal `Cmaj7` or `D.C. al Fine` strings.

For the same bytes, Acorde rendered with explicit `--width 900 --staff-size 24
--measures-per-system 4`. Its 5,981-byte SVG has one `svg`, eight `g`, two `path`, and four `text`
elements (SHA-256 `68892eaebd4682dde1e0a9c3ea10d6901bafb32f8ae27dc5662d2e2fd082b39b`), and includes `A` and `Cmaj7`. The page geometries and font/glyph resources differ, so
these artifacts are neither visual comparisons nor evidence of engraving parity.

## MuseScore / MusicXML

MuseScore Studio 4.7.4 was present locally. Its MSCX-to-MusicXML output (SHA-256
`6b421d5d69ba23d536e844d19098f24f98fa5ed0a622ec875e79ddeecb412043`) declares the standard
MusicXML 4.0 public DTD. Acorde now accepts that one conventional public declaration without
loading it, while still rejecting internal subsets, `ENTITY`, and `SYSTEM` declarations.

The existing `interchange_subset.mscx` fixture has contradictory `<pitch>` and `<tpc>` values;
MuseScore exported it as a measure rest. It is therefore excluded from semantic comparison.
Repeated headless conversion of the valid `external_tab.musicxml` fixture terminated inside the
local MuseScore process with a mutex error before writing an artifact. This is a host-runner
failure, not evidence about Acorde or MuseScore semantic parity. A valid MuseScore round-trip
remains a Phase 17A gate.

## alphaTab / tablature

The new minimal authored MusicXML fixture `tests/fixtures/external_tab.musicxml` has SHA-256
`3cce8d2f100e3b68a3430c4ccaa0ad737318179ba74ddf47bd31bc41b3ff37df`. It defines standard guitar
tuning and a low-E third-fret G2. Acorde parses it as internal low-to-high tuning
`[40, 45, 50, 55, 59, 64]`, internal string 1, fret 3, and G2, with no diagnostics.

alphaTab 1.8.4, loaded through Node 24.5.0 `ScoreLoader.loadScoreFromBytes`, parsed Acorde's
MusicXML export (SHA-256 `2587a92b88188ee7cc84af69a4479960d4d0ad1798fdef97565c2c50dceadea8`)
as one master bar with tuning `[64, 59, 55, 50, 45, 40]`, string 1, fret 3, and
`realValue` 43. Acorde now converts between its low-to-high internal string indexing and the
high-to-low MusicXML technical-string convention at the parser and serializer boundaries.

This only checks a single structural/playback-pitch projection. No browser rendering, scheduling
trace, SoundFont output, or cross-browser visual comparison was performed.

## Related analysis evidence

The separate [music21 observation](external-analysis-evidence.md) records the bounded Phase 17D
MusicXML analysis projection. It does not imply agreement with any engraving or interchange
result above.
