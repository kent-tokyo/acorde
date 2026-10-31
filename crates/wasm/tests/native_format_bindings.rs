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
