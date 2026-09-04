use acorde_core::{Clef, Duration, Step};
/// Integration tests: parse a fixture, serialize, re-parse, and verify
/// that key musical properties are preserved across the round-trip.
use acorde_io::{parse_midi, parse_musicxml, serialize_musicxml};

// Fixtures live at the workspace root under tests/fixtures/.
// include_str! paths are relative to this source file:
//   crates/io/tests/roundtrip.rs  →  ../../.. → workspace root → tests/fixtures/
static SIMPLE_XML: &str = include_str!("../../../tests/fixtures/simple.musicxml");
static MULTIPART_XML: &str = include_str!("../../../tests/fixtures/multipart.musicxml");
static MULTIVOICE_XML: &str = include_str!("../../../tests/fixtures/multivoice.musicxml");
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
            "multivoice.musicxml" => include_bytes!("../../../tests/fixtures/multivoice.musicxml"),
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
    assert_eq!(report["version_policy"], "workspace version is 1.1.0");
    assert!(WORKSPACE_MANIFEST.contains("version = \"1.1.0\""));
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
fn musicxml_unpitched_note_is_reported_instead_of_claiming_pitch_equivalence() {
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
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "musicxml.unsupported-element.unpitched"
            && diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/unpitched"))
    }));
    assert_eq!(
        report.score.parts[0].staves[0].measures[0].voices[0][0].pitches[0].to_midi(),
        72,
        "unpitched display position is retained until percussion mapping exists"
    );
    assert!(report.score.parts[0].staves[0].measures[0].voices[0][0].is_unpitched);
    assert_eq!(
        report.score.parts[0].staves[0].measures[0].voices[0][0]
            .instrument_id
            .as_deref(),
        Some("P1-I2")
    );
    assert_eq!(
        report.score.parts[0].percussion_instruments[0].midi_unpitched,
        Some(38)
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
            },
            acorde_core::StyledText {
                style: acorde_core::TextStyle::Expression,
                text: "dolce".to_string(),
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
    assert!(report.diagnostics.is_empty());
    assert_eq!(report.score.parts.len(), 4);
    assert_eq!(report.score.parts[0].staves[0].measures.len(), 23);
    assert_eq!(report.score.metadata.title, "Aloha Oe");
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
    fn projection(
        score: &acorde_core::Score,
    ) -> Vec<(i32, acorde_core::Duration, Option<(u8, u8)>)> {
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
    fn projection(
        score: &acorde_core::Score,
    ) -> Vec<(
        usize,
        usize,
        usize,
        usize,
        bool,
        u8,
        Vec<i32>,
        acorde_core::Duration,
        Option<(
            String,
            String,
            Option<String>,
            Option<String>,
            Vec<(u8, i8, String)>,
        )>,
    )> {
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
    let mut score = Score::new("T", 120, 4, 4, 0, 2);
    score.parts[0].staves[0].measures[1].tempo = Some(60);
    let xml = serialize_musicxml(&score).expect("serialize failed");
    let score2 = parse_musicxml(&xml).expect("parse failed");
    // Measure 1 tempo override must survive the roundtrip
    assert_eq!(score2.parts[0].staves[0].measures[1].tempo, Some(60));
    // Measure 0 gets the global tempo from <sound tempo> in the direction block
    assert_eq!(score2.parts[0].staves[0].measures[0].tempo, Some(120));
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
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(
        report.diagnostics[0].code,
        "musicxml.export-unsupported-capo"
    );
    assert_eq!(
        report.diagnostics[0].source_location.as_deref(),
        Some("/score/part/1/staff/1/tablature/capo")
    );
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
    assert_eq!(
        tab.capo, 0,
        "capo is intentionally outside MusicXML staff-details"
    );
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

#[cfg(all(feature = "abc", feature = "musicxml"))]
#[test]
fn non_mei_exports_report_harmonic_type_loss() {
    use acorde_core::{ChordDefinition, ChordSymbol, Duration, Note, Pitch, Score, Step};
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
    let musicxml = parse_musicxml(&serialize_musicxml(&score).expect("tab XML serializes"))
        .expect("tab XML parses");
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
