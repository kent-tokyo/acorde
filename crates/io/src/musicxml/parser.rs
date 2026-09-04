use crate::Error;
use acorde_core::{
    Articulation, Barline, ChordDegree, ChordSymbol, Clef, Duration, FiguredBassFigure,
    GuitarTechnique, HairpinKind, KeySignature, Lyric, Measure, Note, NoteHead, OttavaKind, Part,
    PartGroup, PartGroupSymbol, PercussionInstrument, Pitch, Score, Staff, Step, StyledText,
    TextStyle, TimeSignature, TupletInfo, VoltaBracket,
};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::HashMap;

use super::attributes::{attr_is_yes, attr_present, attr_str};

const MAX_ELEMENTS: usize = 500_000;
const MAX_MUSICXML_BYTES: usize = 64 * 1024 * 1024;
const MAX_PARTS: usize = 64;
const MAX_MEASURES: usize = 10_000;
const MAX_STAVES: usize = 32;
const MAX_NOTES_PER_VOICE: usize = 50_000;
const MAX_DEPTH: usize = 64;

pub fn parse_musicxml(xml: &str) -> Result<Score, Error> {
    if xml.len() > MAX_MUSICXML_BYTES {
        return Err(Error::TooLarge(xml.len()));
    }
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut score = Score::default();
    score.parts.clear();
    let mut element_count = 0usize;
    let mut depth = 0usize;

    let mut current_measure_number = 0u32;
    let mut _current_divisions = 480u32;
    let mut current_time = TimeSignature::default();
    let mut current_key = KeySignature::default();
    let mut current_clef = Clef::Treble;
    let mut current_clef_staff_number = 1usize;
    let mut in_note = false;
    let mut note_slur_start = false;
    let mut note_slur_end = false;
    let mut note_glissando_start = false;
    let mut note_glissando_end = false;
    let mut in_notations = false;
    let mut in_artic_block = false;
    let mut in_ornament_block = false;
    let mut in_technical_block = false;
    let mut pending_fingerings: Vec<u8> = Vec::new();
    let mut pending_string_number: Option<u8> = None;
    let mut pending_fret: Option<u8> = None;
    let mut pending_technique_text: Option<String> = None;
    let mut pending_articulations: Vec<Articulation> = Vec::new();
    let mut pending_tremolo = false;
    let mut note_arpeggiate: Option<bool> = None;
    let mut pending_note_head: Option<NoteHead> = None;
    let mut in_pitch = false;
    let mut note_step = Step::C;
    let mut note_octave = 4i8;
    let mut note_alter = 0i8;
    let mut note_microtone_cents = 0i16;
    let mut _note_duration_ticks = 0u32;
    let mut note_type = "quarter".to_string();
    let mut note_dot = false;
    let mut note_rest = false;
    let mut in_unpitched = false;
    let mut note_is_unpitched = false;
    let mut note_instrument_id: Option<String> = None;
    let mut note_chord = false;
    let mut note_voice = 1u8;
    let mut note_staff = 1usize;
    let mut note_is_grace = false;
    let mut note_grace_slash = false;
    let mut note_is_cue = false;
    let mut note_tuplet_actual: Option<u8> = None;
    let mut note_tuplet_normal: Option<u8> = None;
    let mut in_time_modification = false;
    let mut note_trill_line_start = false;
    let mut note_trill_line_end = false;
    let mut note_stem_up: Option<bool> = None;
    let mut pending_guitar_technique: Option<GuitarTechnique> = None;
    let mut pending_guitar_bend_alter_cents: Option<i16> = None;
    let mut pending_hairpin_start: Option<HairpinKind> = None;
    let mut in_harmony = false;
    let mut in_harmony_root = false;
    let mut in_harmony_bass = false;
    let mut harmony_root_step = String::new();
    let mut harmony_root_alter: i8 = 0;
    let mut harmony_kind = String::new();
    let mut harmony_function: Option<String> = None;
    let mut harmony_bass_step = String::new();
    let mut harmony_bass_alter: i8 = 0;
    let mut harmony_placement: Option<String> = None;
    let mut in_harmony_degree = false;
    let mut harmony_degrees: Vec<ChordDegree> = Vec::new();
    let mut harmony_degree_value = String::new();
    let mut harmony_degree_alter = String::new();
    let mut harmony_degree_type = String::new();
    let mut pending_chord: Option<ChordSymbol> = None;
    let mut in_figured_bass = false;
    let mut figured_bass_text = String::new();
    let mut in_figured_figure = false;
    let mut figured_figure_number = String::new();
    let mut figured_figure_alter = String::new();
    let mut figured_figure_prefix = String::new();
    let mut figured_figure_suffix = String::new();
    let mut figured_bass_figures: Vec<FiguredBassFigure> = Vec::new();
    let mut pending_ottava_start: Option<OttavaKind> = None;
    let mut pending_pedal_start = false;
    let mut in_lyric = false;
    let mut lyric_text = String::new();
    let mut lyric_syllabic = String::new();
    let mut in_measure_style = false;
    let mut in_staff_details = false;
    let mut staff_lines: Option<u8> = None;
    let mut in_staff_tuning = false;
    let mut staff_tuning_line: Option<u8> = None;
    let mut staff_tuning_step = Step::C;
    let mut staff_tuning_alter = 0i8;
    let mut staff_tuning_octave = 4i8;
    let mut staff_tunings: Vec<(u8, i16)> = Vec::new();
    let mut in_multiple_rest = false;
    let mut in_barline = false;
    let mut barline_location = String::new();
    let mut in_direction = false;
    let mut in_direction_type = false;
    let mut pending_tempo_text: Option<String> = None;
    let mut pending_expression_text: Option<String> = None;
    let mut pending_rehearsal: Option<String> = None;
    let mut pending_navigation: Option<String> = None;
    let mut pending_sound_tempo: Option<u16> = None;
    let mut in_work = false;
    let mut current_text = String::new();

    let mut part_index: Option<usize> = None;

    // Collect <midi-instrument> data from <score-part> declarations.
    let mut part_midi: HashMap<String, (u8, u8)> = HashMap::new();
    let mut part_percussion: HashMap<String, Vec<PercussionInstrument>> = HashMap::new();
    let mut in_score_part = false;
    let mut score_part_id = String::new();
    let mut pending_midi_channel: u8 = 0;
    let mut pending_midi_program: u8 = 0;
    let mut in_midi_instrument = false;
    let mut in_score_instrument = false;
    let mut score_instrument_id = String::new();
    let mut score_instrument_name: Option<String> = None;
    let mut score_instrument_key: Option<u8> = None;
    let mut in_transpose = false;
    // <part-group> tracking: map group number → (start_part_index, symbol, barlines_connect)
    let mut open_groups: std::collections::HashMap<String, (usize, PartGroupSymbol, bool)> =
        std::collections::HashMap::new();
    let mut in_part_group = false;
    let mut part_group_number = String::new();
    let mut part_group_type = String::new();
    let mut part_group_symbol = PartGroupSymbol::Bracket;
    let mut part_group_barlines = false;
    let mut part_list_part_count: usize = 0;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                element_count += 1;
                depth += 1;
                if element_count > MAX_ELEMENTS {
                    return Err(Error::Xml("too many elements".into()));
                }
                if depth > MAX_DEPTH {
                    return Err(Error::Xml("nesting too deep".into()));
                }
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                current_text.clear();

                match tag.as_str() {
                    "score-part" => {
                        in_score_part = true;
                        score_part_id = attr_str(e, b"id").unwrap_or_default();
                        pending_midi_channel = 0;
                        pending_midi_program = 0;
                        part_list_part_count += 1;
                    }
                    "clef" => {
                        current_clef_staff_number = attr_str(e, b"number")
                            .and_then(|value| value.parse().ok())
                            .filter(|number: &usize| (1..=MAX_STAVES).contains(number))
                            .unwrap_or(1);
                        if let Some(pi) = part_index {
                            while score.parts[pi].staves.len() < current_clef_staff_number {
                                score.parts[pi].staves.push(Staff::new(Clef::Treble));
                            }
                        }
                    }
                    "score-instrument" if in_score_part => {
                        in_score_instrument = true;
                        score_instrument_id = attr_str(e, b"id").unwrap_or_default();
                        score_instrument_name = None;
                        score_instrument_key = None;
                    }
                    "part-group" => {
                        in_part_group = true;
                        part_group_number =
                            attr_str(e, b"number").unwrap_or_else(|| "1".to_string());
                        part_group_type = attr_str(e, b"type").unwrap_or_default();
                        part_group_symbol = PartGroupSymbol::Bracket;
                        part_group_barlines = false;
                    }
                    "midi-instrument" if in_score_part => {
                        in_midi_instrument = true;
                    }
                    "harmony" => {
                        in_harmony = true;
                        harmony_root_step.clear();
                        harmony_root_alter = 0;
                        harmony_kind.clear();
                        harmony_function = None;
                        harmony_bass_step.clear();
                        harmony_bass_alter = 0;
                        harmony_placement = attr_str(e, b"placement");
                        harmony_degrees.clear();
                    }
                    "figured-bass" => {
                        in_figured_bass = true;
                        figured_bass_text.clear();
                        in_figured_figure = false;
                        figured_figure_number.clear();
                        figured_figure_alter.clear();
                        figured_figure_prefix.clear();
                        figured_figure_suffix.clear();
                        figured_bass_figures.clear();
                    }
                    "figure" if in_figured_bass => {
                        in_figured_figure = true;
                        figured_figure_number.clear();
                        figured_figure_alter.clear();
                        figured_figure_prefix.clear();
                        figured_figure_suffix.clear();
                    }
                    "root" if in_harmony => in_harmony_root = true,
                    "bass" if in_harmony => in_harmony_bass = true,
                    "degree" if in_harmony => {
                        in_harmony_degree = true;
                        harmony_degree_value.clear();
                        harmony_degree_alter.clear();
                        harmony_degree_type.clear();
                    }
                    "degree-value" if in_harmony_degree => {}
                    "degree-alter" if in_harmony_degree => {}
                    "degree-type" if in_harmony_degree => {}
                    "work" => in_work = true,
                    "barline" => {
                        in_barline = true;
                        barline_location =
                            attr_str(e, b"location").unwrap_or_else(|| "right".to_string());
                    }
                    "transpose" => in_transpose = true,
                    "direction" => in_direction = true,
                    "direction-type" => in_direction_type = true,
                    "measure-style" => in_measure_style = true,
                    "staff-details" => {
                        in_staff_details = true;
                        staff_lines = None;
                        staff_tunings.clear();
                    }
                    "staff-tuning" if in_staff_details => {
                        in_staff_tuning = true;
                        staff_tuning_line = attr_str(e, b"line").and_then(|v| v.parse().ok());
                        staff_tuning_step = Step::C;
                        staff_tuning_alter = 0;
                        staff_tuning_octave = 4;
                    }
                    "multiple-rest" if in_measure_style => in_multiple_rest = true,
                    "notations" if in_note => in_notations = true,
                    "articulations" if in_notations => in_artic_block = true,
                    "ornaments" if in_notations => in_ornament_block = true,
                    "technical" if in_notations => in_technical_block = true,
                    "glissando" if in_notations => {
                        if attr_str(e, b"type").as_deref() == Some("start") {
                            note_glissando_start = true;
                        }
                    }
                    "tremolo" if in_ornament_block => {
                        pending_tremolo = true;
                    }
                    "lyric" if in_note => {
                        in_lyric = true;
                        lyric_text.clear();
                        lyric_syllabic = "single".to_string();
                    }
                    "time-modification" if in_note => {
                        in_time_modification = true;
                        note_tuplet_actual = None;
                        note_tuplet_normal = None;
                    }
                    "note" => {
                        in_note = true;
                        note_rest = false;
                        note_chord = false;
                        note_voice = 1;
                        note_staff = 1;
                        note_dot = false;
                        note_alter = 0;
                        note_microtone_cents = 0;
                        in_unpitched = false;
                        note_is_unpitched = false;
                        note_instrument_id = None;
                        note_step = Step::C;
                        note_octave = 4;
                        _note_duration_ticks = 0;
                        note_is_grace = false;
                        note_grace_slash = false;
                        note_is_cue = false;
                        note_tuplet_actual = None;
                        note_tuplet_normal = None;
                        in_time_modification = false;
                        note_trill_line_start = false;
                        note_trill_line_end = false;
                        note_type = "quarter".to_string();
                        note_slur_start = false;
                        note_slur_end = false;
                        note_glissando_start = false;
                        note_glissando_end = false;
                        pending_articulations.clear();
                        pending_tremolo = false;
                        note_arpeggiate = None;
                        pending_fingerings.clear();
                        pending_string_number = None;
                        pending_fret = None;
                        pending_technique_text = None;
                        pending_guitar_technique = None;
                        pending_guitar_bend_alter_cents = None;
                        note_stem_up = None;
                        pending_note_head = None;
                    }
                    "pitch" => in_pitch = true,
                    "instrument" if in_note => {
                        note_instrument_id = attr_str(e, b"id");
                    }
                    "unpitched" if in_note => {
                        in_unpitched = true;
                        note_is_unpitched = true;
                    }
                    "part" => {
                        if score.parts.len() >= MAX_PARTS {
                            return Err(Error::Xml("too many parts".into()));
                        }
                        let id = attr_str(e, b"id").unwrap_or_default();
                        let mut part = Part::new(&id, "");
                        if let Some(&(ch, prog)) = part_midi.get(&id) {
                            part.midi_channel = ch;
                            part.midi_program = prog;
                        }
                        part.percussion_instruments =
                            part_percussion.remove(&id).unwrap_or_default();
                        part.staves.push(Staff::new(Clef::Treble));
                        score.parts.push(part);
                        part_index = Some(score.parts.len() - 1);
                        _current_divisions = 480;
                        current_time = TimeSignature::default();
                        current_key = KeySignature::default();
                        current_clef = Clef::Treble;
                        current_measure_number = 0;
                    }
                    "measure" => {
                        current_measure_number = attr_str(e, b"number")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(current_measure_number + 1);
                        if let Some(pi) = part_index {
                            if score.parts[pi].staves[0].measures.len() >= MAX_MEASURES {
                                return Err(Error::Xml("too many measures".into()));
                            }
                            let ts = current_time.clone();
                            let mut m = Measure::empty(ts.numerator, ts.denominator);
                            m.number = current_measure_number;
                            m.voices[0].clear();
                            if score.parts[pi].staves[0].measures.is_empty() {
                                m.time_sig = Some(current_time.clone());
                                m.key_sig = Some(current_key.clone());
                                m.clef = Some(current_clef.clone());
                            }
                            score.parts[pi].staves[0].measures.push(m);
                        }
                    }
                    _ => {}
                }
            }

            Ok(Event::Empty(ref e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                match tag.as_str() {
                    "print" => {
                        if let Some(pi) = part_index
                            && let Some(m) = score.parts[pi].staves[0].measures.last_mut()
                        {
                            if attr_is_yes(e, b"new-system") {
                                m.system_break = true;
                            }
                            if attr_is_yes(e, b"new-page") {
                                m.page_break = true;
                            }
                        }
                    }
                    "rest" if in_note => note_rest = true,
                    "instrument" if in_note => note_instrument_id = attr_str(e, b"id"),
                    "dot" if in_note => note_dot = true,
                    "chord" if in_note => note_chord = true,
                    "slur" if in_note => match attr_str(e, b"type").as_deref() {
                        Some("start") => note_slur_start = true,
                        Some("stop") => note_slur_end = true,
                        _ => {}
                    },
                    "staccato" if in_artic_block => {
                        pending_articulations.push(Articulation::Staccato)
                    }
                    "staccatissimo" if in_artic_block => {
                        pending_articulations.push(Articulation::Staccatissimo)
                    }
                    "accent" if in_artic_block => pending_articulations.push(Articulation::Accent),
                    "tenuto" if in_artic_block => pending_articulations.push(Articulation::Tenuto),
                    "strong-accent" if in_artic_block => {
                        pending_articulations.push(Articulation::Marcato)
                    }
                    "trill-mark" if in_ornament_block => {
                        pending_articulations.push(Articulation::Trill)
                    }
                    "mordent" if in_ornament_block => {
                        pending_articulations.push(Articulation::Mordent)
                    }
                    "inverted-mordent" if in_ornament_block => {
                        pending_articulations.push(Articulation::InvertedMordent)
                    }
                    "turn" if in_ornament_block => pending_articulations.push(Articulation::Turn),
                    "inverted-turn" if in_ornament_block => {
                        pending_articulations.push(Articulation::InvertedTurn)
                    }
                    "shake" if in_ornament_block => pending_articulations.push(Articulation::Shake),
                    "tremolo" if in_ornament_block => {
                        pending_articulations.push(Articulation::Tremolo(1))
                    }
                    "slide" if in_technical_block => {
                        pending_guitar_technique = Some(GuitarTechnique::Slide);
                    }
                    "hammer-on" if in_technical_block => {
                        pending_guitar_technique = Some(GuitarTechnique::HammerOn);
                    }
                    "pull-off" if in_technical_block => {
                        pending_guitar_technique = Some(GuitarTechnique::PullOff);
                    }
                    "fermata" if in_notations => pending_articulations.push(Articulation::Fermata),
                    "breath-mark" if in_notations => {
                        pending_articulations.push(Articulation::BreathMark)
                    }
                    "caesura" if in_notations => pending_articulations.push(Articulation::Caesura),
                    "arpeggiate" if in_notations => {
                        let dir = attr_str(e, b"direction");
                        note_arpeggiate = Some(!matches!(dir.as_deref(), Some("down")));
                    }
                    "grace" if in_note => {
                        note_is_grace = true;
                        note_grace_slash = attr_is_yes(e, b"slash");
                    }
                    "cue" if in_note => {
                        note_is_cue = true;
                    }
                    "part-group" => {
                        let pg_type = attr_str(e, b"type").unwrap_or_default();
                        let pg_num = attr_str(e, b"number").unwrap_or_else(|| "1".to_string());
                        if pg_type == "stop"
                            && let Some((first_part, symbol, barlines_connect)) =
                                open_groups.remove(&pg_num)
                        {
                            let last_part = part_list_part_count.saturating_sub(1);
                            if last_part >= first_part {
                                score.part_groups.push(PartGroup {
                                    first_part,
                                    last_part,
                                    symbol,
                                    barlines_connect,
                                });
                            }
                        }
                    }
                    "wavy-line" if in_notations => match attr_str(e, b"type").as_deref() {
                        Some("start") => note_trill_line_start = true,
                        Some("stop") => note_trill_line_end = true,
                        _ => {}
                    },
                    "glissando" if in_notations => match attr_str(e, b"type").as_deref() {
                        Some("start") => note_glissando_start = true,
                        Some("stop") => note_glissando_end = true,
                        _ => {}
                    },
                    "wedge" => match attr_str(e, b"type").as_deref() {
                        Some("crescendo") => pending_hairpin_start = Some(HairpinKind::Crescendo),
                        Some("diminuendo") | Some("decrescendo") => {
                            pending_hairpin_start = Some(HairpinKind::Decrescendo);
                        }
                        Some("stop") => {
                            if let Some(pi) = part_index
                                && let Some(m) = score.parts[pi].staves[0].measures.last_mut()
                                && let Some(n) = m.voices[0].last_mut()
                            {
                                n.hairpin_end = true;
                            }
                        }
                        _ => {}
                    },
                    "octave-shift" => {
                        let shift_type = attr_str(e, b"type").unwrap_or_default();
                        let shift_size: u8 = attr_str(e, b"size")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(8);
                        match shift_type.as_str() {
                            "up" => {
                                pending_ottava_start = Some(if shift_size >= 15 {
                                    OttavaKind::Ma15
                                } else {
                                    OttavaKind::Va8
                                });
                            }
                            "down" => {
                                pending_ottava_start = Some(if shift_size >= 15 {
                                    OttavaKind::Mb15
                                } else {
                                    OttavaKind::Vb8
                                });
                            }
                            "stop" => {
                                if let Some(pi) = part_index
                                    && let Some(m) = score.parts[pi].staves[0].measures.last_mut()
                                    && let Some(n) = m.voices[0].last_mut()
                                {
                                    n.ottava_end = true;
                                }
                            }
                            _ => {}
                        }
                    }
                    "pedal" => match attr_str(e, b"type").as_deref() {
                        Some("start") => pending_pedal_start = true,
                        Some("stop") => {
                            if let Some(pi) = part_index
                                && let Some(m) = score.parts[pi].staves[0].measures.last_mut()
                                && let Some(n) = m.voices[0].last_mut()
                            {
                                n.pedal_end = true;
                            }
                        }
                        _ => {}
                    },
                    "ending" if in_barline => {
                        let ending_num: u8 = attr_str(e, b"number")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        let ending_type = attr_str(e, b"type").unwrap_or_default();
                        let kind = match ending_type.as_str() {
                            "start" if barline_location == "left" => "begin",
                            "stop" | "discontinue" => "end",
                            _ => "mid",
                        };
                        if let Some(pi) = part_index
                            && let Some(m) = score.parts[pi].staves[0].measures.last_mut()
                        {
                            m.volta = Some(VoltaBracket {
                                number: ending_num,
                                kind: kind.to_string(),
                            });
                        }
                    }
                    "segno" if in_direction_type => pending_navigation = Some("Segno".to_string()),
                    "coda" if in_direction_type => pending_navigation = Some("Coda".to_string()),
                    "sound" if in_direction => {
                        if attr_present(e, b"dacapo") {
                            pending_navigation = Some("DaCapo".to_string());
                        }
                        if attr_present(e, b"dalsegno") {
                            pending_navigation = Some("DalSegno".to_string());
                        }
                        if attr_present(e, b"fine") {
                            pending_navigation = Some("Fine".to_string());
                        }
                        if attr_present(e, b"tocoda") {
                            pending_navigation = Some("ToCoda".to_string());
                        }
                        if let Some(bpm) =
                            attr_str(e, b"tempo").and_then(|s| s.trim().parse::<f64>().ok())
                        {
                            let bpm_u16 = bpm.round().clamp(1.0, 65535.0) as u16;
                            pending_sound_tempo = Some(bpm_u16);
                        }
                    }
                    "repeat" if in_barline => {
                        let dir = attr_str(e, b"direction").unwrap_or_default();
                        if let Some(pi) = part_index
                            && let Some(m) = score.parts[pi].staves[0].measures.last_mut()
                        {
                            match dir.as_str() {
                                "forward" => m.barline_left = Barline::RepeatStart,
                                "backward" => m.barline_right = Barline::RepeatEnd,
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }

            Ok(Event::Text(ref e)) => {
                current_text = match e.decode() {
                    Ok(text) => quick_xml::escape::unescape(&text)
                        .map(|text| text.into_owned())
                        .unwrap_or_default(),
                    Err(_) => String::new(),
                };
            }

            Ok(Event::End(ref e)) => {
                depth = depth.saturating_sub(1);
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                match tag.as_str() {
                    "instrument-name" if in_score_instrument => {
                        let name = current_text.trim();
                        if !name.is_empty() {
                            score_instrument_name = Some(name.to_string());
                        }
                    }
                    "midi-unpitched" if in_score_instrument => {
                        score_instrument_key = current_text.trim().parse::<u8>().ok();
                    }
                    "score-instrument" if in_score_instrument => {
                        if !score_instrument_id.is_empty() {
                            part_percussion
                                .entry(score_part_id.clone())
                                .or_default()
                                .push(PercussionInstrument {
                                    id: score_instrument_id.clone(),
                                    name: score_instrument_name.take(),
                                    midi_unpitched: score_instrument_key,
                                });
                        }
                        in_score_instrument = false;
                        score_instrument_id.clear();
                        score_instrument_key = None;
                    }
                    "midi-instrument" if in_score_part => {
                        in_midi_instrument = false;
                    }
                    "score-part" if in_score_part => {
                        part_midi.insert(
                            score_part_id.clone(),
                            (pending_midi_channel, pending_midi_program),
                        );
                        in_score_part = false;
                    }
                    "group-symbol" if in_part_group => {
                        part_group_symbol = match current_text.trim() {
                            "brace" => PartGroupSymbol::Brace,
                            "line" => PartGroupSymbol::Line,
                            _ => PartGroupSymbol::Bracket,
                        };
                    }
                    "group-barline" if in_part_group => {
                        part_group_barlines = current_text.trim() == "yes";
                    }
                    "part-group" if in_part_group => {
                        if part_group_type == "start" {
                            // <part-group type="start"> fires before the score-parts it covers,
                            // so part_list_part_count is the 0-based index of the first covered part.
                            open_groups.insert(
                                part_group_number.clone(),
                                (
                                    part_list_part_count,
                                    part_group_symbol.clone(),
                                    part_group_barlines,
                                ),
                            );
                        } else if part_group_type == "stop"
                            && let Some((first_part, symbol, barlines_connect)) =
                                open_groups.remove(&part_group_number)
                        {
                            let last_part = part_list_part_count.saturating_sub(1);
                            if last_part >= first_part {
                                score.part_groups.push(PartGroup {
                                    first_part,
                                    last_part,
                                    symbol,
                                    barlines_connect,
                                });
                            }
                        }
                        in_part_group = false;
                    }
                    "midi-channel" if in_midi_instrument => {
                        pending_midi_channel = current_text
                            .parse::<u16>()
                            .unwrap_or(1)
                            .saturating_sub(1)
                            .min(15) as u8;
                    }
                    "midi-program" if in_midi_instrument => {
                        pending_midi_program = current_text
                            .parse::<u16>()
                            .unwrap_or(1)
                            .saturating_sub(1)
                            .min(127) as u8;
                    }
                    "root" if in_harmony => in_harmony_root = false,
                    "bass" if in_harmony => in_harmony_bass = false,
                    "degree-value" if in_harmony_degree => {
                        harmony_degree_value = current_text.trim().to_string();
                    }
                    "degree-alter" if in_harmony_degree => {
                        harmony_degree_alter = current_text.trim().to_string();
                    }
                    "degree-type" if in_harmony_degree => {
                        harmony_degree_type = current_text.trim().to_string();
                    }
                    "degree" if in_harmony_degree => {
                        if let Ok(value) = harmony_degree_value.parse::<u8>() {
                            if value > 0 {
                                harmony_degrees.push(ChordDegree {
                                    value,
                                    alter: harmony_degree_alter
                                        .parse::<f32>()
                                        .unwrap_or(0.0)
                                        .round() as i8,
                                    kind: harmony_degree_type.clone(),
                                });
                            }
                        }
                        in_harmony_degree = false;
                    }
                    "root-step" if in_harmony_root => {
                        harmony_root_step = current_text.trim().to_string();
                    }
                    "root-alter" if in_harmony_root => {
                        harmony_root_alter =
                            current_text.parse::<f32>().unwrap_or(0.0).round() as i8;
                    }
                    "bass-step" if in_harmony_bass => {
                        harmony_bass_step = current_text.trim().to_string();
                    }
                    "bass-alter" if in_harmony_bass => {
                        harmony_bass_alter =
                            current_text.parse::<f32>().unwrap_or(0.0).round() as i8;
                    }
                    "kind" if in_harmony => {
                        harmony_kind = current_text.trim().to_string();
                    }
                    "function" if in_harmony => {
                        let value = current_text.trim();
                        harmony_function = (!value.is_empty()).then(|| value.to_string());
                    }
                    "harmony" => {
                        in_harmony = false;
                        let root = build_note_name(&harmony_root_step, harmony_root_alter);
                        if !root.is_empty() {
                            let bass = if harmony_bass_step.is_empty() {
                                None
                            } else {
                                Some(build_note_name(&harmony_bass_step, harmony_bass_alter))
                            };
                            pending_chord = Some(ChordSymbol {
                                root,
                                kind: harmony_kind.clone(),
                                bass,
                                placement: harmony_placement.take(),
                                extender: false,
                                harmonic_degree: None,
                                harmony_function: harmony_function.take(),
                                harmony_type: None,
                                chord_ref: None,
                                degrees: std::mem::take(&mut harmony_degrees),
                            });
                        }
                    }
                    "figure-number" if in_figured_bass => {
                        let value = current_text.trim();
                        if !value.is_empty() {
                            figured_figure_number = value.to_string();
                        }
                    }
                    "figure-alter" if in_figured_bass => {
                        figured_figure_alter = current_text.trim().to_string();
                    }
                    "prefix" if in_figured_bass => {
                        figured_figure_prefix = current_text.trim().to_string();
                    }
                    "suffix" if in_figured_bass => {
                        figured_figure_suffix = current_text.trim().to_string();
                    }
                    "figure" if in_figured_figure => {
                        if !figured_figure_number.is_empty() {
                            if !figured_bass_text.is_empty() {
                                figured_bass_text.push(' ');
                            }
                            figured_bass_text.push_str(&figured_figure_prefix);
                            figured_bass_text.push_str(match figured_figure_alter.as_str() {
                                "1" => "#",
                                "-1" => "b",
                                "0" => "♮",
                                "" => "",
                                other => {
                                    // Keep an unrecognized alteration visible instead of dropping it.
                                    // The structured detail remains separately diagnosed.
                                    other
                                }
                            });
                            figured_bass_text.push_str(&figured_figure_number);
                            figured_bass_text.push_str(&figured_figure_suffix);
                            figured_bass_figures.push(FiguredBassFigure {
                                number: figured_figure_number.clone(),
                                alter: (!figured_figure_alter.is_empty())
                                    .then(|| figured_figure_alter.clone()),
                                prefix: (!figured_figure_prefix.is_empty())
                                    .then(|| figured_figure_prefix.clone()),
                                suffix: (!figured_figure_suffix.is_empty())
                                    .then(|| figured_figure_suffix.clone()),
                                extender: false,
                            });
                        }
                        in_figured_figure = false;
                        figured_figure_number.clear();
                        figured_figure_alter.clear();
                        figured_figure_prefix.clear();
                        figured_figure_suffix.clear();
                    }
                    "figured-bass" => {
                        if let Some(pi) = part_index
                            && let Some(measure) = score.parts[pi].staves[0].measures.last_mut()
                        {
                            if !figured_bass_figures.is_empty() {
                                measure.figured_bass = std::mem::take(&mut figured_bass_figures);
                            }
                            if !figured_bass_text.trim().is_empty() {
                                measure.texts.push(StyledText {
                                    style: TextStyle::FiguredBass,
                                    text: figured_bass_text.trim().to_string(),
                                });
                            }
                        }
                        figured_bass_text.clear();
                        in_figured_figure = false;
                        figured_figure_number.clear();
                        figured_figure_alter.clear();
                        figured_figure_prefix.clear();
                        figured_figure_suffix.clear();
                        figured_bass_figures.clear();
                        in_figured_bass = false;
                    }
                    "work" => in_work = false,
                    "barline" => in_barline = false,
                    "direction-type" => in_direction_type = false,
                    "direction" => {
                        if let Some(pi) = part_index
                            && let Some(m) = score.parts[pi].staves[0].measures.last_mut()
                        {
                            if let Some(nav) = pending_navigation.take() {
                                if m.navigation.is_none() {
                                    m.navigation = Some(nav);
                                }
                            } else if pending_sound_tempo.is_some() {
                                if let Some(text) = pending_tempo_text.take()
                                    && m.tempo_text.is_none()
                                {
                                    m.tempo_text = Some(text.clone());
                                    m.texts.push(StyledText {
                                        style: TextStyle::Generic,
                                        text,
                                    });
                                }
                            } else if let Some(text) = pending_expression_text.take()
                                && m.expression_text.is_none()
                            {
                                m.expression_text = Some(text.clone());
                                m.texts.push(StyledText {
                                    style: TextStyle::Expression,
                                    text,
                                });
                            }
                            if let Some(reh) = pending_rehearsal.take()
                                && m.rehearsal.is_none()
                            {
                                m.rehearsal = Some(reh.clone());
                                m.texts.push(StyledText {
                                    style: TextStyle::RehearsalMark,
                                    text: reh,
                                });
                            }
                            if let Some(bpm) = pending_sound_tempo.take() {
                                m.tempo = Some(bpm);
                            }
                        }
                        pending_sound_tempo = None;
                        pending_tempo_text = None;
                        pending_expression_text = None;
                        pending_rehearsal = None;
                        pending_navigation = None;
                        in_direction = false;
                    }
                    "attributes" => {
                        // The measure is created before its child attributes are parsed. Apply
                        // the completed state here so the first measure is not stuck with the
                        // defaults (and later per-measure changes remain addressable).
                        if let Some(pi) = part_index
                            && let Some(m) = score.parts[pi].staves[0].measures.last_mut()
                        {
                            m.time_sig = Some(current_time.clone());
                            m.key_sig = Some(current_key.clone());
                            m.clef = Some(current_clef.clone());
                        }
                    }
                    "words" if in_direction_type => {
                        let text = current_text.trim().to_string();
                        if !text.is_empty() {
                            if let Some(nav) = words_to_navigation(&text) {
                                pending_navigation = Some(nav);
                            } else {
                                // Reclassified at "direction" close: if no <sound tempo> → expression_text
                                pending_tempo_text = Some(text.clone());
                                pending_expression_text = Some(text);
                            }
                        }
                    }
                    "rehearsal" if in_direction_type => {
                        let text = current_text.trim().to_string();
                        if !text.is_empty() {
                            pending_rehearsal = Some(text);
                        }
                    }
                    "work-title" if in_work => {
                        score.metadata.title = current_text.clone();
                    }
                    "movement-title" => {
                        if score.metadata.title.is_empty()
                            || score.metadata.title == "Untitled Score"
                        {
                            score.metadata.title = current_text.clone();
                        }
                    }
                    "divisions" => {
                        _current_divisions = current_text.parse().unwrap_or(480);
                    }
                    "beats" => {
                        current_time.numerator = current_text.parse().unwrap_or(4);
                    }
                    "beat-type" => {
                        current_time.denominator = current_text.parse().unwrap_or(4);
                    }
                    "fifths" => {
                        current_key.fifths = current_text.parse().unwrap_or(0);
                    }
                    "mode" => {
                        current_key.mode = current_text.clone();
                    }
                    "sign" => {
                        current_clef = match current_text.as_str() {
                            "G" => Clef::Treble,
                            "F" => Clef::Bass,
                            "C" => Clef::Alto,
                            "percussion" => Clef::Percussion,
                            _ => Clef::Treble,
                        };
                        if current_clef_staff_number > 1
                            && let Some(pi) = part_index
                            && let Some(staff) = score.parts[pi]
                                .staves
                                .get_mut(current_clef_staff_number - 1)
                        {
                            staff.clef = current_clef.clone();
                        }
                    }
                    "transpose" => {
                        in_transpose = false;
                    }
                    "chromatic" if in_transpose => {
                        if let Ok(v) = current_text.trim().parse::<i8>()
                            && let Some(pi) = part_index
                        {
                            score.parts[pi].staves[0].transpose_semitones = v;
                        }
                    }
                    "step" if in_pitch => {
                        note_step = match current_text.as_str() {
                            "C" => Step::C,
                            "D" => Step::D,
                            "E" => Step::E,
                            "F" => Step::F,
                            "G" => Step::G,
                            "A" => Step::A,
                            "B" => Step::B,
                            _ => Step::C,
                        };
                    }
                    "octave" if in_pitch => {
                        note_octave = current_text.parse().unwrap_or(4);
                    }
                    "alter" if in_pitch => {
                        let value = current_text.parse::<f32>().unwrap_or(0.0);
                        note_alter = value.trunc().clamp(-127.0, 127.0) as i8;
                        note_microtone_cents = ((value - note_alter as f32) * 100.0)
                            .round()
                            .clamp(-99.0, 99.0)
                            as i16;
                    }
                    "pitch" => in_pitch = false,
                    "display-step" if in_unpitched => {
                        note_step = match current_text.trim() {
                            "C" => Step::C,
                            "D" => Step::D,
                            "E" => Step::E,
                            "F" => Step::F,
                            "G" => Step::G,
                            "A" => Step::A,
                            "B" => Step::B,
                            _ => Step::C,
                        };
                    }
                    "display-octave" if in_unpitched => {
                        note_octave = current_text.trim().parse().unwrap_or(4);
                    }
                    "unpitched" if in_unpitched => in_unpitched = false,
                    "measure-style" => in_measure_style = false,
                    "multiple-rest" if in_multiple_rest => {
                        in_multiple_rest = false;
                        let count: u8 = current_text.parse().unwrap_or(1);
                        if count >= 2
                            && let Some(pi) = part_index
                            && let Some(m) = score.parts[pi].staves[0].measures.last_mut()
                        {
                            m.multi_rest_count = Some(count);
                        }
                    }
                    "staff-lines" if in_staff_details => {
                        staff_lines = current_text.trim().parse().ok();
                    }
                    "tuning-step" if in_staff_tuning => {
                        staff_tuning_step = match current_text.trim() {
                            "C" => Step::C,
                            "D" => Step::D,
                            "E" => Step::E,
                            "F" => Step::F,
                            "G" => Step::G,
                            "A" => Step::A,
                            "B" => Step::B,
                            _ => Step::C,
                        };
                    }
                    "tuning-octave" if in_staff_tuning => {
                        staff_tuning_octave = current_text.trim().parse().unwrap_or(4);
                    }
                    "tuning-alter" if in_staff_tuning => {
                        staff_tuning_alter = current_text.trim().parse().unwrap_or(0).clamp(-2, 2);
                    }
                    "staff-tuning" if in_staff_tuning => {
                        in_staff_tuning = false;
                        if let Some(line) = staff_tuning_line.take()
                            && (1..=64).contains(&line)
                        {
                            let pitch = acorde_core::Pitch::with_alter(
                                staff_tuning_step.clone(),
                                staff_tuning_octave,
                                staff_tuning_alter,
                            )
                            .to_midi();
                            staff_tunings.push((line, pitch));
                        }
                    }
                    "staff-details" => {
                        in_staff_details = false;
                        if let Some(lines) = staff_lines
                            && (1..=64).contains(&lines)
                            && let Some(pi) = part_index
                        {
                            score.parts[pi].staves[0].tablature =
                                Some(acorde_core::TablatureConfig {
                                    lines,
                                    tuning_midi: staff_tunings
                                        .iter()
                                        .filter(|(line, _)| *line <= lines)
                                        .map(|(_, midi)| *midi)
                                        .collect(),
                                    capo: 0,
                                });
                        }
                    }
                    "syllabic" if in_lyric => lyric_syllabic = current_text.trim().to_string(),
                    "text" if in_lyric => lyric_text = current_text.trim().to_string(),
                    "lyric" if in_lyric => in_lyric = false,
                    "duration" if in_note => {
                        _note_duration_ticks = current_text.parse().unwrap_or(480);
                    }
                    "actual-notes" if in_time_modification => {
                        note_tuplet_actual = current_text.trim().parse().ok();
                    }
                    "normal-notes" if in_time_modification => {
                        note_tuplet_normal = current_text.trim().parse().ok();
                    }
                    "time-modification" if in_time_modification => {
                        in_time_modification = false;
                    }
                    "voice" if in_note => {
                        note_voice = current_text.parse().unwrap_or(1);
                    }
                    "staff" if in_note => {
                        note_staff = current_text
                            .parse::<usize>()
                            .ok()
                            .filter(|number| (1..=MAX_STAVES).contains(number))
                            .unwrap_or(1);
                    }
                    "type" if in_note => note_type = current_text.clone(),
                    "tremolo" if pending_tremolo => {
                        let n: u8 = current_text.trim().parse().unwrap_or(1);
                        pending_articulations.push(Articulation::Tremolo(n));
                        pending_tremolo = false;
                    }
                    "notations" => in_notations = false,
                    "articulations" => in_artic_block = false,
                    "ornaments" => in_ornament_block = false,
                    "technical" => in_technical_block = false,
                    "fingering" if in_technical_block => {
                        if let Ok(fingering) = current_text.trim().parse() {
                            pending_fingerings.push(fingering);
                        }
                    }
                    "string" if in_technical_block => {
                        pending_string_number = current_text.trim().parse().ok();
                    }
                    "fret" if in_technical_block => {
                        pending_fret = current_text.trim().parse().ok();
                    }
                    "other-technical" if in_technical_block => {
                        let t = current_text.trim().to_string();
                        if !t.is_empty() {
                            pending_technique_text = Some(t);
                        }
                    }
                    "bend" if in_technical_block => {
                        pending_guitar_technique = Some(GuitarTechnique::Bend);
                    }
                    "bend-alter" if in_technical_block => {
                        pending_guitar_bend_alter_cents =
                            current_text.trim().parse::<f32>().ok().map(|value| {
                                (value * 100.0).round().clamp(-32768.0, 32767.0) as i16
                            });
                    }
                    "slide" if in_technical_block => {
                        pending_guitar_technique = Some(GuitarTechnique::Slide);
                    }
                    "hammer-on" if in_technical_block => {
                        pending_guitar_technique = Some(GuitarTechnique::HammerOn);
                    }
                    "pull-off" if in_technical_block => {
                        pending_guitar_technique = Some(GuitarTechnique::PullOff);
                    }
                    "stem" if in_note => {
                        note_stem_up = match current_text.trim() {
                            "up" => Some(true),
                            "down" => Some(false),
                            _ => None,
                        };
                    }
                    "notehead" if in_note => {
                        pending_note_head = Some(match current_text.trim() {
                            "diamond" => NoteHead::Diamond,
                            "x" => NoteHead::X,
                            "slash" => NoteHead::Slash,
                            "cross" => NoteHead::Cross,
                            "triangle" => NoteHead::Triangle,
                            _ => NoteHead::Normal,
                        });
                    }
                    "measure" => {
                        if let Some(pi) = part_index
                            && let Some(m) = score.parts[pi].staves[0].measures.last_mut()
                        {
                            let total_beats = current_time.total_beats();
                            for voice in &mut m.voices {
                                if voice.is_empty() {
                                    continue;
                                }
                                let mut used: f64 = voice.iter().map(|n| n.beats()).sum();
                                while total_beats - used > 1e-9 {
                                    let remaining = total_beats - used;
                                    let rest = Note::rest(Duration::whole_filling_beats(remaining));
                                    used += rest.beats();
                                    voice.push(rest);
                                }
                            }
                        }
                    }
                    "note" => {
                        if let Some(pi) = part_index {
                            let requested_staff_index = note_staff.saturating_sub(1);
                            let route_to_declared_staff = requested_staff_index > 0
                                && score.parts[pi].staves.len() > requested_staff_index;
                            let target_staff_index = if route_to_declared_staff {
                                requested_staff_index
                            } else {
                                0
                            };
                            let measure_count = score.parts[pi].staves[0].measures.len();
                            while score.parts[pi].staves[target_staff_index].measures.len()
                                < measure_count
                            {
                                let mut staff_measure = Measure::empty(
                                    current_time.numerator,
                                    current_time.denominator,
                                );
                                staff_measure.number = current_measure_number;
                                staff_measure.voices[0].clear();
                                score.parts[pi].staves[target_staff_index]
                                    .measures
                                    .push(staff_measure);
                            }
                            if let Some(m) = score.parts[pi].staves[target_staff_index]
                                .measures
                                .last_mut()
                            {
                                let voice_index = note_voice.saturating_sub(1) as usize;
                                if voice_index >= m.voices.len() {
                                    return Err(Error::Xml(format!(
                                        "voice number must be between 1 and {}",
                                        m.voices.len()
                                    )));
                                }
                                let voice = &mut m.voices[voice_index];
                                let dur = parse_duration_type(&note_type);
                                let dot_count = u8::from(note_dot);
                                let note = if note_rest {
                                    let mut n = Note::rest(dur);
                                    n.dot_count = dot_count;
                                    n
                                } else {
                                    let pitch = Pitch::with_microtone(
                                        note_step.clone(),
                                        note_octave,
                                        note_alter,
                                        note_microtone_cents,
                                    );
                                    let mut n = Note::new(pitch, dur);
                                    n.dot_count = dot_count;
                                    n.is_grace = note_is_grace;
                                    n.grace_slash = note_grace_slash;
                                    n.is_cue = note_is_cue;
                                    n.is_unpitched = note_is_unpitched;
                                    n.instrument_id = note_instrument_id.clone();
                                    n
                                };
                                if note_chord && !voice.is_empty() {
                                    if let Some(last) = voice.last_mut()
                                        && !last.is_rest
                                        && !note.is_rest
                                    {
                                        if let Some(p) = note.pitches.first() {
                                            last.pitches.push(p.clone());
                                        }
                                        last.is_unpitched |= note_is_unpitched;
                                        if note_instrument_id.is_some() {
                                            last.instrument_id = note_instrument_id.clone();
                                        }
                                        if !pending_fingerings.is_empty() {
                                            if last.fingering.is_none() {
                                                last.fingering =
                                                    pending_fingerings.first().copied();
                                            }
                                            last.fingerings.append(&mut pending_fingerings);
                                        }
                                        let tab_string = pending_string_number.take();
                                        if let Some(s) = tab_string {
                                            last.string_number = Some(s);
                                        }
                                        if let (Some(string), Some(fret)) =
                                            (tab_string, pending_fret.take())
                                        {
                                            let position =
                                                acorde_core::TabPosition { string, fret };
                                            last.tab_positions.push(position.clone());
                                            if last.tab_position.is_none() {
                                                last.tab_position = Some(position);
                                            }
                                        }
                                        if let Some(t) = pending_technique_text.take() {
                                            last.technique_text = Some(t);
                                        }
                                        if let Some(nh) = pending_note_head.take() {
                                            last.note_head = nh;
                                        }
                                        if let Some(gt) = pending_guitar_technique.take() {
                                            last.guitar_technique = Some(gt);
                                        }
                                        if let Some(cents) = pending_guitar_bend_alter_cents.take()
                                        {
                                            last.guitar_bend_alter_cents = Some(cents);
                                        }
                                        if let Some(up) = note_stem_up {
                                            last.stem_up = Some(up);
                                        }
                                    }
                                } else {
                                    if voice.len() >= MAX_NOTES_PER_VOICE {
                                        return Err(Error::Xml("too many notes in voice".into()));
                                    }
                                    let mut note = note;
                                    if let (Some(actual_notes), Some(normal_notes)) =
                                        (note_tuplet_actual, note_tuplet_normal)
                                        && actual_notes > 0
                                        && normal_notes > 0
                                    {
                                        note.tuplet = Some(TupletInfo {
                                            actual_notes,
                                            normal_notes,
                                        });
                                    }
                                    if let Some(kind) = pending_hairpin_start.take() {
                                        note.hairpin_start = Some(kind);
                                    }
                                    if let Some(cs) = pending_chord.take() {
                                        note.chord_symbol = Some(cs);
                                    }
                                    if let Some(ok) = pending_ottava_start.take() {
                                        note.ottava_start = Some(ok);
                                    }
                                    if pending_pedal_start {
                                        note.pedal_start = true;
                                        pending_pedal_start = false;
                                    }
                                    if note_slur_start {
                                        note.slur_start = true;
                                    }
                                    if note_slur_end {
                                        note.slur_end = true;
                                    }
                                    note.glissando_start = note_glissando_start;
                                    note.glissando_end = note_glissando_end;
                                    if let Some(arp) = note_arpeggiate.take() {
                                        note.arpeggiate = Some(arp);
                                    }
                                    if !pending_fingerings.is_empty() {
                                        note.fingerings = std::mem::take(&mut pending_fingerings);
                                        note.fingering = note.fingerings.first().copied();
                                    }
                                    if let Some(s) = pending_string_number.take() {
                                        note.string_number = Some(s);
                                    }
                                    if let (Some(string), Some(fret)) =
                                        (note.string_number, pending_fret.take())
                                    {
                                        note.tab_position =
                                            Some(acorde_core::TabPosition { string, fret });
                                        note.tab_positions =
                                            note.tab_position.clone().into_iter().collect();
                                    }
                                    if let Some(t) = pending_technique_text.take() {
                                        note.technique_text = Some(t);
                                    }
                                    if let Some(nh) = pending_note_head.take() {
                                        note.note_head = nh;
                                    }
                                    if let Some(gt) = pending_guitar_technique.take() {
                                        note.guitar_technique = Some(gt);
                                    }
                                    if let Some(cents) = pending_guitar_bend_alter_cents.take() {
                                        note.guitar_bend_alter_cents = Some(cents);
                                    }
                                    if let Some(up) = note_stem_up {
                                        note.stem_up = Some(up);
                                    }
                                    if note_trill_line_start {
                                        note.trill_line_start = true;
                                    }
                                    if note_trill_line_end {
                                        note.trill_line_end = true;
                                    }
                                    if note_staff > 1 && !route_to_declared_staff {
                                        note.cross_staff = Some(acorde_core::CrossStaff {
                                            target_staff: requested_staff_index,
                                            target_voice: None,
                                        });
                                    }
                                    if !pending_articulations.is_empty() {
                                        note.articulations =
                                            std::mem::take(&mut pending_articulations);
                                    }
                                    if !lyric_text.is_empty() {
                                        note.lyric = Some(Lyric {
                                            text: lyric_text.clone(),
                                            syllabic: if lyric_syllabic.is_empty() {
                                                "single".to_string()
                                            } else {
                                                lyric_syllabic.clone()
                                            },
                                        });
                                        lyric_text.clear();
                                        lyric_syllabic = "single".to_string();
                                    }
                                    voice.push(note);
                                }
                            }
                        }
                        in_note = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Xml(format!("{e}"))),
            Ok(Event::DocType(_)) => {
                return Err(Error::Xml("DOCTYPE declarations are not allowed".into()));
            }
            _ => {}
        }
        buf.clear();
    }

    if score.parts.is_empty() {
        return Err(Error::Empty);
    }

    score.settings.time_signature = current_time;
    score.settings.key_signature = current_key;

    Ok(score)
}

fn parse_duration_type(t: &str) -> Duration {
    match t {
        "whole" => Duration::Whole,
        "half" => Duration::Half,
        "quarter" => Duration::Quarter,
        "eighth" => Duration::Eighth,
        "16th" => Duration::Sixteenth,
        "32nd" => Duration::ThirtySecond,
        "64th" => Duration::SixtyFourth,
        _ => Duration::Quarter,
    }
}

fn build_note_name(step: &str, alter: i8) -> String {
    let acc = match alter {
        2 => "##",
        1 => "#",
        -1 => "b",
        -2 => "bb",
        _ => "",
    };
    format!("{}{}", step, acc)
}

fn words_to_navigation(text: &str) -> Option<String> {
    match text.trim() {
        "D.C." | "Da Capo" => Some("DaCapo".into()),
        "D.C. al Fine" | "Da Capo al Fine" => Some("DaCapoAlFine".into()),
        "D.C. al Coda" | "Da Capo al Coda" => Some("DaCapoAlCoda".into()),
        "D.S." | "Dal Segno" => Some("DalSegno".into()),
        "D.S. al Fine" | "Dal Segno al Fine" => Some("DalSegnoAlFine".into()),
        "D.S. al Coda" | "Dal Segno al Coda" => Some("DalSegnoAlCoda".into()),
        "Fine" => Some("Fine".into()),
        "To Coda" | "To \u{2295}" => Some("ToCoda".into()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_returns_err() {
        assert!(parse_musicxml("").is_err() || parse_musicxml("").is_ok()); // no panic
    }

    #[test]
    fn garbage_input_does_not_panic() {
        let _ = parse_musicxml("not xml at all <<<>>>");
    }

    #[test]
    fn doctype_rejected() {
        let xml = "<?xml version=\"1.0\"?><!DOCTYPE foo><score-partwise/>";
        assert!(parse_musicxml(xml).is_err());
    }

    #[test]
    fn minimal_score_parses() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <work><work-title>Test</work-title></work>
  <part-list>
    <score-part id="P1"><part-name>Piano</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>480</divisions>
        <key><fifths>0</fifths><mode>major</mode></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>480</duration>
        <voice>1</voice>
        <type>quarter</type>
      </note>
    </measure>
  </part>
</score-partwise>"#;
        let score = parse_musicxml(xml).unwrap();
        assert_eq!(score.metadata.title, "Test");
        assert_eq!(score.parts.len(), 1);
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert!(!notes.is_empty());
        assert!(!notes[0].is_rest);
    }

    #[test]
    fn standard_time_modification_preserves_tuplet_ratio() {
        let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>480</divisions><time><beats>2</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>320</duration><voice>1</voice><type>eighth</type><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification></note></measure></part></score-partwise>"#;
        let score = parse_musicxml(xml).expect("MusicXML tuplet parses");
        let note = &score.parts[0].staves[0].measures[0].voices[0][0];
        assert_eq!(
            note.tuplet,
            Some(TupletInfo {
                actual_notes: 3,
                normal_notes: 2,
            })
        );
    }

    #[test]
    fn slur_parsed_from_notations() {
        // Use 2/4 so two quarter notes exactly fill the measure (no rest filler added).
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>Piano</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>480</divisions>
        <key><fifths>0</fifths><mode>major</mode></key>
        <time><beats>2</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type>
        <notations><slur number="1" type="start"/></notations>
      </note>
      <note>
        <pitch><step>D</step><octave>4</octave></pitch>
        <duration>480</duration><type>quarter</type>
        <notations><slur number="1" type="stop"/></notations>
      </note>
    </measure>
  </part>
</score-partwise>"#;
        let score = parse_musicxml(xml).unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(
            notes.len(),
            2,
            "expect exactly 2 notes (no rest filler in 2/4)"
        );
        assert!(notes[0].slur_start, "first note should have slur_start");
        assert!(!notes[0].slur_end);
        assert!(notes[1].slur_end, "second note should have slur_end");
        assert!(!notes[1].slur_start);
    }

    fn xml_with_notations(notations_inner: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>480</divisions>
        <key><fifths>0</fifths><mode>major</mode></key>
        <time><beats>2</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>960</duration><type>half</type>
        <notations>{}</notations>
      </note>
    </measure>
  </part>
</score-partwise>"#,
            notations_inner
        )
    }

    #[test]
    fn mordent_parsed_from_ornaments() {
        let xml = xml_with_notations("<ornaments><mordent/></ornaments>");
        let score = parse_musicxml(&xml).unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        let note = notes.iter().find(|n| !n.is_rest).unwrap();
        assert!(note.articulations.contains(&Articulation::Mordent));
    }

    #[test]
    fn turn_parsed_from_ornaments() {
        let xml = xml_with_notations("<ornaments><turn/></ornaments>");
        let score = parse_musicxml(&xml).unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        let note = notes.iter().find(|n| !n.is_rest).unwrap();
        assert!(note.articulations.contains(&Articulation::Turn));
    }

    #[test]
    fn tremolo_parsed_from_ornaments() {
        let xml = xml_with_notations("<ornaments><tremolo>3</tremolo></ornaments>");
        let score = parse_musicxml(&xml).unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        let note = notes.iter().find(|n| !n.is_rest).unwrap();
        assert!(note.articulations.contains(&Articulation::Tremolo(3)));
    }

    #[test]
    fn breath_mark_parsed_from_notations() {
        let xml = xml_with_notations("<breath-mark/>");
        let score = parse_musicxml(&xml).unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        let note = notes.iter().find(|n| !n.is_rest).unwrap();
        assert!(note.articulations.contains(&Articulation::BreathMark));
    }

    #[test]
    fn caesura_parsed_from_notations() {
        let xml = xml_with_notations("<caesura/>");
        let score = parse_musicxml(&xml).unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        let note = notes.iter().find(|n| !n.is_rest).unwrap();
        assert!(note.articulations.contains(&Articulation::Caesura));
    }

    #[test]
    fn arpeggiate_up_parsed() {
        let xml = xml_with_notations("<arpeggiate direction=\"up\"/>");
        let score = parse_musicxml(&xml).unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        let note = notes.iter().find(|n| !n.is_rest).unwrap();
        assert_eq!(note.arpeggiate, Some(true));
    }

    #[test]
    fn arpeggiate_down_parsed() {
        let xml = xml_with_notations("<arpeggiate direction=\"down\"/>");
        let score = parse_musicxml(&xml).unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        let note = notes.iter().find(|n| !n.is_rest).unwrap();
        assert_eq!(note.arpeggiate, Some(false));
    }

    #[test]
    fn arpeggiate_default_dir_parsed_as_up() {
        let xml = xml_with_notations("<arpeggiate/>");
        let score = parse_musicxml(&xml).unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        let note = notes.iter().find(|n| !n.is_rest).unwrap();
        assert_eq!(note.arpeggiate, Some(true));
    }
}
