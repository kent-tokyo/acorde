use crate::Error;
use acorde_core::Score;

/// Validate values that are represented directly by MIDI channel messages.
pub(super) fn validate_event_ranges(score: &Score) -> Result<(), Error> {
    for (part_index, part) in score.parts.iter().enumerate() {
        if part.midi_channel > 15 {
            return Err(Error::Midi(format!(
                "part {} MIDI channel {} is outside 0..=15",
                part_index + 1,
                part.midi_channel
            )));
        }
        if part.midi_program > 127 {
            return Err(Error::Midi(format!(
                "part {} MIDI program {} is outside 0..=127",
                part_index + 1,
                part.midi_program
            )));
        }
        for (index, bend) in part.midi_pitch_bends.iter().enumerate() {
            if bend.channel > 15 {
                return Err(Error::Midi(format!(
                    "part {} pitch-bend {} channel {} is outside 0..=15",
                    part_index + 1,
                    index + 1,
                    bend.channel
                )));
            }
            if !(-8192..=8191).contains(&bend.value) {
                return Err(Error::Midi(format!(
                    "part {} pitch-bend {} value {} is outside -8192..=8191",
                    part_index + 1,
                    index + 1,
                    bend.value
                )));
            }
        }
        for (index, control) in part.midi_control_changes.iter().enumerate() {
            if control.channel > 15 || control.controller > 127 || control.value > 127 {
                return Err(Error::Midi(format!(
                    "part {} controller {} has a value outside MIDI bounds",
                    part_index + 1,
                    index + 1
                )));
            }
        }
        for (index, program) in part.midi_program_changes.iter().enumerate() {
            if program.channel > 15 || program.program > 127 {
                return Err(Error::Midi(format!(
                    "part {} program change {} has a value outside MIDI bounds",
                    part_index + 1,
                    index + 1
                )));
            }
        }
        for (index, aftertouch) in part.midi_aftertouch.iter().enumerate() {
            if aftertouch.channel > 15
                || aftertouch.value > 127
                || aftertouch.key.is_some_and(|key| key > 127)
            {
                return Err(Error::Midi(format!(
                    "part {} aftertouch {} has a value outside MIDI bounds",
                    part_index + 1,
                    index + 1
                )));
            }
        }
    }
    Ok(())
}
