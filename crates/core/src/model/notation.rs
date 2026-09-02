use serde::{Deserialize, Serialize};

use super::pitch::{Pitch, Step};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Clef {
    Treble,
    Bass,
    Alto,
    Tenor,
    Percussion,
}

impl Clef {
    pub fn to_musicxml_sign(&self) -> &'static str {
        match self {
            Clef::Treble => "G",
            Clef::Bass => "F",
            Clef::Alto => "C",
            Clef::Tenor => "C",
            Clef::Percussion => "percussion",
        }
    }

    pub fn musicxml_line(&self) -> u8 {
        match self {
            Clef::Treble => 2,
            Clef::Bass => 4,
            Clef::Alto => 3,
            Clef::Tenor => 4,
            Clef::Percussion => 2,
        }
    }

    /// MIDI note number of the middle staff line (used for stem direction heuristics).
    pub fn middle_line_midi(&self) -> u8 {
        match self {
            Clef::Treble => 71,     // B4
            Clef::Bass => 50,       // D3
            Clef::Alto => 60,       // C4
            Clef::Tenor => 57,      // A3
            Clef::Percussion => 71, // B4 (same as Treble)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeySignature {
    /// -7 (7 flats) to +7 (7 sharps), 0 = C major / A minor.
    pub fifths: i8,
    /// "major" or "minor"
    pub mode: String,
}

impl Default for KeySignature {
    fn default() -> Self {
        Self {
            fifths: 0,
            mode: "major".to_string(),
        }
    }
}

impl KeySignature {
    // Order of sharps added one by one as fifths increases: F C G D A E B
    const SHARP_ORDER: [Step; 7] = [
        Step::F,
        Step::C,
        Step::G,
        Step::D,
        Step::A,
        Step::E,
        Step::B,
    ];
    // Order of flats: B E A D G C F
    const FLAT_ORDER: [Step; 7] = [
        Step::B,
        Step::E,
        Step::A,
        Step::D,
        Step::G,
        Step::C,
        Step::F,
    ];

    /// Accidental alter (`-1`, `0`, or `+1`) applied to `step` in this key signature.
    pub fn alter_for_step(&self, step: &Step) -> i8 {
        if self.fifths > 0 {
            let count = self.fifths.min(7) as usize;
            if Self::SHARP_ORDER[..count].contains(step) {
                1
            } else {
                0
            }
        } else if self.fifths < 0 {
            let count = (-self.fifths).min(7) as usize;
            if Self::FLAT_ORDER[..count].contains(step) {
                -1
            } else {
                0
            }
        } else {
            0
        }
    }

    /// True if `pitch` is diatonic to this key (octave-independent, checks step + alter).
    pub fn contains_pitch(&self, pitch: &Pitch) -> bool {
        pitch.alter == self.alter_for_step(&pitch.step)
    }

    /// Tonic step and alter for this key.
    ///
    /// Examples: G major → `(Step::G, 0)`, Bb major → `(Step::B, -1)`, F# minor → `(Step::F, 1)`.
    pub fn tonic(&self) -> (Step, i8) {
        if self.mode == "minor" {
            let (maj_step, maj_alter) = Self::major_tonic_from_fifths(self.fifths);
            // Relative minor tonic = major tonic - 3 semitones.
            let major_midi = Pitch::with_alter(maj_step, 4, maj_alter).to_midi();
            let minor_midi = (major_midi - 3).clamp(0, 127) as u8;
            let p = Pitch::from_midi(minor_midi, self.fifths < 0);
            (p.step, p.alter)
        } else {
            Self::major_tonic_from_fifths(self.fifths)
        }
    }

    /// Human-readable key name: `"C major"`, `"G major"`, `"F# minor"`, `"Bb major"` etc.
    pub fn display_name(&self) -> String {
        let (step, alter) = self.tonic();
        let acc = match alter {
            1 => "#",
            -1 => "b",
            _ => "",
        };
        format!("{}{} {}", step.to_char(), acc, self.mode)
    }

    fn major_tonic_from_fifths(fifths: i8) -> (Step, i8) {
        // Index = fifths + 7 (range 0..=14).
        // -7=Cb, -6=Gb, -5=Db, -4=Ab, -3=Eb, -2=Bb, -1=F, 0=C, +1=G, +2=D, +3=A, +4=E, +5=B, +6=F#, +7=C#
        const TONICS: [(Step, i8); 15] = [
            (Step::C, -1), // -7: Cb
            (Step::G, -1), // -6: Gb
            (Step::D, -1), // -5: Db
            (Step::A, -1), // -4: Ab
            (Step::E, -1), // -3: Eb
            (Step::B, -1), // -2: Bb
            (Step::F, 0),  // -1: F
            (Step::C, 0),  //  0: C
            (Step::G, 0),  // +1: G
            (Step::D, 0),  // +2: D
            (Step::A, 0),  // +3: A
            (Step::E, 0),  // +4: E
            (Step::B, 0),  // +5: B
            (Step::F, 1),  // +6: F#
            (Step::C, 1),  // +7: C#
        ];
        let idx = (fifths.clamp(-7, 7) + 7) as usize;
        TONICS[idx].clone()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeSignature {
    pub numerator: u8,
    pub denominator: u8,
}

impl Default for TimeSignature {
    fn default() -> Self {
        Self {
            numerator: 4,
            denominator: 4,
        }
    }
}

impl TimeSignature {
    pub fn beats_per_measure(&self) -> f64 {
        self.numerator as f64
    }

    pub fn beat_unit_beats(&self) -> f64 {
        4.0 / self.denominator as f64
    }

    pub fn total_beats(&self) -> f64 {
        self.beats_per_measure() * self.beat_unit_beats()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Dynamic {
    Pppp,
    Ppp,
    Pp,
    P,
    Mp,
    Mf,
    F,
    Ff,
    Fff,
    Ffff,
    Sfz,
    Rfz,
    Fz,
    Sf,
}

impl Dynamic {
    pub fn to_musicxml_str(&self) -> &'static str {
        match self {
            Dynamic::Pppp => "pppp",
            Dynamic::Ppp => "ppp",
            Dynamic::Pp => "pp",
            Dynamic::P => "p",
            Dynamic::Mp => "mp",
            Dynamic::Mf => "mf",
            Dynamic::F => "f",
            Dynamic::Ff => "ff",
            Dynamic::Fff => "fff",
            Dynamic::Ffff => "ffff",
            Dynamic::Sfz => "sfz",
            Dynamic::Rfz => "rfz",
            Dynamic::Fz => "fz",
            Dynamic::Sf => "sf",
        }
    }

    pub fn to_velocity(&self) -> u8 {
        match self {
            Dynamic::Pppp => 16,
            Dynamic::Ppp => 24,
            Dynamic::Pp => 36,
            Dynamic::P => 48,
            Dynamic::Mp => 60,
            Dynamic::Mf => 72,
            Dynamic::F => 84,
            Dynamic::Ff => 96,
            Dynamic::Fff => 108,
            Dynamic::Ffff => 120,
            Dynamic::Sfz => 112,
            Dynamic::Rfz => 104,
            Dynamic::Fz => 100,
            Dynamic::Sf => 96,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Articulation {
    Staccato,
    Staccatissimo,
    Accent,
    Tenuto,
    Marcato,
    Fermata,
    Trill,
    Mordent,
    InvertedMordent,
    Turn,
    InvertedTurn,
    Shake,
    Tremolo(u8),
    BreathMark,
    Caesura,
}

/// Guitar-specific playing technique attached to a note.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GuitarTechnique {
    Bend,
    Slide,
    HammerOn,
    PullOff,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum Barline {
    #[default]
    Normal,
    Double,
    Final,
    RepeatStart,
    RepeatEnd,
    RepeatBoth,
    Dashed,
    Dotted,
    Invisible,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum HairpinKind {
    Crescendo,
    Decrescendo,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TupletInfo {
    /// Notes in the tuplet group (e.g. 3 for triplet).
    pub actual_notes: u8,
    /// Normal notes displaced (e.g. 2 for triplet = 3-in-2).
    pub normal_notes: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum BeamState {
    #[default]
    None,
    Begin,
    Continue,
    End,
    BeginEnd,
    BackwardHook,
    ForwardHook,
}

/// Ottava (octave transposition bracket).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OttavaKind {
    /// 8va — sounds one octave higher than written.
    Va8,
    /// 8vb — sounds one octave lower than written.
    Vb8,
    /// 15ma — sounds two octaves higher than written.
    Ma15,
    /// 15mb — sounds two octaves lower than written.
    Mb15,
}

impl OttavaKind {
    pub fn musicxml_type(&self) -> &'static str {
        match self {
            OttavaKind::Va8 | OttavaKind::Ma15 => "up",
            OttavaKind::Vb8 | OttavaKind::Mb15 => "down",
        }
    }

    pub fn musicxml_size(&self) -> u8 {
        match self {
            OttavaKind::Va8 | OttavaKind::Vb8 => 8,
            OttavaKind::Ma15 | OttavaKind::Mb15 => 15,
        }
    }
}

/// Note head shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum NoteHead {
    #[default]
    Normal,
    Diamond,  // natural harmonics
    X,        // muted / dead note
    Slash,    // ghost note
    Cross,    // percussion special
    Triangle, // tap harmonics
}

/// Lyric syllable attached to a note.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lyric {
    /// The syllable text.
    pub text: String,
    /// Syllabic position: "single" | "begin" | "middle" | "end"
    pub syllabic: String,
}

/// A typed score text annotation. The text itself is kept separate from its
/// presentation role so consumers do not need to infer semantics from prose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyledText {
    pub style: TextStyle,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextStyle {
    Expression,
    Technique,
    Lyrics,
    ChordSymbol,
    RehearsalMark,
    Generic,
}

/// Cross-staff placement metadata. The note remains in its source staff, so
/// its stable playback address continues to identify the original note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossStaff {
    pub target_staff: usize,
    #[serde(default)]
    pub target_voice: Option<usize>,
}

/// Structured chord symbol attached to a note.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChordSymbol {
    /// Root note name: "C", "F#", "Bb", etc.
    pub root: String,
    /// MusicXML harmony kind: "major", "minor", "dominant", "major-seventh", etc.
    pub kind: String,
    /// Slash-chord bass note.
    pub bass: Option<String>,
}

impl ChordSymbol {
    pub fn display_text(&self) -> String {
        let kind_str = match self.kind.as_str() {
            "major" | "" => "",
            "minor" => "m",
            "dominant" => "7",
            "major-seventh" => "maj7",
            "minor-seventh" => "m7",
            "diminished" => "dim",
            "diminished-seventh" => "dim7",
            "augmented" => "aug",
            "suspended-second" => "sus2",
            "suspended-fourth" => "sus4",
            "half-diminished" => "m7b5",
            "major-sixth" => "6",
            "minor-sixth" => "m6",
            other => other,
        };
        let bass_str = match &self.bass {
            Some(b) => format!("/{}", b),
            None => String::new(),
        };
        format!("{}{}{}", self.root, kind_str, bass_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_alter_c_major_all_natural() {
        let key = KeySignature {
            fifths: 0,
            mode: "major".into(),
        };
        for step in [
            Step::C,
            Step::D,
            Step::E,
            Step::F,
            Step::G,
            Step::A,
            Step::B,
        ] {
            assert_eq!(key.alter_for_step(&step), 0, "step {:?}", step);
        }
    }

    #[test]
    fn key_alter_g_major_fsharp() {
        let key = KeySignature {
            fifths: 1,
            mode: "major".into(),
        };
        assert_eq!(key.alter_for_step(&Step::F), 1);
        assert_eq!(key.alter_for_step(&Step::G), 0);
    }

    #[test]
    fn key_alter_f_major_bflat() {
        let key = KeySignature {
            fifths: -1,
            mode: "major".into(),
        };
        assert_eq!(key.alter_for_step(&Step::B), -1);
        assert_eq!(key.alter_for_step(&Step::C), 0);
    }

    #[test]
    fn key_alter_bb_major() {
        let key = KeySignature {
            fifths: -2,
            mode: "major".into(),
        };
        assert_eq!(key.alter_for_step(&Step::B), -1);
        assert_eq!(key.alter_for_step(&Step::E), -1);
        assert_eq!(key.alter_for_step(&Step::A), 0);
    }

    #[test]
    fn key_contains_pitch_g_major() {
        let key = KeySignature {
            fifths: 1,
            mode: "major".into(),
        };
        // In-key: G, A, B, C, D, E, F# (alter=1)
        assert!(key.contains_pitch(&Pitch::new(Step::G, 4)));
        assert!(key.contains_pitch(&Pitch::new(Step::D, 4)));
        assert!(key.contains_pitch(&Pitch::with_alter(Step::F, 4, 1))); // F#
        // Out-of-key: F natural
        assert!(!key.contains_pitch(&Pitch::new(Step::F, 4)));
    }

    #[test]
    fn key_display_name_c_major() {
        let key = KeySignature {
            fifths: 0,
            mode: "major".into(),
        };
        assert_eq!(key.display_name(), "C major");
    }

    #[test]
    fn key_display_name_bb_major() {
        let key = KeySignature {
            fifths: -2,
            mode: "major".into(),
        };
        assert_eq!(key.display_name(), "Bb major");
    }

    #[test]
    fn key_display_name_fsharp_minor() {
        let key = KeySignature {
            fifths: 3,
            mode: "minor".into(),
        };
        assert_eq!(key.display_name(), "F# minor");
    }

    #[test]
    fn key_tonic_d_major() {
        let key = KeySignature {
            fifths: 2,
            mode: "major".into(),
        };
        let (step, alter) = key.tonic();
        assert_eq!(step, Step::D);
        assert_eq!(alter, 0);
    }

    #[test]
    fn key_tonic_a_minor() {
        // A minor = relative minor of C major (fifths=0)
        let key = KeySignature {
            fifths: 0,
            mode: "minor".into(),
        };
        let (step, alter) = key.tonic();
        assert_eq!(step, Step::A);
        assert_eq!(alter, 0);
    }

    #[test]
    fn chord_display_major() {
        let c = ChordSymbol {
            root: "C".into(),
            kind: "major".into(),
            bass: None,
        };
        assert_eq!(c.display_text(), "C");
    }

    #[test]
    fn chord_display_minor_seventh_slash() {
        let c = ChordSymbol {
            root: "D".into(),
            kind: "minor-seventh".into(),
            bass: Some("F".into()),
        };
        assert_eq!(c.display_text(), "Dm7/F");
    }

    #[test]
    fn time_sig_total_beats_three_four() {
        let ts = TimeSignature {
            numerator: 3,
            denominator: 4,
        };
        assert!((ts.total_beats() - 3.0).abs() < 1e-9);
    }

    #[test]
    fn time_sig_total_beats_six_eight() {
        let ts = TimeSignature {
            numerator: 6,
            denominator: 8,
        };
        assert!((ts.total_beats() - 3.0).abs() < 1e-9);
    }

    #[test]
    fn clef_treble_middle_b4() {
        assert_eq!(Clef::Treble.middle_line_midi(), 71);
    }

    #[test]
    fn clef_bass_middle_d3() {
        assert_eq!(Clef::Bass.middle_line_midi(), 50);
    }

    #[test]
    fn clef_alto_middle_c4() {
        assert_eq!(Clef::Alto.middle_line_midi(), 60);
    }

    #[test]
    fn clef_tenor_middle_a3() {
        assert_eq!(Clef::Tenor.middle_line_midi(), 57);
    }
}
