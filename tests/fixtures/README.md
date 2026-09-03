# Test fixtures

`just_perfect_fifth_on_c.mid` is a public-domain MIDI file by Hyacinth, obtained
from [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Just_perfect_fifth_on_C.mid).
The file description documents the pitch-bend event (`80,64`) used by the MIDI
round-trip regression test. The source page was accessed on 2026-09-03.

SHA-256: `ff60a251a10c7886d4342a6d37fbe0c85ebdd24571d013cbcddfb37ce4399505`

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
