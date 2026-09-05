# Test fixtures

The machine-readable inventory, provenance, checksums, evidence mode, and expected losses are in
[`manifest.json`](manifest.json). The evaluation rules are documented in
[`docs/interchange-evidence.md`](../../docs/interchange-evidence.md).

`just_perfect_fifth_on_c.mid` is a public-domain MIDI file by Hyacinth, obtained
from [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Just_perfect_fifth_on_C.mid).
The file description documents the pitch-bend event (`80,64`) used by the MIDI
round-trip regression test. The source page was accessed on 2026-09-03.

SHA-256: `ff60a251a10c7886d4342a6d37fbe0c85ebdd24571d013cbcddfb37ce4399505`

`4_steps_in_31-et_on_c.mid` and `septimal_major_third_on_c.mid` are additional public-domain
Wikimedia Commons pitch-bend fixtures. Their source pages identify the files as public domain and
describe the authored pitch-bend values; they cover a signed negative and positive nonzero bend.
Their source URLs and SHA-256 values are pinned in `manifest.json`.

## MIDI corpus policy

The checked-in MIDI set is intentionally a small, legally conservative smoke corpus. New
reproducible boundary cases should be self-authored or explicitly dedicated to CC0/public domain;
the manifest must record the source, license, checksum, and expected semantic fields. The target
boundary matrix includes note pairing and overlaps, multiple tracks/channels, tempo and meter
changes, controllers, program changes, aftertouch, percussion channel 10, signed pitch-bends at
`-8192`, `0`, and `8191`, SMPTE timing, long tick gaps, and malformed-event diagnostics. The
in-memory regression cases `generated_boundary_corpus_preserves_event_meaning` and
`generated_boundary_corpus_reports_timing_and_pairing_boundaries` exercise these protocol
boundaries without adding third-party music to the repository.

Large third-party corpora are **not** downloaded during tests and are **not** automatically added
to this directory. They belong to an external validation run with a separate manifest. In
particular, a dataset's top-level license does not by itself establish that every contained MIDI
transcription is redistributable. Only individually reviewed Public Domain/CC0 or otherwise
permission-cleared files may be promoted into the checked-in fixture corpus.

`UprightPianoKW-small-20190703.sf2` is a real SF2 fixture from the FreePats Upright Piano KW
sound bank. It is released under the CC0 1.0 public-domain dedication:
[source and license](https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html). It is used
only for bounded PCM decode and deterministic render regression tests.

SHA-256: `cf2a98eb38a32c4954b4b6e2caae4112d62dd8e892eceefdd7942b0e7d01ac2f`

`FluidR3Mono_GM.sf3` is the real SF3 fixture from the MuseScore 2.1 source tree. Its bundled
license text identifies the FluidR3Mono work as MIT and requires retaining the acknowledgements:
[source](https://github.com/musescore/MuseScore/tree/2.1/share/sound). It is used for the optional
SF3 Vorbis decode and deterministic render regression tests.

SHA-256: `cfcd66d89e8386823400eca64934b14fbea7bf48ba1f00d21189af1262794ec2`

`interchange_subset.mei` and `interchange_subset.mscx` are self-authored, redistributable fixtures
for the local MEI/MSCX semantic boundary. They cover MEI tie/layer/microtone, chord-label,
editorial/navigation data and MSCX tab staff tuning, string/fret, bend, Tremolo, and simple
Harmony/name and typed Text data. Their checksums and expected loss contract are pinned in
`manifest.json`.

`interchange_multistaff.mei` is a self-authored, redistributable MEI fixture for the multi-staff,
multi-layer subset. It covers score-level treble/bass clefs and independent layer content; its
checksum and expected loss contract are pinned in `manifest.json`.

`interchange_figured_bass.mscx` is a self-authored MSCX fixture covering an ordered pair of native
`FiguredBassItem/digit` values. It is used for structured import and no-loss regression coverage;
its checksum and expected loss contract are pinned in `manifest.json`.

`openscore_lieder_aloha_oe.mscx` is an external OpenScore Lieder fixture at the pinned revision
recorded in `manifest.json`. The upstream repository declares the score corpus CC0. It is used as
an import-only MuseScore 3.6.1 smoke corpus; MSCX export is not claimed.

`openscore_omr_score_1003.mscz` is a small compressed MSCZ fixture from the MuseScore OMR
benchmark dataset. The dataset README declares the underlying works Public Domain/CC0-1.0; the
Hugging Face revision, source path, checksum, and import-only evidence mode are pinned in
`manifest.json`. It is used to test bounded ZIP extraction and real MSCX parsing; export is not
claimed.
`openscore_omr_score_1033.mscz` is a second fixture from the same pinned CC0-1.0 dataset and
forms the MuseScore 4.6.3 compressed-MSCZ pair. Both are import-only fixtures.
Three additional MuseScore 4.6.3 MSCZ samples (`1035`, `1036`, and `1016`) cover zero-diagnostic
scores with 1-part/14-measure, 4-part/72-measure, and 1-part/22-measure structures. Their
revisions and checksums are pinned in `manifest.json`.
`openscore_omr_score_1003.mscx` and `openscore_omr_score_1033.mscx` are the corresponding
extracted MSCX payloads from those archives; their source paths and checksums are pinned so the
compressed and uncompressed representations can be tested as a pair.
