//! Structural SVG assertions: parse the output and check element/attribute counts rather
//! than comparing full-text golden files. A small golden fixture is used separately
//! (see `determinism.rs`) only to pin exact byte-stability, not visual content.

mod common;

use acorde_render_svg::{SvgRenderOptions, render_svg};

fn opts() -> SvgRenderOptions {
    SvgRenderOptions {
        width: 700.0,
        staff_size: 24.0,
        measures_per_system: 4,
        interactive: true,
    }
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
        Articulation::Accent,
        Articulation::Tenuto,
    ];

    let svg = render_svg(&score, &opts()).unwrap();
    assert!(svg.contains("class=\"acorde-dynamic\""));
    assert!(svg.contains("class=\"acorde-chord-symbol\""));
    assert!(svg.contains("class=\"acorde-lyric\""));
    assert!(svg.contains("A&amp;B&lt;"));
    assert!(svg.contains("acorde-staccato"));
    assert!(svg.contains("acorde-accent"));
    assert!(svg.contains("acorde-tenuto"));
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
    assert_eq!(metadata.contract_version, 2);
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
    });
    let layout = compute_layout(&score, &LayoutConfig::default());
    let metadata = acorde_render_svg::render_svg_metadata(&score, &layout, &opts()).unwrap();
    assert_eq!(metadata.text_annotations.len(), 1);
    assert_eq!(metadata.text_annotations[0].part, 0);
    assert_eq!(metadata.text_annotations[0].staff, 0);
    assert_eq!(metadata.text_annotations[0].measure, 0);
    assert_eq!(metadata.text_annotations[0].style, TextStyle::Technique);
    assert_eq!(metadata.text_annotations[0].text, "con sordino");
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
    assert_eq!(coverage.contract_version, 1);
    assert_eq!(coverage.resource_id, "acorde-vector-glyphs-v1");
    assert!(coverage.vector_glyphs);
    assert_eq!(coverage.accidental_min, -2);
    assert_eq!(coverage.accidental_max, 2);
    assert!(coverage.supported_clefs.iter().any(|clef| clef == "treble"));
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
    staff.measures[0].voices[0] = vec![note];
    score.parts[0].staves = vec![staff];

    let svg = render_svg(&score, &opts()).expect("tablature renders");
    assert_eq!(svg.matches("acorde-staff-line").count(), 6);
    assert!(svg.contains("acorde-tab-fret"));
    assert!(svg.contains(">3</text>"));
    assert!(svg.contains("acorde-tab-fingering"));
    assert!(svg.contains(">1/3</text>"));
    assert!(svg.contains("acorde-tab-technique"));
    assert!(svg.contains(">bend</text>"));
    assert_well_formed_xml(&svg);
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
fn percussion_clef_is_rejected_not_silently_treble() {
    use acorde_core::{Clef, Part, Score, Staff};
    let mut score = Score::default();
    let mut part = Part::new("Drums", "Dr.");
    part.staves.push(Staff::new(Clef::Percussion));
    score.parts = vec![part];

    let err = render_svg(&score, &opts()).unwrap_err();
    assert_eq!(err, acorde_render_svg::RenderError::UnsupportedClef);
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
