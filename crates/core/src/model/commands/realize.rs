//! MuseScore's "Realize chord symbols": write a staff's chord symbols out as chords.

use super::super::duration::Duration;
use super::super::notation::{ChordSymbol, Clef};
use super::super::pitch::{Pitch, Step};
use super::super::score::{Note, Score};
use super::RealizeChordSymbolsCmd;
use crate::Error;

const STEPS: [Step; 7] = [
    Step::C,
    Step::D,
    Step::E,
    Step::F,
    Step::G,
    Step::A,
    Step::B,
];
/// Semitones above C of each natural letter.
const NATURAL: [i16; 7] = [0, 2, 4, 5, 7, 9, 11];

/// Chord tones as (semitones above the root, letters above the root), root first.
fn chord_tones(kind: &str) -> Option<Vec<(i16, i16)>> {
    const MAJOR: &[(i16, i16)] = &[(0, 0), (4, 2), (7, 4)];
    const MINOR: &[(i16, i16)] = &[(0, 0), (3, 2), (7, 4)];
    let with = |base: &[(i16, i16)], extra: &[(i16, i16)]| {
        base.iter().chain(extra).copied().collect::<Vec<_>>()
    };
    Some(match kind {
        "" | "major" => MAJOR.to_vec(),
        "minor" => MINOR.to_vec(),
        "augmented" => vec![(0, 0), (4, 2), (8, 4)],
        "diminished" => vec![(0, 0), (3, 2), (6, 4)],
        "dominant" => with(MAJOR, &[(10, 6)]),
        "major-seventh" => with(MAJOR, &[(11, 6)]),
        "minor-seventh" => with(MINOR, &[(10, 6)]),
        "diminished-seventh" => vec![(0, 0), (3, 2), (6, 4), (9, 6)],
        "half-diminished" => vec![(0, 0), (3, 2), (6, 4), (10, 6)],
        "augmented-seventh" => vec![(0, 0), (4, 2), (8, 4), (10, 6)],
        "major-minor" | "minor-major" | "minor-major-seventh" => with(MINOR, &[(11, 6)]),
        "major-sixth" => with(MAJOR, &[(9, 5)]),
        "minor-sixth" => with(MINOR, &[(9, 5)]),
        "dominant-ninth" => with(MAJOR, &[(10, 6), (14, 8)]),
        "major-ninth" => with(MAJOR, &[(11, 6), (14, 8)]),
        "minor-ninth" => with(MINOR, &[(10, 6), (14, 8)]),
        "dominant-11th" => with(MAJOR, &[(10, 6), (14, 8), (17, 10)]),
        "major-11th" => with(MAJOR, &[(11, 6), (14, 8), (17, 10)]),
        "minor-11th" => with(MINOR, &[(10, 6), (14, 8), (17, 10)]),
        "dominant-13th" => with(MAJOR, &[(10, 6), (14, 8), (21, 12)]),
        "major-13th" => with(MAJOR, &[(11, 6), (14, 8), (21, 12)]),
        "minor-13th" => with(MINOR, &[(10, 6), (14, 8), (21, 12)]),
        "suspended-second" => vec![(0, 0), (2, 1), (7, 4)],
        "suspended-fourth" => vec![(0, 0), (5, 3), (7, 4)],
        "power" => vec![(0, 0), (7, 4)],
        "major-add9" => with(MAJOR, &[(14, 8)]),
        "minor-add9" => with(MINOR, &[(14, 8)]),
        "dominant-flat-five" => vec![(0, 0), (4, 2), (6, 4), (10, 6)],
        "dominant-sharp-five" => vec![(0, 0), (4, 2), (8, 4), (10, 6)],
        _ => return None,
    })
}

/// A note name ("F#", "Bb") as a letter index and alteration.
fn parse_name(name: &str) -> Option<(usize, i16)> {
    let mut chars = name.trim().chars();
    let letter = chars.next()?.to_ascii_uppercase();
    let index = "CDEFGAB".find(letter)?;
    let alter = chars
        .map(|c| match c {
            '#' | '♯' => 1,
            'b' | '♭' => -1,
            _ => 0,
        })
        .sum();
    Some((index, alter))
}

/// The pitch `letters` letters and `semitones` above a root, spelled from its letters.
fn tone(root: (usize, i16), octave: i8, semitones: i16, letters: i16) -> Pitch {
    let letter_total = root.0 as i16 + letters;
    let letter = letter_total.rem_euclid(7) as usize;
    let tone_octave = i16::from(octave) + letter_total.div_euclid(7);
    let root_midi = NATURAL[root.0] + root.1;
    let natural = NATURAL[letter] + 12 * letter_total.div_euclid(7);
    let alter = (root_midi + semitones - natural).clamp(-2, 2) as i8;
    Pitch::with_alter(STEPS[letter].clone(), tone_octave as i8, alter)
}

/// Close-position voicing of a chord symbol with its root in `octave`, and a slash bass below.
pub(crate) fn realize_chord(chord: &ChordSymbol, octave: i8) -> Option<Vec<Pitch>> {
    let root = parse_name(&chord.root)?;
    let mut tones = chord_tones(chord.kind.as_str())?;
    const DEGREE_SEMITONES: [i16; 14] = [0, 0, 2, 4, 5, 7, 9, 10, 12, 14, 16, 17, 19, 21];
    for degree in &chord.degrees {
        let value = usize::from(degree.value.clamp(1, 13));
        let letters = value as i16 - 1;
        let semitones = DEGREE_SEMITONES[value] + i16::from(degree.alter);
        match degree.kind.as_str() {
            "subtract" => tones.retain(|&(_, l)| l.rem_euclid(7) != letters.rem_euclid(7)),
            "alter" => {
                for entry in &mut tones {
                    if entry.1.rem_euclid(7) == letters.rem_euclid(7) {
                        entry.0 = semitones - 12 * (letters - entry.1) / 7;
                    }
                }
            }
            _ => tones.push((semitones, letters)),
        }
    }
    let mut pitches: Vec<Pitch> = tones
        .iter()
        .map(|&(semitones, letters)| tone(root, octave, semitones, letters))
        .collect();
    pitches.sort_by_key(Pitch::to_midi);
    pitches.dedup_by_key(|pitch| pitch.to_midi());
    if let Some(bass) = chord.bass.as_deref().and_then(parse_name) {
        let lowest = pitches.first().map_or(i16::MAX, Pitch::to_midi);
        let mut bass_octave = octave - 1;
        let mut pitch = tone(bass, bass_octave, 0, 0);
        while pitch.to_midi() >= lowest && bass_octave > 0 {
            bass_octave -= 1;
            pitch = tone(bass, bass_octave, 0, 0);
        }
        pitches.insert(0, pitch);
    }
    Some(pitches)
}

/// Written values of a span given in sixteenths of a beat (sixty-fourth notes), longest first.
fn split_beats(sixteenths: u32) -> Vec<(Duration, u8)> {
    const VALUES: [(Duration, u8, u32); 13] = [
        (Duration::Whole, 1, 96),
        (Duration::Whole, 0, 64),
        (Duration::Half, 1, 48),
        (Duration::Half, 0, 32),
        (Duration::Quarter, 1, 24),
        (Duration::Quarter, 0, 16),
        (Duration::Eighth, 1, 12),
        (Duration::Eighth, 0, 8),
        (Duration::Sixteenth, 1, 6),
        (Duration::Sixteenth, 0, 4),
        (Duration::ThirtySecond, 1, 3),
        (Duration::ThirtySecond, 0, 2),
        (Duration::SixtyFourth, 0, 1),
    ];
    let mut left = sixteenths;
    let mut parts = Vec::new();
    while let Some((value, dots, length)) = VALUES.iter().find(|(_, _, length)| *length <= left) {
        parts.push((value.clone(), *dots));
        left -= length;
    }
    parts
}

pub(super) fn apply_realize_chord_symbols(
    cmd: &RealizeChordSymbolsCmd,
    score: &mut Score,
) -> Result<(), Error> {
    if cmd.target_voice >= 4 {
        return Err(Error::VoiceOutOfRange(cmd.target_voice));
    }
    let source = score
        .parts
        .get(cmd.part_index)
        .ok_or(Error::PartNotFound(cmd.part_index))?
        .staves
        .get(cmd.staff_index)
        .ok_or(Error::StaffNotFound(cmd.staff_index))?;
    // Chord symbols of each bar with their onsets in sixteenths of a beat.
    let symbols: Vec<Vec<(u32, ChordSymbol)>> = source
        .measures
        .iter()
        .map(|measure| {
            let mut found = Vec::new();
            for voice in &measure.voices {
                let mut onset = 0.0f64;
                for note in voice {
                    if let Some(chord) = &note.chord_symbol {
                        found.push(((onset * 16.0).round() as u32, chord.clone()));
                    }
                    onset += note.beats();
                }
            }
            found.sort_by_key(|(onset, _)| *onset);
            found.dedup_by_key(|(onset, _)| *onset);
            found
        })
        .collect();
    let default_meter = score.settings.time_signature.clone();
    let target = score
        .parts
        .get_mut(cmd.target_part)
        .ok_or(Error::PartNotFound(cmd.target_part))?
        .staves
        .get_mut(cmd.target_staff)
        .ok_or(Error::StaffNotFound(cmd.target_staff))?;
    let octave = match target.clef {
        Clef::Bass | Clef::Tenor | Clef::Alto => 3,
        _ => 4,
    };
    let mut held: Option<Vec<Pitch>> = None;
    let bar_count = target.measures.len().min(symbols.len());
    for bar in 0..bar_count {
        let bar_sixteenths = (target.measure_beats(bar, &default_meter) * 16.0).round() as u32;
        // Regions of the bar: (start, end, chord or None for held/rest).
        let mut starts: Vec<(u32, Option<Vec<Pitch>>)> = symbols[bar]
            .iter()
            .filter(|(onset, _)| *onset < bar_sixteenths)
            .map(|(onset, chord)| (*onset, realize_chord(chord, octave)))
            .collect();
        if starts.first().is_none_or(|(onset, _)| *onset > 0) {
            starts.insert(0, (0, None));
        }
        let mut voice = Vec::new();
        for (index, (start, chord)) in starts.iter().enumerate() {
            let end = starts
                .get(index + 1)
                .map_or(bar_sixteenths, |(next, _)| *next);
            if end <= *start {
                continue;
            }
            // A bar or beat without a new symbol holds the chord sounding before it.
            let continues = chord.is_none() && held.is_some();
            if chord.is_some() {
                held = chord.clone();
            }
            let pieces = split_beats(end - start);
            let count = pieces.len();
            for (piece, (value, dots)) in pieces.into_iter().enumerate() {
                let mut note = match &held {
                    Some(pitches) if !pitches.is_empty() => {
                        let mut note = Note::new(pitches[0].clone(), value);
                        note.pitches = pitches.clone();
                        note.tie_end = piece > 0 || continues;
                        note.tie_start = piece + 1 < count;
                        note
                    }
                    _ => Note::rest(value),
                };
                note.dot_count = dots;
                voice.push(note);
            }
            // The held chord ties on into the next region when that region only holds it.
            let next_holds = starts
                .get(index + 1)
                .is_some_and(|(_, next)| next.is_none())
                || (index + 1 == starts.len()
                    && symbols
                        .get(bar + 1)
                        .is_some_and(|next| next.first().is_none_or(|(onset, _)| *onset > 0)));
            if next_holds
                && let Some(last) = voice.last_mut()
                && !last.is_rest
            {
                last.tie_start = true;
            }
        }
        target.measures[bar].voices[cmd.target_voice] = voice;
    }
    // The last bar's chord has nothing to tie into.
    if bar_count == target.measures.len()
        && let Some(last) = target
            .measures
            .last_mut()
            .and_then(|measure| measure.voices[cmd.target_voice].last_mut())
    {
        last.tie_start = false;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{Command, CommandStack, RealizeChordSymbolsCmd};
    use super::realize_chord;
    use crate::{ChordSymbol, Clef, Duration, Note, Pitch, Score, Staff, Step};

    fn chord(root: &str, kind: &str, bass: Option<&str>) -> ChordSymbol {
        serde_json::from_value(serde_json::json!({
            "root": root, "kind": kind, "bass": bass,
        }))
        .expect("chord symbol")
    }

    fn names(pitches: &[Pitch]) -> Vec<String> {
        pitches
            .iter()
            .map(|p| {
                let accidental = match p.alter {
                    1 => "#",
                    -1 => "b",
                    _ => "",
                };
                format!("{:?}{accidental}{}", p.step, p.octave)
            })
            .collect()
    }

    #[test]
    fn chord_symbols_are_voiced_in_close_position_and_spelled_from_their_root() {
        let voiced = |c: ChordSymbol| names(&realize_chord(&c, 4).expect("known kind"));
        assert_eq!(voiced(chord("C", "major", None)), ["C4", "E4", "G4"]);
        assert_eq!(
            voiced(chord("F#", "minor-seventh", None)),
            ["F#4", "A4", "C#5", "E5"]
        );
        assert_eq!(
            voiced(chord("Bb", "dominant", None)),
            ["Bb4", "D5", "F5", "Ab5"]
        );
        assert_eq!(
            voiced(chord("E", "diminished-seventh", None)),
            ["E4", "G4", "Bb4", "Db5"]
        );
        assert_eq!(
            voiced(chord("G", "major", Some("B"))),
            ["B3", "G4", "B4", "D5"]
        );
    }

    #[test]
    fn realize_writes_chords_that_last_until_the_next_symbol() {
        // Bar 1: C at beat 1, G7 at beat 3. Bar 2 has no symbol, so G7 holds; bar 3: F.
        let mut score = Score::new("chords", 120, 4, 4, 0, 3);
        let melody = &mut score.parts[0].staves[0];
        for measure in &mut melody.measures {
            measure.voices[0] = (0..4)
                .map(|_| Note::new(Pitch::new(Step::E, 5), Duration::Quarter))
                .collect();
        }
        melody.measures[0].voices[0][0].chord_symbol = Some(chord("C", "major", None));
        melody.measures[0].voices[0][2].chord_symbol = Some(chord("G", "dominant", None));
        melody.measures[2].voices[0][0].chord_symbol = Some(chord("F", "major", None));
        let mut piano = crate::Part::new("Piano", "Pno.");
        let mut staff = Staff::new(Clef::Bass);
        staff.measures = score.parts[0].staves[0].measures.clone();
        for measure in &mut staff.measures {
            measure.voices = [vec![], vec![], vec![], vec![]];
            measure.voices[0] = vec![Note::rest(Duration::Whole)];
        }
        piano.staves = vec![staff];
        score.parts.push(piano);

        let mut stack = CommandStack::new(10);
        stack
            .execute(
                Command::RealizeChordSymbols(RealizeChordSymbolsCmd {
                    part_index: 0,
                    staff_index: 0,
                    target_part: 1,
                    target_staff: 0,
                    target_voice: 0,
                }),
                &mut score,
            )
            .expect("realizes");
        let bars = &score.parts[1].staves[0].measures;
        let bar1 = &bars[0].voices[0];
        assert_eq!(bar1.len(), 2);
        assert_eq!(names(&bar1[0].pitches), ["C3", "E3", "G3"]);
        assert_eq!(bar1[0].duration, Duration::Half);
        assert_eq!(names(&bar1[1].pitches), ["G3", "B3", "D4", "F4"]);
        assert!(bar1[1].tie_start, "G7 holds over the barline");
        let bar2 = &bars[1].voices[0];
        assert_eq!(bar2.len(), 1);
        assert!(bar2[0].tie_end);
        assert!(!bar2[0].tie_start);
        assert_eq!(names(&bars[2].voices[0][0].pitches), ["F3", "A3", "C4"]);
        assert!(!bars[2].voices[0][0].tie_end);
    }
}
