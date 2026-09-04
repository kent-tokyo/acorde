use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Step {
    C,
    D,
    E,
    F,
    G,
    A,
    B,
}

impl Step {
    pub fn to_semitone(&self) -> u8 {
        match self {
            Step::C => 0,
            Step::D => 2,
            Step::E => 4,
            Step::F => 5,
            Step::G => 7,
            Step::A => 9,
            Step::B => 11,
        }
    }

    pub fn from_char(c: char) -> Option<Self> {
        match c.to_ascii_uppercase() {
            'C' => Some(Step::C),
            'D' => Some(Step::D),
            'E' => Some(Step::E),
            'F' => Some(Step::F),
            'G' => Some(Step::G),
            'A' => Some(Step::A),
            'B' => Some(Step::B),
            _ => None,
        }
    }

    pub fn to_char(&self) -> char {
        match self {
            Step::C => 'C',
            Step::D => 'D',
            Step::E => 'E',
            Step::F => 'F',
            Step::G => 'G',
            Step::A => 'A',
            Step::B => 'B',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pitch {
    pub step: Step,
    pub octave: i8,
    pub alter: i8,
    /// Additional cents beyond the integer semitone alteration.
    #[serde(default)]
    pub microtone_cents: i16,
}

impl Pitch {
    pub fn new(step: Step, octave: i8) -> Self {
        Self {
            step,
            octave,
            alter: 0,
            microtone_cents: 0,
        }
    }

    pub fn with_alter(step: Step, octave: i8, alter: i8) -> Self {
        Self {
            step,
            octave,
            alter,
            microtone_cents: 0,
        }
    }

    pub fn with_microtone(step: Step, octave: i8, alter: i8, microtone_cents: i16) -> Self {
        Self {
            step,
            octave,
            alter,
            microtone_cents: microtone_cents.clamp(-99, 99),
        }
    }

    /// Construct a pitch without silently changing its microtone component.
    ///
    /// Returns `None` when `microtone_cents` is outside the canonical -99..=99
    /// range. Use [`Pitch::with_microtone`] only when clamping is intentional.
    pub fn try_with_microtone(
        step: Step,
        octave: i8,
        alter: i8,
        microtone_cents: i16,
    ) -> Option<Self> {
        (-99..=99).contains(&microtone_cents).then_some(Self {
            step,
            octave,
            alter,
            microtone_cents,
        })
    }

    /// MIDI note number (middle C = 60 = C4).
    pub fn to_midi(&self) -> i16 {
        let semitone = self.step.to_semitone() as i16;
        let base = (self.octave as i16 + 1) * 12;
        (base as f32 + semitone as f32 + self.alter as f32 + self.microtone_cents as f32 / 100.0)
            .round() as i16
    }

    /// Exact sounding pitch in hundredths of a MIDI semitone.
    ///
    /// Unlike [`Pitch::to_midi`], this preserves fractional alteration and is
    /// the canonical value for interchange comparisons and pitch-bend adapters.
    pub fn to_midi_cents(&self) -> i32 {
        let semitone = self.step.to_semitone() as i32;
        ((self.octave as i32 + 1) * 12 + semitone + self.alter as i32) * 100
            + self.microtone_cents as i32
    }

    /// Convert a MIDI note number (0–127) to a `Pitch`.
    ///
    /// `prefer_flat` selects the spelling for accidentals:
    /// - `true`  → Db / Eb / Gb / Ab / Bb
    /// - `false` → C# / D# / F# / G# / A#
    pub fn from_midi(midi: u8, prefer_flat: bool) -> Pitch {
        let pc = midi % 12;
        let (step, alter): (Step, i8) = if prefer_flat {
            match pc {
                0 => (Step::C, 0),
                1 => (Step::D, -1),
                2 => (Step::D, 0),
                3 => (Step::E, -1),
                4 => (Step::E, 0),
                5 => (Step::F, 0),
                6 => (Step::G, -1),
                7 => (Step::G, 0),
                8 => (Step::A, -1),
                9 => (Step::A, 0),
                10 => (Step::B, -1),
                11 => (Step::B, 0),
                _ => (Step::C, 0),
            }
        } else {
            match pc {
                0 => (Step::C, 0),
                1 => (Step::C, 1),
                2 => (Step::D, 0),
                3 => (Step::D, 1),
                4 => (Step::E, 0),
                5 => (Step::F, 0),
                6 => (Step::F, 1),
                7 => (Step::G, 0),
                8 => (Step::G, 1),
                9 => (Step::A, 0),
                10 => (Step::A, 1),
                11 => (Step::B, 0),
                _ => (Step::C, 0),
            }
        };
        let step_semitone = step.to_semitone() as i16 + alter as i16;
        let octave = ((midi as i16 - step_semitone) / 12 - 1) as i8;
        Pitch::with_alter(step, octave, alter)
    }

    /// Scientific pitch notation, e.g. "C4", "F#5", "Bb3".
    pub fn to_scientific_name(&self) -> String {
        let accidental = match self.alter {
            2 => "##",
            1 => "#",
            0 => "",
            -1 => "b",
            -2 => "bb",
            _ => "",
        };
        format!("{}{}{}", self.step.to_char(), accidental, self.octave)
    }

    /// Return the enharmonic equivalent of this pitch.
    ///
    /// When `prefer_flat` is `true`, chromatic pitches use a flat spelling (Db, Eb, Gb, Ab, Bb).
    /// When `false`, they use a sharp spelling (C#, D#, F#, G#, A#).
    /// Natural pitches and edge cases (E#→F, B#→C, Cb→B, Fb→E) are always resolved to the
    /// simplest diatonic form regardless of the flag.
    pub fn respell(&self, prefer_flat: bool) -> Pitch {
        let mut pitch = Pitch::from_midi(self.to_midi().clamp(0, 127) as u8, prefer_flat);
        pitch.microtone_cents = self.microtone_cents;
        pitch
    }
}

impl std::str::FromStr for Pitch {
    type Err = ();

    /// Parse scientific pitch notation: `"C4"`, `"F#5"`, `"Bb3"`, `"C##4"`.
    ///
    /// Accepts upper- or lower-case step letters. Returns `Err(())` on any parse failure.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars().peekable();
        let step = Step::from_char(chars.next().ok_or(())?).ok_or(())?;
        let mut alter: i8 = 0;
        loop {
            match chars.peek() {
                Some('#') => {
                    alter += 1;
                    chars.next();
                }
                Some('b') => {
                    alter -= 1;
                    chars.next();
                }
                _ => break,
            }
        }
        let octave: i8 = chars.collect::<String>().parse().map_err(|_| ())?;
        Ok(Pitch::with_alter(step, octave, alter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn middle_c_midi() {
        assert_eq!(Pitch::new(Step::C, 4).to_midi(), 60);
    }

    #[test]
    fn a4_midi() {
        assert_eq!(Pitch::new(Step::A, 4).to_midi(), 69);
    }

    #[test]
    fn scientific_name_sharp() {
        let p = Pitch::with_alter(Step::F, 5, 1);
        assert_eq!(p.to_scientific_name(), "F#5");
    }

    #[test]
    fn scientific_name_flat() {
        let p = Pitch::with_alter(Step::B, 3, -1);
        assert_eq!(p.to_scientific_name(), "Bb3");
    }

    #[test]
    fn respell_natural_unchanged() {
        let p = Pitch::new(Step::C, 4);
        assert_eq!(p.respell(true), Pitch::new(Step::C, 4));
        assert_eq!(p.respell(false), Pitch::new(Step::C, 4));
    }

    #[test]
    fn respell_csharp_to_db() {
        let p = Pitch::with_alter(Step::C, 4, 1); // C#4, midi=61
        let flat = p.respell(true);
        assert_eq!(flat.step, Step::D);
        assert_eq!(flat.alter, -1);
        assert_eq!(flat.octave, 4);
        assert_eq!(flat.to_midi(), 61);
    }

    #[test]
    fn respell_db_to_csharp() {
        let p = Pitch::with_alter(Step::D, 4, -1); // Db4, midi=61
        let sharp = p.respell(false);
        assert_eq!(sharp.step, Step::C);
        assert_eq!(sharp.alter, 1);
        assert_eq!(sharp.octave, 4);
        assert_eq!(sharp.to_midi(), 61);
    }

    #[test]
    fn respell_bsharp_to_c_next_octave() {
        let p = Pitch::with_alter(Step::B, 4, 1); // B#4, midi=72 (C5)
        let resolved = p.respell(true);
        assert_eq!(resolved.step, Step::C);
        assert_eq!(resolved.alter, 0);
        assert_eq!(resolved.octave, 5);
        assert_eq!(resolved.to_midi(), 72);
    }

    #[test]
    fn respell_cb_to_b_prev_octave() {
        let p = Pitch::with_alter(Step::C, 5, -1); // Cb5, midi=71 (B4)
        let resolved = p.respell(false);
        assert_eq!(resolved.step, Step::B);
        assert_eq!(resolved.alter, 0);
        assert_eq!(resolved.octave, 4);
        assert_eq!(resolved.to_midi(), 71);
    }

    #[test]
    fn from_midi_middle_c() {
        let p = Pitch::from_midi(60, false);
        assert_eq!(p.step, Step::C);
        assert_eq!(p.octave, 4);
        assert_eq!(p.alter, 0);
    }

    #[test]
    fn from_midi_c_sharp_prefer_sharp() {
        let p = Pitch::from_midi(61, false);
        assert_eq!(p.step, Step::C);
        assert_eq!(p.octave, 4);
        assert_eq!(p.alter, 1);
    }

    #[test]
    fn from_midi_d_flat_prefer_flat() {
        let p = Pitch::from_midi(61, true);
        assert_eq!(p.step, Step::D);
        assert_eq!(p.octave, 4);
        assert_eq!(p.alter, -1);
    }

    #[test]
    fn from_midi_a4() {
        let p = Pitch::from_midi(69, false);
        assert_eq!(p.step, Step::A);
        assert_eq!(p.octave, 4);
        assert_eq!(p.alter, 0);
    }

    #[test]
    fn from_midi_respell_roundtrip() {
        for midi in 21u8..=108 {
            for prefer_flat in [false, true] {
                let p = Pitch::from_midi(midi, prefer_flat);
                assert_eq!(
                    p.to_midi() as u8,
                    midi,
                    "from_midi({midi},{prefer_flat}) roundtrip failed: {:?}",
                    p
                );
            }
        }
    }

    #[test]
    fn from_str_c4() {
        let p: Pitch = "C4".parse().unwrap();
        assert_eq!(p, Pitch::new(Step::C, 4));
    }

    #[test]
    fn from_str_fsharp5() {
        let p: Pitch = "F#5".parse().unwrap();
        assert_eq!(p, Pitch::with_alter(Step::F, 5, 1));
    }

    #[test]
    fn from_str_bflat3() {
        let p: Pitch = "Bb3".parse().unwrap();
        assert_eq!(p.step, Step::B);
        assert_eq!(p.alter, -1);
        assert_eq!(p.octave, 3);
        assert_eq!(p.to_midi(), 58);
    }

    #[test]
    fn from_str_double_sharp() {
        let p: Pitch = "C##4".parse().unwrap();
        assert_eq!(p.step, Step::C);
        assert_eq!(p.alter, 2);
        assert_eq!(p.octave, 4);
    }

    #[test]
    fn from_str_invalid_step_returns_err() {
        assert!("X4".parse::<Pitch>().is_err());
        assert!("".parse::<Pitch>().is_err());
        assert!("C".parse::<Pitch>().is_err()); // no octave
    }

    #[test]
    fn from_str_roundtrip() {
        for midi in 21u8..=108 {
            for prefer_flat in [false, true] {
                let p = Pitch::from_midi(midi, prefer_flat);
                let name = p.to_scientific_name();
                let parsed: Pitch = name
                    .parse()
                    .unwrap_or_else(|_| panic!("parse failed for {:?}", name));
                assert_eq!(
                    parsed.to_midi() as u8,
                    midi,
                    "roundtrip failed for {:?} (midi {})",
                    name,
                    midi
                );
            }
        }
    }

    #[test]
    fn respell_fsharp_to_gb() {
        let p = Pitch::with_alter(Step::F, 4, 1); // F#4, midi=66
        let flat = p.respell(true);
        assert_eq!(flat.step, Step::G);
        assert_eq!(flat.alter, -1);
        assert_eq!(flat.to_midi(), 66);
    }

    #[test]
    fn midi_cents_preserves_microtonal_boundary() {
        let quarter_sharp = Pitch::with_microtone(Step::C, 4, 0, 50);
        let quarter_flat = Pitch::with_microtone(Step::C, 4, 0, -50);
        assert_eq!(quarter_sharp.to_midi_cents(), 6050);
        assert_eq!(quarter_flat.to_midi_cents(), 5950);
        assert_eq!(quarter_sharp.to_midi(), 61);
        assert_eq!(quarter_flat.to_midi(), 60);
    }

    #[test]
    fn try_with_microtone_rejects_without_clamping() {
        assert_eq!(
            Pitch::try_with_microtone(Step::C, 4, 0, -99)
                .unwrap()
                .microtone_cents,
            -99
        );
        assert_eq!(
            Pitch::try_with_microtone(Step::C, 4, 0, 99)
                .unwrap()
                .microtone_cents,
            99
        );
        assert!(Pitch::try_with_microtone(Step::C, 4, 0, 100).is_none());
        assert_eq!(
            Pitch::with_microtone(Step::C, 4, 0, 100).microtone_cents,
            99
        );
    }
}
