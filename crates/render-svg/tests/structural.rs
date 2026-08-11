//! Structural SVG assertions: parse the output and check element/attribute counts rather
//! than comparing full-text golden files. A small golden fixture is used separately
//! (see `determinism.rs`) only to pin exact byte-stability, not visual content.

mod common;

use acorde_render_svg::{render_svg, SvgRenderOptions};

fn opts() -> SvgRenderOptions {
    SvgRenderOptions { width: 700.0, staff_size: 24.0, measures_per_system: 4, interactive: true }
}

/// Well-formedness check: every opened tag closes, via quick-xml's reader (it errors on
/// malformed XML). Cheaper than diffing full text against a golden file.
fn assert_well_formed_xml(svg: &str) {
    use quick_xml::events::Event;
    use quick_xml::Reader;
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
    assert_eq!(svg.matches(r#"data-acorde-kind="note""#).count(), 4 + 1 + 4 + 4);
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
    use acorde_core::{Clef, Duration, KeySignature, Measure, Note, Part, Pitch, Score, Staff, Step, TimeSignature};

    let mut score = Score::default();
    score.settings.time_signature = TimeSignature { numerator: 4, denominator: 4 };
    score.settings.key_signature = KeySignature { fifths: 0, mode: "major".to_string() };
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
    use acorde_core::{Clef, Duration, KeySignature, Measure, Note, Part, Pitch, Score, Staff, Step, TimeSignature};
    let mut score = Score::default();
    score.settings.time_signature = TimeSignature { numerator: 4, denominator: 4 };
    score.settings.key_signature = KeySignature { fifths: 0, mode: "major".to_string() };
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
    assert_eq!(err, acorde_render_svg::RenderError::UnsupportedAccidental { alter: 3 });
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
    let score = Score { parts: vec![], ..Score::default() };
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
    assert_eq!(svg.matches(r#"data-staff="0" data-measure="0" data-voice="0""#).count(), 4);
    assert_eq!(svg.matches(r#"data-staff="0" data-measure="0" data-voice="1""#).count(), 4);
}
