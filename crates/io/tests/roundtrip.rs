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
static JUST_PERFECT_FIFTH_MIDI: &[u8] =
    include_bytes!("../../../tests/fixtures/just_perfect_fifth_on_c.mid");

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
    score.parts[0].staves[0].measures[0].voices[0] = vec![note];

    let xml = serialize_musicxml(&score).expect("serialize");
    assert!(xml.contains("<bend>"), "bend in XML");

    let score2 = parse_musicxml(&xml).expect("parse");
    assert_eq!(
        score2.parts[0].staves[0].measures[0].voices[0][0].guitar_technique,
        Some(GuitarTechnique::Bend)
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
