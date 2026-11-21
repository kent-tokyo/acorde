//! Convert the transcription workspace's two named MIDI tracks to a grand staff.

use std::{env, error::Error, fs};

use acorde_core::{Clef, MeasureLength, TimeSignature};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let input = args.next().ok_or("missing input MIDI path")?;
    let output = args.next().ok_or("missing output MusicXML path")?;
    let numerator: u8 = args
        .next()
        .ok_or("missing meter numerator")?
        .to_string_lossy()
        .parse()?;
    let denominator: u8 = args
        .next()
        .ok_or("missing meter denominator")?
        .to_string_lossy()
        .parse()?;
    let pickup: u8 = args
        .next()
        .ok_or("missing pickup length")?
        .to_string_lossy()
        .parse()?;
    if args.next().is_some() {
        return Err(
            "expected MIDI, MusicXML, meter numerator, denominator and pickup length".into(),
        );
    }
    if numerator == 0
        || ![1, 2, 4, 8, 16, 32].contains(&denominator)
        || u32::from(pickup) * u32::from(denominator) >= 16 * u32::from(numerator)
    {
        return Err("invalid meter or pickup length".into());
    }
    let mut score = acorde_io::parse_midi(&fs::read(input)?)?;
    if pickup > 0 {
        score.settings.time_signature = TimeSignature {
            numerator,
            denominator,
        };
        for part in &mut score.parts {
            for staff in &mut part.staves {
                let first = staff.measures.first_mut().ok_or("missing pickup measure")?;
                first.time_sig = None;
                first.actual_length = Some(MeasureLength {
                    numerator: u32::from(pickup),
                    denominator: 16,
                });
                if let Some(second) = staff.measures.get_mut(1) {
                    second.time_sig = None;
                }
            }
        }
    }
    if score
        .parts
        .iter()
        .any(|part| part.name != "right" && part.name != "left")
        || score.parts.len() > 2
    {
        return Err("expected MIDI tracks named right and left".into());
    }
    let (name, short_name) = if score.parts.iter().any(|part| part.midi_program == 21) {
        ("Accordion", "Acc.")
    } else {
        ("Piano", "Pno.")
    };
    if score.parts.len() == 2 {
        if score.parts[0].name == score.parts[1].name {
            return Err("duplicate hand track".into());
        }
        if score.parts[0].name == "left" {
            score.parts.swap(0, 1);
        }
        let mut left = score.parts.remove(1);
        let mut staff = left.staves.pop().ok_or("left hand has no staff")?;
        staff.clef = Clef::Bass;
        score.parts[0].staves.push(staff);
        score.parts[0].name = name.to_owned();
        score.parts[0].short_name = short_name.to_owned();
    } else if let Some(part) = score.parts.first_mut() {
        if part.name == "left" {
            let staff = part.staves.first_mut().ok_or("left hand has no staff")?;
            staff.clef = Clef::Bass;
        }
        part.name = name.to_owned();
        part.short_name = short_name.to_owned();
    }
    fs::write(output, acorde_io::serialize_musicxml(&score)?)?;
    Ok(())
}
