//! Guitar Pro 3/4/5 (`.gp3`, `.gp4`, `.gp5`) binary import.
//!
//! These files are a little-endian stream: a version string, song information, MIDI channel
//! table, master-bar headers, tracks, then every bar of every track beat by beat. Nothing is
//! addressed by offset, so every field must be consumed in order even when acorde does not keep
//! it; unknown or corrupt input stops with an error instead of drifting. The result goes through
//! the same measure completion, hammer/pull resolution and `gp.*` diagnostics as GPIF files.

use super::{
    HOPO_MARK, Losses, finish_bend_curve, finish_measures, gp_dynamic, push_articulation,
    resolve_hammer_pull,
};
use crate::{Diagnostic, Error};
use acorde_core::{
    Articulation, Barline, Clef, Duration, GuitarBendPoint, GuitarTechnique, KeySignature, Lyric,
    Measure, Note, NoteHead, Part, Pitch, Score, Staff, StaffKind, StyledText, TabPosition,
    TablatureConfig, TextStyle, TimeSignature, TupletInfo, VoltaBracket,
};
use std::collections::BTreeMap;

const VERSION_PREFIX: &[u8] = b"FICHIER GUITAR PRO ";
const MAX_BARS: usize = 10_000;
const MAX_TRACKS: usize = 100;
const MAX_BEATS: usize = 1_000;
const MAX_POINTS: usize = 1_000;
const MAX_STRING: usize = 1 << 20;
const MAX_NOTICE_LINES: usize = 1_000;

/// Whether `data` starts like a Guitar Pro 3/4/5 file (a length-prefixed "FICHIER GUITAR PRO").
pub(super) fn is_gp5_family(data: &[u8]) -> bool {
    data.len() > VERSION_PREFIX.len() && data[1..].starts_with(VERSION_PREFIX)
}

struct Bin<'a> {
    data: &'a [u8],
    pos: usize,
}

fn truncated() -> Error {
    Error::Xml("truncated or corrupt Guitar Pro 3/4/5 file".into())
}

impl<'a> Bin<'a> {
    fn bytes(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(count).ok_or_else(truncated)?;
        let slice = self.data.get(self.pos..end).ok_or_else(truncated)?;
        self.pos = end;
        Ok(slice)
    }
    fn skip(&mut self, count: usize) -> Result<(), Error> {
        self.bytes(count).map(|_| ())
    }
    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.bytes(1)?[0])
    }
    fn i8(&mut self) -> Result<i8, Error> {
        Ok(self.u8()? as i8)
    }
    fn bool(&mut self) -> Result<bool, Error> {
        Ok(self.u8()? != 0)
    }
    fn i16(&mut self) -> Result<i16, Error> {
        let b = self.bytes(2)?;
        Ok(i16::from_le_bytes([b[0], b[1]]))
    }
    fn i32(&mut self) -> Result<i32, Error> {
        let b = self.bytes(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn count(&mut self, max: usize, what: &str) -> Result<usize, Error> {
        let value = self.i32()?;
        usize::try_from(value)
            .ok()
            .filter(|value| *value <= max)
            .ok_or_else(|| Error::Xml(format!("Guitar Pro {what} count {value} is out of range")))
    }
    fn text(&mut self, length: usize) -> Result<String, Error> {
        if length > MAX_STRING {
            return Err(truncated());
        }
        Ok(decode_ansi(self.bytes(length)?))
    }
    /// One length byte, then a fixed-width field of `width` bytes.
    fn fixed_string(&mut self, width: usize) -> Result<String, Error> {
        let length = usize::from(self.u8()?);
        let field = self.bytes(width)?;
        Ok(decode_ansi(&field[..length.min(width)]))
    }
    /// A four-byte field size (unused), then a length byte and the text.
    fn int_byte_string_unused(&mut self) -> Result<String, Error> {
        self.skip(4)?;
        let length = usize::from(self.u8()?);
        self.text(length)
    }
    /// A four-byte length, then the text.
    fn int_string(&mut self) -> Result<String, Error> {
        let length = usize::try_from(self.i32()?).map_err(|_| truncated())?;
        self.text(length)
    }
    /// A four-byte size, a length byte, then the text.
    fn int_byte_string(&mut self) -> Result<String, Error> {
        let size = self.i32()?;
        self.u8()?;
        let length = usize::try_from(size.saturating_sub(1)).unwrap_or(0);
        self.text(length)
    }
}

/// Guitar Pro 3–5 strings are ANSI (Windows-1252) text.
fn decode_ansi(bytes: &[u8]) -> String {
    const CP1252: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
        '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
        'ž', 'Ÿ',
    ];
    bytes
        .iter()
        .take_while(|byte| **byte != 0)
        .map(|&byte| match byte {
            0x80..=0x9f => CP1252[usize::from(byte - 0x80)],
            _ => char::from(byte),
        })
        .collect::<String>()
        .trim()
        .to_string()
}

#[derive(Clone, Default)]
struct Header {
    numerator: u8,
    denominator: u8,
    repeat_start: bool,
    repeat_end: bool,
    endings: u8,
    marker: Option<String>,
    key: Option<(i8, bool)>,
    double_bar: bool,
    navigation: Vec<&'static str>,
}

struct Track {
    name: String,
    /// String tunings, highest string first as stored in the file.
    tuning: Vec<i16>,
    capo: u8,
    percussion: bool,
    tablature: bool,
    bass_clef: bool,
    program: u8,
    channel: u8,
}

/// Per-track, per-voice reading state across bars.
#[derive(Default)]
struct VoiceState {
    /// Last sounding fret on each string (file order), for tie destinations.
    frets: [Option<u8>; 7],
    /// Where the previous beat of this voice landed: (measure index, note index).
    previous: Option<(usize, usize)>,
}

struct Reader<'a> {
    bin: Bin<'a>,
    version: u16,
    losses: Losses,
}

/// Parse a Guitar Pro 3/4/5 file into a score and the diagnostics for what the model cannot hold.
pub(super) fn parse_gp5(data: &[u8]) -> Result<(Score, Vec<Diagnostic>), Error> {
    let mut reader = Reader {
        bin: Bin { data, pos: 0 },
        version: 0,
        losses: Losses::default(),
    };
    let score = reader.read_score()?;
    Ok((score, reader.losses.into_diagnostics()))
}

const DIRECTIONS: [Option<&str>; 19] = [
    Some("Coda"),
    None, // double coda
    Some("Segno"),
    None, // segno segno
    Some("Fine"),
    Some("DaCapo"),
    Some("DaCapoAlCoda"),
    None, // da capo al double coda
    Some("DaCapoAlFine"),
    Some("DalSegno"),
    Some("DalSegnoAlCoda"),
    None, // dal segno al double coda
    Some("DalSegnoAlFine"),
    None, // dal segno segno
    None,
    None,
    None,
    Some("ToCoda"),
    None, // da double coda
];

impl Reader<'_> {
    fn read_score(&mut self) -> Result<Score, Error> {
        let version = self.bin.fixed_string(30)?;
        let number = version
            .strip_prefix("FICHIER GUITAR PRO v")
            .and_then(|rest| rest.split_once('.'))
            .and_then(|(major, minor)| {
                let minor: String = minor.chars().take_while(char::is_ascii_digit).collect();
                Some(major.parse::<u16>().ok()? * 100 + minor.parse::<u16>().ok()?)
            })
            .filter(|number| (300..600).contains(number))
            .ok_or_else(|| Error::Xml(format!("unsupported Guitar Pro version \"{version}\"")))?;
        self.version = number;
        let v = number;

        let mut score = Score::default();
        score.parts.clear();
        let title = self.bin.int_byte_string_unused()?;
        let subtitle = self.bin.int_byte_string_unused()?;
        let artist = self.bin.int_byte_string_unused()?;
        let _album = self.bin.int_byte_string_unused()?;
        let words = self.bin.int_byte_string_unused()?;
        let music = if v >= 500 {
            self.bin.int_byte_string_unused()?
        } else {
            words.clone()
        };
        let copyright = self.bin.int_byte_string_unused()?;
        let _tab = self.bin.int_byte_string_unused()?;
        let _instructions = self.bin.int_byte_string_unused()?;
        let notices = self.bin.count(MAX_NOTICE_LINES, "notice line")?;
        for _ in 0..notices {
            self.bin.int_byte_string_unused()?;
        }
        if !title.is_empty() {
            score.metadata.title = title;
        }
        score.metadata.movement_title = subtitle;
        score.metadata.composer = if music.is_empty() { artist } else { music };
        score.metadata.lyricist = words;
        score.metadata.copyright = copyright;

        if v < 500 && self.bin.bool()? {
            self.losses.add(
                "gp.triplet-feel",
                "triplet feel (swing) is not imported; notes keep their written rhythm",
            );
        }
        let mut lyrics_track = None;
        let mut lyric_lines = Vec::new();
        if v >= 400 {
            lyrics_track = usize::try_from(self.bin.i32()? - 1).ok();
            for _ in 0..5 {
                let start = self.bin.i32()?;
                let text = self.bin.int_string()?;
                lyric_lines.push((usize::try_from(start - 1).unwrap_or(0), text));
            }
        }
        if v >= 510 {
            self.bin.skip(19)?; // RSE master volume, effect and equalizer
        }
        if v >= 500 {
            self.bin.skip(28)?; // page size and margins
            self.bin.i16()?; // header/footer visibility flags
            for _ in 0..10 {
                self.bin.int_byte_string()?; // header/footer templates
            }
            self.bin.int_byte_string()?; // tempo text
        }
        let tempo = self.bin.i32()?;
        if v >= 510 {
            self.bin.bool()?; // hide tempo
        }
        self.bin.i32()?; // global key signature (every bar also carries its own)
        if v >= 400 {
            self.bin.u8()?; // octave
        }
        // MIDI channel table: 4 ports × 16 channels.
        let mut programs = [[0u8; 16]; 4];
        for port in &mut programs {
            for program in port.iter_mut() {
                *program = u8::try_from(self.bin.i32()?)
                    .ok()
                    .filter(|p| *p < 128)
                    .unwrap_or(0);
                self.bin.skip(6)?; // volume, balance, chorus, reverb, phaser, tremolo
                self.bin.skip(2)?; // blank
            }
        }
        let mut directions: BTreeMap<usize, Vec<&'static str>> = BTreeMap::new();
        if v >= 500 {
            for direction in DIRECTIONS {
                let bar = self.bin.i16()?;
                if bar <= 0 {
                    continue;
                }
                match direction {
                    Some(direction) => directions
                        .entry(usize::try_from(bar - 1).unwrap_or(0))
                        .or_default()
                        .push(direction),
                    None => self.losses.add(
                        "gp.unsupported-direction",
                        "double coda and segno-segno directions are not imported",
                    ),
                }
            }
            self.bin.skip(4)?;
        }
        let bar_count = self.bin.count(MAX_BARS, "bar")?;
        let track_count = self.bin.count(MAX_TRACKS, "track")?;
        if bar_count == 0 || track_count == 0 {
            return Err(Error::Empty);
        }

        let mut headers: Vec<Header> = Vec::with_capacity(bar_count);
        for index in 0..bar_count {
            let previous = headers.last().cloned();
            let mut header = self.read_header(previous.as_ref())?;
            header.navigation = directions.remove(&index).unwrap_or_default();
            headers.push(header);
        }
        let mut tracks = Vec::with_capacity(track_count);
        for _ in 0..track_count {
            tracks.push(self.read_track(&programs)?);
        }

        // Bars, interleaved: every track's bar for master bar 0, then master bar 1, …
        let mut measures: Vec<Vec<Measure>> = vec![Vec::with_capacity(bar_count); track_count];
        let mut states: Vec<[VoiceState; 2]> =
            (0..track_count).map(|_| Default::default()).collect();
        let mut tempos: BTreeMap<usize, u16> = BTreeMap::new();
        let mut last_dynamics = vec![6_i8; track_count];
        let mut key_fifths = 0_i8;
        for (bar_index, header) in headers.iter().enumerate() {
            if let Some((fifths, _)) = header.key {
                key_fifths = fifths;
            }
            for track_index in 0..track_count {
                let mut measure = Measure::empty(header.numerator, header.denominator);
                measure.number = (bar_index + 1) as u32;
                measure.voices = [vec![], vec![], vec![], vec![]];
                measures[track_index].push(measure);
                let voice_count = if v >= 500 {
                    self.bin.u8()?;
                    2
                } else {
                    1
                };
                for (voice_index, state) in
                    states[track_index].iter_mut().take(voice_count).enumerate()
                {
                    let beats = self.bin.count(MAX_BEATS, "beat")?;
                    for _ in 0..beats {
                        self.read_beat(
                            &tracks[track_index],
                            &mut measures[track_index],
                            bar_index,
                            voice_index,
                            state,
                            &mut last_dynamics[track_index],
                            &mut tempos,
                            key_fifths,
                        )?;
                    }
                }
            }
        }

        self.assemble(
            &mut score,
            &headers,
            tracks,
            measures,
            &tempos,
            tempo,
            lyrics_track,
            lyric_lines,
        );
        finish_measures(&mut score, false);
        resolve_hammer_pull(&mut score);
        super::resolve_beat_ottavas(&mut score);
        Ok(score)
    }

    fn read_header(&mut self, previous: Option<&Header>) -> Result<Header, Error> {
        let v = self.version;
        let flags = self.bin.u8()?;
        let mut header = Header {
            numerator: previous.map_or(4, |p| p.numerator),
            denominator: previous.map_or(4, |p| p.denominator),
            ..Header::default()
        };
        if flags & 0x01 != 0 {
            header.numerator = self.bin.u8()?;
        }
        if flags & 0x02 != 0 {
            header.denominator = self.bin.u8()?;
        }
        if header.numerator == 0 || header.denominator == 0 || !header.denominator.is_power_of_two()
        {
            header.numerator = 4;
            header.denominator = 4;
        }
        header.repeat_start = flags & 0x04 != 0;
        if flags & 0x08 != 0 {
            header.repeat_end = true;
            let count = self.bin.u8()?;
            if count > 2 {
                self.losses.add(
                    "gp.repeat-count",
                    "repeats played more than twice keep only the end-repeat barline",
                );
            }
        }
        if flags & 0x10 != 0 && v < 500 {
            header.endings = self.bin.u8()?;
        }
        if flags & 0x20 != 0 {
            let text = self.bin.int_byte_string()?;
            self.bin.skip(4)?; // colour
            header.marker = (!text.is_empty()).then_some(text);
        }
        if flags & 0x40 != 0 {
            let fifths = self.bin.i8()?;
            let minor = self.bin.u8()? != 0;
            header.key = Some((fifths.clamp(-7, 7), minor));
        }
        if v >= 500 {
            if flags & 0x03 != 0 {
                self.bin.skip(4)?; // beam grouping
            }
            let mask = self.bin.u8()?;
            // GP5 stores the endings as a bit mask; acorde keeps the lowest pass number.
            if mask != 0 {
                if mask.count_ones() > 1 {
                    self.losses.add(
                        "gp.multi-number-ending",
                        "an ending shared by several passes keeps only its first number",
                    );
                }
                header.endings = mask.trailing_zeros() as u8 + 1;
            }
            if self.bin.u8()? != 0 {
                self.losses.add(
                    "gp.triplet-feel",
                    "triplet feel (swing) is not imported; notes keep their written rhythm",
                );
            }
            self.bin.u8()?;
        }
        header.double_bar = flags & 0x80 != 0;
        Ok(header)
    }

    fn read_track(&mut self, programs: &[[u8; 16]; 4]) -> Result<Track, Error> {
        let v = self.version;
        let flags = self.bin.u8()?;
        let name = self.bin.fixed_string(40)?;
        let strings = self.bin.count(7, "string")?;
        let mut tuning = Vec::with_capacity(strings);
        for index in 0..7 {
            let pitch = self.bin.i32()?;
            if index < strings {
                tuning.push(i16::try_from(pitch.clamp(0, 127)).unwrap_or(0));
            }
        }
        let port = usize::try_from(self.bin.i32()? - 1).unwrap_or(0).min(3);
        let channel = usize::try_from(self.bin.i32()? - 1).unwrap_or(0).min(15);
        self.bin.i32()?; // effect channel
        self.bin.i32()?; // fret count
        let capo = u8::try_from(self.bin.i32()?.clamp(0, 24)).unwrap_or(0);
        self.bin.skip(4)?; // colour
        let percussion = flags & 0x01 != 0;
        let mut tablature = !percussion && !tuning.is_empty();
        let mut bass_clef = tuning.last().is_some_and(|low| *low < 35);
        if v >= 500 {
            let staff_flags = self.bin.u8()?;
            tablature &= staff_flags & 0x01 != 0;
            self.bin.skip(4)?; // MIDI automatics, RSE accentuation, bank, humanize
            bass_clef |= self.bin.i32()? == 12;
            self.bin.skip(4 + 4 + 10 + 1 + 1)?;
            self.bin.skip(16)?; // RSE instrument, style, soundbank, unknown
            if v >= 510 {
                self.bin.skip(4)?; // RSE equalizer
                self.bin.int_byte_string()?; // effect name
                self.bin.int_byte_string()?; // effect category
            }
        }
        if percussion {
            self.losses.add(
                "gp.drum-kit",
                "drum-kit tracks import as notes at each articulation's MIDI key, without a kit map",
            );
        }
        Ok(Track {
            name,
            tuning,
            capo,
            percussion,
            tablature,
            bass_clef,
            program: programs[port][channel],
            channel: channel as u8,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn read_beat(
        &mut self,
        track: &Track,
        measures: &mut [Measure],
        bar_index: usize,
        voice_index: usize,
        state: &mut VoiceState,
        last_dynamic: &mut i8,
        tempos: &mut BTreeMap<usize, u16>,
        key_fifths: i8,
    ) -> Result<(), Error> {
        let v = self.version;
        let flags = self.bin.u8()?;
        let dotted = flags & 0x01 != 0;
        let mut empty = false;
        if flags & 0x40 != 0 {
            empty = self.bin.u8()? & 0x02 == 0;
        }
        let duration = match self.bin.i8()? {
            -2 => Duration::Whole,
            -1 => Duration::Half,
            1 => Duration::Eighth,
            2 => Duration::Sixteenth,
            3 => Duration::ThirtySecond,
            4 => Duration::SixtyFourth,
            _ => Duration::Quarter,
        };
        let mut tuplet = None;
        if flags & 0x20 != 0 {
            let actual = self.bin.i32()?;
            let normal = match actual {
                3 => 2,
                5..=7 => 4,
                9..=13 => 8,
                _ => 0,
            };
            if normal > 0 {
                tuplet = Some(TupletInfo {
                    actual_notes: actual as u8,
                    normal_notes: normal,
                });
            }
        }
        let chord_name = if flags & 0x02 != 0 {
            self.read_chord_name()?
        } else {
            None
        };
        let text = if flags & 0x04 != 0 {
            Some(self.bin.int_byte_string_unused()?)
        } else {
            None
        };
        let mut effects = BeatEffects::default();
        if flags & 0x08 != 0 {
            effects = self.read_beat_effects()?;
        }
        if flags & 0x10 != 0 {
            if let Some(bpm) = self.read_mix_table()? {
                tempos.entry(bar_index).or_insert(bpm);
            }
        }
        let string_flags = self.bin.u8()?;
        let mut members: Vec<(u8, Pitch, Option<TabPosition>, NoteRead)> = Vec::new();
        for bit in (0..=6).rev() {
            if string_flags & (1 << bit) == 0 {
                continue;
            }
            let string_index = 6 - bit;
            let read = self.read_note()?;
            if string_index >= track.tuning.len() {
                continue;
            }
            let mut fret = read.fret;
            if read.tie
                && let Some(previous) = state.frets[string_index]
            {
                fret = previous;
            }
            state.frets[string_index] = Some(fret);
            let (midi, tab) = if track.percussion {
                let key = match fret {
                    27 => 42,
                    28 => 60,
                    32 => 31,
                    other => other,
                };
                (key, None)
            } else {
                let midi = i16::from(fret) + track.tuning[string_index] + i16::from(track.capo);
                let tab = track.tablature.then(|| TabPosition {
                    string: (track.tuning.len() - string_index) as u8,
                    fret,
                });
                (u8::try_from(midi.clamp(0, 127)).unwrap_or(0), tab)
            };
            let prefer_flat = (key_fifths < 0) != read.swap_accidental;
            members.push((
                midi,
                Pitch::from_midi(midi.min(127), prefer_flat),
                tab,
                read,
            ));
        }
        let mut beat_ottava = None;
        if v >= 500 {
            let flags2 = self.bin.i16()?;
            if flags2 & 0x800 != 0 {
                self.bin.u8()?;
            }
            beat_ottava = if flags2 & 0x10 != 0 {
                Some(acorde_core::OttavaKind::Va8)
            } else if flags2 & 0x20 != 0 {
                Some(acorde_core::OttavaKind::Vb8)
            } else if flags2 & 0x40 != 0 {
                Some(acorde_core::OttavaKind::Ma15)
            } else if flags2 & 0x100 != 0 {
                Some(acorde_core::OttavaKind::Mb15)
            } else {
                None
            };
        }
        if empty {
            return Ok(());
        }

        let measure_index = bar_index;
        if let Some(text) = text.filter(|text| !text.is_empty()) {
            let measure = &mut measures[measure_index];
            if !measure.texts.iter().any(|existing| existing.text == text) {
                measure.texts.push(StyledText {
                    style: TextStyle::Generic,
                    text,
                    placement: Some("above".to_string()),
                    offset_x: None,
                    offset_y: None,
                    relative_x: None,
                    relative_y: None,
                });
            }
        }

        members.sort_by_key(|member| member.0);
        let mut graces: Vec<(Pitch, Option<TabPosition>, Grace)> = Vec::new();
        let mut note = if members.is_empty() {
            Note::rest(duration.clone())
        } else {
            let mut note = Note::new(members[0].1.clone(), duration.clone());
            note.pitches.clear();
            note
        };
        note.dot_count = u8::from(dotted);
        note.tuplet = tuplet;
        note.ottava_start = beat_ottava;
        if let Some(name) = chord_name {
            match crate::chord_label::parse_chord_label(&name) {
                Some(mut chord) => {
                    chord.placement = Some("above".to_string());
                    note.chord_symbol = Some(chord);
                }
                None => self.losses.add(
                    "gp.unsupported-chord-name",
                    "a chord name that is not a chord symbol (root and kind) is not imported",
                ),
            }
        }
        let mut ties: Vec<bool> = Vec::new();
        let mut dynamic = None;
        let all_emphasised = members.iter().all(|member| member.3.emphasised);
        for (midi, pitch, tab, read) in members {
            note.pitches.push(pitch);
            if read.ghost {
                let member = note.pitches.len() - 1;
                note.set_parenthesized(member, true);
            }
            if let Some(tab) = &tab {
                if note.tab_position.is_none() {
                    note.tab_position = Some(tab.clone());
                }
                note.tab_positions.push(tab.clone());
            }
            if track.percussion {
                note.is_unpitched = true;
            }
            ties.push(read.tie);
            if read.dynamic.is_some() {
                dynamic = read.dynamic;
            }
            if let Some(grace) = read.grace.clone() {
                let fret_offset = i16::from(grace.fret) - i16::from(read.fret);
                let grace_midi = (i16::from(midi) + fret_offset).clamp(0, 127) as u8;
                let grace_tab = tab.map(|tab| TabPosition {
                    string: tab.string,
                    fret: grace.fret,
                });
                graces.push((
                    Pitch::from_midi(grace_midi, key_fifths < 0),
                    grace_tab,
                    grace,
                ));
            }
            self.apply_note(&mut note, &read, &effects);
        }
        if ties.iter().any(|tie| *tie) {
            // Ties are per string: end them on this chord's tied notes and start them on the
            // matching notes (same string, or same pitch off tablature) of the previous beat.
            let ends = ties.clone();
            let starts = note.pitch_tie_starts_or_uniform();
            note.set_pitch_ties(&starts, &ends);
            if let Some((m, n)) = state.previous
                && let Some(previous) = measures[m].voices[voice_index].get_mut(n)
                && !previous.is_rest
            {
                let mut previous_starts = previous.pitch_tie_starts_or_uniform();
                let previous_ends = previous.pitch_tie_ends_or_uniform();
                for (index, tied) in ties.iter().enumerate() {
                    if !tied {
                        continue;
                    }
                    let target = match note.tab_positions.get(index) {
                        Some(tab) => previous
                            .tab_positions
                            .iter()
                            .position(|previous| previous.string == tab.string),
                        None => previous
                            .pitches
                            .iter()
                            .position(|pitch| pitch.to_midi() == note.pitches[index].to_midi()),
                    };
                    if let Some(target) = target.filter(|target| *target < previous_starts.len()) {
                        previous_starts[target] = true;
                    }
                }
                previous.set_pitch_ties(&previous_starts, &previous_ends);
            }
        }
        if !note.is_rest {
            effects.apply(&mut note, &mut self.losses);
            // A beat of only emphasised notes keeps the running dynamic (see `read_note`).
            let value = dynamic.unwrap_or(if all_emphasised { *last_dynamic } else { 6 });
            if value != *last_dynamic {
                note.dynamic = gp_dynamic(dynamic_name(value));
                *last_dynamic = value;
            }
        }
        if !graces.is_empty() {
            let mut grace_note = Note::rest(Duration::ThirtySecond);
            grace_note.is_rest = false;
            grace_note.is_grace = true;
            grace_note.grace_slash = true;
            for (pitch, tab, grace) in &graces {
                grace_note.pitches.push(pitch.clone());
                if let Some(tab) = tab {
                    if grace_note.tab_position.is_none() {
                        grace_note.tab_position = Some(tab.clone());
                    }
                    grace_note.tab_positions.push(tab.clone());
                }
                match grace.transition {
                    1 => grace_note.guitar_technique = Some(GuitarTechnique::Slide),
                    3 => {
                        grace_note.guitar_technique = Some(GuitarTechnique::HammerOn);
                        grace_note.technique_text = Some(HOPO_MARK.to_string());
                    }
                    _ => {}
                }
                if grace.dead {
                    grace_note.note_head = NoteHead::X;
                }
                if grace.on_beat {
                    self.losses.add(
                        "gp.on-beat-grace",
                        "on-beat grace notes import as ordinary (before-beat) grace notes",
                    );
                }
            }
            if track.percussion {
                grace_note.is_unpitched = true;
            }
            measures[measure_index].voices[voice_index].push(grace_note);
        }
        let voice = &mut measures[measure_index].voices[voice_index];
        state.previous = Some((measure_index, voice.len()));
        voice.push(note);
        Ok(())
    }

    /// A beat's chord: its name is kept (it becomes the note's chord symbol); the diagram is
    /// skipped.
    fn read_chord_name(&mut self) -> Result<Option<String>, Error> {
        let v = self.version;
        self.losses.add(
            "gp.unsupported-chord-diagram",
            "chord diagrams (fret grids) attached to beats are not imported; their names are",
        );
        let name = if v >= 500 {
            self.bin.skip(17)?;
            let name = self.bin.fixed_string(21)?;
            self.bin.skip(4)?;
            self.bin.skip(4 + 7 * 4)?;
            self.bin.skip(1 + 5 + 26)?;
            name
        } else if self.bin.u8()? != 0 {
            if v >= 400 {
                self.bin.skip(16)?;
                let name = self.bin.fixed_string(21)?;
                self.bin.skip(4)?;
                self.bin.skip(4 + 7 * 4)?;
                self.bin.skip(1 + 5 + 26)?;
                name
            } else {
                self.bin.skip(25)?;
                let name = self.bin.fixed_string(34)?;
                self.bin.skip(4 + 6 * 4)?;
                self.bin.skip(36)?;
                name
            }
        } else {
            let strings = if v >= 406 { 7 } else { 6 };
            let name = self.bin.int_byte_string()?;
            if self.bin.i32()? > 0 {
                self.bin.skip(strings * 4)?;
            }
            name
        };
        let name = name.trim().to_string();
        Ok((!name.is_empty()).then_some(name))
    }

    fn read_beat_effects(&mut self) -> Result<BeatEffects, Error> {
        let v = self.version;
        let flags = self.bin.u8()?;
        let flags2 = if v >= 400 { self.bin.u8()? } else { 0 };
        let mut effects = BeatEffects::default();
        if flags & 0x10 != 0 {
            self.losses.add(
                "gp.unsupported-volume-swell",
                "fade-in/volume swells are not imported",
            );
        }
        effects.vibrato = (v < 400 && flags & 0x01 != 0) || flags & 0x02 != 0;
        if flags2 & 0x01 != 0 {
            self.losses
                .add("gp.unsupported-rasgueado", "rasgueado is not imported");
        }
        if flags & 0x20 != 0 {
            let kind = self.bin.i8()?;
            if v < 400 {
                self.bin.skip(4)?;
                if kind == 0 {
                    self.losses
                        .add("gp.unsupported-whammy", "whammy-bar dives are not imported");
                }
            }
            // 1 tapping, 2 slapping, 3 popping.
            match kind {
                1 => effects.tap = true,
                2 | 3 => self.losses.add(
                    "gp.unsupported-tapping",
                    "slap and pop marks are not imported (tapping is)",
                ),
                _ => {}
            }
        }
        if flags2 & 0x04 != 0 {
            self.skip_bend()?;
            self.losses
                .add("gp.unsupported-whammy", "whammy-bar dives are not imported");
        }
        if flags & 0x40 != 0 {
            let (up, down) = if v < 500 {
                let down = self.bin.u8()?;
                (self.bin.u8()?, down)
            } else {
                let up = self.bin.u8()?;
                (up, self.bin.u8()?)
            };
            if up > 0 {
                effects.brush_down = Some(false);
            } else if down > 0 {
                effects.brush_down = Some(true);
            }
        }
        if flags2 & 0x02 != 0 {
            effects.pick_up = match self.bin.i8()? {
                1 => Some(true),
                2 => Some(false),
                _ => None,
            };
        }
        if v < 400 {
            effects.natural_harmonic = flags & 0x04 != 0;
            effects.artificial_harmonic = flags & 0x08 != 0;
        }
        Ok(effects)
    }

    fn skip_bend(&mut self) -> Result<(), Error> {
        self.bin.u8()?;
        self.bin.i32()?;
        let points = self.bin.count(MAX_POINTS, "bend point")?;
        self.bin.skip(points * 9)
    }

    /// Mix-table change; returns a tempo change when it carries one.
    fn read_mix_table(&mut self) -> Result<Option<u16>, Error> {
        let v = self.version;
        let instrument = self.bin.i8()?;
        if v >= 500 {
            self.bin.skip(16)?;
        }
        let mut values = [0_i8; 6];
        for value in &mut values {
            *value = self.bin.i8()?;
        }
        if v >= 500 {
            self.bin.int_byte_string()?;
        }
        let tempo = self.bin.i32()?;
        for value in values {
            if value >= 0 {
                self.bin.u8()?;
            }
        }
        if tempo >= 0 {
            self.bin.i8()?;
            if v >= 510 {
                self.bin.u8()?;
            }
        }
        if v >= 400 {
            self.bin.u8()?;
        }
        if v >= 500 && self.bin.i8()? >= 0 {
            self.losses
                .add("gp.unsupported-wah", "wah pedal marks are not imported");
        }
        if v >= 510 {
            self.bin.int_byte_string()?;
            self.bin.int_byte_string()?;
        }
        if instrument >= 0 || values.iter().any(|value| *value >= 0) {
            self.losses.add(
                "gp.unsupported-mix-change",
                "mid-score instrument, volume, pan and effect changes are not imported",
            );
        }
        Ok(u16::try_from(tempo)
            .ok()
            .filter(|bpm| (1..=999).contains(bpm)))
    }

    fn read_note(&mut self) -> Result<NoteRead, Error> {
        let v = self.version;
        let flags = self.bin.u8()?;
        let mut read = NoteRead {
            heavy_accent: flags & 0x02 != 0,
            accent: flags & 0x40 != 0,
            ..NoteRead::default()
        };
        read.ghost = flags & 0x04 != 0;
        if flags & 0x20 != 0 {
            match self.bin.u8()? {
                2 => read.tie = true,
                3 => read.dead = true,
                _ => {}
            }
        }
        if flags & 0x01 != 0 && v < 500 {
            self.bin.skip(2)?; // independent duration and tuplet
        }
        if flags & 0x10 != 0 {
            let dynamic = self.bin.i8()?.clamp(1, 8);
            // Guitar Pro 5 folds ghost and accent emphasis into the stored velocity (a ghost note
            // reads mp, an accent ff, a heavy accent fff); that is playback, not a dynamic mark.
            if flags & (0x02 | 0x04 | 0x40) == 0 {
                read.dynamic = Some(dynamic);
            }
        }
        read.emphasised = flags & (0x02 | 0x04 | 0x40) != 0;
        if flags & 0x20 != 0 {
            read.fret = u8::try_from(self.bin.i8()?.clamp(0, 99)).unwrap_or(0);
        }
        if flags & 0x80 != 0 {
            read.left_finger = self.bin.i8()?;
            if self.bin.i8()? >= 0 {
                self.losses.add(
                    "gp.unsupported-right-hand-fingering",
                    "right-hand (p-i-m-a-c) fingering is not imported",
                );
            }
        } else {
            read.left_finger = -1;
        }
        if v >= 500 {
            if flags & 0x01 != 0 {
                self.bin.skip(8)?; // duration percent
            }
            read.swap_accidental = self.bin.u8()? & 0x02 != 0;
        }
        if flags & 0x08 != 0 {
            self.read_note_effects(&mut read)?;
        }
        Ok(read)
    }

    fn read_note_effects(&mut self, read: &mut NoteRead) -> Result<(), Error> {
        let v = self.version;
        let flags = self.bin.u8()?;
        let flags2 = if v >= 400 { self.bin.u8()? } else { 0 };
        if flags & 0x01 != 0 {
            self.bin.u8()?; // bend type
            self.bin.i32()?; // bend value
            let points = self.bin.count(MAX_POINTS, "bend point")?;
            for _ in 0..points {
                let offset = self.bin.i32()?.clamp(0, 60);
                let value = self.bin.i32()?;
                self.bin.bool()?; // vibrato
                read.bend.push(GuitarBendPoint {
                    position_per_mille: (offset * 1000 / 60) as u16,
                    // 25 per quarter tone: 50 cents per 25 units.
                    alter_cents: value.saturating_mul(2).clamp(-2400, 2400) as i16,
                });
            }
        }
        if flags & 0x10 != 0 {
            let fret = u8::try_from(self.bin.i8()?.clamp(0, 99)).unwrap_or(0);
            self.bin.i8()?; // dynamic
            let transition = self.bin.i8()?;
            self.bin.u8()?; // duration
            let (dead, on_beat) = if v >= 500 {
                let flags = self.bin.u8()?;
                (flags & 0x01 != 0, flags & 0x02 != 0)
            } else {
                (false, false)
            };
            read.grace = Some(Grace {
                fret,
                transition,
                dead,
                on_beat,
            });
        }
        if flags2 & 0x04 != 0 {
            read.tremolo = Some(self.bin.u8()?.clamp(1, 3));
        }
        if flags2 & 0x08 != 0 {
            let kind = self.bin.i8()?;
            let shift_or_legato = if v >= 500 {
                kind & 0x03 != 0
            } else {
                kind == 1 || kind == 2
            };
            if shift_or_legato {
                read.slide = true;
            }
            let other = if v >= 500 {
                kind & !0x03 != 0
            } else {
                !(0..=2).contains(&kind)
            };
            if other {
                self.losses.add(
                    "gp.unsupported-slide-type",
                    "slide-in/out and pick slides are not imported (shift and legato slides are)",
                );
            }
        } else if v < 400 && flags & 0x04 != 0 {
            read.slide = true;
        }
        if flags2 & 0x10 != 0 {
            let kind = self.bin.u8()?;
            if v >= 500 {
                match kind {
                    2 => self.bin.skip(3)?,
                    3 => self.bin.skip(1)?,
                    _ => {}
                }
            }
            read.harmonic = true;
            if kind != 1 {
                self.losses.add(
                    "gp.harmonic-type",
                    "artificial, pinch, tap and semi harmonics import as natural harmonics",
                );
            }
        }
        if flags2 & 0x20 != 0 {
            self.bin.skip(2)?;
            read.trill = true;
        }
        read.let_ring = flags & 0x08 != 0;
        read.hammer = flags & 0x02 != 0;
        read.vibrato = flags2 & 0x40 != 0;
        read.palm_mute = flags2 & 0x02 != 0;
        read.staccato = flags2 & 0x01 != 0;
        Ok(())
    }

    /// Note-level effects on the chord that holds the note (acorde keeps techniques per chord).
    fn apply_note(&mut self, note: &mut Note, read: &NoteRead, effects: &BeatEffects) {
        if read.heavy_accent {
            push_articulation(note, Articulation::Marcato);
        } else if read.accent {
            push_articulation(note, Articulation::Accent);
        }
        if read.vibrato {
            push_articulation(note, Articulation::Vibrato);
        }
        if read.staccato {
            push_articulation(note, Articulation::Staccato);
        }
        if read.dead {
            note.note_head = NoteHead::X;
        }
        if read.palm_mute {
            if read.let_ring {
                self.losses.add(
                    "gp.let-ring-with-palm-mute",
                    "a note both palm-muted and let ring keeps only the palm-mute text",
                );
            }
            note.technique_text = Some("P.M.".to_string());
        } else if read.let_ring && note.technique_text.is_none() {
            note.technique_text = Some("let ring".to_string());
        }
        if read.harmonic || effects.natural_harmonic || effects.artificial_harmonic {
            push_articulation(note, Articulation::Harmonic);
        }
        if read.trill {
            push_articulation(note, Articulation::Trill);
        }
        if let Some(marks) = read.tremolo {
            push_articulation(note, Articulation::Tremolo(marks));
        }
        if !read.bend.is_empty() && note.guitar_technique.is_none() {
            note.guitar_technique = Some(GuitarTechnique::Bend);
            note.guitar_bend_curve = read.bend.clone();
            finish_bend_curve(note);
        }
        if read.hammer && note.guitar_technique.is_none() {
            note.guitar_technique = Some(GuitarTechnique::HammerOn);
            if note.technique_text.is_none() {
                note.technique_text = Some(HOPO_MARK.to_string());
            }
        }
        if read.slide && note.guitar_technique.is_none() {
            note.guitar_technique = Some(GuitarTechnique::Slide);
        }
        if read.left_finger >= 0 {
            match read.left_finger {
                1..=4 if note.pitches.len() == 1 => note.fingering = Some(read.left_finger as u8),
                _ => self.losses.add(
                    "gp.unsupported-fingering",
                    "thumb fingering and fingering on chord members are not imported",
                ),
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn assemble(
        &mut self,
        score: &mut Score,
        headers: &[Header],
        tracks: Vec<Track>,
        mut measures: Vec<Vec<Measure>>,
        tempos: &BTreeMap<usize, u16>,
        tempo: i32,
        lyrics_track: Option<usize>,
        lyric_lines: Vec<(usize, String)>,
    ) {
        let first = &headers[0];
        score.settings.time_signature = TimeSignature {
            numerator: first.numerator,
            denominator: first.denominator,
        };
        let (fifths, minor) = first.key.unwrap_or((0, false));
        score.settings.key_signature = KeySignature {
            fifths,
            mode: if minor { "minor" } else { "major" }.to_string(),
        };
        if let Ok(bpm) = u16::try_from(tempo)
            && (1..=999).contains(&bpm)
        {
            score.settings.tempo_bpm = bpm;
        }
        let mut current_key = (fifths, minor);
        let mut previous_ending = 0;
        for (bar_index, header) in headers.iter().enumerate() {
            let time_changed = bar_index > 0
                && (headers[bar_index - 1].numerator != header.numerator
                    || headers[bar_index - 1].denominator != header.denominator);
            let key_changed = header.key.is_some_and(|key| key != current_key) && bar_index > 0;
            if let Some(key) = header.key {
                current_key = key;
            }
            let next_ending = headers.get(bar_index + 1).map_or(0, |next| next.endings);
            let volta = (header.endings > 0).then(|| {
                let starts = previous_ending != header.endings;
                let ends = next_ending != header.endings;
                VoltaBracket {
                    number: header.endings,
                    kind: match (starts, ends) {
                        (true, true) => "begin_end",
                        (true, false) => "begin",
                        (false, true) => "end",
                        (false, false) => "mid",
                    }
                    .to_string(),
                }
            });
            previous_ending = header.endings;
            if header.navigation.len() > 1 {
                self.losses.add(
                    "gp.unsupported-direction",
                    "a bar with several directions keeps only the first",
                );
            }
            for (track_index, track_measures) in measures.iter_mut().enumerate() {
                let measure = &mut track_measures[bar_index];
                if time_changed {
                    measure.time_sig = Some(TimeSignature {
                        numerator: header.numerator,
                        denominator: header.denominator,
                    });
                }
                if key_changed {
                    measure.key_sig = Some(KeySignature {
                        fifths: current_key.0,
                        mode: if current_key.1 { "minor" } else { "major" }.to_string(),
                    });
                }
                if header.repeat_start {
                    measure.barline_left = Barline::RepeatStart;
                }
                if header.repeat_end {
                    measure.barline_right = Barline::RepeatEnd;
                } else if header.double_bar {
                    measure.barline_right = Barline::Double;
                }
                measure.volta = volta.clone();
                if track_index == 0 {
                    measure.rehearsal = header.marker.clone();
                    measure.navigation = header.navigation.first().map(|nav| nav.to_string());
                    measure.tempo = tempos.get(&bar_index).copied();
                }
            }
        }
        if let Some(bpm) = measures
            .first()
            .and_then(|track| track.first())
            .and_then(|measure| measure.tempo)
        {
            score.settings.tempo_bpm = bpm;
        }
        if let Some(track_measures) = lyrics_track.and_then(|index| measures.get_mut(index)) {
            apply_lyrics(track_measures, &lyric_lines);
        }

        for (track, track_measures) in tracks.into_iter().zip(measures) {
            let mut part = Part::new(
                if track.name.is_empty() {
                    "Track"
                } else {
                    &track.name
                },
                "",
            );
            if !track.percussion {
                part.midi_program = track.program;
            }
            part.midi_channel = track.channel;
            let clef = if track.percussion {
                Clef::Percussion
            } else if track.bass_clef {
                Clef::Bass
            } else {
                Clef::Treble
            };
            let mut staff = Staff::new(clef);
            if track.tablature {
                let mut tuning = track.tuning.clone();
                tuning.reverse();
                staff.tablature = Some(TablatureConfig {
                    lines: tuning.len() as u8,
                    tuning_midi: tuning,
                    capo: track.capo,
                });
                staff.presentation.kind = StaffKind::Tablature;
                staff.presentation.lines = track.tuning.len() as u8;
            } else if track.percussion {
                staff.presentation.kind = StaffKind::Percussion;
            }
            staff.measures = track_measures;
            part.staves.push(staff);
            score.parts.push(part);
        }
    }
}

/// GP4/5 lyric lines: syllables split at spaces and hyphens, laid on the lyric track's sounding
/// notes from each line's start bar.
fn apply_lyrics(measures: &mut [Measure], lines: &[(usize, String)]) {
    for (start, text) in lines {
        let mut syllables = Vec::new();
        for word in text.split_whitespace() {
            let parts: Vec<&str> = word.split('-').filter(|part| !part.is_empty()).collect();
            let count = parts.len();
            for (index, part) in parts.into_iter().enumerate() {
                let syllabic = match (count, index) {
                    (1, _) => "single",
                    (_, 0) => "begin",
                    (_, i) if i + 1 == count => "end",
                    _ => "middle",
                };
                syllables.push(Lyric {
                    text: part.to_string(),
                    syllabic: syllabic.to_string(),
                    extend: false,
                });
            }
        }
        let mut syllables = syllables.into_iter();
        'bars: for measure in measures.iter_mut().skip(*start) {
            for note in &mut measure.voices[0] {
                if note.is_rest || note.is_grace || note.tie_end || note.lyric.is_some() {
                    continue;
                }
                let Some(syllable) = syllables.next() else {
                    break 'bars;
                };
                note.lyric = Some(syllable);
            }
        }
    }
}

fn dynamic_name(value: i8) -> &'static str {
    match value {
        1 => "PPP",
        2 => "PP",
        3 => "P",
        4 => "MP",
        5 => "MF",
        7 => "FF",
        8 => "FFF",
        _ => "F",
    }
}

#[derive(Default)]
struct BeatEffects {
    /// Brush direction: `Some(true)` for a downstroke.
    brush_down: Option<bool>,
    /// Pick stroke: `Some(true)` for an upstroke.
    pick_up: Option<bool>,
    natural_harmonic: bool,
    artificial_harmonic: bool,
    vibrato: bool,
    tap: bool,
}

impl BeatEffects {
    fn apply(&self, note: &mut Note, losses: &mut Losses) {
        if let Some(down) = self.brush_down {
            note.arpeggiate = Some(down);
        }
        match self.pick_up {
            Some(true) => push_articulation(note, Articulation::UpBow),
            Some(false) => push_articulation(note, Articulation::DownBow),
            None => {}
        }
        if self.vibrato {
            push_articulation(note, Articulation::Vibrato);
        }
        if self.tap {
            push_articulation(note, Articulation::Tap);
        }
        if self.artificial_harmonic {
            losses.add(
                "gp.harmonic-type",
                "artificial, pinch, tap and semi harmonics import as natural harmonics",
            );
        }
    }
}

#[derive(Clone)]
struct Grace {
    fret: u8,
    transition: i8,
    dead: bool,
    on_beat: bool,
}

#[derive(Default)]
struct NoteRead {
    fret: u8,
    tie: bool,
    dead: bool,
    /// A ghost note: its notehead is drawn in parentheses.
    ghost: bool,
    vibrato: bool,
    heavy_accent: bool,
    accent: bool,
    dynamic: Option<i8>,
    left_finger: i8,
    swap_accidental: bool,
    emphasised: bool,
    bend: Vec<GuitarBendPoint>,
    grace: Option<Grace>,
    tremolo: Option<u8>,
    slide: bool,
    harmonic: bool,
    trill: bool,
    let_ring: bool,
    hammer: bool,
    palm_mute: bool,
    staccato: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal hand-built Guitar Pro 3 file: one 4/4 bar, one six-string track, two beats.
    fn gp3_file() -> Vec<u8> {
        let mut data = Vec::new();
        let version = b"FICHIER GUITAR PRO v3.00";
        data.push(version.len() as u8);
        data.extend_from_slice(version);
        data.resize(31, 0);
        let int_byte_string = |data: &mut Vec<u8>, text: &str| {
            data.extend_from_slice(&(text.len() as i32 + 1).to_le_bytes());
            data.push(text.len() as u8);
            data.extend_from_slice(text.as_bytes());
        };
        for text in ["Caf\u{e9}", "", "Artist", "", "", "", "", ""] {
            if text == "Caf\u{e9}" {
                // ANSI: é is the single byte 0xE9.
                data.extend_from_slice(&5_i32.to_le_bytes());
                data.push(4);
                data.extend_from_slice(b"Caf\xe9");
            } else {
                int_byte_string(&mut data, text);
            }
        }
        data.extend_from_slice(&0_i32.to_le_bytes()); // notice lines
        data.push(0); // triplet feel
        data.extend_from_slice(&96_i32.to_le_bytes()); // tempo
        data.extend_from_slice(&0_i32.to_le_bytes()); // key
        for _ in 0..64 {
            data.extend_from_slice(&25_i32.to_le_bytes());
            data.extend_from_slice(&[0; 8]);
        }
        data.extend_from_slice(&1_i32.to_le_bytes()); // bars
        data.extend_from_slice(&1_i32.to_le_bytes()); // tracks
        // Master bar: 4/4, repeat start, marker "Intro", key of one sharp.
        data.push(0x01 | 0x02 | 0x04 | 0x20 | 0x40);
        data.extend_from_slice(&[4, 4]);
        int_byte_string(&mut data, "Intro");
        data.extend_from_slice(&[0; 4]);
        data.extend_from_slice(&[1, 0]);
        // Track.
        data.push(0);
        data.push(6);
        let mut name = b"Guitar".to_vec();
        name.resize(40, 0);
        data.extend_from_slice(&name);
        data.extend_from_slice(&6_i32.to_le_bytes());
        for pitch in [64_i32, 59, 55, 50, 45, 40, 0] {
            data.extend_from_slice(&pitch.to_le_bytes());
        }
        for value in [1_i32, 1, 2, 24, 2] {
            data.extend_from_slice(&value.to_le_bytes()); // port, channel, fx, frets, capo
        }
        data.extend_from_slice(&[0; 4]); // colour
        // Bar: two beats.
        data.extend_from_slice(&2_i32.to_le_bytes());
        // Beat 1: dotted half, a two-note chord on strings 1 and 6 (bits 6 and 1).
        data.push(0x01);
        data.push((-1_i8) as u8);
        data.push(0b0100_0010);
        for fret in [3_u8, 0] {
            data.push(0x20 | 0x08); // fret + effects
            data.push(1); // normal note
            data.push(fret);
            data.push(0x02); // hammer-on / pull-off origin
        }
        // Beat 2: a quarter on string 1, fret 1.
        data.push(0);
        data.push(0);
        data.push(0b0100_0000);
        data.push(0x20 | 0x10);
        data.push(1);
        data.push(4); // mp
        data.push(1);
        data
    }

    #[test]
    fn gp3_file_imports_bar_track_and_notes() {
        let data = gp3_file();
        assert!(is_gp5_family(&data));
        let report = crate::parse_gp_with_report(&data).expect("GP3 imports");
        let score = &report.score;
        assert_eq!(score.metadata.title, "Caf\u{e9}");
        assert_eq!(score.metadata.composer, "Artist");
        assert_eq!(score.settings.tempo_bpm, 96);
        assert_eq!(score.settings.key_signature.fifths, 1);
        let staff = &score.parts[0].staves[0];
        let tab = staff.tablature.as_ref().expect("tablature staff");
        // Stored highest string first; acorde lists tuning low to high.
        assert_eq!(tab.tuning_midi, vec![40, 45, 50, 55, 59, 64]);
        assert_eq!(tab.capo, 2);
        let measure = &staff.measures[0];
        assert_eq!(measure.barline_left, Barline::RepeatStart);
        assert_eq!(measure.rehearsal.as_deref(), Some("Intro"));
        let voice = &measure.voices[0];
        assert_eq!(voice.len(), 2);
        // Capo 2: open low E sounds F#2 (42); fret 3 on the high E sounds A4 (69).
        let midis: Vec<i16> = voice[0].pitches.iter().map(Pitch::to_midi).collect();
        assert_eq!(midis, vec![42, 69]);
        assert_eq!(voice[0].dot_count, 1);
        assert_eq!(voice[0].duration, Duration::Half);
        assert!(
            voice[0]
                .tab_positions
                .contains(&TabPosition { string: 6, fret: 3 })
        );
        assert!(
            voice[0]
                .tab_positions
                .contains(&TabPosition { string: 1, fret: 0 })
        );
        // The chord's lowest pitch leads to a higher note: a hammer-on.
        assert_eq!(voice[0].guitar_technique, Some(GuitarTechnique::HammerOn));
        assert_eq!(voice[0].technique_text, None);
        assert_eq!(voice[1].pitches[0].to_midi(), 67);
        assert_eq!(voice[1].dynamic, Some(acorde_core::Dynamic::Mp));
        assert!(acorde_core::validate(score).errors.is_empty());
    }

    #[test]
    fn truncated_and_foreign_gp3_input_is_rejected() {
        let data = gp3_file();
        for cut in [10, 40, 200, data.len() - 1] {
            assert!(crate::parse_gp(&data[..cut]).is_err(), "cut at {cut}");
        }
        let mut wrong = data.clone();
        wrong[20..24].copy_from_slice(b"v9.9");
        assert!(crate::parse_gp(&wrong).is_err());
    }
}
