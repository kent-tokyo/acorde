//! Every browser test in one binary.
//!
//! `wasm-pack test` links each test binary separately and starts a browser for each
//! one (about 16 s). Keeping the files as modules of this binary pays that once.
//! Add a new test file here; `autotests = false` in Cargo.toml stops Cargo from
//! building it as its own binary.
//!
//! Several test files load the same fixture file with `#[path]`; each copy is a
//! separate module, as it was when every file was its own binary.
#![allow(clippy::duplicate_mod)]

#[path = "accordion_dynamics.rs"]
mod accordion_dynamics;
#[path = "accordion_response.rs"]
mod accordion_response;
#[path = "accordion_sections.rs"]
mod accordion_sections;
#[path = "accordion_voicing.rs"]
mod accordion_voicing;
#[path = "arrangement_native_parity.rs"]
mod arrangement_native_parity;
#[path = "arrangement_proposals.rs"]
mod arrangement_proposals;
#[path = "bass_line.rs"]
mod bass_line;
#[path = "chord_progression.rs"]
mod chord_progression;
#[path = "chordal_pad.rs"]
mod chordal_pad;
#[path = "countermelody_development.rs"]
mod countermelody_development;
#[path = "cross_plan_form_proposals.rs"]
mod cross_plan_form_proposals;
#[path = "developed_form_proposals.rs"]
mod developed_form_proposals;
#[path = "ensemble_form.rs"]
mod ensemble_form;
#[path = "ensemble_form_development.rs"]
mod ensemble_form_development;
#[path = "ensemble_form_inner_performance.rs"]
mod ensemble_form_inner_performance;
#[path = "ensemble_form_inner_voices.rs"]
mod ensemble_form_inner_voices;
#[path = "ensemble_roles.rs"]
mod ensemble_roles;
#[path = "free_bass.rs"]
mod free_bass;
#[path = "freilachs_repeat.rs"]
mod freilachs_repeat;
#[path = "harmony_arrangement.rs"]
mod harmony_arrangement;
#[path = "harmony_expansion.rs"]
mod harmony_expansion;
#[path = "harmony_groove_proposals.rs"]
mod harmony_groove_proposals;
#[path = "harmony_voicing.rs"]
mod harmony_voicing;
#[path = "inner_voices.rs"]
mod inner_voices;
#[path = "inner_voices_performance.rs"]
mod inner_voices_performance;
#[path = "instrument_plan.rs"]
mod instrument_plan;
#[path = "integrated_arrangement_proposals.rs"]
mod integrated_arrangement_proposals;
#[path = "issue_343_patterns.rs"]
mod issue_343_patterns;
#[path = "issue_344_melodic_insert.rs"]
mod issue_344_melodic_insert;
#[path = "issue_345_driving.rs"]
mod issue_345_driving;
#[path = "issue_358_359_bindings.rs"]
mod issue_358_359_bindings;
#[path = "issue_359_eventide.rs"]
mod issue_359_eventide;
#[path = "issue_359_polyphonic.rs"]
mod issue_359_polyphonic;
#[path = "issue_377_transposed_arrangement.rs"]
mod issue_377_transposed_arrangement;
#[path = "issue_378_profiled_patterns.rs"]
mod issue_378_profiled_patterns;
#[path = "issue_379_mscz_tempo.rs"]
mod issue_379_mscz_tempo;
#[path = "issue_393_smufl_font.rs"]
mod issue_393_smufl_font;
#[path = "issue_433_arrangement_plan.rs"]
mod issue_433_arrangement_plan;
#[path = "issue_435_arrangement_plan_shuffle.rs"]
mod issue_435_arrangement_plan_shuffle;
#[path = "issue_471_legacy_accompaniment.rs"]
mod issue_471_legacy_accompaniment;
#[path = "issue_475_sectioned_notation.rs"]
mod issue_475_sectioned_notation;
#[path = "issues_399_400_melodic_context.rs"]
mod issues_399_400_melodic_context;
#[path = "native_format_bindings.rs"]
mod native_format_bindings;
#[path = "performer_editions.rs"]
mod performer_editions;
#[path = "phrase_harmony.rs"]
mod phrase_harmony;
#[path = "phrase_harmony_targets.rs"]
mod phrase_harmony_targets;
#[path = "phrase_melody.rs"]
mod phrase_melody;
#[path = "rhythm_section.rs"]
mod rhythm_section;
#[path = "stage678_native_parity.rs"]
mod stage678_native_parity;
#[path = "standard_bass.rs"]
mod standard_bass;
#[path = "standard_bass_arrangement.rs"]
mod standard_bass_arrangement;
#[path = "standard_bass_dominant.rs"]
mod standard_bass_dominant;
#[path = "standard_bass_transition.rs"]
mod standard_bass_transition;
#[path = "transposition_harmony.rs"]
mod transposition_harmony;
