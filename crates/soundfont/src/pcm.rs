//! Deterministic peak leveling for Acorde's bounded PCM16 WAV output.

use crate::{Error, MAX_OFFLINE_WAV_BYTES};

/// Measurements from a peak-leveling pass. A rail sample in the input may have
/// clipped before PCM encoding; leveling cannot restore its lost waveform.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PcmPeakReport {
    pub input_peak: f32,
    pub output_peak: f32,
    pub gain: f32,
    pub input_rail_samples: usize,
}

/// Scale a complete Acorde PCM16 WAV to a caller-selected linear peak below full scale.
///
/// The operation preserves sample count, channels, silence, and relative dynamics. It is
/// intended for explicit export leveling, not loudness matching or repair of earlier clipping.
/// Only the canonical 44-byte PCM16 WAV emitted by this crate is accepted.
pub fn normalize_pcm16_wav_peak(wav: &mut [u8], target_peak: f32) -> Result<PcmPeakReport, Error> {
    if !target_peak.is_finite() || !(1.0 / 32767.0..=1.0).contains(&target_peak) {
        return Err(Error::InvalidConfig);
    }
    if wav.len() > MAX_OFFLINE_WAV_BYTES {
        return Err(Error::OfflineOutputTooLarge(MAX_OFFLINE_WAV_BYTES));
    }
    if wav.len() < 44
        || &wav[0..4] != b"RIFF"
        || u32::from_le_bytes(wav[4..8].try_into().unwrap()) as usize != wav.len() - 8
        || &wav[8..16] != b"WAVEfmt "
        || u32::from_le_bytes(wav[16..20].try_into().unwrap()) != 16
        || u16::from_le_bytes(wav[20..22].try_into().unwrap()) != 1
        || !matches!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 1 | 2)
        || u16::from_le_bytes(wav[32..34].try_into().unwrap())
            != u16::from_le_bytes(wav[22..24].try_into().unwrap()) * 2
        || u64::from(u32::from_le_bytes(wav[28..32].try_into().unwrap()))
            != u64::from(u32::from_le_bytes(wav[24..28].try_into().unwrap()))
                * u64::from(u16::from_le_bytes(wav[32..34].try_into().unwrap()))
        || u16::from_le_bytes(wav[34..36].try_into().unwrap()) != 16
        || &wav[36..40] != b"data"
        || u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize != wav.len() - 44
        || !(wav.len() - 44).is_multiple_of(2)
        || !(wav.len() - 44).is_multiple_of(usize::from(u16::from_le_bytes(
            wav[32..34].try_into().unwrap(),
        )))
    {
        return Err(Error::InvalidSample);
    }

    let mut peak = 0_i32;
    let mut rail_samples = 0;
    for sample in wav[44..].as_chunks::<2>().0 {
        let magnitude = i32::from(i16::from_le_bytes([sample[0], sample[1]])).abs();
        peak = peak.max(magnitude);
        rail_samples += usize::from(magnitude >= 32767);
    }
    if peak == 0 {
        return Ok(PcmPeakReport {
            input_peak: 0.0,
            output_peak: 0.0,
            gain: 1.0,
            input_rail_samples: 0,
        });
    }
    let target_sample = (f64::from(target_peak) * 32767.0).round() as i32;
    let gain = f64::from(target_sample) / f64::from(peak);
    let mut output_peak = 0_i32;
    for sample in wav[44..].as_chunks_mut::<2>().0 {
        let original = f64::from(i16::from_le_bytes([sample[0], sample[1]]));
        let leveled = (original * gain)
            .round()
            .clamp(-f64::from(target_sample), f64::from(target_sample))
            as i16;
        output_peak = output_peak.max(i32::from(leveled).abs());
        sample.copy_from_slice(&leveled.to_le_bytes());
    }
    Ok(PcmPeakReport {
        input_peak: peak as f32 / 32768.0,
        output_peak: output_peak as f32 / 32768.0,
        gain: gain as f32,
        input_rail_samples: rail_samples,
    })
}

#[cfg(test)]
mod tests {
    use super::normalize_pcm16_wav_peak;

    fn wav(samples: &[i16]) -> Vec<u8> {
        let data_bytes = (samples.len() * 2) as u32;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_bytes).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16_u32.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&48_000_u32.to_le_bytes());
        out.extend_from_slice(&96_000_u32.to_le_bytes());
        out.extend_from_slice(&2_u16.to_le_bytes());
        out.extend_from_slice(&16_u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_bytes.to_le_bytes());
        for sample in samples {
            out.extend_from_slice(&sample.to_le_bytes());
        }
        out
    }

    #[test]
    fn peak_leveling_preserves_silence_sign_and_frame_count_without_hitting_rail() {
        let mut audio = wav(&[0, 1000, -2000, 500]);
        let length = audio.len();
        let report = normalize_pcm16_wav_peak(&mut audio, 0.5).unwrap();
        let samples = audio[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
            .collect::<Vec<_>>();
        assert_eq!(audio.len(), length);
        assert_eq!(samples, [0, 8192, -16384, 4096]);
        assert_eq!(report.input_rail_samples, 0);
        assert!(report.output_peak <= 0.5);
    }

    #[test]
    fn invalid_request_and_wav_leave_input_unchanged() {
        let mut audio = wav(&[1000, -2000]);
        let original = audio.clone();
        assert!(normalize_pcm16_wav_peak(&mut audio, f32::NAN).is_err());
        assert_eq!(audio, original);
        audio[20] = 3;
        let malformed = audio.clone();
        assert!(normalize_pcm16_wav_peak(&mut audio, 0.5).is_err());
        assert_eq!(audio, malformed);
    }

    #[test]
    fn reports_existing_rail_samples_before_attenuation() {
        let mut audio = wav(&[i16::MAX, i16::MIN, 0]);
        let report = normalize_pcm16_wav_peak(&mut audio, 0.5).unwrap();
        assert_eq!(report.input_rail_samples, 2);
        assert!(report.output_peak <= 0.5);
    }
}
