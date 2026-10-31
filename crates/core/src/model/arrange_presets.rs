//! Versioned, on-demand preset selection over the explicit layered arrangement engine.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

use super::arrange::{
    AccordionBassMode, AccordionInversion, AccordionMelodicVoice, AccordionPattern,
    AccordionSwingPolicy, GuidedArrangeDiagnostic, LayeredAccordionOptions,
    PositionedAccordionChord, arrange_with_layers,
};
use super::score::{NoteAddr, Score};

const CONTRACT_VERSION: u16 = 1;
const ENGINE_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "+arrangement-presets.2");
const LEGACY_ENGINE_VERSION: &str = "1.3.4+arrangement-presets.1";
const LEGACY_FINGERPRINT_VERSION: u16 = 1;
const CONTENT_FINGERPRINT_VERSION: u16 = 2;
const CONTENT_FINGERPRINT_PREFIX: &str = "v2:";
const MAX_SHUFFLE_MEASURES: usize = 128;
const MAX_SHUFFLE_CHORDS: usize = 512;
const MAX_SHUFFLE_POOL: usize = 32;
const MAX_SHUFFLE_NOTES: usize = 8192;
const MAX_SHUFFLE_SELECTIONS: usize = 4;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccordionPresetSelection {
    pub chords: Vec<PositionedAccordionChord>,
    pub alternative_chords: Option<Vec<PositionedAccordionChord>>,
    pub inversion: Option<AccordionInversion>,
    pub bass_mode: AccordionBassMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccordionPresetConfig {
    pub contract_version: u16,
    pub engine_version: String,
    /// Legacy values are 16 hex digits; `v2:` normalizes regenerated object UUIDs.
    pub input_fingerprint: String,
    pub preset_id: String,
    pub options: LayeredAccordionOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccordionPresetResult {
    pub score: Option<Score>,
    pub resolved_config: Option<AccordionPresetConfig>,
    pub diagnostics: Vec<GuidedArrangeDiagnostic>,
    pub source_notes: Vec<NoteAddr>,
    pub generated_notes: Vec<NoteAddr>,
    pub notated_swing: bool,
    /// The seed is encoded as decimal text so JavaScript does not lose 64-bit precision.
    pub seed: Option<String>,
    pub contract_version: u16,
    pub engine_version: String,
    pub input_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccordionShuffleOptions {
    pub selection: AccordionPresetSelection,
    /// Additional explicit harmony/inversion choices; Acorde samples compatible combinations.
    #[serde(default)]
    pub alternative_selections: Vec<AccordionPresetSelection>,
    pub allowed_preset_ids: Vec<String>,
    /// Unsigned 64-bit decimal text; identical input and seed select the same eligible ID.
    pub seed: String,
    pub previous_config: Option<AccordionPresetConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccordionPresetDescription {
    pub id: String,
    pub accompaniment: Option<AccordionPattern>,
    pub melodic_voice: AccordionMelodicVoice,
    pub swing: AccordionSwingPolicy,
    pub requires_free_bass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccordionPresetCapabilities {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_profile: String,
    pub max_shuffled_measures: usize,
    pub max_shuffled_chords: usize,
    pub max_shuffled_pool: usize,
    pub max_shuffled_source_notes: usize,
    pub max_shuffled_selections: usize,
    pub presets: Vec<AccordionPresetDescription>,
}

const PRESETS: &[(
    &str,
    Option<AccordionPattern>,
    AccordionMelodicVoice,
    AccordionSwingPolicy,
    bool,
)] = &[
    (
        "held-chord",
        Some(AccordionPattern::HeldBassChord),
        AccordionMelodicVoice::Unchanged,
        AccordionSwingPolicy::Straight,
        false,
    ),
    (
        "bass-pulse",
        Some(AccordionPattern::BassChordPulse),
        AccordionMelodicVoice::Unchanged,
        AccordionSwingPolicy::Straight,
        false,
    ),
    (
        "alternating-bass",
        Some(AccordionPattern::AlternatingBassChord),
        AccordionMelodicVoice::Unchanged,
        AccordionSwingPolicy::Straight,
        false,
    ),
    (
        "rising-arpeggio",
        Some(AccordionPattern::RisingArpeggio),
        AccordionMelodicVoice::Unchanged,
        AccordionSwingPolicy::Straight,
        true,
    ),
    (
        "alberti-bass",
        Some(AccordionPattern::AlbertiBass),
        AccordionMelodicVoice::Unchanged,
        AccordionSwingPolicy::Straight,
        true,
    ),
    (
        "offbeat-chords",
        Some(AccordionPattern::OffbeatChords),
        AccordionMelodicVoice::Unchanged,
        AccordionSwingPolicy::Straight,
        false,
    ),
    (
        "swing-pulse",
        Some(AccordionPattern::BassChordPulse),
        AccordionMelodicVoice::Unchanged,
        AccordionSwingPolicy::NotatedAccompanimentTwoToOne,
        false,
    ),
    (
        "walking-bass",
        Some(AccordionPattern::WalkingBass),
        AccordionMelodicVoice::Unchanged,
        AccordionSwingPolicy::Straight,
        true,
    ),
    (
        "third-below",
        None,
        AccordionMelodicVoice::ThirdBelow,
        AccordionSwingPolicy::Straight,
        false,
    ),
    (
        "sixth-below",
        None,
        AccordionMelodicVoice::SixthBelow,
        AccordionSwingPolicy::Straight,
        false,
    ),
    (
        "octave-below",
        None,
        AccordionMelodicVoice::OctaveBelow,
        AccordionSwingPolicy::Straight,
        false,
    ),
    (
        "swing-third",
        Some(AccordionPattern::BassChordPulse),
        AccordionMelodicVoice::ThirdBelow,
        AccordionSwingPolicy::NotatedAccompanimentTwoToOne,
        false,
    ),
    (
        "walking-third",
        Some(AccordionPattern::WalkingBass),
        AccordionMelodicVoice::ThirdBelow,
        AccordionSwingPolicy::Straight,
        true,
    ),
];

pub fn accordion_preset_capabilities() -> AccordionPresetCapabilities {
    AccordionPresetCapabilities {
        contract_version: CONTRACT_VERSION,
        engine_version: ENGINE_VERSION.to_string(),
        source_profile:
            "validated one-part, one-staff monophonic melody with explicit positioned chords"
                .to_string(),
        max_shuffled_measures: MAX_SHUFFLE_MEASURES,
        max_shuffled_chords: MAX_SHUFFLE_CHORDS,
        max_shuffled_pool: MAX_SHUFFLE_POOL,
        max_shuffled_source_notes: MAX_SHUFFLE_NOTES,
        max_shuffled_selections: MAX_SHUFFLE_SELECTIONS,
        presets: PRESETS
            .iter()
            .map(
                |(id, accompaniment, melodic_voice, swing, requires_free_bass)| {
                    AccordionPresetDescription {
                        id: (*id).to_string(),
                        accompaniment: *accompaniment,
                        melodic_voice: *melodic_voice,
                        swing: *swing,
                        requires_free_bass: *requires_free_bass,
                    }
                },
            )
            .collect(),
    }
}

fn hash_score(score: &Score) -> Result<String, serde_json::Error> {
    let data = serde_json::to_vec(score)?;
    let hash = data.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    Ok(format!("{hash:016x}"))
}

fn normalize_object_uuid(id: &mut String, seen: &mut BTreeMap<String, String>) {
    if Uuid::parse_str(id).is_err() {
        return;
    }
    if let Some(normalized) = seen.get(id) {
        *id = normalized.clone();
    } else {
        let normalized = format!("acorde-object-uuid-{}", seen.len());
        seen.insert(id.clone(), normalized.clone());
        *id = normalized;
    }
}

fn content_fingerprint(score: &Score) -> Result<String, serde_json::Error> {
    let mut canonical = score.clone();
    let mut seen = BTreeMap::new();
    normalize_object_uuid(&mut canonical.id, &mut seen);
    for part in &mut canonical.parts {
        normalize_object_uuid(&mut part.id, &mut seen);
        for staff in &mut part.staves {
            for measure in &mut staff.measures {
                for voice in &mut measure.voices {
                    for note in voice {
                        normalize_object_uuid(&mut note.id, &mut seen);
                    }
                }
            }
        }
    }
    // Spanners refer to note addresses, which remain in the hashed score.
    for spanner in &mut canonical.spanners {
        normalize_object_uuid(&mut spanner.id, &mut seen);
    }
    for view in &mut canonical.views {
        normalize_object_uuid(&mut view.id, &mut seen);
    }
    hash_score(&canonical)
}

fn input_fingerprint(score: &Score) -> Result<String, serde_json::Error> {
    content_fingerprint(score).map(|hash| format!("{CONTENT_FINGERPRINT_PREFIX}{hash}"))
}

fn fingerprint_version(fingerprint: &str) -> Option<u16> {
    if fingerprint.starts_with(CONTENT_FINGERPRINT_PREFIX)
        && fingerprint.len() == CONTENT_FINGERPRINT_PREFIX.len() + 16
        && fingerprint[CONTENT_FINGERPRINT_PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        Some(CONTENT_FINGERPRINT_VERSION)
    } else if fingerprint.len() == 16 && fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(LEGACY_FINGERPRINT_VERSION)
    } else {
        None
    }
}

fn fingerprint_for_version(
    score: &Score,
    version: u16,
) -> Result<Option<String>, serde_json::Error> {
    match version {
        LEGACY_FINGERPRINT_VERSION => hash_score(score).map(Some),
        CONTENT_FINGERPRINT_VERSION => input_fingerprint(score).map(Some),
        _ => Ok(None),
    }
}

fn engine_matches_config(config: &AccordionPresetConfig) -> bool {
    match fingerprint_version(&config.input_fingerprint) {
        Some(LEGACY_FINGERPRINT_VERSION) => config.engine_version == LEGACY_ENGINE_VERSION,
        Some(CONTENT_FINGERPRINT_VERSION) => config.engine_version == ENGINE_VERSION,
        _ => false,
    }
}

fn error(
    fingerprint: String,
    seed: Option<String>,
    code: &str,
    message: impl Into<String>,
) -> AccordionPresetResult {
    AccordionPresetResult {
        score: None,
        resolved_config: None,
        diagnostics: vec![GuidedArrangeDiagnostic {
            code: code.to_string(),
            message: message.into(),
            measure_index: None,
        }],
        source_notes: Vec::new(),
        generated_notes: Vec::new(),
        notated_swing: false,
        seed,
        contract_version: CONTRACT_VERSION,
        engine_version: ENGINE_VERSION.to_string(),
        input_fingerprint: fingerprint,
    }
}

fn options_for(id: &str, selection: AccordionPresetSelection) -> Option<LayeredAccordionOptions> {
    let (_, accompaniment, melodic_voice, swing, _) =
        PRESETS.iter().find(|preset| preset.0 == id)?;
    Some(LayeredAccordionOptions {
        chords: selection.chords,
        alternative_chords: selection.alternative_chords,
        inversion: selection.inversion,
        accompaniment: *accompaniment,
        melodic_voice: *melodic_voice,
        swing: *swing,
        bass_mode: selection.bass_mode,
    })
}

fn run(
    score: &Score,
    config: AccordionPresetConfig,
    seed: Option<String>,
) -> AccordionPresetResult {
    let result = arrange_with_layers(score, config.options.clone());
    AccordionPresetResult {
        score: result.score,
        resolved_config: Some(config.clone()),
        diagnostics: result.diagnostics,
        source_notes: result.source_notes,
        generated_notes: result.generated_notes,
        notated_swing: result.notated_swing,
        seed,
        contract_version: CONTRACT_VERSION,
        engine_version: ENGINE_VERSION.to_string(),
        input_fingerprint: config.input_fingerprint,
    }
}

/// Choose one stable preset ID. No host-side pitch or rhythm generation is needed.
pub fn arrange_accordion_preset(
    score: &Score,
    selection: AccordionPresetSelection,
    preset_id: &str,
) -> AccordionPresetResult {
    let Ok(fingerprint) = input_fingerprint(score) else {
        return error(
            String::new(),
            None,
            "invalid-input",
            "source score cannot be serialized",
        );
    };
    let Some(options) = options_for(preset_id, selection) else {
        return error(
            fingerprint,
            None,
            "unknown-preset",
            format!("unknown preset ID: {preset_id}"),
        );
    };
    run(
        score,
        AccordionPresetConfig {
            contract_version: CONTRACT_VERSION,
            engine_version: ENGINE_VERSION.to_string(),
            input_fingerprint: fingerprint,
            preset_id: preset_id.to_string(),
            options,
        },
        None,
    )
}

/// Replay a resolved configuration without sampling, checking contract and input identity.
pub fn arrange_from_resolved_preset(
    score: &Score,
    config: AccordionPresetConfig,
) -> AccordionPresetResult {
    let Ok(fingerprint) = input_fingerprint(score) else {
        return error(
            String::new(),
            None,
            "invalid-input",
            "source score cannot be serialized",
        );
    };
    if config.contract_version != CONTRACT_VERSION || !engine_matches_config(&config) {
        return error(
            fingerprint,
            None,
            "unsupported-version",
            "resolved configuration requires another arrangement engine/contract version",
        );
    }
    let expected = fingerprint_for_version(
        score,
        fingerprint_version(&config.input_fingerprint).unwrap_or_default(),
    )
    .ok()
    .flatten();
    if expected.as_deref() != Some(config.input_fingerprint.as_str()) {
        return error(
            fingerprint,
            None,
            "input-mismatch",
            "resolved configuration belongs to another input Score",
        );
    }
    let selection = AccordionPresetSelection {
        chords: config.options.chords.clone(),
        alternative_chords: config.options.alternative_chords.clone(),
        inversion: config.options.inversion,
        bass_mode: config.options.bass_mode,
    };
    if options_for(&config.preset_id, selection).as_ref() != Some(&config.options) {
        return error(
            fingerprint,
            None,
            "invalid-configuration",
            "preset ID does not match its resolved options",
        );
    }
    run(score, config, None)
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

fn event_signature(score: &Score) -> Vec<(u8, u64, u64)> {
    crate::to_playback_events(score, &Default::default())
        .into_iter()
        .map(|event| {
            (
                event.pitch_midi,
                event.time_beats.to_bits(),
                event.duration_beats.to_bits(),
            )
        })
        .collect()
}

/// Sample a bounded, compatible preset pool. An exhausted pool returns a diagnostic promptly.
pub fn shuffle_accordion_preset(
    score: &Score,
    request: AccordionShuffleOptions,
) -> AccordionPresetResult {
    let Ok(fingerprint) = input_fingerprint(score) else {
        return error(
            String::new(),
            Some(request.seed),
            "invalid-input",
            "source score cannot be serialized",
        );
    };
    let seed = Some(request.seed.clone());
    let Ok(parsed_seed) = request.seed.parse::<u64>() else {
        return error(
            fingerprint,
            seed,
            "invalid-seed",
            "seed must be unsigned 64-bit decimal text",
        );
    };
    if score.parts.len() != 1
        || score.parts[0].staves.len() != 1
        || score.parts[0].staves[0].measures.len() > MAX_SHUFFLE_MEASURES
        || score.parts[0].staves[0]
            .measures
            .iter()
            .flat_map(|measure| measure.voices.iter())
            .map(Vec::len)
            .sum::<usize>()
            > MAX_SHUFFLE_NOTES
        || request.selection.chords.len() > MAX_SHUFFLE_CHORDS
        || request
            .selection
            .alternative_chords
            .as_ref()
            .is_some_and(|chords| chords.len() > MAX_SHUFFLE_CHORDS)
        || request.allowed_preset_ids.len() > MAX_SHUFFLE_POOL
        || request.alternative_selections.len() + 1 > MAX_SHUFFLE_SELECTIONS
        || request.alternative_selections.iter().any(|selection| {
            selection.chords.len() > MAX_SHUFFLE_CHORDS
                || selection
                    .alternative_chords
                    .as_ref()
                    .is_some_and(|chords| chords.len() > MAX_SHUFFLE_CHORDS)
        })
    {
        return error(
            fingerprint,
            seed,
            "input-too-large",
            "shuffle supports one melody staff, at most 128 measures, 8192 source notes, 512 chords per selection, 4 selections and 32 allowed IDs",
        );
    }
    if request
        .allowed_preset_ids
        .iter()
        .any(|id| !PRESETS.iter().any(|preset| preset.0 == id))
    {
        return error(
            fingerprint,
            seed,
            "unknown-preset",
            "allowed pool includes an unknown preset ID",
        );
    }
    if request.previous_config.as_ref().is_some_and(|previous| {
        previous.contract_version != CONTRACT_VERSION
            || !engine_matches_config(previous)
            || fingerprint_for_version(
                score,
                fingerprint_version(&previous.input_fingerprint).unwrap_or_default(),
            )
            .ok()
            .flatten()
            .as_deref()
                != Some(previous.input_fingerprint.as_str())
    }) {
        return error(
            fingerprint,
            seed,
            "previous-mismatch",
            "previous configuration requires this input and engine version",
        );
    }
    let previous_signature = if let Some(previous) = &request.previous_config {
        let replay = arrange_from_resolved_preset(score, previous.clone());
        let Some(previous_score) = replay.score else {
            return error(
                fingerprint,
                seed,
                "previous-mismatch",
                "previous configuration cannot be reproduced on this input",
            );
        };
        Some(event_signature(&previous_score))
    } else {
        None
    };
    let mut eligible = Vec::new();
    for (id, _, _, _, _) in PRESETS {
        if !request
            .allowed_preset_ids
            .iter()
            .any(|allowed| allowed == id)
        {
            continue;
        }
        for selection in std::iter::once(&request.selection).chain(&request.alternative_selections)
        {
            let trial = arrange_accordion_preset(score, selection.clone(), id);
            let (Some(candidate), Some(config)) = (trial.score, trial.resolved_config) else {
                continue;
            };
            if eligible.iter().any(|(_, known, _)| known == &config) {
                continue;
            }
            eligible.push((*id, config, event_signature(&candidate)));
        }
    }
    if eligible.is_empty() {
        return error(
            fingerprint,
            seed,
            "empty-eligible-pool",
            "no allowed preset can realize this input, harmony and bass mode",
        );
    }
    if let Some(previous) = &request.previous_config {
        eligible.retain(|(_, config, signature)| {
            config != previous && previous_signature.as_ref() != Some(signature)
        });
        if eligible.is_empty() {
            return error(
                fingerprint,
                seed,
                "no-different-result",
                "no eligible preset differs musically from the previous configuration",
            );
        }
    }
    let fingerprint_seed = u64::from_str_radix(
        fingerprint
            .strip_prefix(CONTENT_FINGERPRINT_PREFIX)
            .unwrap_or(&fingerprint),
        16,
    )
    .unwrap_or(0);
    let selected = eligible
        [(splitmix64(parsed_seed ^ fingerprint_seed) % eligible.len() as u64) as usize]
        .1
        .clone();
    let mut result = arrange_from_resolved_preset(score, selected);
    result.seed = seed;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Duration, Note, Pitch};

    fn fixture() -> (Score, AccordionPresetSelection) {
        let mut score = Score::new("preset", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::from_midi(67, false), Duration::Whole)];
        let selection = AccordionPresetSelection {
            chords: vec![PositionedAccordionChord {
                measure_index: 0,
                offset_beats: 0.0,
                chord: serde_json::from_value(serde_json::json!({
                    "root": "C", "kind": "major-seventh", "bass": null
                }))
                .unwrap(),
            }],
            alternative_chords: None,
            inversion: None,
            bass_mode: AccordionBassMode::FreeBass,
        };
        (score, selection)
    }

    #[test]
    fn capability_ids_are_stable_and_unique() {
        let capabilities = accordion_preset_capabilities();
        assert_eq!(capabilities.contract_version, 1);
        assert_eq!(capabilities.presets.len(), 13);
        let ids: std::collections::BTreeSet<_> = capabilities
            .presets
            .iter()
            .map(|preset| preset.id.as_str())
            .collect();
        assert_eq!(ids.len(), capabilities.presets.len());
        assert!(ids.contains("swing-third"));
        assert!(ids.contains("walking-bass"));
    }

    #[test]
    fn resolved_config_roundtrip_reproduces_musical_output() {
        let (score, selection) = fixture();
        let first = arrange_accordion_preset(&score, selection, "swing-third");
        assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
        assert!(first.notated_swing);
        let config: AccordionPresetConfig = serde_json::from_str(
            &serde_json::to_string(first.resolved_config.as_ref().unwrap()).unwrap(),
        )
        .unwrap();
        let replay = arrange_from_resolved_preset(&score, config);
        assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
        assert_eq!(first.resolved_config, replay.resolved_config);
        assert_eq!(first.source_notes, replay.source_notes);
        assert_eq!(first.generated_notes, replay.generated_notes);
        assert_eq!(
            event_signature(first.score.as_ref().unwrap()),
            event_signature(replay.score.as_ref().unwrap())
        );
        assert_eq!(
            score.parts[0].staves[0].measures[0].voices[0][0].pitches[0].to_midi(),
            67
        );
    }

    #[test]
    fn reparsed_equivalent_score_replays_with_new_source_ids() {
        let (first, selection) = fixture();
        let (second, _) = fixture();
        assert_ne!(first.id, second.id);
        assert_ne!(first.parts[0].id, second.parts[0].id);
        assert_ne!(
            first.parts[0].staves[0].measures[0].voices[0][0].id,
            second.parts[0].staves[0].measures[0].voices[0][0].id
        );
        let selected = arrange_accordion_preset(&first, selection, "rising-arpeggio");
        let config = selected.resolved_config.clone().unwrap();
        assert_eq!(fingerprint_version(&config.input_fingerprint), Some(2));
        assert_eq!(config.engine_version, ENGINE_VERSION);
        let mut mismatched_engine = config.clone();
        mismatched_engine.engine_version = LEGACY_ENGINE_VERSION.to_string();
        assert_eq!(
            arrange_from_resolved_preset(&second, mismatched_engine).diagnostics[0].code,
            "unsupported-version"
        );
        let replay = arrange_from_resolved_preset(&second, config);
        assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
        let replayed = replay.score.unwrap();
        assert_eq!(
            replayed.parts[0].staves[0].measures[0].voices[0][0].id,
            second.parts[0].staves[0].measures[0].voices[0][0].id
        );
        assert_eq!(replay.source_notes, selected.source_notes);
        assert_eq!(replay.generated_notes, selected.generated_notes);
        assert_eq!(
            event_signature(&replayed),
            event_signature(selected.score.as_ref().unwrap())
        );
    }

    #[test]
    fn content_fingerprint_rejects_changed_music_and_preserves_legacy_strict_replay() {
        let (first, selection) = fixture();
        let selected = arrange_accordion_preset(&first, selection, "held-chord");
        let config = selected.resolved_config.unwrap();
        let mut variants = Vec::new();
        let (mut changed_pitch, _) = fixture();
        changed_pitch.parts[0].staves[0].measures[0].voices[0][0].pitches[0] =
            Pitch::from_midi(68, false);
        variants.push(changed_pitch);
        let (mut changed_duration, _) = fixture();
        changed_duration.parts[0].staves[0].measures[0].voices[0][0].duration = Duration::Half;
        variants.push(changed_duration);
        let (mut changed_tie, _) = fixture();
        changed_tie.parts[0].staves[0].measures[0].voices[0][0].tie_start = true;
        variants.push(changed_tie);
        let (mut changed_tuplet, _) = fixture();
        changed_tuplet.parts[0].staves[0].measures[0].voices[0][0].tuplet_boundary =
            Some(crate::TupletBoundary::Start);
        variants.push(changed_tuplet);
        let (mut changed_repeat, _) = fixture();
        changed_repeat.parts[0].staves[0].measures[0].barline_right = crate::Barline::RepeatEnd;
        variants.push(changed_repeat);
        let (mut changed_chord, _) = fixture();
        changed_chord.parts[0].staves[0].measures[0].voices[0][0].chord_symbol =
            Some(config.options.chords[0].chord.clone());
        variants.push(changed_chord);
        let (mut changed_setting, _) = fixture();
        changed_setting.settings.tempo_bpm += 1;
        variants.push(changed_setting);
        for changed in variants {
            let replay = arrange_from_resolved_preset(&changed, config.clone());
            assert!(replay.score.is_none());
            assert_eq!(replay.diagnostics[0].code, "input-mismatch");
        }

        let mut legacy = config.clone();
        legacy.engine_version = LEGACY_ENGINE_VERSION.to_string();
        legacy.input_fingerprint = hash_score(&first).unwrap();
        let restored: AccordionPresetConfig =
            serde_json::from_value(serde_json::to_value(&legacy).unwrap()).unwrap();
        assert_eq!(fingerprint_version(&restored.input_fingerprint), Some(1));
        assert!(
            arrange_from_resolved_preset(&first, restored.clone())
                .score
                .is_some()
        );
        let (reparsed, _) = fixture();
        assert_eq!(
            arrange_from_resolved_preset(&reparsed, restored).diagnostics[0].code,
            "input-mismatch"
        );
    }

    #[test]
    fn content_fingerprint_preserves_spanner_addresses_and_id_topology() {
        let (mut first, _) = fixture();
        first.spanners.push(
            serde_json::from_value(serde_json::json!({
                "id": Uuid::new_v4().to_string(),
                "kind": "Slur",
                "start": {"part":0,"staff":0,"measure":0,"voice":0,"note":0},
                "end": {"part":0,"staff":0,"measure":0,"voice":0,"note":0}
            }))
            .unwrap(),
        );
        let mut reparsed = first.clone();
        reparsed.id = Uuid::new_v4().to_string();
        reparsed.parts[0].id = Uuid::new_v4().to_string();
        reparsed.parts[0].staves[0].measures[0].voices[0][0].id = Uuid::new_v4().to_string();
        reparsed.spanners[0].id = Uuid::new_v4().to_string();
        assert_eq!(
            content_fingerprint(&first).unwrap(),
            content_fingerprint(&reparsed).unwrap()
        );
        reparsed.spanners[0].end.note = 1;
        assert_ne!(
            content_fingerprint(&first).unwrap(),
            content_fingerprint(&reparsed).unwrap()
        );

        let (mut two_notes, _) = fixture();
        two_notes.parts[0].staves[0].measures[0].voices[0][0].duration = Duration::Half;
        two_notes.parts[0].staves[0].measures[0].voices[0]
            .push(Note::new(Pitch::from_midi(67, false), Duration::Half));
        let mut aliased = two_notes.clone();
        aliased.parts[0].staves[0].measures[0].voices[0][1].id =
            aliased.parts[0].staves[0].measures[0].voices[0][0]
                .id
                .clone();
        assert_ne!(
            content_fingerprint(&two_notes).unwrap(),
            content_fingerprint(&aliased).unwrap()
        );
    }

    #[test]
    fn seeded_shuffle_is_repeatable_and_excludes_previous_configuration() {
        let (score, selection) = fixture();
        let request = AccordionShuffleOptions {
            selection,
            alternative_selections: Vec::new(),
            allowed_preset_ids: vec!["swing-third".into(), "walking-third".into()],
            seed: "18446744073709551615".into(),
            previous_config: None,
        };
        let first = shuffle_accordion_preset(&score, request.clone());
        let again = shuffle_accordion_preset(&score, request.clone());
        assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
        assert_eq!(first.seed.as_deref(), Some("18446744073709551615"));
        assert_eq!(first.resolved_config, again.resolved_config);
        assert_eq!(
            event_signature(first.score.as_ref().unwrap()),
            event_signature(again.score.as_ref().unwrap())
        );

        let mut different_request = request;
        different_request.previous_config = first.resolved_config.clone();
        let different = shuffle_accordion_preset(&score, different_request.clone());
        assert!(
            different.diagnostics.is_empty(),
            "{:?}",
            different.diagnostics
        );
        assert_ne!(
            first.resolved_config.as_ref().unwrap().preset_id,
            different.resolved_config.as_ref().unwrap().preset_id
        );
        different_request.allowed_preset_ids = vec![first.resolved_config.unwrap().preset_id];
        let singleton = shuffle_accordion_preset(&score, different_request);
        assert!(singleton.score.is_none());
        assert_eq!(singleton.diagnostics[0].code, "no-different-result");
    }

    #[test]
    fn shuffle_rejects_a_different_id_with_identical_music() {
        let (mut score, selection) = fixture();
        score.parts[0].staves[0].measures[0].actual_length =
            super::super::score::MeasureLength::from_beats(1.0);
        score.parts[0].staves[0].measures[0].voices[0][0].duration = Duration::Quarter;
        let previous = arrange_accordion_preset(&score, selection.clone(), "bass-pulse");
        assert!(
            previous.diagnostics.is_empty(),
            "{:?}",
            previous.diagnostics
        );
        let request = AccordionShuffleOptions {
            selection,
            alternative_selections: Vec::new(),
            allowed_preset_ids: vec!["alternating-bass".into()],
            seed: "7".into(),
            previous_config: previous.resolved_config,
        };
        let result = shuffle_accordion_preset(&score, request);
        assert!(result.score.is_none());
        assert_eq!(result.diagnostics[0].code, "no-different-result");
    }

    #[test]
    fn shuffle_selects_an_explicit_alternative_harmony_without_host_combination_enumeration() {
        let (score, mut basic) = fixture();
        basic.chords[0].chord.kind = "major".into();
        let mut color = basic.clone();
        color.chords[0].chord.root = "F".into();
        let previous = arrange_accordion_preset(&score, basic.clone(), "held-chord");
        let request = AccordionShuffleOptions {
            selection: basic,
            alternative_selections: vec![color],
            allowed_preset_ids: vec!["held-chord".into()],
            seed: "42".into(),
            previous_config: previous.resolved_config,
        };
        let result = shuffle_accordion_preset(&score, request);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let config = result.resolved_config.unwrap();
        assert_eq!(config.preset_id, "held-chord");
        assert_eq!(config.options.chords[0].chord.root, "F");
    }

    #[test]
    fn shuffle_reports_empty_pool_and_incompatible_standard_bass() {
        let (score, mut selection) = fixture();
        selection.bass_mode = AccordionBassMode::StandardBass;
        let request = AccordionShuffleOptions {
            selection,
            alternative_selections: Vec::new(),
            allowed_preset_ids: vec!["walking-bass".into(), "swing-pulse".into()],
            seed: "42".into(),
            previous_config: None,
        };
        let incompatible = shuffle_accordion_preset(&score, request.clone());
        assert!(incompatible.score.is_none());
        assert_eq!(incompatible.diagnostics[0].code, "empty-eligible-pool");
        let empty = shuffle_accordion_preset(
            &score,
            AccordionShuffleOptions {
                allowed_preset_ids: vec![],
                ..request.clone()
            },
        );
        assert_eq!(empty.diagnostics[0].code, "empty-eligible-pool");
        let unknown = shuffle_accordion_preset(
            &score,
            AccordionShuffleOptions {
                allowed_preset_ids: vec!["not-a-preset".into()],
                ..request
            },
        );
        assert_eq!(unknown.diagnostics[0].code, "unknown-preset");
    }

    #[test]
    fn replay_rejects_wrong_engine_input_and_tampered_options() {
        let (score, selection) = fixture();
        let valid = arrange_accordion_preset(&score, selection, "walking-bass")
            .resolved_config
            .unwrap();
        let mut wrong_engine = valid.clone();
        wrong_engine.contract_version = 2;
        assert_eq!(
            arrange_from_resolved_preset(&score, wrong_engine).diagnostics[0].code,
            "unsupported-version"
        );
        let mut wrong_input = valid.clone();
        wrong_input.input_fingerprint = "v2:0000000000000000".into();
        assert_eq!(
            arrange_from_resolved_preset(&score, wrong_input).diagnostics[0].code,
            "input-mismatch"
        );
        let mut tampered = valid;
        tampered.options.melodic_voice = AccordionMelodicVoice::ThirdBelow;
        assert_eq!(
            arrange_from_resolved_preset(&score, tampered).diagnostics[0].code,
            "invalid-configuration"
        );
    }
}
