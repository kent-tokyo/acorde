#![cfg(all(feature = "musicxml", feature = "mei", feature = "midi"))]

use acorde_wasm as bindings;
use serde_json::Value;

#[test]
fn format_bindings_reopen_scores_and_return_reports_and_svg() {
    let xml = include_str!("../../../tests/fixtures/simple.musicxml");
    let score_json = bindings::parse_musicxml(xml).unwrap();
    let source: acorde_core::Score = serde_json::from_str(&score_json).unwrap();
    assert_eq!(source.metadata.title, "Simple Test");
    assert!(
        bindings::parse_musicxml_render_svg(xml, "{}")
            .unwrap()
            .contains("<svg")
    );
    let report: Value =
        serde_json::from_str(&bindings::parse_musicxml_report(xml).unwrap()).unwrap();
    assert_eq!(report["score"]["metadata"]["title"], "Simple Test");
    let out_xml = bindings::serialize_musicxml(&score_json).unwrap();
    assert_eq!(
        serde_json::from_str::<acorde_core::Score>(&bindings::parse_musicxml(&out_xml).unwrap())
            .unwrap()
            .parts
            .len(),
        source.parts.len()
    );
    let _: Value =
        serde_json::from_str(&bindings::serialize_musicxml_report(&score_json).unwrap()).unwrap();

    let mei = bindings::serialize_mei(&score_json).unwrap();
    assert!(
        bindings::parse_mei_render_svg(&mei, "{}")
            .unwrap()
            .contains("<svg")
    );
    let mei_score: acorde_core::Score =
        serde_json::from_str(&bindings::parse_mei(&mei).unwrap()).unwrap();
    assert_eq!(mei_score.parts.len(), source.parts.len());
    let _: Value = serde_json::from_str(&bindings::parse_mei_report(&mei).unwrap()).unwrap();
    let _: Value =
        serde_json::from_str(&bindings::serialize_mei_report(&score_json).unwrap()).unwrap();

    let mscx = bindings::serialize_mscx(&score_json).unwrap();
    let mscz = bindings::serialize_mscz(&score_json).unwrap();
    for json in [
        bindings::parse_mscx(&mscx).unwrap(),
        bindings::parse_mscz(&mscz).unwrap(),
    ] {
        let score: acorde_core::Score = serde_json::from_str(&json).unwrap();
        assert_eq!(score.parts.len(), source.parts.len());
    }
    assert!(
        bindings::parse_mscx_render_svg(&mscx, "{}")
            .unwrap()
            .contains("<svg")
    );
    assert!(
        bindings::parse_mscz_render_svg(&mscz, "{}")
            .unwrap()
            .contains("<svg")
    );
    for json in [
        bindings::parse_mscx_report(&mscx).unwrap(),
        bindings::parse_mscz_report(&mscz).unwrap(),
        bindings::serialize_mscx_report(&score_json).unwrap(),
        bindings::serialize_mscz_report(&score_json).unwrap(),
    ] {
        let _: Value = serde_json::from_str(&json).unwrap();
    }

    let midi = bindings::serialize_midi(&score_json).unwrap();
    assert!(!midi.is_empty());
    assert!(
        bindings::parse_midi_render_svg(&midi, "{}")
            .unwrap()
            .contains("<svg")
    );
    let midi_score: acorde_core::Score =
        serde_json::from_str(&bindings::parse_midi(&midi).unwrap()).unwrap();
    assert!(!midi_score.parts.is_empty());
    assert!(
        !bindings::serialize_midi_region(&score_json, 0, 0)
            .unwrap()
            .is_empty()
    );
    for json in [
        bindings::parse_midi_report(&midi).unwrap(),
        bindings::serialize_midi_report(&score_json).unwrap(),
    ] {
        let _: Value = serde_json::from_str(&json).unwrap();
    }
}

#[cfg(feature = "musicxml")]
#[test]
fn mxl_binding_returns_the_same_score_in_report_and_direct_parse() {
    let mxl = include_bytes!("../../../tests/fixtures/openscore_lieder_quilter_prelude.mxl");
    let score: Value = serde_json::from_str(&bindings::parse_mxl(mxl).unwrap()).unwrap();
    let report: Value = serde_json::from_str(&bindings::parse_mxl_report(mxl).unwrap()).unwrap();
    assert_eq!(
        report["score"]["metadata"]["title"],
        score["metadata"]["title"]
    );
    assert_eq!(
        report["score"]["parts"].as_array().unwrap().len(),
        score["parts"].as_array().unwrap().len()
    );
    assert!(
        bindings::parse_mxl_render_svg(mxl, "{}")
            .unwrap()
            .contains("<svg")
    );
}

#[test]
fn abc_bindings_keep_notes_through_edit_playback_and_export() {
    let abc = "X:1\nT:Web binding\nM:4/4\nL:1/4\nK:C\nC D E F | G A B c |\n";
    let json = bindings::parse_abc(abc).unwrap();
    let score: acorde_core::Score = serde_json::from_str(&json).unwrap();
    let report: Value = serde_json::from_str(&bindings::parse_abc_report(abc).unwrap()).unwrap();
    assert_eq!(report["score"]["metadata"]["title"], "Web binding");
    assert!(
        bindings::parse_abc_render_svg(abc, "{}")
            .unwrap()
            .contains("<svg")
    );
    let exported = bindings::serialize_abc(&json).unwrap();
    let restored: acorde_core::Score =
        serde_json::from_str(&bindings::parse_abc(&exported).unwrap()).unwrap();
    let pitches = |score: &acorde_core::Score| {
        score.parts[0].staves[0]
            .measures
            .iter()
            .flat_map(|bar| &bar.voices[0])
            .flat_map(|note| &note.pitches)
            .map(|pitch| pitch.to_midi())
            .collect::<Vec<_>>()
    };
    assert_eq!(pitches(&restored), pitches(&score));
    let _: Value = serde_json::from_str(&bindings::serialize_abc_report(&json).unwrap()).unwrap();
    let _: Value = serde_json::from_str(&bindings::validate_score(&json).unwrap()).unwrap();
    let statistics: Value =
        serde_json::from_str(&bindings::score_statistics(&json).unwrap()).unwrap();
    assert_eq!(statistics["part_count"], 1);
    let transposed: acorde_core::Score =
        serde_json::from_str(&bindings::transpose_score(&json, 2).unwrap()).unwrap();
    assert_eq!(
        pitches(&transposed),
        pitches(&score)
            .into_iter()
            .map(|pitch| pitch + 2)
            .collect::<Vec<_>>()
    );
    let extracted: acorde_core::Score =
        serde_json::from_str(&bindings::extract_part(&json, 0).unwrap()).unwrap();
    assert_eq!(pitches(&extracted), pitches(&score));
    let events: Value =
        serde_json::from_str(&bindings::to_playback_events_ex(&json, "{}").unwrap()).unwrap();
    assert_eq!(events.as_array().unwrap().len(), 8);
    assert!(bindings::score_duration_secs(&json).unwrap() > 0.0);
    assert!(bindings::score_duration_secs_region(&json, 0, 0).unwrap() > 0.0);
}

#[test]
fn json_bindings_share_layout_print_and_edit_contracts() {
    let score_json = bindings::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nC D E F | G A B c |\n").unwrap();
    let layout_config = serde_json::to_string(&acorde_layout::LayoutConfig::default()).unwrap();
    let layout = bindings::compute_layout_ex(&score_json, &layout_config).unwrap();
    let svg = bindings::render_score_svg(&score_json, "{}").unwrap();
    let with_layout = bindings::render_score_svg_with_layout(&score_json, &layout, "{}").unwrap();
    assert_eq!(svg, with_layout);
    let custom = r#"{"staff_gap":9.0,"auto_beam_unmarked":true}"#;
    assert!(
        bindings::render_score_svg_with_layout(&score_json, &layout, custom)
            .unwrap()
            .contains("<svg")
    );
    assert!(
        bindings::render_score_svg_row(&score_json, &layout, 0, custom)
            .unwrap()
            .contains("<svg")
    );
    let _: Value = serde_json::from_str(
        &bindings::render_score_metadata(&score_json, &layout, custom).unwrap(),
    )
    .unwrap();
    assert!(
        bindings::render_score_svg_row(&score_json, &layout, 0, "{}")
            .unwrap()
            .contains("<svg")
    );
    let _: Value =
        serde_json::from_str(&bindings::render_score_metadata(&score_json, &layout, "{}").unwrap())
            .unwrap();
    let _: Value = serde_json::from_str(&bindings::render_preflight(&score_json).unwrap()).unwrap();
    let print_config = serde_json::to_string(&acorde_layout::PrintConfig::default()).unwrap();
    let glyph = acorde_layout::GlyphResourceDescriptor {
        contract_version: acorde_layout::GLYPH_RESOURCE_CONTRACT_VERSION,
        resource_key: "publisher-font".into(),
        metrics_contract_version: 1,
        license_notice: "publisher license".into(),
        fallback: acorde_layout::GlyphFallbackPolicy::UseResource("acorde-vector-glyphs-v1".into()),
    };
    let glyph_json = serde_json::to_string(&glyph).unwrap();
    let validated: acorde_layout::GlyphResourceDescriptor =
        serde_json::from_str(&bindings::validate_glyph_resource_descriptor(&glyph_json).unwrap())
            .unwrap();
    assert_eq!(validated, glyph);
    let print_layout = bindings::compute_print_layout(&score_json, &print_config).unwrap();
    let pages: Vec<String> = serde_json::from_str(
        &bindings::render_print_page_svgs(&score_json, &print_layout).unwrap(),
    )
    .unwrap();
    assert!(!pages.is_empty() && pages.iter().all(|page| page.contains("<svg")));
    let trees = bindings::export_page_render_trees(&score_json, &print_config).unwrap();
    bindings::validate_page_render_trees(&score_json, &trees).unwrap();
    let changed: Value = serde_json::from_str(
        &bindings::apply_score_command(&score_json, r#"{"type":"set_tempo","bpm":90}"#).unwrap(),
    )
    .unwrap();
    assert_eq!(changed["score"]["settings"]["tempo_bpm"], 90);
    let changed_score = changed["score"].to_string();
    let patches = bindings::score_patch(&score_json, &changed_score).unwrap();
    let patched: Value =
        serde_json::from_str(&bindings::apply_score_patch(&score_json, &patches).unwrap()).unwrap();
    assert_eq!(patched["settings"]["tempo_bpm"], 90);
    let _: Value =
        serde_json::from_str(&bindings::diff_scores(&score_json, &changed_score).unwrap()).unwrap();
    let options = serde_json::to_string(&acorde_core::MusicalComparisonOptions::default()).unwrap();
    let compared: Value = serde_json::from_str(
        &bindings::compare_musical_positions(&score_json, &score_json, &options).unwrap(),
    )
    .unwrap();
    let original: acorde_core::Score = serde_json::from_str(&score_json).unwrap();
    assert_eq!(
        compared,
        serde_json::to_value(acorde_core::compare_musical_positions(
            &original,
            &original,
            &Default::default()
        ))
        .unwrap()
    );
    let _: Value =
        serde_json::from_str(&bindings::compatibility_report(&score_json, &score_json).unwrap())
            .unwrap();
    let chord_options = acorde_core::AuthoredChordOptions::default();
    let annotations: Value = serde_json::from_str(
        &bindings::annotate_authored_chords(
            &score_json,
            &score_json,
            &serde_json::to_string(&chord_options).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let native_annotations =
        acorde_core::annotate_from_authored_chords(&original, &original, &chord_options);
    assert_eq!(
        annotations,
        serde_json::to_value(native_annotations).unwrap()
    );
    let _: Value =
        serde_json::from_str(&bindings::compute_playback_position(&score_json, "{}", 0.5).unwrap())
            .unwrap();
}

#[test]
fn theory_bindings_match_native_pitch_key_and_beam_results() {
    use acorde_core::{Duration, KeySignature, Note, Pitch, Step, TimeSignature};
    let c = Pitch::new(Step::C, 4);
    let e = Pitch::new(Step::E, 4);
    let g = Pitch::new(Step::G, 4);
    let pitches = vec![c.clone(), e.clone(), g.clone()];
    let pitches_json = serde_json::to_string(&pitches).unwrap();
    let key = KeySignature {
        fifths: 0,
        mode: "major".into(),
    };
    let key_json = serde_json::to_string(&key).unwrap();
    let c_json = serde_json::to_string(&c).unwrap();
    let g_json = serde_json::to_string(&g).unwrap();
    let from_midi: Pitch =
        serde_json::from_str(&bindings::pitch_from_midi(60, false).unwrap()).unwrap();
    let from_name: Pitch = serde_json::from_str(&bindings::pitch_from_str("C4").unwrap()).unwrap();
    assert_eq!(from_midi.to_midi(), c.to_midi());
    assert_eq!(from_name.to_midi(), c.to_midi());
    let interval: Value =
        serde_json::from_str(&bindings::interval_between(&c_json, &g_json).unwrap()).unwrap();
    assert_eq!(
        interval,
        serde_json::to_value(acorde_core::Interval::between(&c, &g)).unwrap()
    );
    assert_eq!(
        bindings::key_alter_for_step(&key_json, "F").unwrap(),
        key.alter_for_step(&Step::F)
    );
    assert_eq!(
        bindings::key_contains_pitch(&key_json, &c_json).unwrap(),
        key.contains_pitch(&c)
    );
    assert_eq!(
        bindings::key_display_name(&key_json).unwrap(),
        key.display_name()
    );
    let chord_json = bindings::detect_chord(&pitches_json).unwrap();
    let chord: acorde_core::ChordSymbol = serde_json::from_str(&chord_json).unwrap();
    let roman: Value =
        serde_json::from_str(&bindings::roman_numeral(&chord_json, &key_json).unwrap()).unwrap();
    assert_eq!(
        roman,
        serde_json::to_value(acorde_core::roman_numeral(&chord, &key)).unwrap()
    );
    let scale: Value =
        serde_json::from_str(&bindings::best_fit_scale(&pitches_json).unwrap()).unwrap();
    assert_eq!(
        scale,
        serde_json::to_value(acorde_core::Scale::best_fit(&pitches)).unwrap()
    );
    let clef_json = serde_json::to_string(&acorde_core::Clef::Treble).unwrap();
    assert_eq!(bindings::clef_middle_line_midi(&clef_json).unwrap(), 71);
    let _: bool = bindings::suggested_stem_up(&pitches_json, &clef_json).unwrap();
    let notes = vec![
        Note::new(c, Duration::Eighth),
        Note::new(e, Duration::Eighth),
    ];
    let time = TimeSignature {
        numerator: 4,
        denominator: 4,
    };
    let beam_json = bindings::compute_beams(
        &serde_json::to_string(&notes).unwrap(),
        &serde_json::to_string(&time).unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&beam_json).unwrap(),
        serde_json::to_value(acorde_core::compute_beams(&notes, &time)).unwrap()
    );
    assert_eq!(
        bindings::command_key_from_json(r#"{"type":"set_tempo","bpm":90}"#).unwrap(),
        "SetTempo"
    );
    assert!(!bindings::gm_program_name(0).is_empty());
    assert!(!bindings::gm_drum_name(36).is_empty());
}

#[test]
fn stateful_web_engine_edits_undoes_and_preserves_history() {
    let source = bindings::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nC D E F | G A B c |\n").unwrap();
    let mut engine = bindings::ScoreEngine::new();
    engine.replace_score(&source).unwrap();
    let initial_version = engine.get_version();
    engine.apply(r#"{"type":"set_tempo","bpm":90}"#).unwrap();
    assert!(engine.get_version() > initial_version);
    assert_eq!(engine.get_undo_key().as_deref(), Some("SetTempo"));
    engine.undo().unwrap();
    assert_eq!(engine.get_redo_key().as_deref(), Some("SetTempo"));
    engine.redo().unwrap();
    let score: acorde_core::Score = serde_json::from_str(&engine.get_score().unwrap()).unwrap();
    assert_eq!(score.settings.tempo_bpm, 90);
    assert!(engine.get_undo_label().is_some());
    engine
        .apply_batch_labeled(
            r#"[{"type":"set_tempo","bpm":100},{"type":"set_tempo","bpm":110}]"#,
            "Tempo pair",
        )
        .unwrap();
    engine.undo().unwrap();
    let score: acorde_core::Score = serde_json::from_str(&engine.get_score().unwrap()).unwrap();
    assert_eq!(score.settings.tempo_bpm, 90);
    assert!(engine.get_redo_label().is_some());
    engine.redo().unwrap();
    let score: acorde_core::Score = serde_json::from_str(&engine.get_score().unwrap()).unwrap();
    assert_eq!(score.settings.tempo_bpm, 110);
    engine.copy_voice(0, 0, 0, 0).unwrap();
    engine.paste_voice(0, 0, 1, 0).unwrap();
    let score: acorde_core::Score = serde_json::from_str(&engine.get_score().unwrap()).unwrap();
    assert_eq!(
        score.parts[0].staves[0].measures[1].voices[0][0].pitches[0].to_midi(),
        60
    );
    let address = r#"{"part":0,"staff":0,"measure":0,"voice":0,"note":0}"#;
    engine
        .set_note_placement(address, Some(1.5), None, None, None)
        .unwrap();
    let score: acorde_core::Score = serde_json::from_str(&engine.get_score().unwrap()).unwrap();
    assert_eq!(
        score.parts[0].staves[0].measures[0].voices[0][0].offset_x,
        Some(1.5)
    );
    engine.set_tuplet(address, "null").unwrap();
    engine.respell_score(true).unwrap();
    engine.respell_score_to_key().unwrap();
    let last_note = r#"{"part":0,"staff":0,"measure":0,"voice":0,"note":3}"#;
    let next_bar = r#"{"part":0,"staff":0,"measure":1,"voice":0,"note":0}"#;
    engine.copy_range(address, last_note).unwrap();
    engine.paste_range(next_bar).unwrap();
    engine
        .apply_batch(r#"[{"type":"set_tempo","bpm":120}]"#)
        .unwrap();
    let tab = r#"{"lines":6,"tuning_midi":[40,45,50,55,59,64],"capo":0}"#;
    engine.set_tablature_config(0, 0, tab).unwrap();
    let score: acorde_core::Score = serde_json::from_str(&engine.get_score().unwrap()).unwrap();
    assert_eq!(
        score.parts[0].staves[0].tablature.as_ref().unwrap().lines,
        6
    );
    engine.set_stem(address, "true").unwrap();
    engine.set_duration(address, "\"Quarter\"", 0).unwrap();
    engine.set_arpeggio(address, "true").unwrap();
    engine.set_cue(address, true).unwrap();
    engine.set_cue(address, false).unwrap();
    engine.set_hidden(address, true).unwrap();
    engine.set_unpitched(address, true).unwrap();
    engine.set_unpitched(address, false).unwrap();
    engine.set_note_head(address, "\"Diamond\"").unwrap();
    engine.respell_staff_region(0, 0, 0, 1, "flat").unwrap();
    engine.begin_slur(address).unwrap();
    engine.end_slur(last_note).unwrap();
    engine.toggle_trill_line(address, last_note).unwrap();
    let score: acorde_core::Score = serde_json::from_str(&engine.get_score().unwrap()).unwrap();
    let first = &score.parts[0].staves[0].measures[0].voices[0][0];
    assert_eq!(first.stem_up, Some(true));
    assert!(first.hidden && first.slur_start && first.trill_line_start);
    assert_eq!(first.note_head, acorde_core::NoteHead::Diamond);
    engine.add_staff(0, "Bass").unwrap();
    engine.delete_staff(0, 1).unwrap();
    let history = engine.export_history().unwrap();
    let comparison: Value = serde_json::from_str(
        &bindings::ScoreEngine::compare_histories(&history, &history).unwrap(),
    )
    .unwrap();
    assert_eq!(comparison, "Equivalent");
    let conflict: Value =
        serde_json::from_str(&bindings::ScoreEngine::history_conflict(&history, &history).unwrap())
            .unwrap();
    assert!(conflict.is_null());
}

#[test]
fn web_history_restores_and_appends_a_simple_remote_edit() {
    let mut local = bindings::ScoreEngine::new();
    let base = local.export_history().unwrap();
    let mut remote = bindings::ScoreEngine::new();
    remote.restore_history(&base).unwrap();
    remote.apply(r#"{"type":"set_tempo","bpm":90}"#).unwrap();
    let remote_history = remote.export_history().unwrap();
    assert_eq!(local.append_history_extension(&remote_history).unwrap(), 1);
    let local_score: Value = serde_json::from_str(&local.get_score().unwrap()).unwrap();
    let remote_score: Value = serde_json::from_str(&remote.get_score().unwrap()).unwrap();
    assert_eq!(local_score, remote_score);
    let relation: Value = serde_json::from_str(
        &bindings::ScoreEngine::compare_histories(
            &local.export_history().unwrap(),
            &remote_history,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(relation, "Equivalent");
}

#[test]
fn web_analysis_cache_updates_and_invalidates_score_snapshots() {
    let score = bindings::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nC D E F |\n").unwrap();
    let edit: Value = serde_json::from_str(
        &bindings::apply_score_command(&score, r#"{"type":"set_tempo","bpm":90}"#).unwrap(),
    )
    .unwrap();
    let updated = edit["score"].to_string();
    let hint = edit["hint"].to_string();
    let mut cache = bindings::AnalysisCache::new(2).unwrap();
    assert!(cache.is_empty());
    assert_eq!(cache.capacity(), 2);
    let analysis = cache.analyze(&score).unwrap();
    assert_eq!(analysis, cache.analyze(&score).unwrap());
    let direct = bindings::analyze_score(&score).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&analysis).unwrap(),
        serde_json::from_str::<Value>(&direct).unwrap()
    );
    let native_score: acorde_core::Score = serde_json::from_str(&score).unwrap();
    assert_eq!(
        bindings::analysis_cache_key(&score).unwrap(),
        acorde_analysis::analysis_cache_key(&native_score)
    );
    let _: Value =
        serde_json::from_str(&bindings::diff_analysis(&analysis, &direct).unwrap()).unwrap();
    let address = r#"{"part":0,"staff":0,"measure":0,"voice":0,"note":0}"#;
    let _: Value =
        serde_json::from_str(&bindings::analysis_provenance(&analysis, address).unwrap()).unwrap();
    let explanation: Value = serde_json::from_str(
        &bindings::explain_analysis_change(&analysis, &direct, address).unwrap(),
    )
    .unwrap();
    assert!(explanation.is_object());
    let _: Value =
        serde_json::from_str(&bindings::affected_analysis_categories(&hint).unwrap()).unwrap();
    let _: Value = serde_json::from_str(&bindings::analysis_refresh_plan(&hint).unwrap()).unwrap();
    let scores = format!("[{score},{score}]");
    let batch: Vec<Value> = serde_json::from_str(&cache.analyze_batch(&scores).unwrap()).unwrap();
    assert_eq!(batch.len(), 2);
    assert_eq!(batch[0], batch[1]);
    let _: Value =
        serde_json::from_str(&cache.analyze_after_edit(&score, &updated).unwrap()).unwrap();
    let _: Value = serde_json::from_str(
        &cache
            .analyze_after_edit_with_diff(&score, &analysis, &updated)
            .unwrap(),
    )
    .unwrap();
    let _: Value = serde_json::from_str(
        &cache
            .analyze_selected_after_edit(&score, &analysis, &updated, "[]")
            .unwrap(),
    )
    .unwrap();
    let _: Value = serde_json::from_str(
        &cache
            .analyze_after_edit_with_hint(&score, &analysis, &updated, &hint)
            .unwrap(),
    )
    .unwrap();
    let _: Value = serde_json::from_str(&cache.stats().unwrap()).unwrap();
    assert!(cache.len() <= cache.capacity());
    cache.analyze(&updated).unwrap();
    assert!(cache.invalidate(&updated).unwrap());
    cache.reset_stats();
    cache.clear();
    assert!(cache.is_empty());
}

#[test]
fn accordion_binding_presets_replay_the_selected_arrangement() {
    let source = bindings::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nG4 |\n").unwrap();
    let selection = r#"{"chords":[{"measure_index":0,"offset_beats":0,"chord":{"root":"C","kind":"major","bass":null}}],"alternative_chords":null,"inversion":null,"bass_mode":"free-bass"}"#;
    let capabilities: Value =
        serde_json::from_str(&bindings::accordion_preset_capabilities().unwrap()).unwrap();
    assert_eq!(capabilities["contract_version"], 1);
    let analysis: Value =
        serde_json::from_str(&bindings::analyze_for_accordion(&source).unwrap()).unwrap();
    assert!(!analysis["candidates"].as_array().unwrap().is_empty());
    let arranged: acorde_core::AccordionPresetResult = serde_json::from_str(
        &bindings::arrange_accordion_preset(&source, selection, "walking-bass").unwrap(),
    )
    .unwrap();
    assert!(arranged.diagnostics.is_empty());
    let request = format!(
        r#"{{"selection":{selection},"allowed_preset_ids":["walking-bass"],"seed":"42","previous_config":null}}"#
    );
    let shuffled: acorde_core::AccordionPresetResult =
        serde_json::from_str(&bindings::shuffle_accordion_preset(&source, &request).unwrap())
            .unwrap();
    let config = serde_json::to_string(shuffled.resolved_config.as_ref().unwrap()).unwrap();
    let replay: acorde_core::AccordionPresetResult =
        serde_json::from_str(&bindings::arrange_from_resolved_preset(&source, &config).unwrap())
            .unwrap();
    assert_eq!(shuffled.resolved_config, replay.resolved_config);
    let score = serde_json::to_string(replay.score.as_ref().unwrap()).unwrap();
    let events: Vec<acorde_core::PlaybackEvent> =
        serde_json::from_str(&bindings::to_playback_events_ex(&score, "{}").unwrap()).unwrap();
    assert!(events.iter().any(|event| {
        event
            .source
            .as_ref()
            .is_some_and(|address| address.staff == 1)
    }));
}

#[test]
fn accordion_web_layers_and_drone_match_native_outputs() {
    let source = bindings::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nG4 |\n").unwrap();
    let score: acorde_core::Score = serde_json::from_str(&source).unwrap();
    let chord = serde_json::json!({"measure_index":0,"offset_beats":0.0,"chord":{"root":"C","kind":"major","bass":null}});
    let guided_json =
        serde_json::json!({"pattern":"alternating-bass-chord","chords":[chord.clone()]});
    let guided: acorde_core::GuidedAccordionOptions =
        serde_json::from_value(guided_json.clone()).unwrap();
    let web: acorde_core::GuidedAccordionResult = serde_json::from_str(
        &bindings::arrange_with_explicit_chords(&source, &guided_json.to_string()).unwrap(),
    )
    .unwrap();
    let native = acorde_core::arrange_with_explicit_chords(&score, guided);
    assert_eq!(web.diagnostics, native.diagnostics);
    assert_eq!(web.generated_notes, native.generated_notes);
    let options = serde_json::json!({"chords":[chord],"alternative_chords":null,"inversion":null,"accompaniment":"walking-bass","melodic_voice":"third-below","swing":"straight","bass_mode":"free-bass"});
    let layers: acorde_core::LayeredAccordionResult = serde_json::from_str(
        &bindings::arrange_with_layers(&source, &options.to_string()).unwrap(),
    )
    .unwrap();
    assert!(layers.diagnostics.is_empty());
    let reusable: acorde_core::AccordionLayerResult = serde_json::from_str(
        &bindings::arrange_accordion_layers(&source, &options.to_string()).unwrap(),
    )
    .unwrap();
    let config = serde_json::to_string(reusable.resolved_config.as_ref().unwrap()).unwrap();
    let replay: acorde_core::AccordionLayerResult =
        serde_json::from_str(&bindings::arrange_from_resolved_layers(&source, &config).unwrap())
            .unwrap();
    assert_eq!(reusable.generated_notes, replay.generated_notes);
    let selection = serde_json::json!({"chords":[{"measure_index":0,"offset_beats":0,"chord":{"root":"C","kind":"major","bass":null}}],"alternative_chords":null,"inversion":null,"bass_mode":"free-bass"});
    let shuffle = serde_json::json!({"selection":selection,"alternative_selections":[],"allowed_accompaniments":["walking-bass"],"allowed_melodic_voices":["third-below"],"allowed_swing_policies":["straight"],"seed":"42","previous_config":null});
    let shuffled: acorde_core::AccordionLayerResult = serde_json::from_str(
        &bindings::shuffle_accordion_layers(&source, &shuffle.to_string()).unwrap(),
    )
    .unwrap();
    let shuffled_config =
        serde_json::to_string(shuffled.resolved_config.as_ref().unwrap()).unwrap();
    let replayed: acorde_core::AccordionLayerResult = serde_json::from_str(
        &bindings::arrange_from_resolved_layers(&source, &shuffled_config).unwrap(),
    )
    .unwrap();
    assert_eq!(shuffled.generated_notes, replayed.generated_notes);
    let drone_options = serde_json::json!({"pitch_midi":48,"bass_mode":"free-bass","layers":null});
    let drone: acorde_core::AccordionDroneResult = serde_json::from_str(
        &bindings::arrange_with_drone(&source, &drone_options.to_string()).unwrap(),
    )
    .unwrap();
    assert!(drone.diagnostics.is_empty());
    assert!(!drone.generated_notes.is_empty());
}

#[test]
fn accordion_web_reduction_reports_the_same_selection_as_native() {
    let xml = include_str!("../../../tests/fixtures/multipart.musicxml");
    let json = bindings::parse_musicxml(xml).unwrap();
    let score: acorde_core::Score = serde_json::from_str(&json).unwrap();
    let options = acorde_core::AccordionReductionOptions {
        right_hand_part_index: Some(0),
        left_hand_part_indices: vec![1],
        loss_policy: acorde_core::AccordionLossPolicy::RequireAll,
        octave_fit: false,
    };
    let native = acorde_core::arrange_for_accordion_with_options(&score, options.clone());
    let web: acorde_core::AccordionReductionResult = serde_json::from_str(
        &bindings::arrange_for_accordion_with_options(
            &json,
            &serde_json::to_string(&options).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(web.diagnostics, native.diagnostics);
    assert_eq!(web.source_map, native.source_map);
    let legacy: Value =
        serde_json::from_str(&bindings::arrange_for_accordion(&json, 0).unwrap()).unwrap();
    assert!(legacy["score"].is_object());
}

#[cfg(feature = "soundfont")]
#[test]
fn playback_and_soundfont_bindings_report_real_inputs() {
    let score = bindings::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nC D E F |\n").unwrap();
    let events_json = bindings::to_playback_events_ex(&score, "{}").unwrap();
    let events: Vec<acorde_core::PlaybackEvent> = serde_json::from_str(&events_json).unwrap();
    let tolerance = acorde_core::PlaybackTimingTolerance::default();
    let result: Value = serde_json::from_str(
        &bindings::compare_playback_timing(
            &events_json,
            &events_json,
            &serde_json::to_string(&tolerance).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        result,
        serde_json::to_value(
            acorde_core::compare_playback_timing(&events, &events, &tolerance).unwrap()
        )
        .unwrap()
    );
    let soundfont = include_bytes!("../../../tests/fixtures/UprightPianoKW-small-20190703.sf2");
    let snapshot: Value = serde_json::from_str(
        &bindings::soundfont_preset_snapshot(soundfont, "test-provider", 0, 0).unwrap(),
    )
    .unwrap();
    assert_eq!(snapshot["provider_version"], "test-provider");
    assert!(!snapshot["zones"].as_array().unwrap().is_empty());
}

#[test]
fn tablature_bindings_match_native_performance_and_roundtrip_reports() {
    let xml = include_str!("../../../tests/fixtures/external_tab.musicxml");
    let json = bindings::parse_musicxml(xml).unwrap();
    let score: acorde_core::Score = serde_json::from_str(&json).unwrap();
    let web: Value =
        serde_json::from_str(&bindings::project_tablature_performance(&json, "{}").unwrap())
            .unwrap();
    let native = acorde_core::project_tablature_performance(&score, &Default::default()).unwrap();
    assert_eq!(web, serde_json::to_value(native).unwrap());
    let web: Value =
        serde_json::from_str(&bindings::tablature_round_trip_report(&json).unwrap()).unwrap();
    let native = acorde_core::tablature_round_trip_report(&score).unwrap();
    assert_eq!(web, serde_json::to_value(native).unwrap());
    let assigned: acorde_core::Score =
        serde_json::from_str(&bindings::assign_tablature_positions(&json).unwrap()).unwrap();
    assert_eq!(assigned.parts.len(), score.parts.len());
}

#[test]
fn web_fragment_copy_and_linked_view_use_canonical_addresses() {
    let source = bindings::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nC D E F | G A B c |\n").unwrap();
    let mut score: acorde_core::Score = serde_json::from_str(&source).unwrap();
    score
        .views
        .push(acorde_core::ScoreView::linked_part("piano", "Piano", 0));
    let source = serde_json::to_string(&score).unwrap();
    let start = acorde_core::NoteAddr {
        part: 0,
        staff: 0,
        measure: 0,
        voice: 0,
        note: 0,
    };
    let end = acorde_core::NoteAddr {
        note: 3,
        ..start.clone()
    };
    let target = acorde_core::NoteAddr {
        measure: 1,
        note: 0,
        ..start.clone()
    };
    let selections = vec![acorde_core::ScoreFragmentSelection { start, end }];
    let fragment =
        bindings::extract_score_fragment(&source, &serde_json::to_string(&selections).unwrap())
            .unwrap();
    let web: Value = serde_json::from_str(&fragment).unwrap();
    assert_eq!(
        web,
        serde_json::to_value(acorde_core::extract_score_fragment(&score, &selections).unwrap())
            .unwrap()
    );
    let mut engine = bindings::ScoreEngine::new();
    engine.replace_score(&source).unwrap();
    engine
        .paste_score_fragment(&fragment, &serde_json::to_string(&target).unwrap())
        .unwrap();
    let pasted: acorde_core::Score = serde_json::from_str(&engine.get_score().unwrap()).unwrap();
    assert_eq!(
        pasted.parts[0].staves[0].measures[1].voices[0][0].pitches[0].to_midi(),
        60
    );
    let plan: Value = serde_json::from_str(
        &bindings::plan_structural_change(
            &source,
            r#"{"type":"split_measure","measure_index":0,"split_at_beats":2.0}"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(plan.is_object());
    let config = serde_json::to_string(&acorde_layout::PrintConfig::default()).unwrap();
    let layout: acorde_layout::PrintLayoutResult = serde_json::from_str(
        &bindings::compute_print_layout_for_view(&source, &config, "piano").unwrap(),
    )
    .unwrap();
    assert!(!layout.pages.is_empty());
    let trees: Value = serde_json::from_str(
        &bindings::export_page_render_trees_for_view(&source, &config, "piano").unwrap(),
    )
    .unwrap();
    assert!(!trees.as_array().unwrap().is_empty());
}

#[test]
fn playback_options_share_only_selected_dynamic_marks() {
    use acorde_core::{Clef, Duration, Dynamic, Measure, Note, Pitch, Score, Staff, Step};
    let mut score = Score::new("shared", 120, 4, 4, 0, 1);
    let mut first = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
    first.dynamic = Some(Dynamic::P);
    score.parts[0].staves[0].measures[0].voices[0] = vec![first];
    let mut lower = Staff::new(Clef::Bass);
    let mut measure = Measure::empty(4, 4);
    measure.voices[0] = vec![Note::new(Pitch::new(Step::C, 3), Duration::Whole)];
    lower.measures.push(measure);
    score.parts[0].staves.push(lower);
    let json = serde_json::to_string(&score).unwrap();
    let options = r#"{"shared_dynamics":[{"part":0,"staff":0,"measure":0,"voice":0,"note":0}]}"#;
    let events: Vec<acorde_core::PlaybackEvent> =
        serde_json::from_str(&bindings::to_playback_events_ex(&json, options).unwrap()).unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|event| event.velocity == 48));
}

#[test]
fn musicxml_measured_tremolo_is_repeated_in_default_playback_binding() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type><notations><ornaments><tremolo type="single">3</tremolo></ornaments></notations></note><note><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note></measure></part></score-partwise>"#;
    let score = bindings::parse_musicxml(xml).unwrap();
    let events: Vec<Value> =
        serde_json::from_str(&bindings::to_playback_events_ex(&score, "{}").unwrap()).unwrap();
    assert_eq!(events.len(), 17);
    assert_eq!(events[15]["time_beats"], 1.875);
    assert_eq!(events[16]["time_beats"], 2.0);
}

#[test]
fn timed_musicxml_pedal_is_available_in_offline_manifest() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><direction><direction-type><pedal type="start" line="no"/></direction-type></direction><direction><direction-type><pedal type="stop" line="no"/></direction-type><offset>3</offset></direction><note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure></part></score-partwise>"#;
    let score = bindings::parse_musicxml(xml).unwrap();
    let manifest: Value = serde_json::from_str(
        &bindings::build_offline_render_manifest(
            &score,
            r#"{"bpm_override":60}"#,
            r#"{"sample_rate_hz":8000}"#,
        )
        .unwrap(),
    )
    .unwrap();
    let events = manifest["semantic_events"].as_array().unwrap();
    let pedals: Vec<_> = events
        .iter()
        .filter(|event| event["kind"]["pedal"].is_object())
        .map(|event| event["frame"].as_u64().unwrap())
        .collect();
    assert_eq!(pedals, vec![0, 24000]);
}
