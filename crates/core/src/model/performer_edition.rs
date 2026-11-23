//! Editions for a standard-bass accordion alone or with a melody partner.
//! The caller supplies the instrument and the written bars entrusted to the partner.
use super::{
    arrange::{GuidedArrangeDiagnostic, PositionedAccordionChord},
    duration::Duration,
    pitch::Pitch,
    score::{Note, Part, Score},
    standard_bass_patterns::{
        ProfiledAccompanimentAction, ProfiledAccompanimentOptions, arrange_profiled_accompaniment,
    },
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "ensemble", rename_all = "kebab-case")]
pub enum PerformerEdition {
    Solo {
        max_right_hand_span: u8,
    },
    Duet {
        partner_measures: Vec<usize>,
        answer_measures: Vec<usize>,
        partner_program: u8,
        partner_range: [u8; 2],
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformerEditionRequest {
    pub accompaniment: ProfiledAccompanimentOptions,
    pub edition: PerformerEdition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformerEditionResult {
    pub score: Score,
    pub left_hand: Vec<ProfiledAccompanimentAction>,
}

fn issue(code: &str, message: &str, measure: Option<usize>) -> GuidedArrangeDiagnostic {
    GuidedArrangeDiagnostic {
        code: code.into(),
        message: message.into(),
        measure_index: measure,
    }
}

fn silent(note: &Note) -> Note {
    let mut rest = Note::rest(note.duration.clone());
    rest.is_grace = note.is_grace;
    rest.is_cue = note.is_cue;
    rest.dot_count = note.dot_count;
    rest.tuplet = note.tuplet.clone();
    rest.tuplet_boundary = note.tuplet_boundary;
    rest
}

/// Add at most one chord tone below a sustained melody note. Short movements and ties
/// are left to the melody alone. Use the button pitches of the harmony at the attack.
fn voice_solo(score: &mut Score, actions: &[ProfiledAccompanimentAction], span: u8) {
    let part = &mut score.parts[0];
    let (right, left) = part.staves.split_at_mut(1);
    let mut groups = actions
        .chunk_by(|a, b| a.target.measure == b.target.measure)
        .peekable();
    for (index, bar) in right[0].measures.iter_mut().enumerate() {
        let Some(group) = groups.next_if(|g| g[0].target.measure == index) else {
            continue;
        };
        let left_voice = &left[0].measures[index].voices[0];
        let mut time = 0.;
        let offsets: Vec<_> = left_voice
            .iter()
            .map(|n| {
                let start = time;
                time += n.beats();
                start
            })
            .collect();
        let mut beat = 0.;
        for note in &mut bar.voices[0] {
            let start = beat;
            beat += note.beats();
            let Some(top) = note.pitches.first().map(Pitch::to_midi) else {
                continue;
            };
            if note.beats() < 2.
                || note.pitch_tie_start(0)
                || note.pitch_tie_end(0)
                || note.is_grace
                || note.is_cue
                || note.is_unpitched
                || note.tuplet.is_some()
            {
                continue;
            }
            let active = group.partition_point(|a| offsets[a.target.note] <= start);
            let Some(action) = active.checked_sub(1).map(|i| &group[i]) else {
                continue;
            };
            // A sustained added note may not cross a change of harmony.
            if group[active..]
                .iter()
                .take_while(|a| offsets[a.target.note] < beat)
                .any(|a| a.candidate != action.candidate)
            {
                continue;
            }
            if let Some(pitch) = ((top - i16::from(span)).max(48)..top - 2).rev().find(|p| {
                action
                    .candidate
                    .chord_midi
                    .iter()
                    .any(|m| i16::from(m % 12) == p % 12)
            }) {
                note.pitches.push(Pitch::from_midi(pitch as u8, false));
            }
        }
    }
}

fn voice_duet(
    score: &mut Score,
    leads: &[usize],
    answers: &[usize],
    program: u8,
    range: [u8; 2],
) -> Result<(), GuidedArrangeDiagnostic> {
    let count = score.parts[0].staves[0].measures.len();
    if range[0] > range[1] || range[1] > 127 || program > 127 {
        return Err(issue(
            "partner-instrument",
            "invalid MIDI range or program",
            None,
        ));
    }
    let flags = |indices: &[usize]| {
        let mut flags = vec![false; count];
        for &i in indices {
            if i >= count || flags[i] {
                return Err(issue(
                    "phrase-address",
                    "invalid or duplicate written measure",
                    Some(i),
                ));
            }
            flags[i] = true;
        }
        Ok(flags)
    };
    let lead = flags(leads)?;
    let answer = flags(answers)?;
    let mut partner = Part::new("Melody partner", "II");
    partner.midi_channel = 1;
    partner.midi_program = program;
    let mut staff = score.parts[0].staves[0].clone();
    for (index, bar) in score.parts[0].staves[0].measures.iter_mut().enumerate() {
        let voice = &mut bar.voices[0];
        let other = &mut staff.measures[index].voices[0];
        if lead[index] && answer[index] {
            return Err(issue(
                "phrase-overlap",
                "partner cannot lead and answer the same phrase",
                Some(index),
            ));
        }
        if lead[index] {
            if voice
                .iter()
                .flat_map(|n| &n.pitches)
                .any(|p| !(i16::from(range[0])..=i16::from(range[1])).contains(&p.to_midi()))
            {
                return Err(issue(
                    "partner-range",
                    "melody outside partner range",
                    Some(index),
                ));
            }
            *voice = other.iter().map(silent).collect();
        } else {
            *other = voice.iter().map(silent).collect();
        }
        if answer[index] {
            let Some((rest, preceding)) = voice.split_last() else {
                return Err(issue("answer-rest", "missing closing rest", Some(index)));
            };
            if !rest.is_rest
                || rest.duration != Duration::Quarter
                || rest.dot_count != 0
                || rest.tuplet.is_some()
            {
                return Err(issue(
                    "answer-rest",
                    "answer needs a closing quarter rest",
                    Some(index),
                ));
            }
            let motif: Vec<_> = preceding
                .iter()
                .rev()
                .filter_map(|n| n.pitches.first())
                .take(2)
                .collect();
            if motif.len() != 2
                || motif
                    .iter()
                    .any(|p| !(i16::from(range[0])..=i16::from(range[1])).contains(&p.to_midi()))
            {
                return Err(issue(
                    "answer-motif",
                    "two in-range melody pitches are required",
                    Some(index),
                ));
            }
            other.pop();
            other.extend(
                motif
                    .into_iter()
                    .map(|p| Note::new(p.clone(), Duration::Eighth)),
            );
        }
    }
    partner.staves.push(staff);
    score.parts.push(partner);
    Ok(())
}

// Playback state (including carried dynamics) can depend on a different preceding
// bar after a handoff or repeat. Verify every original attack against its new owner.
fn check_melody(
    source: &Score,
    edition: &Score,
    partner_measures: &[usize],
) -> Result<(), GuidedArrangeDiagnostic> {
    let events = |score: &Score| {
        crate::to_playback_events_bounded(score, &Default::default())
            .map_err(|_| issue("playback-limit", "edition exceeds playback capacity", None))
    };
    let key = |e: &crate::PlaybackEvent, part: usize, bar: usize| {
        (
            part,
            bar,
            e.time_beats.to_bits(),
            e.duration_beats.to_bits(),
            e.time_secs.to_bits(),
            e.duration_secs.to_bits(),
            e.pitch_midi_cents,
            e.velocity,
        )
    };
    let mut attacks = std::collections::BTreeMap::new();
    for e in events(edition)? {
        if let Some(a) = &e.source
            && a.staff == 0
        {
            *attacks.entry(key(&e, a.part, a.measure)).or_insert(0usize) += 1;
        }
    }
    for e in events(source)? {
        if let Some(a) = &e.source {
            let owner = usize::from(partner_measures.contains(&a.measure));
            let count = attacks
                .get_mut(&key(&e, owner, a.measure))
                .filter(|count| **count > 0)
                .ok_or_else(|| {
                    issue(
                        "changed-melody",
                        "cannot preserve melody timing, pitch and dynamics across this handoff",
                        Some(a.measure),
                    )
                })?;
            *count -= 1;
        }
    }
    Ok(())
}

/// Realize one explicit harmony selection on the supplied buttons, then allocate the melody.
/// Errors discard the entire candidate. Written measure indices are zero based.
pub fn arrange_performer_edition(
    source: &Score,
    request: &PerformerEditionRequest,
) -> Result<PerformerEditionResult, GuidedArrangeDiagnostic> {
    if !source.spanners.is_empty()
        || !source.views.is_empty()
        || !source.object_style_overrides.is_empty()
    {
        return Err(issue(
            "linked-source",
            "source has references requiring remapping",
            None,
        ));
    }
    let mut accompaniment = request.accompaniment.clone();
    let mut written = Vec::new();
    if let Some(staff) = source.parts.first().and_then(|part| part.staves.first()) {
        for (measure_index, bar) in staff.measures.iter().enumerate() {
            let mut beat = 0.;
            for note in &bar.voices[0] {
                if let Some(chord) = &note.chord_symbol {
                    written.push(PositionedAccordionChord {
                        measure_index,
                        offset_beats: beat,
                        chord: chord.clone(),
                    });
                }
                beat += note.beats();
            }
        }
    }
    let alternate = accompaniment.selection.alternative_chords.take();
    let has_written_harmony = !written.is_empty();
    if has_written_harmony {
        if accompaniment.selection.inversion.is_some() {
            return Err(issue(
                "written-harmony",
                "keep the bass of the written progression",
                Some(written[0].measure_index),
            ));
        }
        if !accompaniment.selection.chords.is_empty() && accompaniment.selection.chords != written {
            let index = written
                .iter()
                .find(|c| !accompaniment.selection.chords.contains(c))
                .or_else(|| {
                    accompaniment
                        .selection
                        .chords
                        .iter()
                        .find(|c| !written.contains(c))
                })
                .map(|c| c.measure_index);
            return Err(issue(
                "written-harmony",
                "preserve the positioned source chords",
                index,
            ));
        }
        accompaniment.selection.chords = written;
    }
    let mut base = arrange_profiled_accompaniment(source, &accompaniment);
    if !has_written_harmony
        && base.score.is_none()
        && let Some(chords) = alternate
    {
        accompaniment.selection.chords = chords;
        base = arrange_profiled_accompaniment(source, &accompaniment);
    }
    let mut score = base.score.ok_or_else(|| {
        base.diagnostics
            .into_iter()
            .next()
            .unwrap_or_else(|| issue("accompaniment", "could not realize accompaniment", None))
    })?;
    match &request.edition {
        PerformerEdition::Solo {
            max_right_hand_span,
        } => {
            if *max_right_hand_span > 12 {
                return Err(issue(
                    "hand-span",
                    "right hand span must be at most one octave",
                    None,
                ));
            }
            voice_solo(&mut score, &base.actions, *max_right_hand_span);
        }
        PerformerEdition::Duet {
            partner_measures,
            answer_measures,
            partner_program,
            partner_range,
        } => {
            // Check every playback edge, including jumps from a repeat or ending.
            let visits = super::repeat::measure_sequence(&score);
            for edge in visits.windows(2) {
                if partner_measures.contains(&edge[0]) == partner_measures.contains(&edge[1]) {
                    continue;
                }
                let bars = &score.parts[0].staves[0].measures;
                if bars[edge[0]].voices[0]
                    .last()
                    .is_some_and(|n| n.pitch_tie_start(0))
                    || bars[edge[1]].voices[0]
                        .first()
                        .is_some_and(|n| n.pitch_tie_end(0))
                {
                    return Err(issue(
                        "tie-handoff",
                        "finish the tied note before changing player",
                        Some(edge[1]),
                    ));
                }
            }
            voice_duet(
                &mut score,
                partner_measures,
                answer_measures,
                *partner_program,
                *partner_range,
            )?;
        }
    }
    if !super::validate::validate(&score).is_valid() {
        return Err(issue(
            "invalid-edition",
            "generated edition failed score validation",
            None,
        ));
    }
    let partner = match &request.edition {
        PerformerEdition::Duet {
            partner_measures, ..
        } => partner_measures.as_slice(),
        PerformerEdition::Solo { .. } => &[],
    };
    check_melody(source, &score, partner)?;
    Ok(PerformerEditionResult {
        score,
        left_hand: base.actions,
    })
}
