//! Phrase-level melody selection for a standard-bass simplified arrangement.
//!
//! The melody may move between voices or staves at measure boundaries. Each phrase is an
//! explicit, reviewable mapping; nothing is inferred. The source score is never changed.
use serde::{Deserialize, Serialize};

use super::{
    playback::to_playback_events_bounded,
    score::{NoteAddr, Score},
    standard_bass_arrangement::{
        StandardBassArrangementOptions, StandardBassArrangementResult, arrange_standard_bass,
        reject,
    },
};

const MAX_PHRASES: usize = 64;

/// Measures `first_measure..=last_measure` take their melody from this voice instead of the
/// voice named in the arrangement options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MelodyPhrase {
    pub first_measure: usize,
    pub last_measure: usize,
    pub part: usize,
    pub staff: usize,
    pub voice: usize,
}

type Triple = (usize, usize, usize);

/// Arrange with the melody following `phrases` (all other measures use the options' voice).
/// Source addresses in the result refer to the unmodified score. The selected melody's
/// playback pitch, onset and duration is checked against that score; any problem returns a
/// diagnostic and no partial result.
pub fn arrange_standard_bass_with_phrases(
    score: &Score,
    options: &StandardBassArrangementOptions,
    phrases: &[MelodyPhrase],
) -> StandardBassArrangementResult {
    let base: Triple = (
        options.melody_part,
        options.melody_staff,
        options.melody_voice,
    );
    let bad = |message: &str| reject("invalid-phrase-selection", message.to_string(), None);
    if phrases.is_empty() || phrases.len() > MAX_PHRASES {
        return bad("supply 1 to 64 phrases");
    }
    let mut sorted = phrases.to_vec();
    sorted.sort_by_key(|p| p.first_measure);
    if sorted
        .windows(2)
        .any(|w| w[0].last_measure >= w[1].first_measure)
    {
        return bad("phrases must not overlap");
    }
    let staff_at = |part: usize, staff: usize| score.parts.get(part)?.staves.get(staff);
    let Some(base_staff) = staff_at(base.0, base.1) else {
        return bad("the option's melody staff does not exist");
    };
    let mut work = score.clone();
    for phrase in &sorted {
        let chosen: Triple = (phrase.part, phrase.staff, phrase.voice);
        let Some(staff) = staff_at(phrase.part, phrase.staff) else {
            return bad("a phrase names a staff that does not exist");
        };
        if phrase.voice >= 4
            || phrase.first_measure > phrase.last_measure
            || phrase.last_measure >= staff.measures.len()
            || phrase.last_measure >= base_staff.measures.len()
        {
            return bad("a phrase has an invalid voice or measure range");
        }
        if staff.clef != base_staff.clef
            || staff.transpose_semitones != base_staff.transpose_semitones
            || staff.tablature.is_some()
        {
            return bad("phrase staves must share clef and transposition and not be tablature");
        }
        if chosen == base {
            continue;
        }
        for m in phrase.first_measure..=phrase.last_measure {
            let (a, b) = (&base_staff.measures[m], &staff.measures[m]);
            let beats = |measure: &super::score::Measure, v: usize| -> f64 {
                measure.voices[v].iter().map(|n| n.beats()).sum()
            };
            if a.actual_length != b.actual_length
                || (beats(a, base.2) - beats(b, phrase.voice)).abs() > 1e-9
            {
                return bad("a phrase measure's selected voice does not last as long as the base");
            }
            let taken = std::mem::take(
                &mut work.parts[phrase.part].staves[phrase.staff].measures[m].voices[phrase.voice],
            );
            let old = std::mem::replace(
                &mut work.parts[base.0].staves[base.1].measures[m].voices[base.2],
                taken,
            );
            work.parts[phrase.part].staves[phrase.staff].measures[m].voices[phrase.voice] = old;
        }
    }
    let mut result = arrange_standard_bass(&work, options);
    if result.score.is_none() {
        return result;
    }
    let phrase_of = |measure: usize| {
        sorted
            .iter()
            .find(|p| (p.first_measure..=p.last_measure).contains(&measure))
    };
    // The swap is an involution within a phrase, so mapping back is the same swap.
    let remap = |addr: &mut NoteAddr| {
        if let Some(p) = phrase_of(addr.measure) {
            let chosen: Triple = (p.part, p.staff, p.voice);
            let here: Triple = (addr.part, addr.staff, addr.voice);
            let to = if here == base {
                chosen
            } else if here == chosen {
                base
            } else {
                return;
            };
            (addr.part, addr.staff, addr.voice) = to;
        }
    };
    for pitch in &mut result.source_pitches {
        remap(&mut pitch.source);
    }
    // Compare against the original score, not the swapped working copy.
    let fingerprint = |input: &Score, keep: &dyn Fn(&NoteAddr) -> bool| {
        to_playback_events_bounded(input, &Default::default()).map(|events| {
            let mut kept: Vec<_> = events
                .into_iter()
                .filter(|e| e.source.as_ref().is_some_and(keep))
                .map(|e| {
                    (
                        e.time_beats.to_bits(),
                        e.pitch_midi_cents,
                        e.duration_beats.to_bits(),
                        e.time_secs.to_bits(),
                        e.duration_secs.to_bits(),
                    )
                })
                .collect::<Vec<_>>();
            kept.sort();
            kept
        })
    };
    let expected = fingerprint(score, &|a| match phrase_of(a.measure) {
        Some(p) => (a.part, a.staff, a.voice) == (p.part, p.staff, p.voice),
        None => (a.part, a.staff, a.voice) == base,
    });
    let actual = fingerprint(result.score.as_ref().unwrap_or(score), &|a| {
        (a.part, a.staff, a.voice) == (0, 0, 0)
    });
    if expected.is_err() || actual.is_err() || expected.ok() != actual.ok() {
        return reject(
            "melody-playback-changed",
            "the phrase melody could not be preserved exactly in the original score's playback",
            None,
        );
    }
    result
}
