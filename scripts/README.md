# Scripts

Maintenance, CI, and evidence scripts. Most files here reproduce one bounded comparison or prepare
one pinned corpus; only the scripts marked **CI** run on every pull request. The files stay in one
flat folder because CI, evidence documents, and saved reproduction commands refer to these paths.
Rename or move a script only together with every reference to it.

Python scripts use the standard library unless their docstring names a pinned external tool.
Run them with `python3 -B` from the repository root. Node scripts expect the WASM packages that
their header comment names under `target/wasm-profiles/`.

## Everyday checks

```bash
# Contracts checked in CI (no external tools needed)
python3 -B scripts/validate_interoperability_manifest.py
python3 -B scripts/validate_print_corpus.py --check-only
python3 -B scripts/check_doc_links.py

# Unit tests for the evidence scripts (some modules also need mido or music21)
python3 -B -m unittest discover -s scripts/tests -p 'test_*.py'

# Build size-oriented viewer and full WASM profiles and report their sizes
bash scripts/build-wasm-profiles.sh
```

The WASM checks used in CI also need the `playback-only` and `measured-print` packages. Build
them with the `wasm-pack` commands in the `wasm` job of
[`.github/workflows/ci.yml`](../.github/workflows/ci.yml) before running those scripts.

## CI and build

| Script | Purpose |
| --- | --- |
| `validate_interoperability_manifest.py` | **CI.** Validate the external-interoperability corpus contract. |
| `validate_print_corpus.py` | **CI.** Validate print fixtures and deterministic logical-layout reports. |
| `check_doc_links.py` | **CI.** Check relative links and heading anchors in tracked Markdown. |
| `check-wasm-playback-profile.mjs` | **CI.** Check full, viewer, and playback-only WASM packages. |
| `check-wasm-arrangement-plan.mjs` | **CI.** Check arrangement-plan replay through WASM. |
| `check-wasm-arrangement-plan-shuffle.mjs` | **CI.** Check seeded arrangement-plan selection through WASM. |
| `build-wasm-profiles.sh` | Build the size-oriented WASM profiles into `target/wasm-profiles/`. |
| `report-wasm-profile-sizes.mjs` | Report WASM and declaration sizes for each built profile. |
| `benchmark-wasm-viewer.mjs` | Compare full and viewer WASM load and render time. |
| `check-measured-print-wasm.mjs` | Compare measured print between native and WASM on the same inputs. |

## Engraving and print (Verovio)

See [engraving parity](../docs/engraving-parity.md) and the
[Verovio baseline](../docs/verovio-engraving-baseline.md).

| Script | Purpose |
| --- | --- |
| `export_verovio_pages.mjs` | Render every page with a pinned Verovio package. |
| `compare_print_corpus.py` | Measure Acorde print pages against Verovio on the same MusicXML. |
| `bench_print_parity.mjs` | Time both engines in isolated processes on the same input. |
| `fetch_print_holdout.py` | Fetch and verify pinned print-holdout sources. |
| `prepare_print_holdout.py` | Prepare shared MusicXML for the holdout after a candidate freeze. |
| `evaluate_print_holdout.py` | Evaluate every prepared holdout input, keeping both engines' failures. |
| `normalize_print_staff_size.py` | Make an equal-staff-size input for a separate comparison. |
| `check_print_variants.py` | Screen default/A4 and matched-size/Letter print configurations. |
| `audit_print_margins.py` | Classify A4 margin crossings in measured development SVGs. |
| `audit_print_musicxml.py` | Compare MusicXML pitched events with the imported Score JSON. |
| `render_print_review.py` | Render comparison pages for review; it does not assign a pass. |

## Playback and audio (alphaTab)

See [playback evidence](../docs/playback-evidence.md).

| Script | Purpose |
| --- | --- |
| `export_alphatab_midi.mjs` | Export an SMF playback projection from a pinned alphaTab install. |
| `export_alphatab_audio.mjs` | Export pinned alphaTab synthesized audio without a browser. |
| `compare_playback_onsets.py` | Compare Acorde playback JSON with an external MIDI note schedule. |
| `compare_playback_audio.py` | Compare short score-aligned PCM windows. |
| `compare_playback_pcm_lag.py` | Measure PCM alignment for a same-score, same-SF2 export pair. |
| `compare_audio_corpus.py` | Measure Acorde and alphaTab PCM against one score schedule. |
| `compare_midi_events.py` | Compare channel-event projections from Mido and Acorde. |

## Music analysis (music21)

See the [evaluation contract](../docs/analysis-evaluation.md) and
[analysis evidence](../docs/external-analysis-evidence.md). Holdout scripts follow the order
fetch → freeze → prepare → preflight → score, and must not read labels before the freeze.

| Area | Scripts |
| --- | --- |
| Common | `evaluate_analysis_labels.py`, `music21_projection.py`, `analysis_voice_leading_compare.py` |
| Global key | `fetch_global_key_holdout_scores.py`, `freeze_global_key_holdout.py`, `freeze_global_key_protocol.py`, `prepare_global_key_holdout_inputs.py`, `preflight_global_key_holdout.py`, `score_global_key_holdout.py` |
| Earlier key sets | `compare_bcmh_keys.py`, `compare_beethoven_keys.py`, `compare_mozart_keys.py`, `adapt_beethoven_key_report.py` |
| Harmony (BCMH) | `compare_bcmh_harmony_music21.py`, `compare_bcmh_vertical_harmony.py`, `adapt_bcmh_harmony_report.py` |
| Harmony (When in Rome) | `fetch_wir_harmony_holdout.py`, `freeze_wir_harmony_holdout.py`, `prepare_wir_harmony_inputs.py`, `compare_wir_harmony_music21.py`, `adapt_wir_harmony_report.py` |
| Local key | `freeze_wir_local_key_holdout.py`, `freeze_wir_local_key_protocol.py`, `preflight_wir_local_key_holdout.py`, `score_wir_local_key_holdout.py`, `compare_wir_local_keys_music21.py`, `diagnose_wir_local_keys.py` |
| Cadence (Mozart, development only) | `fetch_mozart_cadence_development.py`, `freeze_mozart_cadence.py`, `prepare_mozart_cadence_inputs.py`, `preflight_mozart_cadence_inputs.py`, `project_mozart_cadence_development.py`, `mozart_cadence_positions.py`, `score_mozart_cadence_acorde_development.py`, `diagnose_mozart_cadence_sonorities.py`, `diagnose_mozart_cadence_transition_rules.py` |

## Interchange evidence

See [external interoperability evidence](../docs/external-interoperability-evidence.md).

| Script | Purpose |
| --- | --- |
| `record_interoperability_comparison.py` | Record one version-pinned interchange comparison. |
| `record_svg_interoperability_observation.py` | Record a hash-pinned MEI-to-SVG observation. |
| `summarize_interoperability_evidence.py` | Combine validated observations without a compatibility score. |
| `validate_external_evidence.py` | Validate an external-tool evidence report. |
| `compare_musicxml_events.py` | Compare MusicXML note/rest event projections from two scores. |
| `compare_musicxml_onsets.py` | Compare attacks and written ends with a MusicXML source oracle. |

## Arrangement development

| Script | Purpose |
| --- | --- |
| `benchmark_arrangement_stages.py`, `arrangement_stage45_benchmark.rs` | Benchmark arrangement stages against pinned core revisions. |
| `derive_stage678_native_parity.py` | Embed native listening-pack data for browser parity checks. |
| `check-arrangement-listening-pack.mjs` | Check that saved arrangement contracts replay to exported MusicXML. |
| `check-arrangement-listening-review.mjs` | Bind a listening review to the exact audio it judged. |
| `build_edition_review.py`, `build_performer_edition_pack.py` | Build player review and audition packs. |
| `build_rhythm_audition_font.py` | Build synthetic audition samples. |
| `record-legacy-link-corpus.mjs`, `legacy-site-ts-loader.mjs` | Record the legacy shared-link fixture from a pinned site commit. |
