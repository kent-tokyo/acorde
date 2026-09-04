use crate::{Diagnostic, Error};
use acorde_core::{
    Articulation, Barline, ChordDegree, ChordSymbol, Clef, Duration, Dynamic, FiguredBassFigure,
    GuitarTechnique, KeySignature, Lyric, Measure, Note, Part, PartGroupSymbol, Pitch, Score,
    Staff, StaffGroup, Step, StyledText, TabPosition, TablatureConfig, TextStyle, TimeSignature,
    TupletInfo, VoltaBracket,
};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::HashMap;
use std::collections::HashSet;

const MAX_ELEMENTS: usize = 500_000;
const MAX_MSCZ_COMPRESSED: usize = 64 * 1024 * 1024;
const MAX_MSCZ_ENTRIES: usize = 1024;
const UNSUPPORTED_MSCX_ELEMENTS: &[&str] = &["Ottava", "Glissando"];
const MIN_HARMONY_TPC: i32 = 6;
const MAX_HARMONY_TPC: i32 = 26;

struct PartMeta {
    name: String,
    midi_program: u8,
    midi_channel: u8,
    staff_ids: Vec<usize>,
    staff_count: usize,
    staff_group_specs: Vec<StaffGroupSpec>,
}

struct StaffGroupSpec {
    symbol: PartGroupSymbol,
    span: usize,
    barlines_connect: bool,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Parse a MuseScore .mscz ZIP file into a Score.
///
/// Supports MuseScore 3.x and 4.x formats. Notes, rests, key/time/clef signatures,
/// part names, MIDI program, tempo, repeat barlines, volta brackets, dynamics,
/// lyrics, slur starts, tablature staff metadata, string/fret positions, fingering, and basic
/// guitar techniques are extracted.
pub fn parse_mscz(data: &[u8]) -> Result<Score, Error> {
    let mscx = extract_mscx_bytes(data)?;
    parse_mscx(&mscx)
}

fn extract_mscx_bytes(data: &[u8]) -> Result<String, Error> {
    if data.len() < 4 {
        return Err(Error::Zip("data too short to be a ZIP file".into()));
    }
    if data.len() > MAX_MSCZ_COMPRESSED {
        return Err(Error::TooLarge(data.len()));
    }
    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| Error::Zip(e.to_string()))?;
    if archive.len() > MAX_MSCZ_ENTRIES {
        return Err(Error::Zip("too many MSCZ archive entries".into()));
    }
    let mut entry_names = HashSet::new();
    let total_uncompressed = (0..archive.len()).try_fold(0_u64, |total, index| {
        let entry = archive
            .by_index(index)
            .map_err(|e| Error::Zip(format!("failed to inspect MSCZ entry: {e}")))?;
        let name = entry.name().to_string();
        validate_archive_entry_path(&name)?;
        if !entry_names.insert(name.clone()) {
            return Err(Error::Zip(format!("duplicate MSCZ entry: '{name}'")));
        }
        total
            .checked_add(entry.size())
            .ok_or(Error::TooLarge(usize::MAX))
    })?;
    if total_uncompressed > MAX_MSCX_SIZE {
        return Err(Error::TooLarge(total_uncompressed as usize));
    }
    extract_mscx(&mut archive)
}

/// Parse a MuseScore 3.x/.4.x .mscx XML string into a Score.
pub fn parse_mscx(xml: &str) -> Result<Score, Error> {
    if xml.len() > MAX_MSCX_SIZE as usize {
        return Err(Error::TooLarge(xml.len()));
    }
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut element_count = 0usize;
    let base_score = Score::default();

    // Part metadata
    let mut parts_meta: Vec<PartMeta> = Vec::new();
    let mut in_part = false;
    let mut cur_part_name = String::new();
    let mut cur_part_program: u8 = 0;
    let mut cur_part_channel: u8 = 0;
    let mut cur_part_staff_ids: Vec<usize> = Vec::new();
    let mut cur_part_staff_count = 0usize;
    let mut cur_part_staff_group_specs: Vec<StaffGroupSpec> = Vec::new();
    let mut in_bar_line_span = false;
    let mut in_instrument = false;
    let mut in_channel = false;

    // Staff data: 1-based staff ID → measures
    let mut staff_measures: HashMap<usize, Vec<Measure>> = HashMap::new();
    let mut staff_clefs: HashMap<usize, Clef> = HashMap::new();
    let mut staff_tablature: HashMap<usize, TablatureConfig> = HashMap::new();

    // Score metadata
    let mut score_title = String::new();
    let mut score_composer = String::new();
    let mut in_meta_tag = false;
    let mut meta_tag_name = String::new();

    // Parsing context flags (using depth markers for clear nesting)
    let mut current_staff_id: Option<usize> = None;
    let mut in_measure = false;
    let mut cur_measure_num = 0u32;

    // Per-measure state
    let mut cur_key: Option<KeySignature> = None;
    let mut cur_time: Option<TimeSignature> = None;
    let mut cur_clef_in_measure: Option<Clef> = None;
    let mut cur_tempo: Option<u16> = None;
    let mut cur_voices: [Vec<Note>; 4] = [vec![], vec![], vec![], vec![]];

    // Feature J: repeat barlines and volta
    let mut cur_barline_left = Barline::Normal;
    let mut cur_barline_right = Barline::Normal;
    let mut cur_volta: Option<VoltaBracket> = None;
    let mut cur_texts: Vec<StyledText> = Vec::new();
    let mut cur_figured_bass: Vec<FiguredBassFigure> = Vec::new();

    // Feature M: MuseScore 4.x voice wrapper container
    let mut in_measure_voice_wrapper = false;
    let mut measure_voice_index: usize = 0;

    // MuseScore tab staff metadata (3.x/4.x use StaffType/StringData).
    let mut in_staff_type = false;
    let mut staff_type_is_tab = false;
    let mut staff_tab_lines: u8 = 6;
    let mut staff_tuning: Vec<i16> = Vec::new();
    let mut in_string_data = false;

    // KeySig parsing
    let mut in_keysig = false;
    let mut keysig_accidental: i8 = 0;
    let mut keysig_mode = String::new();

    // TimeSig parsing
    let mut in_timesig = false;
    let mut timesig_n: u8 = 4;
    let mut timesig_d: u8 = 4;

    // Clef parsing
    let mut in_clef_elem = false;
    let mut clef_type_str = String::new();

    // Tempo parsing
    let mut in_tempo_elem = false;

    // Chord / Rest state
    let mut in_chord = false;
    let mut in_rest_elem = false;
    let mut chord_duration: Option<Duration> = None;
    let mut chord_dots: u8 = 0;
    let mut chord_voice: usize = 0;
    let mut chord_pitches: Vec<Pitch> = Vec::new();
    let mut chord_tab_positions: Vec<TabPosition> = Vec::new();
    let mut chord_tie_start = false;
    let mut chord_slur_start = false; // Feature L
    let mut chord_is_grace = false;
    let mut chord_grace_slash = false;
    let mut chord_arpeggiate: Option<bool> = None;
    let mut in_arpeggio = false;
    let mut chord_tremolo: Option<u8> = None;
    let mut in_tremolo = false;
    let mut in_tuplet = false;
    let mut tuplet_actual_notes: Option<u8> = None;
    let mut tuplet_normal_notes: Option<u8> = None;
    let mut current_tuplet: Option<TupletInfo> = None;
    let mut in_harmony = false;
    let mut harmony_name = String::new();
    let mut harmony_root: Option<i32> = None;
    let mut harmony_bass: Option<i32> = None;
    let mut harmony_placement: Option<String> = None;
    let mut harmony_function: Option<String> = None;
    let mut pending_chord_symbol: Option<ChordSymbol> = None;
    let mut in_text_element = false;
    let mut mscx_text_style = TextStyle::Generic;
    let mut mscx_text_value = String::new();
    let mut in_figured_bass = false;
    let mut in_figured_bass_item = false;
    let mut figured_bass_number = String::new();
    let mut figured_bass_prefix: Option<String> = None;
    let mut figured_bass_suffix: Option<String> = None;
    let mut figured_bass_extender = false;

    // Note state (inside Chord)
    let mut in_note_elem = false;
    let mut note_midi: i32 = 60;
    let mut note_tpc: i32 = 14;
    let mut note_microtone_cents: i16 = 0;
    let mut in_accidental = false;
    let mut note_tab_string: Option<u8> = None;
    let mut note_tab_fret: Option<u8> = None;
    let mut note_fingerings: Vec<u8> = Vec::new();
    let mut note_technique: Option<GuitarTechnique> = None;

    // Spanner/Tie state (Note level)
    let mut in_spanner = false;
    let mut spanner_is_tie = false;
    let mut spanner_has_next = false;

    // Feature L: Slur Spanner state (Chord level)
    let mut in_chord_slur_spanner = false;
    let mut chord_slur_has_next = false;

    // Feature J: Volta Spanner state (Measure level)
    let mut in_volta_spanner = false;
    let mut volta_text = String::new();
    let mut volta_has_next = false;

    // Feature K: Dynamic state
    let mut in_dynamic_elem = false;
    let mut pending_dynamic: Option<Dynamic> = None;

    // Feature K: Lyric state
    let mut in_lyrics_elem = false;
    let mut lyrics_text = String::new();
    let mut lyrics_syllabic = String::new();

    // Accumulated text for the current element
    let mut text = String::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Xml(e.to_string())),
            Ok(Event::DocType(_)) => {
                return Err(Error::Xml("DOCTYPE declarations are not allowed".into()));
            }

            // ── Start events ──────────────────────────────────────────────────
            Ok(Event::Start(ref e)) => {
                element_count += 1;
                if element_count > MAX_ELEMENTS {
                    return Err(Error::Xml("document too large".into()));
                }
                let name = local_name_str(e.local_name().as_ref());
                text.clear();

                match name.as_str() {
                    "metaTag" if !in_part && current_staff_id.is_none() => {
                        in_meta_tag = true;
                        meta_tag_name = attr_str(e, b"name").unwrap_or_default();
                    }
                    "Part" if !in_part && current_staff_id.is_none() => {
                        in_part = true;
                        cur_part_name.clear();
                        cur_part_program = 0;
                        cur_part_channel = 0;
                        cur_part_staff_ids.clear();
                        cur_part_staff_count = 0;
                        cur_part_staff_group_specs.clear();
                    }
                    "Instrument" if in_part => {
                        in_instrument = true;
                    }
                    "Channel" if in_instrument => {
                        in_channel = true;
                    }
                    "barLineSpan" if in_part => {
                        in_bar_line_span = true;
                    }
                    "Staff" if !in_part && current_staff_id.is_none() => {
                        let id = attr_usize(e, b"id").unwrap_or(1);
                        current_staff_id = Some(id);
                        staff_measures.entry(id).or_default();
                        cur_measure_num = 0;
                        in_staff_type = false;
                        staff_type_is_tab = false;
                        staff_tab_lines = 6;
                        staff_tuning.clear();
                    }
                    "Staff" if in_part => {
                        cur_part_staff_count += 1;
                        if let Some(id) = attr_usize(e, b"id") {
                            cur_part_staff_ids.push(id);
                        }
                    }
                    "StaffType" if current_staff_id.is_some() => {
                        in_staff_type = true;
                        staff_type_is_tab = attr_str(e, b"group").as_deref() == Some("tab");
                    }
                    "StringData" if in_staff_type => {
                        in_string_data = true;
                    }
                    "Measure" if current_staff_id.is_some() && !in_measure => {
                        in_measure = true;
                        pending_chord_symbol = None;
                        cur_measure_num += 1;
                        cur_key = None;
                        cur_time = None;
                        cur_clef_in_measure = None;
                        cur_tempo = None;
                        cur_voices = [vec![], vec![], vec![], vec![]];
                        cur_barline_left = Barline::Normal;
                        cur_barline_right = Barline::Normal;
                        cur_volta = None;
                        cur_texts.clear();
                        cur_figured_bass.clear();
                        measure_voice_index = 0;
                        in_measure_voice_wrapper = false;
                    }
                    "KeySig" if in_measure => {
                        in_keysig = true;
                        keysig_accidental = 0;
                        keysig_mode.clear();
                    }
                    "TimeSig" if in_measure => {
                        in_timesig = true;
                        timesig_n = 4;
                        timesig_d = 4;
                    }
                    "Clef" if in_measure => {
                        in_clef_elem = true;
                        clef_type_str.clear();
                    }
                    "Tempo" if in_measure => {
                        in_tempo_elem = true;
                    }
                    "Harmony" if in_measure && !in_chord && !in_rest_elem => {
                        in_harmony = true;
                        harmony_name.clear();
                        harmony_root = None;
                        harmony_bass = None;
                        harmony_placement = None;
                        harmony_function = None;
                    }
                    "Text" if in_measure && !in_chord && !in_rest_elem => {
                        in_text_element = true;
                        mscx_text_style = TextStyle::Generic;
                        mscx_text_value.clear();
                    }
                    "FiguredBass" if in_measure && !in_chord && !in_rest_elem => {
                        in_figured_bass = true;
                        cur_figured_bass.clear();
                    }
                    "FiguredBassItem" if in_figured_bass => {
                        in_figured_bass_item = true;
                        figured_bass_number.clear();
                        figured_bass_prefix = None;
                        figured_bass_suffix = None;
                        figured_bass_extender = false;
                    }
                    "Chord" if in_measure && !in_chord && !in_rest_elem => {
                        in_chord = true;
                        chord_duration = None;
                        chord_dots = 0;
                        chord_voice = if in_measure_voice_wrapper {
                            measure_voice_index
                        } else {
                            0
                        };
                        chord_pitches.clear();
                        chord_tab_positions.clear();
                        chord_tie_start = false;
                        chord_slur_start = false;
                        chord_is_grace = false;
                        chord_grace_slash = false;
                        chord_arpeggiate = None;
                        chord_tremolo = None;
                    }
                    "Tuplet" if in_measure && !in_chord && !in_rest_elem => {
                        in_tuplet = true;
                        tuplet_actual_notes = None;
                        tuplet_normal_notes = None;
                    }
                    "Rest" if in_measure && !in_chord && !in_rest_elem => {
                        in_rest_elem = true;
                        chord_duration = None;
                        chord_dots = 0;
                        chord_voice = if in_measure_voice_wrapper {
                            measure_voice_index
                        } else {
                            0
                        };
                    }
                    "Note" if in_chord && !in_note_elem => {
                        in_note_elem = true;
                        note_midi = 60;
                        note_tpc = 14;
                        note_microtone_cents = 0;
                        note_tab_string = None;
                        note_tab_fret = None;
                        note_fingerings.clear();
                        note_technique = None;
                    }
                    "acciaccatura" | "grace8" | "grace16" | "grace32" | "grace64"
                        if in_chord && !in_note_elem =>
                    {
                        chord_is_grace = true;
                        chord_grace_slash = true;
                    }
                    "appoggiatura" | "grace4" if in_chord && !in_note_elem => {
                        chord_is_grace = true;
                    }
                    "Arpeggio" if in_chord && !in_note_elem => {
                        in_arpeggio = true;
                        chord_arpeggiate = Some(true);
                    }
                    "Tremolo" if in_chord && !in_note_elem => {
                        in_tremolo = true;
                        chord_tremolo = Some(1);
                    }
                    "Accidental" if in_note_elem => {
                        in_accidental = true;
                    }
                    "Bend" if in_note_elem => note_technique = Some(GuitarTechnique::Bend),
                    "Slide" if in_note_elem => note_technique = Some(GuitarTechnique::Slide),
                    "HammerOn" if in_note_elem => note_technique = Some(GuitarTechnique::HammerOn),
                    "PullOff" if in_note_elem => note_technique = Some(GuitarTechnique::PullOff),
                    // Note-level Tie Spanner
                    "Spanner" if in_note_elem => {
                        in_spanner = true;
                        spanner_is_tie = attr_str(e, b"type").as_deref() == Some("Tie");
                        spanner_has_next = false;
                    }
                    "next" if in_spanner => {
                        spanner_has_next = true;
                    }
                    // Feature M: MuseScore 4.x voice wrapper container at Measure level
                    "voice" if in_measure && !in_chord && !in_rest_elem => {
                        in_measure_voice_wrapper = true;
                    }
                    // Feature J: Volta Spanner at Measure level
                    "Spanner" if in_measure && !in_chord && !in_rest_elem => {
                        if attr_str(e, b"type").as_deref() == Some("Volta") {
                            in_volta_spanner = true;
                            volta_text.clear();
                            volta_has_next = false;
                        }
                    }
                    "next" if in_volta_spanner => {
                        volta_has_next = true;
                    }
                    // Feature L: Slur Spanner at Chord level
                    "Spanner" if in_chord && !in_note_elem => {
                        if attr_str(e, b"type").as_deref() == Some("Slur") {
                            in_chord_slur_spanner = true;
                            chord_slur_has_next = false;
                        }
                    }
                    "next" if in_chord_slur_spanner => {
                        chord_slur_has_next = true;
                    }
                    // Feature K: Dynamic at Measure level
                    "Dynamic" if in_measure && !in_chord => {
                        in_dynamic_elem = true;
                    }
                    // Feature K: Lyrics inside Chord
                    "Lyrics" if in_chord => {
                        in_lyrics_elem = true;
                        lyrics_text.clear();
                        lyrics_syllabic.clear();
                    }
                    _ => {}
                }
            }

            // ── End events ────────────────────────────────────────────────────
            Ok(Event::End(ref e)) => {
                let name = local_name_str(e.local_name().as_ref());
                let t = std::mem::take(&mut text);
                let t = t.trim();

                match name.as_str() {
                    // Metadata
                    "metaTag" if in_meta_tag => {
                        match meta_tag_name.as_str() {
                            "workTitle" | "title" => score_title = t.to_string(),
                            "composer" => score_composer = t.to_string(),
                            _ => {}
                        }
                        in_meta_tag = false;
                    }

                    // Part
                    "trackName" if in_part => {
                        cur_part_name = t.to_string();
                    }
                    "Instrument" if in_part => {
                        in_instrument = false;
                    }
                    "Channel" if in_instrument => {
                        in_channel = false;
                    }
                    "barLineSpan" if in_bar_line_span => {
                        if let Some(spec) = cur_part_staff_group_specs.last_mut() {
                            spec.barlines_connect = matches!(t, "1" | "true" | "yes");
                        }
                        in_bar_line_span = false;
                    }
                    "Part" if in_part => {
                        parts_meta.push(PartMeta {
                            name: cur_part_name.clone(),
                            midi_program: cur_part_program,
                            midi_channel: cur_part_channel,
                            staff_ids: cur_part_staff_ids.clone(),
                            staff_count: cur_part_staff_count,
                            staff_group_specs: std::mem::take(&mut cur_part_staff_group_specs),
                        });
                        in_part = false;
                    }

                    // Staff close
                    "Staff" if current_staff_id.is_some() && !in_measure => {
                        let sid = current_staff_id.unwrap_or(1);
                        if staff_type_is_tab {
                            let lines = staff_tab_lines.max(1);
                            let mut tuning = std::mem::take(&mut staff_tuning);
                            tuning.truncate(lines as usize);
                            if tuning.len() < lines as usize {
                                tuning.resize(lines as usize, 0);
                            }
                            staff_tablature.insert(
                                sid,
                                TablatureConfig {
                                    lines,
                                    tuning_midi: tuning,
                                    capo: 0,
                                },
                            );
                        }
                        current_staff_id = None;
                    }

                    // Measure close
                    "Measure" if in_measure => {
                        let sid = current_staff_id.unwrap_or(1);
                        if let Some(clef) = &cur_clef_in_measure {
                            staff_clefs.entry(sid).or_insert_with(|| clef.clone());
                        }
                        let meas = Measure {
                            number: cur_measure_num,
                            time_sig: cur_time.clone(),
                            key_sig: cur_key.clone(),
                            clef: cur_clef_in_measure.clone(),
                            tempo: cur_tempo,
                            barline_left: cur_barline_left.clone(),
                            barline_right: cur_barline_right.clone(),
                            volta: cur_volta.clone(),
                            tempo_text: None,
                            rehearsal: None,
                            navigation: None,
                            expression_text: None,
                            texts: std::mem::take(&mut cur_texts),
                            figured_bass: std::mem::take(&mut cur_figured_bass),
                            multi_rest_count: None,
                            system_break: false,
                            page_break: false,
                            voices: [
                                std::mem::take(&mut cur_voices[0]),
                                std::mem::take(&mut cur_voices[1]),
                                std::mem::take(&mut cur_voices[2]),
                                std::mem::take(&mut cur_voices[3]),
                            ],
                        };
                        staff_measures.entry(sid).or_default().push(meas);
                        in_measure = false;
                    }

                    // KeySig
                    "accidental" if in_keysig => {
                        keysig_accidental = t.parse().unwrap_or(0);
                    }
                    "mode" if in_keysig => {
                        keysig_mode = t.to_string();
                    }
                    "KeySig" if in_keysig => {
                        let mode = if keysig_mode.is_empty() {
                            "major"
                        } else {
                            keysig_mode.as_str()
                        };
                        cur_key = Some(KeySignature {
                            fifths: keysig_accidental,
                            mode: mode.to_string(),
                        });
                        in_keysig = false;
                    }

                    // TimeSig
                    "sigN" if in_timesig => {
                        timesig_n = t.parse().unwrap_or(4);
                    }
                    "sigD" if in_timesig => {
                        timesig_d = t.parse().unwrap_or(4);
                    }
                    "TimeSig" if in_timesig => {
                        cur_time = Some(TimeSignature {
                            numerator: timesig_n,
                            denominator: timesig_d,
                        });
                        in_timesig = false;
                    }

                    // Clef
                    "concertClefType" | "clefType" if in_clef_elem => {
                        clef_type_str = t.to_string();
                    }
                    "Clef" if in_clef_elem => {
                        cur_clef_in_measure = Some(mscz_clef_type(&clef_type_str));
                        in_clef_elem = false;
                    }

                    // Tempo: MuseScore stores quarter-notes-per-second
                    "tempo" if in_tempo_elem => {
                        let qps: f64 = t.parse().unwrap_or(2.0);
                        cur_tempo = Some((qps * 60.0).round() as u16);
                    }
                    "Tempo" if in_tempo_elem => {
                        in_tempo_elem = false;
                    }

                    // MuseScore harmony labels remain available as display text while the
                    // bounded harmonyInfo fields are projected into canonical ChordSymbol data.
                    "name" if in_harmony => {
                        harmony_name = t.to_string();
                    }
                    "root" if in_harmony => {
                        harmony_root =
                            t.trim().parse::<i32>().ok().filter(|value| {
                                (MIN_HARMONY_TPC..=MAX_HARMONY_TPC).contains(value)
                            });
                    }
                    "base" if in_harmony => {
                        harmony_bass =
                            t.trim().parse::<i32>().ok().filter(|value| {
                                (MIN_HARMONY_TPC..=MAX_HARMONY_TPC).contains(value)
                            });
                    }
                    "placement" if in_harmony => {
                        harmony_placement = Some(t.trim().to_string());
                    }
                    "function" if in_harmony => {
                        let value = t.trim();
                        harmony_function = (!value.is_empty()).then(|| value.to_string());
                    }
                    "Harmony" if in_harmony => {
                        if let Some(root) = harmony_root {
                            pending_chord_symbol = Some(mscx_chord_symbol(
                                root,
                                &harmony_name,
                                harmony_bass,
                                harmony_placement.take(),
                                harmony_function.take(),
                            ));
                        }
                        if !harmony_name.trim().is_empty() {
                            cur_texts.push(StyledText {
                                style: TextStyle::ChordSymbol,
                                text: harmony_name.trim().to_string(),
                            });
                        }
                        in_harmony = false;
                    }
                    "style" if in_text_element => {
                        let style = t.to_ascii_lowercase();
                        mscx_text_style = if style.contains("chord") {
                            TextStyle::ChordSymbol
                        } else if style.contains("rehearsal") {
                            TextStyle::RehearsalMark
                        } else if style.contains("technique") {
                            TextStyle::Technique
                        } else if style.contains("expression") || style.contains("tempo") {
                            TextStyle::Expression
                        } else {
                            TextStyle::Generic
                        };
                    }
                    "text" if in_text_element => {
                        mscx_text_value = t.to_string();
                    }
                    "Text" if in_text_element => {
                        if !mscx_text_value.trim().is_empty() {
                            cur_texts.push(StyledText {
                                style: mscx_text_style,
                                text: mscx_text_value.trim().to_string(),
                            });
                        }
                        in_text_element = false;
                    }
                    "digit" if in_figured_bass_item => {
                        figured_bass_number = t.to_string();
                    }
                    "prefix" if in_figured_bass_item => {
                        figured_bass_prefix = mscx_figured_bass_modifier(t);
                    }
                    "suffix" if in_figured_bass_item => {
                        figured_bass_suffix = mscx_figured_bass_modifier(t);
                    }
                    "continuationLine" if in_figured_bass_item => {
                        figured_bass_extender = matches!(t.trim(), "1" | "true");
                    }
                    "FiguredBassItem" if in_figured_bass_item => {
                        if !figured_bass_number.is_empty() {
                            cur_figured_bass.push(FiguredBassFigure {
                                number: figured_bass_number.clone(),
                                alter: None,
                                prefix: figured_bass_prefix.clone(),
                                suffix: figured_bass_suffix.clone(),
                                extender: figured_bass_extender,
                            });
                        }
                        in_figured_bass_item = false;
                    }
                    "FiguredBass" if in_figured_bass => {
                        if !cur_figured_bass.is_empty() {
                            let display = cur_figured_bass
                                .iter()
                                .map(|figure| {
                                    format!(
                                        "{}{}{}",
                                        mscx_figured_bass_display_modifier(
                                            figure.prefix.as_deref()
                                        ),
                                        figure.number,
                                        mscx_figured_bass_display_modifier(
                                            figure.suffix.as_deref()
                                        )
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join(" ");
                            cur_texts.push(StyledText {
                                style: TextStyle::FiguredBass,
                                text: display,
                            });
                        }
                        in_figured_bass = false;
                    }

                    // Chord content
                    "durationType" if in_chord || in_rest_elem => {
                        chord_duration = Some(mscz_duration_type(t));
                    }
                    "dots" if in_chord || in_rest_elem => {
                        chord_dots = t.parse().unwrap_or(0);
                    }
                    // MuseScore 3.x: <voice> is a text element inside Chord/Rest
                    "voice" if (in_chord || in_rest_elem) && !in_note_elem => {
                        let v: usize = t.parse().unwrap_or(1);
                        chord_voice = v.saturating_sub(1).min(3);
                    }
                    // Feature M: MuseScore 4.x voice wrapper end
                    "voice" if in_measure_voice_wrapper && !in_chord && !in_rest_elem => {
                        in_measure_voice_wrapper = false;
                        measure_voice_index += 1;
                    }

                    // Note fields
                    "pitch" if in_note_elem => {
                        note_midi = t.parse().unwrap_or(60);
                    }
                    "tpc" if in_note_elem => {
                        note_tpc = t.parse().unwrap_or(14);
                    }
                    "subtype" if in_accidental => {
                        note_microtone_cents = match t {
                            "quarter-sharp" | "quartersharp" | "qs" => 50,
                            "quarter-flat" | "quarterflat" | "qf" => -50,
                            _ => 0,
                        };
                    }
                    "string" if in_note_elem => {
                        note_tab_string = t.parse::<u8>().ok();
                    }
                    "fret" if in_note_elem => {
                        note_tab_fret = t.parse::<u8>().ok();
                    }
                    "Fingering" if in_note_elem => {
                        if let Ok(fingering) = t.parse::<u8>() {
                            note_fingerings.push(fingering);
                        }
                    }
                    "lines" if in_staff_type && staff_type_is_tab => {
                        if let Ok(lines) = t.parse::<u8>() {
                            staff_tab_lines = lines;
                        }
                    }
                    "string" if in_string_data && in_staff_type && staff_type_is_tab => {
                        if let Ok(tuning) = t.parse::<i16>() {
                            staff_tuning.push(tuning);
                        }
                    }
                    "StringData" if in_string_data => {
                        in_string_data = false;
                    }
                    "StaffType" if in_staff_type => {
                        let sid = current_staff_id.unwrap_or(1);
                        if staff_type_is_tab {
                            let lines = staff_tab_lines.max(1);
                            let mut tuning = std::mem::take(&mut staff_tuning);
                            tuning.truncate(lines as usize);
                            if tuning.len() < lines as usize {
                                tuning.resize(lines as usize, 0);
                            }
                            staff_tablature.insert(
                                sid,
                                TablatureConfig {
                                    lines,
                                    tuning_midi: tuning,
                                    capo: 0,
                                },
                            );
                        }
                        in_staff_type = false;
                        staff_type_is_tab = false;
                    }
                    "Accidental" if in_accidental => {
                        in_accidental = false;
                    }

                    // Note-level Spanner/Tie
                    "next" if in_spanner => {}
                    "Spanner" if in_spanner => {
                        if spanner_is_tie && spanner_has_next {
                            chord_tie_start = true;
                        }
                        in_spanner = false;
                        spanner_is_tie = false;
                    }

                    // Feature L: Chord-level Slur Spanner
                    "Spanner" if in_chord_slur_spanner => {
                        if chord_slur_has_next {
                            chord_slur_start = true;
                        }
                        in_chord_slur_spanner = false;
                    }

                    // Feature J: Volta Spanner
                    "beginText" | "text" if in_volta_spanner => {
                        volta_text = t.to_string();
                    }
                    "Spanner" if in_volta_spanner => {
                        let number = parse_volta_number(&volta_text);
                        let kind = if volta_has_next { "begin" } else { "begin_end" };
                        cur_volta = Some(VoltaBracket {
                            number,
                            kind: kind.to_string(),
                        });
                        in_volta_spanner = false;
                    }

                    // Feature J: endRepeat barline
                    "endRepeat" if in_measure => {
                        cur_barline_right = Barline::RepeatEnd;
                    }

                    // Feature K: Dynamic
                    "subtype" if in_dynamic_elem => {
                        pending_dynamic = parse_dynamic_str(t);
                    }
                    "Dynamic" if in_dynamic_elem => {
                        in_dynamic_elem = false;
                    }

                    // Feature K: Lyrics
                    "text" if in_lyrics_elem => {
                        lyrics_text = t.to_string();
                    }
                    "syllabic" if in_lyrics_elem => {
                        lyrics_syllabic = t.to_string();
                    }
                    "Lyrics" if in_lyrics_elem => {
                        in_lyrics_elem = false;
                    }
                    "subtype" if in_arpeggio => {
                        chord_arpeggiate = Some(!matches!(
                            t.trim().to_ascii_lowercase().as_str(),
                            "down" | "downward"
                        ));
                    }
                    "Arpeggio" if in_arpeggio => {
                        in_arpeggio = false;
                    }
                    "subtype" if in_tremolo => {
                        chord_tremolo = Some(mscx_tremolo_level(t));
                    }
                    "Tremolo" if in_tremolo => {
                        in_tremolo = false;
                    }
                    "actualNotes" if in_tuplet => {
                        tuplet_actual_notes = t.parse::<u8>().ok();
                    }
                    "normalNotes" if in_tuplet => {
                        tuplet_normal_notes = t.parse::<u8>().ok();
                    }
                    "Tuplet" if in_tuplet => {
                        if let (Some(actual_notes), Some(normal_notes)) = (
                            tuplet_actual_notes.filter(|value| *value > 0),
                            tuplet_normal_notes.filter(|value| *value > 0),
                        ) {
                            current_tuplet = Some(TupletInfo {
                                actual_notes,
                                normal_notes,
                            });
                        }
                        in_tuplet = false;
                    }

                    // Note close
                    "Note" if in_note_elem => {
                        let pitch = tpc_midi_to_pitch(note_tpc, note_midi);
                        chord_pitches.push(Pitch::with_microtone(
                            pitch.step,
                            pitch.octave,
                            pitch.alter,
                            note_microtone_cents,
                        ));
                        if let (Some(string), Some(fret)) = (note_tab_string, note_tab_fret) {
                            chord_tab_positions.push(TabPosition { string, fret });
                        }
                        in_note_elem = false;
                    }

                    // Chord close
                    "Chord" if in_chord => {
                        let dur = chord_duration.clone().unwrap_or(Duration::Quarter);
                        if !chord_pitches.is_empty() {
                            let mut note = Note::new(chord_pitches[0].clone(), dur);
                            note.dot_count = chord_dots;
                            note.tie_start = chord_tie_start;
                            note.slur_start = chord_slur_start;
                            note.pitches = chord_pitches.clone();
                            note.tab_positions = chord_tab_positions.clone();
                            note.tab_position = note.tab_positions.first().cloned();
                            note.fingerings = note_fingerings.clone();
                            note.fingering = note.fingerings.first().copied();
                            note.guitar_technique = note_technique.clone();
                            note.tuplet = current_tuplet.clone();
                            note.is_grace = chord_is_grace;
                            note.grace_slash = chord_grace_slash;
                            note.arpeggiate = chord_arpeggiate;
                            note.chord_symbol = pending_chord_symbol.take();
                            if let Some(level) = chord_tremolo {
                                note.articulations.push(Articulation::Tremolo(level));
                            }
                            if let Some(dyn_val) = pending_dynamic.take() {
                                note.dynamic = Some(dyn_val);
                            }
                            if !lyrics_text.is_empty() {
                                note.lyric = Some(Lyric {
                                    text: lyrics_text.clone(),
                                    syllabic: if lyrics_syllabic.is_empty() {
                                        "single".to_string()
                                    } else {
                                        lyrics_syllabic.clone()
                                    },
                                });
                                lyrics_text.clear();
                            }
                            let v = chord_voice.min(3);
                            cur_voices[v].push(note);
                        }
                        in_chord = false;
                    }

                    // Rest close
                    "Rest" if in_rest_elem => {
                        let dur = chord_duration.clone().unwrap_or(Duration::Quarter);
                        let mut rest = Note::rest(dur);
                        rest.dot_count = chord_dots;
                        rest.tuplet = current_tuplet.clone();
                        let v = chord_voice.min(3);
                        cur_voices[v].push(rest);
                        pending_chord_symbol = None;
                        in_rest_elem = false;
                    }

                    _ => {}
                }
            }

            // ── Empty events ──────────────────────────────────────────────────
            Ok(Event::Empty(ref e)) => {
                element_count += 1;
                let name = local_name_str(e.local_name().as_ref());
                match name.as_str() {
                    "Staff" if in_part => {
                        cur_part_staff_count += 1;
                        if let Some(id) = attr_usize(e, b"id") {
                            cur_part_staff_ids.push(id);
                        }
                    }
                    "bracket" if in_part => {
                        let symbol = match attr_str(e, b"type").as_deref() {
                            Some("2") | Some("brace") => PartGroupSymbol::Brace,
                            Some("3") | Some("line") => PartGroupSymbol::Line,
                            _ => PartGroupSymbol::Bracket,
                        };
                        if let Some(span) = attr_usize(e, b"span") {
                            cur_part_staff_group_specs.push(StaffGroupSpec {
                                symbol,
                                span,
                                barlines_connect: false,
                            });
                        }
                    }
                    "program" if in_channel => {
                        if let Some(v) = attr_str(e, b"value") {
                            cur_part_program = v.parse().unwrap_or(0);
                        }
                    }
                    "Bend" if in_note_elem => {
                        note_technique = Some(GuitarTechnique::Bend);
                    }
                    "Slide" if in_note_elem => {
                        note_technique = Some(GuitarTechnique::Slide);
                    }
                    "HammerOn" if in_note_elem => {
                        note_technique = Some(GuitarTechnique::HammerOn);
                    }
                    "PullOff" if in_note_elem => {
                        note_technique = Some(GuitarTechnique::PullOff);
                    }
                    "acciaccatura" | "grace8" | "grace16" | "grace32" | "grace64"
                        if in_chord && !in_note_elem =>
                    {
                        chord_is_grace = true;
                        chord_grace_slash = true;
                    }
                    "appoggiatura" | "grace4" if in_chord && !in_note_elem => {
                        chord_is_grace = true;
                    }
                    "Arpeggio" if in_chord && !in_note_elem => {
                        chord_arpeggiate = Some(true);
                    }
                    "Tremolo" if in_chord && !in_note_elem => {
                        chord_tremolo = Some(1);
                    }
                    // Feature J: Repeat Start barline (empty element)
                    "startRepeat" if in_measure => {
                        cur_barline_left = Barline::RepeatStart;
                    }
                    "endTuplet" if in_measure => {
                        current_tuplet = None;
                    }
                    _ => {}
                }
            }

            // ── Text events ───────────────────────────────────────────────────
            Ok(Event::Text(ref e)) => {
                if let Ok(t) = e.decode()
                    && let Ok(t) = quick_xml::escape::unescape(&t)
                {
                    text.push_str(&t);
                }
            }

            _ => {}
        }
    }

    if element_count == 0 {
        return Err(Error::Xml("empty document".into()));
    }

    assemble_score(
        base_score,
        &score_title,
        &score_composer,
        parts_meta,
        staff_measures,
        staff_clefs,
        staff_tablature,
    )
}

/// Report known MSCX elements that are not represented by the canonical score model.
pub fn loss_diagnostics(xml: &str) -> Vec<Diagnostic> {
    let mut reader = Reader::from_str(xml);
    let mut path: Vec<String> = Vec::new();
    let mut diagnostics = Vec::new();
    let mut harmony_context: Option<(String, bool, bool, bool)> = None;
    let mut harmony_function_path: Option<Vec<String>> = None;
    let mut harmony_function_text = String::new();
    let mut figured_bass_context: Option<(String, Option<String>, bool)> = None;
    let mut declared_staff_paths: HashMap<String, String> = HashMap::new();
    let mut referenced_staff_ids = HashSet::new();
    let mut declared_staff_order = Vec::new();
    let mut anonymous_part_staff_count = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = local_name_str(event.local_name().as_ref());
                path.push(name.clone());
                if matches!(name.as_str(), "Harmony" | "harmonyInfo") {
                    push_harmony_attribute_diagnostics(&mut diagnostics, &path, &event, &name);
                }
                if name == "Staff" {
                    let location = format!("/{}", path.join("/"));
                    if path.iter().any(|element| element == "Part") {
                        if let Some(id) = attr_str(&event, b"id") {
                            referenced_staff_ids.insert(id);
                        } else {
                            anonymous_part_staff_count += 1;
                        }
                    } else {
                        if let Some(id) = attr_str(&event, b"id") {
                            declared_staff_order.push(id.clone());
                            declared_staff_paths.insert(id, location);
                        }
                    }
                }
                if name == "Harmony" {
                    harmony_context = Some((format!("/{}", path.join("/")), false, false, false));
                } else if let Some((_, has_name, has_root, has_unsupported_child)) =
                    harmony_context.as_mut()
                {
                    if name == "name" {
                        *has_name = true;
                    } else if name == "root" {
                        *has_root = true;
                    } else if !matches!(
                        name.as_str(),
                        "harmonyInfo" | "eid" | "base" | "placement" | "function"
                    ) {
                        *has_unsupported_child = true;
                    }
                    if name == "function" {
                        harmony_function_path = Some(path.clone());
                        harmony_function_text.clear();
                    }
                }
                if name == "FiguredBass" {
                    figured_bass_context = Some((format!("/{}", path.join("/")), None, false));
                } else if let Some((_, unsupported, _)) = figured_bass_context.as_mut()
                    && !matches!(
                        name.as_str(),
                        "FiguredBassItem" | "digit" | "prefix" | "suffix" | "continuationLine"
                    )
                {
                    *unsupported = Some(name.clone());
                }
                if UNSUPPORTED_MSCX_ELEMENTS.contains(&name.as_str()) {
                    let mut diagnostic = Diagnostic::warning(
                        format!("mscx.unsupported-element.{name}"),
                        format!("MSCX element '{name}' is outside acorde's supported subset"),
                    );
                    diagnostic.source_location = Some(format!("/{}", path.join("/")));
                    diagnostics.push(diagnostic);
                }
            }
            Ok(Event::Text(event)) if harmony_function_path.is_some() => {
                harmony_function_text.push_str(&String::from_utf8_lossy(event.as_ref()));
            }
            Ok(Event::Empty(event)) => {
                let name = local_name_str(event.local_name().as_ref());
                let mut element_path = path.clone();
                element_path.push(name.clone());
                if matches!(name.as_str(), "Harmony" | "harmonyInfo") {
                    push_harmony_attribute_diagnostics(
                        &mut diagnostics,
                        &element_path,
                        &event,
                        &name,
                    );
                }
                if name == "Staff" {
                    let location = format!("/{}", element_path.join("/"));
                    if path.iter().any(|element| element == "Part") {
                        if let Some(id) = attr_str(&event, b"id") {
                            referenced_staff_ids.insert(id);
                        } else {
                            anonymous_part_staff_count += 1;
                        }
                    } else {
                        if let Some(id) = attr_str(&event, b"id") {
                            declared_staff_order.push(id.clone());
                            declared_staff_paths.insert(id, location);
                        }
                    }
                }
                if name == "Harmony" {
                    let mut diagnostic = Diagnostic::warning(
                        "mscx.unsupported-element.Harmony",
                        "MSCX Harmony has no canonical display label",
                    );
                    let mut element_path = path.clone();
                    element_path.push(name.clone());
                    diagnostic.source_location = Some(format!("/{}", element_path.join("/")));
                    diagnostics.push(diagnostic);
                } else if name == "function" && harmony_context.is_some() {
                    let mut diagnostic = Diagnostic::warning(
                        "mscx.invalid-harmony-function",
                        "MSCX harmony function has no canonical token value",
                    );
                    diagnostic.source_location = Some(format!("/{}", element_path.join("/")));
                    diagnostics.push(diagnostic);
                } else if let Some((_, _, _, has_unsupported_child)) = harmony_context.as_mut() {
                    *has_unsupported_child = true;
                }
                if UNSUPPORTED_MSCX_ELEMENTS.contains(&name.as_str()) {
                    let mut element_path = path.clone();
                    element_path.push(name.clone());
                    let mut diagnostic = Diagnostic::warning(
                        format!("mscx.unsupported-element.{name}"),
                        format!("MSCX element '{name}' is outside acorde's supported subset"),
                    );
                    diagnostic.source_location = Some(format!("/{}", element_path.join("/")));
                    diagnostics.push(diagnostic);
                }
            }
            Ok(Event::End(event)) => {
                let name = local_name_str(event.local_name().as_ref());
                if name == "Harmony"
                    && let Some((harmony_path, has_name, has_root, has_unsupported_child)) =
                        harmony_context.take()
                    && ((!has_name && !has_root) || has_unsupported_child)
                {
                    let mut diagnostic = Diagnostic::warning(
                        "mscx.unsupported-element.Harmony",
                        "MSCX structured Harmony is outside acorde's display-label subset",
                    );
                    diagnostic.source_location = Some(harmony_path);
                    diagnostics.push(diagnostic);
                }
                if name == "function" {
                    if harmony_function_path.take().is_some()
                        && harmony_function_text.trim().is_empty()
                    {
                        let mut diagnostic = Diagnostic::warning(
                            "mscx.invalid-harmony-function",
                            "MSCX harmony function has no canonical token value",
                        );
                        diagnostic.source_location = Some(format!("/{}", path.join("/")));
                        diagnostics.push(diagnostic);
                    }
                    harmony_function_text.clear();
                }
                if name == "FiguredBass"
                    && let Some((figure_path, unsupported, has_digit)) = figured_bass_context.take()
                    && (unsupported.is_some() || !has_digit)
                {
                    let detail = unsupported.map_or_else(
                        || "contains no supported digit".to_string(),
                        |field| format!("contains unsupported child '{field}'"),
                    );
                    let mut diagnostic = Diagnostic::warning(
                        "mscx.unsupported-figured-bass-property",
                        format!("MSCX FiguredBass {detail}"),
                    );
                    diagnostic.source_location = Some(figure_path);
                    diagnostics.push(diagnostic);
                } else if name == "digit"
                    && let Some((_, _, has_digit)) = figured_bass_context.as_mut()
                {
                    *has_digit = true;
                }
                path.pop();
            }
            Ok(Event::Text(event)) => {
                let field = path.last().map(String::as_str);
                if field == Some("subtype")
                    && path.iter().rev().nth(1).map(String::as_str) == Some("Accidental")
                {
                    let value = String::from_utf8_lossy(event.as_ref());
                    if !matches!(
                        value.trim(),
                        "0" | "1"
                            | "none"
                            | "natural"
                            | "accidentalNatural"
                            | "sharp"
                            | "accidentalSharp"
                            | "flat"
                            | "accidentalFlat"
                            | "double-sharp"
                            | "doubleSharp"
                            | "accidentalDoubleSharp"
                            | "double-flat"
                            | "doubleFlat"
                            | "accidentalDoubleFlat"
                            | "quarter-sharp"
                            | "quartersharp"
                            | "qs"
                            | "quarter-flat"
                            | "quarterflat"
                            | "qf"
                    ) {
                        let mut diagnostic = Diagnostic::warning(
                            "mscx.unsupported-accidental-subtype",
                            "MSCX accidental subtype is outside acorde's supported microtonal subset",
                        );
                        diagnostic.source_location = Some(format!("/{}", path.join("/")));
                        diagnostic.preserved_value = Some(value.trim().to_string());
                        diagnostics.push(diagnostic);
                    }
                }
                if matches!(field, Some("root") | Some("base"))
                    && path.iter().any(|element| element == "Harmony")
                {
                    let value = String::from_utf8_lossy(event.as_ref());
                    let valid = value.trim().parse::<i32>().ok().is_some_and(|number| {
                        (MIN_HARMONY_TPC..=MAX_HARMONY_TPC).contains(&number)
                    });
                    if !valid {
                        let mut diagnostic = Diagnostic::warning(
                            "mscx.invalid-harmony-tpc",
                            format!("MSCX Harmony {field:?} is outside the bounded TPC range"),
                        );
                        diagnostic.source_location = Some(format!("/{}", path.join("/")));
                        diagnostic.preserved_value = Some(value.trim().to_string());
                        diagnostics.push(diagnostic);
                    }
                }
                if field == Some("digit")
                    && !event.as_ref().is_empty()
                    && let Some((_, _, has_digit)) = figured_bass_context.as_mut()
                {
                    *has_digit = true;
                }
                if path.iter().any(|element| element == "FiguredBassItem")
                    && matches!(field, Some("digit"))
                    && !event.as_ref().is_empty()
                {
                    let value = String::from_utf8_lossy(event.as_ref());
                    if !value
                        .trim()
                        .parse::<u8>()
                        .is_ok_and(|digit| (1..=9).contains(&digit))
                    {
                        let mut diagnostic = Diagnostic::warning(
                            "mscx.invalid-figured-bass-digit",
                            "MSCX FiguredBass digit is outside the supported 1..=9 range",
                        );
                        diagnostic.source_location = Some(format!("/{}", path.join("/")));
                        diagnostic.preserved_value = Some(value.trim().to_string());
                        diagnostics.push(diagnostic);
                    }
                }
                if path.iter().any(|element| element == "Note")
                    && matches!(field, Some("string") | Some("fret"))
                    && !event.as_ref().is_empty()
                {
                    let value = String::from_utf8_lossy(event.as_ref());
                    let valid = match field {
                        Some("string") => value.trim().parse::<u8>().is_ok_and(|string| string > 0),
                        Some("fret") => value.trim().parse::<u8>().is_ok(),
                        _ => true,
                    };
                    if !valid {
                        let mut diagnostic = Diagnostic::warning(
                            "mscx.invalid-tablature-position",
                            format!(
                                "MSCX Note {field:?} is outside the supported tablature value range"
                            ),
                        );
                        diagnostic.source_location = Some(format!("/{}", path.join("/")));
                        diagnostic.preserved_value = Some(value.trim().to_string());
                        diagnostics.push(diagnostic);
                    }
                }
                if path.iter().any(|element| element == "FiguredBassItem")
                    && matches!(field, Some("prefix") | Some("suffix"))
                    && !event.as_ref().is_empty()
                {
                    let value = String::from_utf8_lossy(event.as_ref());
                    if !value
                        .trim()
                        .parse::<u8>()
                        .is_ok_and(|modifier| modifier <= 8)
                    {
                        let mut diagnostic = Diagnostic::warning(
                            "mscx.invalid-figured-bass-modifier",
                            "MSCX FiguredBass prefix/suffix modifier is outside the supported range",
                        );
                        diagnostic.source_location = Some(format!("/{}", path.join("/")));
                        diagnostic.preserved_value = Some(value.trim().to_string());
                        diagnostics.push(diagnostic);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let anonymous_staff_ids: Vec<String> = declared_staff_order
        .into_iter()
        .filter(|id| !referenced_staff_ids.contains(id))
        .take(anonymous_part_staff_count)
        .collect();
    for id in anonymous_staff_ids {
        referenced_staff_ids.insert(id);
    }
    for (id, location) in declared_staff_paths {
        if !referenced_staff_ids.contains(&id) {
            let mut diagnostic = Diagnostic::warning(
                "mscx.unreferenced-staff",
                "MSCX Staff is not referenced by a Part and may not have canonical part ownership",
            );
            diagnostic.source_location = Some(location);
            diagnostic.preserved_value = Some(id);
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

/// Report known unsupported elements from an MSCZ archive after applying the same archive
/// validation and size limits as [`parse_mscz`].
pub fn loss_diagnostics_mscz(data: &[u8]) -> Result<Vec<Diagnostic>, Error> {
    Ok(loss_diagnostics(&extract_mscx_bytes(data)?))
}

/// Report parsed tablature positions that do not fit the owning staff's line count.
///
/// The XML loss scan handles malformed numeric values. This score-level pass covers values that
/// are syntactically numeric but cannot refer to a string on the parsed tablature staff.
pub fn tab_position_diagnostics(score: &acorde_core::Score) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            let Some(tab) = &staff.tablature else {
                continue;
            };
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    for (note_index, note) in voice.iter().enumerate() {
                        for (position_index, position) in note
                            .tab_position
                            .iter()
                            .chain(note.tab_positions.iter())
                            .enumerate()
                        {
                            if position.string != 0 && position.string <= tab.lines {
                                continue;
                            }
                            let mut diagnostic = Diagnostic::warning(
                                "mscx.invalid-tablature-string",
                                "MSCX note tablature string is outside the owning staff line range",
                            );
                            diagnostic.source_location = Some(format!(
                                "/score/part/{}/staff/{}/measure/{}/voice/{}/note/{}/tab-position/{}",
                                part_index + 1,
                                staff_index + 1,
                                measure_index + 1,
                                voice_index + 1,
                                note_index + 1,
                                position_index + 1
                            ));
                            diagnostic.preserved_value = Some(position.string.to_string());
                            diagnostics.push(diagnostic);
                        }
                    }
                }
            }
        }
    }
    diagnostics
}

// ── Assembly ──────────────────────────────────────────────────────────────────

fn assemble_score(
    mut score: Score,
    title: &str,
    composer: &str,
    parts_meta: Vec<PartMeta>,
    mut staff_measures: HashMap<usize, Vec<Measure>>,
    staff_clefs: HashMap<usize, Clef>,
    staff_tablature: HashMap<usize, TablatureConfig>,
) -> Result<Score, Error> {
    // Replace the default Score parts with the parsed content.
    score.parts.clear();
    if !title.is_empty() {
        score.metadata.title = title.to_string();
    }
    if !composer.is_empty() {
        score.metadata.composer = composer.to_string();
    }

    let build_staves = |ids: &[usize],
                        staff_measures: &mut HashMap<usize, Vec<Measure>>,
                        staff_clefs: &HashMap<usize, Clef>,
                        staff_tablature: &HashMap<usize, TablatureConfig>|
     -> Vec<Staff> {
        ids.iter()
            .map(|&sid| {
                let clef = staff_clefs.get(&sid).cloned().unwrap_or(Clef::Treble);
                let mut s = Staff::new(clef);
                s.tablature = staff_tablature.get(&sid).cloned();
                s.measures = staff_measures.remove(&sid).unwrap_or_default();
                for (i, m) in s.measures.iter_mut().enumerate() {
                    m.number = (i + 1) as u32;
                }
                s
            })
            .collect()
    };

    if parts_meta.is_empty() {
        let mut all_ids: Vec<usize> = staff_measures.keys().copied().collect();
        all_ids.sort();
        let mut part = Part::new("Part 1", "P1");
        part.staves = build_staves(
            &all_ids,
            &mut staff_measures,
            &staff_clefs,
            &staff_tablature,
        );
        score.parts.push(part);
    } else {
        for meta in parts_meta {
            let mut part = Part::new(&meta.name, &meta.name);
            part.midi_program = meta.midi_program;
            part.midi_channel = meta.midi_channel;
            let ids = if meta.staff_ids.is_empty() {
                let mut remaining: Vec<usize> = staff_measures.keys().copied().collect();
                remaining.sort();
                remaining
                    .into_iter()
                    .take(meta.staff_count.max(1))
                    .collect()
            } else {
                meta.staff_ids
            };
            part.staves = build_staves(&ids, &mut staff_measures, &staff_clefs, &staff_tablature);
            let mut group_start = 0usize;
            for spec in meta.staff_group_specs {
                if spec.span >= 2 && group_start.saturating_add(spec.span) <= part.staves.len() {
                    part.staff_groups.push(StaffGroup {
                        first_staff: group_start,
                        last_staff: group_start + spec.span - 1,
                        symbol: spec.symbol,
                        barlines_connect: spec.barlines_connect,
                    });
                    group_start += spec.span;
                }
            }
            score.parts.push(part);
        }
    }

    // Propagate first tempo/time/key to score settings
    if let Some(first_staff) = score.parts.first().and_then(|p| p.staves.first()) {
        let measures = &first_staff.measures;
        if let Some(bpm) = measures.iter().find_map(|m| m.tempo) {
            score.settings.tempo_bpm = bpm;
        }
        if let Some(ts) = measures.iter().find_map(|m| m.time_sig.clone()) {
            score.settings.time_signature = ts;
        }
        if let Some(ks) = measures.iter().find_map(|m| m.key_sig.clone()) {
            score.settings.key_signature = ks;
        }
    }

    Ok(score)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convert a MuseScore TPC value + MIDI number to a acorde Pitch.
///
/// TPC circle-of-fifths encoding:
/// - 6=Fb … 12=Bb (flats), 13=F … 19=B (naturals), 20=F# … 26=B# (sharps)
fn tpc_midi_to_pitch(tpc: i32, midi: i32) -> Pitch {
    const STEPS: [Step; 7] = [
        Step::F,
        Step::C,
        Step::G,
        Step::D,
        Step::A,
        Step::E,
        Step::B,
    ];
    let step_idx = (tpc - 13).rem_euclid(7) as usize;
    let step = STEPS[step_idx.min(6)].clone();
    let alter = (tpc - 13).div_euclid(7) as i8;
    let raw_oct = (midi / 12 - 1) as i8;
    for &oct in &[raw_oct, raw_oct - 1, raw_oct + 1] {
        let p = Pitch::with_alter(step.clone(), oct, alter);
        if p.to_midi() as i32 == midi {
            return p;
        }
    }
    Pitch::with_alter(step, raw_oct, alter)
}

/// Convert MuseScore's harmony root TPC and name subset to the canonical chord symbol.
///
/// MuseScore stores roots on the same circle-of-fifths TPC axis as notes.  The
/// canonical model intentionally keeps the kind vocabulary format-neutral; names
/// outside the known MusicXML-like subset are retained verbatim and remain eligible
/// for a source-located diagnostic in the report layer.
fn mscx_chord_symbol(
    root_tpc: i32,
    name: &str,
    bass_tpc: Option<i32>,
    placement: Option<String>,
    harmony_function: Option<String>,
) -> ChordSymbol {
    const STEPS: [&str; 7] = ["F", "C", "G", "D", "A", "E", "B"];
    let step_index = (root_tpc - 13).rem_euclid(7) as usize;
    let alteration = (root_tpc - 13).div_euclid(7);
    let mut root = STEPS[step_index].to_string();
    if alteration > 0 {
        root.push_str(&"#".repeat(alteration as usize));
    } else if alteration < 0 {
        root.push_str(&"b".repeat((-alteration) as usize));
    }
    let trimmed_name = name.trim();
    let (kind_name, degrees) = mscx_harmony_kind_and_degrees(trimmed_name);
    let kind = match kind_name {
        "" => "major",
        "m" => "minor",
        "7" => "dominant",
        "maj7" => "major-seventh",
        "maj" => "major",
        "min7" => "minor-seventh",
        "min" => "minor",
        "m7" => "minor-seventh",
        "dim" => "diminished",
        "dim7" => "diminished-seventh",
        "aug" => "augmented",
        "sus2" => "suspended-second",
        "sus4" => "suspended-fourth",
        "m7b5" => "half-diminished",
        "6" => "major-sixth",
        "m6" => "minor-sixth",
        other => other,
    };
    ChordSymbol {
        root,
        kind: kind.to_string(),
        bass: bass_tpc.map(mscx_tpc_to_note_name),
        placement,
        extender: false,
        harmonic_degree: None,
        harmony_function,
        harmony_type: None,
        chord_ref: None,
        degrees,
    }
}

/// Split the compact extension suffixes that MuseScore stores in Harmony/name.
///
/// The parser intentionally recognizes only unambiguous MusicXML-compatible
/// tokens. Unknown suffixes remain in the kind string instead of being guessed.
fn mscx_harmony_kind_and_degrees(name: &str) -> (&str, Vec<ChordDegree>) {
    let quality_len = [
        "maj7", "m7b5", "dim7", "sus2", "sus4", "min7", "m6", "maj", "min", "dim", "aug", "m", "7",
        "6",
    ]
    .iter()
    .find_map(|quality| name.strip_prefix(quality).map(|_| quality.len()))
    .unwrap_or(name.len());
    let (quality, suffix) = name.split_at(quality_len);
    if suffix.is_empty() {
        return (quality, Vec::new());
    }
    let mut degrees = Vec::new();
    let mut rest = suffix;
    while !rest.is_empty() {
        let (kind, tail) = if let Some(tail) = rest.strip_prefix("add") {
            ("add", tail)
        } else if let Some(tail) = rest.strip_prefix("no") {
            ("subtract", tail)
        } else {
            ("alter", rest)
        };
        let alter_len = tail
            .chars()
            .take_while(|ch| matches!(ch, '#' | 'b'))
            .count();
        let (accidentals, value_text) = tail.split_at(alter_len);
        let value_len = value_text.chars().take_while(char::is_ascii_digit).count();
        if value_len == 0 {
            return (name, Vec::new());
        }
        let value = match value_text[..value_len].parse::<u8>() {
            Ok(value) if value > 0 => value,
            _ => return (name, Vec::new()),
        };
        let alter = match accidentals {
            "" => 0,
            "#" => 1,
            "##" => 2,
            "b" => -1,
            "bb" => -2,
            _ => return (name, Vec::new()),
        };
        degrees.push(ChordDegree {
            value,
            alter,
            kind: kind.to_string(),
        });
        rest = &value_text[value_len..];
    }
    (quality, degrees)
}

fn mscx_tpc_to_note_name(tpc: i32) -> String {
    const STEPS: [&str; 7] = ["F", "C", "G", "D", "A", "E", "B"];
    let step_index = (tpc - 13).rem_euclid(7) as usize;
    let alteration = (tpc - 13).div_euclid(7);
    let mut note = STEPS[step_index].to_string();
    if alteration > 0 {
        note.push_str(&"#".repeat(alteration as usize));
    } else if alteration < 0 {
        note.push_str(&"b".repeat((-alteration) as usize));
    }
    note
}

fn mscx_figured_bass_modifier(value: &str) -> Option<String> {
    match value.parse::<u8>().ok()? {
        0 => None,
        1 => Some("double-flat".to_string()),
        2 => Some("flat".to_string()),
        3 => Some("natural".to_string()),
        4 => Some("sharp".to_string()),
        5 => Some("double-sharp".to_string()),
        6 => Some("cross".to_string()),
        7 => Some("backslash".to_string()),
        8 => Some("slash".to_string()),
        _ => None,
    }
}

fn mscx_figured_bass_display_modifier(value: Option<&str>) -> &str {
    match value {
        Some("double-flat") => "bb",
        Some("flat") => "b",
        Some("natural") => "♮",
        Some("sharp") => "#",
        Some("double-sharp") => "##",
        Some("cross") => "x",
        Some("backslash") => "\\",
        Some("slash") => "/",
        _ => "",
    }
}

fn mscz_duration_type(s: &str) -> Duration {
    match s {
        "whole" => Duration::Whole,
        "half" => Duration::Half,
        "quarter" => Duration::Quarter,
        "eighth" => Duration::Eighth,
        "16th" => Duration::Sixteenth,
        "32nd" => Duration::ThirtySecond,
        "measure" => Duration::Whole,
        _ => Duration::Quarter,
    }
}

fn mscz_clef_type(s: &str) -> Clef {
    match s {
        "G" | "G8vb" | "G15ma" | "G8va" => Clef::Treble,
        "F" | "F8vb" | "F15mb" | "F8va" => Clef::Bass,
        "C" => Clef::Alto,
        "TAB" | "TAB4" => Clef::Treble, // best approximation
        "PERC" | "PERC2" => Clef::Percussion,
        _ => Clef::Treble,
    }
}

fn parse_dynamic_str(s: &str) -> Option<Dynamic> {
    match s {
        "pppp" => Some(Dynamic::Pppp),
        "ppp" => Some(Dynamic::Ppp),
        "pp" => Some(Dynamic::Pp),
        "p" => Some(Dynamic::P),
        "mp" => Some(Dynamic::Mp),
        "mf" => Some(Dynamic::Mf),
        "f" => Some(Dynamic::F),
        "ff" => Some(Dynamic::Ff),
        "fff" => Some(Dynamic::Fff),
        "ffff" => Some(Dynamic::Ffff),
        "sfz" => Some(Dynamic::Sfz),
        "rfz" => Some(Dynamic::Rfz),
        "fz" => Some(Dynamic::Fz),
        "sf" => Some(Dynamic::Sf),
        _ => None,
    }
}

/// Convert MuseScore's tremolo subtype to the canonical number of beams.
/// `r`/`c` identify one-note/two-note tremolos; the current model stores the
/// speed but not that pairing distinction, so both prefixes intentionally map
/// to the same level.
fn mscx_tremolo_level(subtype: &str) -> u8 {
    let normalized = subtype.trim().to_ascii_lowercase();
    if normalized == "buzzroll" {
        return 0;
    }
    normalized
        .strip_prefix('r')
        .or_else(|| normalized.strip_prefix('c'))
        .and_then(|value| value.strip_prefix('8'))
        .map(|_| 1)
        .or_else(|| {
            normalized
                .strip_prefix('r')
                .or_else(|| normalized.strip_prefix('c'))
                .and_then(|value| value.strip_prefix("16"))
                .map(|_| 2)
        })
        .or_else(|| {
            normalized
                .strip_prefix('r')
                .or_else(|| normalized.strip_prefix('c'))
                .and_then(|value| value.strip_prefix("32"))
                .map(|_| 3)
        })
        .or_else(|| {
            normalized
                .strip_prefix('r')
                .or_else(|| normalized.strip_prefix('c'))
                .and_then(|value| value.strip_prefix("64"))
                .map(|_| 4)
        })
        .unwrap_or(1)
}

fn parse_volta_number(text: &str) -> u8 {
    text.chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(1)
}

fn local_name_str(bytes: &[u8]) -> String {
    std::str::from_utf8(bytes).unwrap_or("").to_string()
}

fn attr_str(e: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Option<String> {
    e.attributes()
        .filter_map(|a| a.ok())
        .find(|a| a.key.local_name().as_ref() == key)
        .and_then(|a| String::from_utf8(a.value.to_vec()).ok())
}

fn push_harmony_attribute_diagnostics(
    diagnostics: &mut Vec<Diagnostic>,
    path: &[String],
    event: &quick_xml::events::BytesStart<'_>,
    element: &str,
) {
    for attribute in event.attributes().flatten() {
        let key = local_name_str(attribute.key.local_name().as_ref());
        let value = String::from_utf8_lossy(attribute.value.as_ref()).into_owned();
        let mut diagnostic = Diagnostic::warning(
            format!("mscx.unsupported-attribute.{element}.{key}"),
            format!("MSCX {element} attribute '{key}' is outside the canonical harmony subset"),
        );
        diagnostic.source_location = Some(format!("/{}/@{key}", path.join("/")));
        diagnostic.preserved_value = Some(value);
        diagnostics.push(diagnostic);
    }
}

fn attr_usize(e: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Option<usize> {
    attr_str(e, key)?.parse().ok()
}

const MAX_MSCX_SIZE: u64 = 64 * 1024 * 1024; // 64 MiB

fn validate_archive_entry_path(path: &str) -> Result<(), Error> {
    let normalized = path.replace('\\', "/");
    if normalized.starts_with('/') || normalized.split('/').any(|part| part == "..") {
        return Err(Error::Zip(format!("invalid archive entry path: '{path}'")));
    }
    Ok(())
}

fn extract_mscx<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<String, Error> {
    use std::io::Read;
    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| Error::Zip(e.to_string()))?;
        let name = file.name().to_owned();
        validate_archive_entry_path(&name)?;
        if name.ends_with(".mscx") {
            if file.size() > MAX_MSCX_SIZE {
                return Err(Error::Zip(format!(
                    "mscx entry too large ({} bytes)",
                    file.size()
                )));
            }
            let mut content = String::new();
            file.take(MAX_MSCX_SIZE + 1)
                .read_to_string(&mut content)
                .map_err(|e| Error::Zip(e.to_string()))?;
            if content.len() as u64 > MAX_MSCX_SIZE {
                return Err(Error::TooLarge(content.len()));
            }
            return Ok(content);
        }
    }
    Err(Error::Zip("no .mscx file found in archive".into()))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_mscx(measures: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<museScore version="3.02">
  <Score>
    <Part>
      <Staff id="1"/>
      <trackName>Piano</trackName>
      <Instrument>
        <Channel name="normal">
          <program value="0"/>
        </Channel>
      </Instrument>
    </Part>
    <Staff id="1">
      {}
    </Staff>
  </Score>
</museScore>"#,
            measures
        )
    }

    fn zipped_mscx(xml: &str) -> Vec<u8> {
        use std::io::Write;
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        writer
            .start_file("score.mscx", zip::write::SimpleFileOptions::default())
            .expect("start MSCX entry");
        writer.write_all(xml.as_bytes()).expect("write MSCX entry");
        writer.finish().expect("finish MSCZ archive").into_inner()
    }

    #[test]
    fn parse_mscx_empty_returns_err() {
        assert!(parse_mscx("").is_err());
    }

    #[test]
    fn parse_mscz_empty_returns_err() {
        assert!(parse_mscz(&[]).is_err());
    }

    #[test]
    fn parse_mscz_garbage_returns_err() {
        assert!(parse_mscz(b"not a zip file!!").is_err());
    }

    #[test]
    fn parse_mscz_extracts_and_preserves_mscx_semantics() {
        let xml = simple_mscx(
            r#"<Measure number="1"><Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord></Measure>"#,
        );
        let score = parse_mscz(&zipped_mscx(&xml)).expect("MSCZ parses");
        let note = &score.parts[0].staves[0].measures[0].voices[0][0];
        assert_eq!(note.pitches[0].to_midi_cents(), 6000);
        assert_eq!(note.duration, Duration::Quarter);
    }

    #[test]
    fn parse_mscx_single_note_c4() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <KeySig><accidental>0</accidental></KeySig>
        <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>
        <Clef><concertClefType>G</concertClefType></Clef>
        <Chord>
          <durationType>quarter</durationType>
          <Note><pitch>60</pitch><tpc>14</tpc></Note>
        </Chord>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        assert_eq!(score.parts.len(), 1);
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(notes.len(), 1);
        assert!(!notes[0].is_rest);
        assert_eq!(notes[0].pitches[0].step, Step::C);
        assert_eq!(notes[0].pitches[0].alter, 0);
        assert_eq!(notes[0].duration, Duration::Quarter);
    }

    #[test]
    fn tpc_c_equals_step_c() {
        // tpc=14 → C
        let p = tpc_midi_to_pitch(14, 60);
        assert_eq!(p.step, Step::C);
        assert_eq!(p.alter, 0);
    }

    #[test]
    fn tpc_d_equals_step_d() {
        // tpc=16 → D, midi=62
        let p = tpc_midi_to_pitch(16, 62);
        assert_eq!(p.step, Step::D);
        assert_eq!(p.alter, 0);
    }

    #[test]
    fn tpc_fsharp_correct() {
        // tpc=20 → F#, midi=66
        let p = tpc_midi_to_pitch(20, 66);
        assert_eq!(p.step, Step::F);
        assert_eq!(p.alter, 1);
    }

    #[test]
    fn tpc_bflat_correct() {
        // tpc=12 → Bb, midi=70
        let p = tpc_midi_to_pitch(12, 70);
        assert_eq!(p.step, Step::B);
        assert_eq!(p.alter, -1);
    }

    #[test]
    fn parse_mscx_rest() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Rest><durationType>quarter</durationType></Rest>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(notes.len(), 1);
        assert!(notes[0].is_rest);
        assert_eq!(notes[0].duration, Duration::Quarter);
    }

    #[test]
    fn parse_mscx_dotted_note() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Chord>
          <durationType>quarter</durationType>
          <dots>1</dots>
          <Note><pitch>60</pitch><tpc>14</tpc></Note>
        </Chord>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(notes[0].dot_count, 1);
    }

    #[test]
    fn parse_mscx_key_signature() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <KeySig><accidental>2</accidental></KeySig>
        <Rest><durationType>whole</durationType></Rest>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        let ks = score.parts[0].staves[0].measures[0]
            .key_sig
            .as_ref()
            .unwrap();
        assert_eq!(ks.fifths, 2);
    }

    #[test]
    fn parse_mscx_time_signature() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <TimeSig><sigN>3</sigN><sigD>4</sigD></TimeSig>
        <Rest><durationType>whole</durationType></Rest>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        let ts = score.parts[0].staves[0].measures[0]
            .time_sig
            .as_ref()
            .unwrap();
        assert_eq!(ts.numerator, 3);
        assert_eq!(ts.denominator, 4);
    }

    #[test]
    fn parse_mscx_tablature_staff_and_note_position() {
        let xml = simple_mscx(
            r#"
      <StaffType group="tab">
        <lines>6</lines>
        <StringData>
          <string>40</string><string>45</string><string>50</string>
          <string>55</string><string>59</string><string>64</string>
        </StringData>
      </StaffType>
      <Measure number="1">
        <Chord>
          <durationType>quarter</durationType>
          <Note><pitch>64</pitch><tpc>14</tpc><string>1</string><fret>3</fret><Fingering>2</Fingering><Bend/></Note>
        </Chord>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        let staff = &score.parts[0].staves[0];
        assert_eq!(
            staff.tablature,
            Some(TablatureConfig {
                lines: 6,
                tuning_midi: vec![40, 45, 50, 55, 59, 64],
                capo: 0,
            })
        );
        let note = &staff.measures[0].voices[0][0];
        assert_eq!(note.tab_position, Some(TabPosition { string: 1, fret: 3 }));
        assert_eq!(note.tab_positions, vec![TabPosition { string: 1, fret: 3 }]);
        assert_eq!(note.fingering, Some(2));
        assert_eq!(note.guitar_technique, Some(GuitarTechnique::Bend));
    }

    #[test]
    fn parse_mscx_two_measures() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Chord>
          <durationType>quarter</durationType>
          <Note><pitch>60</pitch><tpc>14</tpc></Note>
        </Chord>
      </Measure>
      <Measure number="2">
        <Rest><durationType>quarter</durationType></Rest>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        assert_eq!(score.parts[0].staves[0].measures.len(), 2);
    }

    #[test]
    fn parse_mscx_voice_2() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Chord>
          <voice>2</voice>
          <durationType>quarter</durationType>
          <Note><pitch>64</pitch><tpc>18</tpc></Note>
        </Chord>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        let m = &score.parts[0].staves[0].measures[0];
        assert!(m.voices[0].is_empty());
        assert_eq!(m.voices[1].len(), 1);
    }

    #[test]
    fn parse_mscx_tempo() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Tempo>
          <tempo>2</tempo>
          <text>&#x266a; = 120</text>
        </Tempo>
        <Rest><durationType>whole</durationType></Rest>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        assert_eq!(score.settings.tempo_bpm, 120);
    }

    #[test]
    fn parse_mscx_chord_multiple_pitches() {
        // A Chord element with two Note children → one acorde Note with two pitches
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Chord>
          <durationType>quarter</durationType>
          <Note><pitch>60</pitch><tpc>14</tpc></Note>
          <Note><pitch>64</pitch><tpc>18</tpc></Note>
        </Chord>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].pitches.len(), 2);
    }

    #[test]
    fn parse_mscx_quarter_accidentals_preserves_microtone() {
        let xml = r#"
        <museScore version="4.0"><Score><Staff id="1"><Measure number="1">
          <Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc>
            <Accidental><subtype>quarter-sharp</subtype></Accidental></Note></Chord>
        </Measure></Staff></Score></museScore>
        "#;
        let score = parse_mscx(xml).expect("MSCX parses");
        assert_eq!(
            score.parts[0].staves[0].measures[0].voices[0][0].pitches[0].microtone_cents,
            50
        );
    }

    #[test]
    fn mscx_unknown_accidental_subtype_is_source_diagnosed() {
        let xml = r#"
        <Score><Staff id="1"><Measure number="1">
          <Chord><durationType>quarter</durationType>
            <Note><pitch>60</pitch><tpc>14</tpc>
              <Accidental><subtype>three-quarter-sharp</subtype></Accidental>
            </Note>
          </Chord>
        </Measure></Staff></Score>
        "#;
        let diagnostics = loss_diagnostics(xml);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "mscx.unsupported-accidental-subtype"
                && diagnostic.preserved_value.as_deref() == Some("three-quarter-sharp")
                && diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/Accidental/subtype"))
        }));
    }

    #[test]
    fn parse_mscx_tuplet_range_preserves_ratio() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Tuplet>
          <normalNotes>2</normalNotes>
          <actualNotes>3</actualNotes>
          <baseNote>eighth</baseNote>
        </Tuplet>
        <Chord><durationType>eighth</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
        <Chord><durationType>eighth</durationType><Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>
        <Chord><durationType>eighth</durationType><Note><pitch>64</pitch><tpc>18</tpc></Note></Chord>
        <endTuplet/>
      </Measure>"#,
        );
        let report = crate::parse_mscx_with_report(&xml).expect("MSCX tuplet parses");
        assert!(report.diagnostics.is_empty());
        let voice = &report.score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(voice.len(), 3);
        assert!(voice.iter().all(|note| {
            note.tuplet
                == Some(TupletInfo {
                    actual_notes: 3,
                    normal_notes: 2,
                })
        }));
    }

    #[test]
    fn parse_mscx_grace_note_preserves_slash() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Chord>
          <durationType>eighth</durationType>
          <acciaccatura/>
          <Note><pitch>60</pitch><tpc>14</tpc></Note>
        </Chord>
      </Measure>"#,
        );
        let report = crate::parse_mscx_with_report(&xml).expect("MSCX grace parses");
        assert!(report.diagnostics.is_empty());
        let note = &report.score.parts[0].staves[0].measures[0].voices[0][0];
        assert!(note.is_grace);
        assert!(note.grace_slash);
    }

    // ── Feature J: Repeat barlines + Volta ───────────────────────────────────

    #[test]
    fn parse_mscx_repeat_start() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <startRepeat/>
        <Rest><durationType>whole</durationType></Rest>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        let m = &score.parts[0].staves[0].measures[0];
        assert_eq!(m.barline_left, Barline::RepeatStart);
        assert_eq!(m.barline_right, Barline::Normal);
    }

    #[test]
    fn parse_mscx_repeat_end() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <endRepeat>2</endRepeat>
        <Rest><durationType>whole</durationType></Rest>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        let m = &score.parts[0].staves[0].measures[0];
        assert_eq!(m.barline_right, Barline::RepeatEnd);
        assert_eq!(m.barline_left, Barline::Normal);
    }

    #[test]
    fn parse_mscx_volta() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Spanner type="Volta">
          <Volta>
            <endHookType>1</endHookType>
            <beginText>1.</beginText>
          </Volta>
        </Spanner>
        <Rest><durationType>whole</durationType></Rest>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        let m = &score.parts[0].staves[0].measures[0];
        let volta = m.volta.as_ref().unwrap();
        assert_eq!(volta.number, 1);
        assert_eq!(volta.kind, "begin_end");
    }

    // ── Feature K: Dynamic + Lyric ────────────────────────────────────────────

    #[test]
    fn parse_mscx_dynamic() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Dynamic><subtype>p</subtype><velocity>49</velocity></Dynamic>
        <Chord>
          <durationType>quarter</durationType>
          <Note><pitch>60</pitch><tpc>14</tpc></Note>
        </Chord>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert_eq!(notes[0].dynamic, Some(Dynamic::P));
    }

    #[test]
    fn parse_mscx_lyric() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Chord>
          <durationType>quarter</durationType>
          <Lyrics>
            <text>hel</text>
            <syllabic>begin</syllabic>
          </Lyrics>
          <Note><pitch>60</pitch><tpc>14</tpc></Note>
        </Chord>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        let lyric = notes[0].lyric.as_ref().unwrap();
        assert_eq!(lyric.text, "hel");
        assert_eq!(lyric.syllabic, "begin");
    }

    #[test]
    fn parse_mscx_structured_figured_bass_items_in_order() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <FiguredBass>
          <FiguredBassItem><digit>6</digit></FiguredBassItem>
          <FiguredBassItem><digit>4</digit></FiguredBassItem>
        </FiguredBass>
        <Chord>
          <durationType>whole</durationType>
          <Note><pitch>60</pitch><tpc>14</tpc></Note>
        </Chord>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).expect("MSCX figured bass parses");
        let measure = &score.parts[0].staves[0].measures[0];
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
            vec![StyledText {
                style: TextStyle::FiguredBass,
                text: "6 4".to_string(),
            }]
        );
        assert!(loss_diagnostics(&xml).is_empty());
    }

    #[test]
    fn parse_mscx_figured_bass_modifiers_preserves_model_and_display() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <FiguredBass>
          <FiguredBassItem><prefix>4</prefix><digit>6</digit><suffix>2</suffix></FiguredBassItem>
        </FiguredBass>
        <Rest><durationType>whole</durationType></Rest>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).expect("MSCX figured bass parses");
        let measure = &score.parts[0].staves[0].measures[0];
        assert_eq!(
            measure.figured_bass,
            vec![FiguredBassFigure {
                number: "6".to_string(),
                alter: None,
                prefix: Some("sharp".to_string()),
                suffix: Some("flat".to_string()),
                extender: false,
            }]
        );
        assert_eq!(measure.texts[0].text, "#6b");
        assert!(loss_diagnostics(&xml).is_empty());
    }

    #[test]
    fn mscx_figured_bass_unmodeled_property_is_source_located() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <FiguredBass>
          <FiguredBassItem><digit>6</digit><unknownProperty>1</unknownProperty></FiguredBassItem>
        </FiguredBass>
        <Rest><durationType>whole</durationType></Rest>
      </Measure>"#,
        );
        let diagnostics = loss_diagnostics(&xml);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            "mscx.unsupported-figured-bass-property"
        );
        assert_eq!(
            diagnostics[0].source_location.as_deref(),
            Some("/museScore/Score/Staff/Measure/FiguredBass")
        );
    }

    #[test]
    fn mscx_figured_bass_invalid_value_is_source_located() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <FiguredBass>
          <FiguredBassItem><digit>0</digit><prefix>99</prefix></FiguredBassItem>
        </FiguredBass>
        <Rest><durationType>whole</durationType></Rest>
      </Measure>"#,
        );
        let diagnostics = loss_diagnostics(&xml);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code, "mscx.invalid-figured-bass-digit");
        assert_eq!(diagnostics[1].code, "mscx.invalid-figured-bass-modifier");
        assert_eq!(
            diagnostics[0].source_location.as_deref(),
            Some("/museScore/Score/Staff/Measure/FiguredBass/FiguredBassItem/digit")
        );
    }

    #[test]
    fn mscx_figured_bass_continuation_line_preserves_extender() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <FiguredBass>
          <FiguredBassItem><digit>6</digit><continuationLine>1</continuationLine></FiguredBassItem>
        </FiguredBass>
        <Rest><durationType>whole</durationType></Rest>
      </Measure>"#,
        );
        let report = crate::parse_mscx_with_report(&xml).expect("MSCX continuation line parses");
        assert!(report.diagnostics.is_empty());
        assert_eq!(
            report.score.parts[0].staves[0].measures[0].figured_bass[0].extender,
            true
        );
    }

    // ── Feature L: Slur ──────────────────────────────────────────────────────

    #[test]
    fn parse_mscx_slur_start() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Chord>
          <durationType>quarter</durationType>
          <Spanner type="Slur">
            <Slur/>
            <next><location><measures>0</measures></location></next>
          </Spanner>
          <Note><pitch>60</pitch><tpc>14</tpc></Note>
        </Chord>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        let notes = &score.parts[0].staves[0].measures[0].voices[0];
        assert!(notes[0].slur_start);
    }

    #[test]
    fn mscx_report_marks_unsupported_notation() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Harmony/>
        <Rest><durationType>whole</durationType></Rest>
      </Measure>"#,
        );
        let report = crate::parse_mscx_with_report(&xml).expect("MSCX report parses");
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(
            report.diagnostics[0].code,
            "mscx.unsupported-element.Harmony"
        );
        assert_eq!(
            report.diagnostics[0].source_location.as_deref(),
            Some("/museScore/Score/Staff/Measure/Harmony")
        );
    }

    #[test]
    fn mscx_harmony_attributes_are_source_located() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Harmony class="roman"><name>C</name></Harmony>
        <Rest><durationType>whole</durationType></Rest>
      </Measure>"#,
        );
        let diagnostics = loss_diagnostics(&xml);
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "mscx.unsupported-attribute.Harmony.class")
            .expect("Harmony attribute is diagnosed");
        assert_eq!(
            diagnostic.source_location.as_deref(),
            Some("/museScore/Score/Staff/Measure/Harmony/@class")
        );
        assert_eq!(diagnostic.preserved_value.as_deref(), Some("roman"));
    }

    #[test]
    fn mscx_invalid_tablature_values_are_source_located() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Chord><Note><pitch>64</pitch><string>zero</string><fret>999</fret></Note></Chord>
      </Measure>"#,
        );
        let diagnostics = loss_diagnostics(&xml);
        let position_diagnostics = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "mscx.invalid-tablature-position")
            .collect::<Vec<_>>();
        assert_eq!(position_diagnostics.len(), 2);
        assert!(position_diagnostics.iter().all(|diagnostic| {
            diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/Note/string") || path.ends_with("/Note/fret"))
                && diagnostic.preserved_value.is_some()
        }));
    }

    #[test]
    fn mscx_numeric_tablature_string_outside_staff_lines_is_source_located() {
        let xml = simple_mscx(
            r#"
      <StaffType group="tab"><lines>6</lines><StringData>
        <string>40</string><string>45</string><string>50</string><string>55</string><string>59</string><string>64</string>
      </StringData></StaffType>
      <Measure number="1">
        <Chord><Note><pitch>64</pitch><string>7</string><fret>3</fret></Note></Chord>
      </Measure>"#,
        );
        let report = crate::parse_mscx_with_report(&xml).expect("MSCX tab report parses");
        let diagnostic = report
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "mscx.invalid-tablature-string")
            .expect("out-of-range numeric string is diagnosed");
        assert!(
            diagnostic
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/tab-position/1"))
        );
        assert_eq!(diagnostic.preserved_value.as_deref(), Some("7"));
    }

    #[test]
    fn mscx_preserves_multiple_fingering_candidates() {
        let xml = r#"<museScore><Score><Staff><Measure><Chord><Note><pitch>64</pitch><Fingering>1</Fingering><Fingering>2</Fingering></Note></Chord></Measure></Staff></Score></museScore>"#;
        let score = parse_mscx(xml).expect("MSCX multiple fingering parses");
        let note = &score.parts[0].staves[0].measures[0].voices[0][0];
        assert_eq!(note.fingering, Some(1));
        assert_eq!(note.fingerings, vec![1, 2]);
        assert!(
            loss_diagnostics(xml).iter().all(|diagnostic| {
                diagnostic.code != "mscx.unsupported-detail.multiple-fingering"
            })
        );
    }

    #[test]
    fn mscx_report_marks_staff_without_part_ownership() {
        let xml = r#"<museScore><Score><Part><Staff id="1"/></Part><Staff id="1"/><Staff id="2"/></Score></museScore>"#;
        let diagnostics = loss_diagnostics(xml);
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "mscx.unreferenced-staff")
            .expect("orphan staff loss is reported");
        assert_eq!(
            diagnostic.source_location.as_deref(),
            Some("/museScore/Score/Staff")
        );
        assert_eq!(diagnostic.preserved_value.as_deref(), Some("2"));
    }

    #[test]
    fn mscx_part_bracket_maps_to_staff_group() {
        let xml = r#"<museScore><Score><Part><Staff id="1"/><Staff id="2"/><bracket type="2" span="2"/><barLineSpan>1</barLineSpan></Part><Staff id="1"><Measure><Rest><durationType>whole</durationType></Rest></Measure></Staff><Staff id="2"><Measure><Rest><durationType>whole</durationType></Rest></Measure></Staff></Score></museScore>"#;
        let score = parse_mscx(xml).expect("MSCX staff bracket parses");
        assert_eq!(score.parts.len(), 1);
        assert_eq!(score.parts[0].staves.len(), 2);
        assert_eq!(score.parts[0].staff_groups.len(), 1);
        let group = &score.parts[0].staff_groups[0];
        assert_eq!((group.first_staff, group.last_staff), (0, 1));
        assert_eq!(group.symbol, PartGroupSymbol::Brace);
        assert!(group.barlines_connect);
    }

    #[test]
    fn parse_mscx_harmony_name_preserves_display_label() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Harmony><name>Cmaj7</name></Harmony>
        <Rest><durationType>whole</durationType></Rest>
      </Measure>"#,
        );
        let report = crate::parse_mscx_with_report(&xml).expect("MSCX harmony parses");
        assert!(report.diagnostics.is_empty());
        let measure = &report.score.parts[0].staves[0].measures[0];
        assert_eq!(measure.texts.len(), 1);
        assert_eq!(measure.texts[0].style, TextStyle::ChordSymbol);
        assert_eq!(measure.texts[0].text, "Cmaj7");
    }

    #[test]
    fn mscx_harmony_common_min_quality_is_structured() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Harmony><harmonyInfo><name>min7</name><root>14</root></harmonyInfo></Harmony>
        <Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
      </Measure>"#,
        );
        let report = crate::parse_mscx_with_report(&xml).expect("MSCX min7 harmony parses");
        assert!(report.diagnostics.is_empty());
        let chord = report.score.parts[0].staves[0].measures[0].voices[0][0]
            .chord_symbol
            .as_ref()
            .expect("harmony attaches to note");
        assert_eq!(chord.root, "C");
        assert_eq!(chord.kind, "minor-seventh");
    }

    #[test]
    fn parse_mscx_structured_harmony_root_attaches_to_following_chord() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Harmony><harmonyInfo><name>7add#9b5no3</name><root>17</root><function>D</function><placement>below</placement></harmonyInfo></Harmony>
        <Chord><durationType>whole</durationType><Note><pitch>57</pitch><tpc>17</tpc></Note></Chord>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).expect("MSCX structured harmony parses");
        let note = &score.parts[0].staves[0].measures[0].voices[0][0];
        assert_eq!(
            note.chord_symbol,
            Some(ChordSymbol {
                root: "A".to_string(),
                kind: "dominant".to_string(),
                bass: None,
                placement: Some("below".to_string()),
                extender: false,
                harmonic_degree: None,
                harmony_function: Some("D".to_string()),
                harmony_type: None,
                chord_ref: None,
                degrees: vec![
                    ChordDegree {
                        value: 9,
                        alter: 1,
                        kind: "add".to_string(),
                    },
                    ChordDegree {
                        value: 5,
                        alter: -1,
                        kind: "alter".to_string(),
                    },
                    ChordDegree {
                        value: 3,
                        alter: 0,
                        kind: "subtract".to_string(),
                    },
                ],
            })
        );
    }

    #[test]
    fn empty_mscx_harmony_function_is_source_located() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Harmony><harmonyInfo><name>7</name><root>14</root><function></function></harmonyInfo></Harmony>
        <Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
      </Measure>"#,
        );
        let report = crate::parse_mscx_with_report(&xml).expect("empty function parses");
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "mscx.invalid-harmony-function"
                && diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/function"))
        }));
        assert_eq!(
            report.score.parts[0].staves[0].measures[0].voices[0][0]
                .chord_symbol
                .as_ref()
                .and_then(|chord| chord.harmony_function.as_deref()),
            None
        );
    }

    #[test]
    fn self_closing_mscx_harmony_function_is_source_located() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Harmony><harmonyInfo><name>7</name><root>14</root><function/></harmonyInfo></Harmony>
        <Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
      </Measure>"#,
        );
        let report = crate::parse_mscx_with_report(&xml).expect("self-closing function parses");
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "mscx.invalid-harmony-function"
                && diagnostic
                    .source_location
                    .as_deref()
                    .is_some_and(|path| path.ends_with("/function"))
        }));
    }

    #[test]
    fn parse_mscx_harmony_base_preserves_slash_bass() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Harmony><harmonyInfo><name>7</name><root>14</root><base>19</base></harmonyInfo></Harmony>
        <Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).expect("MSCX slash harmony parses");
        assert_eq!(
            score.parts[0].staves[0].measures[0].voices[0][0]
                .chord_symbol
                .as_ref()
                .and_then(|chord| chord.bass.as_deref()),
            Some("B")
        );
    }

    #[test]
    fn parse_mscx_harmony_rejects_unbounded_tpc_without_allocating() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Harmony><harmonyInfo><name>7</name><root>2147483647</root></harmonyInfo></Harmony>
        <Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).expect("bounded invalid Harmony parses as a loss");
        assert!(
            score.parts[0].staves[0].measures[0].voices[0][0]
                .chord_symbol
                .is_none()
        );
        let diagnostic = loss_diagnostics(&xml)
            .into_iter()
            .find(|diagnostic| diagnostic.code == "mscx.invalid-harmony-tpc")
            .expect("invalid Harmony TPC is source-located");
        assert_eq!(diagnostic.preserved_value.as_deref(), Some("2147483647"));
    }

    #[test]
    fn parse_mscx_text_preserves_typed_display_text() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Text><style>Expression</style><text>dolce</text></Text>
        <Rest><durationType>whole</durationType></Rest>
      </Measure>"#,
        );
        let report = crate::parse_mscx_with_report(&xml).expect("MSCX text parses");
        assert!(report.diagnostics.is_empty());
        assert_eq!(
            report.score.parts[0].staves[0].measures[0].texts,
            vec![acorde_core::StyledText {
                style: TextStyle::Expression,
                text: "dolce".to_string(),
            }]
        );
    }

    #[test]
    fn parse_mscx_arpeggio_preserves_direction() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Chord>
          <Arpeggio><subtype>Down</subtype></Arpeggio>
          <durationType>quarter</durationType>
          <Note><pitch>60</pitch><tpc>14</tpc></Note>
          <Note><pitch>64</pitch><tpc>18</tpc></Note>
        </Chord>
      </Measure>"#,
        );
        let report = crate::parse_mscx_with_report(&xml).expect("MSCX arpeggio parses");
        assert!(report.diagnostics.is_empty());
        let note = &report.score.parts[0].staves[0].measures[0].voices[0][0];
        assert_eq!(note.arpeggiate, Some(false));
    }

    #[test]
    fn parse_mscx_tremolo_preserves_speed_level() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <Chord>
          <durationType>quarter</durationType>
          <Note><pitch>60</pitch><tpc>14</tpc></Note>
          <Tremolo><subtype>c32</subtype></Tremolo>
        </Chord>
      </Measure>"#,
        );
        let report = crate::parse_mscx_with_report(&xml).expect("MSCX tremolo parses");
        assert!(report.diagnostics.is_empty());
        let note = &report.score.parts[0].staves[0].measures[0].voices[0][0];
        assert!(note.articulations.contains(&Articulation::Tremolo(3)));
    }

    // ── Feature M: MuseScore 4.x voice wrapper ────────────────────────────────

    #[test]
    fn parse_mscx_4x_voice_wrapper() {
        let xml = simple_mscx(
            r#"
      <Measure number="1">
        <voice>
          <Chord>
            <durationType>quarter</durationType>
            <Note><pitch>60</pitch><tpc>14</tpc></Note>
          </Chord>
        </voice>
        <voice>
          <Chord>
            <durationType>quarter</durationType>
            <Note><pitch>64</pitch><tpc>18</tpc></Note>
          </Chord>
        </voice>
      </Measure>"#,
        );
        let score = parse_mscx(&xml).unwrap();
        let m = &score.parts[0].staves[0].measures[0];
        assert_eq!(m.voices[0].len(), 1, "voice 0 should have 1 note");
        assert_eq!(m.voices[1].len(), 1, "voice 1 should have 1 note");
        assert_eq!(m.voices[0][0].pitches[0].step, Step::C);
        assert_eq!(m.voices[1][0].pitches[0].step, Step::E);
    }

    #[test]
    fn archive_paths_reject_traversal_and_backslashes() {
        assert!(validate_archive_entry_path("../score.mscx").is_err());
        assert!(validate_archive_entry_path(r"folder\..\score.mscx").is_err());
        assert!(validate_archive_entry_path("/absolute/score.mscx").is_err());
        assert!(validate_archive_entry_path("score.mscx").is_ok());
    }
}
