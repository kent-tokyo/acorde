/// Returns the General MIDI Level 1 program name for the given 0-based program number.
/// Returns `"Unknown"` if `program >= 128`.
pub fn program_name(program: u8) -> &'static str {
    GM_NAMES.get(program as usize).copied().unwrap_or("Unknown")
}

static GM_NAMES: [&str; 128] = [
    // Piano (0–7)
    "Acoustic Grand Piano",
    "Bright Acoustic Piano",
    "Electric Grand Piano",
    "Honky-tonk Piano",
    "Electric Piano 1",
    "Electric Piano 2",
    "Harpsichord",
    "Clavi",
    // Chromatic Percussion (8–15)
    "Celesta",
    "Glockenspiel",
    "Music Box",
    "Vibraphone",
    "Marimba",
    "Xylophone",
    "Tubular Bells",
    "Dulcimer",
    // Organ (16–23)
    "Drawbar Organ",
    "Percussive Organ",
    "Rock Organ",
    "Church Organ",
    "Reed Organ",
    "Accordion",
    "Harmonica",
    "Tango Accordion",
    // Guitar (24–31)
    "Acoustic Guitar (nylon)",
    "Acoustic Guitar (steel)",
    "Electric Guitar (jazz)",
    "Electric Guitar (clean)",
    "Electric Guitar (muted)",
    "Overdriven Guitar",
    "Distortion Guitar",
    "Guitar harmonics",
    // Bass (32–39)
    "Acoustic Bass",
    "Electric Bass (finger)",
    "Electric Bass (pick)",
    "Fretless Bass",
    "Slap Bass 1",
    "Slap Bass 2",
    "Synth Bass 1",
    "Synth Bass 2",
    // Strings (40–47)
    "Violin",
    "Viola",
    "Cello",
    "Contrabass",
    "Tremolo Strings",
    "Pizzicato Strings",
    "Orchestral Harp",
    "Timpani",
    // Ensemble (48–55)
    "String Ensemble 1",
    "String Ensemble 2",
    "Synth Strings 1",
    "Synth Strings 2",
    "Choir Aahs",
    "Voice Oohs",
    "Synth Voice",
    "Orchestra Hit",
    // Brass (56–63)
    "Trumpet",
    "Trombone",
    "Tuba",
    "Muted Trumpet",
    "French Horn",
    "Brass Section",
    "Synth Brass 1",
    "Synth Brass 2",
    // Reed (64–71)
    "Soprano Sax",
    "Alto Sax",
    "Tenor Sax",
    "Baritone Sax",
    "Oboe",
    "English Horn",
    "Bassoon",
    "Clarinet",
    // Pipe (72–79)
    "Piccolo",
    "Flute",
    "Recorder",
    "Pan Flute",
    "Blown Bottle",
    "Shakuhachi",
    "Whistle",
    "Ocarina",
    // Synth Lead (80–87)
    "Lead 1 (square)",
    "Lead 2 (sawtooth)",
    "Lead 3 (calliope)",
    "Lead 4 (chiff)",
    "Lead 5 (charang)",
    "Lead 6 (voice)",
    "Lead 7 (fifths)",
    "Lead 8 (bass + lead)",
    // Synth Pad (88–95)
    "Pad 1 (new age)",
    "Pad 2 (warm)",
    "Pad 3 (polysynth)",
    "Pad 4 (choir)",
    "Pad 5 (bowed)",
    "Pad 6 (metallic)",
    "Pad 7 (halo)",
    "Pad 8 (sweep)",
    // Synth Effects (96–103)
    "FX 1 (rain)",
    "FX 2 (soundtrack)",
    "FX 3 (crystal)",
    "FX 4 (atmosphere)",
    "FX 5 (brightness)",
    "FX 6 (goblins)",
    "FX 7 (echoes)",
    "FX 8 (sci-fi)",
    // Ethnic (104–111)
    "Sitar",
    "Banjo",
    "Shamisen",
    "Koto",
    "Kalimba",
    "Bag pipe",
    "Fiddle",
    "Shanai",
    // Percussive (112–119)
    "Tinkle Bell",
    "Agogo",
    "Steel Drums",
    "Woodblock",
    "Taiko Drum",
    "Melodic Tom",
    "Synth Drum",
    "Reverse Cymbal",
    // Sound Effects (120–127)
    "Guitar Fret Noise",
    "Breath Noise",
    "Seashore",
    "Bird Tweet",
    "Telephone Ring",
    "Helicopter",
    "Applause",
    "Gunshot",
];

/// Returns the General MIDI percussion instrument name for the given MIDI note number.
///
/// Standard GM percussion map spans notes 35–81. Returns `"Unknown Drum"` outside that range.
pub fn drum_name(note: u8) -> &'static str {
    const LOW: u8 = 35;
    const HIGH: u8 = 81;
    if !(LOW..=HIGH).contains(&note) {
        return "Unknown Drum";
    }
    GM_DRUM_NAMES[(note - LOW) as usize]
}

static GM_DRUM_NAMES: [&str; 47] = [
    "Acoustic Bass Drum", // 35
    "Bass Drum 1",        // 36
    "Side Stick",         // 37
    "Acoustic Snare",     // 38
    "Hand Clap",          // 39
    "Electric Snare",     // 40
    "Low Floor Tom",      // 41
    "Closed Hi-Hat",      // 42
    "High Floor Tom",     // 43
    "Pedal Hi-Hat",       // 44
    "Low Tom",            // 45
    "Open Hi-Hat",        // 46
    "Low-Mid Tom",        // 47
    "Hi-Mid Tom",         // 48
    "Crash Cymbal 1",     // 49
    "High Tom",           // 50
    "Ride Cymbal 1",      // 51
    "Chinese Cymbal",     // 52
    "Ride Bell",          // 53
    "Tambourine",         // 54
    "Splash Cymbal",      // 55
    "Cowbell",            // 56
    "Crash Cymbal 2",     // 57
    "Vibraslap",          // 58
    "Ride Cymbal 2",      // 59
    "Hi Bongo",           // 60
    "Low Bongo",          // 61
    "Mute Hi Conga",      // 62
    "Open Hi Conga",      // 63
    "Low Conga",          // 64
    "High Timbale",       // 65
    "Low Timbale",        // 66
    "High Agogo",         // 67
    "Low Agogo",          // 68
    "Cabasa",             // 69
    "Maracas",            // 70
    "Short Whistle",      // 71
    "Long Whistle",       // 72
    "Short Guiro",        // 73
    "Long Guiro",         // 74
    "Claves",             // 75
    "Hi Wood Block",      // 76
    "Low Wood Block",     // 77
    "Mute Cuica",         // 78
    "Open Cuica",         // 79
    "Mute Triangle",      // 80
    "Open Triangle",      // 81
];

/// Returns the practical playing range `(min_midi, max_midi)` for a GM program number.
///
/// Based on standard orchestral/band instrument ranges. Piano (0) covers 21–108 (A0–C8).
/// Returns `(0, 127)` for programs without a well-defined acoustic range (e.g. sound effects).
pub fn instrument_range(midi_program: u8) -> (u8, u8) {
    GM_RANGES
        .get(midi_program as usize)
        .copied()
        .unwrap_or((0, 127))
}

// Practical playing ranges per GM program (min_midi, max_midi).
// Programs 120–127 (sound effects) use (0, 127) — no meaningful pitch restriction.
static GM_RANGES: [(u8, u8); 128] = [
    // Piano (0–7): 21(A0)–108(C8) for grands; uprights similar
    (21, 108),
    (21, 108),
    (21, 108),
    (21, 108),
    (21, 108),
    (21, 108),
    (21, 108),
    (21, 108),
    // Chromatic Percussion (8–15)
    (60, 108), // Celesta: C4–C8
    (52, 96),  // Glockenspiel: E3–C7
    (48, 84),  // Music Box
    (45, 89),  // Vibraphone: A2–F6
    (36, 96),  // Marimba: C2–C7
    (43, 96),  // Xylophone: G2–C7
    (36, 77),  // Tubular Bells: C2–F5
    (36, 84),  // Dulcimer
    // Organ (16–23)
    (24, 108),
    (24, 108),
    (24, 108),
    (24, 108),
    (24, 108),
    (24, 108),
    (24, 108),
    (24, 108),
    // Guitar (24–31)
    (40, 88),
    (40, 88),
    (40, 88),
    (40, 88),
    (40, 88),
    (40, 88),
    (40, 88),
    (40, 88),
    // Bass (32–39): low instruments
    (28, 67),
    (28, 67),
    (28, 67),
    (28, 67),
    (28, 67),
    (28, 67),
    (28, 67),
    (28, 67),
    // Strings (40–47)
    (55, 103), // Violin: G3–B7
    (48, 91),  // Viola: C3–G6
    (36, 76),  // Cello: C2–E5
    (28, 60),  // Contrabass: E1–C4
    (55, 91),  // Tremolo Strings
    (55, 91),  // Pizzicato Strings
    (21, 108), // Orchestral Harp
    (36, 84),  // Timpani: C2–C6
    // Ensemble (48–55)
    (21, 108),
    (21, 108),
    (21, 108),
    (21, 108),
    (21, 108),
    (21, 108),
    (21, 108),
    (21, 108),
    // Brass (56–63)
    (52, 82), // Trumpet: E3–Bb5
    (36, 67), // Trombone: Bb1–G4
    (28, 67), // Tuba: Bb0–G4
    (52, 82), // Muted Trumpet
    (43, 79), // French Horn: G2–G5
    (52, 82), // Brass Section
    (52, 82), // Synth Brass 1
    (52, 82), // Synth Brass 2
    // Reed (64–71)
    (56, 89), // Soprano Sax: Ab3–E6
    (44, 80), // Alto Sax: Bb2–E5  (concert pitch)
    (38, 75), // Tenor Sax: Ab1–Bb4
    (32, 68), // Baritone Sax: Bb0–Eb4
    (45, 84), // Oboe: A2–C6
    (36, 77), // English Horn: B1–F5
    (34, 77), // Bassoon: Bb1–Bb5
    (52, 96), // Clarinet: E3–C7 (written, concert = -2)
    // Pipe (72–79)
    (60, 96), // Piccolo: C4–C7
    (60, 96), // Flute: C4–C7
    (55, 91), // Recorder
    (55, 89), // Pan Flute
    (48, 84), // Blown Bottle
    (48, 84), // Shakuhachi
    (48, 84), // Whistle
    (48, 84), // Ocarina
    // Synth Lead (80–87): wide range
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    // Synth Pad (88–95)
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    // Synth Effects (96–103)
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    // Ethnic (104–111)
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    // Percussive (112–119)
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    // Sound Effects (120–127)
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
    (0, 127),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gm_program_name_piano() {
        assert_eq!(program_name(0), "Acoustic Grand Piano");
    }

    #[test]
    fn gm_program_name_violin() {
        assert_eq!(program_name(40), "Violin");
    }

    #[test]
    fn gm_program_name_last_entry() {
        assert_eq!(program_name(127), "Gunshot");
    }

    #[test]
    fn gm_program_name_out_of_range() {
        assert_eq!(program_name(128), "Unknown");
        assert_eq!(program_name(255), "Unknown");
    }

    #[test]
    fn drum_name_boundary_low() {
        assert_eq!(drum_name(35), "Acoustic Bass Drum");
    }

    #[test]
    fn drum_name_snare() {
        assert_eq!(drum_name(38), "Acoustic Snare");
    }

    #[test]
    fn drum_name_boundary_high() {
        assert_eq!(drum_name(81), "Open Triangle");
    }

    #[test]
    fn drum_name_out_of_range() {
        assert_eq!(drum_name(0), "Unknown Drum");
        assert_eq!(drum_name(34), "Unknown Drum");
        assert_eq!(drum_name(82), "Unknown Drum");
        assert_eq!(drum_name(255), "Unknown Drum");
    }

    #[test]
    fn instrument_range_piano() {
        assert_eq!(instrument_range(0), (21, 108));
    }

    #[test]
    fn instrument_range_flute() {
        assert_eq!(instrument_range(73), (60, 96));
    }

    #[test]
    fn instrument_range_out_of_gm() {
        assert_eq!(instrument_range(255), (0, 127));
    }
}
