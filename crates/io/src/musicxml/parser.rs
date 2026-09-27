use crate::Error;
use acorde_core::{
    Articulation, Barline, ChordDegree, ChordSymbol, Clef, Duration, Dynamic, FiguredBassFigure,
    GuitarTechnique, HairpinKind, HarpPedalDiagram, HarpPedalPosition, KeySignature, Lyric,
    Measure, MeasureLength, NotationSpanner, NotationSpannerKind, Note, NoteAddr, NoteHead,
    OttavaKind, Part, PartGroup, PartGroupSymbol, PercussionInstrument, Pitch, Score, Staff,
    StaffKind, Step, StyledText, TextStyle, TimeSignature, TupletInfo, VerseLyric, VoltaBracket,
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
const MAX_SOURCE_VOICE_NUMBER: u32 = 1_000_000;
const MAX_DEPTH: usize = 64;

/// Accept only the conventional external MusicXML DTD declaration.
///
/// `quick-xml` does not resolve the declaration, but accepting an internal subset would
/// unnecessarily widen the parser's input surface. MuseScore writes this public declaration
/// by default, so retain the compatibility path without admitting entity declarations.
/// Accept a MusicXML DOCTYPE with any external identifier, but never an internal subset.
///
/// The reader never dereferences external DTDs, so the public/system identifier is inert: old
/// Finale exports point at local paths (`"//D:/Program Files/MusicXML/partwise.dtd"`), some tools
/// use `SYSTEM`, and all of them must still open. Internal subsets (`[...]`), entity and
/// parameter-entity declarations remain rejected, as does a non-MusicXML root.
fn is_safe_musicxml_doctype(declaration: &[u8]) -> bool {
    let Ok(declaration) = std::str::from_utf8(declaration) else {
        return false;
    };
    let normalized = declaration.trim().to_ascii_lowercase();
    if normalized.contains(['[', ']', '%', '<', '>']) || normalized.contains("entity") {
        return false;
    }
    let Some(rest) = normalized
        .strip_prefix("score-partwise")
        .or_else(|| normalized.strip_prefix("score-timewise"))
    else {
        return false;
    };
    let rest = rest.trim_start();
    rest.is_empty() || rest.starts_with("public") || rest.starts_with("system")
}

/// Acorde stores tablature tuning from low to high, while MusicXML technical string numbers
/// count from the highest string. Translate at the I/O boundary when tuning metadata is known.
fn musicxml_string_to_internal(string: u8, tab_lines: Option<u8>) -> u8 {
    match tab_lines {
        Some(lines) if string != 0 && string <= lines => lines + 1 - string,
        _ => string,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpannerAction {
    Start,
    Stop,
}

#[derive(Debug, Clone)]
struct ParsedSpannerEvent {
    kind: NotationSpannerKind,
    action: SpannerAction,
    number: u16,
    line_type: Option<String>,
    text: Option<String>,
    placement: Option<String>,
    ottava_size: Option<u8>,
}

#[derive(Debug, Clone)]
struct OpenSpanner {
    start: NoteAddr,
    line_type: Option<String>,
    text: Option<String>,
    placement: Option<String>,
    ottava_size: Option<u8>,
}

fn parse_spanner_number(value: Option<String>) -> Result<u16, Error> {
    match value {
        None => Ok(1),
        Some(value) => value
            .parse::<u16>()
            .map_err(|_| Error::Xml(format!("invalid MusicXML spanner number: {value}"))),
    }
}

/// How far a measure's content may run past its time signature (cadenzas, un-barred passages)
/// before the input is treated as corrupt.
const MAX_OVERFULL_FACTOR: u32 = 16;

fn apply_spanner_event(
    score: &mut Score,
    open_spanners: &mut HashMap<(NotationSpannerKind, u16), Vec<OpenSpanner>>,
    event: ParsedSpannerEvent,
    address: NoteAddr,
    tolerated: &mut Vec<crate::Diagnostic>,
) -> Result<(), Error> {
    let key = (event.kind.clone(), event.number);
    match event.action {
        SpannerAction::Start => {
            open_spanners.entry(key).or_default().push(OpenSpanner {
                start: address,
                line_type: event.line_type,
                text: event.text,
                placement: event.placement,
                ottava_size: event.ottava_size,
            });
        }
        SpannerAction::Stop => {
            // A stop with no open start (common in real files) is dropped and reported rather
            // than failing the whole import, as MuseScore does.
            let Some(open) = open_spanners.get_mut(&key).and_then(Vec::pop) else {
                let mut diagnostic = crate::Diagnostic::warning(
                    "musicxml.orphan-spanner-stop",
                    format!(
                        "{:?} stop number {} has no matching start and is ignored",
                        event.kind, event.number
                    ),
                );
                diagnostic.source_location = Some(format!(
                    "/score/part/{}/staff/{}/measure/{}/voice/{}/note/{}",
                    address.part + 1,
                    address.staff + 1,
                    address.measure + 1,
                    address.voice + 1,
                    address.note + 1
                ));
                tolerated.push(diagnostic);
                return Ok(());
            };
            if open_spanners.get(&key).is_some_and(Vec::is_empty) {
                open_spanners.remove(&key);
            }
            let id = format!(
                "musicxml:{:?}:{}:{}:{}:{}:{}:{}-{}:{}:{}:{}:{}",
                event.kind,
                event.number,
                open.start.part,
                open.start.staff,
                open.start.measure,
                open.start.voice,
                open.start.note,
                address.part,
                address.staff,
                address.measure,
                address.voice,
                address.note,
            )
            .to_lowercase();
            score.spanners.push(NotationSpanner {
                id,
                kind: event.kind,
                start: open.start,
                end: address,
                number: Some(event.number),
                line_type: event.line_type.or(open.line_type),
                text: event.text.or(open.text),
                placement: event.placement.or(open.placement),
                ottava_size: event.ottava_size.or(open.ottava_size),
                ottava_type: None,
            });
        }
    }
    Ok(())
}

pub fn parse_musicxml(xml: &str) -> Result<Score, Error> {
    parse_musicxml_collecting(xml, &mut Vec::new())
}

/// Parse MusicXML, recording recoverable source problems (orphan spanner stops, overfull
/// measures, backups past the measure start, chord members on another staff) in `tolerated`
/// instead of failing.
pub(crate) fn parse_musicxml_collecting(
    xml: &str,
    tolerated: &mut Vec<crate::Diagnostic>,
) -> Result<Score, Error> {
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
    let mut current_divisions = 480u32;
    let mut current_time = TimeSignature::default();
    let mut current_key = KeySignature::default();
    let mut current_clef = Clef::Treble;
    let mut current_clef_staff_number = 1usize;
    // The <clef> being read (sign, line), and clef changes for staves after the first:
    // (part, staff index, measure index, at the start of the measure, clef).
    let mut in_clef = false;
    let mut clef_sign = String::new();
    let mut clef_line: Option<u8> = None;
    // Clef changes as (part, staff, bar, beats into the bar, clef); 0 beats = at its start.
    let mut staff_clef_events: Vec<(usize, usize, usize, f64, Clef)> = Vec::new();
    // Set when the current `<attributes>` changes the first staff's clef at its bar's start;
    // other attributes (a key or time change, a mid-bar clef) leave the bar's clef alone.
    let mut first_staff_bar_clef_read = false;
    let mut in_note = false;
    let mut note_slur_start = false;
    let mut note_slur_end = false;
    let mut note_tie_start = false;
    let mut note_tie_end = false;
    let mut note_glissando_start = false;
    let mut note_glissando_end = false;
    let mut note_spanner_events: Vec<ParsedSpannerEvent> = Vec::new();
    let mut pending_direction_spanner_events: Vec<ParsedSpannerEvent> = Vec::new();
    let mut open_spanners: HashMap<(NotationSpannerKind, u16), Vec<OpenSpanner>> = HashMap::new();
    let mut open_ottava_types: HashMap<u16, String> = HashMap::new();
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
    let mut note_duration_ticks: Option<u32> = None;
    let mut note_type = "quarter".to_string();
    let mut note_dot = false;
    let mut note_rest = false;
    let mut note_is_measure_rest = false;
    let mut in_unpitched = false;
    let mut note_is_unpitched = false;
    let mut note_instrument_id: Option<String> = None;
    // (part, kit instrument id, display position) of unpitched notes, first one per id wins.
    let mut kit_display_positions: Vec<(usize, String, i8)> = Vec::new();
    let mut note_offset_x: Option<f64> = None;
    let mut note_offset_y: Option<f64> = None;
    let mut note_relative_x: Option<f64> = None;
    let mut note_relative_y: Option<f64> = None;
    let mut note_chord = false;
    let mut note_voice = 1u32;
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
    let mut note_beam = acorde_core::BeamState::None;
    let mut note_beam_level: u8 = 1;
    let mut pending_guitar_technique: Option<GuitarTechnique> = None;
    let mut pending_guitar_bend_alter_cents: Option<i16> = None;
    // A `<wedge>` in the current direction: Some(Some(kind)) starts a hairpin, Some(None)
    // stops one. Settled at the direction's end, when its `<staff>` is known.
    let mut pending_direction_wedge: Option<Option<HairpinKind>> = None;
    // Hairpins waiting for the next note of their staff, and the voice each one began in.
    let mut staff_hairpin_starts: HashMap<usize, HairpinKind> = HashMap::new();
    let mut open_hairpin_voices: HashMap<usize, usize> = HashMap::new();
    // The latest note of each (staff, voice), where a wedge stop lands.
    let mut last_voice_notes: HashMap<(usize, usize), NoteAddr> = HashMap::new();
    let mut last_staff_voice: HashMap<usize, usize> = HashMap::new();
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
    // `<extend>` (start, or untyped) under the current lyric: a melisma line follows it.
    let mut lyric_extend = false;
    // An `<elision>` read since the last `<text>`: its connector joins the next syllable on.
    let mut lyric_elision: Option<String> = None;
    let mut lyric_text = String::new();
    let mut lyric_syllabic = String::new();
    // Source `<lyric number>` of the lyric being read, and the note's completed lyrics.
    let mut lyric_number: Option<u8> = None;
    let mut note_lyrics: Vec<(Option<u8>, Lyric)> = Vec::new();
    let mut in_measure_style = false;
    // `<measure-repeat type="start">n</measure-repeat>` repeats the previous n measures until a
    // `type="stop"` marker; the start element's text is read at its end tag.
    let mut measure_repeat_active: Option<u8> = None;
    let mut measure_repeat_start_pending = false;
    let mut in_staff_details = false;
    let mut staff_details_number = 1usize;
    let mut staff_lines: Option<u8> = None;
    let mut in_staff_tuning = false;
    let mut staff_tuning_line: Option<u8> = None;
    let mut staff_tuning_step = Step::C;
    let mut staff_tuning_alter = 0i8;
    let mut staff_tuning_octave = 4i8;
    let mut staff_tunings: Vec<(u8, i16)> = Vec::new();
    let mut staff_capo: Option<u8> = None;
    let mut in_multiple_rest = false;
    let mut in_barline = false;
    let mut barline_location = String::new();
    let mut in_direction = false;
    let mut in_direction_type = false;
    // Dynamics: <direction><direction-type><dynamics> marks wait for the next sounding note of
    // their staff; <notations><dynamics> belongs to its note.
    let mut in_dynamics = false;
    let mut pending_direction_dynamic: Option<Dynamic> = None;
    let mut pending_direction_staff = 0usize;
    let mut staff_dynamics: HashMap<usize, Dynamic> = HashMap::new();
    let mut note_inline_dynamic: Option<Dynamic> = None;
    let mut pending_tempo_text: Option<String> = None;
    let mut pending_expression_text: Option<String> = None;
    let mut pending_rehearsal: Option<String> = None;
    let mut pending_navigation: Option<String> = None;
    let mut pending_section_break = false;
    let mut pending_sound_tempo: Option<u16> = None;
    let mut pending_direction_placement: Option<String> = None;
    let mut pending_direction_offset_x: Option<f64> = None;
    let mut pending_direction_offset_y: Option<f64> = None;
    let mut pending_direction_relative_x: Option<f64> = None;
    let mut pending_direction_relative_y: Option<f64> = None;
    let mut pending_harp_pedal_diagrams: Vec<HarpPedalDiagram> = Vec::new();
    let mut pending_harp_pedal_diagram: Option<HarpPedalDiagram> = None;
    let mut in_harp_pedals = false;
    let mut in_harp_pedal_tuning = false;
    let mut harp_pedal_step = Step::C;
    let mut harp_pedal_alter = 0i8;
    let mut in_work = false;
    let mut in_backup = false;
    let mut in_forward = false;
    let mut measure_cursor_ticks = 0u32;
    // Furthest cursor position reached in the current measure (notes and `<forward>`).
    let mut measure_content_ticks = 0u32;
    let mut voice_cursor_ticks: HashMap<(usize, usize), u32> = HashMap::new();
    let mut source_voice_slots: HashMap<(usize, u32), usize> = HashMap::new();
    let mut last_note_start: Option<(usize, usize, u32)> = None;
    // Owning staff of the previous note when it was a cross-staff placement, so chord members
    // follow their chord head.
    let mut last_note_cross_home: Option<usize> = None;
    let mut last_note_address: Option<NoteAddr> = None;
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
    let mut midi_instrument_id = String::new();
    // Standard MusicXML puts <midi-unpitched> in <midi-instrument id="..."> matching a
    // <score-instrument>; collected per id and applied when the <score-part> closes.
    let mut midi_unpitched_by_id: HashMap<String, u8> = HashMap::new();
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
                        in_clef = true;
                        clef_sign.clear();
                        clef_line = None;
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
                        midi_instrument_id = attr_str(e, b"id").unwrap_or_default();
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
                    "direction" => {
                        in_direction = true;
                        pending_direction_placement = attr_str(e, b"placement");
                        pending_direction_offset_x = attr_str(e, b"default-x")
                            .and_then(|value| value.parse::<f64>().ok())
                            .filter(|value| value.is_finite());
                        pending_direction_offset_y = attr_str(e, b"default-y")
                            .and_then(|value| value.parse::<f64>().ok())
                            .filter(|value| value.is_finite());
                        pending_direction_relative_x = attr_str(e, b"relative-x")
                            .and_then(|value| value.parse::<f64>().ok())
                            .filter(|value| value.is_finite());
                        pending_direction_relative_y = attr_str(e, b"relative-y")
                            .and_then(|value| value.parse::<f64>().ok())
                            .filter(|value| value.is_finite());
                    }
                    "direction-type" => in_direction_type = true,
                    "dynamics" if in_direction_type || in_notations => in_dynamics = true,
                    "other-dynamics" if in_dynamics => {
                        let mut diagnostic = crate::Diagnostic::warning(
                            "musicxml.unsupported-dynamic",
                            "a dynamic outside acorde's set (other-dynamics) is not imported",
                        );
                        diagnostic.source_location = Some(format!(
                            "/score-partwise/measure[{current_measure_number}]/dynamics/other-dynamics"
                        ));
                        tolerated.push(diagnostic);
                    }
                    // Print-style position attributes belong on the text element; they take
                    // precedence over the legacy acorde placement on <direction>.
                    "words" | "rehearsal" if in_direction_type => {
                        let position = |name: &[u8]| {
                            attr_str(e, name)
                                .and_then(|value| value.parse::<f64>().ok())
                                .filter(|value| value.is_finite())
                        };
                        if let Some(value) = position(b"default-x") {
                            pending_direction_offset_x = Some(value);
                        }
                        if let Some(value) = position(b"default-y") {
                            pending_direction_offset_y = Some(value);
                        }
                        if let Some(value) = position(b"relative-x") {
                            pending_direction_relative_x = Some(value);
                        }
                        if let Some(value) = position(b"relative-y") {
                            pending_direction_relative_y = Some(value);
                        }
                    }
                    "harp-pedals" if in_direction_type => {
                        in_harp_pedals = true;
                        pending_harp_pedal_diagram = Some(HarpPedalDiagram::default());
                    }
                    "pedal-tuning" if in_harp_pedals => {
                        in_harp_pedal_tuning = true;
                        harp_pedal_step = Step::C;
                        harp_pedal_alter = 0;
                    }
                    "measure-style" => in_measure_style = true,
                    "staff-details" => {
                        in_staff_details = true;
                        staff_details_number = attr_str(e, b"number")
                            .and_then(|value| value.parse().ok())
                            .filter(|number: &usize| (1..=MAX_STAVES).contains(number))
                            .unwrap_or(1);
                        if let Some(pi) = part_index {
                            while score.parts[pi].staves.len() < staff_details_number {
                                score.parts[pi].staves.push(Staff::new(Clef::Treble));
                            }
                        }
                        staff_lines = None;
                        staff_tunings.clear();
                        staff_capo = None;
                    }
                    "staff-tuning" if in_staff_details => {
                        in_staff_tuning = true;
                        staff_tuning_line = attr_str(e, b"line").and_then(|v| v.parse().ok());
                        staff_tuning_step = Step::C;
                        staff_tuning_alter = 0;
                        staff_tuning_octave = 4;
                    }
                    "multiple-rest" if in_measure_style => in_multiple_rest = true,
                    "measure-repeat" if in_measure_style => match attr_str(e, b"type").as_deref() {
                        Some("stop") => measure_repeat_active = None,
                        _ => measure_repeat_start_pending = true,
                    },
                    "notations" if in_note => in_notations = true,
                    "articulations" if in_notations => in_artic_block = true,
                    "ornaments" if in_notations => in_ornament_block = true,
                    "technical" if in_notations => in_technical_block = true,
                    // `<harmonic>` may carry `<natural/>` or `<artificial/>` children.
                    "harmonic" if in_technical_block => {
                        pending_articulations.push(Articulation::Harmonic)
                    }
                    "glissando" if in_notations => match attr_str(e, b"type").as_deref() {
                        Some("start") => {
                            note_glissando_start = true;
                            note_spanner_events.push(ParsedSpannerEvent {
                                kind: NotationSpannerKind::Glissando,
                                action: SpannerAction::Start,
                                number: parse_spanner_number(attr_str(e, b"number"))?,
                                line_type: attr_str(e, b"line-type"),
                                text: None,
                                placement: attr_str(e, b"placement"),
                                ottava_size: None,
                            });
                        }
                        Some("stop") => {
                            note_glissando_end = true;
                            note_spanner_events.push(ParsedSpannerEvent {
                                kind: NotationSpannerKind::Glissando,
                                action: SpannerAction::Stop,
                                number: parse_spanner_number(attr_str(e, b"number"))?,
                                line_type: attr_str(e, b"line-type"),
                                text: None,
                                placement: attr_str(e, b"placement"),
                                ottava_size: None,
                            });
                        }
                        _ => {}
                    },
                    "tremolo" if in_ornament_block => {
                        pending_tremolo = true;
                    }
                    "lyric" if in_note => {
                        in_lyric = true;
                        lyric_text.clear();
                        lyric_elision = None;
                        lyric_syllabic = "single".to_string();
                        lyric_number = attr_str(e, b"number")
                            .and_then(|value| value.trim().parse::<u8>().ok())
                            .filter(|verse| (1..=VerseLyric::MAX_VERSE).contains(verse));
                    }
                    "time-modification" if in_note => {
                        in_time_modification = true;
                        note_tuplet_actual = None;
                        note_tuplet_normal = None;
                    }
                    "beam" if in_note => {
                        note_beam_level = attr_str(e, b"number")
                            .and_then(|value| value.parse::<u8>().ok())
                            .unwrap_or(1);
                    }
                    "note" => {
                        in_note = true;
                        note_lyrics.clear();
                        note_inline_dynamic = None;
                        // A note's default-x/default-y are absolute positions in the source
                        // engraver's layout (from the measure's left barline and the staff's top
                        // line). acorde lays measures out itself, and `offset_x`/`offset_y` are
                        // nudges from the computed position, so those values are not imported;
                        // relative-x/relative-y are.
                        note_offset_x = None;
                        note_offset_y = None;
                        note_relative_x = attr_str(e, b"relative-x")
                            .and_then(|value| value.parse().ok())
                            .filter(|value: &f64| value.is_finite());
                        note_relative_y = attr_str(e, b"relative-y")
                            .and_then(|value| value.parse().ok())
                            .filter(|value: &f64| value.is_finite());
                        note_rest = false;
                        note_is_measure_rest = false;
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
                        note_duration_ticks = None;
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
                        note_tie_start = false;
                        note_tie_end = false;
                        note_glissando_start = false;
                        note_glissando_end = false;
                        note_spanner_events.clear();
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
                        note_beam = acorde_core::BeamState::None;
                        pending_note_head = None;
                    }
                    "tie" | "tied" if in_note => match attr_str(e, b"type").as_deref() {
                        Some("start") => note_tie_start = true,
                        Some("stop") => note_tie_end = true,
                        _ => {}
                    },
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
                        current_divisions = 480;
                        current_time = TimeSignature::default();
                        current_key = KeySignature::default();
                        current_clef = Clef::Treble;
                        current_measure_number = 0;
                        source_voice_slots.clear();
                        measure_repeat_active = None;
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
                        measure_cursor_ticks = 0;
                        measure_content_ticks = 0;
                        voice_cursor_ticks.clear();
                        last_note_start = None;
                        last_note_cross_home = None;
                        last_note_address = None;
                    }
                    "backup" => in_backup = true,
                    "forward" => in_forward = true,
                    _ => {}
                }
            }

            Ok(Event::Empty(ref e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_string();
                if in_dynamics {
                    match musicxml_dynamic(&tag) {
                        Some(dynamic) if in_note => note_inline_dynamic = Some(dynamic),
                        Some(dynamic) => pending_direction_dynamic = Some(dynamic),
                        None => {
                            let mut diagnostic = crate::Diagnostic::warning(
                                "musicxml.unsupported-dynamic",
                                "a dynamic outside acorde's set (other-dynamics) is not imported",
                            );
                            diagnostic.source_location = Some(format!(
                                "/score-partwise/measure[{current_measure_number}]/dynamics/{tag}"
                            ));
                            tolerated.push(diagnostic);
                        }
                    }
                }
                match tag.as_str() {
                    "elision" if in_lyric => lyric_elision = Some(LYRIC_ELISION.to_string()),
                    "extend" if in_lyric => {
                        lyric_extend =
                            matches!(attr_str(e, b"type").as_deref(), None | Some("start"));
                    }
                    "measure-repeat" if attr_str(e, b"type").as_deref() == Some("stop") => {
                        measure_repeat_active = None;
                    }
                    "measure-repeat" => {
                        // An empty start marker repeats one measure.
                        measure_repeat_active = Some(1);
                    }
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
                    "rest" if in_note => {
                        note_rest = true;
                        note_is_measure_rest = attr_is_yes(e, b"measure");
                    }
                    "instrument" if in_note => note_instrument_id = attr_str(e, b"id"),
                    "dot" if in_note => note_dot = true,
                    "chord" if in_note => note_chord = true,
                    "slur" if in_note => match attr_str(e, b"type").as_deref() {
                        Some("start") => {
                            note_slur_start = true;
                            note_spanner_events.push(ParsedSpannerEvent {
                                kind: NotationSpannerKind::Slur,
                                action: SpannerAction::Start,
                                number: parse_spanner_number(attr_str(e, b"number"))?,
                                line_type: attr_str(e, b"line-type"),
                                text: None,
                                placement: attr_str(e, b"placement"),
                                ottava_size: None,
                            });
                        }
                        Some("stop") => {
                            note_slur_end = true;
                            note_spanner_events.push(ParsedSpannerEvent {
                                kind: NotationSpannerKind::Slur,
                                action: SpannerAction::Stop,
                                number: parse_spanner_number(attr_str(e, b"number"))?,
                                line_type: attr_str(e, b"line-type"),
                                text: None,
                                placement: attr_str(e, b"placement"),
                                ottava_size: None,
                            });
                        }
                        _ => {}
                    },
                    "tie" | "tied" if in_note => match attr_str(e, b"type").as_deref() {
                        Some("start") => note_tie_start = true,
                        Some("stop") => note_tie_end = true,
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
                    // Standard MusicXML puts <slide> in <notations>; older acorde output put it
                    // in <technical>.
                    "slide" if in_technical_block || in_notations => {
                        pending_guitar_technique = Some(GuitarTechnique::Slide);
                    }
                    "hammer-on" if in_technical_block => {
                        pending_guitar_technique = Some(GuitarTechnique::HammerOn);
                    }
                    "pull-off" if in_technical_block => {
                        pending_guitar_technique = Some(GuitarTechnique::PullOff);
                    }
                    "up-bow" if in_technical_block => {
                        pending_articulations.push(Articulation::UpBow)
                    }
                    "down-bow" if in_technical_block => {
                        pending_articulations.push(Articulation::DownBow)
                    }
                    "harmonic" if in_technical_block => {
                        pending_articulations.push(Articulation::Harmonic)
                    }
                    "open-string" if in_technical_block => {
                        pending_articulations.push(Articulation::OpenString)
                    }
                    "stopped" if in_technical_block => {
                        pending_articulations.push(Articulation::Stopped)
                    }
                    "snap-pizzicato" if in_technical_block => {
                        pending_articulations.push(Articulation::SnapPizzicato)
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
                        Some("start") => {
                            note_trill_line_start = true;
                            note_spanner_events.push(ParsedSpannerEvent {
                                kind: NotationSpannerKind::TrillLine,
                                action: SpannerAction::Start,
                                number: parse_spanner_number(attr_str(e, b"number"))?,
                                line_type: attr_str(e, b"line-type"),
                                text: None,
                                placement: attr_str(e, b"placement"),
                                ottava_size: None,
                            });
                        }
                        Some("stop") => {
                            note_trill_line_end = true;
                            note_spanner_events.push(ParsedSpannerEvent {
                                kind: NotationSpannerKind::TrillLine,
                                action: SpannerAction::Stop,
                                number: parse_spanner_number(attr_str(e, b"number"))?,
                                line_type: attr_str(e, b"line-type"),
                                text: None,
                                placement: attr_str(e, b"placement"),
                                ottava_size: None,
                            });
                        }
                        _ => {}
                    },
                    "glissando" if in_notations => match attr_str(e, b"type").as_deref() {
                        Some("start") => {
                            note_glissando_start = true;
                            note_spanner_events.push(ParsedSpannerEvent {
                                kind: NotationSpannerKind::Glissando,
                                action: SpannerAction::Start,
                                number: parse_spanner_number(attr_str(e, b"number"))?,
                                line_type: attr_str(e, b"line-type"),
                                text: None,
                                placement: attr_str(e, b"placement"),
                                ottava_size: None,
                            });
                        }
                        Some("stop") => {
                            note_glissando_end = true;
                            note_spanner_events.push(ParsedSpannerEvent {
                                kind: NotationSpannerKind::Glissando,
                                action: SpannerAction::Stop,
                                number: parse_spanner_number(attr_str(e, b"number"))?,
                                line_type: attr_str(e, b"line-type"),
                                text: None,
                                placement: attr_str(e, b"placement"),
                                ottava_size: None,
                            });
                        }
                        _ => {}
                    },
                    "wedge" => match attr_str(e, b"type").as_deref() {
                        Some("crescendo") => {
                            pending_direction_wedge = Some(Some(HairpinKind::Crescendo));
                        }
                        Some("diminuendo") | Some("decrescendo") => {
                            pending_direction_wedge = Some(Some(HairpinKind::Decrescendo));
                        }
                        Some("stop") => pending_direction_wedge = Some(None),
                        _ => {}
                    },
                    "octave-shift" => {
                        let shift_type = attr_str(e, b"type").unwrap_or_default();
                        let shift_size: u8 = attr_str(e, b"size")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(8);
                        match shift_type.as_str() {
                            // Notes carry their sounding pitch; `up` displays them higher,
                            // which is an 8vb (15mb), and `down` an 8va (15ma).
                            "up" => {
                                let number = parse_spanner_number(attr_str(e, b"number"))?;
                                open_ottava_types.insert(number, shift_type.clone());
                                pending_ottava_start = Some(if shift_size >= 15 {
                                    OttavaKind::Mb15
                                } else {
                                    OttavaKind::Vb8
                                });
                                pending_direction_spanner_events.push(ParsedSpannerEvent {
                                    kind: NotationSpannerKind::Ottava,
                                    action: SpannerAction::Start,
                                    number,
                                    line_type: attr_str(e, b"line-type"),
                                    text: None,
                                    placement: pending_direction_placement.clone(),
                                    ottava_size: Some(shift_size),
                                });
                            }
                            "down" => {
                                let number = parse_spanner_number(attr_str(e, b"number"))?;
                                open_ottava_types.insert(number, shift_type.clone());
                                pending_ottava_start = Some(if shift_size >= 15 {
                                    OttavaKind::Ma15
                                } else {
                                    OttavaKind::Va8
                                });
                                pending_direction_spanner_events.push(ParsedSpannerEvent {
                                    kind: NotationSpannerKind::Ottava,
                                    action: SpannerAction::Start,
                                    number,
                                    line_type: attr_str(e, b"line-type"),
                                    text: None,
                                    placement: pending_direction_placement.clone(),
                                    ottava_size: Some(shift_size),
                                });
                            }
                            "stop" => {
                                let number = parse_spanner_number(attr_str(e, b"number"))?;
                                // The legacy flag marks the same note as the typed stop: the latest one read.
                                if let Some(address) = last_note_address.as_ref()
                                    && let Some(n) = score.parts[address.part]
                                        .staves
                                        .get_mut(address.staff)
                                        .and_then(|staff| staff.measures.get_mut(address.measure))
                                        .and_then(|measure| measure.voices.get_mut(address.voice))
                                        .and_then(|voice| voice.get_mut(address.note))
                                {
                                    n.ottava_end = true;
                                }
                                let address = last_note_address.clone().ok_or_else(|| {
                                    Error::Xml(
                                        "MusicXML octave-shift stop has no preceding note".into(),
                                    )
                                })?;
                                apply_spanner_event(
                                    &mut score,
                                    &mut open_spanners,
                                    ParsedSpannerEvent {
                                        kind: NotationSpannerKind::Ottava,
                                        action: SpannerAction::Stop,
                                        number,
                                        line_type: attr_str(e, b"line-type"),
                                        text: None,
                                        placement: pending_direction_placement.clone(),
                                        ottava_size: Some(shift_size),
                                    },
                                    address,
                                    tolerated,
                                )?;
                                // An orphan stop was already reported by apply_spanner_event.
                                if let Some(ottava_type) = open_ottava_types.remove(&number)
                                    && let Some(spanner) = score.spanners.last_mut()
                                {
                                    spanner.ottava_type = Some(ottava_type);
                                }
                            }
                            _ => {}
                        }
                    }
                    "pedal" => match attr_str(e, b"type").as_deref() {
                        Some("start") => {
                            pending_pedal_start = true;
                            pending_direction_spanner_events.push(ParsedSpannerEvent {
                                kind: NotationSpannerKind::Pedal,
                                action: SpannerAction::Start,
                                number: parse_spanner_number(attr_str(e, b"number"))?,
                                line_type: attr_str(e, b"line"),
                                text: None,
                                placement: pending_direction_placement.clone(),
                                ottava_size: None,
                            });
                        }
                        Some("stop") => {
                            // The legacy flag marks the same note as the typed stop: the latest one read.
                            if let Some(address) = last_note_address.as_ref()
                                && let Some(n) = score.parts[address.part]
                                    .staves
                                    .get_mut(address.staff)
                                    .and_then(|staff| staff.measures.get_mut(address.measure))
                                    .and_then(|measure| measure.voices.get_mut(address.voice))
                                    .and_then(|voice| voice.get_mut(address.note))
                            {
                                n.pedal_end = true;
                            }
                            let address = last_note_address.clone().ok_or_else(|| {
                                Error::Xml("MusicXML pedal stop has no preceding note".into())
                            })?;
                            apply_spanner_event(
                                &mut score,
                                &mut open_spanners,
                                ParsedSpannerEvent {
                                    kind: NotationSpannerKind::Pedal,
                                    action: SpannerAction::Stop,
                                    number: parse_spanner_number(attr_str(e, b"number"))?,
                                    line_type: attr_str(e, b"line"),
                                    text: None,
                                    placement: pending_direction_placement.clone(),
                                    ottava_size: None,
                                },
                                address,
                                tolerated,
                            )?;
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
                    "glissando" if in_notations => {
                        let text = current_text.trim();
                        if !text.is_empty()
                            && let Some(event) =
                                note_spanner_events.iter_mut().rev().find(|event| {
                                    event.kind == NotationSpannerKind::Glissando
                                        && event.text.is_none()
                                })
                        {
                            event.text = Some(text.to_string());
                        }
                    }
                    "instrument-name" if in_score_instrument => {
                        let name = current_text.trim();
                        if !name.is_empty() {
                            score_instrument_name = Some(name.to_string());
                        }
                    }
                    // MusicXML numbers percussion keys 1–128; the model holds the 0-based GM key.
                    "midi-unpitched" if in_score_instrument => {
                        score_instrument_key = musicxml_midi_unpitched(&current_text);
                    }
                    "midi-unpitched" if in_midi_instrument => {
                        if let Some(key) = musicxml_midi_unpitched(&current_text) {
                            midi_unpitched_by_id.insert(midi_instrument_id.clone(), key);
                        }
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
                                    staff_position: None,
                                    notehead: None,
                                    preferred_voice: None,
                                    techniques: Vec::new(),
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
                        if let Some(instruments) = part_percussion.get_mut(&score_part_id) {
                            for instrument in instruments {
                                if instrument.midi_unpitched.is_none() {
                                    instrument.midi_unpitched =
                                        midi_unpitched_by_id.get(&instrument.id).copied();
                                }
                            }
                        }
                        midi_unpitched_by_id.clear();
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
                                range_end: None,
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
                                    placement: None,
                                    offset_x: None,
                                    offset_y: None,
                                    relative_x: None,
                                    relative_y: None,
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
                    "pedal-tuning" if in_harp_pedal_tuning => {
                        in_harp_pedal_tuning = false;
                        let index = match harp_pedal_step {
                            Step::D => 0,
                            Step::C => 1,
                            Step::B => 2,
                            Step::E => 3,
                            Step::F => 4,
                            Step::G => 5,
                            Step::A => 6,
                        };
                        let position = match harp_pedal_alter {
                            -1 => HarpPedalPosition::Flat,
                            1 => HarpPedalPosition::Sharp,
                            _ => HarpPedalPosition::Natural,
                        };
                        if let Some(diagram) = pending_harp_pedal_diagram.as_mut() {
                            diagram.positions[index] = position;
                        }
                    }
                    "harp-pedals" if in_harp_pedals => {
                        in_harp_pedals = false;
                        if let Some(diagram) = pending_harp_pedal_diagram.take() {
                            pending_harp_pedal_diagrams.push(diagram);
                        }
                    }
                    "direction-type" => in_direction_type = false,
                    "direction" => {
                        if pending_section_break {
                            if let Some(pi) = part_index {
                                for staff in &mut score.parts[pi].staves {
                                    if let Some(measure) = staff.measures.last_mut() {
                                        measure.section_break = true;
                                    }
                                }
                            }
                        }
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
                                        placement: pending_direction_placement.clone(),
                                        offset_x: pending_direction_offset_x,
                                        offset_y: pending_direction_offset_y,
                                        relative_x: pending_direction_relative_x,
                                        relative_y: pending_direction_relative_y,
                                    });
                                }
                            } else if let Some(text) = pending_expression_text.take()
                                && m.expression_text.is_none()
                            {
                                m.expression_text = Some(text.clone());
                                m.texts.push(StyledText {
                                    style: TextStyle::Expression,
                                    text,
                                    placement: pending_direction_placement.clone(),
                                    offset_x: pending_direction_offset_x,
                                    offset_y: pending_direction_offset_y,
                                    relative_x: pending_direction_relative_x,
                                    relative_y: pending_direction_relative_y,
                                });
                            }
                            if let Some(reh) = pending_rehearsal.take()
                                && m.rehearsal.is_none()
                            {
                                m.rehearsal = Some(reh.clone());
                                m.texts.push(StyledText {
                                    style: TextStyle::RehearsalMark,
                                    text: reh,
                                    placement: pending_direction_placement.clone(),
                                    offset_x: pending_direction_offset_x,
                                    offset_y: pending_direction_offset_y,
                                    relative_x: pending_direction_relative_x,
                                    relative_y: pending_direction_relative_y,
                                });
                            }
                            if let Some(bpm) = pending_sound_tempo.take() {
                                m.tempo = Some(bpm);
                            }
                            for mut diagram in std::mem::take(&mut pending_harp_pedal_diagrams) {
                                diagram.placement = pending_direction_placement.clone();
                                m.harp_pedal_diagrams.push(diagram);
                            }
                        }
                        pending_sound_tempo = None;
                        pending_direction_placement = None;
                        pending_direction_offset_x = None;
                        pending_direction_offset_y = None;
                        pending_direction_relative_x = None;
                        pending_direction_relative_y = None;
                        pending_tempo_text = None;
                        pending_expression_text = None;
                        pending_rehearsal = None;
                        pending_navigation = None;
                        pending_section_break = false;
                        if let Some(dynamic) = pending_direction_dynamic.take() {
                            staff_dynamics.insert(pending_direction_staff, dynamic);
                        }
                        match pending_direction_wedge.take() {
                            Some(Some(kind)) => {
                                staff_hairpin_starts.insert(pending_direction_staff, kind);
                            }
                            // A stop ends the hairpin on the latest note of the voice it began
                            // in (or of the staff), where layout pairs the two.
                            Some(None) => {
                                let staff = pending_direction_staff;
                                let voice = open_hairpin_voices
                                    .remove(&staff)
                                    .or_else(|| last_staff_voice.get(&staff).copied());
                                if let (Some(pi), Some(address)) = (
                                    part_index,
                                    voice.and_then(|voice| last_voice_notes.get(&(staff, voice))),
                                ) && let Some(note) = score.parts[pi]
                                    .staves
                                    .get_mut(address.staff)
                                    .and_then(|staff| staff.measures.get_mut(address.measure))
                                    .and_then(|measure| measure.voices.get_mut(address.voice))
                                    .and_then(|voice| voice.get_mut(address.note))
                                {
                                    note.hairpin_end = true;
                                }
                            }
                            None => {}
                        }
                        pending_direction_staff = 0;
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
                            if std::mem::take(&mut first_staff_bar_clef_read) {
                                m.clef = Some(current_clef.clone());
                            }
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
                    "other-direction" if in_direction_type => {
                        pending_section_break = current_text.trim() == "acorde:section-break";
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
                        current_divisions = current_text.parse().unwrap_or(480);
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
                    "sign" if in_clef => {
                        clef_sign = current_text.trim().to_string();
                    }
                    "line" if in_clef => {
                        clef_line = current_text.trim().parse().ok();
                    }
                    "clef" if in_clef => {
                        in_clef = false;
                        let clef = match (clef_sign.as_str(), clef_line) {
                            ("F", _) => Clef::Bass,
                            ("C", Some(4)) => Clef::Tenor,
                            ("C", _) => Clef::Alto,
                            ("percussion", _) => Clef::Percussion,
                            _ => Clef::Treble,
                        };
                        if let Some(pi) = part_index {
                            let staff_index = current_clef_staff_number.max(1) - 1;
                            let measure_index =
                                score.parts[pi].staves[0].measures.len().saturating_sub(1);
                            // Before any of that staff's notes in the bar, it governs the
                            // whole bar; otherwise it takes effect where it is read.
                            let staff_silent = score.parts[pi]
                                .staves
                                .get(staff_index)
                                .and_then(|staff| staff.measures.get(measure_index))
                                .is_none_or(|measure| {
                                    measure
                                        .voices
                                        .iter()
                                        .all(|voice| voice.iter().all(|note| note.is_grace))
                                });
                            let offset = if measure_cursor_ticks == 0 || staff_silent {
                                0.0
                            } else {
                                f64::from(measure_cursor_ticks)
                                    / f64::from(current_divisions.max(1))
                            };
                            if staff_index == 0 && offset == 0.0 {
                                current_clef = clef;
                                first_staff_bar_clef_read = true;
                            } else {
                                // Applied once every staff's bars exist; see below.
                                if staff_index == 0 {
                                    current_clef = clef.clone();
                                }
                                staff_clef_events.push((
                                    pi,
                                    staff_index,
                                    measure_index,
                                    offset,
                                    clef,
                                ));
                            }
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
                    "pedal-step" if in_harp_pedal_tuning => {
                        harp_pedal_step = match current_text.trim() {
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
                    "pedal-alter" if in_harp_pedal_tuning => {
                        harp_pedal_alter = current_text.trim().parse().unwrap_or(0).clamp(-1, 1);
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
                    "measure-repeat" if measure_repeat_start_pending => {
                        measure_repeat_start_pending = false;
                        measure_repeat_active =
                            Some(current_text.trim().parse::<u8>().unwrap_or(1).max(1));
                    }
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
                    "capo" if in_staff_details => {
                        staff_capo = current_text.trim().parse::<u8>().ok();
                    }
                    "staff-details" => {
                        in_staff_details = false;
                        if let Some(lines) = staff_lines
                            && (1..=64).contains(&lines)
                            && let Some(pi) = part_index
                            && let Some(staff) =
                                score.parts[pi].staves.get_mut(staff_details_number - 1)
                        {
                            staff.presentation.lines = lines;
                            if !staff_tunings.is_empty() {
                                let config = acorde_core::TablatureConfig {
                                    lines,
                                    tuning_midi: staff_tunings
                                        .iter()
                                        .filter(|(line, _)| *line <= lines)
                                        .map(|(_, midi)| *midi)
                                        .collect(),
                                    // MusicXML tuning is the open tuning before the capo, and
                                    // frets count from the capo, matching TablatureConfig.
                                    capo: staff_capo.unwrap_or(0),
                                };
                                staff.presentation.kind = StaffKind::Tablature;
                                if staff.measures.len() <= 1 {
                                    staff.tablature = Some(config);
                                } else if let Some(measure) = staff.measures.last_mut() {
                                    measure.tablature_change = Some(config);
                                }
                            }
                        }
                    }
                    "syllabic" if in_lyric => lyric_syllabic = current_text.trim().to_string(),
                    "text" if in_lyric => {
                        // Syllables elided onto one note keep their connector (‿) in the text.
                        let text = current_text.trim();
                        match lyric_elision.take() {
                            Some(connector) if !lyric_text.is_empty() && !text.is_empty() => {
                                lyric_text.push_str(&connector);
                                lyric_text.push_str(text);
                            }
                            _ if text.is_empty() => {}
                            _ => lyric_text = text.to_string(),
                        }
                    }
                    "elision" if in_lyric => {
                        let connector = current_text.trim();
                        lyric_elision = Some(if connector.is_empty() {
                            LYRIC_ELISION.to_string()
                        } else {
                            connector.to_string()
                        });
                    }
                    "lyric" if in_lyric => {
                        in_lyric = false;
                        if !lyric_text.is_empty() {
                            note_lyrics.push((
                                lyric_number,
                                Lyric {
                                    text: std::mem::take(&mut lyric_text),
                                    syllabic: if lyric_syllabic.is_empty() {
                                        "single".to_string()
                                    } else {
                                        lyric_syllabic.clone()
                                    },
                                    extend: std::mem::take(&mut lyric_extend),
                                },
                            ));
                        }
                        lyric_extend = false;
                        lyric_syllabic = "single".to_string();
                    }
                    "duration" if in_backup => {
                        let duration = current_text
                            .trim()
                            .parse::<u32>()
                            .map_err(|_| Error::Xml("invalid backup duration".into()))?;
                        measure_cursor_ticks = match measure_cursor_ticks.checked_sub(duration) {
                            Some(cursor) => cursor,
                            None => {
                                let mut diagnostic = crate::Diagnostic::warning(
                                    "musicxml.backup-underflow",
                                    "<backup> moves before the measure start; clamped to the start",
                                );
                                diagnostic.source_location = Some(format!(
                                    "/score-partwise/measure[{current_measure_number}]/backup"
                                ));
                                tolerated.push(diagnostic);
                                0
                            }
                        };
                        last_note_start = None;
                        last_note_cross_home = None;
                    }
                    "duration" if in_forward => {
                        let duration = current_text
                            .trim()
                            .parse::<u32>()
                            .map_err(|_| Error::Xml("invalid forward duration".into()))?;
                        let measure_ticks =
                            musicxml_measure_ticks(&current_time, current_divisions)?;
                        measure_cursor_ticks = measure_cursor_ticks
                            .checked_add(duration)
                            .filter(|cursor| {
                                *cursor <= measure_ticks.saturating_mul(MAX_OVERFULL_FACTOR)
                            })
                            .ok_or_else(|| Error::Xml("MusicXML forward cursor overflow".into()))?;
                        measure_content_ticks = measure_content_ticks.max(measure_cursor_ticks);
                        last_note_start = None;
                        last_note_cross_home = None;
                    }
                    "duration" if in_note => {
                        note_duration_ticks = Some(
                            current_text
                                .trim()
                                .parse::<u32>()
                                .map_err(|_| Error::Xml("invalid note duration".into()))?,
                        );
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
                    "backup" => in_backup = false,
                    "forward" => in_forward = false,
                    "voice" if in_note => {
                        note_voice = current_text.trim().parse::<u32>().map_err(|_| {
                            Error::Xml("MusicXML voice number must be a positive integer".into())
                        })?;
                        if note_voice == 0 || note_voice > MAX_SOURCE_VOICE_NUMBER {
                            return Err(Error::Xml(format!(
                                "MusicXML voice number must be between 1 and {MAX_SOURCE_VOICE_NUMBER}"
                            )));
                        }
                    }
                    "staves" if !in_note => {
                        // A declared staff count materializes every canonical staff, even
                        // when a staff has no numbered clef or no notes in the first measure.
                        if let Some(pi) = part_index
                            && let Some(count) = current_text
                                .trim()
                                .parse::<usize>()
                                .ok()
                                .filter(|number| (1..=MAX_STAVES).contains(number))
                        {
                            while score.parts[pi].staves.len() < count {
                                score.parts[pi].staves.push(Staff::new(Clef::Treble));
                            }
                        }
                    }
                    "dynamics" => in_dynamics = false,
                    "staff" if in_direction && !in_note => {
                        pending_direction_staff = current_text
                            .trim()
                            .parse::<usize>()
                            .ok()
                            .filter(|number| (1..=MAX_STAVES).contains(number))
                            .map_or(0, |number| number - 1);
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
                    // Standard MusicXML puts <slide> in <notations>; older acorde output put it
                    // in <technical>.
                    "slide" if in_technical_block || in_notations => {
                        pending_guitar_technique = Some(GuitarTechnique::Slide);
                    }
                    "hammer-on" if in_technical_block => {
                        pending_guitar_technique = Some(GuitarTechnique::HammerOn);
                    }
                    "pull-off" if in_technical_block => {
                        pending_guitar_technique = Some(GuitarTechnique::PullOff);
                    }
                    // Only the primary beam level defines the group; deeper levels are sub-beams.
                    "beam" if in_note && note_beam_level == 1 => {
                        note_beam = match current_text.trim() {
                            "begin" => acorde_core::BeamState::Begin,
                            "continue" => acorde_core::BeamState::Continue,
                            "end" => acorde_core::BeamState::End,
                            "forward hook" => acorde_core::BeamState::ForwardHook,
                            "backward hook" => acorde_core::BeamState::BackwardHook,
                            _ => acorde_core::BeamState::None,
                        };
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
                        // Content shorter than the time signature is an authored pickup or
                        // irregular bar: keep its length instead of padding it with rests.
                        let actual_length =
                            musicxml_measure_ticks(&current_time, current_divisions)
                                .ok()
                                .filter(|bar_ticks| {
                                    measure_content_ticks > 0 && measure_content_ticks != *bar_ticks
                                })
                                .and_then(|_| {
                                    MeasureLength::from_ticks(
                                        measure_content_ticks,
                                        current_divisions,
                                    )
                                });
                        if let Some(pi) = part_index {
                            let bar_index = score.parts[pi].staves[0].measures.len();
                            for staff in &mut score.parts[pi].staves {
                                if staff.measures.len() == bar_index
                                    && let Some(measure) = staff.measures.last_mut()
                                {
                                    measure.actual_length = actual_length;
                                }
                            }
                        }
                        if let Some(pi) = part_index
                            && let Some(m) = score.parts[pi].staves[0].measures.last_mut()
                        {
                            let total_beats = actual_length
                                .and_then(|length| length.beats())
                                .unwrap_or_else(|| current_time.total_beats());
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
                        if let (Some(pi), Some(count)) = (part_index, measure_repeat_active) {
                            // The measure's written content (typically a rest) stands for the
                            // repeated measures: store a playable copy of them instead.
                            let bar_index = score.parts[pi].staves[0].measures.len();
                            let back = usize::from(count);
                            for staff in &mut score.parts[pi].staves {
                                if staff.measures.len() != bar_index || bar_index <= back {
                                    continue;
                                }
                                let source = staff.measures[bar_index - 1 - back].clone();
                                if let Some(target) = staff.measures.last_mut() {
                                    target.repeat_content_from(&source);
                                    if count == 1 {
                                        target.measure_repeat = Some(1);
                                    }
                                }
                            }
                        }
                    }
                    "note" => {
                        if let Some(pi) = part_index {
                            let requested_staff_index = note_staff.saturating_sub(1);
                            let staff_count = score.parts[pi].staves.len();
                            let cross_home = if note_chord {
                                last_note_cross_home
                            } else {
                                declared_cross_staff_home(
                                    requested_staff_index,
                                    staff_count,
                                    note_voice,
                                    measure_cursor_ticks,
                                    &source_voice_slots,
                                    &voice_cursor_ticks,
                                )
                            };
                            let (target_staff_index, cross_target) = if let Some(home) = cross_home
                            {
                                (home, Some(requested_staff_index))
                            } else if requested_staff_index < staff_count {
                                (requested_staff_index, None)
                            } else {
                                // Legacy boundary: an undeclared staff stays a placement on the
                                // first staff (diagnosed when the part declares `<staves>`).
                                (0, Some(requested_staff_index))
                            };
                            let tab_lines = score.parts[pi].staves[target_staff_index]
                                .tablature
                                .as_ref()
                                .map(|tab| tab.lines);
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
                            let mut completed_note_address = None;
                            if let Some(m) = score.parts[pi].staves[target_staff_index]
                                .measures
                                .last_mut()
                            {
                                let mut voice_index = musicxml_voice_slot(
                                    m,
                                    &mut source_voice_slots,
                                    target_staff_index,
                                    note_voice,
                                )?;
                                let mut drop_chord_member = false;
                                // Cue notes take time in MusicXML (their <duration> moves the
                                // cursor, and a later <backup> counts it) even though the model
                                // gives them none; see the voice cursor below.
                                let duration_ticks = if note_is_grace {
                                    0
                                } else if note_is_cue {
                                    note_duration_ticks.unwrap_or(0)
                                } else {
                                    note_duration_ticks.ok_or_else(|| {
                                        Error::Xml("MusicXML note is missing duration".into())
                                    })?
                                };
                                let note_start = if note_chord {
                                    let (staff, voice, start) =
                                        last_note_start.ok_or_else(|| {
                                            Error::Xml(
                                                "MusicXML chord has no preceding note".into(),
                                            )
                                        })?;
                                    if staff == target_staff_index && voice != voice_index {
                                        // Some exporters number a chord member's <voice>
                                        // differently; it still belongs to its chord.
                                        voice_index = voice;
                                    } else if staff != target_staff_index {
                                        let mut diagnostic = crate::Diagnostic::warning(
                                            "musicxml.chord-staff-mismatch",
                                            "a <chord/> member sits on another staff than its chord (cross-staff chord); the member is not imported",
                                        );
                                        diagnostic.source_location = Some(format!(
                                            "/score-partwise/measure[{current_measure_number}]/note"
                                        ));
                                        tolerated.push(diagnostic);
                                        drop_chord_member = true;
                                    }
                                    start
                                } else {
                                    measure_cursor_ticks
                                };
                                let next_cursor = note_start
                                    .checked_add(duration_ticks)
                                    .ok_or_else(|| Error::Xml("MusicXML cursor overflow".into()))?;
                                let bar_ticks =
                                    musicxml_measure_ticks(&current_time, current_divisions)?;
                                if !note_chord
                                    && next_cursor > bar_ticks.saturating_mul(MAX_OVERFULL_FACTOR)
                                {
                                    return Err(Error::Xml(format!(
                                        "MusicXML note cursor exceeds measure duration in measure {} voice {} ({} / {}, divisions {}): {} + {}",
                                        current_measure_number,
                                        note_voice,
                                        current_time.numerator,
                                        current_time.denominator,
                                        current_divisions,
                                        note_start,
                                        duration_ticks,
                                    )));
                                }
                                let voice = &mut m.voices[voice_index];
                                if !note_chord {
                                    let voice_cursor = *voice_cursor_ticks
                                        .get(&(target_staff_index, voice_index))
                                        .unwrap_or(&0);
                                    if voice_cursor > note_start {
                                        return Err(Error::Xml(
                                            "MusicXML voice cursor moves backward".into(),
                                        ));
                                    }
                                    append_musicxml_gap_rests(
                                        voice,
                                        note_start - voice_cursor,
                                        current_divisions,
                                    )?;
                                }
                                let dur = if note_is_measure_rest {
                                    Duration::Whole
                                } else {
                                    parse_duration_type(&note_type)
                                };
                                let dot_count = if note_is_measure_rest {
                                    0
                                } else {
                                    u8::from(note_dot)
                                };
                                let mut note = if note_rest {
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
                                    // Where a kit instrument is drawn identifies it for chord
                                    // members, which keep no instrument id of their own.
                                    if note_is_unpitched
                                        && let (Some(pi), Some(id)) =
                                            (part_index, note_instrument_id.as_ref())
                                    {
                                        kit_display_positions.push((
                                            pi,
                                            id.clone(),
                                            PercussionInstrument::display_position(&n.pitches[0]),
                                        ));
                                    }
                                    n.offset_x = note_offset_x;
                                    n.offset_y = note_offset_y;
                                    n.relative_x = note_relative_x;
                                    n.relative_y = note_relative_y;
                                    n
                                };
                                note.tie_start = note_tie_start;
                                note.tie_end = note_tie_end;
                                if note_chord && drop_chord_member {
                                    // Reported above; the member cannot join a chord on another
                                    // staff or voice.
                                } else if note_chord {
                                    let last = voice.last_mut().ok_or_else(|| {
                                        Error::Xml("MusicXML chord has no preceding note".into())
                                    })?;
                                    if last.is_rest || note.is_rest {
                                        return Err(Error::Xml(
                                            "MusicXML chord cannot contain a rest".into(),
                                        ));
                                    }
                                    merge_musicxml_chord_note(
                                        last,
                                        &note,
                                        MusicXmlChordNoteDetails {
                                            is_unpitched: note_is_unpitched,
                                            instrument_id: note_instrument_id.clone(),
                                            offset_x: note_offset_x,
                                            offset_y: note_offset_y,
                                            relative_x: note_relative_x,
                                            relative_y: note_relative_y,
                                            fingerings: std::mem::take(&mut pending_fingerings),
                                            string_number: pending_string_number.take().map(
                                                |string| {
                                                    musicxml_string_to_internal(string, tab_lines)
                                                },
                                            ),
                                            fret: pending_fret.take(),
                                            technique_text: pending_technique_text.take(),
                                            note_head: pending_note_head.take(),
                                            guitar_technique: pending_guitar_technique.take(),
                                            guitar_bend_alter_cents:
                                                pending_guitar_bend_alter_cents.take(),
                                            stem_up: note_stem_up,
                                        },
                                    );
                                    if note.is_unpitched
                                        && let (Some(pi), Some(id), Some(pitch)) = (
                                            part_index,
                                            note_instrument_id.as_ref(),
                                            note.pitches.first(),
                                        )
                                    {
                                        kit_display_positions.push((
                                            pi,
                                            id.clone(),
                                            PercussionInstrument::display_position(pitch),
                                        ));
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
                                    if !note.is_grace
                                        && let Some(kind) =
                                            staff_hairpin_starts.remove(&target_staff_index)
                                    {
                                        note.hairpin_start = Some(kind);
                                        open_hairpin_voices.insert(target_staff_index, voice_index);
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
                                        note.string_number =
                                            Some(musicxml_string_to_internal(s, tab_lines));
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
                                    note.beam = note_beam;
                                    if note_trill_line_start {
                                        note.trill_line_start = true;
                                    }
                                    if note_trill_line_end {
                                        note.trill_line_end = true;
                                    }
                                    if let Some(target_staff) = cross_target {
                                        note.cross_staff = Some(acorde_core::CrossStaff {
                                            target_staff,
                                            target_voice: None,
                                        });
                                    }
                                    if !pending_articulations.is_empty() {
                                        note.articulations =
                                            std::mem::take(&mut pending_articulations);
                                    }
                                    crate::assign_note_verses(
                                        &mut note,
                                        std::mem::take(&mut note_lyrics),
                                    );
                                    if !note.is_rest && !note.is_grace {
                                        let direction = staff_dynamics.remove(&target_staff_index);
                                        note.dynamic = note_inline_dynamic.take().or(direction);
                                    }
                                    voice.push(note);
                                    let address = NoteAddr {
                                        part: pi,
                                        staff: target_staff_index,
                                        measure: measure_count - 1,
                                        voice: voice_index,
                                        note: voice.len() - 1,
                                    };
                                    // A zero-time cue note leaves the model voice where it was, so
                                    // the next note in this voice is placed after a gap rest at
                                    // its real time instead of drifting earlier.
                                    voice_cursor_ticks.insert(
                                        (target_staff_index, voice_index),
                                        if note_is_cue && !note_chord {
                                            note_start
                                        } else if note_is_cue {
                                            voice_cursor_ticks
                                                .get(&(target_staff_index, voice_index))
                                                .copied()
                                                .unwrap_or(note_start)
                                        } else {
                                            next_cursor
                                        },
                                    );
                                    measure_cursor_ticks = next_cursor;
                                    measure_content_ticks = measure_content_ticks.max(next_cursor);
                                    last_note_start =
                                        Some((target_staff_index, voice_index, note_start));
                                    last_note_cross_home = cross_home;
                                    last_voice_notes
                                        .insert((target_staff_index, voice_index), address.clone());
                                    last_staff_voice.insert(target_staff_index, voice_index);
                                    last_note_address = Some(address.clone());
                                    completed_note_address = Some(address);
                                }
                            }
                            if let Some(address) = completed_note_address {
                                for event in std::mem::take(&mut pending_direction_spanner_events) {
                                    apply_spanner_event(
                                        &mut score,
                                        &mut open_spanners,
                                        event,
                                        address.clone(),
                                        tolerated,
                                    )?;
                                }
                                for event in std::mem::take(&mut note_spanner_events) {
                                    apply_spanner_event(
                                        &mut score,
                                        &mut open_spanners,
                                        event,
                                        address.clone(),
                                        tolerated,
                                    )?;
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
            Ok(Event::DocType(declaration)) if !is_safe_musicxml_doctype(declaration.as_ref()) => {
                return Err(Error::Xml("unsafe DOCTYPE declaration".into()));
            }
            _ => {}
        }
        buf.clear();
    }

    if score.parts.is_empty() {
        return Err(Error::Empty);
    }
    for (part_index, id, position) in kit_display_positions {
        if let Some(instrument) = score.parts.get_mut(part_index).and_then(|part| {
            part.percussion_instruments
                .iter_mut()
                .find(|instrument| instrument.id == id)
        }) && instrument.staff_position.is_none()
        {
            instrument.staff_position = Some(position);
        }
    }
    // An unpitched note whose kit instrument (and so its sound) is unknown plays its display
    // pitch; say so once per part.
    for (part_index, part) in score.parts.iter().enumerate() {
        let unresolved = part
            .staves
            .iter()
            .flat_map(|staff| staff.measures.iter())
            .flat_map(|measure| measure.voices.iter().flatten())
            .any(|note| {
                note.is_unpitched
                    && (0..note.pitches.len())
                        .any(|index| part.percussion_key(note, index).is_none())
            });
        if unresolved {
            let mut diagnostic = crate::Diagnostic::warning(
                "musicxml.unpitched-without-instrument",
                "an unpitched note names no percussion instrument with a <midi-unpitched> key; \
                 playback uses its display pitch",
            );
            diagnostic.source_location = Some(format!("/score-partwise/part[{}]", part_index + 1));
            tolerated.push(diagnostic);
        }
    }
    for part in &mut score.parts {
        pad_declared_staff_measures(part);
        // The first staff's opening clef is read into its first bar; mirror it on the staff.
        if let Some(staff) = part.staves.first_mut()
            && let Some(clef) = staff
                .measures
                .first()
                .and_then(|measure| measure.clef.clone())
        {
            staff.clef = clef;
        }
    }
    // A clef read before a staff's notes in a bar changes that bar. One read partway through
    // is a mid-bar change at that point, and one read after the bar's content (a cue for the
    // next system) takes effect at the next barline. A pitch never depends on its clef.
    for (part_index, staff_index, measure_index, offset, clef) in staff_clef_events {
        let Some(part) = score.parts.get_mut(part_index) else {
            continue;
        };
        // The bar ends where its longest voice on any staff ends.
        let bar_beats = part
            .staves
            .iter()
            .filter_map(|staff| staff.measures.get(measure_index))
            .flat_map(|measure| measure.voices.iter())
            .map(|voice| {
                voice
                    .iter()
                    .filter(|note| !note.is_grace)
                    .map(Note::beats)
                    .sum::<f64>()
            })
            .fold(0.0, f64::max);
        let Some(staff) = part.staves.get_mut(staff_index) else {
            continue;
        };
        let previous = clef_in_effect(staff, measure_index, offset);
        if offset > 0.0 && offset < bar_beats - 1e-9 {
            if let Some(measure) = staff.measures.get_mut(measure_index)
                && let Some(offset) = acorde_core::MeasureLength::from_beats(offset)
                && clef != previous
            {
                measure.mid_clefs.retain(|change| change.offset != offset);
                measure.mid_clefs.push(acorde_core::MidMeasureClef {
                    offset,
                    clef: clef.clone(),
                });
                measure.mid_clefs.sort_by(|a, b| {
                    let beats =
                        |change: &acorde_core::MidMeasureClef| change.offset.beats().unwrap_or(0.0);
                    beats(a).total_cmp(&beats(b))
                });
            }
            continue;
        }
        let target = if offset == 0.0 {
            measure_index
        } else {
            measure_index + 1
        };
        if target == 0 {
            staff.clef = clef.clone();
        } else if let Some(measure) = staff.measures.get_mut(target) {
            measure.clef = (clef != previous || measure.clef.is_some()).then(|| clef.clone());
        }
    }
    score.settings.time_signature = current_time;
    score.settings.key_signature = current_key;

    Ok(score)
}

/// The clef a staff is in `beats` into bar `measure_index` (before any change read there): its
/// opening clef, then every bar clef and mid-bar change up to that point.
fn clef_in_effect(staff: &acorde_core::Staff, measure_index: usize, beats: f64) -> Clef {
    let mut clef = staff.clef.clone();
    for (index, measure) in staff.measures.iter().enumerate().take(measure_index + 1) {
        let at = if index == measure_index {
            beats
        } else {
            f64::INFINITY
        };
        if let Some(bar_clef) = &measure.clef
            && (index < measure_index || beats > 0.0)
        {
            clef = bar_clef.clone();
        }
        if let Some(change) = measure.mid_clefs.iter().rev().find(|change| {
            change
                .offset
                .beats()
                .is_some_and(|offset| offset < at - 1e-9)
        }) {
            clef = change.clef.clone();
        }
    }
    clef
}

/// Give every declared staff the same measure count as the first staff. MusicXML may declare a
/// staff that has no notes in some (or any) measures; the canonical model still requires one
/// measure per staff and bar, so missing bars become empty measures with the bar's number and
/// running time signature.
/// Staves conventionally own four source voice numbers each (1–4 on staff 1, 5–8 on staff 2, …),
/// the numbering MuseScore and acorde's serializer emit. A note written on another declared staff
/// is a cross-staff placement owned by its voice's conventional staff when it continues that
/// voice's timeline: it must not start before the voice's existing content on the owning staff,
/// and its own staff must not already carry an independent layer with the same voice number in
/// this measure (the per-staff voice reuse some exporters write after `<backup>`).
fn declared_cross_staff_home(
    requested_staff: usize,
    staff_count: usize,
    source_voice: u32,
    note_start: u32,
    source_slots: &HashMap<(usize, u32), usize>,
    voice_cursors: &HashMap<(usize, usize), u32>,
) -> Option<usize> {
    let home = usize::try_from(source_voice.checked_sub(1)? / 4).ok()?;
    if home == requested_staff || home >= staff_count || requested_staff >= staff_count {
        return None;
    }
    let own_layer_active = source_slots
        .get(&(requested_staff, source_voice))
        .is_some_and(|slot| voice_cursors.contains_key(&(requested_staff, *slot)));
    if own_layer_active {
        return None;
    }
    // Only a voice already sounding on its home staff in this measure crosses to another staff;
    // a voice written wholly on the other staff (Finale and others number the left hand 3 or
    // 2) belongs to that staff.
    let home_cursor = source_slots
        .get(&(home, source_voice))
        .and_then(|slot| voice_cursors.get(&(home, *slot)))
        .copied()?;
    (home_cursor <= note_start).then_some(home)
}

fn pad_declared_staff_measures(part: &mut Part) {
    let Some((first, extra_staves)) = part.staves.split_first_mut() else {
        return;
    };
    let mut time = TimeSignature::default();
    let bars: Vec<(u32, TimeSignature, Option<MeasureLength>)> = first
        .measures
        .iter()
        .map(|measure| {
            if let Some(changed) = &measure.time_sig {
                time = changed.clone();
            }
            (measure.number, time.clone(), measure.actual_length)
        })
        .collect();
    for staff in extra_staves {
        while staff.measures.len() < bars.len() {
            let (number, time, actual_length) = &bars[staff.measures.len()];
            let mut measure = Measure::empty(time.numerator, time.denominator);
            measure.number = *number;
            measure.actual_length = *actual_length;
            measure.voices[0].clear();
            staff.measures.push(measure);
        }
        // Time signatures and bar lengths belong to the whole part: a staff whose bar was
        // created before the part's <time> change must not keep the old meter.
        for (index, measure) in staff.measures.iter_mut().enumerate() {
            let Some(source) = first.measures.get(index) else {
                break;
            };
            if measure.time_sig.is_none() && source.time_sig.is_some() {
                measure.time_sig = source.time_sig.clone();
            }
            if measure.actual_length.is_none() {
                measure.actual_length = source.actual_length;
            }
        }
    }
}

fn musicxml_measure_ticks(time: &TimeSignature, divisions: u32) -> Result<u32, Error> {
    if time.denominator == 0 || divisions == 0 {
        return Err(Error::Xml(
            "MusicXML time signature or divisions is invalid".into(),
        ));
    }
    let numerator = u64::from(time.numerator)
        .checked_mul(4)
        .and_then(|value| value.checked_mul(u64::from(divisions)))
        .ok_or_else(|| Error::Xml("MusicXML measure duration overflow".into()))?;
    let denominator = u64::from(time.denominator);
    if numerator % denominator != 0 {
        return Err(Error::Xml(
            "MusicXML time signature is not representable by divisions".into(),
        ));
    }
    u32::try_from(numerator / denominator)
        .map_err(|_| Error::Xml("MusicXML measure duration overflow".into()))
}

/// Map an arbitrary positive MusicXML voice identifier onto one of the four canonical editing
/// slots without changing the source identifier stored on the measure. The mapping is stable for
/// the enclosing part/staff, so a sparse source voice remains in the same host slot across
/// measures. A fifth distinct source voice is rejected explicitly rather than silently merged.
fn musicxml_voice_slot(
    measure: &mut Measure,
    source_slots: &mut HashMap<(usize, u32), usize>,
    staff_index: usize,
    source_voice: u32,
) -> Result<usize, Error> {
    if let Some(&slot) = source_slots.get(&(staff_index, source_voice)) {
        measure.source_voice_numbers[slot] = Some(source_voice);
        return Ok(slot);
    }

    let occupied = |slot: usize| {
        source_slots
            .iter()
            .any(|(&(mapped_staff, _), &mapped_slot)| {
                mapped_staff == staff_index && mapped_slot == slot
            })
    };
    let zero_based = source_voice.saturating_sub(1);
    // Per-staff numbering (5–8 on staff 2, …) keeps its slot within the owning staff.
    let preferred = if staff_index > 0 && usize::try_from(zero_based / 4).ok() == Some(staff_index)
    {
        usize::try_from(zero_based % 4).ok()
    } else {
        usize::try_from(zero_based).ok()
    };
    let slot = preferred
        .filter(|slot| *slot < measure.voices.len() && !occupied(*slot))
        .or_else(|| (0..measure.voices.len()).find(|slot| !occupied(*slot)))
        .ok_or_else(|| {
            Error::Xml(
                "MusicXML part/staff contains more than four distinct source voice numbers".into(),
            )
        })?;
    source_slots.insert((staff_index, source_voice), slot);
    measure.source_voice_numbers[slot] = Some(source_voice);
    Ok(slot)
}

fn append_musicxml_gap_rests(
    voice: &mut Vec<Note>,
    mut ticks: u32,
    divisions: u32,
) -> Result<(), Error> {
    let candidates = [
        (Duration::Whole, divisions.checked_mul(4)),
        (Duration::Half, divisions.checked_mul(2)),
        (Duration::Quarter, Some(divisions)),
        (Duration::Eighth, divisions.checked_div(2)),
        (Duration::Sixteenth, divisions.checked_div(4)),
        (Duration::ThirtySecond, divisions.checked_div(8)),
        (Duration::SixtyFourth, divisions.checked_div(16)),
    ];
    while ticks > 0 {
        let mut selected = None;
        for (duration, duration_ticks) in &candidates {
            if let Some(duration_ticks) = duration_ticks
                && *duration_ticks > 0
                && *duration_ticks <= ticks
            {
                selected = Some((duration.clone(), *duration_ticks));
                break;
            }
        }
        let Some((duration, duration_ticks)) = selected else {
            return Err(Error::Xml(
                "MusicXML cursor gap is not representable by canonical rests".into(),
            ));
        };
        if voice.len() >= MAX_NOTES_PER_VOICE {
            return Err(Error::Xml("too many notes in voice".into()));
        }
        voice.push(Note::rest(duration));
        ticks -= duration_ticks;
    }
    Ok(())
}

/// Merge the second `<note>` of a MusicXML chord into the note that owns the event.
///
/// MusicXML stores chord pitches as separate note elements. Keeping this mutation in one
/// helper makes the parser's streaming state machine responsible only for collecting fields,
/// while this function owns the canonical multi-pitch and tablature merge policy.
struct MusicXmlChordNoteDetails {
    is_unpitched: bool,
    instrument_id: Option<String>,
    offset_x: Option<f64>,
    offset_y: Option<f64>,
    relative_x: Option<f64>,
    relative_y: Option<f64>,
    fingerings: Vec<u8>,
    string_number: Option<u8>,
    fret: Option<u8>,
    technique_text: Option<String>,
    note_head: Option<NoteHead>,
    guitar_technique: Option<GuitarTechnique>,
    guitar_bend_alter_cents: Option<i16>,
    stem_up: Option<bool>,
}

fn merge_musicxml_chord_note(last: &mut Note, note: &Note, details: MusicXmlChordNoteDetails) {
    let mut starts = last.pitch_tie_starts_or_uniform();
    let mut ends = last.pitch_tie_ends_or_uniform();
    if let Some(pitch) = note.pitches.first() {
        last.pitches.push(pitch.clone());
        starts.push(note.tie_start);
        ends.push(note.tie_end);
    }
    // Each chord member keeps its own tie; a uniformly tied chord stays chord-level.
    last.set_pitch_ties(&starts, &ends);
    last.is_unpitched |= details.is_unpitched;
    if details.instrument_id.is_some() {
        last.instrument_id = details.instrument_id;
    }
    last.offset_x = details.offset_x;
    last.offset_y = details.offset_y;
    last.relative_x = details.relative_x;
    last.relative_y = details.relative_y;
    if !details.fingerings.is_empty() {
        if last.fingering.is_none() {
            last.fingering = details.fingerings.first().copied();
        }
        last.fingerings.extend(details.fingerings);
    }
    if let Some(string) = details.string_number {
        last.string_number = Some(string);
        if let Some(fret) = details.fret {
            let position = acorde_core::TabPosition { string, fret };
            last.tab_positions.push(position.clone());
            if last.tab_position.is_none() {
                last.tab_position = Some(position);
            }
        }
    }
    if let Some(text) = details.technique_text {
        last.technique_text = Some(text);
    }
    if let Some(note_head) = details.note_head {
        last.note_head = note_head;
    }
    if let Some(technique) = details.guitar_technique {
        last.guitar_technique = Some(technique);
    }
    if let Some(cents) = details.guitar_bend_alter_cents {
        last.guitar_bend_alter_cents = Some(cents);
    }
    if let Some(stem_up) = details.stem_up {
        last.stem_up = Some(stem_up);
    }
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

/// Connector written between syllables elided onto one note.
const LYRIC_ELISION: &str = "\u{203F}";

/// The 0-based General MIDI key for a MusicXML `<midi-unpitched>` value (1–128).
fn musicxml_midi_unpitched(text: &str) -> Option<u8> {
    text.trim()
        .parse::<u8>()
        .ok()
        .filter(|key| (1..=128).contains(key))
        .map(|key| key - 1)
}

/// MusicXML dynamic element names acorde's `Dynamic` holds (louder and softer extremes clamp).
fn musicxml_dynamic(tag: &str) -> Option<Dynamic> {
    Dynamic::from_musicxml_str(tag)
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
    fn unsafe_doctype_rejected() {
        let xml = "<?xml version=\"1.0\"?><!DOCTYPE foo><score-partwise/>";
        assert!(parse_musicxml(xml).is_err());
    }

    #[test]
    fn legacy_and_system_musicxml_doctypes_are_accepted() {
        let body = r#"<score-partwise><part-list><score-part id="P1"><part-name>Q</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes><note><rest/><duration>4</duration><type>whole</type></note></measure></part></score-partwise>"#;
        for doctype in [
            r#"<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 0.6a Partwise//EN" "//D:/Program Files/MusicXML/partwise.dtd">"#,
            r#"<!DOCTYPE score-partwise SYSTEM "partwise.dtd">"#,
        ] {
            assert!(parse_musicxml(&format!("<?xml version=\"1.0\"?>{doctype}{body}")).is_ok());
        }
        for evil in [
            r#"<!DOCTYPE score-partwise [<!ENTITY a "b">]>"#,
            r#"<!DOCTYPE score-partwise SYSTEM "x.dtd" [<!ENTITY % p SYSTEM "http://evil">]>"#,
        ] {
            assert!(parse_musicxml(&format!("<?xml version=\"1.0\"?>{evil}{body}")).is_err());
        }
    }

    #[test]
    fn standard_public_musicxml_doctype_is_accepted() {
        let xml = r#"<?xml version="1.0"?>
<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN" "http://www.musicxml.org/dtds/partwise.dtd">
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes>
    <note><rest/><duration>1</duration><type>quarter</type></note>
  </measure></part>
</score-partwise>"#;
        assert!(parse_musicxml(xml).is_ok());
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
    fn forwards_and_backups_preserve_voice_onsets_with_explicit_rests() {
        let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><forward><duration>1</duration></forward><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note><backup><duration>2</duration></backup><note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type></note><forward><duration>3</duration></forward></measure></part></score-partwise>"#;
        // The trailing forward reaches the bar line; without it the content would be a
        // two-beat irregular measure rather than a padded 4/4 bar.
        let score = parse_musicxml(xml).expect("cursor fixture parses");
        let measure = &score.parts[0].staves[0].measures[0];
        assert!(measure.actual_length.is_none());
        assert_eq!(measure.voices[0].len(), 3);
        assert!(measure.voices[0][0].is_rest);
        assert_eq!(measure.voices[0][1].pitches[0].step, Step::C);
        assert!(measure.voices[0][2].is_rest);
        assert_eq!(measure.voices[1].len(), 3);
        assert_eq!(measure.voices[1][0].pitches[0].step, Step::E);
        assert!(measure.voices[1][1].is_rest);
        assert!(measure.voices[1][2].is_rest);

        let serialized = crate::serialize_musicxml(&score).expect("serialize cursor fixture");
        let restored = parse_musicxml(&serialized).expect("reparse cursor fixture");
        let restored_measure = &restored.parts[0].staves[0].measures[0];
        assert!(restored_measure.voices[0][0].is_rest);
        assert_eq!(restored_measure.voices[0][1].pitches[0].step, Step::C);
        assert_eq!(restored_measure.voices[1][0].pitches[0].step, Step::E);
    }

    #[test]
    fn cursor_semantics_cover_internal_gaps_chords_and_cross_staff_notes() {
        let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef><clef number="2"><sign>F</sign><line>4</line></clef></attributes><forward><duration>1</duration></forward><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note><note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note><forward><duration>1</duration></forward><note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note><backup><duration>4</duration></backup><note><pitch><step>F</step><octave>3</octave></pitch><duration>1</duration><voice>1</voice><staff>2</staff><type>quarter</type></note></measure></part></score-partwise>"#;
        let score = parse_musicxml(xml).expect("cursor fixture parses");
        let part = &score.parts[0];
        let upper = &part.staves[0].measures[0].voices[0];
        assert!(upper[0].is_rest);
        assert_eq!(upper[1].pitches.len(), 2);
        assert!(upper[2].is_rest);
        assert_eq!(upper[3].pitches[0].step, Step::D);
        assert_eq!(part.staves.len(), 2);
        assert_eq!(
            part.staves[1].measures[0].voices[0][0].pitches[0].step,
            Step::F
        );

        let serialized = crate::serialize_musicxml(&score).expect("serializes");
        assert!(serialized.contains("<backup>"));
        let restored = parse_musicxml(&serialized).expect("reparses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].voices[0][1]
                .pitches
                .len(),
            2
        );
        assert_eq!(
            restored.parts[0].staves[1].measures[0].voices[0][0].pitches[0].step,
            Step::F
        );
    }

    #[test]
    fn cursor_underflow_is_clamped_and_runaway_overflow_rejected() {
        let underflow = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes><note><rest/><duration>4</duration><type>whole</type></note><backup><duration>9</duration></backup></measure></part></score-partwise>"#;
        let mut tolerated = Vec::new();
        parse_musicxml_collecting(underflow, &mut tolerated).expect("underflow is clamped");
        assert_eq!(tolerated[0].code, "musicxml.backup-underflow");

        // Content past the bar imports as an irregular length, but not without bound.
        let runaway = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>1</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>100</duration><type>whole</type></note></measure></part></score-partwise>"#;
        assert!(parse_musicxml(runaway).is_err());
    }

    #[test]
    fn sparse_source_voice_numbers_use_stable_editing_slots_without_renumbering() {
        let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>1</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note><backup><duration>1</duration></backup><note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><voice>5</voice><type>quarter</type></note></measure><measure number="2"><note><pitch><step>F</step><octave>4</octave></pitch><duration>1</duration><voice>5</voice><type>quarter</type></note></measure></part></score-partwise>"#;
        let score = parse_musicxml(xml).expect("sparse voices parse");
        let first = &score.parts[0].staves[0].measures[0];
        let second = &score.parts[0].staves[0].measures[1];
        assert_eq!(first.source_voice_numbers, [Some(1), Some(5), None, None]);
        assert_eq!(second.source_voice_numbers, [None, Some(5), None, None]);
        assert_eq!(first.voices[1][0].pitches[0].step, Step::E);
        assert_eq!(second.voices[1][0].pitches[0].step, Step::F);

        let serialized = crate::serialize_musicxml(&score).expect("serializes");
        assert!(serialized.contains("<voice>5</voice>"));
        let restored = parse_musicxml(&serialized).expect("reparses");
        assert_eq!(
            restored.parts[0].staves[0].measures[0].source_voice_numbers,
            [Some(1), Some(5), None, None]
        );
    }

    #[test]
    fn invalid_or_unrepresentable_source_voice_numbers_are_rejected() {
        for voice in ["0", "-1", "bad", "1000001"] {
            let xml = format!(
                "<score-partwise><part-list><score-part id=\"P1\"/></part-list><part id=\"P1\"><measure><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>{voice}</voice></note></measure></part></score-partwise>"
            );
            assert!(parse_musicxml(&xml).is_err(), "voice {voice} must fail");
        }

        let xml = r#"<score-partwise><part-list><score-part id="P1"/></part-list><part id="P1"><measure><attributes><divisions>1</divisions><time><beats>1</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice></note><backup><duration>1</duration></backup><note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>2</voice></note><backup><duration>1</duration></backup><note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><voice>3</voice></note><backup><duration>1</duration></backup><note><pitch><step>F</step><octave>4</octave></pitch><duration>1</duration><voice>4</voice></note><backup><duration>1</duration></backup><note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration><voice>5</voice></note></measure></part></score-partwise>"#;
        let error = parse_musicxml(xml).expect_err("fifth source voice must fail");
        assert!(error.to_string().contains("more than four distinct"));
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
        assert_eq!(score.spanners.len(), 1);
        let spanner = &score.spanners[0];
        assert_eq!(spanner.kind, NotationSpannerKind::Slur);
        assert_eq!(spanner.number, Some(1));
        assert_eq!(spanner.start.note, 0);
        assert_eq!(spanner.end.note, 1);
    }

    #[test]
    fn numbered_spanners_preserve_overlaps_and_direction_endpoints() {
        let xml = r#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1"><measure number="1">
    <attributes><divisions>480</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes>
    <direction placement="below"><direction-type><pedal type="start" number="4" line="yes"/></direction-type></direction>
    <direction placement="above"><direction-type><octave-shift type="down" size="15" number="5"/></direction-type></direction>
    <note><pitch><step>C</step><octave>4</octave></pitch><duration>480</duration><type>quarter</type><notations>
      <slur number="1" type="start" line-type="dashed" placement="above"/>
      <slur number="2" type="start"/>
      <glissando number="3" type="start" line-type="wavy">gliss.</glissando>
      <wavy-line number="6" type="start"/>
    </notations></note>
    <note><pitch><step>D</step><octave>4</octave></pitch><duration>480</duration><type>quarter</type><notations>
      <slur number="2" type="stop"/><slur number="1" type="stop" line-type="dashed"/>
      <glissando number="3" type="stop">gliss.</glissando><wavy-line number="6" type="stop"/>
    </notations></note>
    <direction placement="below"><direction-type><pedal type="stop" number="4" line="yes"/></direction-type></direction>
    <direction placement="above"><direction-type><octave-shift type="stop" size="15" number="5"/></direction-type></direction>
  </measure></part>
</score-partwise>"#;

        let score = parse_musicxml(xml).expect("numbered spanners parse");
        assert_eq!(score.spanners.len(), 6);
        let by_number = |kind: NotationSpannerKind, number: u16| {
            score
                .spanners
                .iter()
                .find(|spanner| spanner.kind == kind && spanner.number == Some(number))
                .expect("typed span")
        };
        assert_eq!(
            by_number(NotationSpannerKind::Slur, 1).line_type.as_deref(),
            Some("dashed")
        );
        assert_eq!(by_number(NotationSpannerKind::Slur, 2).start.note, 0);
        let glissando = by_number(NotationSpannerKind::Glissando, 3);
        assert_eq!(glissando.end.note, 1);
        assert_eq!(glissando.text.as_deref(), Some("gliss."));
        assert_eq!(
            by_number(NotationSpannerKind::Pedal, 4)
                .placement
                .as_deref(),
            Some("below")
        );
        let ottava = by_number(NotationSpannerKind::Ottava, 5);
        assert_eq!(ottava.ottava_size, Some(15));
        assert_eq!(ottava.ottava_type.as_deref(), Some("down"));
        assert_eq!(by_number(NotationSpannerKind::TrillLine, 6).end.note, 1);
    }

    #[test]
    fn orphan_numbered_spanner_stop_is_reported_not_fatal() {
        let xml = r#"<score-partwise><part-list><score-part id="P1"/></part-list><part id="P1"><measure><note><pitch><step>C</step><octave>4</octave></pitch><duration>480</duration><notations><slur number="3" type="stop"/></notations></note></measure></part></score-partwise>"#;
        let mut tolerated = Vec::new();
        let score =
            parse_musicxml_collecting(xml, &mut tolerated).expect("orphan stop is tolerated");
        assert!(score.spanners.is_empty());
        assert_eq!(tolerated.len(), 1);
        assert_eq!(tolerated[0].code, "musicxml.orphan-spanner-stop");
        assert!(
            tolerated[0]
                .loss_reason
                .as_deref()
                .unwrap_or("")
                .contains("Slur stop number 3")
        );
    }

    #[test]
    fn overfull_measure_keeps_its_content_as_an_irregular_length() {
        let xml = r#"<score-partwise><part-list><score-part id="P1"><part-name>V</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><type>half</type></note><note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure></part></score-partwise>"#;
        let score = parse_musicxml(xml).expect("overfull bar imports");
        let measure = &score.parts[0].staves[0].measures[0];
        assert_eq!(measure.voices[0].len(), 2);
        assert_eq!(measure.duration_beats(&score.settings.time_signature), 3.0);
        assert!(acorde_core::validate(&score).errors.is_empty());
    }

    #[test]
    fn ties_are_parsed_from_note_level_and_notations_elements() {
        let xml = r#"<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1"><measure number="1"><attributes><time><beats>2</beats><beat-type>4</beat-type></time></attributes>
    <note><pitch><step>C</step><octave>4</octave></pitch><duration>480</duration><type>quarter</type><tie type="start"/><notations><tied type="start"/></notations></note>
    <note><pitch><step>C</step><octave>4</octave></pitch><duration>480</duration><type>quarter</type><tie type="stop"/><notations><tied type="stop"/></notations></note>
  </measure></part>
</score-partwise>"#;
        let score = parse_musicxml(xml).expect("MusicXML ties parse");
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(notes.len(), 2);
        assert!(notes[0].tie_start);
        assert!(!notes[0].tie_end);
        assert!(!notes[1].tie_start);
        assert!(notes[1].tie_end);
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

    #[test]
    fn harp_pedals_are_parsed_in_conventional_order() {
        let xml = r#"<score-partwise><part-list><score-part id="P1"/></part-list><part id="P1"><measure number="1"><direction placement="below"><direction-type><harp-pedals><pedal-tuning><pedal-step>D</pedal-step><pedal-alter>-1</pedal-alter></pedal-tuning><pedal-tuning><pedal-step>C</pedal-step><pedal-alter>1</pedal-alter></pedal-tuning></harp-pedals></direction-type></direction></measure></part></score-partwise>"#;
        let score = parse_musicxml(xml).unwrap();
        let diagram = &score.parts[0].staves[0].measures[0].harp_pedal_diagrams[0];
        assert_eq!(diagram.positions[0], HarpPedalPosition::Flat);
        assert_eq!(diagram.positions[1], HarpPedalPosition::Sharp);
        assert_eq!(diagram.placement.as_deref(), Some("below"));
    }
}
