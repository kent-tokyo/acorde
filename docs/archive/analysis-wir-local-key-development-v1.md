# When in Rome local keys: development diagnostic

The [JSON report](analysis-wir-local-key-development-v1.json) uses the two
previously inspected 12-work When in Rome cohorts. It verifies each prepared
MXL input against the hash in its existing harmony report before calling
`acorde report`. The 24 works provide 3,405 scored RomanText positions with a
human local-key label. The input preparation and labeled-position exclusions
are inherited from those development reports. This does not consume the
third-cohort fixed harmony holdout as local-key development data.

The prototype sums written pitch-class durations in a centered window of
physical measures and correlates them against the 30 major/minor Aarden-Essen
profiles used by the current global-key candidate. The first staff's written
key signature chooses between correlation-tied enharmonic spellings. It does
not smooth neighboring answers or see the human label when predicting. The
opt-in Rust API and `analyze-local-keys` CLI reproduce the Python radius-four
key for every physical measure of all 24 works; the report records the exact
binary SHA-256.

| Radius on each side | Exact local key | Pitch class and mode |
| ---: | ---: | ---: |
| 0 | 1,498 / 3,405 | 1,559 / 3,405 |
| 1 | 2,048 / 3,405 | 2,120 / 3,405 |
| 2 | 2,145 / 3,405 | 2,221 / 3,405 |
| 3 | 2,169 / 3,405 | 2,252 / 3,405 |
| **4** | **2,325 / 3,405** | **2,336 / 3,405** |
| 6 | 2,199 / 3,405 | 2,275 / 3,405 |
| 8 | 2,201 / 3,405 | 2,264 / 3,405 |
| 12 | 2,198 / 3,405 | 2,252 / 3,405 |

The radius-four exact count is 787/1,159 in the first cohort and 1,538/2,246
in the second. As context only, using each tool's *single whole-score key* at
every position matches 2,015/3,405 for Acorde and 1,916/3,405 for music21.
Those whole-score values are not a matched local-key comparison. Radius four
was chosen after inspecting all eight radii on these same works, so its count
must not be presented as a fixed evaluation or a superiority result.

The API returns one key per physical measure. Among 1,917 annotated measures,
79 have more than one human local key within the measure, covering 251 of the
3,405 positions. Thirteen labels use a key spelling outside the current
`-7..=7` key-signature candidate set. The method also needs explicit handling
for alternate physical bar partitions and transposing parts before a broader
contract. A future matched comparison must run a pinned music21 *local-key*
recipe on the same answer-free input and independently labeled positions, then
freeze an untouched cohort before tuning further.

Reproduce the development diagnostic with:

```sh
python3 -B scripts/diagnose_wir_local_keys.py \
  --cohort docs/archive/analysis-wir-harmony-development-v5.json \
  docs/archive/analysis-wir-harmony-holdout-selection-v1.json \
  /private/tmp/acorde-wir-holdout \
  --cohort docs/archive/analysis-wir-harmony-development-2-v2.json \
  docs/archive/analysis-wir-harmony-holdout-2-selection-v1.json \
  /private/tmp/acorde-wir-holdout-2-clean \
  --acorde target/debug/acorde \
  --check-rust \
  --output docs/archive/analysis-wir-local-key-development-v1.json
```

The temporary input paths above are local copies whose bytes are verified by
the script. The report stores their input SHA-256 values so a replacement
checkout with identical bytes can be used.
