use serde::{Deserialize, Serialize};
use super::pitch::Pitch;
use super::notation::KeySignature;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScaleKind {
    Major,
    NaturalMinor,
    HarmonicMinor,
    MelodicMinor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Aeolian,
    Locrian,
    MajorPentatonic,
    MinorPentatonic,
    Blues,
    WholeTone,
    Chromatic,
}

impl ScaleKind {
    /// Semitone intervals from root to each degree (not including the octave).
    pub fn intervals(&self) -> &'static [u8] {
        match self {
            ScaleKind::Major           => &[0, 2, 4, 5, 7, 9, 11],
            ScaleKind::NaturalMinor    => &[0, 2, 3, 5, 7, 8, 10],
            ScaleKind::HarmonicMinor   => &[0, 2, 3, 5, 7, 8, 11],
            ScaleKind::MelodicMinor    => &[0, 2, 3, 5, 7, 9, 11],
            ScaleKind::Dorian          => &[0, 2, 3, 5, 7, 9, 10],
            ScaleKind::Phrygian        => &[0, 1, 3, 5, 7, 8, 10],
            ScaleKind::Lydian          => &[0, 2, 4, 6, 7, 9, 11],
            ScaleKind::Mixolydian      => &[0, 2, 4, 5, 7, 9, 10],
            ScaleKind::Aeolian         => &[0, 2, 3, 5, 7, 8, 10],
            ScaleKind::Locrian         => &[0, 1, 3, 5, 6, 8, 10],
            ScaleKind::MajorPentatonic => &[0, 2, 4, 7, 9],
            ScaleKind::MinorPentatonic => &[0, 3, 5, 7, 10],
            ScaleKind::Blues           => &[0, 3, 5, 6, 7, 10],
            ScaleKind::WholeTone       => &[0, 2, 4, 6, 8, 10],
            ScaleKind::Chromatic       => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scale {
    pub root: Pitch,
    pub kind: ScaleKind,
}

impl Scale {
    pub fn new(root: Pitch, kind: ScaleKind) -> Self {
        Self { root, kind }
    }

    /// Build a scale from a key signature (uses major or natural minor based on mode).
    pub fn from_key(key: &KeySignature) -> Self {
        let (step, alter) = key.tonic();
        let root = Pitch::with_alter(step, 4, alter);
        let kind = if key.mode == "minor" { ScaleKind::NaturalMinor } else { ScaleKind::Major };
        Self { root, kind }
    }

    /// All pitches of this scale in ascending order (root octave preserved).
    pub fn pitches(&self) -> Vec<Pitch> {
        let root_midi = self.root.to_midi() as i32;
        self.kind.intervals().iter().map(|&interval| {
            let midi = (root_midi + interval as i32).clamp(0, 127) as u8;
            Pitch::from_midi(midi, self.root.alter < 0)
        }).collect()
    }

    /// True if `pitch` is a member of this scale (octave-independent, by pitch class).
    pub fn contains(&self, pitch: &Pitch) -> bool {
        let root_class = self.root.to_midi() % 12;
        let pitch_class = pitch.to_midi() % 12;
        let interval = (pitch_class as i32 - root_class as i32).rem_euclid(12) as u8;
        self.kind.intervals().contains(&interval)
    }

    /// Scale degree of `pitch` (1-based), or `None` if not in the scale.
    pub fn degree(&self, pitch: &Pitch) -> Option<usize> {
        let root_class = self.root.to_midi() % 12;
        let pitch_class = pitch.to_midi() % 12;
        let interval = (pitch_class as i32 - root_class as i32).rem_euclid(12) as u8;
        self.kind.intervals().iter().position(|&i| i == interval).map(|pos| pos + 1)
    }

    /// Transpose this scale by `semitones`.
    pub fn transpose(&self, semitones: i8) -> Scale {
        let new_midi = (self.root.to_midi() as i32 + semitones as i32).clamp(0, 127) as u8;
        Scale {
            root: Pitch::from_midi(new_midi, self.root.alter < 0),
            kind: self.kind.clone(),
        }
    }

    /// Find the scale that best fits the given pitches (octave-independent).
    ///
    /// Tries all 12 roots × all non-Chromatic ScaleKinds and picks the one
    /// with the most pitches covered, breaking ties by preferring scales with
    /// more degrees (7-note > 6-note > 5-note) then by `ScaleKind` declaration
    /// order (Major first). Returns `None` if `pitches` is empty.
    pub fn best_fit(pitches: &[Pitch]) -> Option<Scale> {
        if pitches.is_empty() {
            return None;
        }

        const CANDIDATES: &[ScaleKind] = &[
            ScaleKind::Major,
            ScaleKind::NaturalMinor,
            ScaleKind::HarmonicMinor,
            ScaleKind::MelodicMinor,
            ScaleKind::Dorian,
            ScaleKind::Phrygian,
            ScaleKind::Lydian,
            ScaleKind::Mixolydian,
            ScaleKind::Aeolian,
            ScaleKind::Locrian,
            ScaleKind::MajorPentatonic,
            ScaleKind::MinorPentatonic,
            ScaleKind::Blues,
            ScaleKind::WholeTone,
        ];

        let mut best_scale: Option<Scale> = None;
        let mut best_covered: usize = 0;
        let mut best_degrees: usize = 0;

        for root_pc in 0u8..12 {
            let root = Pitch::from_midi(60 + root_pc, false);
            for kind in CANDIDATES {
                let scale = Scale::new(root.clone(), kind.clone());
                let covered = pitches.iter().filter(|p| scale.contains(p)).count();
                let degrees = kind.intervals().len();
                if covered > best_covered
                    || (covered == best_covered && degrees > best_degrees)
                {
                    best_covered = covered;
                    best_degrees = degrees;
                    best_scale = Some(scale);
                }
            }
        }

        best_scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::pitch::Step;

    fn c4() -> Pitch { Pitch::new(Step::C, 4) }
    fn d4() -> Pitch { Pitch::new(Step::D, 4) }
    fn g4() -> Pitch { Pitch::new(Step::G, 4) }

    #[test]
    fn c_major_pitches() {
        let scale = Scale::new(c4(), ScaleKind::Major);
        let pitches = scale.pitches();
        assert_eq!(pitches.len(), 7);
        assert_eq!(pitches[0].step, Step::C);
        assert_eq!(pitches[2].step, Step::E);
        assert_eq!(pitches[4].step, Step::G);
        assert_eq!(pitches[6].step, Step::B);
    }

    #[test]
    fn c_major_contains() {
        let scale = Scale::new(c4(), ScaleKind::Major);
        assert!(scale.contains(&c4()));
        assert!(scale.contains(&g4()));
        assert!(!scale.contains(&Pitch::with_alter(Step::F, 4, 1))); // F# not in C major
    }

    #[test]
    fn c_major_degree() {
        let scale = Scale::new(c4(), ScaleKind::Major);
        assert_eq!(scale.degree(&c4()), Some(1));
        assert_eq!(scale.degree(&d4()), Some(2));
        assert_eq!(scale.degree(&g4()), Some(5));
        assert_eq!(scale.degree(&Pitch::with_alter(Step::F, 4, 1)), None);
    }

    #[test]
    fn c_major_transpose_up_fifth() {
        let scale = Scale::new(c4(), ScaleKind::Major);
        let g_major = scale.transpose(7);
        assert_eq!(g_major.root.step, Step::G);
        assert_eq!(g_major.kind, ScaleKind::Major);
    }

    #[test]
    fn a_natural_minor_pitches() {
        let a4 = Pitch::new(Step::A, 4);
        let scale = Scale::new(a4, ScaleKind::NaturalMinor);
        let pitches = scale.pitches();
        assert_eq!(pitches.len(), 7);
        assert_eq!(pitches[0].step, Step::A);
        assert_eq!(pitches[2].step, Step::C);
    }

    #[test]
    fn blues_scale_has_six_degrees() {
        let scale = Scale::new(c4(), ScaleKind::Blues);
        assert_eq!(scale.pitches().len(), 6);
    }

    #[test]
    fn chromatic_has_twelve_degrees() {
        let scale = Scale::new(c4(), ScaleKind::Chromatic);
        assert_eq!(scale.pitches().len(), 12);
    }

    #[test]
    fn from_key_g_major() {
        let key = KeySignature { fifths: 1, mode: "major".to_string() };
        let scale = Scale::from_key(&key);
        assert_eq!(scale.root.step, Step::G);
        assert_eq!(scale.kind, ScaleKind::Major);
    }

    #[test]
    fn from_key_a_minor() {
        let key = KeySignature { fifths: 0, mode: "minor".to_string() };
        let scale = Scale::from_key(&key);
        assert_eq!(scale.root.step, Step::A);
        assert_eq!(scale.kind, ScaleKind::NaturalMinor);
    }

    #[test]
    fn best_fit_c_major() {
        let pitches = [
            Pitch::new(Step::C, 4), Pitch::new(Step::D, 4), Pitch::new(Step::E, 4),
            Pitch::new(Step::F, 4), Pitch::new(Step::G, 4), Pitch::new(Step::A, 4),
            Pitch::new(Step::B, 4),
        ];
        let scale = Scale::best_fit(&pitches).unwrap();
        assert_eq!(scale.root.step, Step::C);
        assert_eq!(scale.kind, ScaleKind::Major);
    }

    #[test]
    fn best_fit_c_blues() {
        // C Blues: C Eb F Gb G Bb — flat-5 prevents any 7-note diatonic match
        let pitches = [
            Pitch::new(Step::C, 4),
            Pitch::with_alter(Step::E, 4, -1), // Eb
            Pitch::new(Step::F, 4),
            Pitch::with_alter(Step::G, 4, -1), // Gb
            Pitch::new(Step::G, 4),
            Pitch::with_alter(Step::B, 4, -1), // Bb
        ];
        let scale = Scale::best_fit(&pitches).unwrap();
        assert_eq!(scale.root.step, Step::C);
        assert_eq!(scale.kind, ScaleKind::Blues);
    }

    #[test]
    fn best_fit_empty_returns_none() {
        assert!(Scale::best_fit(&[]).is_none());
    }
}
