# Source-positioned analysis evaluation

The existing `benchmarks/analysis.json` checks category **counts**. It is a useful
regression check, but equal counts can conceal wrong labels or wrong positions.
`scripts/evaluate_analysis_labels.py` adds a separate label-level evaluation
contract. It compares each system with independent gold annotations at the same
source position and reports results by case, split, system, and task. It does not
parse scores or convert raw tool output into predictions; those adapters still
need to be built for each analysis task.

Run a prepared manifest with:

```sh
python3 -B scripts/evaluate_analysis_labels.py /path/to/manifest.json \
  --output /path/to/report.json
python3 -B -m unittest discover -s scripts/tests \
  -p 'test_evaluate_analysis_labels.py'
```

The input is JSON with `schema_version: 1`, `system_profiles`, and `cases`.
Profiles name an exact version, command, and option object for each system.
Every case has a unique `id`, `input_sha256`, `split` (`development`,
`calibration`, or `holdout`), `gold`, and one result per profiled system.
`input_path` may point to the original file, relative to the manifest; when
present, the evaluator reads its bytes and verifies the SHA-256. The report
marks `input_verified: false` when it is absent. The supplied command, version,
and options are recorded provenance, not independently validated execution.

A gold row has `task`, `at`, and `labels`, where `labels` lists accepted
equivalent answers fixed before evaluation. A prediction has `task`, `at`, and
one `label`. `at: "score"` addresses whole-score results. Event positions use
`part`, `staff`, `measure`, `beat`, and optionally `voice`. The beat is a
nonnegative fraction string such as `"1/2"`; `"2/4"` compares equal to it.
Span positions use `{"start": <event>, "end": <event>}`. A system that could
not parse the score records `{"error": "reason"}`. An empty array means the
system analyzed the score and returned no predictions.

The evaluator counts a correct prediction only when task, position, and label
match. A wrong label counts as a prediction and a missed gold label; a missing
position and an extra position are tracked separately. Duplicated task/position
pairs are invalid, since silently choosing one would hide an error. Precision,
recall, and F1 are reported per task and over all rows. Empty denominators are
`null`, never a manufactured perfect score. Parsing failures are counted as
failed cases and excluded from conditional accuracy; compare the analyzed-case
rate alongside it. The report deliberately keeps development, calibration, and
holdout totals separate.

For an external comparison, freeze a work list and input hashes before tuning.
Split by work rather than by file, movement, or transposition. Remove answer
annotations from both systems' musical inputs, and compare imported note timing
and voice placement before scoring analysis. Record every parse failure and
alignment exclusion with its reason. If a holdout score informs a fix, move it
to development and freeze a new untouched holdout. This evaluator does not
enforce those research procedures; the manifest and per-score reports must
document them.

Local-key, cadence, and voice-leading adapters still need common source
positions, explicit normalization rules, and hand-reviewed gold. The harmony
adapter below covers only BCMH's source-annotated onsets in a previously used
development corpus. New untouched holdouts must fix eligibility without using
either system's analysis prediction.

## Existing global-key comparison adapter

`scripts/adapt_beethoven_key_report.py` projects the saved Beethoven v2.3 key
comparison into this contract, preserving all 64 selected movements and any
conversion or parser failures. It assigns the historical results to
`development`: the results have already been inspected and cannot serve as a
new untouched holdout. The Acorde binary is identified by its recorded SHA-256
because that source report did not record a package version. Given the retained
converted MusicXML directory, `--work-dir` supplies each common input path for
hash verification; otherwise the evaluator explicitly reports
`input_verified: false`.

```sh
python3 -B scripts/adapt_beethoven_key_report.py \
  docs/archive/analysis-beethoven-key-holdout.json /tmp/beethoven-labels.json
python3 -B scripts/evaluate_analysis_labels.py \
  /tmp/beethoven-labels.json --output /tmp/beethoven-label-report.json
```

On the saved report, this reproduces 57/64 correct global keys for Acorde and
56/64 for music21, with all 64 cases analyzed by both. This is a projection of
the existing observation, not a new run of either analyzer. The archived report
does not include the 64 converted XML files, so those input hashes cannot be
rechecked by this command alone. The next untouched key holdout must use
retained, hash-verified common inputs and a preregistered split.

## BCMH harmony development adapter

`scripts/compare_bcmh_harmony_music21.py` contract 4 retains every accepted
source annotation beat, gold Roman label, and both systems' prediction. The
[full rerun and conditions](archive/analysis-bcmh-harmony-labels-v4.md) cover
100 works and 4,627 scored rows. The adapter rejects the earlier contract 3
report because its bounded error examples omit most predictions.

```sh
python3 -B scripts/adapt_bcmh_harmony_report.py \
  docs/archive/analysis-bcmh-harmony-labels-v5.json \
  /path/to/BCMH_dataset.zip /tmp/bcmh-label-manifest.json \
  --input-dir /tmp/bcmh-cleaned-inputs
python3 -B scripts/evaluate_analysis_labels.py \
  /tmp/bcmh-label-manifest.json --output /tmp/bcmh-label-evaluation.json
```

The adapter verifies the pinned archive, each stripped musical input and the
retained per-row counts before writing the manifest. With `--input-dir`, the
evaluator reads back all 100 inputs and verifies their hashes. All cases are
assigned to `development`. The 97 equal-timeline cases have 4,627 gold rows;
the remaining three are labeled as comparison exclusions, distinguishing two
parse failures from one unequal timeline. Scores with equal timelines still
exclude 208 source-labeled rows in 75 measures under the strict onset rule.
The task label is `roman_at_source_annotated_onset`: it does not score extra
labels outside annotated source beats. The common evaluator reproduces 2,193
correct labels for Acorde and 2,073 for music21 10.5.2 under the narrow
canonical normalization. These are development counts.

## When in Rome harmony development and fixed holdout

`scripts/freeze_wir_harmony_holdout.py` selects paired
`score.mxl`/`analysis.txt` works from fixed composer prefixes in the complete
pinned Git tree. Cohorts 2 and 3 use disjoint lexicographic offsets and, for
some composers, distinct collections. It reads no score, annotation, or
prediction when selecting works.
`scripts/fetch_wir_harmony_holdout.py` verifies each file against its pinned
Git blob and records SHA-256 before evaluation. The
[current development report](archive/analysis-wir-harmony-development-v5.md)
compares the opt-in candidate on two inspected 12-work cohorts after correcting
the comparator's triplet timing. It matches 366 versus 348 of 1,159 labels in
cohort 1 and 399 versus 394 of 2,246 in cohort 2. Cohort 2's first frozen
[evaluation](archive/analysis-wir-harmony-holdout-2-v1.md) failed at 348 versus
394; it became development data before the later candidate was tuned. Other
analysis tasks still need independently frozen label-level holdouts.

`scripts/adapt_wir_harmony_report.py` checks the archived report against the
selection and hash record, then projects all aligned gold positions and both
predictions into the common evaluator. With `--source-dir`, every MusicXML
input is read back and SHA-256 verified by the evaluator. The saved
[cohort 1 projection](archive/analysis-wir-harmony-development-label-evaluation-v3.json)
reproduces 366 and 348 correct labels with all 12 inputs verified.

```sh
python3 -B scripts/adapt_wir_harmony_report.py \
  docs/archive/analysis-wir-harmony-development-v5.json \
  docs/archive/analysis-wir-harmony-holdout-selection-v1.json \
  docs/archive/analysis-wir-harmony-holdout-hashes-v1.json \
  /tmp/wir-label-manifest.json --source-dir /path/to/verified-first-cohort
python3 -B scripts/evaluate_analysis_labels.py \
  /tmp/wir-label-manifest.json --output /tmp/wir-label-evaluation.json
```

The third cohort's [protocol](archive/analysis-wir-harmony-holdout-3-protocol-v1.json)
pinned the candidate binary, evaluation sources, selection, hashes, and gate
before scoring. Its [prepared inputs](archive/analysis-wir-harmony-holdout-3-inputs-v1.json)
remove 1,358 embedded `<harmony>` answer elements from three quartet scores;
the other nine scores retain their source bytes. Both systems received the same
prepared MXL bytes. The [fixed result](archive/analysis-wir-harmony-holdout-3-v1.md)
passes its aggregate Roman-at-primary-annotation-position gate: 559 versus 508
correct of 2,341 aligned positions, with 12/12 parses each and 16 documented
exclusions. The common evaluator verifies all 12 input hashes. This result
does not cover unannotated positions, local-key analysis, cadence, or voice
leading.

The [local-key development diagnostic](archive/analysis-wir-local-key-development-v1.md)
reuses only the first two inspected cohorts and their verified inputs. The
opt-in centered-window API matches 2,325 of 3,405 annotated local-key
positions at a radius selected on those same works. Its Rust output matches
the prototype on all 24 works. A [paired development comparison](archive/analysis-wir-local-key-paired-development-v1.md)
uses music21 10.5.2's native local-key analyzer on the same verified scores
and positions. Its best radius over the same development grid matches
2,157/3,405, with 3,401 available positions. This remains inspected
development evidence; a new untouched cohort and source-beat-level local-key
contract are required.

## Mozart cadence inputs awaiting a scoring contract

The [Batik cadence selection](archive/analysis-mozart-cadence-selection-v1.json)
pins 36 paired MusicXML and cadence-CSV movements by Git blob. The first six
complete sonatas form 18 development movements; the other six form 18 reserved
cadence-label movements. The [prepared-input record](archive/analysis-mozart-cadence-inputs-v1.json)
hashes scores after removing embedded harmony and other possible annotation
elements and after two input-only syntax repairs. The [preflight](archive/analysis-mozart-cadence-preflight-v1.md)
verifies both music21 10.5.2 and Acorde parse all 36 prepared scores. Cadence
annotations in the reserved split have not been read. Score-level prior
exposure from the separate global-key work is disclosed in the selection.

The opt-in [Acorde cadence development rerun](archive/analysis-mozart-cadence-acorde-development-v2.json)
finds 30/333 annotated positions with 29 false positives after the fixed
timeline exclusions. The earlier [legacy pass](archive/analysis-mozart-cadence-acorde-development-v1.json)
found 7 with 122 false positives. Both reports are Acorde-only development
diagnostics, not a comparison with music21.

Before running a cadence comparison, map repeated performance-note IDs back
to the folded MusicXML note IDs, define exact score positions and class mapping,
and score false positives over the same eligible positions. Pin the music21
cadence recipe, its version, and the candidate binary before touching the
reserved labels. Parser acceptance alone does not validate note alignment.
