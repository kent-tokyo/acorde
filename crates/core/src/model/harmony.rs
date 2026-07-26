use super::notation::{ChordSymbol, KeySignature};
use super::pitch::Pitch;

/// Chord templates: sorted semitone intervals from the root (inclusive of 0).
const TEMPLATES: &[(&[u8], &str)] = &[
    (&[0, 4, 7],        "major"),
    (&[0, 3, 7],        "minor"),
    (&[0, 4, 7, 10],    "dominant"),
    (&[0, 4, 7, 11],    "major-seventh"),
    (&[0, 3, 7, 10],    "minor-seventh"),
    (&[0, 3, 6],        "diminished"),
    (&[0, 3, 6, 9],     "diminished-seventh"),
    (&[0, 3, 6, 10],    "half-diminished"),
    (&[0, 4, 8],        "augmented"),
    (&[0, 2, 7],        "suspended-second"),
    (&[0, 5, 7],        "suspended-fourth"),
    (&[0, 4, 7, 9],     "major-sixth"),
    (&[0, 3, 7, 9],     "minor-sixth"),
];

/// Detect the chord name from a slice of pitches.
///
/// Returns `None` when fewer than 2 pitches are provided or no template matches.
/// Octave is ignored; only pitch classes (0–11) are compared.
/// Inversions are detected by trying every pitch class as the root.
/// When the lowest-sounding pitch differs from the root, a slash-chord bass note is set.
pub fn detect_chord(pitches: &[Pitch]) -> Option<ChordSymbol> {
    if pitches.len() < 2 {
        return None;
    }

    // Collect unique pitch classes, preserving first occurrence (used for root name).
    let mut pcs: Vec<(u8, &Pitch)> = Vec::new();
    for p in pitches {
        let pc = (p.to_midi().rem_euclid(12)) as u8;
        if !pcs.iter().any(|(c, _)| *c == pc) {
            pcs.push((pc, p));
        }
    }

    let bass_pitch = pitches.iter().min_by_key(|p| p.to_midi())?;
    let bass_pc = (bass_pitch.to_midi().rem_euclid(12)) as u8;

    // Try each unique pitch class as the root.
    for &(root_pc, root_pitch) in &pcs {
        let mut intervals: Vec<u8> = pcs
            .iter()
            .map(|(pc, _)| (pc + 12 - root_pc) % 12)
            .collect();
        intervals.sort_unstable();

        for &(template, kind) in TEMPLATES {
            if intervals.as_slice() == template {
                let acc = match root_pitch.alter { 1 => "#", -1 => "b", _ => "" };
                let root = format!("{}{}", root_pitch.step.to_char(), acc);

                let bass = if bass_pc != root_pc {
                    let b_acc = match bass_pitch.alter { 1 => "#", -1 => "b", _ => "" };
                    Some(format!("{}{}", bass_pitch.step.to_char(), b_acc))
                } else {
                    None
                };

                return Some(ChordSymbol { root, kind: kind.to_string(), bass });
            }
        }
    }

    None
}

/// Parse a root string like "C", "F#", "Bb" into a MIDI pitch class (0–11).
fn root_to_pc(root: &str) -> Option<u8> {
    let mut chars = root.chars();
    let base = match chars.next()? {
        'C' => 0u8, 'D' => 2, 'E' => 4, 'F' => 5, 'G' => 7, 'A' => 9, 'B' => 11,
        _ => return None,
    };
    let pc = match chars.next() {
        Some('#') => base + 1,
        Some('b') => base.wrapping_sub(1),
        None => base,
        _ => return None,
    };
    Some(pc % 12)
}

/// Returns the Roman numeral analysis string for `chord` in the context of `key`.
///
/// - Uppercase for major-quality chords (I, IV, V7, …).
/// - Lowercase for minor-quality chords (ii, iii, vi, …).
/// - Suffix: `o` for diminished, `o7` for diminished seventh, `ø7` for half-diminished,
///   `+` for augmented, `7` for dominant seventh, `maj7` for major seventh.
/// - Slash chords append `/N` where N is the Roman numeral of the bass note.
/// - Returns `None` when the chord root or bass is outside the key's scale.
pub fn roman_numeral(chord: &ChordSymbol, key: &KeySignature) -> Option<String> {
    const NUMERALS: &[&str] = &["I", "II", "III", "IV", "V", "VI", "VII"];

    // Key root pitch class and scale intervals.
    let (key_step, key_alter) = key.tonic();
    let key_root_pc = {
        let base: u8 = match key_step {
            crate::Step::C => 0, crate::Step::D => 2, crate::Step::E => 4,
            crate::Step::F => 5, crate::Step::G => 7, crate::Step::A => 9,
            crate::Step::B => 11,
        };
        ((base as i8 + key_alter).rem_euclid(12)) as u8
    };
    let scale_intervals: &[u8] = if key.mode == "minor" {
        &[0, 2, 3, 5, 7, 8, 10]   // natural minor
    } else {
        &[0, 2, 4, 5, 7, 9, 11]   // major
    };

    // Map a pitch class to a scale degree (0-based index into NUMERALS).
    let pc_to_degree = |pc: u8| -> Option<usize> {
        let interval = ((pc as i16 - key_root_pc as i16).rem_euclid(12)) as u8;
        scale_intervals.iter().position(|&i| i == interval)
    };

    let chord_pc = root_to_pc(&chord.root)?;
    let degree = pc_to_degree(chord_pc)?;
    let numeral = NUMERALS[degree];

    // Quality → case and suffix.
    let (upper, suffix) = match chord.kind.as_str() {
        "major"              => (true,  ""),
        "dominant"           => (true,  "7"),
        "major-seventh"      => (true,  "maj7"),
        "major-sixth"        => (true,  "6"),
        "augmented"          => (true,  "+"),
        k if k.starts_with("suspended") => (true, ""),
        "minor"              => (false, ""),
        "minor-seventh"      => (false, "7"),
        "minor-sixth"        => (false, "6"),
        "diminished"         => (false, "o"),
        "diminished-seventh" => (false, "o7"),
        "half-diminished"    => (false, "\u{00f8}7"),
        _                    => (true,  ""),
    };

    let rn = if upper {
        format!("{}{}", numeral, suffix)
    } else {
        format!("{}{}", numeral.to_lowercase(), suffix)
    };

    // Slash bass.
    let slash = if let Some(bass) = &chord.bass {
        let bass_pc = root_to_pc(bass)?;
        let bass_degree = pc_to_degree(bass_pc)?;
        let bass_numeral = NUMERALS[bass_degree];
        format!("/{}", bass_numeral)
    } else {
        String::new()
    };

    Some(format!("{}{}", rn, slash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Pitch, Step};

    fn p(step: Step, octave: i8) -> Pitch { Pitch::new(step, octave) }
    fn pa(step: Step, octave: i8, alter: i8) -> Pitch { Pitch::with_alter(step, octave, alter) }

    #[test]
    fn detect_chord_c_major_root_pos() {
        let pitches = [p(Step::C, 4), p(Step::E, 4), p(Step::G, 4)];
        let cs = detect_chord(&pitches).unwrap();
        assert_eq!(cs.root, "C");
        assert_eq!(cs.kind, "major");
        assert_eq!(cs.bass, None);
    }

    #[test]
    fn detect_chord_g_dominant_seventh() {
        let pitches = [p(Step::G, 4), p(Step::B, 4), p(Step::D, 4), p(Step::F, 4)];
        let cs = detect_chord(&pitches).unwrap();
        assert_eq!(cs.root, "G");
        assert_eq!(cs.kind, "dominant");
    }

    #[test]
    fn detect_chord_d_minor() {
        let pitches = [p(Step::D, 4), p(Step::F, 4), p(Step::A, 4)];
        let cs = detect_chord(&pitches).unwrap();
        assert_eq!(cs.root, "D");
        assert_eq!(cs.kind, "minor");
    }

    #[test]
    fn detect_chord_first_inversion() {
        // E4-G4-C5 = C major first inversion; bass (E) ≠ root (C) → slash chord
        let pitches = [p(Step::E, 4), p(Step::G, 4), p(Step::C, 5)];
        let cs = detect_chord(&pitches).unwrap();
        assert_eq!(cs.root, "C");
        assert_eq!(cs.kind, "major");
        assert_eq!(cs.bass, Some("E".to_string()));
    }

    #[test]
    fn detect_chord_slash_chord() {
        // G3-C4-E4-G4 = C/G
        let pitches = [p(Step::G, 3), p(Step::C, 4), p(Step::E, 4), p(Step::G, 4)];
        let cs = detect_chord(&pitches).unwrap();
        assert_eq!(cs.root, "C");
        assert_eq!(cs.kind, "major");
        assert_eq!(cs.bass, Some("G".to_string()));
    }

    #[test]
    fn detect_chord_too_few_notes() {
        let pitches = [p(Step::C, 4)];
        assert!(detect_chord(&pitches).is_none());
    }

    #[test]
    fn detect_chord_no_template_match() {
        // Cluster: C and C#
        let pitches = [p(Step::C, 4), pa(Step::C, 4, 1)];
        assert!(detect_chord(&pitches).is_none());
    }

    #[test]
    fn detect_chord_diminished() {
        // B-D-F = B diminished
        let pitches = [p(Step::B, 3), p(Step::D, 4), p(Step::F, 4)];
        let cs = detect_chord(&pitches).unwrap();
        assert_eq!(cs.root, "B");
        assert_eq!(cs.kind, "diminished");
    }

    #[test]
    fn detect_chord_flat_root() {
        // Bb-D-F = Bb major
        let pitches = [pa(Step::B, 3, -1), p(Step::D, 4), p(Step::F, 4)];
        let cs = detect_chord(&pitches).unwrap();
        assert_eq!(cs.root, "Bb");
        assert_eq!(cs.kind, "major");
    }

    fn c_major_key() -> KeySignature { KeySignature { fifths: 0, mode: "major".to_string() } }

    #[test]
    fn roman_numeral_i_major() {
        let chord = ChordSymbol { root: "C".to_string(), kind: "major".to_string(), bass: None };
        assert_eq!(roman_numeral(&chord, &c_major_key()), Some("I".to_string()));
    }

    #[test]
    fn roman_numeral_v7() {
        let chord = ChordSymbol { root: "G".to_string(), kind: "dominant".to_string(), bass: None };
        assert_eq!(roman_numeral(&chord, &c_major_key()), Some("V7".to_string()));
    }

    #[test]
    fn roman_numeral_ii_minor() {
        let chord = ChordSymbol { root: "D".to_string(), kind: "minor".to_string(), bass: None };
        assert_eq!(roman_numeral(&chord, &c_major_key()), Some("ii".to_string()));
    }

    #[test]
    fn roman_numeral_vii_diminished() {
        let chord = ChordSymbol { root: "B".to_string(), kind: "diminished".to_string(), bass: None };
        assert_eq!(roman_numeral(&chord, &c_major_key()), Some("viio".to_string()));
    }

    #[test]
    fn roman_numeral_out_of_key_returns_none() {
        // F# is not in C major
        let chord = ChordSymbol { root: "F#".to_string(), kind: "major".to_string(), bass: None };
        assert!(roman_numeral(&chord, &c_major_key()).is_none());
    }

    #[test]
    fn roman_numeral_slash_chord() {
        // C/G = I/V
        let chord = ChordSymbol { root: "C".to_string(), kind: "major".to_string(), bass: Some("G".to_string()) };
        assert_eq!(roman_numeral(&chord, &c_major_key()), Some("I/V".to_string()));
    }
}
