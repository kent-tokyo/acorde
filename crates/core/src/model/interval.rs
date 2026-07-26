use serde::{Deserialize, Serialize};

use super::pitch::Pitch;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum IntervalQuality {
    Perfect,
    Major,
    Minor,
    Augmented,
    Diminished,
}

/// Signed chromatic interval between two pitches.
///
/// `semitones > 0` = ascending, `< 0` = descending, `0` = unison.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Interval {
    semitones: i16,
}

impl Interval {
    /// Interval from pitch `a` to pitch `b` (positive = ascending).
    pub fn between(a: &Pitch, b: &Pitch) -> Self {
        Self { semitones: b.to_midi() - a.to_midi() }
    }

    /// Raw signed semitone count.
    pub fn semitones(&self) -> i16 { self.semitones }

    /// Absolute (unsigned) semitone distance.
    pub fn abs_semitones(&self) -> u16 { self.semitones.unsigned_abs() }

    /// True when `b` was higher than `a` in [`Interval::between`].
    pub fn is_ascending(&self) -> bool { self.semitones > 0 }

    /// Interval number within an octave (1 = unison, 2 = second … 8 = octave).
    ///
    /// Compound intervals (> octave) return the simple equivalent (e.g. a ninth → 2).
    pub fn simple_number(&self) -> u8 {
        const TABLE: [u8; 12] = [1, 2, 2, 3, 3, 4, 4, 5, 6, 6, 7, 7];
        TABLE[(self.semitones.unsigned_abs() as usize) % 12]
    }

    /// Quality of the simple interval.
    ///
    /// The tritone (6 semitones) is treated as Augmented (A4).
    pub fn quality(&self) -> IntervalQuality {
        match (self.semitones.unsigned_abs() as usize) % 12 {
            0  => IntervalQuality::Perfect,
            1  => IntervalQuality::Minor,
            2  => IntervalQuality::Major,
            3  => IntervalQuality::Minor,
            4  => IntervalQuality::Major,
            5  => IntervalQuality::Perfect,
            6  => IntervalQuality::Augmented,
            7  => IntervalQuality::Perfect,
            8  => IntervalQuality::Minor,
            9  => IntervalQuality::Major,
            10 => IntervalQuality::Minor,
            11 => IntervalQuality::Major,
            _  => IntervalQuality::Perfect,
        }
    }

    /// Human-readable label: `"P1"`, `"M3"`, `"P5"`, `"m7"`, `"A4"`, `"P8"` etc.
    ///
    /// Octave equivalents (12, 24, …) are shown as `"P8"`.
    pub fn display(&self) -> String {
        let abs = self.semitones.unsigned_abs() as usize;
        let q = match self.quality() {
            IntervalQuality::Perfect    => "P",
            IntervalQuality::Major      => "M",
            IntervalQuality::Minor      => "m",
            IntervalQuality::Augmented  => "A",
            IntervalQuality::Diminished => "d",
        };
        let n = if abs.is_multiple_of(12) && abs >= 12 { 8 } else { self.simple_number() };
        format!("{}{}", q, n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Pitch, Step};

    #[test]
    fn interval_unison_p1() {
        let c4 = Pitch::new(Step::C, 4);
        let iv = Interval::between(&c4, &c4);
        assert_eq!(iv.semitones(), 0);
        assert_eq!(iv.display(), "P1");
    }

    #[test]
    fn interval_major_third_c4_e4() {
        let a = Pitch::new(Step::C, 4);
        let b = Pitch::new(Step::E, 4);
        let iv = Interval::between(&a, &b);
        assert_eq!(iv.semitones(), 4);
        assert_eq!(iv.quality(), IntervalQuality::Major);
        assert_eq!(iv.display(), "M3");
        assert!(iv.is_ascending());
    }

    #[test]
    fn interval_perfect_fifth_c4_g4() {
        let a = Pitch::new(Step::C, 4);
        let b = Pitch::new(Step::G, 4);
        let iv = Interval::between(&a, &b);
        assert_eq!(iv.semitones(), 7);
        assert_eq!(iv.display(), "P5");
    }

    #[test]
    fn interval_minor_seventh_descending() {
        // A4 (MIDI 69) to B3 (MIDI 59): descending minor seventh (-10 semitones)
        let a = Pitch::new(Step::A, 4);
        let b = Pitch::new(Step::B, 3);
        let iv = Interval::between(&a, &b);
        assert_eq!(iv.semitones(), -10);
        assert_eq!(iv.display(), "m7");
        assert!(!iv.is_ascending());
    }

    #[test]
    fn interval_tritone() {
        let a = Pitch::new(Step::C, 4);
        let b = Pitch::with_alter(Step::F, 4, 1); // F#4
        let iv = Interval::between(&a, &b);
        assert_eq!(iv.semitones(), 6);
        assert_eq!(iv.quality(), IntervalQuality::Augmented);
        assert_eq!(iv.display(), "A4");
    }

    #[test]
    fn interval_octave() {
        let a = Pitch::new(Step::C, 4);
        let b = Pitch::new(Step::C, 5);
        let iv = Interval::between(&a, &b);
        assert_eq!(iv.semitones(), 12);
        assert_eq!(iv.display(), "P8");
    }

    #[test]
    fn interval_minor_second() {
        let a = Pitch::new(Step::E, 4);
        let b = Pitch::new(Step::F, 4);
        let iv = Interval::between(&a, &b);
        assert_eq!(iv.semitones(), 1);
        assert_eq!(iv.display(), "m2");
    }
}
