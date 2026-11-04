//! Connect positioned harmony to an explicit standard-bass instrument.
use super::{
    arrange::AccordionBassMode,
    chord_plan::{ChordPlan, ChordPlanDiagnostic, chord_plan_to_preset_selection},
    standard_bass::{
        StandardBassProfile, standard_bass_candidates, standard_bass_request_for_chord,
    },
    standard_bass_preset::ProfiledBassPulseOptions,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfiledChordPlanResult {
    /// Present only when every entry has an eligible button pair.
    pub options: Option<ProfiledBassPulseOptions>,
    pub diagnostics: Vec<ChordPlanDiagnostic>,
    pub requires_performer_review: bool,
}

/// Check harmony and positions, not score coverage or physical playability.
/// No score is generated; a candidate pair models simultaneous sounding pitches.
pub fn profile_chord_plan(
    plan: &ChordPlan,
    profile: &StandardBassProfile,
    allow_chord_omissions: bool,
    allow_chord_additions: bool,
) -> ProfiledChordPlanResult {
    if plan.entries.len() > 512 {
        return ProfiledChordPlanResult {
            options: None,
            diagnostics: vec![ChordPlanDiagnostic {
                code: "plan-entry-limit".into(),
                measure_index: None,
                source: None,
            }],
            requires_performer_review: true,
        };
    }
    let result = chord_plan_to_preset_selection(plan, AccordionBassMode::FreeBass);
    let mut diagnostics = result.diagnostics;
    if diagnostics.is_empty() {
        for entry in &plan.entries {
            let code = match standard_bass_request_for_chord(
                &entry.chord,
                allow_chord_omissions,
                allow_chord_additions,
            ) {
                Err(_) => Some("unsupported-standard-bass-chord"),
                Ok(request) => match standard_bass_candidates(profile, &request) {
                    Err(_) => Some("invalid-standard-bass-profile"),
                    Ok(candidates)
                        if !candidates
                            .iter()
                            .any(super::standard_bass_preset::candidate_fits_generated_range) =>
                    {
                        Some("no-standard-bass-candidate")
                    }
                    Ok(_) => None,
                },
            };
            if let Some(code) = code {
                diagnostics.push(ChordPlanDiagnostic {
                    code: code.into(),
                    measure_index: Some(entry.position.measure_index),
                    source: entry.sources.first().and_then(|source| source.note.clone()),
                });
            }
        }
    }
    let options = if diagnostics.is_empty() {
        result.selection.map(|mut selection| {
            selection.bass_mode = AccordionBassMode::StandardBass;
            ProfiledBassPulseOptions {
                selection,
                profile: profile.clone(),
                allow_chord_omissions,
                allow_chord_additions,
            }
        })
    } else {
        None
    };
    ProfiledChordPlanResult {
        options,
        diagnostics,
        requires_performer_review: true,
    }
}
