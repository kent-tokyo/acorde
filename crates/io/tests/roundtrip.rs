use acorde_core::{
    AddMeasureCmd, AddNoteCmd, Clef, Command, Duration, NoteAddr, Pitch, ScoreEngine,
    ScoreFragmentSelection, SetMeasureTextCmd, Step, StyledText, TextStyle, extract_score_fragment,
};
/// Integration tests: parse a fixture, serialize, re-parse, and verify
/// that key musical properties are preserved across the round-trip.
use acorde_io::{parse_midi, parse_musicxml, serialize_musicxml};

// Fixtures live at the workspace root under tests/fixtures/.
// include_str! paths are relative to this source file:
//   crates/io/tests/roundtrip.rs  →  ../../.. → workspace root → tests/fixtures/
static SIMPLE_XML: &str = include_str!("../../../tests/fixtures/simple.musicxml");
static MULTIPART_XML: &str = include_str!("../../../tests/fixtures/multipart.musicxml");
static MULTIVOICE_XML: &str = include_str!("../../../tests/fixtures/multivoice.musicxml");
static FRAGMENT_RICH_XML: &str = include_str!("../../../tests/fixtures/fragment_rich.musicxml");
static CROSS_STAFF_FRAGMENT_XML: &str =
    include_str!("../../../tests/fixtures/cross_staff_fragment.musicxml");
static SECTION_BREAKS_XML: &str = include_str!("../../../tests/fixtures/section_breaks.musicxml");
static DECLARED_STAVES_XML: &str =
    include_str!("../../../tests/fixtures/declared_staves_cross_staff.musicxml");
static FIXTURE_MANIFEST: &str = include_str!("../../../tests/fixtures/manifest.json");
static INTERCHANGE_REPORT: &str = include_str!("../../../docs/interchange-report.json");
static WORKSPACE_MANIFEST: &str = include_str!("../../../Cargo.toml");
#[cfg(all(feature = "mei", feature = "mscz"))]
static INTERCHANGE_MEI: &str = include_str!("../../../tests/fixtures/interchange_subset.mei");
#[cfg(feature = "mei")]
static INTERCHANGE_MULTISTAFF_MEI: &str =
    include_str!("../../../tests/fixtures/interchange_multistaff.mei");
#[cfg(feature = "mei")]
static INTERCHANGE_HARM_ANALYSIS_MEI: &str =
    include_str!("../../../tests/fixtures/interchange_harm_analysis.mei");
#[cfg(all(feature = "mei", feature = "mscz"))]
static INTERCHANGE_MSCX: &str = include_str!("../../../tests/fixtures/interchange_subset.mscx");
#[cfg(feature = "mscz")]
static INTERCHANGE_FIGURED_BASS_MSCX: &str =
    include_str!("../../../tests/fixtures/interchange_figured_bass.mscx");
#[cfg(feature = "mscz")]
static OPENSCORE_LIEDER_MSCX: &str =
    include_str!("../../../tests/fixtures/openscore_lieder_aloha_oe.mscx");
#[cfg(feature = "mscz")]
static OPENSCORE_OMR_MSCZ: &[u8] =
    include_bytes!("../../../tests/fixtures/openscore_omr_score_1003.mscz");
#[cfg(feature = "mscz")]
static OPENSCORE_OMR_MSCZ_SECOND: &[u8] =
    include_bytes!("../../../tests/fixtures/openscore_omr_score_1033.mscz");
#[cfg(feature = "mscz")]
static OPENSCORE_OMR_MSCZ_THIRD: &[u8] =
    include_bytes!("../../../tests/fixtures/openscore_omr_score_1035.mscz");
#[cfg(feature = "mscz")]
static OPENSCORE_OMR_MSCZ_FOURTH: &[u8] =
    include_bytes!("../../../tests/fixtures/openscore_omr_score_1036.mscz");
#[cfg(feature = "mscz")]
static OPENSCORE_OMR_MSCZ_FIFTH: &[u8] =
    include_bytes!("../../../tests/fixtures/openscore_omr_score_1016.mscz");
#[cfg(feature = "mscz")]
static OPENSCORE_OMR_MSCX: &str =
    include_str!("../../../tests/fixtures/openscore_omr_score_1003.mscx");
#[cfg(feature = "mscz")]
static OPENSCORE_OMR_MSCX_SECOND: &str =
    include_str!("../../../tests/fixtures/openscore_omr_score_1033.mscx");
static JUST_PERFECT_FIFTH_MIDI: &[u8] =
    include_bytes!("../../../tests/fixtures/just_perfect_fifth_on_c.mid");
static FOUR_STEPS_31ET_MIDI: &[u8] =
    include_bytes!("../../../tests/fixtures/4_steps_in_31-et_on_c.mid");
static SEPTIMAL_MAJOR_THIRD_MIDI: &[u8] =
    include_bytes!("../../../tests/fixtures/septimal_major_third_on_c.mid");

// ── helpers ───────────────────────────────────────────────────────────────────

fn notes_in(score: &acorde_core::Score, part: usize, measure: usize) -> &[acorde_core::Note] {
    &score.parts[part].staves[0].measures[measure].voices[0]
}

// ── simple.musicxml ───────────────────────────────────────────────────────────

#[test]
fn simple_musicxml_parses() {
    let score = parse_musicxml(SIMPLE_XML).expect("parse failed");
    assert_eq!(score.metadata.title, "Simple Test");
    assert_eq!(score.parts.len(), 1);
    assert_eq!(score.parts[0].staves[0].measures.len(), 2);

    // Measure 1: C D E F (quarter notes)
    let notes = notes_in(&score, 0, 0);
    assert_eq!(notes.len(), 4);
    assert_eq!(notes[0].pitches[0].step, Step::C);
    assert_eq!(notes[1].pitches[0].step, Step::D);
    assert_eq!(notes[2].pitches[0].step, Step::E);
    assert_eq!(notes[3].pitches[0].step, Step::F);
    for n in notes {
        assert_eq!(n.duration, Duration::Quarter);
        assert!(!n.is_rest);
    }

    // Measure 2: G half + half rest
    let notes2 = notes_in(&score, 0, 1);
    assert_eq!(notes2[0].pitches[0].step, Step::G);
    assert_eq!(notes2[0].duration, Duration::Half);
    assert!(notes2[1].is_rest);
}

#[test]
fn musicxml_typed_spanner_survives_structural_edit_and_roundtrip() {
    let xml = r#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1"><measure number="1"><attributes><divisions>480</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
    <note><pitch><step>C</step><octave>4</octave></pitch><duration>480</duration><type>quarter</type><notations><slur number="1" type="start"/></notations></note>
    <note><pitch><step>D</step><octave>4</octave></pitch><duration>480</duration><type>quarter</type><notations><slur number="1" type="stop"/></notations></note>
    <note><pitch><step>E</step><octave>4</octave></pitch><duration>480</duration><type>quarter</type></note>
    <note><pitch><step>F</step><octave>4</octave></pitch><duration>480</duration><type>quarter</type></note>
  </measure></part>
</score-partwise>"#;
    let mut score = parse_musicxml(xml).expect("parse typed span");
    acorde_core::model::commands::apply_command(
        &Command::AddNote(AddNoteCmd {
            part_index: 0,
            staff_index: 0,
            measure_index: 0,
            voice: 0,
            position: 0,
            pitch: Some(Pitch::new(Step::B, 3)),
            duration: Duration::Quarter,
            dot_count: 0,
            is_rest: false,
            tuplet: None,
        }),
        &mut score,
    )
    .expect("structural edit applies");
    assert_eq!(score.spanners.len(), 1);
    assert_eq!(score.spanners[0].start.note, 1);
    assert_eq!(score.spanners[0].end.note, 2);

    let restored = parse_musicxml(&serialize_musicxml(&score).expect("serialize edited score"))
        .expect("reparse edited score");
    assert_eq!(restored.spanners.len(), 1);
    assert_eq!(restored.spanners[0].start.note, 1);
    assert_eq!(restored.spanners[0].end.note, 2);
}

#[test]
fn styled_measure_text_command_roundtrips_supported_musicxml_styles() {
    let mut score = parse_musicxml(SIMPLE_XML).expect("parse fixture");
    for (text_index, styled) in [
        StyledText {
            style: TextStyle::Expression,
            text: "dolce".to_string(),
            placement: None,
            offset_x: None,
            offset_y: None,
            relative_x: None,
            relative_y: None,
        },
        StyledText {
            style: TextStyle::RehearsalMark,
            text: "A".to_string(),
            placement: Some("below".to_string()),
            offset_x: None,
            offset_y: None,
            relative_x: None,
            relative_y: None,
        },
    ]
    .into_iter()
    .enumerate()
    {
        acorde_core::model::commands::apply_command(
            &Command::SetMeasureText(SetMeasureTextCmd {
                part_index: 0,
                staff_index: 0,
                measure_index: 0,
                text_index,
                text: Some(styled),
            }),
            &mut score,
        )
        .expect("styled text command applies");
    }
    let serialized = serialize_musicxml(&score).expect("serialize styled text");
    let restored = parse_musicxml(&serialized).expect("reparse styled text");
    let texts = &restored.parts[0].staves[0].measures[0].texts;
    assert!(texts.contains(&StyledText {
        style: TextStyle::Expression,
        text: "dolce".to_string(),
        placement: Some("above".to_string()),
        offset_x: None,
        offset_y: None,
        relative_x: None,
        relative_y: None,
    }));
    assert!(texts.contains(&StyledText {
        style: TextStyle::RehearsalMark,
        text: "A".to_string(),
        placement: Some("below".to_string()),
        offset_x: None,
        offset_y: None,
        relative_x: None,
        relative_y: None,
    }));
}

#[test]
fn musicxml_direction_default_offsets_roundtrip_on_styled_text() {
    let xml = SIMPLE_XML.replacen(
        "<note",
        "<direction placement=\"below\" default-x=\"12.5\" default-y=\"-3\" relative-x=\"1.25\" relative-y=\"-0.5\"><direction-type><words>dolce</words></direction-type></direction><note",
        1,
    );
    let score = parse_musicxml(&xml).expect("parse positioned direction");
    let styled = score.parts[0].staves[0].measures[0]
        .texts
        .iter()
        .find(|text| text.text == "dolce")
        .expect("positioned direction text");
    assert_eq!(styled.placement.as_deref(), Some("below"));
    assert_eq!(styled.offset_x, Some(12.5));
    assert_eq!(styled.offset_y, Some(-3.0));
    assert_eq!(styled.relative_x, Some(1.25));
    assert_eq!(styled.relative_y, Some(-0.5));

    let restored =
        parse_musicxml(&serialize_musicxml(&score).expect("serialize positioned direction"))
            .expect("reparse positioned direction");
    let restored_text = restored.parts[0].staves[0].measures[0]
        .texts
        .iter()
        .find(|text| text.text == "dolce")
        .expect("reparsed positioned direction text");
    assert_eq!(restored_text.placement.as_deref(), Some("below"));
    assert_eq!(restored_text.offset_x, Some(12.5));
    assert_eq!(restored_text.offset_y, Some(-3.0));
    assert_eq!(restored_text.relative_x, Some(1.25));
    assert_eq!(restored_text.relative_y, Some(-0.5));
}

#[test]
fn musicxml_note_placement_offsets_roundtrip_and_are_not_reported_as_loss() {
    // default-x/default-y are the source engraver's absolute layout, not nudges: they are not
    // imported (applying them to acorde's own layout pushed notes out of their bars).
    let xml = SIMPLE_XML.replacen(
        "<note",
        "<note default-x=\"12.5\" default-y=\"-3\" relative-x=\"1.25\" relative-y=\"-0.5\"",
        1,
    );
    let mut score = parse_musicxml(&xml).expect("parse positioned note");
    let note = notes_in(&score, 0, 0)
        .iter()
        .find(|note| !note.is_rest)
        .expect("positioned note");
    assert_eq!(note.offset_x, None);
    assert_eq!(note.offset_y, None);
    assert_eq!(note.relative_x, Some(1.25));
    assert_eq!(note.relative_y, Some(-0.5));

    // A host nudge in offset_* exports as part of relative-x/-y, so the note moves the same.
    let index = notes_in(&score, 0, 0)
        .iter()
        .position(|note| !note.is_rest)
        .expect("note index");
    score.parts[0].staves[0].measures[0].voices[0][index].offset_x = Some(12.5);
    score.parts[0].staves[0].measures[0].voices[0][index].offset_y = Some(-3.0);
    let written = serialize_musicxml(&score).expect("serialize positioned note");
    assert!(!written.contains("<note default-x"));
    let restored = parse_musicxml(&written).expect("reparse positioned note");
    let restored_note = notes_in(&restored, 0, 0)
        .iter()
        .find(|note| !note.is_rest)
        .expect("reparsed positioned note");
    assert_eq!(restored_note.relative_x, Some(13.75));
    assert_eq!(restored_note.relative_y, Some(-3.5));
}

#[test]
fn simple_musicxml_roundtrip_preserves_structure() {
    let score1 = parse_musicxml(SIMPLE_XML).expect("first parse failed");
    let xml2 = serialize_musicxml(&score1).expect("serialize failed");
    let score2 = parse_musicxml(&xml2).expect("second parse failed");

    assert_eq!(score1.metadata.title, score2.metadata.title);
    assert_eq!(score1.parts.len(), score2.parts.len());
    assert_eq!(score1.settings.tempo_bpm, score2.settings.tempo_bpm);
    assert_eq!(
        score1.settings.time_signature,
        score2.settings.time_signature
    );

    let m1 = &score1.parts[0].staves[0].measures;
    let m2 = &score2.parts[0].staves[0].measures;
    assert_eq!(m1.len(), m2.len());

    for (ma, mb) in m1.iter().zip(m2.iter()) {
        let va = &ma.voices[0];
        let vb = &mb.voices[0];
        assert_eq!(va.len(), vb.len(), "voice length mismatch in measure");
        for (na, nb) in va.iter().zip(vb.iter()) {
            assert_eq!(na.is_rest, nb.is_rest);
            assert_eq!(na.duration, nb.duration);
            assert_eq!(na.dot_count, nb.dot_count);
            if !na.is_rest {
                assert_eq!(na.pitches[0].step, nb.pitches[0].step);
                assert_eq!(na.pitches[0].octave, nb.pitches[0].octave);
                assert_eq!(na.pitches[0].alter, nb.pitches[0].alter);
            }
        }
    }
}

#[test]
fn rich_musicxml_fragment_fixture_roundtrips_and_pastes_undoably() {
    let score = parse_musicxml(FRAGMENT_RICH_XML).expect("rich fixture parses");
    assert_eq!(score.parts[0].staves[0].measures.len(), 2);
    assert_eq!(
        score.parts[0].staves[0].measures[0].source_voice_numbers,
        [Some(1), Some(5), None, None]
    );
    assert_eq!(score.spanners.len(), 1);
    assert!(score.parts[0].staves[0].measures[0].voices[0][0].is_rest);
    assert!(
        score.parts[0].staves[0].measures[0].voices[0][1..4]
            .iter()
            .all(|note| note
                .tuplet
                .as_ref()
                .is_some_and(|tuplet| { (tuplet.actual_notes, tuplet.normal_notes) == (3, 2) }))
    );
    assert_eq!(
        score.parts[0].staves[0].measures[0].voices[0][1]
            .lyric
            .as_ref()
            .map(|lyric| lyric.text.as_str()),
        Some("frag")
    );
    let restored = parse_musicxml(&serialize_musicxml(&score).expect("rich fixture serializes"))
        .expect("rich fixture reparses");
    assert!(restored.parts[0].staves[0].measures[0].voices[0][0].is_rest);
    assert!(
        restored.parts[0].staves[0].measures[0].voices[0][1..4]
            .iter()
            .all(|note| note
                .tuplet
                .as_ref()
                .is_some_and(|tuplet| { (tuplet.actual_notes, tuplet.normal_notes) == (3, 2) }))
    );

    let mut engine = ScoreEngine::new();
    engine.try_replace_score(restored).expect("score is valid");
    engine
        .apply(Command::AddMeasure(AddMeasureCmd { after_index: 1 }))
        .expect("third measure adds");
    engine
        .apply(Command::AddMeasure(AddMeasureCmd { after_index: 2 }))
        .expect("fourth measure adds");
    let start = NoteAddr {
        part: 0,
        staff: 0,
        measure: 0,
        voice: 0,
        note: 0,
    };
    let fragment = extract_score_fragment(
        &engine.score,
        &[
            ScoreFragmentSelection {
                start: start.clone(),
                end: NoteAddr {
                    measure: 1,
                    ..start.clone()
                },
            },
            ScoreFragmentSelection {
                start: NoteAddr {
                    voice: 1,
                    ..start.clone()
                },
                end: NoteAddr {
                    measure: 1,
                    voice: 1,
                    ..start.clone()
                },
            },
        ],
    )
    .expect("fragment extracts");
    let fragment = serde_json::from_str(
        &serde_json::to_string(&fragment).expect("fragment serializes as JSON"),
    )
    .expect("fragment reparses from JSON");
    engine
        .paste_score_fragment(
            fragment,
            NoteAddr {
                measure: 2,
                ..start.clone()
            },
        )
        .expect("fragment pastes");
    assert_eq!(engine.score.spanners.len(), 2);
    assert!(engine.score.parts[0].staves[0].measures[2].voices[0][0].is_rest);
    assert!(
        engine.score.parts[0].staves[0].measures[2].voices[0][1..4]
            .iter()
            .all(|note| note
                .tuplet
                .as_ref()
                .is_some_and(|tuplet| { (tuplet.actual_notes, tuplet.normal_notes) == (3, 2) }))
    );
    let pasted_slur = engine
        .score
        .spanners
        .iter()
        .find(|spanner| spanner.start.measure == 2)
        .expect("pasted cross-measure slur");
    assert_eq!((pasted_slur.start.measure, pasted_slur.end.measure), (2, 3));
    assert_eq!(
        engine.score.parts[0].staves[0].measures[2].source_voice_numbers,
        [Some(1), Some(5), None, None]
    );
    engine.undo().expect("paste undoes");
    assert_eq!(engine.score.spanners.len(), 1);
    engine.redo().expect("paste redoes");
    assert_eq!(engine.score.spanners.len(), 2);
}

#[test]
fn musicxml_section_break_roundtrips_without_becoming_a_layout_break() {
    let mut score = parse_musicxml(SIMPLE_XML).expect("fixture parses");
    score.parts[0].staves[0].measures[1].section_break = true;
    let xml = serialize_musicxml(&score).expect("serializes section break");
    assert!(xml.contains("<other-direction>acorde:section-break</other-direction>"));
    assert!(!xml.contains("<print new-system=\"yes\""));
    let restored = parse_musicxml(&xml).expect("reparses section break");
    let measure = &restored.parts[0].staves[0].measures[1];
    assert!(measure.section_break);
    assert!(!measure.system_break);
    assert!(!measure.page_break);
}

#[test]
fn multipart_section_break_fixture_preserves_leading_middle_and_trailing_boundaries() {
    let score = parse_musicxml(SECTION_BREAKS_XML).expect("section-break fixture parses");
    assert_eq!(score.parts.len(), 2);
    for part in &score.parts {
        for staff in &part.staves {
            assert_eq!(staff.measures.len(), 3);
            assert!(staff.measures.iter().all(|measure| measure.section_break));
            assert!(
                staff
                    .measures
                    .iter()
                    .all(|measure| !measure.system_break && !measure.page_break)
            );
        }
    }

    let restored =
        parse_musicxml(&serialize_musicxml(&score).expect("section-break fixture serializes"))
            .expect("section-break fixture reparses");
    for part in &restored.parts {
        for staff in &part.staves {
            assert!(staff.measures.iter().all(|measure| measure.section_break));
        }
    }
}

#[test]
fn cross_staff_musicxml_fragment_pastes_into_piano_without_losing_voice_identity() {
    let parsed = parse_musicxml(CROSS_STAFF_FRAGMENT_XML).expect("cross-staff fixture parses");
    let xml = serialize_musicxml(&parsed).expect("cross-staff fixture serializes");
    assert!(xml.contains("<staff>2</staff>"));
    let parsed = parse_musicxml(&xml).expect("cross-staff fixture reparses");
    let source = &parsed.parts[0].staves[0].measures[0];
    assert_eq!(source.source_voice_numbers, [Some(5), None, None, None]);
    assert_eq!(
        source.voices[0][0]
            .cross_staff
            .as_ref()
            .map(|value| value.target_staff),
        Some(1)
    );
    let fragment = extract_score_fragment(
        &parsed,
        &[ScoreFragmentSelection {
            start: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 0,
            },
            end: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 0,
            },
        }],
    )
    .expect("cross-staff fragment extracts");

    let mut engine = ScoreEngine::new();
    engine
        .try_replace_score(acorde_core::Score::template(
            acorde_core::ScoreTemplate::Piano,
        ))
        .expect("piano target validates");
    engine
        .paste_score_fragment(
            fragment,
            NoteAddr {
                part: 0,
                staff: 0,
                measure: 1,
                voice: 0,
                note: 0,
            },
        )
        .expect("cross-staff fragment pastes");
    let pasted = &engine.score.parts[0].staves[0].measures[1];
    assert_eq!(pasted.source_voice_numbers, [Some(5), None, None, None]);
    assert_eq!(
        pasted.voices[0][0]
            .cross_staff
            .as_ref()
            .map(|value| value.target_staff),
        Some(1)
    );
}

#[test]
fn exchange_voices_preserves_sparse_musicxml_voice_identity_and_backup_structure() {
    let score = parse_musicxml(MULTIVOICE_XML).expect("multivoice fixture parses");
    let mut engine = ScoreEngine::new();
    engine.try_replace_score(score).expect("fixture validates");
    engine
        .exchange_voices(0, 0, 0, 0, 0, 1)
        .expect("voices exchange");
    let measure = &engine.score.parts[0].staves[0].measures[0];
    assert_eq!(measure.source_voice_numbers, [Some(2), Some(1), None, None]);
    assert_eq!(measure.voices[0][0].pitches[0].octave, 3);
    assert_eq!(measure.voices[1][0].pitches[0].octave, 4);

    let xml = serialize_musicxml(&engine.score).expect("exchanged score serializes");
    assert!(xml.contains("<backup>"));
    let restored = parse_musicxml(&xml).expect("exchanged score reparses");
    let restored = &restored.parts[0].staves[0].measures[0];
    let voice_octaves: std::collections::BTreeMap<u32, i8> = restored
        .source_voice_numbers
        .iter()
        .enumerate()
        .filter_map(|(slot, source_voice)| {
            source_voice
                .map(|source_voice| (source_voice, restored.voices[slot][0].pitches[0].octave))
        })
        .collect();
    assert_eq!(voice_octaves.get(&1), Some(&4));
    assert_eq!(voice_octaves.get(&2), Some(&3));
}

#[test]
fn fixture_scores_have_deterministic_json_and_roundtrip_identity() {
    for fixture in [SIMPLE_XML, MULTIPART_XML, MULTIVOICE_XML] {
        let score = parse_musicxml(fixture).expect("fixture parses");
        let first = serde_json::to_string(&score).expect("score serializes");
        let second = serde_json::to_string(&score).expect("score serializes deterministically");
        assert_eq!(first, second);
        let restored: acorde_core::Score =
            serde_json::from_str(&first).expect("score JSON round-trips");
        let restored_json = serde_json::to_string(&restored).expect("restored score serializes");
        assert_eq!(restored_json, first);
    }
}

#[test]
fn fixture_manifest_has_pinned_evidence_contract() {
    let manifest: serde_json::Value =
        serde_json::from_str(FIXTURE_MANIFEST).expect("fixture manifest is valid JSON");
    assert_eq!(manifest["schema_version"], 1);
    let fixtures = manifest["fixtures"]
        .as_array()
        .expect("fixture manifest has an array");
    assert!(fixtures.len() >= 7);

    for fixture in fixtures {
        let path = fixture["path"].as_str().expect("fixture path");
        assert!(!path.is_empty());
        assert!(fixture["format"].as_str().is_some());
        assert!(fixture["license"].as_str().is_some());
        let checksum = fixture["sha256"].as_str().expect("fixture checksum");
        assert_eq!(checksum.len(), 64);
        assert!(checksum.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert!(matches!(
            fixture["round_trip"].as_str(),
            Some("semantic") | Some("import-only") | Some("decode-render")
        ));
        assert!(fixture["expected_losses"].as_array().is_some());
    }
}

#[test]
fn fixture_manifest_sha256_matches_checked_in_files() {
    use sha2::{Digest, Sha256};

    let manifest: serde_json::Value =
        serde_json::from_str(FIXTURE_MANIFEST).expect("fixture manifest is valid JSON");
    for fixture in manifest["fixtures"].as_array().expect("fixture array") {
        let path = fixture["path"].as_str().expect("fixture path");
        let expected = fixture["sha256"].as_str().expect("fixture sha256");
        let bytes: &[u8] = match path {
            "simple.musicxml" => include_bytes!("../../../tests/fixtures/simple.musicxml"),
            "multipart.musicxml" => include_bytes!("../../../tests/fixtures/multipart.musicxml"),
            "fragment_rich.musicxml" => {
                include_bytes!("../../../tests/fixtures/fragment_rich.musicxml")
            }
            "external_tab.musicxml" => {
                include_bytes!("../../../tests/fixtures/external_tab.musicxml")
            }
            "section_breaks.musicxml" => {
                include_bytes!("../../../tests/fixtures/section_breaks.musicxml")
            }
            "multivoice.musicxml" => include_bytes!("../../../tests/fixtures/multivoice.musicxml"),
            "declared_staves_cross_staff.musicxml" => {
                include_bytes!("../../../tests/fixtures/declared_staves_cross_staff.musicxml")
            }
            "render_preflight_unsupported.musicxml" => {
                include_bytes!("../../../tests/fixtures/render_preflight_unsupported.musicxml")
            }
            "interchange_subset.mei" => {
                include_bytes!("../../../tests/fixtures/interchange_subset.mei")
            }
            "interchange_multistaff.mei" => {
                include_bytes!("../../../tests/fixtures/interchange_multistaff.mei")
            }
            "interchange_harm_analysis.mei" => {
                include_bytes!("../../../tests/fixtures/interchange_harm_analysis.mei")
            }
            "interchange_subset.mscx" => {
                include_bytes!("../../../tests/fixtures/interchange_subset.mscx")
            }
            "interchange_figured_bass.mscx" => {
                include_bytes!("../../../tests/fixtures/interchange_figured_bass.mscx")
            }
            "openscore_lieder_aloha_oe.mscx" => {
                include_bytes!("../../../tests/fixtures/openscore_lieder_aloha_oe.mscx")
            }
            "openscore_omr_score_1003.mscz" => {
                include_bytes!("../../../tests/fixtures/openscore_omr_score_1003.mscz")
            }
            "openscore_omr_score_1033.mscz" => {
                include_bytes!("../../../tests/fixtures/openscore_omr_score_1033.mscz")
            }
            "openscore_omr_score_1035.mscz" => {
                include_bytes!("../../../tests/fixtures/openscore_omr_score_1035.mscz")
            }
            "openscore_omr_score_1036.mscz" => {
                include_bytes!("../../../tests/fixtures/openscore_omr_score_1036.mscz")
            }
            "openscore_omr_score_1016.mscz" => {
                include_bytes!("../../../tests/fixtures/openscore_omr_score_1016.mscz")
            }
            "openscore_omr_score_1003.mscx" => {
                include_bytes!("../../../tests/fixtures/openscore_omr_score_1003.mscx")
            }
            "openscore_omr_score_1033.mscx" => {
                include_bytes!("../../../tests/fixtures/openscore_omr_score_1033.mscx")
            }
            "sample.abc" => include_bytes!("../../../tests/fixtures/sample.abc"),
            "just_perfect_fifth_on_c.mid" => {
                include_bytes!("../../../tests/fixtures/just_perfect_fifth_on_c.mid")
            }
            "4_steps_in_31-et_on_c.mid" => {
                include_bytes!("../../../tests/fixtures/4_steps_in_31-et_on_c.mid")
            }
            "septimal_major_third_on_c.mid" => {
                include_bytes!("../../../tests/fixtures/septimal_major_third_on_c.mid")
            }
            "UprightPianoKW-small-20190703.sf2" => {
                include_bytes!("../../../tests/fixtures/UprightPianoKW-small-20190703.sf2")
            }
            "FluidR3Mono_GM.sf3" => include_bytes!("../../../tests/fixtures/FluidR3Mono_GM.sf3"),
            other => panic!("manifest fixture has no embedded test mapping: {other}"),
        };
        let actual = format!("{:x}", Sha256::digest(bytes));
        assert_eq!(actual, expected, "fixture checksum mismatch: {path}");
    }
}

#[test]
fn interchange_report_has_machine_checked_phase_evidence() {
    let report: serde_json::Value =
        serde_json::from_str(INTERCHANGE_REPORT).expect("interchange report is valid JSON");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["version_policy"], "workspace version is 1.2.14");
    assert!(WORKSPACE_MANIFEST.contains("version = \"1.2.14\""));
    assert_eq!(
        report["phase_7_policy"]["status"],
        "local-slices-available-external-gates-open"
    );
    assert!(
        report["phase_7_policy"]["open_gates"]
            .as_array()
            .is_some_and(|gates| gates.len() >= 4)
    );
    assert_eq!(
        report["sample_measurements"]["mscz_musescore_4_6_3"]
            .as_array()
            .expect("MSCZ sample measurements")
            .len(),
        5
    );
    assert_eq!(
        report["sample_measurements"]["midi_pitch_bend_public_domain"]
            .as_array()
            .expect("MIDI sample measurements")
            .len(),
        3
    );
    let phases = report["phases"].as_object().expect("phase map");
    for phase in ["6A", "6B", "6C", "6D", "6E", "6F", "6G"] {
        let entry = &phases[phase];
        assert!(
            entry["status"].as_str().is_some(),
            "status missing for {phase}"
        );
        assert!(
            entry["evidence"].as_array().is_some(),
            "evidence missing for {phase}"
        );
        assert_eq!(
            entry["status"].as_str(),
            Some("local-gate-passed"),
            "local release gate status missing for {phase}"
        );
        assert!(
            !entry["evidence"]
                .as_array()
                .expect("evidence array")
                .is_empty(),
            "empty evidence for {phase}"
        );
    }
    let gate_contract = report["phase_gate_contract"]
        .as_object()
        .expect("phase BUILD/MEASURE/GATE contract");
    for phase in ["6A", "6B", "6C", "6D", "6E", "6F", "6G"] {
        let contract = gate_contract[phase]
            .as_object()
            .unwrap_or_else(|| panic!("missing gate contract for {phase}"));
        for stage in ["BUILD", "MEASURE", "GATE"] {
            assert!(
                contract[stage]
                    .as_str()
                    .is_some_and(|text| !text.trim().is_empty()),
                "missing {stage} contract evidence for {phase}"
            );
        }
    }
    assert!(report["comparison_rules"]["pitch"].as_str().is_some());
    let external_gates = report["external_gates"]
        .as_array()
        .expect("external gate array");
    assert!(external_gates.len() >= 6);
    assert!(
        external_gates
            .iter()
            .all(|gate| { gate.as_str().is_some_and(|text| !text.trim().is_empty()) })
    );
    for required in [
        "held-out",
        "permissioned guitar/bass tablature",
        "engraving-application glyph metrics",
        "audio rendering equivalence",
        "complex MEI/MSCX harmony",
    ] {
        assert!(
            external_gates
                .iter()
                .any(|gate| gate.as_str().is_some_and(|text| text.contains(required))),
            "missing external gate declaration: {required}"
        );
    }
    assert_eq!(
        report["issue_18_ip_gate"]["status"].as_str(),
        Some("deferred-risk-not-low")
    );
    let local_updates = report["local_updates"]
        .as_array()
        .expect("local update evidence array");
    for required in [
        "Note.is_unpitched",
        "Note.instrument_id",
        "score-instrument and midi-unpitched",
        "ordered Note.fingerings",
        "MusicXML figured-bass figure number/alter",
        "Part.staff_groups",
        "canonical percussion resolution",
        "multiple Fingering",
        "Part ownership",
        "Staff count",
        "bracket and barLineSpan",
    ] {
        assert!(
            local_updates
                .iter()
                .any(|entry| entry.as_str().is_some_and(|text| text.contains(required))),
            "missing local evidence update: {required}"
        );
    }
}

#[cfg(all(feature = "mscz", feature = "musicxml", feature = "midi"))]
#[test]
fn sample_measurements_match_current_parser_output() {
    let report: serde_json::Value =
        serde_json::from_str(INTERCHANGE_REPORT).expect("interchange report is valid JSON");
    let mscz_rows = report["sample_measurements"]["mscz_musescore_4_6_3"]
        .as_array()
        .expect("MSCZ measurement rows");
    let mscz_samples = [
        ("openscore_omr_score_1003.mscz", OPENSCORE_OMR_MSCZ),
        ("openscore_omr_score_1033.mscz", OPENSCORE_OMR_MSCZ_SECOND),
        ("openscore_omr_score_1035.mscz", OPENSCORE_OMR_MSCZ_THIRD),
        ("openscore_omr_score_1036.mscz", OPENSCORE_OMR_MSCZ_FOURTH),
        ("openscore_omr_score_1016.mscz", OPENSCORE_OMR_MSCZ_FIFTH),
    ];
    for (fixture, bytes) in mscz_samples {
        let parsed = acorde_io::parse_mscz_with_report(bytes).expect("MSCZ parses");
        let row = mscz_rows
            .iter()
            .find(|row| row["fixture"] == fixture)
            .expect("MSCZ measurement row");
        assert_eq!(
            parsed.diagnostics.len(),
            row["diagnostics"].as_u64().unwrap() as usize
        );
        assert_eq!(
            parsed.score.parts.len(),
            row["parts"].as_u64().unwrap() as usize
        );
        let measures: usize = parsed
            .score
            .parts
            .iter()
            .flat_map(|part| part.staves.iter())
            .map(|staff| staff.measures.len())
            .sum();
        assert_eq!(measures, row["measures"].as_u64().unwrap() as usize);
    }

    let midi_rows = report["sample_measurements"]["midi_pitch_bend_public_domain"]
        .as_array()
        .expect("MIDI measurement rows");
    let midi_samples = [
        ("just_perfect_fifth_on_c.mid", JUST_PERFECT_FIFTH_MIDI, None),
        (
            "4_steps_in_31-et_on_c.mid",
            FOUR_STEPS_31ET_MIDI,
            Some(-1850),
        ),
        (
            "septimal_major_third_on_c.mid",
            SEPTIMAL_MAJOR_THIRD_MIDI,
            Some(1437),
        ),
    ];
    for (fixture, bytes, expected_bend) in midi_samples {
        let parsed = acorde_io::parse_midi_with_report(bytes).expect("MIDI parses");
        let row = midi_rows
            .iter()
            .find(|row| row["fixture"] == fixture)
            .expect("MIDI measurement row");
        assert_eq!(
            parsed.diagnostics.len(),
            row["diagnostics"].as_u64().unwrap() as usize
        );
        assert_eq!(
            parsed.score.parts.len(),
            row["parts"].as_u64().unwrap() as usize
        );
        let measures: usize = parsed
            .score
            .parts
            .iter()
            .flat_map(|part| part.staves.iter())
            .map(|staff| staff.measures.len())
            .sum();
        assert_eq!(measures, row["measures"].as_u64().unwrap() as usize);
        if let Some(expected_bend) = expected_bend {
            assert!(
                parsed
                    .score
                    .parts
                    .iter()
                    .flat_map(|part| part.midi_pitch_bends.iter())
                    .any(|bend| bend.channel == 0 && bend.value == expected_bend)
            );
        }
    }
}

#[test]
fn musicxml_simple_figured_bass_display_text_roundtrips() {
    let xml = SIMPLE_XML.replacen(
        "<note",
        "<figured-bass><figure><figure-number>6</figure-number></figure></figured-bass><note",
        1,
    );
    let report = acorde_io::parse_musicxml_with_report(&xml).expect("MusicXML report parses");
    assert!(report.diagnostics.is_empty());
    let texts = &report.score.parts[0].staves[0].measures[0].texts;
    assert_eq!(
        texts,
        &[acorde_core::StyledText {
            style: acorde_core::TextStyle::FiguredBass,
            text: "6".to_string(),
            placement: None,
            offset_x: None,
            offset_y: None,
            relative_x: None,
            relative_y: None,
        }]
    );
    let serialized = acorde_io::serialize_musicxml(&report.score).expect("MusicXML serializes");
    assert!(serialized.contains("<figured-bass>"));
    assert!(serialized.contains("<figure-number>6</figure-number>"));
    let restored = acorde_io::parse_musicxml(&serialized).expect("serialized MusicXML parses");
    assert_eq!(restored.parts[0].staves[0].measures[0].texts, *texts);
}

#[test]
fn musicxml_structured_chord_degrees_roundtrip() {
    let xml = SIMPLE_XML.replacen(
        "<note",
        "<harmony placement=\"above\"><root><root-step>C</root-step></root><kind>dominant</kind><degree><degree-value>9</degree-value><degree-alter>1</degree-alter><degree-type>add</degree-type></degree></harmony><note",
        1,
    );
    let report = acorde_io::parse_musicxml_with_report(&xml).expect("MusicXML report parses");
    assert!(report.diagnostics.is_empty());
    let chord = report.score.parts[0].staves[0].measures[0].voices[0][0]
        .chord_symbol
        .as_ref()
        .expect("degree harmony attaches to note");
    assert_eq!(chord.placement.as_deref(), Some("above"));
    assert_eq!(
        chord.degrees,
        vec![acorde_core::ChordDegree {
            value: 9,
            alter: 1,
            kind: "add".to_string(),
        }]
    );
    let serialized = acorde_io::serialize_musicxml(&report.score).expect("MusicXML serializes");
    let restored = acorde_io::parse_musicxml(&serialized).expect("serialized MusicXML parses");
    assert_eq!(
        restored.parts[0].staves[0].measures[0].voices[0][0]
            .chord_symbol
            .as_ref()
            .expect("restored harmony")
            .degrees,
        chord.degrees
    );
}

#[test]
fn musicxml_invalid_chord_degree_is_source_located() {
    let xml = SIMPLE_XML.replacen(
        "<note",
        "<harmony><root><root-step>C</root-step></root><kind>major</kind><degree><degree-value>0</degree-value><degree-type>unsupported</degree-type></degree></harmony><note",
        1,
    );
    let report = acorde_io::parse_musicxml_with_report(&xml).expect("MusicXML report parses");
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "musicxml.invalid-degree-value"
            && diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/degree"))
            && diagnostic.preserved_value.as_deref() == Some("0")
    }));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "musicxml.unsupported-degree-type"
            && diagnostic.preserved_value.as_deref() == Some("unsupported")
    }));
}

#[test]
fn musicxml_structured_figured_bass_roundtrips_without_loss() {
    let xml = SIMPLE_XML.replacen(
        "<note",
        "<figured-bass><figure><prefix>+</prefix><figure-number>6</figure-number><suffix>b</suffix></figure></figured-bass><note",
        1,
    );
    let report = acorde_io::parse_musicxml_with_report(&xml).expect("MusicXML report parses");
    assert!(report.diagnostics.is_empty());
    assert_eq!(
        report.score.parts[0].staves[0].measures[0].figured_bass,
        vec![acorde_core::FiguredBassFigure {
            number: "6".to_string(),
            alter: None,
            prefix: Some("+".to_string()),
            suffix: Some("b".to_string()),
            extender: false,
        }]
    );
    assert_eq!(
        report.score.parts[0].staves[0].measures[0].texts[0].text,
        "+6b"
    );
    let serialized =
        acorde_io::serialize_musicxml(&report.score).expect("structured figure serializes");
    let restored = acorde_io::parse_musicxml(&serialized).expect("flattened figure reparses");
    assert_eq!(restored.parts[0].staves[0].measures[0].texts[0].text, "+6b");
}

#[cfg(feature = "mei")]
#[test]
fn figured_bass_semantics_match_between_mei_and_musicxml() {
    let mei = r#"<mei><music><body><mdiv><score><section><measure n="1"><fb><f>#6+</f></fb><staff n="1"><layer n="1"><note pname="c" oct="4" dur="4"/></layer></staff></measure></section></score></mdiv></body></music></mei>"#;
    let musicxml = SIMPLE_XML.replacen(
        "</note>",
        "<figured-bass><figure><prefix>#</prefix><figure-number>6</figure-number><suffix>+</suffix></figure></figured-bass></note>",
        1,
    );
    let mei_score = acorde_io::parse_mei(mei).expect("MEI figured bass parses");
    let musicxml_score =
        acorde_io::parse_musicxml(&musicxml).expect("MusicXML figured bass parses");
    let project = |figure: &acorde_core::FiguredBassFigure| {
        let alter = figure.alter.clone().or_else(|| {
            figure.prefix.as_deref().and_then(|prefix| match prefix {
                "#" | "♯" => Some("1".to_string()),
                "b" | "♭" => Some("-1".to_string()),
                "♮" => Some("0".to_string()),
                _ => None,
            })
        });
        (figure.number.clone(), alter, figure.suffix.clone())
    };
    let mei_projection = mei_score.parts[0].staves[0].measures[0]
        .figured_bass
        .iter()
        .map(project)
        .collect::<Vec<_>>();
    let musicxml_projection = musicxml_score.parts[0].staves[0].measures[0]
        .figured_bass
        .iter()
        .map(project)
        .collect::<Vec<_>>();
    assert_eq!(mei_projection, musicxml_projection);
}

#[cfg(feature = "mei")]
#[test]
fn harmony_function_semantics_match_between_mei_and_musicxml() {
    let mei = r##"<mei><music><body><mdiv><score><section><measure n="1"><harm startid="#n1" deg="V7" func="D" type="roman">C7</harm><staff n="1"><layer n="1"><note xml:id="n1" pname="c" oct="4" dur="4"/></layer></staff></measure></section></score></mdiv></body></music></mei>"##;
    let musicxml = SIMPLE_XML.replacen(
        "<note",
        "<harmony><root><root-step>C</root-step></root><kind>dominant</kind><function>D</function></harmony><note",
        1,
    );
    let mei_chord = acorde_io::parse_mei(mei)
        .expect("MEI harmony function parses")
        .parts[0]
        .staves[0]
        .measures[0]
        .voices[0][0]
        .chord_symbol
        .clone()
        .expect("MEI harmony attaches");
    let musicxml_chord = acorde_io::parse_musicxml(&musicxml)
        .expect("MusicXML harmony function parses")
        .parts[0]
        .staves[0]
        .measures[0]
        .voices[0][0]
        .chord_symbol
        .clone()
        .expect("MusicXML harmony attaches");
    let project = |chord: &acorde_core::ChordSymbol| {
        (
            chord.root.clone(),
            chord.kind.clone(),
            chord.harmony_function.clone(),
        )
    };
    assert_eq!(project(&mei_chord), project(&musicxml_chord));
}

#[test]
fn musicxml_figured_bass_alter_flattens_to_visible_accidental() {
    let xml = SIMPLE_XML.replacen(
        "<note",
        "<figured-bass><figure><figure-number>6</figure-number><figure-alter>-1</figure-alter></figure></figured-bass><note",
        1,
    );
    let report = acorde_io::parse_musicxml_with_report(&xml).expect("MusicXML report parses");
    assert!(report.diagnostics.is_empty());
    assert_eq!(
        report.score.parts[0].staves[0].measures[0].figured_bass[0].alter,
        Some("-1".to_string())
    );
    assert_eq!(
        report.score.parts[0].staves[0].measures[0].texts[0].text,
        "b6"
    );
    let serialized = acorde_io::serialize_musicxml(&report.score).expect("alter serializes");
    let restored = acorde_io::parse_musicxml(&serialized).expect("alter reparses");
    assert_eq!(restored.parts[0].staves[0].measures[0].texts[0].text, "b6");
}

#[test]
fn musicxml_empty_figured_bass_is_diagnosed() {
    let xml = SIMPLE_XML.replacen("<note", "<figured-bass/><note", 1);
    let report = acorde_io::parse_musicxml_with_report(&xml).expect("MusicXML report parses");
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "musicxml.unsupported-detail.figured-bass"
            && diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/figured-bass"))
    }));
}

#[test]
fn musicxml_unpitched_note_keeps_its_display_and_plays_its_kit_key() {
    let xml = SIMPLE_XML
        .replacen(
            "<part-name>Piano</part-name>",
            "<part-name>Piano</part-name><score-instrument id=\"P1-I2\"><instrument-name>Snare Drum</instrument-name><midi-unpitched>38</midi-unpitched></score-instrument>",
            1,
        )
        .replacen(
        "<pitch><step>C</step><octave>4</octave></pitch>",
        "<unpitched><display-step>C</display-step><display-octave>5</display-octave></unpitched><instrument id=\"P1-I2\"/>",
        1,
        );
    let report = acorde_io::parse_musicxml_with_report(&xml).expect("MusicXML report parses");
    assert!(!report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "musicxml.unsupported-element.unpitched"
            || diagnostic.code == "musicxml.unpitched-without-instrument"
    }));
    assert_eq!(
        report.score.parts[0].staves[0].measures[0].voices[0][0].pitches[0].to_midi(),
        72,
        "the unpitched display position is kept"
    );
    // MusicXML's 1-based <midi-unpitched>38 is GM key 37 (side stick), and playback sounds it.
    let events =
        acorde_core::to_playback_events(&report.score, &acorde_core::PlaybackOptions::default());
    assert_eq!(events[0].pitch_midi, 37);
    assert!(report.score.parts[0].staves[0].measures[0].voices[0][0].is_unpitched);
    assert_eq!(
        report.score.parts[0].staves[0].measures[0].voices[0][0]
            .instrument_id
            .as_deref(),
        Some("P1-I2")
    );
    assert_eq!(
        report.score.parts[0].percussion_instruments[0].midi_unpitched,
        Some(37)
    );
    let serialized = acorde_io::serialize_musicxml(&report.score).expect("serialize unpitched");
    assert!(serialized.contains("<unpitched>"));
    assert!(serialized.contains("<instrument id=\"P1-I2\"/>"));
    assert!(serialized.contains("<midi-unpitched>38</midi-unpitched>"));
    let reparsed = acorde_io::parse_musicxml(&serialized).expect("reparse unpitched");
    let reparsed_note = &reparsed.parts[0].staves[0].measures[0].voices[0][0];
    assert!(reparsed_note.is_unpitched);
    assert_eq!(reparsed_note.instrument_id.as_deref(), Some("P1-I2"));
    assert_eq!(reparsed_note.pitches[0].to_midi(), 72);
}

#[cfg(feature = "mei")]
#[test]
fn mei_import_report_does_not_silently_drop_pedal() {
    let mei = r##"<mei><music><body><mdiv><score><section><measure n="1"><pedal dir="down" startid="#n1" endid="#n1" tstamp="1" tstamp2="1m+3"/><staff n="1"><layer n="1"><note xml:id="n1" pname="c" oct="4" dur="4"/></layer></staff></measure></section></score></mdiv></body></music></mei>"##;
    let report = acorde_io::parse_mei_with_report(mei).expect("MEI parses");
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "mei.unsupported-detail.pedal"
            && diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/pedal"))
            && diagnostic.preserved_value.as_deref().is_some_and(|value| {
                value.contains("startid=#n1")
                    && value.contains("endid=#n1")
                    && value.contains("tstamp=1")
                    && value.contains("tstamp2=1m+3")
            })
    }));
}

#[cfg(feature = "mei")]
#[test]
fn mei_octave_span_roundtrips_through_canonical_model() {
    let mei = r##"<mei><music><body><mdiv><score><section><measure n="1"><octave startid="#n1" endid="#n2" dis="8" dis.place="above"/><staff n="1"><layer n="1"><note xml:id="n1" pname="c" oct="4" dur="4"/><note xml:id="n2" pname="d" oct="4" dur="4"/></layer></staff></measure></section></score></mdiv></body></music></mei>"##;
    let report = acorde_io::parse_mei_with_report(mei).expect("MEI octave parses");
    assert!(report.diagnostics.is_empty());
    let voice = &report.score.parts[0].staves[0].measures[0].voices[0];
    assert_eq!(voice[0].ottava_start, Some(acorde_core::OttavaKind::Va8));
    assert!(voice[1].ottava_end);
    let serialized = acorde_io::serialize_mei(&report.score).expect("MEI octave serializes");
    let restored = acorde_io::parse_mei_with_report(&serialized).expect("serialized MEI parses");
    let restored_voice = &restored.score.parts[0].staves[0].measures[0].voices[0];
    assert_eq!(
        restored_voice[0].ottava_start,
        Some(acorde_core::OttavaKind::Va8)
    );
    assert!(restored_voice[1].ottava_end);
}

#[cfg(feature = "mei")]
#[test]
fn mei_pedal_span_roundtrips_through_canonical_model() {
    let mei = r##"<mei><music><body><mdiv><score><section><measure n="1"><pedal dir="down" startid="#n1" endid="#n2"/><staff n="1"><layer n="1"><note xml:id="n1" pname="c" oct="4" dur="4"/><note xml:id="n2" pname="d" oct="4" dur="4"/></layer></staff></measure></section></score></mdiv></body></music></mei>"##;
    let report = acorde_io::parse_mei_with_report(mei).expect("MEI pedal parses");
    assert!(report.diagnostics.is_empty());
    let voice = &report.score.parts[0].staves[0].measures[0].voices[0];
    assert!(voice[0].pedal_start);
    assert!(voice[1].pedal_end);
    let serialized = acorde_io::serialize_mei(&report.score).expect("MEI pedal serializes");
    let restored = acorde_io::parse_mei_with_report(&serialized).expect("serialized MEI parses");
    let restored_voice = &restored.score.parts[0].staves[0].measures[0].voices[0];
    assert!(restored_voice[0].pedal_start);
    assert!(restored_voice[1].pedal_end);
}

#[cfg(feature = "mei")]
#[test]
fn mei_simple_figured_bass_display_text_roundtrips() {
    let mei = r#"<mei><music><body><mdiv><score><section><measure n="1"><fb><f>6</f></fb><staff n="1"><layer n="1"><note pname="c" oct="4" dur="1"/></layer></staff></measure></section></score></mdiv></body></music></mei>"#;
    let report = acorde_io::parse_mei_with_report(mei).expect("MEI figured bass parses");
    assert!(report.diagnostics.is_empty());
    let texts = &report.score.parts[0].staves[0].measures[0].texts;
    assert_eq!(
        texts,
        &[acorde_core::StyledText {
            style: acorde_core::TextStyle::FiguredBass,
            text: "6".to_string(),
            placement: None,
            offset_x: None,
            offset_y: None,
            relative_x: None,
            relative_y: None,
        }]
    );
    let serialized = acorde_io::serialize_mei(&report.score).expect("MEI serializes");
    assert!(serialized.contains("<fb><f>6</f></fb>"));
    let restored = acorde_io::parse_mei(&serialized).expect("serialized MEI reparses");
    assert_eq!(restored.parts[0].staves[0].measures[0].texts, *texts);
}

#[cfg(feature = "mei")]
#[test]
fn mei_harmonic_analysis_fixture_roundtrips_structured_fields() {
    let report = acorde_io::parse_mei_with_report(INTERCHANGE_HARM_ANALYSIS_MEI)
        .expect("MEI harmonic-analysis fixture parses");
    assert!(report.diagnostics.is_empty());
    let note = &report.score.parts[0].staves[0].measures[0].voices[0][0];
    let chord = note.chord_symbol.as_ref().expect("attached harmony");
    assert_eq!(chord.root, "C");
    assert_eq!(chord.kind, "dominant");
    assert!(chord.extender);
    assert_eq!(chord.harmonic_degree.as_deref(), Some("V7"));
    assert_eq!(chord.harmony_type.as_deref(), Some("roman"));
    assert_eq!(chord.chord_ref.as_deref(), Some("#harmonychordA"));
    assert_eq!(report.score.chord_definitions.len(), 1);
    assert_eq!(
        report.score.chord_definitions[0].id.as_deref(),
        Some("harmonychordA")
    );
    assert_eq!(report.score.chord_definitions[0].members.len(), 2);
    assert_eq!(
        report.score.chord_definitions[0].members[0].id.as_deref(),
        Some("member1")
    );
    assert_eq!(
        report.score.chord_definitions[0].members[0].tab_fret,
        Some(3)
    );
    assert_eq!(
        report.score.chord_definitions[0].members[1]
            .pitch
            .as_ref()
            .map(|pitch| pitch.microtone_cents),
        Some(25)
    );
    assert_eq!(
        report.score.chord_definitions[0].members[1].tab_course,
        Some(2)
    );
    assert_eq!(report.score.chord_definitions[0].barres.len(), 1);
    assert_eq!(report.score.chord_definitions[0].barres[0].fret, Some(3));
    let serialized = acorde_io::serialize_mei(&report.score).expect("MEI fixture serializes");
    let restored = acorde_io::parse_mei_with_report(&serialized).expect("serialized MEI parses");
    assert_eq!(
        restored.score.parts[0].staves[0].measures[0].voices[0][0].chord_symbol,
        note.chord_symbol
    );
    assert_eq!(
        restored.score.chord_definitions,
        report.score.chord_definitions
    );
}

#[cfg(feature = "mei")]
#[test]
fn mei_import_report_marks_unrepresentable_octave_attributes() {
    let mei = r#"<mei><music><body><mdiv><score><section><measure n="1"><octave tstamp="1" dis="8" dis.place="above"/><staff n="1"><layer n="1"><note pname="c" oct="4" dur="4"/></layer></staff></measure></section></score></mdiv></body></music></mei>"#;
    let report = acorde_io::parse_mei_with_report(mei).expect("MEI parses");
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "mei.unsupported-detail.octave"
            && diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/octave"))
    }));
}

#[test]
fn musicxml_import_report_preserves_bend_amount_and_reports_other_technique_details() {
    let xml = SIMPLE_XML.replacen(
        "</note>",
        "<notations><technical><bend><bend-alter>2</bend-alter></bend><slide type=\"stop\"/></technical></notations></note>",
        1,
    );
    let report = acorde_io::parse_musicxml_with_report(&xml).expect("MusicXML parses");
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "musicxml.unsupported-detail.bend-alter")
    );
    assert_eq!(
        report.score.parts[0].staves[0].measures[0].voices[0][0].guitar_bend_alter_cents,
        Some(200)
    );
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "musicxml.unsupported-detail.slide-type"
            && diagnostic.preserved_value.as_deref() == Some("stop")
            && diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/slide"))
    }));
}

#[test]
fn musicxml_invalid_tablature_values_are_source_located() {
    let xml = SIMPLE_XML.replacen(
        "</note>",
        "<notations><technical><string>0</string><fret>not-a-fret</fret></technical></notations></note>",
        1,
    );
    let report = acorde_io::parse_musicxml_with_report(&xml).expect("MusicXML parses");
    let diagnostics = report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "musicxml.invalid-tablature-position")
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.source_location.as_deref().is_some_and(|path| {
            path.ends_with("/technical/string") || path.ends_with("/technical/fret")
        }) && diagnostic.preserved_value.is_some()
    }));
}

#[cfg(all(feature = "mei", feature = "mscz"))]
#[test]
fn manifest_interchange_fixtures_parse_without_declared_losses() {
    let mei = acorde_io::parse_mei_with_report(INTERCHANGE_MEI).expect("MEI fixture parses");
    assert!(mei.diagnostics.is_empty());
    assert_eq!(mei.score.parts[0].staves[0].measures[0].voices[1].len(), 1);
    let mei_measure = &mei.score.parts[0].staves[0].measures[0];
    assert_eq!(mei_measure.rehearsal.as_deref(), Some("A"));
    assert_eq!(mei_measure.expression_text, None);
    assert_eq!(mei_measure.navigation.as_deref(), Some("DaCapoAlFine"));
    assert_eq!(
        mei_measure.texts,
        vec![acorde_core::StyledText {
            style: acorde_core::TextStyle::ChordSymbol,
            text: "Cmaj7".to_string(),
            placement: None,
            offset_x: None,
            offset_y: None,
            relative_x: None,
            relative_y: None,
        }]
    );
    let mei_serialized = acorde_io::serialize_mei(&mei.score).expect("MEI fixture serializes");
    assert!(mei_serialized.contains("<harm>Cmaj7</harm>"));
    assert!(mei_serialized.contains("<dir>D.C. al Fine</dir>"));
    let mei_restored = acorde_io::parse_mei(&mei_serialized).expect("serialized MEI reparses");
    let restored_measure = &mei_restored.parts[0].staves[0].measures[0];
    assert_eq!(restored_measure.texts, mei_measure.texts);
    assert_eq!(restored_measure.navigation, mei_measure.navigation);
    let mscx = acorde_io::parse_mscx_with_report(INTERCHANGE_MSCX).expect("MSCX fixture parses");
    assert!(mscx.diagnostics.is_empty());
    assert_eq!(
        mscx.score.parts[0].staves[0]
            .tablature
            .as_ref()
            .map(|tab| tab.lines),
        Some(6)
    );
    let mscx_measure = &mscx.score.parts[0].staves[0].measures[0];
    assert_eq!(
        mscx_measure.texts,
        vec![
            acorde_core::StyledText {
                style: acorde_core::TextStyle::ChordSymbol,
                text: "Cmaj7".to_string(),
                placement: None,
                offset_x: None,
                offset_y: None,
                relative_x: None,
                relative_y: None,
            },
            acorde_core::StyledText {
                style: acorde_core::TextStyle::Expression,
                text: "dolce".to_string(),
                placement: None,
                offset_x: None,
                offset_y: None,
                relative_x: None,
                relative_y: None,
            },
        ]
    );
    assert_eq!(
        mscx_measure.voices[0][0].guitar_technique,
        Some(acorde_core::GuitarTechnique::Bend)
    );
}

#[cfg(feature = "mscz")]
#[test]
fn mscx_figured_bass_fixture_roundtrips_structured_order() {
    let report = acorde_io::parse_mscx_with_report(INTERCHANGE_FIGURED_BASS_MSCX)
        .expect("MSCX figured-bass fixture parses");
    assert!(report.diagnostics.is_empty());
    let measure = &report.score.parts[0].staves[0].measures[0];
    assert_eq!(
        measure
            .figured_bass
            .iter()
            .map(|figure| figure.number.as_str())
            .collect::<Vec<_>>(),
        vec!["6", "4"]
    );
    assert_eq!(
        measure.texts,
        vec![acorde_core::StyledText {
            style: acorde_core::TextStyle::FiguredBass,
            text: "6 4".to_string(),
            placement: None,
            offset_x: None,
            offset_y: None,
            relative_x: None,
            relative_y: None,
        }]
    );
}

#[test]
fn musicxml_chord_symbol_placement_roundtrips() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>T</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>480</divisions><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><harmony placement="below"><root><root-step>C</root-step></root><kind>dominant</kind><function>V7</function><bass><bass-step>G</bass-step></bass></harmony><note><pitch><step>C</step><octave>4</octave></pitch><duration>1920</duration><voice>1</voice><type>whole</type></note></measure></part></score-partwise>"#;
    let score = acorde_io::parse_musicxml(xml).expect("MusicXML harmony parses");
    assert_eq!(
        score.parts[0].staves[0].measures[0].voices[0][0]
            .chord_symbol
            .as_ref()
            .and_then(|chord| chord.placement.as_deref()),
        Some("below")
    );
    assert_eq!(
        score.parts[0].staves[0].measures[0].voices[0][0]
            .chord_symbol
            .as_ref()
            .and_then(|chord| chord.harmony_function.as_deref()),
        Some("V7")
    );
    let serialized = acorde_io::serialize_musicxml(&score).expect("MusicXML harmony serializes");
    assert!(serialized.contains("<harmony placement=\"below\">"));
    assert!(serialized.contains("<function>V7</function>"));
    let restored = acorde_io::parse_musicxml(&serialized).expect("serialized harmony reparses");
    assert_eq!(
        restored.parts[0].staves[0].measures[0].voices[0][0]
            .chord_symbol
            .as_ref()
            .and_then(|chord| chord.placement.as_deref()),
        Some("below")
    );
    assert_eq!(
        restored.parts[0].staves[0].measures[0].voices[0][0]
            .chord_symbol
            .as_ref()
            .and_then(|chord| chord.harmony_function.as_deref()),
        Some("V7")
    );
}

#[cfg(feature = "mscz")]
#[test]
fn openscore_lieder_cc0_fixture_parses_as_external_smoke_corpus() {
    let report = acorde_io::parse_mscx_with_report(OPENSCORE_LIEDER_MSCX)
        .expect("OpenScore Lieder MSCX fixture parses");
    // OpenScore keeps playback hairpins, pedal lines, and most dynamics hidden; they import as
    // visible marks and each hidden flag is reported rather than silently dropped.
    assert_eq!(report.diagnostics.len(), 61);
    assert!(
        report
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "mscx.unsupported-visibility")
    );
    assert!(acorde_core::validate(&report.score).is_valid());
    let notes: Vec<&acorde_core::Note> = report
        .score
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .flat_map(|staff| &staff.measures)
        .flat_map(|measure| measure.voices.iter().flatten())
        .collect();
    let count =
        |test: fn(&acorde_core::Note) -> bool| notes.iter().filter(|note| test(note)).count();
    assert_eq!(count(|note| note.hairpin_start.is_some()), 16);
    assert_eq!(count(|note| note.hairpin_end), 16);
    assert_eq!(count(|note| note.pedal_start), 27);
    assert_eq!(count(|note| note.pedal_end), 27);
    assert_eq!(count(|note| note.slur_start), 5);
    assert_eq!(count(|note| note.slur_end), 5);
    let first_staff = &report.score.parts[0].staves[0].measures;
    assert_eq!(first_staff.iter().filter(|m| m.system_break).count(), 4);
    assert_eq!(first_staff.iter().filter(|m| m.page_break).count(), 1);
    // One `<BarLine><subtype>double</subtype>` per staff that carries it in the source.
    assert_eq!(
        report
            .score
            .parts
            .iter()
            .flat_map(|part| &part.staves)
            .flat_map(|staff| &staff.measures)
            .filter(|m| matches!(m.barline_right, acorde_core::Barline::Double))
            .count(),
        5
    );
    assert_eq!(report.score.parts.len(), 4);
    assert_eq!(report.score.parts[0].staves[0].measures.len(), 23);
    assert_eq!(report.score.metadata.title, "Aloha Oe");
    assert!(report.score.parts.iter().any(|part| {
        part.staves.iter().any(|staff| {
            staff.measures.iter().any(|measure| {
                measure
                    .voices
                    .iter()
                    .any(|voice| voice.iter().any(|note| note.tie_end))
            })
        })
    }));
}

#[cfg(feature = "mei")]
#[test]
fn multistaff_mei_fixture_preserves_staff_clef_and_layers() {
    let report = acorde_io::parse_mei_with_report(INTERCHANGE_MULTISTAFF_MEI)
        .expect("multi-staff MEI fixture parses");
    assert!(report.diagnostics.is_empty());
    assert_eq!(report.score.parts[0].staves.len(), 2);
    assert_eq!(
        report.score.parts[0].staves[0].clef,
        acorde_core::Clef::Treble
    );
    assert_eq!(
        report.score.parts[0].staves[1].clef,
        acorde_core::Clef::Bass
    );
    assert_eq!(
        report.score.parts[0].staves[0].measures[0].voices[1].len(),
        1
    );
    assert_eq!(
        report.score.parts[0].staves[1].measures[0].voices[0][0].pitches[0].to_midi_cents(),
        4800
    );
    let serialized = acorde_io::serialize_mei(&report.score).expect("multi-staff MEI serializes");
    let restored = acorde_io::parse_mei(&serialized).expect("serialized multi-staff MEI parses");
    assert_eq!(restored.parts[0].staves.len(), 2);
    assert_eq!(restored.parts[0].staves[1].clef, acorde_core::Clef::Bass);
}

#[cfg(all(feature = "abc", feature = "mei", feature = "mscz"))]
#[test]
fn microtone_semantics_match_across_declared_format_boundaries() {
    let abc = "X:1\nT:Quarter\nM:4/4\nL:1/4\nK:C\n^/C|\n";
    let mei = r#"<mei><music><body><mdiv><score><scoreDef><staffGrp><staffDef n="1"/></staffGrp></scoreDef><section><measure n="1"><staff n="1"><layer n="1"><note pname="c" oct="4" dur="4" accid="qs"/></layer></staff></measure></section></score></mdiv></body></music></mei>"#;
    let mscx = r#"<museScore><Score><Part><Staff id="1"/></Part><Staff id="1"><Measure><Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc><Accidental><subtype>quarter-sharp</subtype></Accidental></Note></Chord></Measure></Staff></Score></museScore>"#;

    let abc_pitch = acorde_io::parse_abc(abc).unwrap().parts[0].staves[0].measures[0].voices[0][0]
        .pitches[0]
        .to_midi_cents();
    let mei_pitch = acorde_io::parse_mei(mei).unwrap().parts[0].staves[0].measures[0].voices[0][0]
        .pitches[0]
        .to_midi_cents();
    let mscx_pitch =
        acorde_io::parse_mscx(mscx).unwrap().parts[0].staves[0].measures[0].voices[0][0].pitches[0]
            .to_midi_cents();
    assert_eq!([abc_pitch, mei_pitch, mscx_pitch], [6050, 6050, 6050]);
}

#[test]
fn musicxml_fractional_alteration_preserves_exact_model_cents() {
    let xml = r#"<?xml version="1.0"?><score-partwise version="3.1"><part-list><score-part id="P1"><part-name>T</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>1</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>C</step><alter>0.25</alter><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure></part></score-partwise>"#;
    let score = parse_musicxml(xml).expect("fractional MusicXML parses");
    let pitch = &score.parts[0].staves[0].measures[0].voices[0][0].pitches[0];
    assert_eq!(pitch.to_midi_cents(), 6025);
    let serialized = serialize_musicxml(&score).expect("fractional MusicXML serializes");
    let restored = parse_musicxml(&serialized).expect("serialized fractional MusicXML parses");
    assert_eq!(
        restored.parts[0].staves[0].measures[0].voices[0][0].pitches[0].to_midi_cents(),
        6025
    );
}

#[test]
fn musicxml_negative_and_compound_fractional_alterations_preserve_cents() {
    let xml = r#"<?xml version="1.0"?><score-partwise version="3.1"><part-list><score-part id="P1"><part-name>T</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>C</step><alter>-0.25</alter><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note><note><pitch><step>C</step><alter>1.25</alter><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure></part></score-partwise>"#;
    let score = parse_musicxml(xml).expect("boundary MusicXML parses");
    let voice = &score.parts[0].staves[0].measures[0].voices[0];
    assert_eq!(
        voice
            .iter()
            .map(|note| note.pitches[0].to_midi_cents())
            .collect::<Vec<_>>(),
        vec![5975, 6125]
    );
    let serialized = serialize_musicxml(&score).expect("boundary MusicXML serializes");
    let restored = parse_musicxml(&serialized).expect("serialized boundary MusicXML parses");
    assert_eq!(
        restored.parts[0].staves[0].measures[0].voices[0]
            .iter()
            .map(|note| note.pitches[0].to_midi_cents())
            .collect::<Vec<_>>(),
        vec![5975, 6125]
    );
}

#[cfg(all(feature = "abc", feature = "mei", feature = "mscz"))]
#[test]
fn semantic_projection_matches_across_local_format_boundaries() {
    fn projection(score: &acorde_core::Score) -> Vec<(i32, acorde_core::Duration, bool, u8)> {
        score.parts[0].staves[0].measures[0].voices[0]
            .iter()
            .map(|note| {
                (
                    note.pitches
                        .first()
                        .map_or(0, acorde_core::Pitch::to_midi_cents),
                    note.duration.clone(),
                    note.is_rest,
                    note.dot_count,
                )
            })
            .collect()
    }
    let abc = acorde_io::parse_abc("X:1\nT:T\nM:2/4\nL:1/4\nK:C\n^/C D|\n").unwrap();
    let mei = acorde_io::parse_mei(
        r#"<mei><music><body><mdiv><score><section><measure n="1"><staff n="1"><layer n="1"><note pname="c" oct="4" dur="4" accid="qs"/><note pname="d" oct="4" dur="4"/></layer></staff></measure></section></score></mdiv></body></music></mei>"#,
    )
    .unwrap();
    let mscx = acorde_io::parse_mscx(
        r#"<museScore><Score><Part><Staff id="1"/></Part><Staff id="1"><Measure><Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc><Accidental><subtype>quarter-sharp</subtype></Accidental></Note></Chord><Chord><durationType>quarter</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord></Measure></Staff></Score></museScore>"#,
    )
    .unwrap();
    let expected = vec![
        (6050, acorde_core::Duration::Quarter, false, 0),
        (6200, acorde_core::Duration::Quarter, false, 0),
    ];
    assert_eq!(projection(&abc), expected);
    assert_eq!(projection(&mei), expected);
    assert_eq!(projection(&mscx), expected);
}

#[cfg(all(feature = "mei", feature = "mscz"))]
#[test]
fn tuplet_semantics_match_across_musicxml_mei_and_mscx() {
    type TupletProjection = (i32, acorde_core::Duration, Option<(u8, u8)>);

    fn projection(score: &acorde_core::Score) -> Vec<TupletProjection> {
        score.parts[0].staves[0].measures[0].voices[0]
            .iter()
            .filter(|note| !note.is_rest)
            .map(|note| {
                (
                    note.pitches[0].to_midi_cents(),
                    note.duration.clone(),
                    note.tuplet
                        .as_ref()
                        .map(|tuplet| (tuplet.actual_notes, tuplet.normal_notes)),
                )
            })
            .collect()
    }
    let musicxml = acorde_io::parse_musicxml(
        r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>T</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>480</divisions><time><beats>2</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>320</duration><voice>1</voice><type>eighth</type><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification></note></measure></part></score-partwise>"#,
    )
    .unwrap();
    let mei = acorde_io::parse_mei(
        r#"<mei><music><body><mdiv><score><section><measure n="1"><staff n="1"><layer n="1"><tuplet num="3" numbase="2"><note pname="c" oct="4" dur="8"/></tuplet></layer></staff></measure></section></score></mdiv></body></music></mei>"#,
    )
    .unwrap();
    let mscx = acorde_io::parse_mscx(
        r#"<museScore><Score><Part><Staff id="1"/></Part><Staff id="1"><Measure><Tuplet><normalNotes>2</normalNotes><actualNotes>3</actualNotes><baseNote>eighth</baseNote></Tuplet><Chord><durationType>eighth</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord><endTuplet/></Measure></Staff></Score></museScore>"#,
    )
    .unwrap();
    let expected = vec![(6000, acorde_core::Duration::Eighth, Some((3, 2)))];
    assert_eq!(projection(&musicxml), expected);
    assert_eq!(projection(&mei), expected);
    assert_eq!(projection(&mscx), expected);
}

#[cfg(all(feature = "mei", feature = "mscz"))]
#[test]
fn chord_label_semantics_match_across_mei_and_mscx() {
    fn labels(score: &acorde_core::Score) -> Vec<(acorde_core::TextStyle, String)> {
        let measure = &score.parts[0].staves[0].measures[0];
        let mut labels = measure
            .texts
            .iter()
            .map(|text| (text.style, text.text.clone()))
            .collect::<Vec<_>>();
        if let Some(expression) = measure.expression_text.as_deref() {
            labels.push((acorde_core::TextStyle::Expression, expression.to_string()));
        }
        labels
    }
    let mei = acorde_io::parse_mei(
        r#"<mei><music><body><mdiv><score><section><measure n="1"><harm>Cmaj7</harm><dir>dolce</dir><staff n="1"><layer n="1"><rest dur="1"/></layer></staff></measure></section></score></mdiv></body></music></mei>"#,
    )
    .unwrap();
    let mscx = acorde_io::parse_mscx(
        r#"<museScore><Score><Part><Staff id="1"/></Part><Staff id="1"><Measure><Harmony><name>Cmaj7</name></Harmony><Text><style>Expression</style><text>dolce</text></Text><Rest><durationType>whole</durationType></Rest></Measure></Staff></Score></museScore>"#,
    )
    .unwrap();
    assert_eq!(labels(&mei), labels(&mscx));
}

#[cfg(all(feature = "mei", feature = "mscz"))]
#[test]
fn compact_chord_degree_semantics_match_across_mei_and_mscx() {
    fn degree_projection(score: &acorde_core::Score) -> Vec<acorde_core::ChordDegree> {
        score.parts[0].staves[0].measures[0].voices[0]
            .iter()
            .find_map(|note| {
                note.chord_symbol
                    .as_ref()
                    .map(|chord| chord.degrees.clone())
            })
            .unwrap_or_default()
    }
    let mei = acorde_io::parse_mei(
        r##"<mei><music><body><mdiv><score><section><measure n="1"><harm startid="#n1" place="above">C7add#9b5no3/E</harm><staff n="1"><layer n="1"><note xml:id="n1" pname="c" oct="4" dur="1"/></layer></staff></measure></section></score></mdiv></body></music></mei>"##,
    )
    .unwrap();
    let mscx = acorde_io::parse_mscx(
        r#"<museScore><Score><Part><Staff id="1"/></Part><Staff id="1"><Measure><Harmony><harmonyInfo><name>7add#9b5no3</name><root>14</root><base>18</base><placement>above</placement></harmonyInfo></Harmony><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord></Measure></Staff></Score></museScore>"#,
    )
    .unwrap();
    let musicxml = acorde_io::parse_musicxml(
        &SIMPLE_XML
            .replacen(
                "<note",
                "<harmony placement=\"above\"><root><root-step>C</root-step></root><kind>dominant</kind><degree><degree-value>9</degree-value><degree-alter>1</degree-alter><degree-type>add</degree-type></degree><degree><degree-value>5</degree-value><degree-alter>-1</degree-alter><degree-type>alter</degree-type></degree><degree><degree-value>3</degree-value><degree-type>subtract</degree-type></degree></harmony><note",
                1,
            ),
    )
    .unwrap();
    assert_eq!(degree_projection(&musicxml), degree_projection(&mei));
    assert_eq!(degree_projection(&mei), degree_projection(&mscx));
}

#[test]
fn multivoice_musicxml_preserves_voice_structure_and_playback_addresses() {
    let score1 = parse_musicxml(MULTIVOICE_XML).expect("multi-voice parse failed");
    let measure = &score1.parts[0].staves[0].measures[0];
    assert_eq!(measure.voices[0].len(), 4);
    assert_eq!(measure.voices[1].len(), 2);
    assert!(measure.voices[2].is_empty());
    assert_eq!(measure.voices[0][0].pitches[0].step, Step::C);
    assert_eq!(measure.voices[1][0].pitches[0].step, Step::C);

    let xml2 = serialize_musicxml(&score1).expect("multi-voice serialize failed");
    assert!(xml2.contains("<backup>"));
    let score2 = parse_musicxml(&xml2).expect("multi-voice reparse failed");
    let measure2 = &score2.parts[0].staves[0].measures[0];
    for voice_index in 0..4 {
        let left = &measure.voices[voice_index];
        let right = &measure2.voices[voice_index];
        assert_eq!(
            left.len(),
            right.len(),
            "voice {voice_index} length mismatch"
        );
        for (left_note, right_note) in left.iter().zip(right) {
            assert_eq!(left_note.is_rest, right_note.is_rest);
            assert_eq!(left_note.duration, right_note.duration);
            assert_eq!(left_note.dot_count, right_note.dot_count);
            if !left_note.is_rest {
                assert_eq!(left_note.pitches, right_note.pitches);
            }
        }
    }

    let events = acorde_core::to_playback_events(
        &score2,
        &acorde_core::PlaybackOptions {
            metronome: None,
            ..Default::default()
        },
    );
    assert!(
        events
            .iter()
            .any(|event| event.address.as_deref() == Some("0:0:0:0:0"))
    );
    assert!(
        events
            .iter()
            .any(|event| event.address.as_deref() == Some("0:0:0:1:0"))
    );
}

#[test]
fn public_domain_pitch_bend_fixture_roundtrips_semantically() {
    let score1 = parse_midi(JUST_PERFECT_FIFTH_MIDI).expect("licensed MIDI fixture parses");
    assert_eq!(score1.parts.len(), 2);
    assert_eq!(score1.parts[0].midi_pitch_bends.len(), 1);
    assert_eq!(score1.parts[0].midi_pitch_bends[0].tick, 0);
    assert_eq!(score1.parts[0].midi_pitch_bends[0].channel, 0);
    assert_eq!(score1.parts[0].midi_pitch_bends[0].value, 80);
    assert_eq!(score1.parts[1].midi_pitch_bends.len(), 1);
    assert_eq!(score1.parts[1].midi_pitch_bends[0].channel, 1);
    assert_eq!(score1.parts[1].midi_pitch_bends[0].value, 0);

    let midi2 = acorde_io::serialize_midi(&score1).expect("fixture serializes");
    let score2 = parse_midi(&midi2).expect("serialized fixture reparses");
    let notes1: Vec<_> = score1
        .parts
        .iter()
        .flat_map(|part| part.staves.iter())
        .flat_map(|staff| staff.measures.iter())
        .flat_map(|measure| measure.voices[0].iter())
        .map(|note| {
            (
                note.is_rest,
                note.duration.clone(),
                note.pitches
                    .iter()
                    .map(acorde_core::Pitch::to_midi_cents)
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let notes2: Vec<_> = score2
        .parts
        .iter()
        .flat_map(|part| part.staves.iter())
        .flat_map(|staff| staff.measures.iter())
        .flat_map(|measure| measure.voices[0].iter())
        .map(|note| {
            (
                note.is_rest,
                note.duration.clone(),
                note.pitches
                    .iter()
                    .map(acorde_core::Pitch::to_midi_cents)
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(
        notes1, notes2,
        "MIDI note event meanings changed during round-trip"
    );
    let bends1: Vec<_> = score1
        .parts
        .iter()
        .flat_map(|part| part.midi_pitch_bends.iter())
        .collect();
    let bends2: Vec<_> = score2
        .parts
        .iter()
        .flat_map(|part| part.midi_pitch_bends.iter())
        .collect();
    assert_eq!(
        bends1, bends2,
        "pitch-bend events changed during round-trip"
    );
}

#[test]
fn public_domain_pitch_bend_corpus_covers_signed_nonzero_values() {
    let cases = [
        (FOUR_STEPS_31ET_MIDI, -1850),
        (SEPTIMAL_MAJOR_THIRD_MIDI, 1437),
    ];
    for (bytes, expected) in cases {
        let score1 = parse_midi(bytes).expect("public-domain MIDI fixture parses");
        let bends1: Vec<_> = score1
            .parts
            .iter()
            .flat_map(|part| part.midi_pitch_bends.iter())
            .filter(|bend| bend.channel == 0)
            .collect();
        assert_eq!(bends1.len(), 1);
        assert_eq!(bends1[0].value, expected);

        let score2 = parse_midi(
            &acorde_io::serialize_midi(&score1).expect("public-domain fixture serializes"),
        )
        .expect("serialized public-domain fixture reparses");
        let bends2: Vec<_> = score2
            .parts
            .iter()
            .flat_map(|part| part.midi_pitch_bends.iter())
            .filter(|bend| bend.channel == 0)
            .collect();
        assert_eq!(bends1, bends2);
    }
}

#[cfg(feature = "mscz")]
#[test]
fn cc0_mscz_fixture_parses_with_zero_diagnostics() {
    let report =
        acorde_io::parse_mscz_with_report(OPENSCORE_OMR_MSCZ).expect("CC0 MSCZ fixture parses");
    assert!(report.diagnostics.is_empty());
    assert_eq!(report.score.parts.len(), 1);
    assert_eq!(report.score.parts[0].staves[0].measures.len(), 4);
}

#[cfg(feature = "mscz")]
#[test]
fn cc0_mscz_musescore_4_pair_parses_with_declared_diagnostics() {
    for (bytes, expected_diagnostics) in [
        (OPENSCORE_OMR_MSCZ, 0),
        (OPENSCORE_OMR_MSCZ_SECOND, 0),
        (OPENSCORE_OMR_MSCZ_THIRD, 0),
        (OPENSCORE_OMR_MSCZ_FOURTH, 0),
        (OPENSCORE_OMR_MSCZ_FIFTH, 0),
    ] {
        let report = acorde_io::parse_mscz_with_report(bytes).expect("CC0 MSCZ fixture parses");
        assert_eq!(report.diagnostics.len(), expected_diagnostics);
        assert!(
            report
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code == "mscx.unsupported-element.Harmony")
        );
        assert!(!report.score.parts.is_empty());
        assert!(
            report
                .score
                .parts
                .iter()
                .all(|part| part.staves.iter().any(|staff| !staff.measures.is_empty()))
        );
    }
    let structured = acorde_io::parse_mscz(OPENSCORE_OMR_MSCZ_SECOND)
        .expect("structured MuseScore Harmony fixture parses");
    let chord_symbol_count = structured
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .flat_map(|staff| &staff.measures)
        .flat_map(|measure| &measure.voices)
        .flat_map(|voice| voice.iter())
        .filter(|note| note.chord_symbol.is_some())
        .count();
    assert!(
        chord_symbol_count >= 10,
        "real MSCX Harmony roots should attach to canonical notes"
    );
}

#[cfg(feature = "mscz")]
#[test]
fn cc0_mscz_pair_extracted_mscx_files_parse_with_same_declared_boundary() {
    for (xml, expected_diagnostics) in [(OPENSCORE_OMR_MSCX, 0), (OPENSCORE_OMR_MSCX_SECOND, 0)] {
        let report = acorde_io::parse_mscx_with_report(xml).expect("extracted MSCX parses");
        assert_eq!(report.diagnostics.len(), expected_diagnostics);
        assert!(!report.score.parts.is_empty());
    }
}

#[cfg(all(feature = "mscz", feature = "musicxml"))]
#[test]
fn cc0_mscz_samples_roundtrip_through_musicxml_semantically() {
    type ChordProjection = Option<(
        String,
        String,
        Option<String>,
        Option<String>,
        Vec<(u8, i8, String)>,
    )>;
    type NoteProjection = (
        usize,
        usize,
        usize,
        usize,
        bool,
        u8,
        Vec<i32>,
        acorde_core::Duration,
        ChordProjection,
    );

    fn projection(score: &acorde_core::Score) -> Vec<NoteProjection> {
        let mut result = Vec::new();
        for (part_index, part) in score.parts.iter().enumerate() {
            for (staff_index, staff) in part.staves.iter().enumerate() {
                for (measure_index, measure) in staff.measures.iter().enumerate() {
                    for (voice_index, voice) in measure.voices.iter().enumerate() {
                        // MusicXML export fills an incomplete final measure with trailing rests.
                        // Ignore only that canonical completion; preserve internal rests.
                        let last_non_rest = voice.iter().rposition(|note| !note.is_rest);
                        for (note_index, note) in voice.iter().enumerate() {
                            if note.is_rest && last_non_rest.is_some_and(|last| note_index > last) {
                                continue;
                            }
                            result.push((
                                part_index,
                                staff_index,
                                measure_index,
                                voice_index,
                                note.is_rest,
                                note.dot_count,
                                note.pitches
                                    .iter()
                                    .map(acorde_core::Pitch::to_midi_cents)
                                    .collect(),
                                note.duration.clone(),
                                note.chord_symbol.as_ref().map(|chord| {
                                    (
                                        chord.root.clone(),
                                        chord.kind.clone(),
                                        chord.bass.clone(),
                                        chord.placement.clone(),
                                        chord
                                            .degrees
                                            .iter()
                                            .map(|degree| {
                                                (degree.value, degree.alter, degree.kind.clone())
                                            })
                                            .collect(),
                                    )
                                }),
                            ));
                        }
                    }
                }
            }
        }
        result
    }

    for bytes in [
        OPENSCORE_OMR_MSCZ,
        OPENSCORE_OMR_MSCZ_SECOND,
        OPENSCORE_OMR_MSCZ_THIRD,
        OPENSCORE_OMR_MSCZ_FOURTH,
        OPENSCORE_OMR_MSCZ_FIFTH,
    ] {
        let source = acorde_io::parse_mscz(bytes).expect("MSCZ parses");
        let xml = serialize_musicxml(&source).expect("MSCZ score serializes as MusicXML");
        let restored = parse_musicxml(&xml).expect("MusicXML reparses");
        assert_eq!(projection(&source), projection(&restored));
    }
}

// ── multipart.musicxml ────────────────────────────────────────────────────────

#[test]
fn multipart_musicxml_parses() {
    let score = parse_musicxml(MULTIPART_XML).expect("parse failed");
    assert_eq!(score.metadata.title, "Multi-Part Test");
    assert_eq!(score.parts.len(), 2);

    // Violin: 3/4 in D major, 3 quarter notes
    let ts = &score.settings.time_signature;
    // The time sig from the first part/measure should propagate to score settings
    // or be accessible via the first measure's time_sig field
    let violin_notes = notes_in(&score, 0, 0);
    assert_eq!(violin_notes.iter().filter(|n| !n.is_rest).count(), 3);

    // Cello: dotted half note
    let cello_notes = notes_in(&score, 1, 0);
    let cello_measure = &score.parts[1].staves[0].measures[0];
    assert_eq!(cello_measure.clef, Some(Clef::Bass));
    assert_eq!(
        cello_measure.key_sig.as_ref().map(|key| key.fifths),
        Some(2)
    );
    assert_eq!(
        cello_measure
            .time_sig
            .as_ref()
            .map(|time| (time.numerator, time.denominator)),
        Some((3, 4))
    );
    let pitched: Vec<_> = cello_notes.iter().filter(|n| !n.is_rest).collect();
    assert_eq!(pitched.len(), 1);
    assert_eq!(pitched[0].duration, Duration::Half);
    assert_eq!(pitched[0].dot_count, 1);
    assert_eq!(pitched[0].pitches[0].step, Step::D);

    let _ = ts; // used above via score
}

#[test]
fn multipart_musicxml_roundtrip() {
    let score1 = parse_musicxml(MULTIPART_XML).expect("first parse failed");
    let xml2 = serialize_musicxml(&score1).expect("serialize failed");
    let score2 = parse_musicxml(&xml2).expect("second parse failed");

    assert_eq!(score1.parts.len(), score2.parts.len());
    for (p1, p2) in score1.parts.iter().zip(score2.parts.iter()) {
        let measures1 = &p1.staves[0].measures;
        let measures2 = &p2.staves[0].measures;
        assert_eq!(measures1.len(), measures2.len());
    }
}

// ── MusicXML midi-instrument ──────────────────────────────────────────────────

#[test]
fn musicxml_midi_instrument_roundtrip() {
    let mut score = acorde_core::Score::new("Instrument Test", 120, 4, 4, 0, 2);
    score.parts[0].midi_channel = 1;
    score.parts[0].midi_program = 40; // Violin (0-based)

    let xml = serialize_musicxml(&score).expect("serialize failed");
    assert!(
        xml.contains("<midi-channel>2</midi-channel>"),
        "1-based channel not found"
    );
    assert!(
        xml.contains("<midi-program>41</midi-program>"),
        "1-based program not found"
    );

    let score2 = parse_musicxml(&xml).expect("parse failed");
    assert_eq!(
        score2.parts[0].midi_channel, 1,
        "channel should survive round-trip"
    );
    assert_eq!(
        score2.parts[0].midi_program, 40,
        "program should survive round-trip"
    );
}

// ── fuzz guard ────────────────────────────────────────────────────────────────

#[test]
fn fuzz_empty_returns_err() {
    assert!(parse_musicxml("").is_err());
}

#[test]
fn fuzz_garbage_returns_err() {
    assert!(parse_musicxml("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").is_err());
}

#[test]
fn fuzz_64_mib_garbage_returns_err() {
    let garbage = "x".repeat(64 * 1024 * 1024);
    assert!(parse_musicxml(&garbage).is_err());
}

#[test]
fn fuzz_doctype_injection_rejected() {
    let evil = r#"<?xml version="1.0"?><!DOCTYPE foo [<!ENTITY xxe SYSTEM "file:///etc/passwd">]><score-partwise/>"#;
    assert!(parse_musicxml(evil).is_err());
}

#[test]
fn fuzz_large_nesting_rejected() {
    // Build deeply nested XML
    let open: String = "<a>".repeat(70);
    let close: String = "</a>".repeat(70);
    let xml = format!("<score-partwise>{open}x{close}</score-partwise>");
    assert!(parse_musicxml(&xml).is_err());
}

// ── ABC Notation ──────────────────────────────────────────────────────────────

#[cfg(feature = "abc")]
mod abc_tests {
    use acorde_core::{Duration, Step};
    use acorde_io::parse_abc;

    static SAMPLE_ABC: &str = include_str!("../../../tests/fixtures/sample.abc");

    #[test]
    fn abc_parses_title_and_composer() {
        let score = parse_abc(SAMPLE_ABC).expect("parse failed");
        assert_eq!(score.metadata.title, "Sample Tune");
        assert_eq!(score.metadata.composer, "Test Composer");
    }

    #[test]
    fn abc_parses_two_measures() {
        let score = parse_abc(SAMPLE_ABC).expect("parse failed");
        assert_eq!(score.parts[0].staves[0].measures.len(), 2);
    }

    #[test]
    fn abc_first_measure_notes() {
        let score = parse_abc(SAMPLE_ABC).expect("parse failed");
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        let pitched: Vec<_> = notes.iter().filter(|n| !n.is_rest).collect();
        assert_eq!(pitched.len(), 4);
        assert_eq!(pitched[0].pitches[0].step, Step::C);
        assert_eq!(pitched[1].pitches[0].step, Step::D);
        assert_eq!(pitched[2].pitches[0].step, Step::E);
        assert_eq!(pitched[3].pitches[0].step, Step::F);
        for n in &pitched {
            assert_eq!(n.duration, Duration::Quarter);
        }
    }

    #[test]
    fn abc_second_measure_notes() {
        let score = parse_abc(SAMPLE_ABC).expect("parse failed");
        let notes = &score.parts[0].staves[0].measures[1].voices[0];
        let pitched: Vec<_> = notes.iter().filter(|n| !n.is_rest).collect();
        assert_eq!(pitched.len(), 4);
        assert_eq!(pitched[0].pitches[0].step, Step::G);
        assert_eq!(pitched[1].pitches[0].step, Step::A);
        assert_eq!(pitched[2].pitches[0].step, Step::B);
    }

    #[test]
    fn abc_fuzz_empty_returns_err() {
        assert!(parse_abc("").is_err());
    }

    #[test]
    fn abc_fuzz_garbage_returns_err() {
        assert!(parse_abc("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").is_err());
    }
}

// ── MusicXML per-measure tempo round-trip ────────────────────────────────────

#[test]
fn musicxml_per_measure_tempo_roundtrip() {
    use acorde_core::Score;
    let mut score = Score::new("T", 96, 4, 4, 0, 2);
    score.parts[0].staves[0].measures[1].tempo = Some(60);
    let xml = serialize_musicxml(&score).expect("serialize failed");
    let score2 = parse_musicxml(&xml).expect("parse failed");
    // Measure 1 tempo override must survive the roundtrip
    assert_eq!(score2.parts[0].staves[0].measures[1].tempo, Some(60));
    // The unmarked opening tempo travels as a bare <sound tempo>: playback keeps it, and no
    // metronome mark is invented for the first bar.
    assert_eq!(score2.parts[0].staves[0].measures[0].tempo, None);
    assert_eq!(score2.settings.tempo_bpm, 96);
}

#[test]
fn musicxml_measure0_tempo_override_no_duplicate() {
    // When measure 0 carries a tempo override, the MIDI serializer must emit
    // exactly one Tempo event at tick 0 (not two).
    use acorde_core::Score;
    let mut score = Score::new("T", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].tempo = Some(90);
    let midi = acorde_io::serialize_midi(&score).expect("midi serialize failed");
    // 90 BPM = 666_666 µs/beat = [0x0A, 0x2C, 0x2A]
    let target = [0x0Au8, 0x2C, 0x2A];
    let count = midi.windows(3).filter(|w| *w == target).count();
    assert_eq!(
        count, 1,
        "tick-0 tempo should appear exactly once, found {count}"
    );
}

// ── MusicXML Staff.transpose_semitones round-trip ────────────────────────────

#[test]
fn musicxml_transpose_semitones_roundtrip() {
    use acorde_core::Score;
    let mut score = Score::new("T", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].transpose_semitones = -2;
    let xml = serialize_musicxml(&score).expect("serialize failed");
    let score2 = parse_musicxml(&xml).expect("parse failed");
    assert_eq!(score2.parts[0].staves[0].transpose_semitones, -2);
}

#[test]
fn musicxml_transpose_zero_not_emitted() {
    // transpose_semitones == 0 → no <transpose> block in output
    use acorde_core::Score;
    let score = Score::new("T", 120, 4, 4, 0, 1);
    let xml = serialize_musicxml(&score).expect("serialize failed");
    assert!(!xml.contains("<transpose>"));
}

// ── Slur roundtrip ────────────────────────────────────────────────────────────

#[test]
fn musicxml_slur_roundtrip() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    // 2/4 so two quarter notes fill the measure; clear default rests first.
    let mut score = Score::new("Slur Test", 120, 2, 4, 0, 1);
    let mut note_a = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
    note_a.slur_start = true;
    let mut note_b = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
    note_b.slur_end = true;
    score.parts[0].staves[0].measures[0].voices[0] = vec![note_a, note_b];

    let xml = serialize_musicxml(&score).expect("serialize failed");
    assert!(
        xml.contains("type=\"start\""),
        "slur start should be in XML"
    );
    assert!(xml.contains("type=\"stop\""), "slur stop should be in XML");

    let score2 = parse_musicxml(&xml).expect("parse failed");
    let notes = &score2.parts[0].staves[0].measures[0].voices[0];
    let start_note = notes.iter().find(|n| n.slur_start);
    let end_note = notes.iter().find(|n| n.slur_end);
    assert!(start_note.is_some(), "slur_start survives roundtrip");
    assert!(end_note.is_some(), "slur_end survives roundtrip");
}

// ── Articulation roundtrip ────────────────────────────────────────────────────

#[test]
fn musicxml_articulation_roundtrip() {
    use acorde_core::{Articulation, Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("Artic Test", 120, 2, 4, 0, 1);
    let mut note_a = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
    note_a.articulations = vec![Articulation::Staccato, Articulation::Fermata];
    let note_b = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
    score.parts[0].staves[0].measures[0].voices[0] = vec![note_a, note_b];

    let xml = serialize_musicxml(&score).expect("serialize failed");
    assert!(xml.contains("<staccato/>"), "staccato should be in XML");
    assert!(xml.contains("<fermata/>"), "fermata should be in XML");

    let score2 = parse_musicxml(&xml).expect("parse failed");
    let n0 = &score2.parts[0].staves[0].measures[0].voices[0][0];
    assert!(
        n0.articulations.contains(&Articulation::Staccato),
        "staccato survives roundtrip"
    );
    assert!(
        n0.articulations.contains(&Articulation::Fermata),
        "fermata survives roundtrip"
    );
}

// ── Technical field roundtrips ────────────────────────────────────────────────

#[test]
fn musicxml_technique_text_roundtrip() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("Tech", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
    note.technique_text = Some("pizz.".to_string());
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];
    let xml = serialize_musicxml(&score).expect("serialize failed");
    assert!(
        xml.contains("<other-technical>pizz.</other-technical>"),
        "technique_text in XML"
    );
    let score2 = parse_musicxml(&xml).expect("parse failed");
    assert_eq!(
        score2.parts[0].staves[0].measures[0].voices[0][0]
            .technique_text
            .as_deref(),
        Some("pizz.")
    );
}

#[test]
fn musicxml_fingering_roundtrip() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("Finger", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::G, 4), Duration::Whole);
    note.fingering = Some(3);
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];
    let xml = serialize_musicxml(&score).expect("serialize failed");
    assert!(xml.contains("<fingering>3</fingering>"), "fingering in XML");
    let score2 = parse_musicxml(&xml).expect("parse failed");
    assert_eq!(
        score2.parts[0].staves[0].measures[0].voices[0][0].fingering,
        Some(3)
    );
}

#[test]
fn musicxml_multiple_fingering_candidates_roundtrip() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("Fingerings", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::G, 4), Duration::Whole);
    note.fingerings = vec![1, 3, 4];
    note.fingering = note.fingerings.first().copied();
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];
    let xml = serialize_musicxml(&score).expect("serialize multiple fingerings");
    assert_eq!(xml.matches("<fingering>").count(), 3);
    let restored = parse_musicxml(&xml).expect("parse multiple fingerings");
    let note = &restored.parts[0].staves[0].measures[0].voices[0][0];
    assert_eq!(note.fingering, Some(1));
    assert_eq!(note.fingerings, vec![1, 3, 4]);
}

#[test]
fn musicxml_string_number_roundtrip() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("String", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::A, 3), Duration::Whole);
    note.string_number = Some(2);
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];
    let xml = serialize_musicxml(&score).expect("serialize failed");
    assert!(xml.contains("<string>2</string>"), "string_number in XML");
    let score2 = parse_musicxml(&xml).expect("parse failed");
    assert_eq!(
        score2.parts[0].staves[0].measures[0].voices[0][0].string_number,
        Some(2)
    );
}

#[test]
fn musicxml_tablature_tuning_and_techniques_roundtrip() {
    use acorde_core::{
        Duration, GuitarTechnique, Note, Pitch, Score, Staff, Step, TablatureConfig,
    };
    let mut score = Score::new("Tab", 120, 4, 4, 0, 1);
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 61, 55, 50, 45, 40],
        capo: 2,
    });
    staff.measures.push(acorde_core::Measure::empty(4, 4));
    let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
    note.tab_position = Some(acorde_core::TabPosition { string: 1, fret: 0 });
    note.guitar_technique = Some(GuitarTechnique::Slide);
    staff.measures[0].voices[0] = vec![note];
    score.parts[0].staves = vec![staff];

    let xml = serialize_musicxml(&score).expect("tablature serializes");
    let report = acorde_io::serialize_musicxml_with_report(&score).expect("tab export reports");
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    assert!(xml.contains("<capo>2</capo>"));
    assert!(xml.contains("<staff-tuning line=\"1\"><tuning-step>E</tuning-step>"));
    assert!(xml.contains(
        "<staff-tuning line=\"2\"><tuning-step>C</tuning-step><tuning-alter>1</tuning-alter>"
    ));
    assert!(xml.contains("<staff-tuning line=\"6\"><tuning-step>E</tuning-step>"));
    let restored = parse_musicxml(&xml).expect("tablature reparses");
    let tab = restored.parts[0].staves[0]
        .tablature
        .as_ref()
        .expect("tab staff");
    assert_eq!(tab.lines, 6);
    assert_eq!(tab.tuning_midi, vec![64, 61, 55, 50, 45, 40]);
    assert_eq!(tab.capo, 2, "MusicXML staff-details capo round-trips");
    let restored_note = &restored.parts[0].staves[0].measures[0].voices[0][0];
    assert_eq!(
        restored_note
            .tab_position
            .as_ref()
            .map(|p| (p.string, p.fret)),
        Some((1, 0))
    );
    assert_eq!(restored_note.guitar_technique, Some(GuitarTechnique::Slide));
}

#[cfg(all(feature = "midi", feature = "musicxml"))]
#[test]
fn musicxml_export_reports_midi_pitch_bend_loss_without_silent_drop() {
    for bytes in [
        JUST_PERFECT_FIFTH_MIDI,
        FOUR_STEPS_31ET_MIDI,
        SEPTIMAL_MAJOR_THIRD_MIDI,
    ] {
        let imported = acorde_io::parse_midi(bytes).expect("MIDI fixture parses");
        let bend_count: usize = imported
            .parts
            .iter()
            .map(|part| part.midi_pitch_bends.len())
            .sum();
        assert!(bend_count > 0);

        let report = acorde_io::serialize_musicxml_with_report(&imported)
            .expect("MusicXML export reports pitch-bend loss");
        let bend_diagnostics: Vec<_> = report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "musicxml.export-unsupported-midi-pitch-bend")
            .collect();
        assert_eq!(bend_diagnostics.len(), bend_count);
        assert!(bend_diagnostics.iter().all(|diagnostic| {
            diagnostic.source_location.is_some()
                && diagnostic
                    .preserved_value
                    .as_deref()
                    .is_some_and(|value| value.contains("tick=") && value.contains("value="))
                && diagnostic.loss_reason.is_some()
        }));
    }
}

#[cfg(all(feature = "midi", feature = "musicxml"))]
#[test]
fn musicxml_export_reports_unrepresentable_midi_automation() {
    use acorde_core::{MidiAftertouch, MidiControlChange, MidiProgramChange, Score};
    let mut score = Score::new("MIDI automation", 120, 4, 4, 0, 1);
    let part = &mut score.parts[0];
    part.midi_control_changes = vec![MidiControlChange {
        tick: 120,
        channel: 0,
        controller: 64,
        value: 127,
    }];
    part.midi_program_changes = vec![MidiProgramChange {
        tick: 240,
        channel: 0,
        program: 40,
    }];
    part.midi_aftertouch = vec![MidiAftertouch {
        tick: 360,
        channel: 0,
        key: Some(60),
        value: 80,
    }];

    let report = acorde_io::serialize_musicxml_with_report(&score)
        .expect("MusicXML export reports MIDI automation loss");
    for (code, path) in [
        (
            "musicxml.export-unsupported-midi-control-change",
            "/score/part/1/midi-control-change/1",
        ),
        (
            "musicxml.export-unsupported-midi-program-change",
            "/score/part/1/midi-program-change/1",
        ),
        (
            "musicxml.export-unsupported-midi-aftertouch",
            "/score/part/1/midi-aftertouch/1",
        ),
    ] {
        let diagnostic = report
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == code)
            .expect("MIDI automation diagnostic exists");
        assert_eq!(diagnostic.source_location.as_deref(), Some(path));
        assert!(diagnostic.preserved_value.is_some());
        assert!(diagnostic.loss_reason.is_some());
    }
}

#[cfg(all(feature = "abc", feature = "musicxml"))]
#[test]
fn non_mei_exports_report_harmonic_type_loss() {
    use acorde_core::{ChordDefinition, ChordSymbol, Duration, Note, NoteAddr, Pitch, Score, Step};
    let mut score = Score::new("harmonic type", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
    note.chord_symbol = Some(ChordSymbol {
        root: "C".to_string(),
        kind: "major".to_string(),
        bass: None,
        placement: None,
        extender: false,
        harmonic_degree: None,
        harmony_function: None,
        harmony_type: Some("roman".to_string()),
        chord_ref: Some("#harmonychordA".to_string()),
        range_end: Some(NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        }),
        degrees: Vec::new(),
    });
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];
    score.chord_definitions.push(ChordDefinition {
        id: Some("harmonychordA".to_string()),
        label: Some("C".to_string()),
        kind: Some("guitar".to_string()),
        fret_position: Some(3),
        tab_strings: None,
        tab_courses: None,
        members: Vec::new(),
        barres: Vec::new(),
    });

    let musicxml = acorde_io::serialize_musicxml_with_report(&score).unwrap();
    let musicxml_loss = musicxml
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "musicxml.export-unsupported-mei-harmony-type")
        .expect("MusicXML harmonic type loss is diagnosed");
    assert_eq!(musicxml_loss.preserved_value.as_deref(), Some("roman"));
    assert!(
        musicxml_loss
            .source_location
            .as_deref()
            .is_some_and(|path| path.ends_with("/harm@type"))
    );

    let abc = acorde_io::serialize_abc_with_report(&score).unwrap();
    let abc_loss = abc
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.preserved_value.as_deref() == Some("roman"))
        .expect("ABC harmonic type loss is diagnosed");
    assert!(
        abc_loss
            .source_location
            .as_deref()
            .is_some_and(|path| path.ends_with("/harm@type"))
    );
    let musicxml_ref_loss = musicxml
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "musicxml.export-unsupported-mei-chordref")
        .expect("MusicXML chordref loss is diagnosed");
    assert_eq!(
        musicxml_ref_loss.preserved_value.as_deref(),
        Some("#harmonychordA")
    );
    let musicxml_range_loss = musicxml
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "musicxml.export-unsupported-harmony-range")
        .expect("MusicXML harmony range loss is diagnosed");
    assert_eq!(
        musicxml_range_loss.preserved_value.as_deref(),
        Some("0:0:0:0:0")
    );
    assert!(
        musicxml_range_loss
            .source_location
            .as_deref()
            .is_some_and(|path| path.ends_with("/harm@end"))
    );
    let abc_ref_loss = abc
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.preserved_value.as_deref() == Some("#harmonychordA"))
        .expect("ABC chordref loss is diagnosed");
    assert!(
        abc_ref_loss
            .source_location
            .as_deref()
            .is_some_and(|path| path.ends_with("/harm@chordref"))
    );
    let musicxml_definition_loss = musicxml
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "musicxml.export-unsupported-mei-chord-definition")
        .expect("MusicXML chord definition loss is diagnosed");
    assert_eq!(
        musicxml_definition_loss.preserved_value.as_deref(),
        Some("harmonychordA")
    );
    let abc_definition_loss = abc
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.preserved_value.as_deref() == Some("harmonychordA"))
        .expect("ABC chord definition loss is diagnosed");
    assert!(
        abc_definition_loss
            .source_location
            .as_deref()
            .is_some_and(|path| path.ends_with("/chord-definitions/1"))
    );
}

#[cfg(feature = "mscz")]
#[test]
fn tablature_semantics_match_between_musicxml_and_mscx() {
    use acorde_core::{Duration, GuitarTechnique, Note, Pitch, Score, Step, TablatureConfig};

    let mut score = Score::new("Tab equivalence", 120, 4, 4, 0, 1);
    let staff = &mut score.parts[0].staves[0];
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![40, 45, 50, 55, 59, 64],
        capo: 0,
    });
    let mut note = Note::new(Pitch::new(Step::G, 2), Duration::Quarter);
    note.tab_position = Some(acorde_core::TabPosition { string: 1, fret: 3 });
    note.fingering = Some(2);
    note.guitar_technique = Some(GuitarTechnique::Slide);
    staff.measures[0].voices[0] = vec![note];
    let serialized = serialize_musicxml(&score).expect("tab XML serializes");
    assert!(
        serialized.contains("<string>6</string>"),
        "MusicXML string numbers count from the high string"
    );
    let musicxml = parse_musicxml(&serialized).expect("tab XML parses");
    let mscx = acorde_io::parse_mscx(
    r#"<museScore><Score><Part><Staff id="1"/></Part><Staff id="1"><StaffType group="tab"><lines>6</lines><StringData><string>40</string><string>45</string><string>50</string><string>55</string><string>59</string><string>64</string></StringData></StaffType><Measure><Chord><durationType>quarter</durationType><Note><pitch>43</pitch><tpc>8</tpc><string>1</string><fret>3</fret><Fingering>2</Fingering><Slide/></Note></Chord></Measure></Staff></Score></museScore>"#,
    )
    .expect("tab MSCX parses");

    let xml_staff = &musicxml.parts[0].staves[0];
    let mscx_staff = &mscx.parts[0].staves[0];
    assert_eq!(xml_staff.tablature, mscx_staff.tablature);
    assert_eq!(
        xml_staff.measures[0].voices[0][0].tab_position,
        mscx_staff.measures[0].voices[0][0].tab_position
    );
    assert_eq!(
        xml_staff.measures[0].voices[0][0].guitar_technique,
        mscx_staff.measures[0].voices[0][0].guitar_technique
    );
    assert_eq!(
        xml_staff.measures[0].voices[0][0].fingering,
        mscx_staff.measures[0].voices[0][0].fingering
    );
}

#[test]
fn musicxml_cue_note_roundtrip() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("Cue", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::G, 4), Duration::Quarter);
    note.is_cue = true;
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];
    let xml = serialize_musicxml(&score).expect("serialize failed");
    assert!(xml.contains("<cue/>"), "cue element in XML");
    let score2 = parse_musicxml(&xml).expect("parse failed");
    assert!(score2.parts[0].staves[0].measures[0].voices[0][0].is_cue);
}

#[test]
fn cue_note_beats_zero() {
    use acorde_core::{Duration, Note, Pitch, Step};
    let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
    assert!((note.beats() - 1.0).abs() < 1e-9, "normal note beats");
    note.is_cue = true;
    assert_eq!(note.beats(), 0.0, "cue note beats are zero");
}

#[test]
fn musicxml_notehead_diamond_roundtrip() {
    use acorde_core::{Duration, Note, NoteHead, Pitch, Score, Step};
    let mut score = Score::new("NH", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Whole);
    note.note_head = NoteHead::Diamond;
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];
    let xml = serialize_musicxml(&score).expect("serialize failed");
    assert!(
        xml.contains("<notehead>diamond</notehead>"),
        "diamond in XML"
    );
    let score2 = parse_musicxml(&xml).expect("parse failed");
    assert_eq!(
        score2.parts[0].staves[0].measures[0].voices[0][0].note_head,
        NoteHead::Diamond
    );
}

#[test]
fn musicxml_notehead_normal_not_emitted() {
    use acorde_core::Score;
    let score = Score::new("NH", 120, 4, 4, 0, 1);
    let xml = serialize_musicxml(&score).expect("serialize failed");
    assert!(!xml.contains("<notehead>"), "normal notehead not emitted");
}

// ── Part group ─────────────────────────────────────────────────────────────────

#[test]
fn musicxml_part_group_bracket_roundtrip() {
    use acorde_core::{PartGroup, PartGroupSymbol, Score};
    let mut score = Score::template(acorde_core::ScoreTemplate::StringQuartet);
    score.part_groups.push(PartGroup {
        first_part: 0,
        last_part: 3,
        symbol: PartGroupSymbol::Bracket,
        barlines_connect: true,
    });
    let xml = serialize_musicxml(&score).expect("serialize failed");
    assert!(xml.contains("part-group"), "part-group in XML");
    assert!(xml.contains("bracket"), "bracket symbol in XML");
    let score2 = parse_musicxml(&xml).expect("parse failed");
    assert_eq!(score2.part_groups.len(), 1);
    assert_eq!(score2.part_groups[0].first_part, 0);
    assert_eq!(score2.part_groups[0].last_part, 3);
    assert_eq!(score2.part_groups[0].symbol, PartGroupSymbol::Bracket);
}

// ── Trill line ─────────────────────────────────────────────────────────────────

#[test]
fn musicxml_trill_line_roundtrip() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("Trill", 120, 4, 4, 0, 2);
    let mut n1 = Note::new(Pitch::new(Step::C, 5), Duration::Half);
    n1.trill_line_start = true;
    let mut n2 = Note::new(Pitch::new(Step::D, 5), Duration::Half);
    n2.trill_line_end = true;
    score.parts[0].staves[0].measures[0].voices[0] = vec![n1, n2];
    let xml = serialize_musicxml(&score).expect("serialize failed");
    assert!(xml.contains("wavy-line"), "wavy-line in XML");
    let score2 = parse_musicxml(&xml).expect("parse failed");
    let v = &score2.parts[0].staves[0].measures[0].voices[0];
    assert!(v[0].trill_line_start, "trill_line_start on first note");
    assert!(v[1].trill_line_end, "trill_line_end on second note");
}

// ── Expression text ────────────────────────────────────────────────────────────

#[test]
fn musicxml_expression_text_roundtrip() {
    use acorde_core::Score;
    let mut score = Score::new("Expr", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].expression_text = Some("dolce".to_string());
    let xml = serialize_musicxml(&score).expect("serialize failed");
    assert!(
        xml.contains("<words>dolce</words>"),
        "expression words in XML"
    );
    let score2 = parse_musicxml(&xml).expect("parse failed");
    assert_eq!(
        score2.parts[0].staves[0].measures[0].expression_text,
        Some("dolce".to_string())
    );
}

// ── ScorePatch / apply_patch ──────────────────────────────────────────────────

#[test]
fn score_patch_apply_round_trips_note_replacement() {
    use acorde_core::{Duration, Note, Pitch, Score, Step, apply_patch, score_patch};
    let mut score_a = Score::new("P", 120, 4, 4, 0, 1);
    score_a.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
    let mut score_b = score_a.clone();
    score_b.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::new(Step::D, 4), Duration::Whole)];

    let patches = score_patch(&score_a, &score_b);
    assert!(!patches.is_empty(), "patch list is non-empty");
    let result = apply_patch(&score_a, &patches).expect("apply_patch failed");
    let orig_pitch = &score_b.parts[0].staves[0].measures[0].voices[0][0].pitches[0];
    let patched_pitch = &result.parts[0].staves[0].measures[0].voices[0][0].pitches[0];
    assert_eq!(patched_pitch.step, orig_pitch.step);
}

#[test]
fn score_patch_identical_scores_produces_empty_patch() {
    use acorde_core::{Score, score_patch};
    let score = Score::new("P", 120, 4, 4, 0, 1);
    assert!(score_patch(&score, &score).is_empty());
}

#[test]
fn apply_patch_out_of_bounds_returns_err() {
    use acorde_core::{Score, ScorePatch, apply_patch};
    let score = Score::new("P", 120, 4, 4, 0, 1);
    let patches = vec![ScorePatch::RemoveNote {
        part: 99,
        staff: 0,
        measure: 0,
        voice: 0,
        note_index: 0,
    }];
    assert!(apply_patch(&score, &patches).is_err());
}

// ── New Feature round-trips ───────────────────────────────────────────────────

#[test]
fn musicxml_stem_up_roundtrip() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("Stem", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
    note.stem_up = Some(true);
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let xml = serialize_musicxml(&score).expect("serialize");
    assert!(xml.contains("<stem>up</stem>"), "stem up should be in XML");

    let score2 = parse_musicxml(&xml).expect("parse");
    assert_eq!(
        score2.parts[0].staves[0].measures[0].voices[0][0].stem_up,
        Some(true)
    );
}

#[test]
fn musicxml_stem_down_roundtrip() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("Stem", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::G, 5), Duration::Quarter);
    note.stem_up = Some(false);
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let xml = serialize_musicxml(&score).expect("serialize");
    assert!(
        xml.contains("<stem>down</stem>"),
        "stem down should be in XML"
    );

    let score2 = parse_musicxml(&xml).expect("parse");
    assert_eq!(
        score2.parts[0].staves[0].measures[0].voices[0][0].stem_up,
        Some(false)
    );
}

#[test]
fn musicxml_inverted_mordent_roundtrip() {
    use acorde_core::{Articulation, Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("Ornament", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
    note.articulations = vec![Articulation::InvertedMordent];
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let xml = serialize_musicxml(&score).expect("serialize");
    assert!(
        xml.contains("<inverted-mordent/>"),
        "inverted-mordent in XML"
    );

    let score2 = parse_musicxml(&xml).expect("parse");
    let arts = &score2.parts[0].staves[0].measures[0].voices[0][0].articulations;
    assert!(arts.contains(&Articulation::InvertedMordent));
}

#[test]
fn musicxml_inverted_turn_roundtrip() {
    use acorde_core::{Articulation, Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("Ornament", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
    note.articulations = vec![Articulation::InvertedTurn];
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let xml = serialize_musicxml(&score).expect("serialize");
    assert!(xml.contains("<inverted-turn/>"), "inverted-turn in XML");

    let score2 = parse_musicxml(&xml).expect("parse");
    let arts = &score2.parts[0].staves[0].measures[0].voices[0][0].articulations;
    assert!(arts.contains(&Articulation::InvertedTurn));
}

#[test]
fn musicxml_shake_roundtrip() {
    use acorde_core::{Articulation, Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("Ornament", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
    note.articulations = vec![Articulation::Shake];
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let xml = serialize_musicxml(&score).expect("serialize");
    assert!(xml.contains("<shake/>"), "shake in XML");

    let score2 = parse_musicxml(&xml).expect("parse");
    let arts = &score2.parts[0].staves[0].measures[0].voices[0][0].articulations;
    assert!(arts.contains(&Articulation::Shake));
}

#[test]
fn musicxml_guitar_bend_roundtrip() {
    use acorde_core::{Duration, GuitarTechnique, Note, Pitch, Score, Step};
    let mut score = Score::new("Guitar", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::G, 4), Duration::Quarter);
    note.guitar_technique = Some(GuitarTechnique::Bend);
    note.guitar_bend_alter_cents = Some(200);
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let xml = serialize_musicxml(&score).expect("serialize");
    assert!(xml.contains("<bend>"), "bend in XML");

    let score2 = parse_musicxml(&xml).expect("parse");
    assert_eq!(
        score2.parts[0].staves[0].measures[0].voices[0][0].guitar_technique,
        Some(GuitarTechnique::Bend)
    );
    assert_eq!(
        score2.parts[0].staves[0].measures[0].voices[0][0].guitar_bend_alter_cents,
        Some(200)
    );
}

#[test]
fn musicxml_guitar_hammer_on_roundtrip() {
    use acorde_core::{Duration, GuitarTechnique, Note, Pitch, Score, Step};
    let mut score = Score::new("Guitar", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::A, 4), Duration::Quarter);
    note.guitar_technique = Some(GuitarTechnique::HammerOn);
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let xml = serialize_musicxml(&score).expect("serialize");
    assert!(xml.contains("<hammer-on"), "hammer-on in XML");

    let score2 = parse_musicxml(&xml).expect("parse");
    assert_eq!(
        score2.parts[0].staves[0].measures[0].voices[0][0].guitar_technique,
        Some(GuitarTechnique::HammerOn)
    );
}

#[test]
fn musicxml_guitar_pull_off_roundtrip() {
    use acorde_core::{Duration, GuitarTechnique, Note, Pitch, Score, Step};
    let mut score = Score::new("Guitar", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::new(Step::B, 4), Duration::Quarter);
    note.guitar_technique = Some(GuitarTechnique::PullOff);
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let xml = serialize_musicxml(&score).expect("serialize");
    assert!(xml.contains("<pull-off"), "pull-off in XML");

    let score2 = parse_musicxml(&xml).expect("parse");
    assert_eq!(
        score2.parts[0].staves[0].measures[0].voices[0][0].guitar_technique,
        Some(GuitarTechnique::PullOff)
    );
}

#[test]
fn musicxml_measure_local_tablature_tuning_roundtrip() {
    use acorde_core::{Score, TablatureConfig};
    let mut score = Score::new("Scordatura", 120, 4, 4, 0, 2);
    score.parts[0].staves[0].tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![40, 45, 50, 55, 59, 64],
        capo: 0,
    });
    let changed = TablatureConfig {
        lines: 6,
        tuning_midi: vec![38, 45, 50, 55, 59, 64],
        capo: 0,
    };
    score.parts[0].staves[0].measures[1].tablature_change = Some(changed.clone());

    let xml = serialize_musicxml(&score).expect("serialize");
    assert_eq!(xml.matches("<staff-details>").count(), 2);
    let restored = parse_musicxml(&xml).expect("parse");
    assert_eq!(
        restored.parts[0].staves[0].measures[1].tablature_change,
        Some(changed)
    );
}

#[test]
fn musicxml_staff_lines_roundtrip_without_implying_tablature() {
    use acorde_core::{Score, StaffKind};
    let mut score = Score::new("One-line percussion", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].presentation.lines = 1;

    let xml = serialize_musicxml(&score).expect("serialize");
    assert!(xml.contains("<staff-lines>1</staff-lines>"));
    let restored = parse_musicxml(&xml).expect("parse");
    let staff = &restored.parts[0].staves[0];
    assert_eq!(staff.presentation.lines, 1);
    assert_eq!(staff.presentation.kind, StaffKind::Standard);
    assert!(staff.tablature.is_none());
}

#[test]
fn musicxml_extra_staff_lines_roundtrip_to_matching_staff() {
    use acorde_core::{Duration, Note, Pitch, Score, ScoreTemplate, StaffKind, Step};
    let mut score = Score::template(ScoreTemplate::Piano);
    score.parts[0].staves[1].presentation.lines = 4;
    score.parts[0].staves[1].measures[0].voices[0] =
        vec![Note::new(Pitch::new(Step::C, 3), Duration::Quarter)];

    let xml = serialize_musicxml(&score).expect("serialize");
    assert!(xml.contains("<staff-details number=\"2\">"));
    let restored = parse_musicxml(&xml).expect("parse");
    let staff = &restored.parts[0].staves[1];
    assert_eq!(staff.presentation.lines, 4);
    assert_eq!(staff.presentation.kind, StaffKind::Standard);
    assert!(staff.tablature.is_none());
}

fn cross_staff_target(score: &acorde_core::Score, staff: usize, note: usize) -> Option<usize> {
    score.parts[0].staves[staff].measures[0].voices[0][note]
        .cross_staff
        .as_ref()
        .map(|placement| placement.target_staff)
}

#[test]
fn declared_musicxml_staves_materialize_and_round_trip() {
    let parsed = parse_musicxml(DECLARED_STAVES_XML).expect("declared-staves fixture parses");
    assert!(acorde_core::validate(&parsed).is_valid());
    let part = &parsed.parts[0];
    assert_eq!(part.staves.len(), 2);
    assert!(part.staves.iter().all(|staff| staff.measures.len() == 2));

    // Voice 1 continues its timeline on staff 2 for one note: a placement owned by staff 1.
    let upper = &part.staves[0].measures[0].voices[0];
    assert_eq!(upper.len(), 4);
    assert_eq!(upper[2].pitches[0].step, Step::E);
    assert_eq!(cross_staff_target(&parsed, 0, 2), Some(1));
    assert_eq!(cross_staff_target(&parsed, 0, 1), None);
    // Voice 5 is owned by the declared second staff.
    let lower = &part.staves[1].measures[0];
    assert_eq!(lower.source_voice_numbers[0], Some(5));
    assert_eq!(lower.voices[0].len(), 2);
    assert!(
        lower.voices[0]
            .iter()
            .all(|note| note.cross_staff.is_none())
    );

    let xml = serialize_musicxml(&parsed).expect("declared-staves fixture serializes");
    assert!(xml.contains("<staves>2</staves>"));
    let restored = parse_musicxml(&xml).expect("declared-staves fixture reparses");
    assert_eq!(restored.parts[0].staves.len(), 2);
    for (original, restored) in part.staves.iter().zip(&restored.parts[0].staves) {
        assert_eq!(original.measures.len(), restored.measures.len());
        for (left, right) in original.measures.iter().zip(&restored.measures) {
            assert_eq!(left.source_voice_numbers, right.source_voice_numbers);
            for (left, right) in left.voices.iter().zip(&right.voices) {
                assert_eq!(left.len(), right.len());
                for (left, right) in left.iter().zip(right) {
                    assert_eq!(left.is_rest, right.is_rest);
                    assert_eq!(left.pitches, right.pitches);
                    assert_eq!(left.duration, right.duration);
                    assert_eq!(left.cross_staff, right.cross_staff);
                }
            }
        }
    }
}

#[test]
fn empty_declared_musicxml_staff_is_preserved() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><staves>2</staves><clef number="1"><sign>G</sign><line>2</line></clef></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type><staff>1</staff></note></measure><measure number="2"><note><rest/><duration>4</duration><voice>1</voice><type>whole</type><staff>1</staff></note></measure></part></score-partwise>"#;
    let parsed = parse_musicxml(xml).expect("empty declared staff parses");
    assert!(acorde_core::validate(&parsed).is_valid());
    assert_eq!(parsed.parts[0].staves.len(), 2);
    assert_eq!(parsed.parts[0].staves[1].measures.len(), 2);
    assert_eq!(parsed.parts[0].staves[1].measures[1].number, 2);

    let serialized = serialize_musicxml(&parsed).expect("empty declared staff serializes");
    assert!(serialized.contains("<staves>2</staves>"));
    assert!(serialized.contains("<clef number=\"2\">"));
    let restored = parse_musicxml(&serialized).expect("empty declared staff reparses");
    assert_eq!(restored.parts[0].staves.len(), 2);
    assert!(acorde_core::validate(&restored).is_valid());
}

#[test]
fn imported_declared_staves_accept_undoable_cross_staff_edit() {
    use acorde_core::{CrossStaff, SetCrossStaffCmd};

    let parsed = parse_musicxml(DECLARED_STAVES_XML).expect("declared-staves fixture parses");
    let mut engine = ScoreEngine::new();
    engine
        .try_replace_score(parsed)
        .expect("imported two-staff score validates");
    engine
        .apply(Command::SetCrossStaff(SetCrossStaffCmd {
            part_index: 0,
            staff_index: 0,
            measure_index: 0,
            voice: 0,
            note_index: 1,
            placement: Some(CrossStaff {
                target_staff: 1,
                target_voice: None,
            }),
        }))
        .expect("imported note accepts a live target staff");
    assert_eq!(cross_staff_target(&engine.score, 0, 1), Some(1));
    engine.undo().expect("cross-staff edit undoes");
    assert_eq!(cross_staff_target(&engine.score, 0, 1), None);
    engine.redo().expect("cross-staff edit redoes");
    assert_eq!(cross_staff_target(&engine.score, 0, 1), Some(1));

    let json = serde_json::to_string(&engine.score).expect("score saves as JSON");
    let reloaded: acorde_core::Score = serde_json::from_str(&json).expect("JSON reloads");
    assert_eq!(cross_staff_target(&reloaded, 0, 1), Some(1));

    let xml = serialize_musicxml(&engine.score).expect("score saves as MusicXML");
    let reparsed = parse_musicxml(&xml).expect("MusicXML reloads");
    assert_eq!(reparsed.parts[0].staves.len(), 2);
    assert_eq!(cross_staff_target(&reparsed, 0, 1), Some(1));
    assert_eq!(cross_staff_target(&reparsed, 0, 2), Some(1));
    assert_eq!(reparsed.parts[0].staves[1].measures[0].voices[0].len(), 2);
}

#[test]
fn musicxml_time_local_capo_change_round_trips() {
    use acorde_core::{Duration, Measure, Note, Pitch, Score, Staff, Step, TablatureConfig};
    let mut score = Score::new("Capo change", 120, 4, 4, 0, 1);
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 0,
    });
    for _ in 0..2 {
        let mut measure = Measure::empty(4, 4);
        let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Whole);
        note.tab_position = Some(acorde_core::TabPosition { string: 1, fret: 0 });
        measure.voices[0] = vec![note];
        staff.measures.push(measure);
    }
    staff.measures[1].tablature_change = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 3,
    });
    score.parts[0].staves = vec![staff];

    let xml = serialize_musicxml(&score).expect("capo change serializes");
    assert_eq!(xml.matches("<capo>").count(), 1);
    let restored = parse_musicxml(&xml).expect("capo change reparses");
    let staff = &restored.parts[0].staves[0];
    assert_eq!(staff.tablature.as_ref().map(|tab| tab.capo), Some(0));
    assert_eq!(
        staff.measures[1]
            .tablature_change
            .as_ref()
            .map(|tab| tab.capo),
        Some(3)
    );
}

const PICKUP_XML: &str = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list><part id="P1"><measure number="0" implicit="yes"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note></measure><measure number="1"><note><pitch><step>C</step><octave>5</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note></measure><measure number="2"><note><pitch><step>B</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note><note><rest/><duration>1</duration><voice>1</voice><type>quarter</type></note></measure></part></score-partwise>"#;

#[test]
fn musicxml_pickup_and_incomplete_final_bar_keep_their_length() {
    use acorde_core::MeasureLength;
    let parsed = parse_musicxml(PICKUP_XML).expect("pickup fixture parses");
    assert!(acorde_core::validate(&parsed).is_valid());
    let measures = &parsed.parts[0].staves[0].measures;
    let quarter = MeasureLength {
        numerator: 1,
        denominator: 4,
    };
    let three_quarters = MeasureLength {
        numerator: 3,
        denominator: 4,
    };
    assert_eq!(measures[0].actual_length, Some(quarter));
    assert_eq!(measures[0].voices[0].len(), 1, "no padding rests");
    assert_eq!(measures[1].actual_length, None);
    assert_eq!(measures[2].actual_length, Some(three_quarters));
    // 1 + 4 + 3 beats at the default 120 BPM.
    assert!((acorde_core::score_duration_secs(&parsed) - 4.0).abs() < 1e-9);

    let xml = serialize_musicxml(&parsed).expect("pickup serializes");
    assert!(xml.contains("<measure number=\"0\" implicit=\"yes\">"));
    assert_eq!(xml.matches("implicit=").count(), 1);
    let restored = parse_musicxml(&xml).expect("pickup reparses");
    let restored = &restored.parts[0].staves[0].measures;
    assert_eq!(restored[0].actual_length, Some(quarter));
    assert_eq!(restored[2].actual_length, Some(three_quarters));
    assert_eq!(restored[0].voices[0].len(), 1);
}

#[cfg(feature = "mscz")]
#[test]
fn mscx_measure_len_round_trips_as_actual_length() {
    use acorde_core::MeasureLength;
    let parsed = parse_musicxml(PICKUP_XML).expect("pickup fixture parses");
    let mscx = acorde_io::serialize_mscx(&parsed).expect("pickup serializes as MSCX");
    assert!(mscx.contains("len=\"1/4\""));
    assert!(mscx.contains("len=\"3/4\""));
    let restored = acorde_io::parse_mscx(&mscx).expect("MSCX pickup reparses");
    let measures = &restored.parts[0].staves[0].measures;
    assert_eq!(
        measures[0].actual_length,
        Some(MeasureLength {
            numerator: 1,
            denominator: 4
        })
    );
    assert_eq!(measures[1].actual_length, None);
    assert!(acorde_core::validate(&restored).is_valid());
}

const VERSES_XML: &str = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>C</step><octave>5</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><lyric number="1"><syllabic>begin</syllabic><text>Hal</text></lyric><lyric number="2"><syllabic>single</syllabic><text>Sing</text></lyric><lyric number="3"><syllabic>single</syllabic><text>Joy</text></lyric></note><note><pitch><step>D</step><octave>5</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><lyric number="1"><syllabic>end</syllabic><text>le</text></lyric><lyric><syllabic>single</syllabic><text>now</text></lyric></note></measure></part></score-partwise>"#;

fn verse_texts(note: &acorde_core::Note) -> Vec<(u8, String)> {
    note.lyric
        .iter()
        .map(|lyric| (1, lyric.text.clone()))
        .chain(
            note.additional_lyrics
                .iter()
                .map(|entry| (entry.verse, entry.lyric.text.clone())),
        )
        .collect()
}

#[test]
fn musicxml_lyric_verses_are_kept_and_round_trip() {
    let parsed = parse_musicxml(VERSES_XML).expect("verses parse");
    assert!(acorde_core::validate(&parsed).is_valid());
    let voice = &parsed.parts[0].staves[0].measures[0].voices[0];
    assert_eq!(
        verse_texts(&voice[0]),
        vec![(1, "Hal".into()), (2, "Sing".into()), (3, "Joy".into())]
    );
    // An unnumbered lyric takes the lowest free verse instead of overwriting verse 1.
    assert_eq!(
        verse_texts(&voice[1]),
        vec![(1, "le".into()), (2, "now".into())]
    );

    let xml = serialize_musicxml(&parsed).expect("verses serialize");
    assert!(xml.contains("<lyric number=\"3\">"));
    let restored = parse_musicxml(&xml).expect("verses reparse");
    let restored = &restored.parts[0].staves[0].measures[0].voices[0];
    assert_eq!(verse_texts(&restored[0]), verse_texts(&voice[0]));
    assert_eq!(verse_texts(&restored[1]), verse_texts(&voice[1]));
    assert_eq!(restored[0].additional_lyrics, voice[0].additional_lyrics);
}

#[cfg(feature = "mscz")]
#[test]
fn mscx_lyric_verses_round_trip_with_zero_based_numbers() {
    let parsed = parse_musicxml(VERSES_XML).expect("verses parse");
    let mscx = acorde_io::serialize_mscx(&parsed).expect("verses serialize as MSCX");
    assert!(mscx.contains("<Lyrics><no>2</no>"));
    let restored = acorde_io::parse_mscx(&mscx).expect("MSCX verses reparse");
    let original = &parsed.parts[0].staves[0].measures[0].voices[0];
    let restored = &restored.parts[0].staves[0].measures[0].voices[0];
    assert_eq!(verse_texts(&restored[0]), verse_texts(&original[0]));
    assert_eq!(verse_texts(&restored[1]), verse_texts(&original[1]));
}

#[cfg(all(feature = "abc", feature = "mei", feature = "midi"))]
#[test]
fn verse_exports_report_losses_or_round_trip_through_mei() {
    let parsed = parse_musicxml(VERSES_XML).expect("verses parse");
    let located = |diagnostics: &[acorde_io::Diagnostic], code: &str| {
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == code)
            .map(|diagnostic| {
                (
                    diagnostic.source_location.clone().unwrap_or_default(),
                    diagnostic.preserved_value.clone().unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>()
    };
    let abc = acorde_io::serialize_abc_with_report(&parsed).expect("ABC export");
    let losses = located(&abc.diagnostics, "abc.export-unsupported-lyric-verse");
    assert_eq!(losses.len(), 3);
    assert!(losses.contains(&(
        "/score/part/1/staff/1/measure/1/voice/1/note/1/lyric/3".into(),
        "Joy".into()
    )));
    // MEI writes every verse as `<verse n>` note content, so it reports no verse loss.
    let mei = acorde_io::serialize_mei_with_report(&parsed).expect("MEI export");
    assert!(located(&mei.diagnostics, "mei.export-unsupported-lyric-verse").is_empty());
    assert!(mei.output.contains("<verse n=\"2\">"));
    let restored = acorde_io::parse_mei(&mei.output).expect("MEI verses reparse");
    let original = &parsed.parts[0].staves[0].measures[0].voices[0];
    let restored = &restored.parts[0].staves[0].measures[0].voices[0];
    assert_eq!(verse_texts(&restored[0]), verse_texts(&original[0]));
    assert_eq!(verse_texts(&restored[1]), verse_texts(&original[1]));
    let midi = acorde_io::serialize_midi_with_report(&parsed).expect("MIDI export");
    assert_eq!(
        located(&midi.diagnostics, "midi.export-unsupported-lyric-verse").len(),
        3
    );
}

#[cfg(feature = "mscz")]
const MSCX_LINES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="4.20"><Score><Division>480</Division>
<Part id="1"><Staff id="1"/><trackName>Piano</trackName><Instrument id="piano"><longName>Piano</longName></Instrument></Part>
<Staff id="1">
<Measure><LayoutBreak><subtype>section</subtype></LayoutBreak><voice>
<TimeSig><sigN>3</sigN><sigD>4</sigD></TimeSig>
<Spanner type="HairPin"><HairPin><subtype>1</subtype></HairPin><next><location><fractions>3/4</fractions></location></next></Spanner>
<Spanner type="Pedal"><Pedal><visible>0</visible></Pedal><next><location><fractions>3/4</fractions></location></next></Spanner>
<Chord><durationType>half</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
<Chord><durationType>quarter</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
</voice></Measure>
<Measure><voice>
<Spanner type="HairPin"><prev><location><fractions>-1/4</fractions></location></prev></Spanner>
<Spanner type="Pedal"><prev><location><fractions>-1/4</fractions></location></prev></Spanner>
<Rest><durationType>measure</durationType><duration>3/4</duration></Rest>
<BarLine><subtype>end</subtype></BarLine>
</voice></Measure>
<Measure><voice><Rest><durationType>measure</durationType><duration>3/4</duration></Rest><BarLine><subtype>heavy</subtype></BarLine></voice></Measure>
</Staff></Score></museScore>"#;

#[cfg(feature = "mscz")]
#[test]
fn mscx_voice_level_lines_breaks_barlines_and_measure_rests_import() {
    let report = acorde_io::parse_mscx_with_report(MSCX_LINES).expect("MSCX lines parse");
    let score = &report.score;
    assert!(
        acorde_core::validate(score).is_valid(),
        "{:?}",
        acorde_core::validate(score).errors
    );
    let measures = &score.parts[0].staves[0].measures;
    let first = &measures[0].voices[0];
    assert_eq!(
        first[0].hairpin_start,
        Some(acorde_core::HairpinKind::Decrescendo)
    );
    assert!(first[0].pedal_start);
    // The end markers open the next measure, so they close the previous measure's last chord.
    assert!(first[1].hairpin_end);
    assert!(first[1].pedal_end);
    assert!(measures[0].section_break);
    assert!(matches!(
        measures[1].barline_right,
        acorde_core::Barline::Final
    ));
    assert!(matches!(
        measures[2].barline_right,
        acorde_core::Barline::Normal
    ));
    // A 3/4 measure rest imports as a plain whole rest that fills the bar.
    assert!(measures[1].voices[0][0].is_plain_whole_rest());
    assert_eq!(
        acorde_core::measure_beats_remaining(score, 0, 0, 1, 0).expect("capacity"),
        0.0
    );
    let codes: Vec<(&str, Option<&str>)> = report
        .diagnostics
        .iter()
        .map(|diagnostic| {
            (
                diagnostic.code.as_str(),
                diagnostic.preserved_value.as_deref(),
            )
        })
        .collect();
    assert!(codes.contains(&("mscx.unsupported-visibility", Some("0"))));
    assert!(codes.contains(&("mscx.unsupported-barline", Some("heavy"))));
}

#[test]
fn musicxml_measure_rest_fills_any_time_signature() {
    use acorde_core::{Command, CommandStack, SetTimeSignatureCmd};
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>V</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>2</divisions><time><beats>6</beats><beat-type>8</beat-type></time></attributes><note><rest measure="yes"/><duration>6</duration><voice>1</voice></note></measure><measure number="2"><note><pitch><step>C</step><octave>5</octave></pitch><duration>6</duration><voice>1</voice><type>half</type><dot/></note></measure></part></score-partwise>"#;
    let mut score = parse_musicxml(xml).expect("6/8 measure rest parses");
    assert!(
        acorde_core::validate(&score).is_valid(),
        "{:?}",
        acorde_core::validate(&score).errors
    );
    // 3 + 3 quarter-note beats at 120 BPM.
    assert!((acorde_core::score_duration_secs(&score) - 3.0).abs() < 1e-9);
    let serialized = serialize_musicxml(&score).expect("measure rest serializes");
    assert!(serialized.contains("<rest measure=\"yes\"/>"));
    let restored = parse_musicxml(&serialized).expect("measure rest reparses");
    assert!(restored.parts[0].staves[0].measures[0].voices[0][0].is_plain_whole_rest());

    let mut stack = CommandStack::new(10);
    stack
        .execute(
            Command::SetTimeSignature(SetTimeSignatureCmd {
                numerator: 3,
                denominator: 4,
            }),
            &mut score,
        )
        .expect("time signature changes");
    assert!(score.parts[0].staves[0].measures[0].voices[0][0].is_plain_whole_rest());
}

#[cfg(feature = "mscz")]
#[test]
fn mscx_breaks_barlines_and_measure_rests_round_trip() {
    let parsed = acorde_io::parse_mscx(MSCX_LINES).expect("MSCX lines parse");
    let report = acorde_io::serialize_mscx_with_report(&parsed).expect("MSCX export");
    assert!(
        report
            .output
            .contains("<LayoutBreak><subtype>section</subtype></LayoutBreak>")
    );
    assert!(
        report
            .output
            .contains("<BarLine><subtype>end</subtype></BarLine>")
    );
    assert!(
        report
            .output
            .contains("<durationType>measure</durationType><duration>3/4</duration>")
    );
    assert!(!report.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .source_location
            .as_deref()
            .is_some_and(|path| path.ends_with("/layout"))
    }));
    // Hairpins and pedal lines are written as MuseScore spanners, so they are not reported.
    assert!(!report.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .source_location
            .as_deref()
            .is_some_and(|path| path.ends_with("/measure/1/voice/1/note/1"))
    }));
    let restored = acorde_io::parse_mscx(&report.output).expect("MSCX export reparses");
    let lines = |score: &acorde_core::Score| -> Vec<(bool, bool, bool, bool)> {
        score.parts[0].staves[0]
            .measures
            .iter()
            .flat_map(|measure| measure.voices.iter().flatten())
            .map(|note| {
                (
                    note.hairpin_start.is_some(),
                    note.hairpin_end,
                    note.pedal_start,
                    note.pedal_end,
                )
            })
            .collect()
    };
    assert_eq!(lines(&restored), lines(&parsed));
    assert!(acorde_core::validate(&restored).is_valid());
    let measures = &restored.parts[0].staves[0].measures;
    assert!(measures[0].section_break);
    assert!(matches!(
        measures[1].barline_right,
        acorde_core::Barline::Final
    ));
    assert!(measures[2].voices[0][0].is_plain_whole_rest());
}

fn technique_note_xml(technical: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Violin</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>A</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type><notations><technical>{technical}</technical></notations></note></measure></part></score-partwise>"#
    )
}

#[test]
fn string_technique_marks_round_trip_through_musicxml() {
    use acorde_core::Articulation;
    let xml = technique_note_xml(
        "<up-bow/><down-bow/><harmonic><natural/></harmonic><open-string/><stopped/><snap-pizzicato/>",
    );
    let parsed = parse_musicxml(&xml).expect("technique marks parse");
    let expected = vec![
        Articulation::UpBow,
        Articulation::DownBow,
        Articulation::Harmonic,
        Articulation::OpenString,
        Articulation::Stopped,
        Articulation::SnapPizzicato,
    ];
    let note = &parsed.parts[0].staves[0].measures[0].voices[0][0];
    assert_eq!(note.articulations, expected);
    let report = acorde_io::serialize_musicxml_with_report(&parsed).expect("marks serialize");
    assert!(report.output.contains("<technical>"));
    assert!(report.output.contains("<snap-pizzicato/>"));
    assert!(!report.output.contains("<articulations>"));
    let restored = parse_musicxml(&report.output).expect("marks reparse");
    assert_eq!(
        restored.parts[0].staves[0].measures[0].voices[0][0].articulations,
        expected
    );
}

#[cfg(all(feature = "mscz", feature = "mei", feature = "abc"))]
#[test]
fn string_technique_marks_round_trip_through_mscx_mei_and_abc() {
    use acorde_core::Articulation;
    let parsed = parse_musicxml(&technique_note_xml("<up-bow/><down-bow/><harmonic/>"))
        .expect("technique marks parse");
    let marks = |score: &acorde_core::Score| {
        score.parts[0].staves[0].measures[0].voices[0][0]
            .articulations
            .clone()
    };
    let expected = vec![
        Articulation::UpBow,
        Articulation::DownBow,
        Articulation::Harmonic,
    ];
    let mscx = acorde_io::serialize_mscx(&parsed).expect("MSCX export");
    assert!(mscx.contains("stringsUpBow"));
    assert_eq!(
        marks(&acorde_io::parse_mscx(&mscx).expect("MSCX reparse")),
        expected
    );
    let mei = acorde_io::serialize_mei(&parsed).expect("MEI export");
    assert!(mei.contains("artic=\"") && mei.contains("dnbow"));
    assert_eq!(
        marks(&acorde_io::parse_mei(&mei).expect("MEI reparse")),
        expected
    );
    // ABC has no harmonic decoration: the bowings survive and the harmonic is a reported loss.
    let abc = acorde_io::serialize_abc_with_report(&parsed).expect("ABC export");
    assert!(abc.output.contains("!upbow!"));
    assert_eq!(
        marks(&acorde_io::parse_abc(&abc.output).expect("ABC reparse")),
        vec![Articulation::UpBow, Articulation::DownBow]
    );
    assert!(!abc.diagnostics.is_empty());
}

#[test]
fn unmodeled_musicxml_repeats_and_lines_are_source_diagnosed() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Drums</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><measure-style><measure-repeat type="start">2</measure-repeat></measure-style></attributes><direction><direction-type><dashes type="start"/></direction-type></direction><note><rest measure="yes"/><duration>4</duration><voice>1</voice><lyric><text>la</text><extend/></lyric></note></measure></part></score-partwise>"#;
    let report = acorde_io::parse_musicxml_with_report(xml).expect("fixture parses");
    let codes: Vec<(&str, &str)> = report
        .diagnostics
        .iter()
        .map(|diagnostic| {
            (
                diagnostic.code.as_str(),
                diagnostic.source_location.as_deref().unwrap_or_default(),
            )
        })
        .collect();
    assert!(codes.contains(&(
        "musicxml.unsupported-multi-measure-repeat",
        "/score-partwise/part/measure/attributes/measure-style/measure-repeat"
    )));
    // Dashes are imported as `NotationSpannerKind::Dashes`, so they are not diagnosed.
    assert!(
        !codes
            .iter()
            .any(|(code, _)| *code == "musicxml.unsupported-element.dashes")
    );
    // Lyric extenders are imported (`Lyric::extend`), so they are no longer diagnosed.
    assert!(
        !codes
            .iter()
            .any(|(code, _)| *code == "musicxml.unsupported-element.extend")
    );
}

const MEASURE_REPEAT_XML: &str = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Guitar</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note><note><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note></measure><measure number="2"><attributes><measure-style><measure-repeat type="start">1</measure-repeat></measure-style></attributes><note><rest measure="yes"/><duration>4</duration><voice>1</voice></note></measure><measure number="3"><note><rest measure="yes"/><duration>4</duration><voice>1</voice></note></measure><measure number="4"><attributes><measure-style><measure-repeat type="stop"/></measure-style></attributes><note><pitch><step>C</step><octave>5</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note></measure></part></score-partwise>"#;

#[test]
fn musicxml_measure_repeats_play_the_repeated_measure_and_round_trip() {
    let parsed = parse_musicxml(MEASURE_REPEAT_XML).expect("measure repeats parse");
    let report = acorde_core::validate(&parsed);
    assert!(report.is_valid(), "{:?}", report.errors);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let measures = &parsed.parts[0].staves[0].measures;
    assert_eq!(measures[1].measure_repeat, Some(1));
    assert_eq!(measures[2].measure_repeat, Some(1));
    assert_eq!(measures[3].measure_repeat, None);
    assert!(measures[1].same_sounding_content(&measures[0]));
    assert!(measures[2].same_sounding_content(&measures[0]));
    assert_ne!(measures[1].voices[0][0].id, measures[0].voices[0][0].id);
    // Two notes in the written bar, two in each repeat, one in the last bar.
    let events = acorde_core::to_playback_events(&parsed, &acorde_core::PlaybackOptions::default());
    assert_eq!(events.len(), 7);

    let xml = serialize_musicxml(&parsed).expect("measure repeats serialize");
    assert_eq!(
        xml.matches("<measure-repeat type=\"start\">1</measure-repeat>")
            .count(),
        1
    );
    assert_eq!(xml.matches("<measure-repeat type=\"stop\"/>").count(), 1);
    let restored = parse_musicxml(&xml).expect("measure repeats reparse");
    let restored = &restored.parts[0].staves[0].measures;
    assert_eq!(
        restored
            .iter()
            .map(|m| m.measure_repeat)
            .collect::<Vec<_>>(),
        vec![None, Some(1), Some(1), None]
    );
    assert!(restored[2].same_sounding_content(&restored[0]));
}

#[test]
fn edited_repeat_source_is_reported_as_differing() {
    let mut parsed = parse_musicxml(MEASURE_REPEAT_XML).expect("measure repeats parse");
    parsed.parts[0].staves[0].measures[0].voices[0][0].pitches[0].octave = 3;
    let report = acorde_core::validate(&parsed);
    assert!(report.is_valid());
    assert!(report.warnings.iter().any(|warning| matches!(
        warning,
        acorde_core::ValidationWarning::MeasureRepeatContentDiffers {
            measure: 1,
            source: 0,
            ..
        }
    )));
}

#[cfg(feature = "mscz")]
#[test]
fn mscx_measure_repeats_import_and_round_trip() {
    let mscx = r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="3.02"><Score><Division>480</Division>
<Part><Staff id="1"/><trackName>Guitar</trackName><Instrument><longName>Guitar</longName></Instrument></Part>
<Staff id="1">
<Measure><voice><TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
<Chord><durationType>half</durationType><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord>
<Chord><durationType>half</durationType><Note><pitch>67</pitch><tpc>15</tpc></Note></Chord>
</voice></Measure>
<Measure><voice><RepeatMeasure><durationType>measure</durationType><duration>4/4</duration></RepeatMeasure></voice></Measure>
<Measure><measureRepeatCount>2</measureRepeatCount><voice><MeasureRepeat><subtype>2</subtype><durationType>measure</durationType><duration>4/4</duration></MeasureRepeat></voice></Measure>
</Staff></Score></museScore>"#;
    let report = acorde_io::parse_mscx_with_report(mscx).expect("MSCX repeats parse");
    let score = &report.score;
    assert!(acorde_core::validate(score).is_valid());
    let measures = &score.parts[0].staves[0].measures;
    assert_eq!(measures[1].measure_repeat, Some(1));
    assert!(measures[1].same_sounding_content(&measures[0]));
    // A two-measure repeat keeps its playable copy but not the sign, and says so.
    assert_eq!(measures[2].measure_repeat, None);
    assert!(measures[2].same_sounding_content(&measures[0]));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "mscx.unsupported-multi-measure-repeat")
    );

    let exported = acorde_io::serialize_mscx(score).expect("MSCX repeats export");
    assert!(exported.contains("<measureRepeatCount>1</measureRepeatCount>"));
    let restored = acorde_io::parse_mscx(&exported).expect("MSCX repeats reparse");
    let restored = &restored.parts[0].staves[0].measures;
    assert_eq!(restored[1].measure_repeat, Some(1));
    assert!(restored[1].same_sounding_content(&restored[0]));
}

#[cfg(all(feature = "mscz", feature = "mei"))]
#[test]
fn mscx_part_default_clefs_set_initial_staff_clefs_and_survive_mei() {
    use acorde_core::Clef;
    // The Lieder file has no first-measure <Clef> for "Men" or the piano left hand: their bass
    // clefs come only from Part/Staff/defaultClef and Instrument/clef staff="2".
    let score = acorde_io::parse_mscx(OPENSCORE_LIEDER_MSCX).expect("Lieder parses");
    let clefs = |score: &acorde_core::Score| {
        score
            .parts
            .iter()
            .flat_map(|part| part.staves.iter().map(|staff| staff.clef.clone()))
            .collect::<Vec<_>>()
    };
    let expected = vec![
        Clef::Treble,
        Clef::Treble,
        Clef::Bass,
        Clef::Treble,
        Clef::Bass,
    ];
    assert_eq!(clefs(&score), expected);
    let mei = acorde_io::serialize_mei(&score).expect("MEI export");
    assert_eq!(
        clefs(&acorde_io::parse_mei(&mei).expect("MEI reparse")),
        expected
    );
}

#[cfg(all(feature = "musicxml", feature = "mei"))]
#[test]
fn cross_staff_notes_round_trip_through_mei_staff_attribute() {
    let xml = include_str!("../../../tests/fixtures/declared_staves_cross_staff.musicxml");
    let parsed = parse_musicxml(xml).expect("cross-staff MusicXML parses");
    let crossings = |score: &acorde_core::Score| {
        score.parts[0]
            .staves
            .iter()
            .flat_map(|staff| &staff.measures)
            .flat_map(|measure| measure.voices.iter().flatten())
            .filter_map(|note| note.cross_staff.as_ref().map(|cross| cross.target_staff))
            .collect::<Vec<_>>()
    };
    assert!(!crossings(&parsed).is_empty());
    let export = acorde_io::serialize_mei_with_report(&parsed).expect("MEI export");
    assert!(export.diagnostics.is_empty(), "{:?}", export.diagnostics);
    let restored = acorde_io::parse_mei(&export.output).expect("MEI reparse");
    assert_eq!(crossings(&restored), crossings(&parsed));
}

#[cfg(feature = "musicxml")]
#[test]
fn musicxml_explicit_beams_import_and_round_trip() {
    use acorde_core::BeamState;
    // One beam over all six eighths of a 6/8 bar, which default beat beaming would split 3+3.
    let note = |step: &str, beam: &str| {
        format!(
            "<note><pitch><step>{step}</step><octave>5</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type><beam number=\"1\">{beam}</beam></note>"
        )
    };
    let notes = [
        note("C", "begin"),
        note("D", "continue"),
        note("E", "continue"),
        note("F", "continue"),
        note("G", "continue"),
        note("A", "end"),
    ]
    .concat();
    let xml = format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Flute</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>2</divisions><time><beats>6</beats><beat-type>8</beat-type></time></attributes>{notes}</measure></part></score-partwise>"#
    );
    let expected = [
        BeamState::Begin,
        BeamState::Continue,
        BeamState::Continue,
        BeamState::Continue,
        BeamState::Continue,
        BeamState::End,
    ];
    let beams = |score: &acorde_core::Score| {
        score.parts[0].staves[0].measures[0].voices[0]
            .iter()
            .map(|note| note.beam)
            .collect::<Vec<_>>()
    };
    let parsed = parse_musicxml(&xml).expect("beamed MusicXML parses");
    assert_eq!(beams(&parsed), expected);
    let written = acorde_io::serialize_musicxml(&parsed).expect("beams serialize");
    assert_eq!(
        beams(&parse_musicxml(&written).expect("beams reparse")),
        expected
    );
}

#[cfg(feature = "musicxml")]
#[test]
fn utf16_musicxml_decodes_and_parses() {
    let xml = "<?xml version='1.0' encoding='UTF-16'?><score-partwise><part-list><score-part id=\"P1\"><part-name>Cantus</part-name></score-part></part-list><part id=\"P1\"><measure number=\"1\"><attributes><divisions>1</divisions></attributes><note><pitch><step>G</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure></part></score-partwise>";
    let units = xml.encode_utf16().collect::<Vec<_>>();
    let mut little = vec![0xFF, 0xFE];
    little.extend(units.iter().flat_map(|unit| unit.to_le_bytes()));
    let mut big = vec![0xFE, 0xFF];
    big.extend(units.iter().flat_map(|unit| unit.to_be_bytes()));
    let no_bom = units
        .iter()
        .flat_map(|unit| unit.to_le_bytes())
        .collect::<Vec<_>>();
    let mut utf8_bom = vec![0xEF, 0xBB, 0xBF];
    utf8_bom.extend(xml.as_bytes());
    for bytes in [little, big, no_bom, utf8_bom] {
        let text = acorde_io::decode_xml_text(&bytes).expect("decodes");
        let score = parse_musicxml(&text).expect("decoded MusicXML parses");
        assert_eq!(score.parts[0].staves[0].measures[0].voices[0].len(), 1);
    }
    assert!(acorde_io::decode_xml_text(&[0xFF, 0xFE, 0x3C]).is_err());
}

#[cfg(feature = "musicxml")]
#[test]
fn musicxml_cue_notes_keep_timing_on_import_and_export() {
    // A part's cue passage in voice 1 over the rest of voice 2, as orchestral parts write it: the
    // cue notes take time, so the <backup> after them is only valid if the reader counts it.
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Oboe</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><note><cue/><pitch><step>C</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note><note><pitch><step>E</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note><backup><duration>4</duration></backup><note><rest/><duration>4</duration><voice>2</voice><type>whole</type></note></measure></part></score-partwise>"#;
    let check = |score: &acorde_core::Score| {
        let voices = &score.parts[0].staves[0].measures[0].voices;
        assert!(voices[0][0].is_cue);
        // The real note keeps its beat-3 position behind a half rest standing in for the cue.
        assert!(voices[0][1].is_rest);
        assert_eq!(voices[0][1].duration, acorde_core::Duration::Half);
        assert_eq!(voices[0][2].pitches[0].step, acorde_core::Step::E);
        assert_eq!(voices[1].len(), 1);
        assert!(acorde_core::validate(score).errors.is_empty());
    };
    let report = acorde_io::parse_musicxml_with_report(xml).expect("cue MusicXML parses");
    assert!(
        report
            .diagnostics
            .iter()
            .all(|d| d.code != "musicxml.backup-underflow"),
        "{:?}",
        report.diagnostics
    );
    check(&report.score);
    let written = acorde_io::serialize_musicxml(&report.score).expect("cue MusicXML serializes");
    assert!(!written.contains("<duration>0</duration>"));
    assert!(written.contains("<backup><duration>960</duration></backup>"));
    check(&parse_musicxml(&written).expect("cue MusicXML reparses"));
}

#[cfg(feature = "musicxml")]
#[test]
fn musicxml_partial_chord_ties_keep_their_pitch() {
    // Only the C of the C-E chord is tied into the next bar.
    let note = |step: &str, chord: bool, tie: &str| {
        format!(
            "<note>{}<pitch><step>{step}</step><octave>4</octave></pitch><duration>4</duration>{tie}<voice>1</voice><type>whole</type></note>",
            if chord { "<chord/>" } else { "" }
        )
    };
    let xml = format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>{}{}</measure><measure number="2">{}</measure></part></score-partwise>"#,
        note("C", false, r#"<tie type="start"/>"#),
        note("E", true, ""),
        note("C", false, r#"<tie type="stop"/>"#),
    );
    let check = |score: &acorde_core::Score| {
        let chord = &score.parts[0].staves[0].measures[0].voices[0][0];
        assert!(chord.tie_start);
        assert!(chord.pitch_tie_start(0));
        assert!(!chord.pitch_tie_start(1), "E is not tied");
        assert!(score.parts[0].staves[0].measures[1].voices[0][0].tie_end);
    };
    let report = acorde_io::parse_musicxml_with_report(&xml).expect("parses");
    check(&report.score);
    let written = acorde_io::serialize_musicxml(&report.score).expect("exports");
    assert_eq!(
        written.matches(r#"<tie type="start"/>"#).count(),
        1,
        "{written}"
    );
    check(&acorde_io::parse_musicxml(&written).expect("reparses"));
    // A fully tied chord stays in the chord-level form.
    let full = xml.replace(
        r#"<duration>4</duration><voice>1</voice><type>whole</type></note></measure>"#,
        r#"<duration>4</duration><tie type="start"/><voice>1</voice><type>whole</type></note></measure>"#,
    );
    let score = acorde_io::parse_musicxml(&full).expect("parses");
    let chord = &score.parts[0].staves[0].measures[0].voices[0][0];
    assert!(chord.tie_start && chord.pitch_tie_starts.is_empty());
}

#[cfg(all(feature = "musicxml", feature = "mei"))]
#[test]
fn mei_partial_chord_ties_round_trip_per_note() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><tie type="start"/><voice>1</voice><type>whole</type></note><note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note></measure><measure number="2"><note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><tie type="stop"/><voice>1</voice><type>whole</type></note></measure></part></score-partwise>"#;
    let score = acorde_io::parse_musicxml(xml).expect("parses");
    let mei = acorde_io::serialize_mei(&score).expect("MEI exports");
    // The tie sits on the C inside the chord, not on the whole chord.
    assert!(mei.contains(r#"pname="c" oct="4" tie="i""#), "{mei}");
    assert!(!mei.contains(r#"<chord xml:id="n1_1_1_1" dur="1" tie"#));
    let back = acorde_io::parse_mei(&mei).expect("MEI reparses");
    let chord = &back.parts[0].staves[0].measures[0].voices[0][0];
    assert_eq!(chord.pitch_tie_starts, vec![true, false]);
}

#[cfg(feature = "musicxml")]
#[test]
fn musicxml_voice_written_wholly_on_the_other_staff_belongs_to_that_staff() {
    // Finale-style piano: the left hand is voice 3 but sits on staff 2 for the whole bar.
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Pno</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time><staves>2</staves><clef number="1"><sign>G</sign><line>2</line></clef><clef number="2"><sign>F</sign><line>4</line></clef></attributes><note><pitch><step>C</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>half</type><staff>1</staff></note><backup><duration>2</duration></backup><note><pitch><step>C</step><octave>3</octave></pitch><duration>2</duration><voice>3</voice><type>half</type><staff>2</staff></note></measure></part></score-partwise>"#;
    let score = parse_musicxml(xml).expect("parses");
    let staves = &score.parts[0].staves;
    let left_hand: Vec<_> = staves[1].measures[0].voices.iter().flatten().collect();
    assert_eq!(left_hand.len(), 1, "voice 3 lands on staff 2");
    assert!(left_hand[0].cross_staff.is_none());
    assert!(
        staves[0].measures[0]
            .voices
            .iter()
            .flatten()
            .all(|note| note.cross_staff.is_none())
    );
}

#[cfg(feature = "musicxml")]
#[test]
fn musicxml_clef_changes_on_a_later_staff_are_kept() {
    // Staff 2 starts in bass clef, switches to treble at the end of bar 1 (a cue for bar 2) and
    // back to bass at the start of bar 3; staff 1 reads a C clef on line 4 (tenor).
    let note = |step: &str, octave: u8, staff: u8| {
        format!(
            "<note><pitch><step>{step}</step><octave>{octave}</octave></pitch><duration>2</duration><voice>{}</voice><type>half</type><staff>{staff}</staff></note>",
            if staff == 1 { 1 } else { 5 }
        )
    };
    let bar = |number: u8, lead: &str, tail: &str, low: &str| {
        format!(
            "<measure number=\"{number}\">{lead}{}<backup><duration>2</duration></backup>{}{tail}</measure>",
            note("D", 4, 1),
            note(low, 3, 2)
        )
    };
    let xml = format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Pno</part-name></score-part></part-list><part id="P1">{}{}{}</part></score-partwise>"#,
        bar(
            1,
            r#"<attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time><staves>2</staves><clef number="1"><sign>C</sign><line>4</line></clef><clef number="2"><sign>F</sign><line>4</line></clef></attributes>"#,
            r#"<attributes><clef number="2"><sign>G</sign><line>2</line></clef></attributes>"#,
            "C"
        ),
        bar(2, "", "", "A"),
        bar(
            3,
            r#"<attributes><clef number="2"><sign>F</sign><line>4</line></clef></attributes>"#,
            "",
            "C"
        ),
    );
    let score = parse_musicxml(&xml).expect("parses");
    let staves = &score.parts[0].staves;
    assert_eq!(staves[0].clef, acorde_core::Clef::Tenor);
    assert_eq!(staves[1].clef, acorde_core::Clef::Bass);
    assert_eq!(staves[1].measures[0].clef, None);
    assert_eq!(staves[1].measures[1].clef, Some(acorde_core::Clef::Treble));
    assert_eq!(staves[1].measures[2].clef, Some(acorde_core::Clef::Bass));
}

#[cfg(feature = "musicxml")]
#[test]
fn musicxml_dynamics_import_and_round_trip() {
    // A direction dynamic on staff 2 waits for that staff's next note; one in <notations>
    // belongs to its own note; other-dynamics has no acorde equivalent and is reported.
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Pno</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time><staves>2</staves><clef number="1"><sign>G</sign><line>2</line></clef><clef number="2"><sign>F</sign><line>4</line></clef></attributes><direction placement="below"><direction-type><dynamics><pp/></dynamics></direction-type><staff>2</staff></direction><direction><direction-type><dynamics><other-dynamics>mfz</other-dynamics></dynamics></direction-type></direction><note><pitch><step>C</step><octave>5</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>1</staff><notations><dynamics><sfz/></dynamics></notations></note><note><pitch><step>D</step><octave>5</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>1</staff></note><backup><duration>2</duration></backup><note><rest/><duration>1</duration><voice>5</voice><type>quarter</type><staff>2</staff></note><note><pitch><step>C</step><octave>3</octave></pitch><duration>1</duration><voice>5</voice><type>quarter</type><staff>2</staff></note></measure></part></score-partwise>"#;
    let report = acorde_io::parse_musicxml_with_report(xml).expect("parses");
    let staves = &report.score.parts[0].staves;
    let upper = &staves[0].measures[0].voices[0];
    assert_eq!(upper[0].dynamic, Some(acorde_core::Dynamic::Sfz));
    assert_eq!(upper[1].dynamic, None);
    let lower: Vec<_> = staves[1].measures[0].voices.iter().flatten().collect();
    assert!(lower[0].is_rest && lower[0].dynamic.is_none());
    assert_eq!(lower[1].dynamic, Some(acorde_core::Dynamic::Pp));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "musicxml.unsupported-dynamic")
    );
    let written = serialize_musicxml(&report.score).expect("exports");
    let back = parse_musicxml(&written).expect("reparses");
    assert_eq!(
        back.parts[0].staves[0].measures[0].voices[0][0].dynamic,
        Some(acorde_core::Dynamic::Sfz)
    );
    let back_lower: Vec<_> = back.parts[0].staves[1].measures[0]
        .voices
        .iter()
        .flatten()
        .collect();
    assert_eq!(back_lower[1].dynamic, Some(acorde_core::Dynamic::Pp));
}

#[test]
fn musicxml_mid_bar_clef_changes_import_and_round_trip() {
    use acorde_core::{Clef, MeasureLength, MidMeasureClef};
    // Bar 1: staff 1 changes to bass after two quarters; staff 2 to treble after one. A clef
    // after staff 2's last note of bar 1 (a courtesy clef) begins bar 2.
    let note = |step: &str, octave: u8, voice: u8, staff: u8| {
        format!(
            "<note><pitch><step>{step}</step><octave>{octave}</octave></pitch><duration>1</duration><voice>{voice}</voice><type>quarter</type><staff>{staff}</staff></note>"
        )
    };
    let clef = |number: u8, sign: &str, line: u8| {
        format!(
            "<attributes><clef number=\"{number}\"><sign>{sign}</sign><line>{line}</line></clef></attributes>"
        )
    };
    let bar1 = [
        note("C", 5, 1, 1),
        note("D", 5, 1, 1),
        clef(1, "F", 4),
        note("E", 3, 1, 1),
        note("F", 3, 1, 1),
        "<backup><duration>4</duration></backup>".to_string(),
        note("C", 3, 5, 2),
        clef(2, "G", 2),
        note("D", 4, 5, 2),
        note("E", 4, 5, 2),
        note("F", 4, 5, 2),
        clef(2, "C", 3),
    ]
    .concat();
    let bar2 = [
        note("G", 3, 1, 1),
        note("A", 3, 1, 1),
        note("B", 3, 1, 1),
        note("C", 4, 1, 1),
        "<backup><duration>4</duration></backup>".to_string(),
        note("C", 4, 5, 2),
        note("D", 4, 5, 2),
        note("E", 4, 5, 2),
        note("F", 4, 5, 2),
    ]
    .concat();
    let xml = format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Pno</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><staves>2</staves><clef number="1"><sign>G</sign><line>2</line></clef><clef number="2"><sign>F</sign><line>4</line></clef></attributes>{bar1}</measure><measure number="2">{bar2}</measure></part></score-partwise>"#
    );
    let change = |beats: f64, clef: Clef| MidMeasureClef {
        offset: MeasureLength::from_beats(beats).unwrap(),
        clef,
    };
    let check = |score: &acorde_core::Score| {
        let staves = &score.parts[0].staves;
        assert_eq!(staves[0].clef, Clef::Treble);
        assert_eq!(
            staves[0].measures[0].mid_clefs,
            vec![change(2.0, Clef::Bass)]
        );
        assert_ne!(staves[0].measures[0].clef, Some(Clef::Bass));
        assert_eq!(staves[1].clef, Clef::Bass);
        assert_eq!(
            staves[1].measures[0].mid_clefs,
            vec![change(1.0, Clef::Treble)]
        );
        assert_eq!(staves[1].measures[1].clef, Some(Clef::Alto));
        assert!(acorde_core::validate(score).errors.is_empty());
    };
    let score = parse_musicxml(&xml).expect("parses");
    check(&score);
    let written = serialize_musicxml(&score).expect("exports");
    check(&parse_musicxml(&written).expect("reparses"));
}

#[cfg(all(feature = "mei", feature = "mscz"))]
#[test]
fn mei_and_mscx_keep_mid_bar_clef_changes() {
    use acorde_core::{
        Clef, Duration, Measure, MeasureLength, MidMeasureClef, Note, Part, Pitch, Score, Staff,
        Step,
    };
    let change = |beats: f64, clef: Clef| MidMeasureClef {
        offset: MeasureLength::from_beats(beats).unwrap(),
        clef,
    };
    let mut score = Score::new("clefs", 120, 4, 4, 0, 2);
    let mut part = Part::new("Vc.", "");
    let mut staff = Staff::new(Clef::Bass);
    for index in 0..2 {
        let mut measure = Measure::empty(4, 4);
        measure.number = index + 1;
        measure.voices[0] = [(Step::C, 3), (Step::E, 3), (Step::C, 4), (Step::E, 4)]
            .into_iter()
            .map(|(step, octave)| Note::new(Pitch::new(step, octave), Duration::Quarter))
            .collect();
        staff.measures.push(measure);
    }
    staff.measures[0].mid_clefs = vec![change(2.0, Clef::Tenor), change(3.0, Clef::Treble)];
    staff.measures[1].clef = Some(Clef::Bass);
    staff.measures[1].mid_clefs = vec![change(1.0, Clef::Alto)];
    part.staves = vec![staff];
    score.parts = vec![part];
    assert!(acorde_core::validate(&score).errors.is_empty());
    let expected: Vec<_> = score.parts[0].staves[0]
        .measures
        .iter()
        .map(|measure| (measure.clef.clone(), measure.mid_clefs.clone()))
        .collect();
    let summary = |score: &Score| -> Vec<_> {
        score.parts[0].staves[0]
            .measures
            .iter()
            .map(|measure| (measure.clef.clone(), measure.mid_clefs.clone()))
            .collect()
    };
    let mei = acorde_io::serialize_mei(&score).expect("MEI export");
    assert_eq!(
        summary(&acorde_io::parse_mei(&mei).expect("MEI import")),
        expected
    );
    let mscx = acorde_io::serialize_mscx(&score).expect("MSCX export");
    let from_mscx = acorde_io::parse_mscx(&mscx).expect("MSCX import");
    assert_eq!(summary(&from_mscx)[0].1, expected[0].1);
    assert_eq!(summary(&from_mscx)[1], expected[1]);
}

#[test]
fn lyric_extenders_and_compound_dynamics_survive_interchange() {
    use acorde_core::{Duration, Dynamic, Lyric, Note, Pitch, Score, Step};
    let mut score = Score::new("melisma", 120, 4, 4, 0, 1);
    let mut notes: Vec<Note> = [Step::C, Step::D, Step::E, Step::F]
        .into_iter()
        .map(|step| Note::new(Pitch::new(step, 4), Duration::Quarter))
        .collect();
    notes[0].lyric = Some(Lyric {
        text: "Ah".into(),
        syllabic: "single".into(),
        extend: true,
    });
    notes[0].dynamic = Some(Dynamic::Fp);
    notes[3].lyric = Some(Lyric {
        text: "men".into(),
        syllabic: "single".into(),
        extend: false,
    });
    notes[3].dynamic = Some(Dynamic::Sfp);
    score.parts[0].staves[0].measures[0].voices[0] = notes;
    let check = |score: &Score, format: &str| {
        let voice = &score.parts[0].staves[0].measures[0].voices[0];
        assert!(
            voice[0].lyric.as_ref().is_some_and(|l| l.extend),
            "{format}"
        );
        assert!(
            voice[3].lyric.as_ref().is_some_and(|l| !l.extend),
            "{format}"
        );
        assert_eq!(voice[0].dynamic, Some(Dynamic::Fp), "{format}");
        assert_eq!(voice[3].dynamic, Some(Dynamic::Sfp), "{format}");
    };
    let xml = serialize_musicxml(&score).expect("MusicXML export");
    assert!(xml.contains("<extend/>"));
    check(&parse_musicxml(&xml).expect("MusicXML import"), "MusicXML");
    #[cfg(feature = "mei")]
    check(
        &acorde_io::parse_mei(&acorde_io::serialize_mei(&score).unwrap()).unwrap(),
        "MEI",
    );
    #[cfg(feature = "mscz")]
    {
        let mscx = acorde_io::serialize_mscx(&score).unwrap();
        assert!(mscx.contains("<ticks_f>3/4</ticks_f>"));
        check(&acorde_io::parse_mscx(&mscx).unwrap(), "MSCX");
    }
}

#[test]
fn musicxml_lyric_elision_keeps_both_syllables() {
    let xml = SIMPLE_XML.replacen(
        "<pitch><step>C</step><octave>4</octave></pitch>",
        "<pitch><step>C</step><octave>4</octave></pitch><lyric number=\"1\"><syllabic>single</syllabic><text>se</text><elision/><syllabic>begin</syllabic><text>a</text></lyric>",
        1,
    );
    assert!(xml.contains("<elision/>"));
    let report = acorde_io::parse_musicxml_with_report(&xml).expect("parses");
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "musicxml.unsupported-element.elision")
    );
    let check = |score: &acorde_core::Score| {
        let lyric = score.parts[0].staves[0].measures[0].voices[0][0]
            .lyric
            .clone()
            .expect("lyric");
        assert_eq!(lyric.text, "se\u{203F}a");
        assert_eq!(lyric.syllabic, "begin");
    };
    check(&report.score);
    let written = serialize_musicxml(&report.score).expect("exports");
    assert!(written.contains("<elision>"));
    check(&parse_musicxml(&written).expect("reparses"));
}

#[test]
fn typed_direction_spanners_keep_their_end_note_and_reach_other_exporters() {
    use acorde_core::{
        Duration, NotationSpanner, NotationSpannerKind, Note, NoteAddr, Pitch, Score, Step,
    };
    let mut score = Score::new("spans", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] = (0..4)
        .map(|_| Note::new(Pitch::new(Step::C, 5), Duration::Quarter))
        .collect();
    let address = |note| NoteAddr {
        part: 0,
        staff: 0,
        measure: 0,
        voice: 0,
        note,
    };
    for (kind, id) in [
        (NotationSpannerKind::Ottava, "ottava"),
        (NotationSpannerKind::Pedal, "pedal"),
        (NotationSpannerKind::Slur, "slur"),
    ] {
        score.spanners.push(NotationSpanner {
            id: id.into(),
            kind,
            start: address(0),
            end: address(2),
            number: Some(1),
            line_type: None,
            text: None,
            placement: None,
            ottava_size: Some(8),
            ottava_type: Some("down".into()),
        });
    }
    // MusicXML writes a direction's stop after its last note, so the span does not shrink.
    let back = parse_musicxml(&serialize_musicxml(&score).expect("exports")).expect("imports");
    for spanner in &back.spanners {
        assert_eq!(
            (spanner.start.note, spanner.end.note),
            (0, 2),
            "{:?}",
            spanner.kind
        );
    }
    // Spans held only as typed spanners are not dropped by exporters reading note flags.
    let flagged = score.with_legacy_spanner_flags();
    let notes = &flagged.parts[0].staves[0].measures[0].voices[0];
    assert!(notes[0].slur_start && notes[2].slur_end && notes[0].pedal_start);
    assert!(notes[0].ottava_start.is_some() && notes[2].ottava_end);
    #[cfg(feature = "mei")]
    {
        let mei = acorde_io::serialize_mei(&score).expect("MEI export");
        assert!(mei.contains("<slur") && mei.contains("<pedal") && mei.contains("<octave"));
    }
    #[cfg(feature = "mscz")]
    {
        let report = acorde_io::serialize_mscx_with_report(&score).expect("MSCX export");
        assert!(
            report.output.contains("<Spanner type=\"Slur\">")
                && report.output.contains("<Spanner type=\"Pedal\">")
                && report.output.contains("<Spanner type=\"Ottava\">"),
            "typed-only spans are written as MuseScore spanners"
        );
    }
}

#[test]
fn musicxml_hairpins_stay_on_their_staff_and_voice() {
    // A left-hand hairpin: the wedge directions name staff 2, and the stop comes after the
    // right hand's notes in the file.
    let note = |step: &str, octave: u8, voice: u8, staff: u8| {
        format!(
            "<note><pitch><step>{step}</step><octave>{octave}</octave></pitch><duration>1</duration><voice>{voice}</voice><type>quarter</type><staff>{staff}</staff></note>"
        )
    };
    let wedge = |kind: &str, staff: u8| {
        format!(
            "<direction placement=\"below\"><direction-type><wedge type=\"{kind}\"/></direction-type><staff>{staff}</staff></direction>"
        )
    };
    let body = [
        note("C", 5, 1, 1),
        note("D", 5, 1, 1),
        "<backup><duration>2</duration></backup>".to_string(),
        wedge("crescendo", 2),
        note("C", 3, 5, 2),
        note("D", 3, 5, 2),
        wedge("stop", 2),
    ]
    .concat();
    let xml = format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Pno</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time><staves>2</staves><clef number="1"><sign>G</sign><line>2</line></clef><clef number="2"><sign>F</sign><line>4</line></clef></attributes>{body}</measure></part></score-partwise>"#
    );
    let check = |score: &acorde_core::Score| {
        let upper = &score.parts[0].staves[0].measures[0].voices;
        assert!(
            upper
                .iter()
                .flatten()
                .all(|n| n.hairpin_start.is_none() && !n.hairpin_end)
        );
        let lower: Vec<_> = score.parts[0].staves[1].measures[0]
            .voices
            .iter()
            .flatten()
            .collect();
        assert!(lower[0].hairpin_start.is_some());
        assert!(lower[1].hairpin_end);
    };
    let score = parse_musicxml(&xml).expect("parses");
    check(&score);
    check(&parse_musicxml(&serialize_musicxml(&score).unwrap()).unwrap());
}

#[test]
fn musicxml_left_hand_pedal_ends_on_its_own_staff() {
    let note = |step: &str, octave: u8, voice: u8, staff: u8| {
        format!(
            "<note><pitch><step>{step}</step><octave>{octave}</octave></pitch><duration>1</duration><voice>{voice}</voice><type>quarter</type><staff>{staff}</staff></note>"
        )
    };
    let pedal = |kind: &str| {
        format!(
            "<direction placement=\"below\"><direction-type><pedal type=\"{kind}\" line=\"yes\"/></direction-type><staff>2</staff></direction>"
        )
    };
    let body = [
        note("C", 5, 1, 1),
        note("D", 5, 1, 1),
        "<backup><duration>2</duration></backup>".to_string(),
        pedal("start"),
        note("C", 3, 5, 2),
        note("D", 3, 5, 2),
        pedal("stop"),
    ]
    .concat();
    let xml = format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Pno</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time><staves>2</staves><clef number="1"><sign>G</sign><line>2</line></clef><clef number="2"><sign>F</sign><line>4</line></clef></attributes>{body}</measure></part></score-partwise>"#
    );
    let score = parse_musicxml(&xml).expect("parses");
    let upper: Vec<_> = score.parts[0].staves[0].measures[0]
        .voices
        .iter()
        .flatten()
        .collect();
    assert!(upper.iter().all(|n| !n.pedal_start && !n.pedal_end));
    let lower: Vec<_> = score.parts[0].staves[1].measures[0]
        .voices
        .iter()
        .flatten()
        .collect();
    assert!(lower[0].pedal_start && lower[1].pedal_end);
}

#[test]
fn musicxml_octave_shift_down_is_an_8va() {
    // MusicXML pitches sound; `down` means they are displayed an octave lower: 8va.
    let xml = SIMPLE_XML.replacen(
        "<note>",
        "<direction placement=\"above\"><direction-type><octave-shift type=\"down\" size=\"8\"/></direction-type></direction><note>",
        1,
    );
    let score = parse_musicxml(&xml).expect("parses");
    let first = &score.parts[0].staves[0].measures[0].voices[0][0];
    assert_eq!(first.ottava_start, Some(acorde_core::OttavaKind::Va8));
    let written = serialize_musicxml(&score).expect("exports");
    assert!(written.contains("<octave-shift type=\"down\" size=\"8\""));
    let back = parse_musicxml(&written).expect("reparses");
    assert_eq!(
        back.parts[0].staves[0].measures[0].voices[0][0].ottava_start,
        Some(acorde_core::OttavaKind::Va8)
    );
}

#[test]
fn musicxml_endings_with_text_and_one_bar_endings_are_imported() {
    let bar = |number: u32, left: &str, right: &str| {
        format!(
            "<measure number=\"{number}\">{left}<note><pitch><step>C</step><octave>5</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note>{right}</measure>"
        )
    };
    let body = [
        bar(1, "", ""),
        bar(
            2,
            "<barline location=\"left\"><ending number=\"1\" type=\"start\">1.</ending></barline>",
            "<barline location=\"right\"><bar-style>light-heavy</bar-style><ending number=\"1\" type=\"stop\"/><repeat direction=\"backward\"/></barline>",
        ),
        bar(
            3,
            "<barline location=\"left\"><ending number=\"2\" type=\"start\">2.</ending></barline>",
            "<barline location=\"right\"><ending number=\"2\" type=\"discontinue\"/></barline>",
        ),
    ]
    .concat();
    let xml = format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>V</part-name></score-part></part-list><part id="P1"><attributes><divisions>1</divisions></attributes>{body}</part></score-partwise>"#
    )
    .replacen(
        "<measure number=\"1\">",
        "<measure number=\"1\"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>",
        1,
    )
    .replacen("<attributes><divisions>1</divisions></attributes><measure", "<measure", 1);
    let score = parse_musicxml(&xml).expect("parses");
    let voltas: Vec<_> = score.parts[0].staves[0]
        .measures
        .iter()
        .map(|measure| measure.volta.as_ref().map(|v| (v.number, v.kind.clone())))
        .collect();
    assert_eq!(voltas[0], None);
    assert_eq!(voltas[1], Some((1, "begin_end".to_string())));
    assert_eq!(voltas[2], Some((2, "begin_end".to_string())));
}

#[test]
fn musicxml_dashes_lines_are_imported_drawn_and_exported() {
    // cresc. - - - from the first note to the second.
    let mut parts = SIMPLE_XML.splitn(3, "</note>");
    let (first, second, rest) = (
        parts.next().unwrap(),
        parts.next().unwrap(),
        parts.next().unwrap_or_default(),
    );
    let xml = format!(
        "{}</note>{}</note><direction><direction-type><dashes type=\"stop\" number=\"1\"/></direction-type></direction>{}",
        first.replacen(
            "<note>",
            "<direction placement=\"below\"><direction-type><words>cresc.</words></direction-type><direction-type><dashes type=\"start\" number=\"1\"/></direction-type></direction><note>",
            1,
        ),
        second,
        rest
    );
    let report = acorde_io::parse_musicxml_with_report(&xml).expect("parses");
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "musicxml.unsupported-element.dashes")
    );
    let dashes = report
        .score
        .spanners
        .iter()
        .find(|spanner| spanner.kind == acorde_core::NotationSpannerKind::Dashes)
        .expect("a dashes spanner");
    assert_eq!((dashes.start.note, dashes.end.note), (0, 1));
    let written = serialize_musicxml(&report.score).expect("exports");
    let back = parse_musicxml(&written).expect("reparses");
    let again = back
        .spanners
        .iter()
        .find(|spanner| spanner.kind == acorde_core::NotationSpannerKind::Dashes)
        .expect("a dashes spanner after a round trip");
    assert_eq!((again.start.note, again.end.note), (0, 1));
}

#[test]
fn musicxml_zero_length_forward_is_not_reported() {
    let xml = SIMPLE_XML.replacen(
        "<note>",
        "<forward><duration>0</duration><voice>2</voice></forward><note>",
        1,
    );
    let report = acorde_io::parse_musicxml_with_report(&xml).expect("parses");
    assert!(
        !report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "musicxml.invalid-numeric-value")
    );
}

#[cfg(feature = "mscz")]
#[test]
fn mscx_exports_slurs_hairpins_pedals_and_ottavas() {
    use acorde_core::{Duration, HairpinKind, Note, OttavaKind, Pitch, Score, Step};
    let mut score = Score::new("lines", 120, 4, 4, 0, 2);
    for measure in &mut score.parts[0].staves[0].measures {
        measure.voices[0] = (0..4)
            .map(|_| Note::new(Pitch::new(Step::G, 4), Duration::Quarter))
            .collect();
    }
    {
        let first = &mut score.parts[0].staves[0].measures[0].voices[0];
        first[0].slur_start = true;
        first[2].slur_end = true;
        first[1].hairpin_start = Some(HairpinKind::Decrescendo);
        first[0].pedal_start = true;
        first[2].ottava_start = Some(OttavaKind::Va8);
    }
    {
        let second = &mut score.parts[0].staves[0].measures[1].voices[0];
        second[1].hairpin_end = true;
        second[3].pedal_end = true;
        second[0].ottava_end = true;
    }
    let report = acorde_io::serialize_mscx_with_report(&score).expect("exports");
    assert!(
        report.diagnostics.is_empty(),
        "spans are exported, not reported: {:?}",
        report.diagnostics
    );
    let mscx = report.output;
    assert!(mscx.contains("<Spanner type=\"Slur\"><Slur></Slur><next><location><fractions>1/2</fractions></location></next></Spanner>"));
    assert!(mscx.contains("<Spanner type=\"HairPin\"><HairPin><subtype>1</subtype></HairPin><next><location><measures>1</measures><fractions>1/4</fractions></location></next></Spanner>"));
    let back = acorde_io::parse_mscx(&mscx).expect("imports");
    let voice = |measure: usize| &back.parts[0].staves[0].measures[measure].voices[0];
    assert!(voice(0)[0].slur_start && voice(0)[2].slur_end);
    assert_eq!(voice(0)[1].hairpin_start, Some(HairpinKind::Decrescendo));
    assert!(voice(1)[1].hairpin_end);
    assert!(voice(0)[0].pedal_start && voice(1)[3].pedal_end);
    assert_eq!(voice(0)[2].ottava_start, Some(OttavaKind::Va8));
    assert!(voice(1)[0].ottava_end);
}

#[cfg(feature = "mscz")]
#[test]
fn mscx_keeps_every_part_of_a_multi_part_score() {
    use acorde_core::{Clef, Duration, Measure, Note, Part, Pitch, Score, Staff, Step};
    let mut score = Score::new("quartet", 120, 4, 4, 0, 1);
    score.parts = [("Vn.", Clef::Treble, Step::E), ("Vc.", Clef::Bass, Step::C)]
        .into_iter()
        .map(|(name, clef, step)| {
            let mut part = Part::new(name, name);
            let mut staff = Staff::new(clef);
            let mut measure = Measure::empty(4, 4);
            measure.voices[0] = vec![Note::new(Pitch::new(step, 3), Duration::Whole)];
            staff.measures.push(measure);
            part.staves = vec![staff];
            part
        })
        .collect();
    let mscx = acorde_io::serialize_mscx(&score).expect("exports");
    assert!(mscx.contains("<Part><Staff id=\"2\"/>"));
    let back = acorde_io::parse_mscx(&mscx).expect("imports");
    assert_eq!(back.parts.len(), 2);
    for (part, step) in back.parts.iter().zip([Step::E, Step::C]) {
        assert_eq!(part.staves[0].measures.len(), 1);
        assert_eq!(
            part.staves[0].measures[0].voices[0][0].pitches[0].step,
            step
        );
    }
}

#[cfg(feature = "mscz")]
#[test]
fn mscx_round_trips_tuplets_graces_fermatas_ornaments_repeats_voltas_and_chords() {
    use acorde_core::{
        Articulation, Barline, ChordSymbol, Duration, Note, Pitch, Score, Step, TupletInfo,
        VoltaBracket,
    };
    let mut score = Score::new("mscx", 120, 2, 4, 0, 3);
    let triplet = TupletInfo {
        actual_notes: 3,
        normal_notes: 2,
    };
    {
        let measures = &mut score.parts[0].staves[0].measures;
        let mut grace = Note::new(Pitch::new(Step::B, 4), Duration::Eighth);
        grace.is_grace = true;
        grace.grace_slash = true;
        let mut notes = vec![grace];
        for step in [Step::C, Step::D, Step::E] {
            let mut note = Note::new(Pitch::new(step, 5), Duration::Eighth);
            note.tuplet = Some(triplet.clone());
            notes.push(note);
        }
        let mut last = Note::new(Pitch::new(Step::F, 5), Duration::Quarter);
        last.articulations = vec![Articulation::Fermata, Articulation::Trill];
        let mut symbol: ChordSymbol =
            serde_json::from_str(r#"{"root":"F","kind":"major-seventh","bass":"A"}"#)
                .expect("chord symbol");
        symbol.placement = None;
        last.chord_symbol = Some(symbol);
        notes.push(last);
        measures[0].voices[0] = notes;
        measures[0].barline_right = Barline::RepeatEnd;
        measures[0].barline_left = Barline::RepeatStart;
        measures[1].voices[0] = vec![Note::new(Pitch::new(Step::G, 4), Duration::Half)];
        measures[1].volta = Some(VoltaBracket {
            number: 1,
            kind: "begin_end".into(),
        });
        measures[2].voices[0] = vec![Note::new(Pitch::new(Step::G, 4), Duration::Half)];
        measures[2].volta = Some(VoltaBracket {
            number: 2,
            kind: "begin_end".into(),
        });
    }
    let report = acorde_io::serialize_mscx_with_report(&score).expect("exports");
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    let back = acorde_io::parse_mscx(&report.output).expect("imports");
    let measures = &back.parts[0].staves[0].measures;
    let first = &measures[0].voices[0];
    assert!(first[0].is_grace && first[0].grace_slash);
    assert!(
        first[1..4]
            .iter()
            .all(|note| note.tuplet == Some(triplet.clone()))
    );
    assert!(first[4].tuplet.is_none());
    assert!(first[4].articulations.contains(&Articulation::Fermata));
    assert!(first[4].articulations.contains(&Articulation::Trill));
    let chord = first[4].chord_symbol.as_ref().expect("chord symbol");
    assert_eq!(
        (chord.root.as_str(), chord.bass.as_deref()),
        ("F", Some("A"))
    );
    assert!(measures[0].texts.is_empty(), "no duplicate chord text");
    assert_eq!(measures[0].barline_left, Barline::RepeatStart);
    assert_eq!(measures[0].barline_right, Barline::RepeatEnd);
    let voltas: Vec<_> = measures
        .iter()
        .map(|measure| measure.volta.as_ref().map(|v| (v.number, v.kind.clone())))
        .collect();
    assert_eq!(
        voltas,
        vec![
            None,
            Some((1, "begin_end".to_string())),
            Some((2, "begin_end".to_string()))
        ]
    );
}

#[cfg(feature = "mei")]
#[test]
fn mei_writes_and_reads_endings() {
    use acorde_core::{Duration, Note, Pitch, Score, Step, VoltaBracket};
    let mut score = Score::new("endings", 120, 2, 4, 0, 4);
    for measure in &mut score.parts[0].staves[0].measures {
        measure.voices[0] = vec![Note::new(Pitch::new(Step::G, 4), Duration::Half)];
    }
    let measures = &mut score.parts[0].staves[0].measures;
    measures[1].volta = Some(VoltaBracket {
        number: 1,
        kind: "begin".into(),
    });
    measures[2].volta = Some(VoltaBracket {
        number: 1,
        kind: "end".into(),
    });
    measures[3].volta = Some(VoltaBracket {
        number: 2,
        kind: "begin_end".into(),
    });
    let mei = acorde_io::serialize_mei(&score).expect("exports");
    assert_eq!(mei.matches("<ending ").count(), 2);
    let back = acorde_io::parse_mei(&mei).expect("imports");
    let voltas: Vec<_> = back.parts[0].staves[0]
        .measures
        .iter()
        .map(|m| m.volta.as_ref().map(|v| (v.number, v.kind.clone())))
        .collect();
    assert_eq!(
        voltas,
        vec![
            None,
            Some((1, "begin".to_string())),
            Some((1, "end".to_string())),
            Some((2, "begin_end".to_string()))
        ]
    );
}

#[test]
fn musicxml_export_writes_the_tempo_once_not_at_every_attributes_change() {
    use acorde_core::{KeySignature, Score};
    let mut score = Score::new("tempo", 96, 4, 4, 0, 3);
    score.parts[0].staves[0].measures[2].key_sig = Some(KeySignature {
        fifths: 2,
        mode: "major".into(),
    });
    let xml = serialize_musicxml(&score).expect("exports");
    assert_eq!(xml.matches("<sound tempo=").count(), 1);
    assert!(xml.contains("<sound tempo=\"96\"/>"));
    assert!(!xml.contains("<metronome>"));
}

#[test]
fn musicxml_writes_an_opening_tempo_mark_once_in_the_first_part() {
    use acorde_core::Score;
    let mut score = Score::new("marked", 120, 4, 4, 0, 1);
    let mut second = acorde_core::Part::new("Cello", "Vc.");
    second.staves = score.parts[0].staves.clone();
    score.parts.push(second);
    for part in &mut score.parts {
        part.staves[0].measures[0].tempo = Some(72);
    }
    let xml = serialize_musicxml(&score).expect("exports");
    assert_eq!(xml.matches("<metronome>").count(), 1);
    assert_eq!(xml.matches("<sound tempo=").count(), 1);
    let back = parse_musicxml(&xml).expect("imports");
    assert_eq!(back.parts[0].staves[0].measures[0].tempo, Some(72));
}

#[test]
fn mei_round_trips_tremolos_arpeggios_and_tablature_graces_and_lyrics() {
    use acorde_core::{
        Articulation, Duration, Lyric, Note, Pitch, Score, Step, TabPosition, TablatureConfig,
    };
    let mut score = Score::new("mei marks", 120, 4, 4, 0, 1);
    let mut guitar = acorde_core::Part::new("Guitar", "Gtr.");
    guitar.staves = score.parts[0].staves.clone();
    score.parts.push(guitar);
    let mut tremolo = Note::new(Pitch::new(Step::C, 5), Duration::Half);
    tremolo.articulations.push(Articulation::Tremolo(3));
    let mut arpeggio = Note::new(Pitch::new(Step::E, 4), Duration::Half);
    arpeggio.pitches.push(Pitch::new(Step::G, 4));
    arpeggio.arpeggiate = Some(false);
    score.parts[0].staves[0].measures[0].voices[0] = vec![tremolo, arpeggio];

    let staff = &mut score.parts[1].staves[0];
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![40, 45, 50, 55, 59, 64],
        capo: 0,
    });
    let mut grace = Note::new(Pitch::new(Step::A, 3), Duration::Eighth);
    grace.is_grace = true;
    grace.tab_position = Some(TabPosition { string: 4, fret: 2 });
    let mut sung = Note::new(Pitch::new(Step::B, 3), Duration::Whole);
    sung.tab_position = Some(TabPosition { string: 4, fret: 4 });
    sung.lyric = Some(Lyric {
        text: "la".into(),
        syllabic: "single".into(),
        extend: false,
    });
    sung.articulations.push(Articulation::Tremolo(2));
    sung.arpeggiate = Some(true);
    staff.measures[0].voices[0] = vec![grace, sung];

    let mei = acorde_io::serialize_mei(&score).expect("exports");
    assert!(mei.contains("stem.mod=\"3slash\""));
    assert!(mei.contains("order=\"down\""));
    let back = acorde_io::parse_mei(&mei).expect("imports");
    let notes = &back.parts[0].staves[0].measures[0].voices[0];
    assert!(notes[0].articulations.contains(&Articulation::Tremolo(3)));
    assert_eq!(notes[1].arpeggiate, Some(false));
    let tab = &back.parts[1].staves[0].measures[0].voices[0];
    assert!(tab[0].is_grace);
    assert_eq!(tab[1].lyric.as_ref().map(|l| l.text.as_str()), Some("la"));
    assert!(tab[1].articulations.contains(&Articulation::Tremolo(2)));
    assert_eq!(tab[1].arpeggiate, Some(true));
}

#[test]
fn musicxml_reads_final_double_dashed_and_invisible_barlines() {
    use acorde_core::{Barline, Score};
    let mut score = Score::new("barlines", 120, 4, 4, 0, 5);
    let mut piano = acorde_core::Part::new("Piano", "Pno.");
    piano.staves = vec![score.parts[0].staves[0].clone(); 2];
    score.parts.push(piano);
    for part in &mut score.parts {
        for staff in &mut part.staves {
            staff.measures[0].barline_right = Barline::Double;
            staff.measures[1].barline_right = Barline::Dashed;
            staff.measures[2].barline_right = Barline::Invisible;
            staff.measures[3].barline_right = Barline::RepeatEnd;
            staff.measures[4].barline_right = Barline::Final;
        }
    }
    let xml = serialize_musicxml(&score).expect("exports");
    let back = parse_musicxml(&xml).expect("imports");
    for part in &back.parts {
        for staff in &part.staves {
            let bars: Vec<_> = staff
                .measures
                .iter()
                .map(|m| m.barline_right.clone())
                .collect();
            assert_eq!(
                bars,
                vec![
                    Barline::Double,
                    Barline::Dashed,
                    Barline::Invisible,
                    Barline::RepeatEnd,
                    Barline::Final
                ]
            );
        }
    }
}

#[test]
fn mscx_keeps_invisible_barlines_and_a_volta_that_runs_to_the_last_bar() {
    use acorde_core::{Barline, Duration, Note, Pitch, Score, Step, VoltaBracket};
    let mut score = Score::new("coda volta", 120, 2, 4, 0, 4);
    for measure in &mut score.parts[0].staves[0].measures {
        measure.voices[0] = vec![Note::new(Pitch::new(Step::A, 4), Duration::Half)];
    }
    let measures = &mut score.parts[0].staves[0].measures;
    measures[0].barline_right = Barline::Invisible;
    measures[1].barline_right = Barline::RepeatEnd;
    measures[2].volta = Some(VoltaBracket {
        number: 2,
        kind: "begin".into(),
    });
    measures[3].volta = Some(VoltaBracket {
        number: 2,
        kind: "end".into(),
    });
    measures[3].barline_right = Barline::Final;
    let mscx = acorde_io::serialize_mscx(&score).expect("exports");
    let back = acorde_io::parse_mscx(&mscx).expect("imports");
    let measures = &back.parts[0].staves[0].measures;
    assert_eq!(measures[0].barline_right, Barline::Invisible);
    assert_eq!(
        measures
            .iter()
            .map(|m| m.volta.as_ref().map(|v| (v.number, v.kind.clone())))
            .collect::<Vec<_>>(),
        vec![
            None,
            None,
            Some((2, "begin".to_string())),
            Some((2, "end".to_string()))
        ]
    );
}

#[test]
fn mscx_round_trips_cross_staff_chords_as_staff_moves() {
    use acorde_core::{CrossStaff, Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("cross", 120, 4, 4, 0, 1);
    let mut piano = acorde_core::Part::new("Piano", "Pno.");
    piano.staves = vec![score.parts[0].staves[0].clone(); 2];
    score.parts.push(piano);
    let mut down = Note::new(Pitch::new(Step::C, 3), Duration::Whole);
    down.cross_staff = Some(CrossStaff {
        target_staff: 1,
        target_voice: None,
    });
    score.parts[1].staves[0].measures[0].voices[0] = vec![down];
    let mut up = Note::new(Pitch::new(Step::G, 4), Duration::Whole);
    up.cross_staff = Some(CrossStaff {
        target_staff: 0,
        target_voice: None,
    });
    score.parts[1].staves[1].measures[0].voices[0] = vec![up];
    let mscx = acorde_io::serialize_mscx(&score).expect("exports");
    assert!(mscx.contains("<staffMove>1</staffMove>"));
    assert!(mscx.contains("<staffMove>-1</staffMove>"));
    let back = acorde_io::parse_mscx(&mscx).expect("imports");
    let staves = &back.parts[1].staves;
    assert_eq!(
        staves[0].measures[0].voices[0][0].cross_staff,
        Some(CrossStaff {
            target_staff: 1,
            target_voice: None
        })
    );
    assert_eq!(
        staves[1].measures[0].voices[0][0].cross_staff,
        Some(CrossStaff {
            target_staff: 0,
            target_voice: None
        })
    );
}

#[test]
fn mscx_wraps_every_voice_so_second_voices_stay_apart() {
    use acorde_core::{Barline, Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("voices", 120, 4, 4, 0, 2);
    let measures = &mut score.parts[0].staves[0].measures;
    measures[0].voices[0] = vec![Note::new(Pitch::new(Step::E, 5), Duration::Whole)];
    measures[0].voices[1] = vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
    measures[1].voices[0] = vec![Note::new(Pitch::new(Step::D, 5), Duration::Whole)];
    measures[1].voices[2] = vec![Note::new(Pitch::new(Step::G, 3), Duration::Whole)];
    measures[1].barline_right = Barline::Final;
    let mscx = acorde_io::serialize_mscx(&score).expect("exports");
    // MuseScore reads a bar's chords only inside <voice>.
    assert!(!mscx.contains("\"><Chord>"));
    assert!(!mscx.contains("</voice><Chord>"));
    assert!(
        mscx.contains("<BarLine><subtype>end</subtype></BarLine></voice><voice></voice><voice>")
    );
    let back = acorde_io::parse_mscx(&mscx).expect("imports");
    let steps = |m: usize, v: usize| {
        back.parts[0].staves[0].measures[m].voices[v]
            .iter()
            .map(|n| n.pitches[0].step.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(steps(0, 0), vec![Step::E]);
    assert_eq!(steps(0, 1), vec![Step::C]);
    assert_eq!(steps(1, 0), vec![Step::D]);
    assert!(steps(1, 1).is_empty());
    assert_eq!(steps(1, 2), vec![Step::G]);
    assert_eq!(
        back.parts[0].staves[0].measures[1].barline_right,
        Barline::Final
    );
}

#[test]
fn mei_keeps_a_part_whose_key_signature_differs_from_the_others() {
    use acorde_core::{KeySignature, Score};
    let mut score = Score::new("keys", 120, 4, 4, -1, 3);
    let mut second = acorde_core::Part::new("Tenor", "T.");
    second.staves = score.parts[0].staves.clone();
    score.parts.push(second);
    let key = |fifths| {
        Some(KeySignature {
            fifths,
            mode: "major".into(),
        })
    };
    score.parts[0].staves[0].measures[0].key_sig = key(-1);
    score.parts[1].staves[0].measures[0].key_sig = key(0);
    // A later change the parts share is still one score-wide change.
    score.parts[0].staves[0].measures[2].key_sig = key(2);
    score.parts[1].staves[0].measures[2].key_sig = key(2);
    let mei = acorde_io::serialize_mei(&score).expect("exports");
    assert!(mei.contains("<staffDef n=\"2\" keysig=\"0\"/>"));
    assert!(mei.contains("<scoreDef keysig=\"2s\"/>"));
    let back = acorde_io::parse_mei(&mei).expect("imports");
    let keys = |part: usize| {
        let mut running = back.settings.key_signature.fifths;
        back.parts[part].staves[0]
            .measures
            .iter()
            .map(|m| {
                if let Some(key) = &m.key_sig {
                    running = key.fifths;
                }
                running
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(0), vec![-1, -1, 2]);
    assert_eq!(keys(1), vec![0, 0, 2]);
}

#[test]
fn chord_kinds_have_labels_that_mei_reads_back() {
    use acorde_core::{ChordSymbol, Duration, Note, Pitch, Score, Step};
    let kinds = [
        ("augmented-seventh", "Caug7"),
        ("major-minor", "CmMaj7"),
        ("dominant-13th", "C13"),
        ("minor-11th", "Cm11"),
        ("dominant-ninth", "C9"),
        ("major-add9", "Cadd9"),
        ("dominant-sharp-five", "C7#5"),
    ];
    let mut score = Score::new("chords", 120, 4, 4, 0, 7);
    for (measure, (kind, label)) in score.parts[0].staves[0].measures.iter_mut().zip(kinds) {
        let chord: ChordSymbol = serde_json::from_value(serde_json::json!({
            "root": "C",
            "kind": kind,
        }))
        .expect("chord symbol");
        assert_eq!(chord.display_text(), label);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
        note.chord_symbol = Some(chord);
        measure.voices[0] = vec![note];
    }
    let mei = acorde_io::serialize_mei(&score).expect("exports");
    let back = acorde_io::parse_mei(&mei).expect("imports");
    let read: Vec<_> = back.parts[0].staves[0]
        .measures
        .iter()
        .map(|m| m.voices[0][0].chord_symbol.as_ref().map(|c| c.kind.clone()))
        .collect();
    assert_eq!(
        read,
        kinds
            .iter()
            .map(|(kind, _)| Some(kind.to_string()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn hidden_notes_and_rests_round_trip_through_every_notation_format() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("hidden", 120, 4, 4, 0, 2);
    let mut rest = Note::rest(Duration::Half);
    rest.hidden = true;
    let mut note = Note::new(Pitch::new(Step::C, 5), Duration::Quarter);
    note.hidden = true;
    score.parts[0].staves[0].measures[0].voices[0] = vec![
        rest,
        note,
        Note::new(Pitch::new(Step::D, 5), Duration::Quarter),
    ];
    let mut bar_rest = Note::rest(Duration::Whole);
    bar_rest.hidden = true;
    score.parts[0].staves[0].measures[1].voices[0] = vec![bar_rest];
    let hidden = |score: &Score| {
        score.parts[0].staves[0]
            .measures
            .iter()
            .flat_map(|m| m.voices[0].iter().map(|n| n.hidden))
            .collect::<Vec<_>>()
    };
    let expected = vec![true, true, false, true];
    let xml = serialize_musicxml(&score).expect("musicxml");
    assert!(xml.contains("<note print-object=\"no\">"));
    assert_eq!(
        hidden(&parse_musicxml(&xml).expect("musicxml back")),
        expected
    );
    let mei = acorde_io::serialize_mei(&score).expect("mei");
    assert!(mei.contains("<space dur=\"2\""));
    assert_eq!(
        hidden(&acorde_io::parse_mei(&mei).expect("mei back")),
        expected
    );
    let mscx = acorde_io::serialize_mscx(&score).expect("mscx");
    assert_eq!(
        hidden(&acorde_io::parse_mscx(&mscx).expect("mscx back")),
        expected
    );
    let abc = acorde_io::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nx2 c d|X|\n").expect("abc");
    assert_eq!(
        abc.parts[0].staves[0]
            .measures
            .iter()
            .flat_map(|m| m.voices[0].iter().map(|n| (n.is_rest, n.hidden)))
            .collect::<Vec<_>>(),
        vec![(true, true), (false, false), (false, false), (true, true)]
    );
}

#[test]
fn musicxml_forward_gaps_import_as_hidden_rests() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>C</step><octave>5</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note><backup><duration>4</duration></backup><forward><duration>2</duration><voice>2</voice></forward><note><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><voice>2</voice><type>half</type></note></measure></part></score-partwise>"#;
    let score = parse_musicxml(xml).expect("imports");
    let voice = &score.parts[0].staves[0].measures[0].voices[1];
    assert_eq!(voice.len(), 2);
    assert!(voice[0].is_rest && voice[0].hidden);
    assert!(!voice[1].hidden);
}

#[test]
fn a_grace_note_inside_a_triplet_does_not_split_its_bracket() {
    use acorde_core::{Duration, Note, Pitch, Score, Step, TupletInfo};
    let mut score = Score::new("grace triplet", 120, 1, 4, 0, 1);
    let triplet = TupletInfo {
        actual_notes: 3,
        normal_notes: 2,
    };
    let eighth = |step| {
        let mut note = Note::new(Pitch::new(step, 5), Duration::Eighth);
        note.tuplet = Some(triplet.clone());
        note
    };
    let mut grace = eighth(Step::D);
    grace.is_grace = true;
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![eighth(Step::C), grace, eighth(Step::E), eighth(Step::F)];
    let xml = serialize_musicxml(&score).expect("exports");
    assert_eq!(xml.matches("<tuplet type=\"start\"").count(), 1);
    assert_eq!(xml.matches("<tuplet type=\"stop\"").count(), 1);
    let back = parse_musicxml(&xml).expect("imports");
    let timed: Vec<_> = back.parts[0].staves[0].measures[0].voices[0]
        .iter()
        .filter(|n| !n.is_grace)
        .map(|n| n.tuplet.clone())
        .collect();
    assert_eq!(timed, vec![Some(triplet.clone()); 3]);
}

#[test]
fn musicxml_reads_double_dots_and_notes_without_a_type() {
    use acorde_core::Duration;
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>8</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>C</step><octave>5</octave></pitch><duration>14</duration><voice>1</voice><type>half</type><dot/><dot/></note><note><pitch><step>D</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice></note><note><pitch><step>E</step><octave>5</octave></pitch><duration>12</duration><voice>1</voice></note><note><pitch><step>F</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification></note></measure></part></score-partwise>"#;
    let score = parse_musicxml(xml).expect("imports");
    let notes: Vec<_> = score.parts[0].staves[0].measures[0].voices[0]
        .iter()
        .map(|n| (n.duration.clone(), n.dot_count))
        .collect();
    assert_eq!(
        notes,
        vec![
            (Duration::Half, 2),
            (Duration::Sixteenth, 0),
            (Duration::Quarter, 1),
            // Two ticks under 3:2 is a dotted sixteenth triplet.
            (Duration::Sixteenth, 1),
        ]
    );
}

#[test]
fn abc_reads_and_writes_multi_line_tunes_with_chords_dynamics_voices_and_key_changes() {
    use acorde_core::{Clef, HairpinKind};
    let abc = "X:1\nT:Reel\nM:4/4\nL:1/8\nK:D\nV:T1 name=\"Fiddle\"\n!p! \"D\"A>B !<(!c2 !<)!d2 !f!e2|\"G\"g4 \"A7\"a4|\nw: la la la la la la la\nK:G\n[M:3/4]\"Em\"g2 \"^rit.\"a2 b2|]\nV:B clef=bass\nD,8|D,8|G,6|]\n";
    let score = acorde_io::parse_abc(abc).expect("parses");
    let check = |score: &acorde_core::Score, label: &str| {
        assert_eq!(score.parts.len(), 2, "{label}");
        assert_eq!(score.parts[0].name, "Fiddle", "{label}");
        assert_eq!(score.parts[1].staves[0].clef, Clef::Bass, "{label}");
        let measures = &score.parts[0].staves[0].measures;
        assert_eq!(measures.len(), 3, "{label}");
        let first = &measures[0].voices[0];
        assert_eq!(
            first[0].chord_symbol.as_ref().map(|c| c.display_text()),
            Some("D".to_string()),
            "{label}"
        );
        assert!(first[0].dynamic.is_some(), "{label}");
        assert_eq!(first[0].dot_count, 1, "{label}");
        assert_eq!(
            first[2].hairpin_start,
            Some(HairpinKind::Crescendo),
            "{label}"
        );
        assert!(first[3].hairpin_end, "{label}");
        assert_eq!(
            measures[1].voices[0][1]
                .chord_symbol
                .as_ref()
                .map(|c| c.kind.as_str()),
            Some("dominant"),
            "{label}"
        );
        assert_eq!(
            measures[2].key_sig.as_ref().map(|k| k.fifths),
            Some(1),
            "{label}"
        );
        assert_eq!(
            measures[2]
                .time_sig
                .as_ref()
                .map(|t| (t.numerator, t.denominator)),
            Some((3, 4)),
            "{label}"
        );
        assert_eq!(
            first[0].lyric.as_ref().map(|l| l.text.as_str()),
            Some("la"),
            "{label}"
        );
        assert_eq!(score.parts[1].staves[0].measures.len(), 3, "{label}");
    };
    check(&score, "import");
    let text = acorde_io::serialize_abc(&score).expect("exports");
    check(
        &acorde_io::parse_abc(&text).expect("re-imports"),
        "round trip",
    );
}

#[test]
fn abc_keeps_pickups_line_opening_repeats_double_dots_empty_bars_and_final_barlines() {
    use acorde_core::{Barline, Duration};
    let abc = "X:1\nM:4/4\nL:1/4\nK:C\nG|c7/4 d/4 e f|\n|:g4:|\nV:2 clef=bass\nz|C,4|D,4|]\n";
    let score = acorde_io::parse_abc(abc).expect("parses");
    let melody = &score.parts[0].staves[0].measures;
    assert_eq!(melody.len(), 3);
    assert!(
        melody[0].actual_length.is_some(),
        "a short first bar is a pickup"
    );
    assert_eq!(melody[0].voices[0].len(), 1);
    assert_eq!(
        (
            melody[1].voices[0][0].duration.clone(),
            melody[1].voices[0][0].dot_count
        ),
        (Duration::Quarter, 2)
    );
    assert_eq!(melody[2].barline_left, Barline::RepeatStart);
    assert_eq!(melody[2].barline_right, Barline::RepeatEnd);
    assert_eq!(
        score.parts[1].staves[0].measures[2].barline_right,
        Barline::Final
    );

    let mut with_empty = score.clone();
    with_empty.parts[1].staves[0].measures[1].voices[0].clear();
    let text = acorde_io::serialize_abc(&with_empty).expect("exports");
    let back = acorde_io::parse_abc(&text).expect("re-imports");
    assert_eq!(back.parts[0].staves[0].measures.len(), 3, "{text}");
    assert_eq!(back.parts[1].staves[0].measures.len(), 3, "{text}");
    assert_eq!(
        back.parts[0].staves[0].measures[2].barline_left,
        Barline::RepeatStart
    );
    assert_eq!(
        back.parts[0].staves[0].measures[1].voices[0][0].dot_count,
        2
    );
}

#[test]
fn abc_voice_overlays_keep_second_voices() {
    let abc = "X:1\nM:4/4\nL:1/4\nK:C\ne f g a & c4|b4 & x2 G2|\n";
    let score = acorde_io::parse_abc(abc).expect("parses");
    let measures = &score.parts[0].staves[0].measures;
    assert_eq!(measures.len(), 2);
    assert_eq!(measures[0].voices[0].len(), 4);
    assert_eq!(measures[0].voices[1].len(), 1);
    assert_eq!(measures[1].voices[1].len(), 2);
    assert!(measures[1].voices[1][0].hidden);
    let text = acorde_io::serialize_abc(&score).expect("exports");
    let back = acorde_io::parse_abc(&text).expect("re-imports");
    let counts = |s: &acorde_core::Score| {
        s.parts[0].staves[0]
            .measures
            .iter()
            .map(|m| (m.voices[0].len(), m.voices[1].len()))
            .collect::<Vec<_>>()
    };
    assert_eq!(counts(&back), counts(&score), "{text}");
}

#[test]
fn midi_places_every_bar_at_its_own_start_and_writes_key_signatures() {
    use acorde_core::{Duration, KeySignature, Note, Pitch, Score, Step, TimeSignature};
    let mut score = Score::new("bars", 120, 3, 4, 0, 3);
    let staff = &mut score.parts[0].staves[0];
    // Voice 2 is empty in bar 1 and plays in bar 3; bar 2 is in 2/4 and D major.
    staff.measures[0].voices[0] = vec![Note::new(Pitch::new(Step::C, 4), Duration::Half)];
    staff.measures[1].time_sig = Some(TimeSignature {
        numerator: 2,
        denominator: 4,
    });
    staff.measures[1].key_sig = Some(KeySignature {
        fifths: 2,
        mode: "major".into(),
    });
    staff.measures[1].voices[0] = vec![Note::new(Pitch::new(Step::D, 4), Duration::Half)];
    staff.measures[2].voices[1] = vec![Note::new(Pitch::new(Step::G, 3), Duration::Quarter)];
    let bytes = acorde_io::serialize_midi(&score).expect("exports");
    let smf = midly::Smf::parse(&bytes).expect("valid SMF");
    let note_on_ticks = |key: u8| {
        let mut tick = 0u64;
        let mut found = Vec::new();
        for event in &smf.tracks[1] {
            tick += u64::from(event.delta.as_int());
            if let midly::TrackEventKind::Midi {
                message: midly::MidiMessage::NoteOn { key: k, vel },
                ..
            } = event.kind
                && k.as_int() == key
                && vel.as_int() > 0
            {
                found.push(tick);
            }
        }
        found
    };
    // 3/4 bar (1440) + 2/4 bar (960): voice 2's G3 starts bar 3 at 2400, not at 0.
    assert_eq!(note_on_ticks(55), vec![2400]);
    let mut keys = Vec::new();
    let mut tick = 0u64;
    for event in &smf.tracks[0] {
        tick += u64::from(event.delta.as_int());
        match event.kind {
            midly::TrackEventKind::Meta(midly::MetaMessage::KeySignature(fifths, _)) => {
                keys.push((tick, fifths));
            }
            midly::TrackEventKind::Meta(midly::MetaMessage::TimeSignature(n, ..)) if tick > 0 => {
                assert_eq!((tick, n), (1440, 2));
            }
            _ => {}
        }
    }
    assert_eq!(keys, vec![(0, 0), (1440, 2)]);
}

#[test]
fn abc_tempo_changes_round_trip() {
    let abc = "X:1\nM:4/4\nL:1/4\nQ:1/4=100\nK:C\nc4|[Q:3/8=40]d4|\nQ:\"Presto\" 1/2=90\ne4|\n";
    let score = acorde_io::parse_abc(abc).expect("parses");
    let tempos = |score: &acorde_core::Score| {
        score.parts[0].staves[0]
            .measures
            .iter()
            .map(|m| m.tempo)
            .collect::<Vec<_>>()
    };
    assert_eq!(score.settings.tempo_bpm, 100);
    assert_eq!(tempos(&score), vec![None, Some(60), Some(180)]);
    let text = acorde_io::serialize_abc(&score).expect("exports");
    assert_eq!(
        tempos(&acorde_io::parse_abc(&text).expect("re-imports")),
        tempos(&score)
    );
}

#[test]
fn midi_import_splits_overlapping_notes_into_voices_and_ties_notes_across_barlines() {
    use acorde_core::{Duration, Note, Pitch, Score, Step, TimeSignature};
    // Bar 1 in 4/4: a whole-note bass under four quarters. Bar 2 in 3/4: a dotted half tied on
    // into bar 3.
    let mut score = Score::new("midi", 120, 4, 4, 0, 3);
    let staff = &mut score.parts[0].staves[0];
    staff.measures[0].voices[0] = [Step::E, Step::F, Step::G, Step::A]
        .into_iter()
        .map(|step| Note::new(Pitch::new(step, 5), Duration::Quarter))
        .collect();
    staff.measures[0].voices[1] = vec![Note::new(Pitch::new(Step::C, 3), Duration::Whole)];
    staff.measures[1].time_sig = Some(TimeSignature {
        numerator: 3,
        denominator: 4,
    });
    let mut held = Note::new(Pitch::new(Step::D, 5), Duration::Half);
    held.dot_count = 1;
    held.tie_start = true;
    staff.measures[1].voices[0] = vec![held];
    let mut end = Note::new(Pitch::new(Step::D, 5), Duration::Quarter);
    end.tie_end = true;
    staff.measures[2].voices[0] = vec![end, Note::rest(Duration::Half)];

    let bytes = acorde_io::serialize_midi(&score).expect("exports");
    let back = acorde_io::parse_midi(&bytes).expect("imports");
    let measures = &back.parts[0].staves[0].measures;
    assert_eq!(measures.len(), 3);
    assert_eq!(
        measures[1]
            .time_sig
            .as_ref()
            .map(|t| (t.numerator, t.denominator)),
        Some((3, 4))
    );
    assert_eq!(
        measures[0].voices[0].len(),
        4,
        "the melody keeps its four quarters"
    );
    assert_eq!(measures[0].voices[1].len(), 1, "the bass is its own voice");
    assert_eq!(measures[0].voices[1][0].duration, Duration::Whole);
    let bar2 = &measures[1].voices[0];
    assert_eq!(
        (bar2[0].duration.clone(), bar2[0].dot_count),
        (Duration::Half, 1)
    );
    assert!(bar2[0].tie_start);
    let bar3 = &measures[2].voices[0];
    assert!(bar3[0].tie_end);
    assert_eq!(bar3[0].duration, Duration::Quarter);
    assert!(bar3[1].is_rest);
}

#[test]
fn abc_verses_follow_their_music_lines() {
    let abc = "X:1\nM:3/4\nL:1/4\nK:C\nC D E|\nw: hel-lo there\nw: sec-ond verse\nF G A|\nw: three four five\n";
    let score = acorde_io::parse_abc(abc).expect("parses");
    let words = |score: &acorde_core::Score, verse: u8| {
        score.parts[0].staves[0]
            .measures
            .iter()
            .flat_map(|m| m.voices[0].iter())
            .map(|n| {
                if verse == 1 {
                    n.lyric.as_ref().map(|l| l.text.clone())
                } else {
                    n.additional_lyrics
                        .iter()
                        .find(|entry| entry.verse == verse)
                        .map(|entry| entry.lyric.text.clone())
                }
            })
            .collect::<Vec<_>>()
    };
    let some = |w: &[&str]| {
        w.iter()
            .map(|w| (!w.is_empty()).then(|| w.to_string()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        words(&score, 1),
        some(&["hel", "lo", "there", "three", "four", "five"])
    );
    assert_eq!(words(&score, 2), some(&["sec", "ond", "verse", "", "", ""]));
    let text = acorde_io::serialize_abc(&score).expect("exports");
    let back = acorde_io::parse_abc(&text).expect("re-imports");
    assert_eq!(words(&back, 1), words(&score, 1), "{text}");
    assert_eq!(words(&back, 2), words(&score, 2), "{text}");
}

#[test]
fn mei_keeps_a_part_whose_meter_differs_from_the_others() {
    use acorde_core::{Score, TimeSignature};
    let mut score = Score::new("meters", 120, 3, 4, 0, 3);
    let mut second = acorde_core::Part::new("Voice", "V.");
    second.staves = score.parts[0].staves.clone();
    score.parts.push(second);
    let meter = |numerator, denominator| {
        Some(TimeSignature {
            numerator,
            denominator,
        })
    };
    score.parts[0].staves[0].measures[0].time_sig = meter(3, 4);
    score.parts[1].staves[0].measures[0].time_sig = meter(6, 8);
    score.parts[0].staves[0].measures[2].time_sig = meter(2, 4);
    score.parts[1].staves[0].measures[2].time_sig = meter(2, 4);
    let mei = acorde_io::serialize_mei(&score).expect("exports");
    assert!(
        mei.contains("<staffDef n=\"2\" meter.count=\"6\" meter.unit=\"8\"/>"),
        "{mei}"
    );
    let back = acorde_io::parse_mei(&mei).expect("imports");
    let meters = |part: usize| {
        let staff = &back.parts[part].staves[0];
        (0..staff.measures.len())
            .map(|i| {
                let t = staff.meter_at(i, &back.settings.time_signature);
                (t.numerator, t.denominator)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(meters(0), vec![(3, 4), (3, 4), (2, 4)]);
    assert_eq!(meters(1), vec![(6, 8), (6, 8), (2, 4)]);
}
