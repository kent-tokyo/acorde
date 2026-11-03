# Eventide regression fixture

`eventide-site-36997ef8.mscz` is the unmodified `public/scores/abide-with-me-eventide.mscz` from fisarmonica.org commit `36997ef87eeba8d6d729af3ed6fa066e30b6f686`, SHA-256 `e449dd23e492d98becf2a7f0675319d510798c219c856df5bd1eb7d810fb31e3`. The site's source record identifies it as the four-voice [Mutopia Eventide edition](https://www.mutopiaproject.org/cgibin/piece-info.cgi?id=1241), which Mutopia labels public domain, transposed to E-flat with editorial tempo. It is used as input, not as a published accordion edition.

The test makes explicit pitch selections and uses intentionally artificial per-measure harmony and a mock button profile with all linear positions set to zero. It checks software contracts, not musical suitability, actual reach, engraving quality or performer approval. No generated score from this test is published.

# Sarabande regression fixture

`sarabande-bwv1012-site-36997ef8.mscz` is the unmodified `public/scores/accordion/bach-sarabande-bwv1012.mscz` from fisarmonica.org commit `36997ef87eeba8d6d729af3ed6fa066e30b6f686`, SHA-256 `e7ccdc27532768c78175415a2501079e4cac224265643aa029ca1bd9f0607d9e`. Its embedded copyright tag reads: "Hajo Dezelski, Mutopia Project, CC BY-SA 3.0; converted from the edition's MIDI for playback; free-bass hand assignment by fisarmonica.org, CC BY-SA 3.0". **This file is therefore CC BY-SA 3.0, unlike the public-domain Eventide fixture**; keep the attribution and license with any redistribution. It is a MIDI-derived conversion, not a verified engraving, and is used only as input for software regression, never as a published edition.

The test picks the top pitch of each chord in the first voice as an explicit host choice, uses artificial per-measure diatonic harmony and a mock profile with all positions zero, and checks the loss report (every source pitch kept or omitted exactly once), fail-closed ambiguity and omission diagnostics.
