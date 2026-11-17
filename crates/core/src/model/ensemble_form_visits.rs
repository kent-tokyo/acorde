//! Performance-order variation of generated form parts on repeat visits.
//! The written score and its source parts stay available separately.

use serde::{Deserialize, Serialize};

use super::chord_proposals::fingerprint;
use super::ensemble_form::{EnsembleFormDiagnostic, EnsembleFormRequest, arrange_ensemble_form};
use super::notation::Barline;
use super::playback::{PlaybackEvent, PlaybackOptions, to_playback_events};
use super::repeat::measure_sequence;
use super::score::Score;

pub const FORM_VISITS_CONTRACT_VERSION: u16 = 1;
pub const FORM_VISITS_ENGINE_VERSION: &str =
    concat!(env!("CARGO_PKG_VERSION"), "+ensemble-form-visits.1");
const MAX_VARIANTS: usize = 8;
const MAX_VISITS: usize = 2048;
const MAX_EVENTS: usize = 250_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormVisitVariation {
    /// Zero-based occurrence in playback order, including repeats/endings.
    pub visit_index: usize,
    /// Complete alternate written plan; only this visit's generated events
    /// replace the base plan's generated events.
    pub alternative: EnsembleFormRequest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnsembleFormVisitsRequest {
    pub base: EnsembleFormRequest,
    pub variations: Vec<FormVisitVariation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedFormVisits {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_fingerprint: String,
    pub request: EnsembleFormVisitsRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleFormVisitsResult {
    /// The base written score is suitable for notation and ordinary export.
    pub written_score: Option<Score>,
    /// Playback-order events include visit-specific generated variations.
    pub events: Vec<PlaybackEvent>,
    pub written_measures_by_visit: Vec<usize>,
    pub resolved: Option<ResolvedFormVisits>,
    pub diagnostics: Vec<EnsembleFormDiagnostic>,
}

/// An unfolded score whose written bars follow the selected playback visits.
/// The original source and the compact written base remain separate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleFormPerformanceScoreResult {
    pub score: Option<Score>,
    pub written_measures_by_visit: Vec<usize>,
    pub resolved: Option<ResolvedFormVisits>,
    pub diagnostics: Vec<EnsembleFormDiagnostic>,
}

fn score_fail(code: &str, visit: Option<usize>) -> EnsembleFormPerformanceScoreResult {
    EnsembleFormPerformanceScoreResult {
        score: None,
        written_measures_by_visit: Vec::new(),
        resolved: None,
        diagnostics: vec![issue(code, visit)],
    }
}

pub(super) fn same_sounding_event(left: &PlaybackEvent, right: &PlaybackEvent) -> bool {
    let close = |a: f64, b: f64| a.is_finite() && b.is_finite() && (a - b).abs() < 1e-8;
    if !close(left.time_beats, right.time_beats)
        || !close(left.time_secs, right.time_secs)
        || !close(left.duration_beats, right.duration_beats)
        || !close(left.duration_secs, right.duration_secs)
    {
        return false;
    }
    let mut left = left.clone();
    let mut right = right.clone();
    for item in [&mut left, &mut right] {
        item.address = None;
        item.source = None;
        item.time_beats = 0.0;
        item.time_secs = 0.0;
        item.duration_beats = 0.0;
        item.duration_secs = 0.0;
    }
    left == right
}

pub(super) fn unfolded_measure(
    written_score: &Score,
    written_staff: &super::score::Staff,
    written: &super::score::Measure,
    physical: usize,
    visit: usize,
    part: usize,
    staff: usize,
) -> Option<super::score::Measure> {
    let mut bar = written.clone();
    bar.number = u32::try_from(visit + 1).ok()?;
    bar.barline_left = Barline::Normal;
    bar.barline_right = Barline::Normal;
    bar.volta = None;
    bar.navigation = None;
    bar.measure_repeat = None;
    bar.multi_rest_count = None;
    bar.time_sig = Some(
        written_staff
            .meter_at(physical, &written_score.settings.time_signature)
            .clone(),
    );
    bar.key_sig = written_staff
        .measures
        .get(..=physical)?
        .iter()
        .rev()
        .find_map(|measure| measure.key_sig.clone())
        .or_else(|| Some(written_score.settings.key_signature.clone()));
    let mut clef = written_staff.clef.clone();
    for prior in written_staff.measures.get(..physical)? {
        if let Some(change) = &prior.clef {
            clef = change.clone();
        }
        clef = prior.clef_at(&clef, f64::MAX);
    }
    bar.clef = Some(written.clef.clone().unwrap_or(clef));
    for (voice_index, voice) in bar.voices.iter_mut().enumerate() {
        for (note_index, note) in voice.iter_mut().enumerate() {
            note.id = format!("performance:{visit}:{part}:{staff}:{voice_index}:{note_index}");
        }
    }
    Some(bar)
}

/// Unfold repeats into a notation/export score, choosing each generated
/// measure from the base or its visit-specific alternative. Accept the result
/// only if its complete playback matches the source-bound event contract.
pub fn arrange_ensemble_form_visits_score(
    source: &Score,
    request: &EnsembleFormVisitsRequest,
) -> EnsembleFormPerformanceScoreResult {
    let selected = arrange_ensemble_form_visits(source, request);
    if let Some(diagnostic) = selected.diagnostics.first() {
        return EnsembleFormPerformanceScoreResult {
            score: None,
            written_measures_by_visit: Vec::new(),
            resolved: None,
            diagnostics: vec![diagnostic.clone()],
        };
    }
    if !source.spanners.is_empty()
        || !source.views.is_empty()
        || !source.object_style_overrides.is_empty()
    {
        return score_fail("unsupported-form-performance-refs", None);
    }
    let Some(base) = selected.written_score.as_ref() else {
        return score_fail("base-form-failed", None);
    };
    let mut variants = Vec::with_capacity(request.variations.len());
    for variation in &request.variations {
        let built = arrange_ensemble_form(source, &variation.alternative);
        let Some(score) = built.score else {
            return score_fail("variant-form-failed", Some(variation.visit_index));
        };
        if score.parts.len() != base.parts.len()
            || score
                .parts
                .iter()
                .zip(&base.parts)
                .any(|(left, right)| left.staves.len() != right.staves.len())
        {
            return score_fail("form-visit-part-mismatch", Some(variation.visit_index));
        }
        variants.push((variation.visit_index, score));
    }
    let mut performance = base.clone();
    for (part_index, part) in performance.parts.iter_mut().enumerate() {
        for (staff_index, staff) in part.staves.iter_mut().enumerate() {
            let mut measures = Vec::with_capacity(selected.written_measures_by_visit.len());
            let mut prior_meter = None;
            let mut prior_key = None;
            let mut prior_clef = staff.clef.clone();
            for (visit, &physical) in selected.written_measures_by_visit.iter().enumerate() {
                let chosen = if part_index < source.parts.len() {
                    base
                } else {
                    variants
                        .iter()
                        .find(|(index, _)| *index == visit)
                        .map_or(base, |(_, score)| score)
                };
                let Some(written_staff) = chosen
                    .parts
                    .get(part_index)
                    .and_then(|part| part.staves.get(staff_index))
                else {
                    return score_fail("form-visit-part-mismatch", Some(visit));
                };
                let Some(written) = written_staff.measures.get(physical) else {
                    return score_fail("form-visit-part-mismatch", Some(visit));
                };
                let Some(mut bar) = unfolded_measure(
                    chosen,
                    written_staff,
                    written,
                    physical,
                    visit,
                    part_index,
                    staff_index,
                ) else {
                    return score_fail("form-performance-unfold-failed", Some(visit));
                };
                let effective_meter = bar.time_sig.clone();
                let effective_key = bar.key_sig.clone();
                let effective_clef = bar.clef.clone().unwrap_or_else(|| prior_clef.clone());
                if visit > 0 && effective_meter == prior_meter {
                    bar.time_sig = None;
                }
                if visit > 0 && effective_key == prior_key {
                    bar.key_sig = None;
                }
                if effective_clef == prior_clef {
                    bar.clef = None;
                }
                prior_clef = bar.clef_at(&effective_clef, f64::MAX);
                prior_meter = effective_meter;
                prior_key = effective_key;
                measures.push(bar);
            }
            staff.measures = measures;
        }
    }
    if !super::validate::validate(&performance).is_valid() {
        return score_fail("invalid-form-performance-score", None);
    }
    let actual = to_playback_events(&performance, &PlaybackOptions::default());
    if actual.len() != selected.events.len()
        || !actual
            .iter()
            .zip(&selected.events)
            .all(|(left, right)| same_sounding_event(left, right))
    {
        return score_fail("form-performance-playback-mismatch", None);
    }
    EnsembleFormPerformanceScoreResult {
        score: Some(performance),
        written_measures_by_visit: selected.written_measures_by_visit,
        resolved: selected.resolved,
        diagnostics: Vec::new(),
    }
}

/// Rebuild a saved performance score against the original source and engine.
pub fn replay_ensemble_form_visits_score(
    source: &Score,
    resolved: &ResolvedFormVisits,
) -> EnsembleFormPerformanceScoreResult {
    if resolved.contract_version != FORM_VISITS_CONTRACT_VERSION
        || resolved.engine_version != FORM_VISITS_ENGINE_VERSION
    {
        return score_fail("unsupported-form-visits-contract", None);
    }
    if fingerprint(source).as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return score_fail("form-source-mismatch", None);
    }
    arrange_ensemble_form_visits_score(source, &resolved.request)
}

fn issue(code: &str, visit: Option<usize>) -> EnsembleFormDiagnostic {
    EnsembleFormDiagnostic {
        code: code.into(),
        section_index: None,
        measure_index: visit,
        role: None,
    }
}

fn fail(diagnostic: EnsembleFormDiagnostic) -> EnsembleFormVisitsResult {
    EnsembleFormVisitsResult {
        written_score: None,
        events: Vec::new(),
        written_measures_by_visit: Vec::new(),
        resolved: None,
        diagnostics: vec![diagnostic],
    }
}

fn visit_starts(score: &Score, sequence: &[usize]) -> Option<Vec<f64>> {
    let mut starts = Vec::with_capacity(sequence.len() + 1);
    starts.push(0.0);
    for &measure in sequence {
        let length = score.playback_measure_beats(measure);
        if !length.is_finite() || length <= 0.0 {
            return None;
        }
        let next = starts.last()? + length;
        if !next.is_finite() {
            return None;
        }
        starts.push(next);
    }
    Some(starts)
}

fn events_by_visit<'a>(
    events: &'a [PlaybackEvent],
    sequence: &[usize],
    starts: &[f64],
) -> Option<Vec<Vec<&'a PlaybackEvent>>> {
    if events.len() > MAX_EVENTS {
        return None;
    }
    let mut groups = vec![Vec::new(); sequence.len()];
    for event in events {
        let source = event.source.as_ref()?;
        if !event.time_beats.is_finite() || event.time_beats < 0.0 {
            return None;
        }
        let visit = starts
            .partition_point(|start| *start <= event.time_beats + 1e-9)
            .saturating_sub(1);
        if visit >= sequence.len()
            || source.measure != sequence[visit]
            || event.time_beats >= starts[visit + 1] - 1e-9
        {
            return None;
        }
        groups[visit].push(event);
    }
    Some(groups)
}

fn source_events(events: &[PlaybackEvent], source_parts: usize) -> Vec<&PlaybackEvent> {
    events
        .iter()
        .filter(|event| event.part_index < source_parts)
        .collect()
}

fn same_parts(base: &Score, alternate: &Score) -> bool {
    base.parts.len() == alternate.parts.len()
        && base
            .parts
            .iter()
            .zip(&alternate.parts)
            .all(|(left, right)| {
                left.midi_channel == right.midi_channel && left.midi_program == right.midi_program
            })
}

fn within_event_budget(score: &Score, sequence: &[usize]) -> bool {
    // Measured tremolo can realize multiple attacks from one written note.
    // Check a conservative bound before allocating the expanded event list.
    let mut written = 0usize;
    for &measure in sequence {
        for part in &score.parts {
            for staff in &part.staves {
                let Some(bar) = staff.measures.get(measure) else {
                    return false;
                };
                for voice in &bar.voices {
                    written = voice.iter().fold(written, |sum, note| {
                        sum.saturating_add(note.pitches.len().max(1))
                    });
                    if written > MAX_EVENTS / 32 {
                        return false;
                    }
                }
            }
        }
    }
    true
}

/// Generate playback-order variations while retaining a score for the written
/// arrangement. Reject timing profiles that cannot be mapped unambiguously to
/// physical measure visits; no partial event list is returned.
pub fn arrange_ensemble_form_visits(
    score: &Score,
    request: &EnsembleFormVisitsRequest,
) -> EnsembleFormVisitsResult {
    if request.variations.is_empty() || request.variations.len() > MAX_VARIANTS {
        return fail(issue("form-visit-limit", None));
    }
    let sequence = measure_sequence(score);
    if sequence.is_empty() || sequence.len() > MAX_VISITS {
        return fail(issue("form-visit-limit", None));
    }
    let mut previous = None;
    for variation in &request.variations {
        if variation.visit_index >= sequence.len()
            || previous.is_some_and(|index| index >= variation.visit_index)
            || !sequence[..variation.visit_index].contains(&sequence[variation.visit_index])
        {
            return fail(issue("invalid-form-visit", Some(variation.visit_index)));
        }
        previous = Some(variation.visit_index);
    }
    let base = arrange_ensemble_form(score, &request.base);
    let Some(written) = base.score else {
        return fail(
            base.diagnostics
                .into_iter()
                .next()
                .unwrap_or_else(|| issue("base-form-failed", None)),
        );
    };
    if !within_event_budget(&written, &sequence) {
        return fail(issue("form-visit-event-limit", None));
    }
    let starts = match visit_starts(score, &sequence) {
        Some(starts) => starts,
        None => return fail(issue("unsupported-form-visit-timing", None)),
    };
    let base_events = to_playback_events(&written, &PlaybackOptions::default());
    let Some(base_groups) = events_by_visit(&base_events, &sequence, &starts) else {
        return fail(issue("unsupported-form-visit-timing", None));
    };
    let mut selected: Vec<Vec<PlaybackEvent>> = base_groups
        .iter()
        .map(|group| group.iter().map(|event| (*event).clone()).collect())
        .collect();
    let source_parts = score.parts.len();
    let base_source = source_events(&base_events, source_parts);
    for variation in &request.variations {
        let alternative = arrange_ensemble_form(score, &variation.alternative);
        let Some(variant) = alternative.score else {
            return fail(
                alternative
                    .diagnostics
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| issue("variant-form-failed", Some(variation.visit_index))),
            );
        };
        if !same_parts(&written, &variant) {
            return fail(issue(
                "form-visit-part-mismatch",
                Some(variation.visit_index),
            ));
        }
        if !within_event_budget(&variant, &sequence) {
            return fail(issue("form-visit-event-limit", Some(variation.visit_index)));
        }
        let alternative_events = to_playback_events(&variant, &PlaybackOptions::default());
        if source_events(&alternative_events, source_parts) != base_source {
            return fail(issue(
                "form-visit-source-mismatch",
                Some(variation.visit_index),
            ));
        }
        let Some(alternative_groups) = events_by_visit(&alternative_events, &sequence, &starts)
        else {
            return fail(issue(
                "unsupported-form-visit-timing",
                Some(variation.visit_index),
            ));
        };
        let visit = variation.visit_index;
        let source_events = selected[visit]
            .iter()
            .filter(|event| event.part_index < source_parts)
            .cloned()
            .collect::<Vec<_>>();
        let variant_generated = alternative_groups[visit]
            .iter()
            .filter(|event| event.part_index >= source_parts)
            .map(|event| (*event).clone())
            .collect::<Vec<_>>();
        let base_generated = selected[visit]
            .iter()
            .filter(|event| event.part_index >= source_parts)
            .cloned()
            .collect::<Vec<_>>();
        if variant_generated == base_generated {
            return fail(issue("form-visit-unchanged", Some(visit)));
        }
        selected[visit] = source_events.into_iter().chain(variant_generated).collect();
    }
    let mut events = selected.into_iter().flatten().collect::<Vec<_>>();
    events.sort_by(|left, right| {
        left.time_beats
            .total_cmp(&right.time_beats)
            .then(left.part_index.cmp(&right.part_index))
            .then(left.pitch_midi.cmp(&right.pitch_midi))
    });
    let Some(source_fingerprint) = fingerprint(score) else {
        return fail(issue("invalid-form-source", None));
    };
    EnsembleFormVisitsResult {
        written_score: Some(written),
        events,
        written_measures_by_visit: sequence,
        resolved: Some(ResolvedFormVisits {
            contract_version: FORM_VISITS_CONTRACT_VERSION,
            engine_version: FORM_VISITS_ENGINE_VERSION.into(),
            source_fingerprint,
            request: request.clone(),
        }),
        diagnostics: Vec::new(),
    }
}

/// Rebuild a saved visit schedule against its original source and engine.
pub fn replay_ensemble_form_visits(
    score: &Score,
    resolved: &ResolvedFormVisits,
) -> EnsembleFormVisitsResult {
    if resolved.contract_version != FORM_VISITS_CONTRACT_VERSION
        || resolved.engine_version != FORM_VISITS_ENGINE_VERSION
    {
        return fail(issue("unsupported-form-visits-contract", None));
    }
    if fingerprint(score).as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return fail(issue("form-source-mismatch", None));
    }
    arrange_ensemble_form_visits(score, &resolved.request)
}
