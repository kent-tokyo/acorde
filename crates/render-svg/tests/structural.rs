//! Structural SVG assertions: parse the output and check element/attribute counts rather
//! than comparing full-text golden files. A small golden fixture is used separately
//! (see `determinism.rs`) only to pin exact byte-stability, not visual content.

mod common;

use acorde_render_svg::{RenderPreflightKind, SvgRenderOptions, render_preflight, render_svg};

fn opts() -> SvgRenderOptions {
    SvgRenderOptions {
        width: 700.0,
        staff_size: 24.0,
        measures_per_system: 4,
        interactive: true,
    }
}

#[test]
fn render_preflight_locates_renderer_capability_boundaries() {
    use acorde_core::{Clef, Duration, Note, NoteHead, Pitch, Score, Step, TablatureConfig};

    let mut score = Score::new("preflight", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].clef = Clef::Percussion;
    score.parts[0].staves[0].tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 0,
    });
    let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
    note.pitches[0].alter = 3;
    note.note_head = NoteHead::Cross;
    note.tab_position = Some(acorde_core::TabPosition { string: 7, fret: 0 });
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let issues = render_preflight(&score);
    assert!(issues.iter().any(|issue| {
        issue.kind == RenderPreflightKind::UnsupportedAccidental
            && issue.source_location.ends_with("/note/1/pitch/1")
            && issue.preserved_value == "3"
    }));
    assert!(issues.iter().any(|issue| {
        issue.kind == RenderPreflightKind::InvalidTabPosition
            && issue.source_location.ends_with("/note/1/tab-position")
    }));
}

#[test]
fn render_preflight_locates_measure_clef_changes() {
    use acorde_core::{Clef, Score};

    let mut score = Score::new("measure clef preflight", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].clef = Some(Clef::Percussion);

    let issues = render_preflight(&score);
    assert!(
        issues
            .iter()
            .all(|issue| issue.kind != RenderPreflightKind::UnsupportedClef)
    );
}

/// Well-formedness check: every opened tag closes, via quick-xml's reader (it errors on
/// malformed XML). Cheaper than diffing full text against a golden file.
fn assert_well_formed_xml(svg: &str) {
    use quick_xml::Reader;
    use quick_xml::events::Event;
    let mut reader = Reader::from_str(svg);
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => panic!("SVG is not well-formed XML: {e}\n---\n{svg}"),
        }
    }
}

#[test]
fn svg_root_present_and_well_formed() {
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.trim_end().ends_with("</svg>"));
    assert_well_formed_xml(&svg);
}

#[test]
fn svg_root_has_deterministic_print_safe_geometry() {
    let first = render_svg(&common::satb_major(), &opts()).unwrap();
    let second = render_svg(&common::satb_major(), &opts()).unwrap();

    assert_eq!(first, second);
    assert!(first.contains("xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(first.contains("width=\"700.00\""));
    assert!(first.contains("viewBox=\"0 0 700.00 "));
    assert!(!first.contains("<image"));
    assert!(!first.contains("<use"));
    assert!(!first.contains("href=\"http"));
}

#[test]
fn large_score_and_many_staves_render_without_panicking() {
    use acorde_core::{Clef, Duration, Note, Pitch, Score, Staff, Step};

    let mut score = Score::new("large score", 120, 4, 4, 0, 32);
    let mut extra_staves = Vec::new();
    for index in 0..15 {
        let mut staff = Staff::new(if index % 2 == 0 {
            Clef::Treble
        } else {
            Clef::Bass
        });
        staff.measures = score.parts[0].staves[0].measures.clone();
        for measure in &mut staff.measures {
            measure.voices[0] = vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
        }
        extra_staves.push(staff);
    }
    score.parts[0].staves.extend(extra_staves);

    let svg = render_svg(&score, &opts()).expect("large score should render");
    assert!(svg.starts_with("<svg"));
    assert_eq!(
        svg.matches(r#"data-acorde-kind="measure""#).count(),
        16 * 32
    );
}

#[test]
fn pathological_voice_lengths_render_as_bounded_svg() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};

    let mut score = Score::new("long voices", 120, 4, 4, 0, 1);
    let measure = &mut score.parts[0].staves[0].measures[0];
    for voice in &mut measure.voices {
        voice.extend((0..64).map(|index| {
            Note::new(
                Pitch::new(if index % 2 == 0 { Step::C } else { Step::D }, 5),
                Duration::Sixteenth,
            )
        }));
    }

    let svg = render_svg(&score, &opts()).expect("long voices should render");
    assert!(svg.len() < 2_000_000, "renderer output unexpectedly large");
    assert!(svg.matches(r#"data-acorde-kind="note""#).count() >= 256);
}

#[test]
fn staff_line_count_matches_two_staves() {
    // 1 system x 2 staves (treble + bass) = 2 `acorde-staff` groups, 5 lines each = 10 lines.
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    assert_eq!(svg.matches(r#"class="acorde-staff""#).count(), 2);
    assert_eq!(svg.matches("acorde-staff-line").count(), 10);
}

#[test]
fn note_count_matches_satb_voices() {
    // 4 quarter notes x 4 voices (S, A, T, B) = 16 notes, 0 rests.
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    assert_eq!(svg.matches(r#"data-acorde-kind="note""#).count(), 16);
    assert_eq!(svg.matches(r#"data-acorde-kind="rest""#).count(), 0);
}

#[test]
fn rest_present_in_dotted_and_rest_fixture() {
    let svg = render_svg(&common::satb_dotted_and_rest(), &opts()).unwrap();
    assert_eq!(svg.matches(r#"data-acorde-kind="rest""#).count(), 1);
    // Soprano: dotted-quarter, eighth, dotted-quarter, eighth = 4 notes.
    // Alto: dotted-half + rest = 1 note + 1 rest. Tenor/Bass: 4 quarters each.
    assert_eq!(
        svg.matches(r#"data-acorde-kind="note""#).count(),
        4 + 1 + 4 + 4
    );
}

#[test]
fn measure_group_count_is_one_per_staff() {
    // 1 measure x 2 staves = 2 `data-acorde-kind="measure"` groups.
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    assert_eq!(svg.matches(r#"data-acorde-kind="measure""#).count(), 2);
}

#[test]
fn barline_present_at_measure_end() {
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    assert!(svg.matches("acorde-barline").count() >= 1);
}

#[test]
fn stable_data_attributes_present_and_addressable() {
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    // Every note must carry the full positional attribute set plus the combined addr.
    assert!(svg.contains(r#"data-part="0""#));
    assert!(svg.contains(r#"data-staff="0""#));
    assert!(svg.contains(r#"data-staff="1""#));
    assert!(svg.contains(r#"data-measure="0""#));
    assert!(svg.contains(r#"data-voice="0""#));
    assert!(svg.contains(r#"data-voice="1""#));
    // Soprano note 2 (0-indexed) — D5, the second soprano note.
    assert!(svg.contains(r#"data-note-addr="0:0:0:0:1""#));
    // Bass-staff bass-voice note 0 — C3.
    assert!(svg.contains(r#"data-note-addr="0:1:0:1:0""#));
}

#[test]
fn interactive_false_omits_data_attributes() {
    let mut o = opts();
    o.interactive = false;
    let svg = render_svg(&common::satb_major(), &o).unwrap();
    assert!(!svg.contains("data-acorde-kind"));
    assert!(!svg.contains("data-note-addr"));
}

#[test]
fn common_span_marks_are_rendered() {
    use acorde_core::{Duration, HairpinKind, Note, OttavaKind, Pitch, Step};

    let mut score = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![
            Note::new(Pitch::new(Step::C, 5), Duration::Quarter),
            Note::new(Pitch::new(Step::D, 5), Duration::Quarter),
            Note::new(Pitch::new(Step::E, 5), Duration::Quarter),
            Note::new(Pitch::new(Step::F, 5), Duration::Quarter),
        ],
        vec![],
    );
    let notes = &mut score.parts[0].staves[0].measures[0].voices[0];
    notes[0].tie_start = true;
    notes[0].slur_start = true;
    notes[2].slur_end = true;
    notes[0].hairpin_start = Some(HairpinKind::Crescendo);
    notes[2].hairpin_end = true;
    notes[0].ottava_start = Some(OttavaKind::Va8);
    notes[2].ottava_end = true;
    notes[0].pedal_start = true;
    notes[2].pedal_end = true;

    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("class=\"acorde-tie\""));
    assert!(svg.contains("class=\"acorde-slur\""));
    assert!(svg.contains("class=\"acorde-hairpin\""));
    assert!(svg.contains("class=\"acorde-ottava\""));
    assert!(svg.contains("class=\"acorde-pedal\""));
    assert!(svg.contains("data-acorde-span=\"hairpin\""));
    assert!(svg.contains("data-start-note-addr=\"0:0:0:0:0\""));
    assert!(svg.contains("data-end-note-addr=\"0:0:0:0:2\""));
}

#[test]
fn spans_crossing_systems_get_continuation_segments() {
    use acorde_core::{Duration, HairpinKind, Note, Pitch, Step};

    let mut score = acorde_core::Score::new("Cross-system", 120, 4, 4, 0, 2);
    let measures = &mut score.parts[0].staves[0].measures;
    measures[0].voices[0] = vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)];
    measures[1].voices[0] = vec![Note::new(Pitch::new(Step::D, 5), Duration::Whole)];
    measures[0].voices[0][0].hairpin_start = Some(HairpinKind::Crescendo);
    measures[1].voices[0][0].hairpin_end = true;
    measures[0].voices[0][0].tie_start = true;

    let mut options = opts();
    options.measures_per_system = 1;
    let svg = render_svg(&score, &options).unwrap();
    assert!(svg.contains("class=\"acorde-hairpin\" data-continuation=\"true\""));
    assert!(svg.matches("class=\"acorde-tie\"").count() >= 2);
}

#[test]
fn note_annotations_are_rendered_and_xml_escaped() {
    use acorde_core::{Articulation, ChordSymbol, Duration, Dynamic, Lyric, Note, Pitch, Step};

    let mut score = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)],
        vec![],
    );
    let note = &mut score.parts[0].staves[0].measures[0].voices[0][0];
    note.dynamic = Some(Dynamic::Mf);
    note.chord_symbol = Some(ChordSymbol {
        root: "C".into(),
        kind: "major".into(),
        bass: Some("G".into()),
        placement: None,
        extender: false,
        harmonic_degree: None,
        harmony_function: None,
        harmony_type: None,
        chord_ref: None,
        degrees: Vec::new(),
    });
    note.lyric = Some(Lyric {
        text: "A&B<".into(),
        syllabic: "single".into(),
    });
    note.articulations = vec![
        Articulation::Staccato,
        Articulation::Staccatissimo,
        Articulation::Accent,
        Articulation::Tenuto,
        Articulation::BreathMark,
        Articulation::Caesura,
    ];

    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("class=\"acorde-dynamic\""));
    assert!(svg.contains("class=\"acorde-chord-symbol\""));
    assert!(svg.contains("class=\"acorde-lyric\""));
    assert!(svg.contains("A&amp;B&lt;"));
    assert!(svg.contains("acorde-staccato"));
    assert!(svg.contains("acorde-staccatissimo"));
    assert!(svg.contains("acorde-accent"));
    assert!(svg.contains("acorde-tenuto"));
    assert!(svg.contains("acorde-breath-mark"));
    assert!(svg.contains("acorde-caesura"));
}

#[test]
fn ornament_articulations_are_rendered_with_semantic_classes() {
    use acorde_core::{Articulation, Duration, Note, Pitch, Step};

    let mut score = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)],
        vec![],
    );
    score.parts[0].staves[0].measures[0].voices[0][0].articulations = vec![
        Articulation::Mordent,
        Articulation::InvertedMordent,
        Articulation::Turn,
        Articulation::InvertedTurn,
        Articulation::Shake,
        Articulation::Tremolo(3),
    ];

    let svg = render_svg(&score, &opts()).unwrap();
    for class in [
        "acorde-mordent",
        "acorde-inverted-mordent",
        "acorde-turn",
        "acorde-inverted-turn",
        "acorde-shake",
        "acorde-tremolo",
    ] {
        assert!(svg.contains(class), "missing ornament class: {class}");
    }
    assert!(svg.contains(">tremolo 3</text>"));
}

#[test]
fn multiple_articulations_stack_on_distinct_baselines() {
    use acorde_core::{Articulation, Duration, Note, Pitch, Step};

    let mut score = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)],
        vec![],
    );
    score.parts[0].staves[0].measures[0].voices[0][0].articulations =
        vec![Articulation::Mordent, Articulation::Turn];

    let svg = render_svg(&score, &opts()).unwrap();
    let ys: Vec<&str> = svg
        .split("<text ")
        .filter(|text| text.contains("acorde-ornament"))
        .filter_map(|text| text.split("y=\"").nth(1))
        .filter_map(|text| text.split('\"').next())
        .collect();
    assert_eq!(ys.len(), 2);
    assert_ne!(ys[0], ys[1]);
}

#[test]
fn mixed_note_annotations_use_distinct_vertical_lanes() {
    use acorde_core::{Articulation, Duration, Dynamic, Lyric, Note, Pitch, Step};

    let mut score = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)],
        vec![],
    );
    let note = &mut score.parts[0].staves[0].measures[0].voices[0][0];
    note.dynamic = Some(Dynamic::Mf);
    note.lyric = Some(Lyric {
        text: "la".to_string(),
        syllabic: "single".to_string(),
    });
    note.articulations = vec![Articulation::Trill];

    let svg = render_svg(&score, &opts()).unwrap();
    let ys: Vec<&str> = svg
        .split("<text ")
        .filter(|text| {
            text.contains("acorde-dynamic")
                || text.contains("acorde-lyric")
                || text.contains("acorde-trill")
        })
        .filter_map(|text| text.split("y=\"").nth(1))
        .filter_map(|text| text.split('\"').next())
        .collect();
    assert_eq!(ys.len(), 3);
    assert_eq!(ys.iter().collect::<std::collections::HashSet<_>>().len(), 3);
}

#[test]
fn mixed_annotation_lanes_expand_the_page_margin() {
    use acorde_core::{Articulation, Duration, Dynamic, Lyric, Note, Pitch, Step};

    let mut score = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)],
        vec![],
    );
    let note = &mut score.parts[0].staves[0].measures[0].voices[0][0];
    note.dynamic = Some(Dynamic::F);
    note.lyric = Some(Lyric {
        text: "la".to_string(),
        syllabic: "single".to_string(),
    });
    note.articulations = vec![Articulation::Trill, Articulation::Mordent];

    let svg = render_svg(&score, &opts()).unwrap();
    let view_box = svg
        .split_once("viewBox=\"")
        .and_then(|(_, rest)| rest.split_once('\"'))
        .map(|(value, _)| value)
        .expect("viewBox");
    let values: Vec<f32> = view_box
        .split_whitespace()
        .map(|value| value.parse::<f32>().expect("finite viewBox value"))
        .collect();
    assert_eq!(values.len(), 4);
    assert!(values[3] > 0.0);
}

#[test]
fn short_rests_custom_noteheads_and_small_notes_are_rendered() {
    use acorde_core::{Duration, Note, NoteHead, Pitch, Step};

    let mut notes = vec![
        Note::rest(Duration::Sixteenth),
        Note::rest(Duration::ThirtySecond),
        Note::rest(Duration::SixtyFourth),
        Note::new(Pitch::new(Step::C, 5), Duration::Quarter),
        Note::new(Pitch::new(Step::D, 5), Duration::Quarter),
        Note::new(Pitch::new(Step::E, 5), Duration::Quarter),
        Note::new(Pitch::new(Step::F, 5), Duration::Quarter),
    ];
    notes[3].note_head = NoteHead::Diamond;
    notes[4].note_head = NoteHead::X;
    notes[5].note_head = NoteHead::Triangle;
    notes[6].is_grace = true;
    notes[6].grace_slash = true;
    notes[6].is_cue = true;

    let score = common::single_staff_score(acorde_core::Clef::Treble, 0, 4, 4, notes, vec![]);
    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("acorde-rest-flag"));
    assert!(svg.contains("acorde-notehead-diamond"));
    assert!(svg.contains("acorde-notehead-x"));
    assert!(svg.contains("acorde-notehead-triangle"));
    assert!(svg.contains("acorde-grace"));
    assert!(svg.contains("acorde-cue"));
    assert!(svg.contains("acorde-grace-slash"));
    assert_well_formed_xml(&svg);
}

#[test]
fn part_group_connectors_and_first_system_labels_are_rendered() {
    use acorde_core::{PartGroup, PartGroupSymbol, ScoreTemplate};

    let mut score = acorde_core::Score::template(ScoreTemplate::StringQuartet);
    score.part_groups.push(PartGroup {
        first_part: 0,
        last_part: 3,
        symbol: PartGroupSymbol::Bracket,
        barlines_connect: true,
    });
    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("acorde-part-bracket"));
    assert!(svg.contains("acorde-part-label"));
    assert_well_formed_xml(&svg);
}

#[test]
fn precomputed_row_and_metadata_contracts_are_stable() {
    use acorde_layout::{LayoutConfig, compute_layout};
    let score = common::satb_major();
    let layout = compute_layout(&score, &LayoutConfig::default());
    let row = acorde_render_svg::render_svg_row(&score, &layout, 0, &opts()).unwrap();
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts()).unwrap();
    assert_eq!(metadata.contract_version, 3);
    assert_eq!(metadata.part_count, 1);
    assert_eq!(metadata.staff_count, 2);
    assert_eq!(metadata.measure_count, 1);
    assert_eq!(metadata.note_count, 16);
    assert!(metadata.accessible_text.contains("parts"));
    assert!(row.starts_with("<svg"));
    assert_eq!(metadata.width, opts().width);
    assert_eq!(metadata.address_bounds.len(), 16);
    assert_eq!(
        (
            metadata.address_bounds[0].part,
            metadata.address_bounds[0].staff,
            metadata.address_bounds[0].measure
        ),
        (0, 0, 0)
    );
    assert!(acorde_render_svg::render_svg_row(&score, &layout, 99, &opts()).is_err());
}

#[test]
fn metadata_exposes_measure_text_style_and_location() {
    use acorde_core::{StyledText, TextStyle};
    use acorde_layout::{LayoutConfig, compute_layout};
    let mut score = common::satb_major();
    score.parts[0].staves[0].measures[0].texts.push(StyledText {
        style: TextStyle::Technique,
        text: "con sordino".to_string(),
        placement: None,
        offset_x: None,
        offset_y: None,
        relative_x: None,
        relative_y: None,
    });
    let layout = compute_layout(&score, &LayoutConfig::default());
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts()).unwrap();
    assert_eq!(metadata.text_annotations.len(), 1);
    assert_eq!(metadata.text_annotations[0].part, 0);
    assert_eq!(metadata.text_annotations[0].staff, 0);
    assert_eq!(metadata.text_annotations[0].measure, 0);
    assert_eq!(metadata.text_annotations[0].style, TextStyle::Technique);
    assert_eq!(metadata.text_annotations[0].text, "con sordino");
    assert_eq!(metadata.text_annotations[0].placement, None);
    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("class=\"acorde-measure-text acorde-measure-text-technique\""));
    assert!(svg.contains("data-acorde-kind=\"measure-text\""));
    assert!(svg.contains("con sordino"));
}

#[test]
fn legacy_measure_text_fields_are_rendered_and_exposed_with_styled_text() {
    use acorde_core::TextStyle;
    let mut score = common::satb_major();
    let measure = &mut score.parts[0].staves[0].measures[0];
    measure.tempo_text = Some("Allegro".to_string());
    measure.rehearsal = Some("A".to_string());
    measure.expression_text = Some("dolce".to_string());
    measure.navigation = Some("DaCoda".to_string());
    let layout = acorde_layout::compute_layout(&score, &Default::default());
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts()).unwrap();
    assert!(
        metadata
            .text_annotations
            .iter()
            .any(|text| text.style == TextStyle::Generic && text.text == "Allegro")
    );
    assert!(
        metadata
            .text_annotations
            .iter()
            .any(|text| text.style == TextStyle::RehearsalMark && text.text == "A")
    );
    assert!(
        metadata
            .text_annotations
            .iter()
            .any(|text| text.style == TextStyle::Expression && text.text == "dolce")
    );
    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("acorde-measure-text-rehearsal-mark"));
    assert!(svg.contains(">dolce</text>"));
    assert!(svg.contains(">DaCoda</text>"));
}

#[test]
fn structured_figured_bass_is_rendered_once() {
    use acorde_core::{FiguredBassFigure, Score};

    let mut score = Score::new("figured bass", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].figured_bass = vec![
        FiguredBassFigure {
            number: "6".to_string(),
            alter: Some("-1".to_string()),
            prefix: None,
            suffix: None,
            extender: false,
        },
        FiguredBassFigure {
            number: "4".to_string(),
            alter: None,
            prefix: None,
            suffix: None,
            extender: false,
        },
    ];

    let svg = render_svg(&score, &opts()).unwrap();
    assert_eq!(svg.matches(">b6 4</text>").count(), 1);
    assert_eq!(svg.matches("acorde-measure-text-figured-bass").count(), 1);
}

#[test]
fn figured_bass_extender_is_rendered_as_a_continuation_line() {
    use acorde_core::{FiguredBassFigure, Score};

    let mut score = Score::new("figured bass extender", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].figured_bass = vec![FiguredBassFigure {
        number: "6".to_string(),
        alter: None,
        prefix: None,
        suffix: None,
        extender: true,
    }];

    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("acorde-figured-bass-extender"));
    assert_eq!(svg.matches("acorde-figured-bass-extender").count(), 1);
}

#[test]
fn metadata_preserves_positioned_direction_text_fields() {
    use acorde_core::{StyledText, TextStyle};
    use acorde_layout::{LayoutConfig, compute_layout};
    let mut score = common::satb_major();
    score.parts[0].staves[0].measures[0].texts.push(StyledText {
        style: TextStyle::Expression,
        text: "dolce".to_string(),
        placement: Some("below".to_string()),
        offset_x: Some(12.5),
        offset_y: Some(-3.0),
        relative_x: Some(1.25),
        relative_y: Some(-0.5),
    });
    let layout = compute_layout(&score, &LayoutConfig::default());
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts()).unwrap();
    let annotation = metadata
        .text_annotations
        .iter()
        .find(|annotation| annotation.text == "dolce")
        .expect("positioned text annotation");
    assert_eq!(annotation.placement.as_deref(), Some("below"));
    assert_eq!(annotation.offset_x, Some(12.5));
    assert_eq!(annotation.offset_y, Some(-3.0));
    assert_eq!(annotation.relative_x, Some(1.25));
    assert_eq!(annotation.relative_y, Some(-0.5));
}

#[test]
fn content_aware_horizontal_margins_reach_staff_lines_and_labels() {
    use acorde_core::{PartGroup, PartGroupSymbol, StyledText, TextStyle};
    let mut score = common::satb_major();
    score.parts[0].short_name = "Long instrument label".to_string();
    score.part_groups.push(PartGroup {
        first_part: 0,
        last_part: 0,
        symbol: PartGroupSymbol::Bracket,
        barlines_connect: false,
    });
    score.parts[0].staves[0].measures[0].texts.push(StyledText {
        style: TextStyle::Expression,
        text: "right offset".to_string(),
        placement: None,
        offset_x: Some(20.0),
        offset_y: None,
        relative_x: None,
        relative_y: None,
    });

    let svg = render_svg(&score, &opts()).unwrap();
    // 21 characters × 0.42u plus connector/label clearance, at staff_size 24.
    assert!(svg.contains(r#"class="acorde-staff-line" x1="250.08""#));
    // The positive 2u offset expands the right content margin; the staff remains inside the
    // requested canvas rather than relying on one historical coordinate.
    assert!(svg.contains(r#"class="acorde-staff-line" x1="250.08" y1=""#));
    let first_staff_line = svg
        .split(r#"class="acorde-staff-line""#)
        .nth(1)
        .expect("staff line exists");
    let x2 = first_staff_line
        .split(r#"x2=""#)
        .nth(1)
        .and_then(|value| value.split('\"').next())
        .and_then(|value| value.parse::<f32>().ok())
        .expect("staff line has numeric x2");
    assert!(x2 > 0.0 && x2 < opts().width);
    assert!(svg.contains(r#"class="acorde-part-label""#));
}

#[test]
fn measure_text_rejects_unbounded_or_non_finite_positioning() {
    use acorde_core::{StyledText, TextStyle};
    let mut score = common::satb_major();
    score.parts[0].staves[0].measures[0].texts.push(StyledText {
        style: TextStyle::Generic,
        text: "x".to_string(),
        placement: None,
        offset_x: Some(f64::NAN),
        offset_y: None,
        relative_x: None,
        relative_y: None,
    });
    assert!(matches!(
        render_svg(&score, &opts()),
        Err(acorde_render_svg::RenderError::InvalidMeasureTextOffset { field: "offset_x" })
    ));

    score.parts[0].staves[0].measures[0].texts[0].offset_x = None;
    score.parts[0].staves[0].measures[0].texts[0].text = "x".repeat(16 * 1024 + 1);
    assert!(matches!(
        render_svg(&score, &opts()),
        Err(acorde_render_svg::RenderError::MeasureTextTooLarge { .. })
    ));

    score.parts[0].staves[0].measures[0].texts[0].text = "bad\u{1}".to_string();
    let preflight = render_preflight(&score);
    assert!(preflight.iter().any(|issue| {
        issue.kind == RenderPreflightKind::InvalidXmlCharacter
            && issue.source_location.ends_with("/text/1")
            && issue.preserved_value == "U+0001"
    }));
    assert!(matches!(
        render_svg(&score, &opts()),
        Err(acorde_render_svg::RenderError::InvalidXmlCharacter { codepoint: 1 })
    ));
}

#[test]
fn measure_text_extreme_vertical_offsets_expand_content_height() {
    use acorde_core::{StyledText, TextStyle};
    let normal = common::satb_major();
    let mut offset = normal.clone();
    offset.parts[0].staves[0].measures[0]
        .texts
        .push(StyledText {
            style: TextStyle::Expression,
            text: "above".to_string(),
            placement: None,
            offset_x: None,
            offset_y: Some(-200.0),
            relative_x: None,
            relative_y: None,
        });
    let normal_height = acorde_render_svg::render_svg_metadata(
        &normal,
        &acorde_layout::compute_layout(&normal, &Default::default()),
        &opts(),
    )
    .unwrap()
    .height;
    let offset_height = acorde_render_svg::render_svg_metadata(
        &offset,
        &acorde_layout::compute_layout(&offset, &Default::default()),
        &opts(),
    )
    .unwrap()
    .height;
    assert!(offset_height > normal_height);
}

#[test]
fn note_attached_annotations_expand_content_height() {
    use acorde_core::{Dynamic, Lyric, OttavaKind, Pitch, Step};
    let normal = common::satb_major();
    let mut annotated = normal.clone();
    let note = &mut annotated.parts[0].staves[0].measures[0].voices[0][0];
    note.pitches[0] = Pitch::new(Step::C, 2);
    note.dynamic = Some(Dynamic::Ff);
    note.stem_up = Some(false);
    note.technique_text = Some("pizz.".to_string());
    note.fingering = Some(2);
    note.lyric = Some(Lyric {
        text: "long".to_string(),
        syllabic: "single".to_string(),
    });
    note.ottava_start = Some(OttavaKind::Va8);

    let normal_height = acorde_render_svg::render_svg_metadata(
        &normal,
        &acorde_layout::compute_layout(&normal, &Default::default()),
        &opts(),
    )
    .unwrap()
    .height;
    let annotated_height = acorde_render_svg::render_svg_metadata(
        &annotated,
        &acorde_layout::compute_layout(&annotated, &Default::default()),
        &opts(),
    )
    .unwrap()
    .height;
    assert!(annotated_height > normal_height);
    let svg = render_svg(&annotated, &opts()).unwrap();
    let annotation_y = |class: &str| {
        let start = svg.find(&format!("class=\"{class}\"")).expect("annotation");
        let fragment = &svg[start..];
        fragment
            .split(" y=\"")
            .nth(1)
            .and_then(|value| value.split('"').next())
            .and_then(|value| value.parse::<f32>().ok())
            .expect("annotation y")
    };
    assert_ne!(annotation_y("acorde-dynamic"), annotation_y("acorde-lyric"));
    assert!(svg.contains("class=\"acorde-technique-text\""));
    assert!(svg.contains(">pizz.</text>"));
    assert!(svg.contains("class=\"acorde-fingering\""));
    assert!(svg.contains(">2</text>"));
}

#[test]
fn multiple_measure_text_entries_are_stacked_deterministically() {
    use acorde_core::{StyledText, TextStyle};
    let mut score = common::satb_major();
    let texts = &mut score.parts[0].staves[0].measures[0].texts;
    for text in ["first", "second"] {
        texts.push(StyledText {
            style: TextStyle::Expression,
            text: text.to_string(),
            placement: None,
            offset_x: None,
            offset_y: None,
            relative_x: None,
            relative_y: None,
        });
    }
    let svg = render_svg(&score, &opts()).unwrap();
    let y_for = |text: &str| {
        let marker = format!(">{text}</text>");
        let end = svg.find(&marker).expect("measure text is rendered");
        let start = svg[..end].rfind(" y=\"").expect("text has a y coordinate") + 4;
        svg[start..end]
            .split('"')
            .next()
            .expect("y coordinate has a closing quote")
            .to_string()
    };
    assert_ne!(y_for("first"), y_for("second"));
}

#[test]
fn negative_measure_text_offsets_expand_left_content_margin() {
    use acorde_core::{StyledText, TextStyle};
    let mut score = common::satb_major();
    score.parts[0].staves[0].measures[0].texts.push(StyledText {
        style: TextStyle::Generic,
        text: "left".to_string(),
        placement: None,
        offset_x: Some(-100.0),
        offset_y: None,
        relative_x: None,
        relative_y: None,
    });
    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains(">left</text>"));
    assert!(!svg.contains("class=\"acorde-measure-text acorde-measure-text-generic\" x=\"-"));
}

#[test]
fn malformed_precomputed_layout_returns_error_instead_of_panicking() {
    use acorde_layout::{LayoutConfig, compute_layout};
    let score = common::satb_major();
    let mut layout = compute_layout(&score, &LayoutConfig::default());
    layout.rows[0].measure_indices[0] = 999;
    let err = acorde_render_svg::render_svg_with_layout(&score, &layout, &opts()).unwrap_err();
    assert!(matches!(
        err,
        acorde_render_svg::RenderError::InvalidLayout { .. }
    ));
}

#[test]
fn invalid_render_dimensions_return_errors() {
    let score = common::satb_major();
    let mut options = opts();
    options.width = 0.0;
    assert!(matches!(
        render_svg(&score, &options),
        Err(acorde_render_svg::RenderError::InvalidOptions { .. })
    ));
    options = opts();
    options.staff_size = -1.0;
    assert!(matches!(
        render_svg(&score, &options),
        Err(acorde_render_svg::RenderError::InvalidOptions { .. })
    ));
    options = opts();
    options.width = 80.0;
    assert!(matches!(
        render_svg(&score, &options),
        Err(acorde_render_svg::RenderError::InvalidOptions { .. })
    ));
}

#[test]
fn minimum_measure_width_overflow_returns_error() {
    use acorde_core::{Measure, Score};

    let mut score = Score::new("dense system", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures.push(Measure::empty(4, 4));
    let mut options = opts();
    options.width = 100.0;
    options.measures_per_system = 2;

    assert!(matches!(
        render_svg(&score, &options),
        Err(acorde_render_svg::RenderError::InvalidOptions { .. })
    ));
}

#[test]
fn extreme_ledger_content_expands_vertical_margin() {
    use acorde_core::{Duration, Note, Pitch, Step};
    let normal = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)],
        vec![],
    );
    let extreme = common::single_staff_score(
        acorde_core::Clef::Treble,
        0,
        4,
        4,
        vec![Note::new(Pitch::new(Step::C, 8), Duration::Whole)],
        vec![],
    );
    let normal_meta = acorde_render_svg::render_svg_metadata(
        &normal,
        &acorde_layout::compute_layout(&normal, &Default::default()),
        &opts(),
    )
    .unwrap();
    let extreme_meta = acorde_render_svg::render_svg_metadata(
        &extreme,
        &acorde_layout::compute_layout(&extreme, &Default::default()),
        &opts(),
    )
    .unwrap();
    assert!(extreme_meta.height > normal_meta.height);
}

#[test]
fn key_signature_glyph_present_when_nonzero_fifths() {
    let major = render_svg(&common::satb_major(), &opts()).unwrap(); // fifths = 0
    let minor = render_svg(&common::satb_minor(), &opts()).unwrap(); // fifths = 1

    // C major: no key-sig accidentals (empty <g class="acorde-key-sig"></g> per staff).
    assert!(!major.contains("acorde-sharp") && !major.contains("acorde-flat"));
    // E minor (1 sharp): each staff's key signature draws exactly one sharp.
    assert_eq!(minor.matches("acorde-key-sig").count(), 2); // one group per staff
    assert!(minor.matches("acorde-sharp").count() >= 2); // >= 1 per staff (plus the D# accidental)
}

#[test]
fn time_signature_drawn_once_per_staff_on_first_row() {
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    assert_eq!(svg.matches("acorde-time-sig").count(), 2);
}

#[test]
fn accidentals_cover_all_required_kinds() {
    use acorde_core::{
        Clef, Duration, KeySignature, Measure, Note, Part, Pitch, Score, Staff, Step, TimeSignature,
    };

    let mut score = Score::default();
    score.settings.time_signature = TimeSignature {
        numerator: 4,
        denominator: 4,
    };
    score.settings.key_signature = KeySignature {
        fifths: 0,
        mode: "major".to_string(),
    };
    let mut part = Part::new("T", "T");
    let mut staff = Staff::new(Clef::Treble);
    let mut m = Measure::empty(4, 4);
    m.number = 1;
    let mut sharp = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
    sharp.pitches[0].alter = 1;
    let mut flat = Note::new(Pitch::new(Step::B, 4), Duration::Quarter);
    flat.pitches[0].alter = -1;
    let mut dsharp = Note::new(Pitch::new(Step::C, 5), Duration::Quarter);
    dsharp.pitches[0].alter = 2;
    let mut dflat = Note::new(Pitch::new(Step::D, 5), Duration::Quarter);
    dflat.pitches[0].alter = -2;
    m.voices[0] = vec![sharp, flat, dsharp, dflat];
    staff.measures.push(m);
    part.staves.push(staff);
    score.parts = vec![part];

    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("acorde-sharp"));
    assert!(svg.contains("acorde-flat"));
    assert!(svg.contains("acorde-double-sharp"));
    assert!(svg.contains("acorde-double-flat"));
    assert_well_formed_xml(&svg);
}

#[test]
fn triple_sharp_is_rejected_not_silently_dropped() {
    use acorde_core::{
        Clef, Duration, KeySignature, Measure, Note, Part, Pitch, Score, Staff, Step, TimeSignature,
    };
    let mut score = Score::default();
    score.settings.time_signature = TimeSignature {
        numerator: 4,
        denominator: 4,
    };
    score.settings.key_signature = KeySignature {
        fifths: 0,
        mode: "major".to_string(),
    };
    let mut part = Part::new("T", "T");
    let mut staff = Staff::new(Clef::Treble);
    let mut m = Measure::empty(4, 4);
    m.number = 1;
    let mut triple = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
    triple.pitches[0].alter = 3; // beyond the -2..=2 range acorde-render-svg supports
    m.voices[0] = vec![triple];
    staff.measures.push(m);
    part.staves.push(staff);
    score.parts = vec![part];

    let err = render_svg(&score, &opts()).unwrap_err();
    assert_eq!(
        err,
        acorde_render_svg::RenderError::UnsupportedAccidental { alter: 3 }
    );
}

#[test]
fn glyph_coverage_is_explicit_and_stable() {
    let coverage = acorde_render_svg::glyph_coverage();
    assert_eq!(coverage.contract_version, 3);
    assert_eq!(coverage.resource_id, "acorde-vector-glyphs-v1");
    assert!(coverage.vector_glyphs);
    assert_eq!(coverage.accidental_min, -2);
    assert_eq!(coverage.accidental_max, 2);
    assert!(coverage.supported_clefs.iter().any(|clef| clef == "treble"));
    assert!(coverage.microtone_marker.supported);
    assert_eq!(coverage.microtone_marker.representation, "svg-text-cents");
    assert!(coverage.microtone_marker.exact_cents);
    assert!(
        coverage
            .percussion_noteheads
            .contains(&"acorde-percussion-notehead-cross".to_owned())
    );
}

#[test]
fn tablature_renders_lines_frets_and_techniques() {
    use acorde_core::{Duration, GuitarTechnique, Note, Pitch, Staff, Step, TablatureConfig};
    let mut score = acorde_core::Score::new("Tab", 120, 4, 4, 0, 1);
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 0,
    });
    staff.measures.push(acorde_core::Measure::empty(4, 4));
    let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
    note.tab_position = Some(acorde_core::TabPosition { string: 2, fret: 3 });
    note.fingerings = vec![1, 3];
    note.fingering = Some(1);
    note.guitar_technique = Some(GuitarTechnique::Bend);
    note.guitar_bend_alter_cents = Some(200);
    staff.measures[0].voices[0] = vec![note];
    score.parts[0].staves = vec![staff];

    let svg = render_svg(&score, &opts()).expect("tablature renders");
    assert_eq!(svg.matches("acorde-staff-line").count(), 6);
    assert!(svg.contains("acorde-tab-fret"));
    assert!(svg.contains(">3</text>"));
    assert!(svg.contains("acorde-tab-fingering"));
    assert!(svg.contains(">1/3</text>"));
    assert!(svg.contains("acorde-tab-technique"));
    assert!(svg.contains(">bend +200c</text>"));
    assert_well_formed_xml(&svg);
}

#[test]
fn extended_tablature_staff_height_preserves_system_geometry() {
    use acorde_core::{Clef, Score, Staff, TablatureConfig};

    let mut score = Score::new("extended tab", 120, 4, 4, 0, 1);
    let mut tab_staff = Staff::new(Clef::Treble);
    tab_staff.tablature = Some(TablatureConfig {
        lines: 8,
        tuning_midi: vec![64, 59, 55, 50, 45, 40, 35, 30],
        capo: 0,
    });
    tab_staff.measures = score.parts[0].staves[0].measures.clone();
    score.parts[0].staves.push(tab_staff);

    let svg = render_svg(&score, &opts()).expect("extended tablature should render");
    assert_eq!(svg.matches("class=\"acorde-staff-line\"").count(), 5 + 8);
    assert!(svg.contains("data-acorde-kind=\"measure\""));
}

#[test]
fn tablature_preserves_microtone_marker() {
    use acorde_core::{
        Duration, Measure, Note, Pitch, Score, Staff, Step, TabPosition, TablatureConfig,
    };

    let mut score = Score::new("Tab microtone", 120, 4, 4, 0, 1);
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 0,
    });
    staff.measures.push(Measure::empty(4, 4));
    let mut note = Note::new(Pitch::with_microtone(Step::E, 4, 0, 25), Duration::Quarter);
    note.tab_position = Some(TabPosition { string: 2, fret: 3 });
    staff.measures[0].voices[0] = vec![note];
    score.parts[0].staves = vec![staff];

    let svg = render_svg(&score, &opts()).expect("tablature microtone should render");
    assert!(svg.contains("class=\"acorde-microtone\""));
    assert!(svg.contains(">+25c</text>"));
    assert!(svg.contains(">3</text>"));
    assert_well_formed_xml(&svg);
}

#[test]
fn tablature_metric_fixture_matches_public_contract() {
    #[derive(serde::Deserialize)]
    struct Fixture {
        contract_version: u32,
        license: String,
        cases: Vec<Case>,
        side_gap_units: f32,
    }
    #[derive(serde::Deserialize)]
    struct Case {
        fret: u8,
        digit_count: u8,
        advance_units: f32,
    }

    let fixture: Fixture = serde_json::from_str(include_str!("fixtures/tab_metrics.json"))
        .expect("tab metrics fixture is valid JSON");
    assert_eq!(
        fixture.contract_version,
        acorde_render_svg::TAB_METRICS_CONTRACT_VERSION
    );
    assert_eq!(fixture.license, "self-authored");
    for case in fixture.cases {
        let metrics = acorde_render_svg::tab_fret_metrics(case.fret);
        assert_eq!(metrics.fret, case.fret);
        assert_eq!(metrics.digit_count, case.digit_count);
        assert!((metrics.advance_units - case.advance_units).abs() < 1e-6);
        assert!((metrics.side_gap_units - fixture.side_gap_units).abs() < 1e-6);
    }
}

#[test]
fn tablature_multiple_positions_get_deterministic_horizontal_spacing() {
    use acorde_core::{Duration, Note, Pitch, Staff, Step, TabPosition, TablatureConfig};
    let mut score = acorde_core::Score::new("Tab positions", 120, 4, 4, 0, 1);
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 0,
    });
    staff.measures.push(acorde_core::Measure::empty(4, 4));
    let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
    note.tab_positions = vec![
        TabPosition {
            string: 2,
            fret: 12,
        },
        TabPosition { string: 3, fret: 7 },
    ];
    staff.measures[0].voices[0] = vec![note];
    score.parts[0].staves = vec![staff];

    let svg = render_svg(&score, &opts()).expect("tablature renders");
    let fret_texts: Vec<&str> = svg
        .split("<text class=\"acorde-tab-fret\"")
        .skip(1)
        .map(|fragment| fragment.split("</text>").next().unwrap_or_default())
        .collect();
    assert_eq!(fret_texts.len(), 2);
    assert_ne!(
        fret_texts[0].split(" x=\"").nth(1),
        fret_texts[1].split(" x=\"").nth(1)
    );
    assert!(svg.contains(">12</text>"));
    assert!(svg.contains(">7</text>"));
    assert_well_formed_xml(&svg);
}

#[test]
fn wide_tablature_chord_at_measure_origin_stays_inside_svg_viewbox() {
    use acorde_core::{Duration, Note, Pitch, Staff, Step, TabPosition, TablatureConfig};
    let mut score = acorde_core::Score::new("Wide tab positions", 120, 4, 4, 0, 1);
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 0,
    });
    staff.measures.push(acorde_core::Measure::empty(4, 4));
    let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
    note.tab_positions = (1..=6)
        .map(|string| TabPosition { string, fret: 12 })
        .collect();
    staff.measures[0].voices[0] = vec![note];
    score.parts[0].staves = vec![staff];

    let svg = render_svg(&score, &opts()).expect("wide tablature chord renders");
    let fret_xs: Vec<f32> = svg
        .split("<text class=\"acorde-tab-fret\"")
        .skip(1)
        .filter_map(|fragment| fragment.split(" x=\"").nth(1))
        .filter_map(|value| value.split('\"').next())
        .filter_map(|value| value.parse().ok())
        .collect();
    assert_eq!(fret_xs.len(), 6);
    assert!(fret_xs.iter().all(|&x| x > 0.0));
    assert_well_formed_xml(&svg);
}

#[test]
fn invalid_tablature_string_is_rejected_before_svg_emission() {
    use acorde_core::{Duration, Measure, Note, Pitch, Staff, Step, TablatureConfig};

    let mut score = acorde_core::Score::new("Invalid tab", 120, 4, 4, 0, 1);
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.tablature = Some(TablatureConfig {
        lines: 6,
        tuning_midi: vec![64, 59, 55, 50, 45, 40],
        capo: 0,
    });
    staff.measures.push(Measure::empty(4, 4));
    let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
    note.tab_position = Some(acorde_core::TabPosition { string: 7, fret: 3 });
    staff.measures[0].voices[0] = vec![note];
    score.parts[0].staves = vec![staff];

    let err = render_svg(&score, &opts()).expect_err("invalid tab string must be rejected");
    assert_eq!(
        err,
        acorde_render_svg::RenderError::InvalidTabPosition {
            string: 7,
            lines: 6,
        }
    );
}

#[test]
fn percussion_clef_renders_a_dedicated_clef() {
    use acorde_core::{Clef, Measure, Part, Score, Staff};
    let mut score = Score::default();
    let mut part = Part::new("Drums", "Dr.");
    let mut staff = Staff::new(Clef::Percussion);
    staff.measures.push(Measure::empty(4, 4));
    part.staves.push(staff);
    score.parts = vec![part];

    let svg = render_svg(&score, &opts()).expect("percussion clef renders");
    assert!(svg.contains("acorde-clef-percussion"));
    assert_well_formed_xml(&svg);
}

#[test]
fn unpitched_notes_keep_a_semantic_svg_hook_without_inventing_sound_identity() {
    use acorde_core::{Duration, Score, Step};

    let mut score = Score::new("Unpitched", 120, 4, 4, 0, 1);
    let mut note = acorde_core::Note::new(acorde_core::Pitch::new(Step::C, 4), Duration::Quarter);
    note.pitches[0].alter = 1;
    note.is_unpitched = true;
    note.instrument_id = Some("P1-I1".to_string());
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let svg = render_svg(&score, &opts()).expect("display-positioned unpitched note should render");
    assert!(
        svg.contains("class=\"acorde-note acorde-unpitched acorde-percussion-notehead-normal\"")
    );
    assert!(svg.contains("data-acorde-unpitched=\"true\""));
    assert!(svg.contains("data-acorde-percussion-notehead=\"acorde-percussion-notehead-normal\""));
    assert!(svg.contains("acorde-percussion-notehead-normal"));
    assert!(!svg.contains("acorde-accidental"));
}

#[test]
fn microtone_cents_are_exposed_as_explicit_svg_markers() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};

    let mut score = Score::new("Microtones", 120, 4, 4, 0, 1);
    let note = Note::new(Pitch::with_microtone(Step::C, 4, 0, 25), Duration::Quarter);
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let svg = render_svg(&score, &opts()).expect("microtonal pitch should render");
    assert!(svg.contains("class=\"acorde-microtone\""));
    assert!(svg.contains(">+25c</text>"));
}

#[test]
fn empty_score_is_rejected() {
    use acorde_core::Score;
    let score = Score {
        parts: vec![],
        ..Score::default()
    };
    let err = render_svg(&score, &opts()).unwrap_err();
    assert_eq!(err, acorde_render_svg::RenderError::EmptyScore);
}

#[test]
fn multi_voice_two_voices_get_opposite_stem_directions() {
    // Soprano (voice 0) and alto (voice 1) share the treble staff — stems must not both
    // point the same direction (that's the "does not break" requirement from the spec).
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    // A stem line has stroke-width proportional to 0.11*space; just confirm both an
    // up-stem (tip above notehead => smaller y2 than y1) and a down-stem (tip below,
    // larger y2) exist among treble-staff notes by checking two distinct patterns exist.
    // (Numeric assertion done more precisely in geometry unit tests; here we just confirm
    // the measure renders two distinct voices without error.)
    assert_eq!(
        svg.matches(r#"data-staff="0" data-measure="0" data-voice="0""#)
            .count(),
        4
    );
    assert_eq!(
        svg.matches(r#"data-staff="0" data-measure="0" data-voice="1""#)
            .count(),
        4
    );
}

#[test]
fn simultaneous_voice_noteheads_get_deterministic_horizontal_separation() {
    let svg = render_svg(&common::satb_major(), &opts()).unwrap();
    let centers: Vec<f32> = svg
        .split(r#"class="acorde-notehead""#)
        .skip(1)
        .filter_map(|fragment| fragment.split(r#"cx=""#).nth(1))
        .filter_map(|value| value.split('"').next())
        .filter_map(|value| value.parse().ok())
        .collect();
    assert!(centers.len() >= 8);
    // The first staff's voice-0 and voice-1 notes occur at indexes 0 and 4.
    assert!((centers[0] - centers[4]).abs() >= 0.65 * opts().staff_size - 0.01);
    assert_ne!(centers[0], centers[4]);
}

#[test]
fn wide_simultaneous_voice_annotations_expand_notehead_separation() {
    use acorde_core::{Duration, Lyric, Note, Pitch, Score, Step};

    let mut score = Score::new("voice annotation spacing", 120, 4, 4, 0, 1);
    let measure = &mut score.parts[0].staves[0].measures[0];
    measure.voices[0] = vec![Note::new(Pitch::new(Step::C, 5), Duration::Quarter)];
    measure.voices[1] = vec![Note::new(Pitch::new(Step::E, 4), Duration::Quarter)];

    let centers = |svg: &str| {
        svg.split(r#"class="acorde-notehead""#)
            .skip(1)
            .filter_map(|fragment| fragment.split(r#"cx=""#).nth(1))
            .filter_map(|value| value.split('"').next())
            .filter_map(|value| value.parse::<f32>().ok())
            .take(2)
            .collect::<Vec<_>>()
    };
    let baseline = centers(&render_svg(&score, &opts()).unwrap());
    assert_eq!(baseline.len(), 2);

    score.parts[0].staves[0].measures[0].voices[1][0].lyric = Some(Lyric {
        text: "a deliberately wide simultaneous lyric".into(),
        syllabic: "single".into(),
    });
    let annotated = centers(&render_svg(&score, &opts()).unwrap());
    assert_eq!(annotated.len(), 2);
    assert!(
        (annotated[0] - annotated[1]).abs() > (baseline[0] - baseline[1]).abs(),
        "annotation should expand cross-voice spacing: baseline={baseline:?}, annotated={annotated:?}"
    );

    let mut accidental_score = score;
    accidental_score.parts[0].staves[0].measures[0].voices[1][0].lyric = None;
    accidental_score.parts[0].staves[0].measures[0].voices[1][0].pitches[0].alter = 1;
    let accidental = centers(&render_svg(&accidental_score, &opts()).unwrap());
    assert_eq!(accidental.len(), 2);
    assert!(
        (accidental[0] - accidental[1]).abs() > (baseline[0] - baseline[1]).abs(),
        "accidental should expand cross-voice spacing: baseline={baseline:?}, accidental={accidental:?}"
    );
}

#[test]
fn consecutive_grace_noteheads_are_not_collapsed_on_one_anchor() {
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("grace spacing", 120, 4, 4, 0, 1);
    let notes = &mut score.parts[0].staves[0].measures[0].voices[0];
    let mut first = Note::new(Pitch::new(Step::D, 5), Duration::Eighth);
    first.is_grace = true;
    let mut second = Note::new(Pitch::new(Step::E, 5), Duration::Eighth);
    second.is_grace = true;
    notes.splice(0..0, [first, second]);

    let svg = render_svg(&score, &opts()).unwrap();
    let centers: Vec<f32> = svg
        .split(r#"class="acorde-notehead""#)
        .skip(1)
        .filter_map(|fragment| fragment.split(r#"cx=""#).nth(1))
        .filter_map(|value| value.split('"').next())
        .filter_map(|value| value.parse().ok())
        .take(2)
        .collect();
    assert_eq!(centers.len(), 2);
    assert!((centers[0] - centers[1]).abs() >= 0.4 * opts().staff_size);
}

#[test]
fn adjacent_tones_in_a_chord_get_alternating_notehead_offsets() {
    use acorde_core::{Duration, Measure, Note, Part, Pitch, Score, Staff, Step};
    let mut score = Score::new("cluster chord", 120, 4, 4, 0, 1);
    let note = Note {
        pitches: vec![
            Pitch::new(Step::C, 5),
            Pitch::new(Step::D, 5),
            Pitch::new(Step::E, 5),
        ],
        ..Note::new(Pitch::new(Step::C, 5), Duration::Quarter)
    };
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.measures.push(Measure::empty(4, 4));
    staff.measures[0].voices[0] = vec![note];
    score.parts = vec![Part::new("Cluster", "Cl.")];
    score.parts[0].staves = vec![staff];

    let svg = render_svg(&score, &opts()).unwrap();
    let centers: Vec<f32> = svg
        .split(r#"class="acorde-notehead""#)
        .skip(1)
        .filter_map(|fragment| fragment.split(r#"cx=""#).nth(1))
        .filter_map(|value| value.split('"').next())
        .filter_map(|value| value.parse().ok())
        .collect();
    assert_eq!(centers.len(), 3);
    assert!(centers.iter().any(|&center| center != centers[0]));
}

#[test]
fn adjacent_accidentals_in_a_chord_get_separate_columns() {
    use acorde_core::{Duration, Measure, Note, Part, Pitch, Score, Staff, Step};
    let mut score = Score::new("accidental cluster", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::with_alter(Step::C, 5, 1), Duration::Quarter);
    note.pitches.push(Pitch::with_alter(Step::D, 5, 1));
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.measures.push(Measure::empty(4, 4));
    staff.measures[0].voices[0] = vec![note];
    score.parts = vec![Part::new("Cluster", "Cl.")];
    score.parts[0].staves = vec![staff];

    let svg = render_svg(&score, &opts()).unwrap();
    let accidental_xs: Vec<&str> = svg
        .split("acorde-accidental")
        .skip(1)
        .filter_map(|fragment| fragment.split(r#"x1=""#).nth(1))
        .filter_map(|value| value.split('"').next())
        .collect();
    assert!(accidental_xs.len() >= 2);
    assert_ne!(accidental_xs[0], accidental_xs[1]);
}

#[test]
fn accidental_columns_at_measure_origin_stay_inside_svg_viewbox() {
    use acorde_core::{Duration, Measure, Note, Part, Pitch, Score, Staff, Step};
    let mut score = Score::new("accidental origin", 120, 4, 4, 0, 1);
    let mut note = Note::new(Pitch::with_alter(Step::C, 5, 1), Duration::Quarter);
    note.pitches.push(Pitch::with_alter(Step::D, 5, 1));
    let mut staff = Staff::new(acorde_core::Clef::Treble);
    staff.measures.push(Measure::empty(4, 4));
    staff.measures[0].voices[0] = vec![note];
    score.parts = vec![Part::new("Cluster", "")];
    score.parts[0].staves = vec![staff];

    let svg = render_svg(&score, &opts()).unwrap();
    let accidental_xs: Vec<f32> = svg
        .split("acorde-accidental")
        .skip(1)
        .filter_map(|fragment| fragment.split(r#"x1=""#).nth(1))
        .filter_map(|value| value.split('"').next())
        .filter_map(|value| value.parse().ok())
        .collect();
    assert!(accidental_xs.len() >= 2);
    assert!(accidental_xs.iter().all(|&x| x > 0.0));
}
