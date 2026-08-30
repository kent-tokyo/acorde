use serde::{Deserialize, Serialize};

/// Note duration as a power-of-two fraction of a whole note.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Duration {
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    SixtyFourth,
}

impl Duration {
    /// Exact fraction relative to a whole note = 1.
    pub fn as_fraction(&self) -> (u32, u32) {
        match self {
            Duration::Whole => (1, 1),
            Duration::Half => (1, 2),
            Duration::Quarter => (1, 4),
            Duration::Eighth => (1, 8),
            Duration::Sixteenth => (1, 16),
            Duration::ThirtySecond => (1, 32),
            Duration::SixtyFourth => (1, 64),
        }
    }

    /// Duration in ticks when divisions = 480 (ticks per quarter note).
    pub fn to_ticks(&self, dot_count: u8) -> u32 {
        let (num, den) = self.as_fraction();
        let base_ticks = 4 * 480 * num / den;
        let mut ticks = base_ticks;
        let mut dot_value = base_ticks / 2;
        for _ in 0..dot_count {
            ticks += dot_value;
            dot_value /= 2;
        }
        ticks
    }

    /// MusicXML `<type>` string.
    pub fn to_musicxml_type(&self) -> &'static str {
        match self {
            Duration::Whole => "whole",
            Duration::Half => "half",
            Duration::Quarter => "quarter",
            Duration::Eighth => "eighth",
            Duration::Sixteenth => "16th",
            Duration::ThirtySecond => "32nd",
            Duration::SixtyFourth => "64th",
        }
    }

    /// Duration in beats (quarter = 1.0), accounting for dots.
    pub fn beats(&self, dot_count: u8) -> f64 {
        let (num, den) = self.as_fraction();
        let base = 4.0 * num as f64 / den as f64;
        let mut total = base;
        let mut dot = base / 2.0;
        for _ in 0..dot_count {
            total += dot;
            dot /= 2.0;
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_ticks() {
        assert_eq!(Duration::Quarter.to_ticks(0), 480);
    }

    #[test]
    fn dotted_quarter_ticks() {
        assert_eq!(Duration::Quarter.to_ticks(1), 720);
    }

    #[test]
    fn whole_beats() {
        assert!((Duration::Whole.beats(0) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn dotted_half_beats() {
        assert!((Duration::Half.beats(1) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn musicxml_type_strings() {
        assert_eq!(Duration::Sixteenth.to_musicxml_type(), "16th");
        assert_eq!(Duration::ThirtySecond.to_musicxml_type(), "32nd");
    }
}
