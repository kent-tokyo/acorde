use crate::Error;
use acorde_core::{Duration, Note, TupletInfo};

const MAX_NOTES_PER_VOICE: usize = 50_000;

pub(crate) fn append_gap_rests(
    voice: &mut Vec<Note>,
    mut ticks: u32,
    divisions: u32,
) -> Result<(), Error> {
    let candidates = [
        (Duration::Longa, divisions.checked_mul(16)),
        (Duration::Breve, divisions.checked_mul(8)),
        (Duration::Whole, divisions.checked_mul(4)),
        (Duration::Half, divisions.checked_mul(2)),
        (Duration::Quarter, Some(divisions)),
        (
            Duration::Eighth,
            divisions.is_multiple_of(2).then_some(divisions / 2),
        ),
        (
            Duration::Sixteenth,
            divisions.is_multiple_of(4).then_some(divisions / 4),
        ),
        (
            Duration::ThirtySecond,
            divisions.is_multiple_of(8).then_some(divisions / 8),
        ),
        (
            Duration::SixtyFourth,
            divisions.is_multiple_of(16).then_some(divisions / 16),
        ),
        (
            Duration::HundredTwentyEighth,
            divisions.is_multiple_of(32).then_some(divisions / 32),
        ),
    ];
    while ticks > 0 {
        // Prefer one hidden, exact triplet rest over several approximate ordinary rests.
        // MusicXML <forward> is rhythmic positioning, so it must not draw a visible rest.
        let triplet = candidates.iter().find_map(|(duration, base_ticks)| {
            let base = (*base_ticks)?;
            (u64::from(base) * 2 == u64::from(ticks) * 3).then(|| duration.clone())
        });
        if let Some(duration) = triplet {
            if voice.len() >= MAX_NOTES_PER_VOICE {
                return Err(Error::Xml("too many notes in voice".into()));
            }
            let mut rest = Note::rest(duration);
            rest.hidden = true;
            rest.tuplet = Some(TupletInfo {
                actual_notes: 3,
                normal_notes: 2,
            });
            voice.push(rest);
            break;
        }
        // Preserve other exactly representable fractional gaps using a hidden
        // tuplet rest. Choose the shortest usable base and never round ticks.
        let exact_canonical = candidates.iter().any(|(_, base)| *base == Some(ticks));
        let fractional = if exact_canonical {
            None
        } else {
            candidates.iter().rev().find_map(|(duration, base)| {
                let base = (*base)?;
                if base <= ticks {
                    return None;
                }
                let (mut a, mut b) = (base, ticks);
                while b != 0 {
                    (a, b) = (b, a % b);
                }
                let actual = u8::try_from(base / a).ok()?;
                let normal = u8::try_from(ticks / a).ok()?;
                Some((duration.clone(), actual, normal))
            })
        };
        if let Some((duration, actual_notes, normal_notes)) = fractional {
            if voice.len() >= MAX_NOTES_PER_VOICE {
                return Err(Error::Xml("too many notes in voice".into()));
            }
            let mut rest = Note::rest(duration);
            rest.hidden = true;
            rest.tuplet = Some(TupletInfo {
                actual_notes,
                normal_notes,
            });
            voice.push(rest);
            break;
        }
        let mut selected = None;
        for (duration, duration_ticks) in &candidates {
            if let Some(duration_ticks) = duration_ticks
                && *duration_ticks > 0
                && *duration_ticks <= ticks
            {
                selected = Some((duration.clone(), *duration_ticks));
                break;
            }
        }
        let Some((duration, duration_ticks)) = selected else {
            return Err(Error::Xml(
                "Cursor gap is not representable by canonical rests".into(),
            ));
        };
        if voice.len() >= MAX_NOTES_PER_VOICE {
            return Err(Error::Xml("too many notes in voice".into()));
        }
        // A `<forward>` gap draws nothing; the rest keeps its time hidden.
        let mut rest = Note::rest(duration);
        rest.hidden = true;
        voice.push(rest);
        ticks -= duration_ticks;
    }
    Ok(())
}
