//! Guitar Pro 6/7/8 (`.gpx`, `.gp`) import.
//!
//! A `.gp` file (Guitar Pro 7/8) is a ZIP archive whose `Content/score.gpif` holds the score as
//! GPIF XML; a `.gpx` file (Guitar Pro 6) stores the same `score.gpif` in a "BCFZ"-compressed
//! sector file system. GPIF is
//! referential: master bars list one bar per staff, bars list voices, voices list beats, beats
//! reference a rhythm and notes, all by id. This importer resolves those references into the
//! canonical [`Score`]: one part per track, one staff per GPIF staff, tablature tuning and capo,
//! string/fret positions, and the guitar techniques acorde models. Everything else it meets is
//! reported as a typed diagnostic rather than dropped silently.

use crate::{Diagnostic, Error, ImportReport, REPORT_SCHEMA_VERSION};
use acorde_core::{
    Articulation, Barline, BeamState, Clef, Duration, Dynamic, GuitarBendPoint, GuitarTechnique,
    KeySignature, Lyric, Measure, MeasureLength, Note, NoteHead, Part, Pitch, Score, Staff,
    StaffKind, Step, StyledText, TabPosition, TablatureConfig, TextStyle, TimeSignature,
    TupletInfo, VoltaBracket, compute_beams,
};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Read};
use zip::ZipArchive;

const MAX_GP_COMPRESSED: usize = 64 * 1024 * 1024;
const MAX_GP_ENTRIES: usize = 4096;
const MAX_GPIF_BYTES: u64 = 64 * 1024 * 1024;
const MAX_GPIF_ELEMENTS: usize = 4_000_000;
const MAX_GPIF_DEPTH: usize = 128;
const MAX_GP_MEASURES: usize = 10_000;
const GPIF_ENTRY: &str = "Content/score.gpif";

/// Parse a Guitar Pro 6 (`.gpx`) or 7/8 (`.gp`) file; the container is detected from its bytes.
pub fn parse_gp(data: &[u8]) -> Result<Score, Error> {
    Ok(parse_gp_with_report(data)?.score)
}

/// Parse a Guitar Pro 6/7/8 file, reporting GPIF content outside acorde's model.
pub fn parse_gp_with_report(data: &[u8]) -> Result<ImportReport, Error> {
    let xml = if data.starts_with(b"BCFZ") || data.starts_with(b"BCFS") {
        gpx::read_gpif(data)?
    } else {
        read_gpif(data)?
    };
    let (score, diagnostics) = parse_gpif(&xml)?;
    Ok(ImportReport {
        schema_version: REPORT_SCHEMA_VERSION,
        format: "gp".to_string(),
        score,
        diagnostics,
    })
}

/// Extract `Content/score.gpif` from a `.gp` archive, with size and entry-count limits.
fn read_gpif(data: &[u8]) -> Result<String, Error> {
    if data.len() > MAX_GP_COMPRESSED {
        return Err(Error::TooLarge(data.len()));
    }
    let mut archive = ZipArchive::new(Cursor::new(data))
        .map_err(|error| Error::Zip(format!("invalid Guitar Pro archive: {error}")))?;
    if archive.len() > MAX_GP_ENTRIES {
        return Err(Error::Zip("too many Guitar Pro archive entries".into()));
    }
    let entry = archive.by_name(GPIF_ENTRY).map_err(|_| {
        Error::Zip(format!(
            "'{GPIF_ENTRY}' not found; only Guitar Pro 6/7/8 (.gpx/.gp) files are supported"
        ))
    })?;
    if entry.size() > MAX_GPIF_BYTES {
        return Err(Error::TooLarge(entry.size() as usize));
    }
    let mut bytes = Vec::new();
    entry
        .take(MAX_GPIF_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| Error::Zip(format!("failed to read '{GPIF_ENTRY}': {error}")))?;
    if bytes.len() as u64 > MAX_GPIF_BYTES {
        return Err(Error::TooLarge(bytes.len()));
    }
    crate::decode_xml_text(&bytes)
}

mod gpx;

// ── Minimal bounded DOM ─────────────────────────────────────────────────────

#[derive(Debug, Default)]
struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    text: String,
    children: Vec<Node>,
}

impl Node {
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|child| child.name == name)
    }

    fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |child| child.name == name)
    }

    fn path(&self, path: &str) -> Option<&Node> {
        path.split('/')
            .try_fold(self, |node, name| node.child(name))
    }

    fn text_at(&self, path: &str) -> Option<&str> {
        self.path(path).map(|node| node.text.trim())
    }

    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// `<Properties><Property name="...">` lookup used by GPIF tracks, staves and notes.
    fn property(&self, name: &str) -> Option<&Node> {
        self.child("Properties")?
            .children_named("Property")
            .find(|property| property.attr("name") == Some(name))
    }
}

fn parse_dom(xml: &str) -> Result<Node, Error> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<Node> = vec![Node::default()];
    let mut elements = 0usize;
    let open = |event: &quick_xml::events::BytesStart<'_>| -> Result<Node, Error> {
        let mut node = Node {
            name: String::from_utf8_lossy(event.local_name().as_ref()).into_owned(),
            ..Node::default()
        };
        for attribute in event.attributes().flatten() {
            let value = attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|error| Error::Xml(error.to_string()))?;
            node.attrs.push((
                String::from_utf8_lossy(attribute.key.local_name().as_ref()).into_owned(),
                value.into_owned(),
            ));
        }
        Ok(node)
    };
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                elements += 1;
                if elements > MAX_GPIF_ELEMENTS || stack.len() > MAX_GPIF_DEPTH {
                    return Err(Error::Xml("GPIF document is too large or too deep".into()));
                }
                stack.push(open(&event)?);
            }
            Ok(Event::Empty(event)) => {
                elements += 1;
                if elements > MAX_GPIF_ELEMENTS {
                    return Err(Error::Xml("GPIF document is too large".into()));
                }
                let node = open(&event)?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                }
            }
            Ok(Event::End(_)) => {
                let node = stack
                    .pop()
                    .ok_or_else(|| Error::Xml("unbalanced GPIF".into()))?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => return Err(Error::Xml("unbalanced GPIF".into())),
                }
            }
            Ok(Event::Text(text)) => {
                if let Some(node) = stack.last_mut() {
                    let decoded = text
                        .decode()
                        .map_err(|error| Error::Xml(error.to_string()))?;
                    node.text.push_str(&decoded);
                }
            }
            Ok(Event::CData(data)) => {
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&String::from_utf8_lossy(data.as_ref()));
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if let Some(node) = stack.last_mut() {
                    let reference = format!("&{};", String::from_utf8_lossy(reference.as_ref()));
                    if let Ok(decoded) = quick_xml::escape::unescape(&reference) {
                        node.text.push_str(&decoded);
                    }
                }
            }
            Ok(Event::DocType(_)) => {
                return Err(Error::Xml(
                    "DOCTYPE declarations are not allowed in GPIF".into(),
                ));
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(Error::Xml(error.to_string())),
            _ => {}
        }
    }
    let mut document = stack
        .pop()
        .filter(|_| stack.is_empty())
        .ok_or_else(|| Error::Xml("unbalanced GPIF".into()))?;
    document
        .children
        .pop()
        .ok_or_else(|| Error::Xml("empty GPIF document".into()))
}

// ── Conversion ──────────────────────────────────────────────────────────────

/// Unsupported GPIF content, counted per kind and reported once per kind.
#[derive(Default)]
struct Losses(BTreeMap<&'static str, (usize, &'static str)>);

impl Losses {
    fn add(&mut self, code: &'static str, reason: &'static str) {
        self.0.entry(code).or_insert((0, reason)).0 += 1;
    }

    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.0
            .into_iter()
            .map(|(code, (count, reason))| {
                let mut diagnostic = Diagnostic::warning(code, reason);
                diagnostic.preserved_value = Some(count.to_string());
                diagnostic
            })
            .collect()
    }
}

struct StaffInfo {
    tuning: Option<Vec<i16>>,
    capo: u8,
}

fn ids(text: &str) -> Vec<i64> {
    text.split_whitespace()
        .filter_map(|value| value.parse::<i64>().ok())
        .collect()
}

fn index_by_id<'a>(root: &'a Node, container: &str, item: &'a str) -> HashMap<String, &'a Node> {
    root.child(container)
        .map(|node| {
            node.children_named(item)
                .filter_map(|child| Some((child.attr("id")?.to_string(), child)))
                .collect()
        })
        .unwrap_or_default()
}

fn gp_duration(value: &str, losses: &mut Losses) -> Duration {
    match value {
        "Whole" => Duration::Whole,
        "Half" => Duration::Half,
        "Quarter" => Duration::Quarter,
        "Eighth" => Duration::Eighth,
        "16th" => Duration::Sixteenth,
        "32nd" => Duration::ThirtySecond,
        "64th" => Duration::SixtyFourth,
        _ => {
            losses.add(
                "gp.unsupported-duration",
                "note values shorter than a 64th are imported as 64ths",
            );
            Duration::SixtyFourth
        }
    }
}

fn gp_dynamic(value: &str) -> Option<Dynamic> {
    Some(match value {
        "PPP" => Dynamic::Ppp,
        "PP" => Dynamic::Pp,
        "P" => Dynamic::P,
        "MP" => Dynamic::Mp,
        "MF" => Dynamic::Mf,
        "F" => Dynamic::F,
        "FF" => Dynamic::Ff,
        "FFF" => Dynamic::Fff,
        _ => return None,
    })
}

fn gp_clef(value: &str) -> Option<Clef> {
    Some(match value {
        "G2" => Clef::Treble,
        "F4" => Clef::Bass,
        "C3" => Clef::Alto,
        "C4" => Clef::Tenor,
        "Neutral" => Clef::Percussion,
        _ => return None,
    })
}

fn step_semitone(step: &Step) -> i16 {
    match step {
        Step::C => 0,
        Step::D => 2,
        Step::E => 4,
        Step::F => 5,
        Step::G => 7,
        Step::A => 9,
        Step::B => 11,
    }
}

/// The note's sounding pitch from its MIDI number, spelled with the GPIF concert-pitch step and
/// accidental when they agree with it.
fn gp_pitch(note: &Node, midi: u8) -> Pitch {
    let spelled = note.property("ConcertPitch").and_then(|property| {
        let pitch = property.child("Pitch")?;
        let step = pitch
            .text_at("Step")?
            .chars()
            .next()
            .and_then(Step::from_char)?;
        let alter: i16 = match pitch.text_at("Accidental").unwrap_or("") {
            "" => 0,
            "#" => 1,
            "b" => -1,
            "##" | "x" => 2,
            "bb" => -2,
            _ => return None,
        };
        let natural = i16::from(midi) - alter - step_semitone(&step);
        (natural.rem_euclid(12) == 0).then(|| {
            let octave = (natural / 12 - 1) as i8;
            let mut pitch = Pitch::new(step, octave);
            pitch.alter = alter as i8;
            pitch
        })
    });
    spelled
        .filter(|pitch| pitch.to_midi() == i16::from(midi))
        .unwrap_or_else(|| Pitch::from_midi(midi, false))
}

fn property_text<'a>(note: &'a Node, name: &str, child: &str) -> Option<&'a str> {
    note.property(name)?.text_at(child)
}

fn property_float(note: &Node, name: &str) -> Option<f64> {
    property_text(note, name, "Float")?.parse::<f64>().ok()
}

/// Fill `beats` of rests into `voice` using the largest note values that fit.
fn pad_with_rests(voice: &mut Vec<Note>, mut beats: f64) {
    const VALUES: [(Duration, f64); 7] = [
        (Duration::Whole, 4.0),
        (Duration::Half, 2.0),
        (Duration::Quarter, 1.0),
        (Duration::Eighth, 0.5),
        (Duration::Sixteenth, 0.25),
        (Duration::ThirtySecond, 0.125),
        (Duration::SixtyFourth, 0.0625),
    ];
    while beats > 1e-6 {
        let Some((duration, length)) = VALUES.iter().find(|(_, length)| *length <= beats + 1e-6)
        else {
            break;
        };
        voice.push(Note::rest(duration.clone()));
        beats -= length;
    }
}

/// Convert GPIF XML into a score and the diagnostics for what the model cannot hold.
pub fn parse_gpif(xml: &str) -> Result<(Score, Vec<Diagnostic>), Error> {
    let root = parse_dom(xml)?;
    if root.name != "GPIF" {
        return Err(Error::Xml(format!(
            "expected a GPIF document, found <{}>",
            root.name
        )));
    }
    let mut losses = Losses::default();
    let rhythms = index_by_id(&root, "Rhythms", "Rhythm");
    let notes = index_by_id(&root, "Notes", "Note");
    let beats = index_by_id(&root, "Beats", "Beat");
    let voices = index_by_id(&root, "Voices", "Voice");
    let bars = index_by_id(&root, "Bars", "Bar");

    let mut score = Score::default();
    score.parts.clear();
    if let Some(info) = root.child("Score") {
        let text = |name: &str| info.text_at(name).unwrap_or("").to_string();
        if !text("Title").is_empty() {
            score.metadata.title = text("Title");
        }
        score.metadata.movement_title = text("SubTitle");
        score.metadata.composer = if text("Music").is_empty() {
            text("Artist")
        } else {
            text("Music")
        };
        score.metadata.lyricist = text("Words");
        score.metadata.copyright = text("Copyright");
    }

    // Tracks → parts; GPIF staves → staves (flattened in master-bar order).
    let mut staff_infos: Vec<(usize, StaffInfo)> = Vec::new();
    let tracks = root
        .child("Tracks")
        .map(|tracks| tracks.children_named("Track").collect::<Vec<_>>())
        .unwrap_or_default();
    for track in &tracks {
        let name = track.text_at("Name").unwrap_or("Track");
        let mut part = Part::new(name, track.text_at("ShortName").unwrap_or(""));
        let program = track
            .text_at("Sounds/Sound/MIDI/Program")
            .or_else(|| track.text_at("GeneralMidi/Program"))
            .and_then(|value| value.parse::<u8>().ok());
        if let Some(program) = program.filter(|program| *program < 128) {
            part.midi_program = program;
        }
        let channel = track
            .text_at("MidiConnection/PrimaryChannel")
            .or_else(|| track.text_at("GeneralMidi/PrimaryChannel"))
            .and_then(|value| value.parse::<u8>().ok());
        if let Some(channel) = channel.filter(|channel| *channel < 16) {
            part.midi_channel = channel;
        }
        let drums = track.text_at("InstrumentSet/Type") == Some("drumKit");
        if drums {
            losses.add(
                "gp.drum-kit",
                "drum-kit tracks import as notes at each articulation's MIDI key, without a kit map",
            );
        }
        // GP 7.5+ stores staff properties per staff; GP 7.0 on the track.
        let staff_nodes: Vec<&Node> = match track.child("Staves") {
            Some(staves) => staves.children_named("Staff").collect(),
            None => vec![*track],
        };
        let part_index = score.parts.len();
        for staff_node in staff_nodes {
            let tuning = (!drums)
                .then(|| staff_node.property("Tuning"))
                .flatten()
                .and_then(|property| property.text_at("Pitches"))
                .map(|pitches| {
                    pitches
                        .split_whitespace()
                        .filter_map(|value| value.parse::<i16>().ok())
                        .collect::<Vec<_>>()
                })
                .filter(|tuning| !tuning.is_empty() && tuning.len() <= 16);
            let capo = staff_node
                .property("CapoFret")
                .and_then(|property| property.text_at("Fret"))
                .and_then(|value| value.parse::<u8>().ok())
                .unwrap_or(0);
            let mut staff = Staff::new(if drums {
                Clef::Percussion
            } else {
                Clef::Treble
            });
            if let Some(tuning) = &tuning {
                staff.tablature = Some(TablatureConfig {
                    lines: tuning.len() as u8,
                    tuning_midi: tuning.clone(),
                    capo,
                });
                staff.presentation.kind = StaffKind::Tablature;
                staff.presentation.lines = tuning.len() as u8;
            } else if drums {
                staff.presentation.kind = StaffKind::Percussion;
            }
            part.staves.push(staff);
            staff_infos.push((part_index, StaffInfo { tuning, capo }));
        }
        score.parts.push(part);
    }
    if score.parts.is_empty() {
        return Err(Error::Empty);
    }
    let drum_tracks: Vec<bool> = tracks
        .iter()
        .map(|track| track.text_at("InstrumentSet/Type") == Some("drumKit"))
        .collect();
    let drum_keys: Vec<Vec<u8>> =
        tracks
            .iter()
            .map(|track| {
                track
                    .path("InstrumentSet/Elements")
                    .map(|elements| {
                        elements
                            .children_named("Element")
                            .flat_map(|element| {
                                element.child("Articulations").into_iter().flat_map(
                                    |articulations| articulations.children_named("Articulation"),
                                )
                            })
                            .map(|articulation| {
                                articulation
                                    .text_at("OutputMidiNumber")
                                    .and_then(|value| value.parse::<u8>().ok())
                                    .unwrap_or(38)
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect();

    // Tempo automations (bar index → quarter-note BPM).
    let mut tempos: BTreeMap<usize, u16> = BTreeMap::new();
    if let Some(automations) = root.path("MasterTrack/Automations") {
        for automation in automations.children_named("Automation") {
            if automation.text_at("Type") != Some("Tempo") {
                continue;
            }
            let bar = automation
                .text_at("Bar")
                .and_then(|value| value.parse::<usize>().ok());
            let bpm = automation
                .text_at("Value")
                .and_then(|value| value.split_whitespace().next())
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|bpm| bpm.is_finite() && (1.0..=999.0).contains(bpm));
            if let (Some(bar), Some(bpm)) = (bar, bpm) {
                tempos.entry(bar).or_insert(bpm.round() as u16);
            }
        }
    }

    let master_bars = root
        .child("MasterBars")
        .map(|node| node.children_named("MasterBar").collect::<Vec<_>>())
        .unwrap_or_default();
    if master_bars.len() > MAX_GP_MEASURES {
        return Err(Error::Xml("Guitar Pro file has too many bars".into()));
    }
    let staff_count = staff_infos.len();
    let mut current_time: Option<TimeSignature> = None;
    let mut current_key: Option<i8> = None;
    let mut current_clefs: Vec<Option<Clef>> = vec![None; staff_count];
    let mut last_dynamic: Vec<String> = vec!["F".to_string(); staff_count];
    let mut previous_ending: Option<String> = None;

    for (bar_index, master_bar) in master_bars.iter().enumerate() {
        let (numerator, denominator) = master_bar
            .text_at("Time")
            .and_then(|value| value.split_once('/'))
            .and_then(|(n, d)| Some((n.trim().parse::<u8>().ok()?, d.trim().parse::<u8>().ok()?)))
            .filter(|(n, d)| *n > 0 && *d > 0 && d.is_power_of_two())
            .unwrap_or((4, 4));
        let time = TimeSignature {
            numerator,
            denominator,
        };
        let time_changed = current_time.as_ref() != Some(&time);
        if bar_index == 0 {
            score.settings.time_signature = time.clone();
        }
        let fifths = master_bar
            .text_at("Key/AccidentalCount")
            .and_then(|value| value.parse::<i8>().ok())
            .filter(|fifths| (-7..=7).contains(fifths))
            .unwrap_or(0);
        let mode = if master_bar.text_at("Key/Mode") == Some("Minor") {
            "minor"
        } else {
            "major"
        };
        let key_changed = current_key != Some(fifths);
        if bar_index == 0 {
            score.settings.key_signature = KeySignature {
                fifths,
                mode: mode.to_string(),
            };
        }
        let repeat = master_bar.child("Repeat");
        let repeat_start = repeat.and_then(|node| node.attr("start")) == Some("true");
        let repeat_end = repeat.and_then(|node| node.attr("end")) == Some("true");
        let ending = master_bar
            .text_at("AlternateEndings")
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let next_ending = master_bars
            .get(bar_index + 1)
            .and_then(|next| next.text_at("AlternateEndings"))
            .filter(|value| !value.is_empty());
        let volta = ending.as_ref().and_then(|ending| {
            let numbers = ids(ending);
            if numbers.len() > 1 {
                losses.add(
                    "gp.multi-number-ending",
                    "an ending shared by several passes keeps only its first number",
                );
            }
            let number = u8::try_from(*numbers.first()?).ok()?;
            let starts = previous_ending.as_deref() != Some(ending.as_str());
            let ends = next_ending != Some(ending.as_str());
            Some(VoltaBracket {
                number,
                kind: match (starts, ends) {
                    (true, true) => "begin_end",
                    (true, false) => "begin",
                    (false, true) => "end",
                    (false, false) => "mid",
                }
                .to_string(),
            })
        });
        previous_ending = ending;
        let section = master_bar.child("Section").and_then(|section| {
            let letter = section.text_at("Letter").unwrap_or("");
            let text = section.text_at("Text").unwrap_or("");
            let label = match (letter.is_empty(), text.is_empty()) {
                (false, false) => format!("{letter} {text}"),
                (false, true) => letter.to_string(),
                (true, false) => text.to_string(),
                (true, true) => return None,
            };
            Some(label)
        });
        if master_bar.child("Fermatas").is_some() {
            losses.add(
                "gp.unsupported-fermata",
                "bar fermatas (positioned by offset) are not imported",
            );
        }
        if master_bar.child("Directions").is_some() {
            losses.add(
                "gp.unsupported-direction",
                "coda/segno/fine directions are not imported",
            );
        }
        let bar_ids = master_bar.text_at("Bars").map(ids).unwrap_or_default();

        for staff_index in 0..staff_count {
            let (part_index, info) = &staff_infos[staff_index];
            let local_staff = staff_infos[..staff_index]
                .iter()
                .filter(|(part, _)| part == part_index)
                .count();
            let mut measure = Measure::empty(numerator, denominator);
            measure.number = (bar_index + 1) as u32;
            measure.voices = [vec![], vec![], vec![], vec![]];
            if time_changed && bar_index > 0 {
                measure.time_sig = Some(time.clone());
            }
            if key_changed && bar_index > 0 {
                measure.key_sig = Some(KeySignature {
                    fifths,
                    mode: mode.to_string(),
                });
            }
            if repeat_start {
                measure.barline_left = Barline::RepeatStart;
            }
            if repeat_end {
                measure.barline_right = Barline::RepeatEnd;
            } else if master_bar.child("DoubleBar").is_some() {
                measure.barline_right = Barline::Double;
            }
            measure.volta = volta.clone();
            if staff_index == 0 {
                measure.rehearsal = section.clone();
                measure.tempo = tempos.get(&bar_index).copied();
                if bar_index == 0
                    && let Some(bpm) = measure.tempo
                {
                    score.settings.tempo_bpm = bpm;
                }
            }
            let bar = bar_ids
                .get(staff_index)
                .filter(|id| **id >= 0)
                .and_then(|id| bars.get(&id.to_string()));
            if let Some(bar) = bar {
                if let Some(clef) = bar.text_at("Clef").and_then(gp_clef) {
                    if current_clefs[staff_index].is_none() {
                        score.parts[*part_index].staves[local_staff].clef = clef.clone();
                    } else if current_clefs[staff_index].as_ref() != Some(&clef) {
                        measure.clef = Some(clef.clone());
                    }
                    current_clefs[staff_index] = Some(clef);
                }
                if bar.child("SimileMark").is_some() {
                    losses.add(
                        "gp.unsupported-simile",
                        "simile (repeat-bar) marks are not imported; the bar keeps its notes",
                    );
                }
                let voice_ids = bar.text_at("Voices").map(ids).unwrap_or_default();
                for (voice_index, voice_id) in voice_ids.iter().enumerate().take(4) {
                    if *voice_id < 0 {
                        continue;
                    }
                    let Some(voice) = voices.get(&voice_id.to_string()) else {
                        continue;
                    };
                    let beat_ids = voice.text_at("Beats").map(ids).unwrap_or_default();
                    for beat_id in beat_ids {
                        let Some(beat) = beats.get(&beat_id.to_string()) else {
                            continue;
                        };
                        if let Some(note) = convert_beat(
                            beat,
                            &BeatContext {
                                rhythms: &rhythms,
                                notes: &notes,
                                tuning: info.tuning.as_deref(),
                                drum_keys: drum_tracks
                                    .get(*part_index)
                                    .copied()
                                    .unwrap_or(false)
                                    .then(|| drum_keys[*part_index].as_slice()),
                            },
                            &mut last_dynamic[staff_index],
                            &mut measure,
                            &mut losses,
                        ) {
                            measure.voices[voice_index].push(note);
                        }
                    }
                }
            }
            let _ = info.capo;
            score.parts[*part_index].staves[local_staff]
                .measures
                .push(measure);
        }
        current_time = Some(time);
        current_key = Some(fifths);
    }
    if master_bars.is_empty() {
        return Err(Error::Empty);
    }
    finish_measures(&mut score, &master_bars);
    resolve_hammer_pull(&mut score);
    Ok((score, losses.into_diagnostics()))
}

struct BeatContext<'a> {
    rhythms: &'a HashMap<String, &'a Node>,
    notes: &'a HashMap<String, &'a Node>,
    tuning: Option<&'a [i16]>,
    drum_keys: Option<&'a [u8]>,
}

/// Marker kept in `technique_text` until the hammer-on/pull-off direction is known.
const HOPO_MARK: &str = "\u{0}gp-hopo";

fn convert_beat(
    beat: &Node,
    context: &BeatContext<'_>,
    last_dynamic: &mut String,
    measure: &mut Measure,
    losses: &mut Losses,
) -> Option<Note> {
    let rhythm = beat
        .child("Rhythm")
        .and_then(|rhythm| rhythm.attr("ref"))
        .and_then(|id| context.rhythms.get(id));
    let duration = rhythm
        .and_then(|rhythm| rhythm.text_at("NoteValue"))
        .map(|value| gp_duration(value, losses))
        .unwrap_or(Duration::Quarter);
    let dots = rhythm
        .and_then(|rhythm| rhythm.child("AugmentationDot"))
        .and_then(|dot| dot.attr("count"))
        .and_then(|count| count.parse::<u8>().ok())
        .unwrap_or(0)
        .min(3);
    let tuplet = rhythm
        .and_then(|rhythm| rhythm.child("PrimaryTuplet"))
        .and_then(|tuplet| {
            let actual = tuplet.attr("num")?.parse::<u8>().ok()?;
            let normal = tuplet.attr("den")?.parse::<u8>().ok()?;
            (actual > 0 && normal > 0).then_some(TupletInfo {
                actual_notes: actual,
                normal_notes: normal,
            })
        });
    if rhythm
        .and_then(|rhythm| rhythm.child("SecondaryTuplet"))
        .is_some()
    {
        losses.add(
            "gp.nested-tuplet",
            "nested (secondary) tuplets keep only the outer ratio",
        );
    }

    let note_ids = beat.text_at("Notes").map(ids).unwrap_or_default();
    let mut result: Option<Note> = None;
    let mut member_ties: Vec<(bool, bool)> = Vec::new();
    for note_id in note_ids {
        let Some(node) = context.notes.get(&note_id.to_string()) else {
            continue;
        };
        let string = property_text(node, "String", "String").and_then(|v| v.parse::<u8>().ok());
        let fret = property_text(node, "Fret", "Fret").and_then(|v| v.parse::<u8>().ok());
        let midi = match context.drum_keys {
            Some(keys) => node
                .text_at("InstrumentArticulation")
                .and_then(|value| value.parse::<usize>().ok())
                .and_then(|index| keys.get(index).copied()),
            None => property_text(node, "Midi", "Number")
                .and_then(|value| value.parse::<u8>().ok())
                .or_else(|| {
                    let open = *context.tuning?.get(usize::from(string?))?;
                    u8::try_from(open + i16::from(fret?)).ok()
                }),
        }
        .filter(|midi| *midi <= 127);
        let Some(midi) = midi else {
            losses.add(
                "gp.note-without-pitch",
                "a note with neither a MIDI pitch nor a resolvable string/fret is skipped",
            );
            continue;
        };
        let pitch = if context.drum_keys.is_some() {
            Pitch::from_midi(midi, false)
        } else {
            gp_pitch(node, midi)
        };
        // GPIF strings count from 0 = lowest; acorde's string 1 is the lowest.
        let tab = match (context.tuning, string, fret) {
            (Some(tuning), Some(string), Some(fret)) if usize::from(string) < tuning.len() => {
                Some(TabPosition {
                    string: string + 1,
                    fret,
                })
            }
            _ => None,
        };
        let note = match result.as_mut() {
            Some(note) => {
                note.pitches.push(pitch);
                note
            }
            None => {
                result = Some(Note::new(pitch, duration.clone()));
                result.as_mut()?
            }
        };
        if context.drum_keys.is_some() {
            note.is_unpitched = true;
        }
        if let Some(tab) = tab {
            if note.tab_position.is_none() {
                note.tab_position = Some(tab.clone());
            }
            note.tab_positions.push(tab);
        }
        apply_note_effects(node, note, losses);
        let tie = node.child("Tie");
        let flag = |name: &str| tie.and_then(|tie| tie.attr(name)) == Some("true");
        member_ties.push((flag("origin"), flag("destination")));
    }
    if member_ties.len() > 1
        && member_ties
            .iter()
            .any(|state| state.0 != member_ties[0].0 || state.1 != member_ties[0].1)
    {
        losses.add(
            "gp.partial-chord-tie",
            "a tie on only some notes of a chord is applied to the whole chord",
        );
    }

    let mut note = match result {
        Some(note) => note,
        None => Note::rest(duration),
    };
    note.dot_count = dots;
    note.tuplet = tuplet;
    if let Some(grace) = beat.text_at("GraceNotes") {
        note.is_grace = true;
        if grace == "OnBeat" {
            losses.add(
                "gp.on-beat-grace",
                "on-beat grace notes import as ordinary (before-beat) grace notes",
            );
        }
    }
    // Grace beats carry their own (usually default) dynamic; only main beats mark a change.
    if let Some(dynamic) = beat.text_at("Dynamic")
        && !note.is_rest
        && !note.is_grace
        && dynamic != last_dynamic.as_str()
    {
        note.dynamic = gp_dynamic(dynamic);
        *last_dynamic = dynamic.to_string();
    }
    if let Some(lyrics) = beat.child("Lyrics") {
        for (index, line) in lyrics.children_named("Line").enumerate() {
            let text = line.text.trim();
            if text.is_empty() || note.is_rest {
                continue;
            }
            let lyric = Lyric {
                text: text.to_string(),
                syllabic: "single".to_string(),
            };
            if index == 0 {
                note.lyric = Some(lyric);
            } else if let Ok(verse) = u8::try_from(index + 1) {
                note.additional_lyrics
                    .push(acorde_core::VerseLyric { verse, lyric });
            }
        }
    }
    if let Some(text) = beat.text_at("FreeText").filter(|text| !text.is_empty()) {
        measure.texts.push(StyledText {
            style: TextStyle::Generic,
            text: text.to_string(),
            placement: Some("above".to_string()),
            offset_x: None,
            offset_y: None,
            relative_x: None,
            relative_y: None,
        });
    }
    for (element, code, reason) in [
        (
            "Whammy",
            "gp.unsupported-whammy",
            "whammy-bar dives are not imported",
        ),
        (
            "Chord",
            "gp.unsupported-chord-diagram",
            "chord names/diagrams attached to beats are not imported",
        ),
        (
            "Wah",
            "gp.unsupported-wah",
            "wah pedal marks are not imported",
        ),
        (
            "Fadding",
            "gp.unsupported-volume-swell",
            "fade-in/volume swells are not imported",
        ),
        (
            "Ottavia",
            "gp.unsupported-beat-ottava",
            "beat ottava marks are not imported",
        ),
    ] {
        if beat.child(element).is_some() {
            losses.add(code, reason);
        }
    }
    // Guitar Pro 6 stores whammy dives as beat properties instead of a <Whammy> element.
    if beat.child("Whammy").is_none() && beat.property("WhammyBar").is_some() {
        losses.add("gp.unsupported-whammy", "whammy-bar dives are not imported");
    }
    if !note.is_rest {
        apply_beat_strokes(beat, &mut note, losses);
    }
    Some(note)
}

/// Beat-level picking marks. Tremolo picking becomes stem slashes (`1/2` = one mark, `1/4` = two,
/// `1/8` = three, as Guitar Pro draws them). A pick stroke is written with the down-bow/up-bow
/// signs MusicXML also uses for picking. A brush (strum) becomes an arpeggio: a downstroke strikes
/// the low string first, so it rolls upwards in pitch.
fn apply_beat_strokes(beat: &Node, note: &mut Note, losses: &mut Losses) {
    if let Some(value) = beat.text_at("Tremolo") {
        let marks = match value {
            "1/2" => Some(1),
            "1/4" => Some(2),
            "1/8" => Some(3),
            _ => None,
        };
        match marks {
            Some(marks) => push_articulation(note, Articulation::Tremolo(marks)),
            None => losses.add(
                "gp.unsupported-tremolo-picking",
                "tremolo picking with an unknown speed is not imported",
            ),
        }
    }
    let direction = |name: &str| {
        beat.property(name)
            .map(|property| property.text_at("Direction").unwrap_or("Down") == "Up")
    };
    if let Some(up) = direction("PickStroke") {
        push_articulation(
            note,
            if up {
                Articulation::UpBow
            } else {
                Articulation::DownBow
            },
        );
    }
    if let Some(up) = direction("Brush") {
        note.arpeggiate = Some(!up);
    }
}

fn push_articulation(note: &mut Note, articulation: Articulation) {
    if !note.articulations.contains(&articulation) {
        note.articulations.push(articulation);
    }
}

fn apply_note_effects(node: &Node, note: &mut Note, losses: &mut Losses) {
    if let Some(tie) = node.child("Tie") {
        if tie.attr("origin") == Some("true") {
            note.tie_start = true;
        }
        if tie.attr("destination") == Some("true") {
            note.tie_end = true;
        }
    }
    if node.property("Muted").is_some() {
        note.note_head = NoteHead::X;
    }
    if node.property("PalmMuted").is_some() {
        note.technique_text = Some("P.M.".to_string());
    } else if node.child("LetRing").is_some() && note.technique_text.is_none() {
        note.technique_text = Some("let ring".to_string());
    }
    if let Some(flags) = node
        .text_at("Accent")
        .and_then(|value| value.parse::<u32>().ok())
    {
        let mut push = |articulation: Articulation| {
            if !note.articulations.contains(&articulation) {
                note.articulations.push(articulation);
            }
        };
        if flags & 0x01 != 0 {
            push(Articulation::Staccato);
        }
        if flags & 0x04 != 0 {
            push(Articulation::Marcato);
        }
        if flags & 0x08 != 0 {
            push(Articulation::Accent);
        }
        if flags & 0x10 != 0 {
            push(Articulation::Tenuto);
        }
    }
    if node.property("Harmonic").is_some() && !note.articulations.contains(&Articulation::Harmonic)
    {
        note.articulations.push(Articulation::Harmonic);
        if property_text(node, "HarmonicType", "HType").is_some_and(|kind| kind != "Natural") {
            losses.add(
                "gp.harmonic-type",
                "artificial, pinch, tap and semi harmonics import as natural harmonics",
            );
        }
    }
    if node.child("Trill").is_some() && !note.articulations.contains(&Articulation::Trill) {
        note.articulations.push(Articulation::Trill);
    }
    if node.property("Bended").is_some() && note.guitar_technique.is_none() {
        note.guitar_technique = Some(GuitarTechnique::Bend);
        // GPIF bend values are hundredths of a whole tone (100 = full bend = 200 cents); offsets
        // are percent of the note.
        let point = |value: &str, offset: &str| {
            let alter = property_float(node, value)?;
            let position = property_float(node, offset).unwrap_or(0.0);
            Some(GuitarBendPoint {
                position_per_mille: (position.clamp(0.0, 100.0) * 10.0).round() as u16,
                alter_cents: (alter * 2.0).round().clamp(-2400.0, 2400.0) as i16,
            })
        };
        note.guitar_bend_curve = [
            point("BendOriginValue", "BendOriginOffset"),
            point("BendMiddleValue", "BendMiddleOffset1"),
            point("BendMiddleValue", "BendMiddleOffset2"),
            point("BendDestinationValue", "BendDestinationOffset"),
        ]
        .into_iter()
        .flatten()
        .collect();
        // acorde curves span the whole note: strictly increasing from 0 to 1000 per mille.
        note.guitar_bend_curve
            .sort_by_key(|point| point.position_per_mille);
        note.guitar_bend_curve
            .dedup_by(|a, b| a.position_per_mille == b.position_per_mille);
        if let Some(first) = note.guitar_bend_curve.first().copied()
            && first.position_per_mille > 0
        {
            note.guitar_bend_curve.insert(
                0,
                GuitarBendPoint {
                    position_per_mille: 0,
                    alter_cents: first.alter_cents,
                },
            );
        }
        if let Some(last) = note.guitar_bend_curve.last().copied()
            && last.position_per_mille < 1000
        {
            note.guitar_bend_curve.push(GuitarBendPoint {
                position_per_mille: 1000,
                alter_cents: last.alter_cents,
            });
        }
        note.guitar_bend_alter_cents = note
            .guitar_bend_curve
            .iter()
            .map(|point| point.alter_cents)
            .max_by_key(|cents| cents.abs());
    }
    if node.property("HopoOrigin").is_some() && note.guitar_technique.is_none() {
        note.guitar_technique = Some(GuitarTechnique::HammerOn);
        if note.technique_text.is_none() {
            note.technique_text = Some(HOPO_MARK.to_string());
        }
    }
    if let Some(flags) = property_text(node, "Slide", "Flags").and_then(|v| v.parse::<u32>().ok()) {
        if flags & 0x03 != 0 && note.guitar_technique.is_none() {
            note.guitar_technique = Some(GuitarTechnique::Slide);
        }
        if flags & !0x03 != 0 {
            losses.add(
                "gp.unsupported-slide-type",
                "slide-in/out and pick slides are not imported (shift and legato slides are)",
            );
        }
    }
    for (property, code, reason) in [
        (
            "Tapped",
            "gp.unsupported-tapping",
            "tapping marks are not imported",
        ),
        (
            "LeftHandTapped",
            "gp.unsupported-tapping",
            "tapping marks are not imported",
        ),
    ] {
        if node.property(property).is_some() {
            losses.add(code, reason);
        }
    }
    for (element, code, reason) in [
        (
            "Vibrato",
            "gp.unsupported-vibrato",
            "note vibrato is not imported",
        ),
        (
            "RightFingering",
            "gp.unsupported-right-hand-fingering",
            "right-hand (p-i-m-a-c) fingering is not imported",
        ),
    ] {
        if node.child(element).is_some() {
            losses.add(code, reason);
        }
    }
    // Left-hand fingering: GPIF letters I/M/A/C are fingers 1–4; the thumb (P) has no number.
    if let Some(finger) = node.text_at("LeftFingering") {
        let number = match finger {
            "I" => Some(1),
            "M" => Some(2),
            "A" => Some(3),
            "C" => Some(4),
            _ => None,
        };
        match number {
            Some(number) if note.pitches.len() == 1 => note.fingering = Some(number),
            _ => losses.add(
                "gp.unsupported-fingering",
                "thumb fingering and fingering on chord members are not imported",
            ),
        }
    }
    if node.child("LetRing").is_some() && node.property("PalmMuted").is_some() {
        losses.add(
            "gp.let-ring-with-palm-mute",
            "a note both palm-muted and let ring keeps only the palm-mute text",
        );
    }
}

/// Decide hammer-on versus pull-off from the next note in the same voice, and clear the marker.
fn resolve_hammer_pull(score: &mut Score) {
    for part in &mut score.parts {
        for staff in &mut part.staves {
            for voice_index in 0..4 {
                let mut positions: Vec<(usize, usize)> = Vec::new();
                for (measure_index, measure) in staff.measures.iter().enumerate() {
                    for note_index in 0..measure.voices[voice_index].len() {
                        positions.push((measure_index, note_index));
                    }
                }
                for (index, &(measure_index, note_index)) in positions.iter().enumerate() {
                    let marked = staff.measures[measure_index].voices[voice_index][note_index]
                        .technique_text
                        .as_deref()
                        == Some(HOPO_MARK);
                    if !marked {
                        continue;
                    }
                    let next_midi = positions.get(index + 1).and_then(|&(m, n)| {
                        let next = &staff.measures[m].voices[voice_index][n];
                        (!next.is_rest)
                            .then(|| next.pitches.first().map(Pitch::to_midi))
                            .flatten()
                    });
                    let note = &mut staff.measures[measure_index].voices[voice_index][note_index];
                    let own = note.pitches.first().map(Pitch::to_midi);
                    if let (Some(own), Some(next)) = (own, next_midi)
                        && next < own
                    {
                        note.guitar_technique = Some(GuitarTechnique::PullOff);
                    }
                    note.technique_text = None;
                }
            }
        }
    }
}

/// Make every measure hold its time: an anacrusis keeps its short length, an overfull bar keeps
/// its content as an irregular length, and a short voice 1 is completed with rests (Guitar Pro
/// allows incomplete bars).
fn finish_measures(score: &mut Score, master_bars: &[&Node]) {
    let pickup = master_bars
        .first()
        .is_some_and(|bar| bar.child("Anacrusis").is_some());
    let mut time = score.settings.time_signature.clone();
    let measure_count = score
        .parts
        .first()
        .and_then(|part| part.staves.first())
        .map_or(0, |staff| staff.measures.len());
    let mut times = Vec::with_capacity(measure_count);
    for measure_index in 0..measure_count {
        if let Some(changed) = score.parts[0].staves[0].measures[measure_index]
            .time_sig
            .clone()
        {
            time = changed;
        }
        times.push(time.clone());
    }
    for (measure_index, time) in times.iter().enumerate() {
        let bar_beats = time.total_beats();
        let content = score
            .parts
            .iter()
            .flat_map(|part| part.staves.iter())
            .filter_map(|staff| staff.measures.get(measure_index))
            .flat_map(|measure| measure.voices.iter())
            .map(|voice| acorde_core::voice_duration_beats(voice, bar_beats))
            .fold(0.0_f64, f64::max);
        let length = if (measure_index == 0 && pickup && content > 0.0 && content < bar_beats)
            || content > bar_beats + 1e-9
        {
            MeasureLength::from_ticks((content * 960.0).round() as u32, 960)
        } else {
            None
        };
        let target = length
            .and_then(|length| length.beats())
            .unwrap_or(bar_beats);
        for part in &mut score.parts {
            for staff in &mut part.staves {
                let Some(measure) = staff.measures.get_mut(measure_index) else {
                    continue;
                };
                measure.actual_length = length;
                // Guitar Pro writes an empty bar as a lone placeholder rest beat (often a quarter)
                // and shows it as a bar rest. A bar holding only rests becomes a measure rest.
                if length.is_none()
                    && measure
                        .voices
                        .iter()
                        .flatten()
                        .all(|note| note.is_rest && !note.is_grace)
                {
                    for voice in &mut measure.voices {
                        voice.clear();
                    }
                    measure.voices[0].push(Note::rest(Duration::Whole));
                    continue;
                }
                let filled = acorde_core::voice_duration_beats(&measure.voices[0], target);
                if filled + 1e-9 < target {
                    pad_with_rests(&mut measure.voices[0], target - filled);
                }
                // Guitar Pro beams automatically by beat; GPIF stores no groups for that default.
                for voice in &mut measure.voices {
                    if voice.iter().all(|note| note.beam == BeamState::None) {
                        let beams = compute_beams(voice, time);
                        for (note, beam) in voice.iter_mut().zip(beams) {
                            note.beam = beam;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const GPIF: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<GPIF>
<Score><Title><![CDATA[Riff & Roll]]></Title><Artist>Band</Artist><Music>Composer</Music><Words /></Score>
<MasterTrack><Automations><Automation><Type>Tempo</Type><Linear>false</Linear><Bar>0</Bar><Position>0</Position><Visible>true</Visible><Value>96 2</Value></Automation></Automations></MasterTrack>
<Tracks><Track id="0"><Name>Lead</Name><ShortName>Gt</ShortName>
<Staves><Staff><Properties><Property name="CapoFret"><Fret>2</Fret></Property><Property name="Tuning"><Pitches>38 45 50 55 59 64</Pitches></Property></Properties></Staff></Staves>
<Sounds><Sound><MIDI><Program>29</Program></MIDI></Sound></Sounds><MidiConnection><PrimaryChannel>3</PrimaryChannel></MidiConnection></Track></Tracks>
<MasterBars>
<MasterBar><Key><AccidentalCount>1</AccidentalCount><Mode>Minor</Mode></Key><Time>4/4</Time><Bars>0</Bars><Repeat start="true" end="false" count="0" /><Section><Letter>A</Letter><Text>Intro</Text></Section></MasterBar>
<MasterBar><Key><AccidentalCount>1</AccidentalCount><Mode>Minor</Mode></Key><Time>3/4</Time><Bars>1</Bars><Repeat start="false" end="true" count="2" /></MasterBar>
<MasterBar><Key><AccidentalCount>1</AccidentalCount><Mode>Minor</Mode></Key><Time>3/4</Time><Bars>2</Bars></MasterBar>
</MasterBars>
<Bars><Bar id="0"><Clef>G2</Clef><Voices>0 -1 -1 -1</Voices></Bar><Bar id="1"><Clef>G2</Clef><Voices>1 -1 -1 -1</Voices></Bar><Bar id="2"><Clef>G2</Clef><Voices>2 -1 -1 -1</Voices></Bar></Bars>
<Voices><Voice id="0"><Beats>0 1 2 3</Beats></Voice><Voice id="1"><Beats>4</Beats></Voice><Voice id="2"><Beats>3</Beats></Voice></Voices>
<Beats>
<Beat id="0"><Dynamic>MF</Dynamic><Rhythm ref="0" /><Notes>0 1</Notes><Lyrics><Line>Hey</Line><Line /></Lyrics></Beat>
<Beat id="1"><Dynamic>MF</Dynamic><Rhythm ref="0" /><Notes>2</Notes><Tremolo>1/4</Tremolo><Properties><Property name="PickStroke"><Direction>Up</Direction></Property><Property name="Brush"><Direction>Down</Direction></Property></Properties></Beat>
<Beat id="2"><Dynamic>MF</Dynamic><Rhythm ref="0" /><Notes>3</Notes><Whammy /></Beat>
<Beat id="3"><Dynamic>F</Dynamic><Rhythm ref="1" /></Beat>
<Beat id="4"><Dynamic>F</Dynamic><Rhythm ref="2" /><Notes>4</Notes></Beat>
</Beats>
<Notes>
<Note id="0"><Tie origin="true" destination="false" /><Properties><Property name="ConcertPitch"><Pitch><Step>A</Step><Accidental /><Octave>3</Octave></Pitch></Property><Property name="Fret"><Fret>7</Fret></Property><Property name="Midi"><Number>45</Number></Property><Property name="String"><String>0</String></Property><Property name="PalmMuted"><Enable /></Property></Properties></Note>
<Note id="1"><Properties><Property name="Fret"><Fret>5</Fret></Property><Property name="Midi"><Number>55</Number></Property><Property name="String"><String>2</String></Property></Properties></Note>
<Note id="2"><Properties><Property name="ConcertPitch"><Pitch><Step>F</Step><Accidental>#</Accidental><Octave>4</Octave></Pitch></Property><Property name="Fret"><Fret>7</Fret></Property><Property name="Midi"><Number>66</Number></Property><Property name="String"><String>4</String></Property><Property name="HopoOrigin"><Enable /></Property></Properties><Accent>8</Accent></Note>
<Note id="3"><Properties><Property name="Fret"><Fret>5</Fret></Property><Property name="Midi"><Number>64</Number></Property><Property name="String"><String>4</String></Property><Property name="Bended"><Enable /></Property><Property name="BendOriginValue"><Float>0</Float></Property><Property name="BendOriginOffset"><Float>0</Float></Property><Property name="BendDestinationValue"><Float>100</Float></Property><Property name="BendDestinationOffset"><Float>60</Float></Property></Properties></Note>
<Note id="4"><LeftFingering>M</LeftFingering><Properties><Property name="Fret"><Fret>0</Fret></Property><Property name="Midi"><Number>40</Number></Property><Property name="String"><String>0</String></Property><Property name="Muted"><Enable /></Property></Properties></Note>
</Notes>
<Rhythms><Rhythm id="0"><NoteValue>Quarter</NoteValue></Rhythm><Rhythm id="1"><NoteValue>Eighth</NoteValue><PrimaryTuplet num="3" den="2" /></Rhythm><Rhythm id="2"><NoteValue>Half</NoteValue><AugmentationDot count="1" /></Rhythm></Rhythms>
</GPIF>"#;

    fn archive(gpif: &str) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("VERSION", options).unwrap();
            zip.write_all(b"7.0").unwrap();
            zip.start_file(GPIF_ENTRY, options).unwrap();
            zip.write_all(gpif.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        buffer.into_inner()
    }

    #[test]
    fn gp7_archive_imports_tracks_tab_and_techniques() {
        let report = parse_gp_with_report(&archive(GPIF)).expect("GP7 archive parses");
        let score = &report.score;
        assert_eq!(score.metadata.title, "Riff & Roll");
        assert_eq!(score.metadata.composer, "Composer");
        assert_eq!(score.settings.tempo_bpm, 96);
        assert_eq!(score.settings.key_signature.fifths, 1);
        assert_eq!(score.settings.key_signature.mode, "minor");
        let part = &score.parts[0];
        assert_eq!(
            (part.name.as_str(), part.midi_program, part.midi_channel),
            ("Lead", 29, 3)
        );
        let staff = &part.staves[0];
        let tab = staff.tablature.as_ref().expect("tuning becomes tablature");
        assert_eq!(tab.tuning_midi, vec![38, 45, 50, 55, 59, 64]);
        assert_eq!(tab.capo, 2);
        let first = &staff.measures[0];
        assert_eq!(first.barline_left, Barline::RepeatStart);
        assert_eq!(first.rehearsal.as_deref(), Some("A Intro"));
        let voice = &first.voices[0];
        // Chord: A2 (string 1 = lowest) + G3.
        assert_eq!(
            voice[0]
                .pitches
                .iter()
                .map(Pitch::to_midi)
                .collect::<Vec<_>>(),
            vec![45, 55]
        );
        assert_eq!(
            voice[0].tab_position,
            Some(TabPosition { string: 1, fret: 7 })
        );
        assert_eq!(voice[0].tab_positions.len(), 2);
        assert!(voice[0].tie_start);
        assert_eq!(voice[0].technique_text.as_deref(), Some("P.M."));
        assert_eq!(voice[0].dynamic, Some(Dynamic::Mf));
        assert_eq!(
            voice[0].lyric.as_ref().map(|l| l.text.as_str()),
            Some("Hey")
        );
        // F#4 spelled from ConcertPitch; hammer toward E4 is a pull-off.
        assert_eq!(voice[1].pitches[0].step, Step::F);
        assert_eq!(voice[1].pitches[0].alter, 1);
        assert_eq!(voice[1].guitar_technique, Some(GuitarTechnique::PullOff));
        assert!(voice[1].articulations.contains(&Articulation::Accent));
        assert!(voice[1].articulations.contains(&Articulation::Tremolo(2)));
        assert!(voice[1].articulations.contains(&Articulation::UpBow));
        assert_eq!(voice[1].arpeggiate, Some(true));
        assert_eq!(voice[1].dynamic, None);
        assert_eq!(voice[2].guitar_technique, Some(GuitarTechnique::Bend));
        assert_eq!(voice[2].guitar_bend_alter_cents, Some(200));
        assert!(voice[3].is_rest);
        assert!(voice[3].tuplet.is_some());
        // The incomplete bar is completed with rests so it validates.
        assert!(
            acorde_core::validate(score).errors.is_empty(),
            "{:?}",
            acorde_core::validate(score).errors
        );
        let second = &staff.measures[1];
        // A bar of only a placeholder rest beat is a measure rest, not a padded quarter rest.
        let third = &staff.measures[2].voices[0];
        assert_eq!(third.len(), 1);
        assert!(third[0].is_plain_whole_rest());
        assert_eq!(second.time_sig.as_ref().map(|t| t.numerator), Some(3));
        assert_eq!(second.barline_right, Barline::RepeatEnd);
        assert_eq!(second.voices[0][0].note_head, NoteHead::X);
        assert_eq!(second.voices[0][0].fingering, Some(2));
        assert_eq!(second.voices[0][0].dot_count, 1);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "gp.unsupported-whammy")
        );
        // Only the A of the first chord is tied: the widening is reported, not silent.
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "gp.partial-chord-tie")
        );
    }

    #[test]
    fn non_gp7_input_is_rejected_cleanly() {
        assert!(parse_gp(b"not a zip").is_err());
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            zip.start_file("score.gpx", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"BCFZ").unwrap();
            zip.finish().unwrap();
        }
        let error = parse_gp(&buffer.into_inner()).unwrap_err();
        assert!(error.to_string().contains("Guitar Pro 6/7/8"));
        assert!(parse_gpif("<?xml version=\"1.0\"?><!DOCTYPE GPIF><GPIF/>").is_err());
    }
}
