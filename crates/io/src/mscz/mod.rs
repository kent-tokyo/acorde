use crate::Error;
use acorde_core::{
    Barline, Clef, Duration, Dynamic, KeySignature, Lyric, Measure, Note, Part, Pitch, Score,
    Staff, Step, TimeSignature, VoltaBracket,
};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::HashMap;
use std::collections::HashSet;

const MAX_ELEMENTS: usize = 500_000;
const MAX_MSCZ_COMPRESSED: usize = 64 * 1024 * 1024;
const MAX_MSCZ_ENTRIES: usize = 1024;

struct PartMeta {
    name: String,
    midi_program: u8,
    midi_channel: u8,
    staff_ids: Vec<usize>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Parse a MuseScore .mscz ZIP file into a Score.
///
/// Supports MuseScore 3.x and 4.x formats. Notes, rests, key/time/clef signatures,
/// part names, MIDI program, tempo, repeat barlines, volta brackets, dynamics,
/// lyrics, and slur starts are extracted.
pub fn parse_mscz(data: &[u8]) -> Result<Score, Error> {
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
    let mscx = extract_mscx(&mut archive)?;
    parse_mscx(&mscx)
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
    let mut in_instrument = false;
    let mut in_channel = false;

    // Staff data: 1-based staff ID → measures
    let mut staff_measures: HashMap<usize, Vec<Measure>> = HashMap::new();
    let mut staff_clefs: HashMap<usize, Clef> = HashMap::new();

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

    // Feature M: MuseScore 4.x voice wrapper container
    let mut in_measure_voice_wrapper = false;
    let mut measure_voice_index: usize = 0;

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
    let mut chord_tie_start = false;
    let mut chord_slur_start = false; // Feature L

    // Note state (inside Chord)
    let mut in_note_elem = false;
    let mut note_midi: i32 = 60;
    let mut note_tpc: i32 = 14;

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
                    }
                    "Instrument" if in_part => {
                        in_instrument = true;
                    }
                    "Channel" if in_instrument => {
                        in_channel = true;
                    }
                    "Staff" if !in_part && current_staff_id.is_none() => {
                        let id = attr_usize(e, b"id").unwrap_or(1);
                        current_staff_id = Some(id);
                        staff_measures.entry(id).or_default();
                        cur_measure_num = 0;
                    }
                    "Measure" if current_staff_id.is_some() && !in_measure => {
                        in_measure = true;
                        cur_measure_num += 1;
                        cur_key = None;
                        cur_time = None;
                        cur_clef_in_measure = None;
                        cur_tempo = None;
                        cur_voices = [vec![], vec![], vec![], vec![]];
                        cur_barline_left = Barline::Normal;
                        cur_barline_right = Barline::Normal;
                        cur_volta = None;
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
                        chord_tie_start = false;
                        chord_slur_start = false;
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
                    }
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
                    "Part" if in_part => {
                        parts_meta.push(PartMeta {
                            name: cur_part_name.clone(),
                            midi_program: cur_part_program,
                            midi_channel: cur_part_channel,
                            staff_ids: cur_part_staff_ids.clone(),
                        });
                        in_part = false;
                    }

                    // Staff close
                    "Staff" if current_staff_id.is_some() && !in_measure => {
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
                            texts: Vec::new(),
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

                    // Note close
                    "Note" if in_note_elem => {
                        chord_pitches.push(tpc_midi_to_pitch(note_tpc, note_midi));
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
                        let v = chord_voice.min(3);
                        cur_voices[v].push(rest);
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
                        if let Some(id) = attr_usize(e, b"id") {
                            cur_part_staff_ids.push(id);
                        }
                    }
                    "program" if in_channel => {
                        if let Some(v) = attr_str(e, b"value") {
                            cur_part_program = v.parse().unwrap_or(0);
                        }
                    }
                    // Feature J: Repeat Start barline (empty element)
                    "startRepeat" if in_measure => {
                        cur_barline_left = Barline::RepeatStart;
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
    )
}

// ── Assembly ──────────────────────────────────────────────────────────────────

fn assemble_score(
    mut score: Score,
    title: &str,
    composer: &str,
    parts_meta: Vec<PartMeta>,
    mut staff_measures: HashMap<usize, Vec<Measure>>,
    staff_clefs: HashMap<usize, Clef>,
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
                        staff_clefs: &HashMap<usize, Clef>|
     -> Vec<Staff> {
        ids.iter()
            .map(|&sid| {
                let clef = staff_clefs.get(&sid).cloned().unwrap_or(Clef::Treble);
                let mut s = Staff::new(clef);
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
        part.staves = build_staves(&all_ids, &mut staff_measures, &staff_clefs);
        score.parts.push(part);
    } else {
        for meta in parts_meta {
            let mut part = Part::new(&meta.name, &meta.name);
            part.midi_program = meta.midi_program;
            part.midi_channel = meta.midi_channel;
            let ids = if meta.staff_ids.is_empty() {
                let mut remaining: Vec<usize> = staff_measures.keys().copied().collect();
                remaining.sort();
                remaining.into_iter().take(1).collect()
            } else {
                meta.staff_ids
            };
            part.staves = build_staves(&ids, &mut staff_measures, &staff_clefs);
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
