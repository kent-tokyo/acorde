# Mozart cadence corpus: input-only preflight

The [selection](analysis-mozart-cadence-selection-v1.json) pins `huispaty/batik_plays_mozart@9c5f7004c5ab1f15d64e959d011dbdf3b7863c3e` by Git blob, using only the repository tree. Its 36 paired MusicXML/cadence-CSV movements are split by entire sonata: K279–K284 (18 development movements) and K330–K333, K457, K533 (18 cadence-label holdout movements). No cadence CSV content was read for selection, input preparation, or preflight. Some score files were previously inspected for a **different global-key task**, so the reserved set is a label-level holdout, not an unseen-score claim.

The original MusicXML scores contain 10,352 `<harmony>` elements. The deterministic [preparer](../../scripts/prepare_mozart_cadence_inputs.py) removes those, 2,991 directions, 20 lyrics, and 71 credits from the 36 common inputs. It also corrects one `eigth` note-type spelling in K457-2 and removes one orphan `<chord/>` marker after a `<backup>` in K284-3. Neither repair changes a pitch or duration. The [input record](analysis-mozart-cadence-inputs-v1.json) retains source Git blob and SHA-256, prepared-input SHA-256, and per-score removal/repair counts. The source and prepared MusicXML files are kept in temporary storage, not copied into this repository.

The [parser preflight](analysis-mozart-cadence-preflight-v1.json) verifies the 36 prepared hashes, confirms no stripped element survives, and runs Acorde `report` and music21 10.5.2 on every prepared file. Both parsed **36/36**. Their pitched onset/duration/MIDI-pitch multisets agree in 5,136 of 5,142 physical measures. The six mismatches, fixed before reading reserved labels, are K279-3 measure index 79, K332-1 index 71, K533-1 indices 45/168/173, and K533-2 index 53. Five are pitch disagreements and one is a duration disagreement. Exclude these measures from a paired cadence score unless an input-only repair is documented and both timelines are rechecked.

This check does not prove identical voice placement or cadence predictions. Freeze an exact-location cadence scoring rule and candidate before opening the 18 reserved annotation files; keep those files unread until tuning stops.

After preflight, the development annotation for K279-1 was inspected to design
that rule. Its 2,803 note rows contain 30 cadence-tagged rows but only 20
distinct source note IDs after collapsing performance-repeat suffixes; the
duplicated labels agree in this one movement. The CSV stores `xml_mn`,
`onset_quarter`, `id`, `cadence`, and `cadence_type`. A scorer must map its
repeated performance-note IDs back to MusicXML note IDs and report any
conflicting repeated labels or missing IDs. No reserved annotation was read.

The [18-development-movement projection](analysis-mozart-cadence-development-positions-v1.json)
maps 46,100 annotation rows to 18,471 eligible folded note-onset positions,
including 333 cadence positions. There are no mapping exclusions in this split.
Observed labels are PAC 157, IAC 13, HC 118, DC 17, and EC 28; the
[DCML annotation reference](https://dcmlab.github.io/standards/build/html/reference/reference.html)
defines those categories. This projection has no Acorde or music21 cadence
predictions yet, and the 18 reserved CSV files remain unopened.

The [current Acorde-only development baseline](analysis-mozart-cadence-acorde-development-v1.json)
uses `acorde analyze` on those same prepared inputs. It collapses PAC/IAC to
`Authentic` for the current API, keeps HC/DC/EC as distinct categories, and
excludes the four eligible positions in the one unequal-timeline development
measure. At 18,467 eligible note onsets, it produces 141 cadence positions:
7 correct, 12 wrong type, 122 false positives, and 314 missed gold cadences
out of 333. The existing cadence pass aggregates pitches within each voice
and applies adjacent Roman-transition rules; this result is a diagnostic of
that implementation. It is **not** a music21 comparison or holdout result.

An opt-in [vertical-resolution rerun](analysis-mozart-cadence-acorde-development-v2.json)
uses the same 18 development movements, input hashes, position projection and
timeline exclusions. It matches 30 of 333 gold cadences, produces 29 false
positives, and misses 303. All 30 matches are PAC; the rule does not yet
classify half, deceptive or evaded cadences, and the Authentic API type does
not distinguish PAC from IAC. This remains Acorde-only development evidence.
