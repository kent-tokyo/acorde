//! Voice-range replacement commands with typed-spanner identity preservation.

use super::spanner_remap::{capture_replaced_endpoint_ids, remap_replaced_spanner_endpoints};
use super::{Error, PasteRangeCmd, PasteVoiceCmd, Score, same_voice};

pub(super) fn apply_paste_voice(cmd: &PasteVoiceCmd, score: &mut Score) -> Result<(), Error> {
    let endpoint_ids = capture_replaced_endpoint_ids(score, |address| {
        same_voice(
            address,
            cmd.part_index,
            cmd.staff_index,
            cmd.measure_index,
            cmd.voice_index,
        )
    });
    let voice = score
        .parts
        .get_mut(cmd.part_index)
        .ok_or(Error::PartNotFound(cmd.part_index))?
        .staves
        .get_mut(cmd.staff_index)
        .ok_or(Error::StaffNotFound(cmd.staff_index))?
        .measures
        .get_mut(cmd.measure_index)
        .ok_or(Error::MeasureNotFound(cmd.measure_index))?
        .voices
        .get_mut(cmd.voice_index)
        .ok_or(Error::VoiceOutOfRange(cmd.voice_index))?;
    *voice = cmd.notes.clone();
    remap_replaced_spanner_endpoints(score, &endpoint_ids);
    Ok(())
}

pub(super) fn apply_paste_range(cmd: &PasteRangeCmd, score: &mut Score) -> Result<(), Error> {
    if cmd.voice_index >= 4 {
        return Err(Error::VoiceOutOfRange(cmd.voice_index));
    }
    let endpoint_ids = capture_replaced_endpoint_ids(score, |address| {
        address.part == cmd.part_index
            && address.staff == cmd.staff_index
            && address.voice == cmd.voice_index
            && address.measure >= cmd.target_measure
            && address.measure < cmd.target_measure.saturating_add(cmd.measures.len())
    });
    let part = score
        .parts
        .get_mut(cmd.part_index)
        .ok_or(Error::PartNotFound(cmd.part_index))?;
    let staff = part
        .staves
        .get_mut(cmd.staff_index)
        .ok_or(Error::StaffNotFound(cmd.staff_index))?;
    for (offset, notes) in cmd.measures.iter().enumerate() {
        let measure_index = cmd.target_measure + offset;
        let measure = staff
            .measures
            .get_mut(measure_index)
            .ok_or(Error::MeasureNotFound(measure_index))?;
        measure.voices[cmd.voice_index] = notes.clone();
    }
    remap_replaced_spanner_endpoints(score, &endpoint_ids);
    Ok(())
}
