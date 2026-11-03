//! Explicit-instrument standard-bass accompaniment patterns with per-attack button reports.
//!
//! The default free-bass preset supplies timing only. Every generated attack is
//! replaced by a supplied button; nothing falls back to the default model.
use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use super::{
    arrange::{AccordionBassMode, GuidedArrangeDiagnostic},
    arrange_presets::{AccordionPresetSelection, arrange_accordion_preset},
    pitch::Pitch,
    playback::to_playback_events_bounded,
    score::{NoteAddr, Score},
    standard_bass::{
        StandardBassCandidate, StandardBassProfile, standard_bass_candidates,
        standard_bass_request_for_chord,
    },
    standard_bass_preset::{ProfiledBassPulseActionKind, candidate_fits_generated_range},
    standard_bass_transition::StandardBassTransitionPolicy,
};

/// (preset id, supported meters). Other presets are reported as unsupported.
const PATTERNS: &[(&str, &[(u8, u8)])] = &[
    ("bass-pulse", &[(2, 4), (3, 4), (4, 4), (9, 8)]),
    ("alternating-bass", &[(2, 4), (3, 4), (4, 4), (9, 8)]),
    ("offbeat-chords", &[(2, 4), (3, 4), (4, 4), (9, 8)]),
    ("waltz-bass", &[(3, 4)]),
    ("waltz-alternating-bass", &[(3, 4)]),
];
const MAX_ATTACKS: usize = 131_072;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfiledAccompanimentOptions {
    /// One of [`profiled_accompaniment_capabilities`]' supported pattern ids.
    pub pattern: String,
    pub selection: AccordionPresetSelection,
    pub profile: StandardBassProfile,
    pub allow_chord_omissions: bool,
    pub allow_chord_additions: bool,
    /// Optional caller-supplied screening of the expanded playback attacks.
    #[serde(default)]
    pub transition_policy: Option<StandardBassTransitionPolicy>,
}

/// One written attack: exactly one pressed button, never an implied simultaneous pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfiledAccompanimentAction {
    pub target: NoteAddr,
    pub kind: ProfiledBassPulseActionKind,
    pub button_id: String,
    pub button_position: i16,
    /// Actual MIDI pitches sounded at this attack.
    pub sounding_midi: Vec<u8>,
    /// True for a bass button other than the harmony's bass (e.g. an alternating fifth).
    pub alternate_bass: bool,
    /// Requested chord pitch classes absent from this attack alone.
    pub omitted_pitch_classes: Vec<u8>,
    /// Pitch classes this attack sounds outside the requested chord.
    pub added_pitch_classes: Vec<u8>,
    /// Bass and chord button pair chosen for the harmony in effect. Its omissions
    /// describe the pair sounding together, which this pattern does not do.
    pub candidate: StandardBassCandidate,
}

/// One playback attack in performance order, including repeats and endings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfiledAccompanimentAttack {
    pub action_index: usize,
    pub time_secs: f64,
    pub duration_secs: f64,
    /// Time since the previous attack of either kind; None for the first.
    pub elapsed_ms: Option<f64>,
    /// Movement from the previous button of the same kind; None for the first of its kind.
    pub position_delta: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfiledAccompanimentResult {
    /// None on failure: no partial score, actions, or attacks are returned.
    pub score: Option<Score>,
    pub actions: Vec<ProfiledAccompanimentAction>,
    pub attacks: Vec<ProfiledAccompanimentAttack>,
    /// Selected melody notes, copied unchanged to the same addresses in the output.
    pub source_notes: Vec<NoteAddr>,
    /// Accompaniment notes; they have no source note and are realized by `actions`.
    pub generated_notes: Vec<NoteAddr>,
    pub diagnostics: Vec<GuidedArrangeDiagnostic>,
    pub resolved_options: ProfiledAccompanimentOptions,
    pub requires_performer_review: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfiledAccompanimentPattern {
    pub id: String,
    /// Supported meters as `[numerator, denominator]`.
    pub meters: Vec<[u8; 2]>,
    /// Melody layers combinable with this pattern; currently only the unchanged melody.
    pub melody_layers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnsupportedProfiledPattern {
    pub id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfiledAccompanimentCapabilities {
    pub patterns: Vec<ProfiledAccompanimentPattern>,
    pub unsupported: Vec<UnsupportedProfiledPattern>,
}

/// Machine-readable support table. Every other default preset is listed with a reason.
pub fn profiled_accompaniment_capabilities() -> ProfiledAccompanimentCapabilities {
    let defaults = super::arrange_presets::accordion_preset_capabilities();
    ProfiledAccompanimentCapabilities {
        patterns: PATTERNS
            .iter()
            .map(|(id, meters)| ProfiledAccompanimentPattern {
                id: (*id).into(),
                meters: meters.iter().map(|(n, d)| [*n, *d]).collect(),
                melody_layers: vec!["unchanged".into()],
            })
            .collect(),
        unsupported: defaults
            .presets
            .into_iter()
            .filter(|preset| !PATTERNS.iter().any(|(id, _)| *id == preset.id))
            .map(|preset| UnsupportedProfiledPattern {
                reason: if preset.requires_free_bass {
                    "requires free bass".into()
                } else if preset.accompaniment.is_none() {
                    "melody layer without a bass/chord pattern".into()
                } else {
                    "not yet mapped to explicit buttons".into()
                },
                id: preset.id,
            })
            .collect(),
    }
}

fn fail(
    options: &ProfiledAccompanimentOptions,
    code: &str,
    message: impl Into<String>,
    measure_index: Option<usize>,
) -> ProfiledAccompanimentResult {
    ProfiledAccompanimentResult {
        score: None,
        actions: vec![],
        attacks: vec![],
        source_notes: vec![],
        generated_notes: vec![],
        diagnostics: vec![GuidedArrangeDiagnostic {
            code: code.into(),
            message: message.into(),
            measure_index,
        }],
        resolved_options: options.clone(),
        requires_performer_review: true,
    }
}

type AddrKey = (usize, usize, usize, usize, usize);

fn key(addr: &NoteAddr) -> AddrKey {
    (addr.part, addr.staff, addr.measure, addr.voice, addr.note)
}

fn pitch_classes(midi: &[u8]) -> BTreeSet<u8> {
    midi.iter().map(|pitch| pitch % 12).collect()
}

/// Generate a supported pattern from explicit button pitches. The melody is unchanged;
/// a missing button is diagnosed instead of being supplied by the default model.
pub fn arrange_profiled_accompaniment(
    score: &Score,
    options: &ProfiledAccompanimentOptions,
) -> ProfiledAccompanimentResult {
    let Some((_, meters)) = PATTERNS.iter().find(|(id, _)| *id == options.pattern) else {
        return fail(
            options,
            "unsupported-profiled-pattern",
            "pattern is not available with an explicit button profile",
            None,
        );
    };
    if options.selection.bass_mode != AccordionBassMode::StandardBass {
        return fail(
            options,
            "invalid-bass-mode",
            "select standard bass for an instrument-profiled pattern",
            None,
        );
    }
    let mut meter = (
        score.settings.time_signature.numerator,
        score.settings.time_signature.denominator,
    );
    for (index, measure) in score
        .parts
        .first()
        .and_then(|part| part.staves.first())
        .map_or(&[][..], |staff| &staff.measures)
        .iter()
        .enumerate()
    {
        if let Some(time) = &measure.time_sig {
            meter = (time.numerator, time.denominator);
        }
        if !meters.iter().any(|&(n, d)| (n, d) == meter) {
            return fail(
                options,
                "unsupported-meter",
                format!("{} does not support this meter", options.pattern),
                Some(index),
            );
        }
    }
    let mut template_selection = options.selection.clone();
    template_selection.bass_mode = AccordionBassMode::FreeBass;
    let template = arrange_accordion_preset(score, template_selection, &options.pattern);
    let Some(mut output) = template.score else {
        let d = template.diagnostics.first();
        return fail(
            options,
            d.map_or("preset-failed", |d| d.code.as_str()),
            d.map_or("cannot generate the pattern", |d| d.message.as_str()),
            d.and_then(|d| d.measure_index),
        );
    };
    let mut current: Option<(StandardBassCandidate, BTreeSet<u8>, u8)> = None;
    let mut actions = Vec::new();
    let Some(left) = output
        .parts
        .first_mut()
        .and_then(|part| part.staves.get_mut(1))
    else {
        return fail(
            options,
            "invalid-output",
            "pattern has no accompaniment staff",
            None,
        );
    };
    for (measure_index, bar) in left.measures.iter_mut().enumerate() {
        let Some(voice) = bar.voices.first_mut() else {
            continue;
        };
        for (note_index, note) in voice.iter_mut().enumerate() {
            if let Some(chord) = &note.chord_symbol {
                let request = match standard_bass_request_for_chord(
                    chord,
                    options.allow_chord_omissions,
                    options.allow_chord_additions,
                ) {
                    Ok(request) => request,
                    Err(error) => {
                        return fail(options, "unsupported-chord", error, Some(measure_index));
                    }
                };
                let candidates = match standard_bass_candidates(&options.profile, &request) {
                    Ok(candidates) => candidates,
                    Err(error) => {
                        return fail(options, "invalid-profile", error, Some(measure_index));
                    }
                };
                let Some(candidate) = candidates.into_iter().find(candidate_fits_generated_range)
                else {
                    return fail(
                        options,
                        "no-standard-bass-candidate",
                        "no permitted buttons for this chord in the supplied profile",
                        Some(measure_index),
                    );
                };
                current = Some((
                    candidate,
                    request.pitch_classes.iter().copied().collect(),
                    request.bass_pitch_class,
                ));
            }
            if note.is_rest {
                continue;
            }
            // No supported pattern ties attacks; a tie would keep template pitches.
            if note.tie_start || note.tie_end {
                return fail(
                    options,
                    "unsupported-template-tie",
                    "tied accompaniment notes are not mapped to buttons",
                    Some(measure_index),
                );
            }
            let Some((candidate, requested, bass_class)) = current.as_ref() else {
                return fail(
                    options,
                    "missing-harmony",
                    "no chord covers the accompaniment",
                    Some(measure_index),
                );
            };
            let (kind, button_id, midi, alternate_bass) = if note.pitches.len() == 1 {
                let wanted = note.pitches[0].to_midi().rem_euclid(12) as u8;
                if wanted == *bass_class {
                    (
                        ProfiledBassPulseActionKind::Bass,
                        candidate.bass_button_id.clone(),
                        vec![candidate.bass_midi],
                        false,
                    )
                } else {
                    let from = options
                        .profile
                        .bass_buttons
                        .iter()
                        .find(|button| button.id == candidate.bass_button_id)
                        .map_or(0, |button| button.position);
                    let Some(button) = options
                        .profile
                        .bass_buttons
                        .iter()
                        .filter(|button| {
                            button.midi % 12 == wanted && (36..=84).contains(&button.midi)
                        })
                        .min_by_key(|button| {
                            (
                                (i32::from(button.position) - i32::from(from)).unsigned_abs(),
                                button.midi.abs_diff(candidate.bass_midi),
                            )
                        })
                    else {
                        return fail(
                            options,
                            "missing-alternate-bass-button",
                            format!("the profile has no bass button for pitch class {wanted}"),
                            Some(measure_index),
                        );
                    };
                    (
                        ProfiledBassPulseActionKind::Bass,
                        button.id.clone(),
                        vec![button.midi],
                        true,
                    )
                }
            } else {
                (
                    ProfiledBassPulseActionKind::Chord,
                    candidate.chord_button_id.clone(),
                    candidate.chord_midi.clone(),
                    false,
                )
            };
            let position = match kind {
                ProfiledBassPulseActionKind::Bass => options
                    .profile
                    .bass_buttons
                    .iter()
                    .find(|button| button.id == button_id)
                    .map(|button| button.position),
                ProfiledBassPulseActionKind::Chord => options
                    .profile
                    .chord_buttons
                    .iter()
                    .find(|button| button.id == button_id)
                    .map(|button| button.position),
            };
            let Some(button_position) = position else {
                return fail(
                    options,
                    "invalid-profile",
                    "selected button is absent from the profile",
                    Some(measure_index),
                );
            };
            let sounding = pitch_classes(&midi);
            note.pitches = midi
                .iter()
                .map(|pitch| Pitch::from_midi(*pitch, false))
                .collect();
            actions.push(ProfiledAccompanimentAction {
                target: NoteAddr {
                    part: 0,
                    staff: 1,
                    measure: measure_index,
                    voice: 0,
                    note: note_index,
                },
                kind,
                button_id,
                button_position,
                sounding_midi: midi,
                alternate_bass,
                omitted_pitch_classes: requested.difference(&sounding).copied().collect(),
                added_pitch_classes: sounding.difference(requested).copied().collect(),
                candidate: candidate.clone(),
            });
        }
    }
    if !super::validate::validate(&output).is_valid() {
        return fail(
            options,
            "invalid-output",
            "profiled pattern produced an invalid score",
            None,
        );
    }
    if super::standard_bass_preset::melody_signature(score)
        .ok()
        .is_none_or(|before| {
            super::standard_bass_preset::melody_signature(&output).ok() != Some(before)
        })
    {
        return fail(
            options,
            "melody-playback-changed",
            "selected melody playback could not be preserved exactly",
            None,
        );
    }
    let attacks = match expanded_attacks(&output, &actions, options.transition_policy.as_ref()) {
        Ok(attacks) => attacks,
        Err((code, message, measure)) => return fail(options, code, message, measure),
    };
    ProfiledAccompanimentResult {
        score: Some(output),
        actions,
        attacks,
        source_notes: template.source_notes,
        generated_notes: template.generated_notes,
        diagnostics: vec![],
        resolved_options: options.clone(),
        requires_performer_review: true,
    }
}

type AttackError = (&'static str, String, Option<usize>);

/// Check every playback attack against its action and screen movement in performance order.
fn expanded_attacks(
    output: &Score,
    actions: &[ProfiledAccompanimentAction],
    policy: Option<&StandardBassTransitionPolicy>,
) -> Result<Vec<ProfiledAccompanimentAttack>, AttackError> {
    let too_large = || {
        (
            "transition-playback-too-large",
            "could not construct a bounded playback timeline".to_string(),
            None,
        )
    };
    let events =
        to_playback_events_bounded(output, &Default::default()).map_err(|_| too_large())?;
    let by_target: HashMap<_, _> = actions
        .iter()
        .enumerate()
        .map(|(index, action)| (key(&action.target), index))
        .collect();
    // One entry per (time, address): an attack with its duration and sounding pitches.
    let mut grouped: Vec<(f64, NoteAddr, f64, BTreeSet<u8>)> = Vec::new();
    let mut slots: HashMap<(u64, AddrKey), usize> = HashMap::new();
    for event in &events {
        let Some(addr) = event
            .source
            .as_ref()
            .filter(|addr| addr.part == 0 && addr.staff == 1 && addr.voice == 0)
        else {
            continue;
        };
        if !event.time_secs.is_finite() || event.time_secs < 0.0 {
            return Err((
                "transition-invalid-time",
                "accompaniment playback has an invalid timestamp".into(),
                Some(addr.measure),
            ));
        }
        match slots.get(&(event.time_secs.to_bits(), key(addr))) {
            Some(&slot) => {
                let (_, _, duration, pitches) = &mut grouped[slot];
                *duration = duration.max(event.duration_secs);
                pitches.insert(event.pitch_midi);
            }
            None => {
                slots.insert((event.time_secs.to_bits(), key(addr)), grouped.len());
                grouped.push((
                    event.time_secs,
                    addr.clone(),
                    event.duration_secs,
                    BTreeSet::from([event.pitch_midi]),
                ));
            }
        }
        if grouped.len() > MAX_ATTACKS {
            return Err(too_large());
        }
    }
    grouped.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut attacks = Vec::with_capacity(grouped.len());
    let mut previous_time: Option<f64> = None;
    let mut previous_position: [Option<i16>; 2] = [None, None];
    for (time, addr, duration, pitches) in grouped {
        let Some(&action_index) = by_target.get(&key(&addr)) else {
            return Err((
                "transition-unmapped-action",
                "playback attack has no button action".into(),
                Some(addr.measure),
            ));
        };
        let action = &actions[action_index];
        if pitches != action.sounding_midi.iter().copied().collect() {
            return Err((
                "transition-playback-mismatch",
                "button action and sounding pitches disagree".into(),
                Some(addr.measure),
            ));
        }
        let side = usize::from(action.kind == ProfiledBassPulseActionKind::Chord);
        let elapsed_ms = previous_time.map(|previous| (time - previous) * 1000.0);
        let position_delta = previous_position[side].map(|previous| {
            (i32::from(action.button_position) - i32::from(previous))
                .unsigned_abs()
                .min(u32::from(u16::MAX)) as u16
        });
        if let Some(policy) = policy {
            if let Some(elapsed) = elapsed_ms
                && elapsed + 1e-7 < f64::from(policy.min_attack_interval_ms)
            {
                return Err((
                    "transition-too-fast",
                    format!(
                        "button attacks are {elapsed:.3} ms apart; policy requires {} ms",
                        policy.min_attack_interval_ms
                    ),
                    Some(addr.measure),
                ));
            }
            if let Some(distance) = position_delta
                && distance > policy.max_position_delta
            {
                return Err((
                    "transition-too-far",
                    format!(
                        "button movement is {distance} units; policy allows {}",
                        policy.max_position_delta
                    ),
                    Some(addr.measure),
                ));
            }
        }
        previous_time = Some(time);
        previous_position[side] = Some(action.button_position);
        attacks.push(ProfiledAccompanimentAttack {
            action_index,
            time_secs: time,
            duration_secs: duration,
            elapsed_ms,
            position_delta,
        });
    }
    if attacks.is_empty() {
        return Err((
            "transition-no-attacks",
            "no audible standard-bass attacks were generated".into(),
            None,
        ));
    }
    Ok(attacks)
}
