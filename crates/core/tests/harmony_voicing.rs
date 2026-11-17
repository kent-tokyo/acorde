use acorde_core::{
    HarmonyRootPolicy, HarmonyVoicingEdge, HarmonyVoicingError, HarmonyVoicingNode,
    HarmonyVoicingRequest, RootVoicingDecision, plan_harmony_voicings,
};

fn node(root: &str, kind: &str, bass: Option<u8>) -> HarmonyVoicingNode {
    HarmonyVoicingNode {
        chord: serde_json::from_value(serde_json::json!({"root":root,"kind":kind})).unwrap(),
        bass_midi: bass,
        melody_ceiling_midi: Some(76),
        accompaniment_floor_midi: None,
    }
}
fn request(nodes: Vec<HarmonyVoicingNode>) -> HarmonyVoicingRequest {
    let edges = (1..nodes.len())
        .map(|to| HarmonyVoicingEdge {
            from: to - 1,
            to,
            repeat_boundary: false,
        })
        .collect();
    HarmonyVoicingRequest {
        nodes,
        edges,
        root_policy: HarmonyRootPolicy::Automatic,
        voices: 4,
        low_midi: 48,
        high_midi: 84,
        center_midi: 60,
        max_leap_semitones: 12,
        parallel_perfect_penalty: 0,
    }
}

#[test]
fn rootless_dm9_requires_a_separate_low_root_and_preserves_guide_tones() {
    let mut request = request(vec![node("D", "minor-ninth", Some(38))]);
    let result = plan_harmony_voicings(&request).unwrap();
    let voicing = &result.voicings[0];
    assert_eq!(
        voicing.root_decision,
        RootVoicingDecision::OmittedWithBassSupport
    );
    let mut names = voicing
        .tones
        .iter()
        .map(|tone| tone.pitch.step.to_char())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(names, ['A', 'C', 'E', 'F']);
    assert!(voicing.tones.iter().all(|tone| tone.degree != 1));
    for bass in [None, Some(36), Some(62)] {
        request.nodes[0].bass_midi = bass;
        let result = plan_harmony_voicings(&request).unwrap();
        assert_eq!(
            result.voicings[0].root_decision,
            RootVoicingDecision::IncludedWithoutBassSupport
        );
        assert!(result.voicings[0].tones.iter().any(|tone| tone.degree == 1));
        assert!(result.voicings[0].tones.iter().any(|tone| tone.degree == 3));
        assert!(result.voicings[0].tones.iter().any(|tone| tone.degree == 7));
    }
    request.root_policy = HarmonyRootPolicy::Omit;
    request.nodes[0].bass_midi = None;
    assert_eq!(
        plan_harmony_voicings(&request).unwrap().voicings[0].root_decision,
        RootVoicingDecision::OmittedByPolicy
    );
}

#[test]
fn every_voice_has_a_bounded_successor_including_the_repeat_return() {
    let mut request = request(vec![
        node("D", "minor-ninth", Some(38)),
        node("G", "dominant-13th", Some(31)),
        node("C", "major-ninth", Some(36)),
    ]);
    request.edges.push(HarmonyVoicingEdge {
        from: 2,
        to: 0,
        repeat_boundary: true,
    });
    let result = plan_harmony_voicings(&request).unwrap();
    assert_eq!(result, plan_harmony_voicings(&request).unwrap());
    assert_eq!(result.transitions.len(), 3);
    assert!(result.transitions[2].edge.repeat_boundary);
    for transition in &result.transitions {
        assert_eq!(transition.voices.len(), 4);
        for (voice, motion) in transition.voices.iter().enumerate() {
            assert_eq!(motion.voice, voice);
            assert!(motion.semitones.unsigned_abs() <= 12);
            assert_eq!(
                i16::from(motion.from_midi),
                result.voicings[transition.edge.from].tones[voice]
                    .pitch
                    .to_midi()
            );
            assert_eq!(
                i16::from(motion.to_midi),
                result.voicings[transition.edge.to].tones[voice]
                    .pitch
                    .to_midi()
            );
        }
    }
    for voicing in &result.voicings {
        assert!(
            voicing
                .tones
                .windows(2)
                .all(|pair| pair[0].pitch.to_midi() < pair[1].pitch.to_midi())
        );
        assert!(
            voicing
                .tones
                .iter()
                .all(|tone| (48..=76).contains(&tone.pitch.to_midi()))
        );
        assert!(voicing.candidates_considered <= 32);
    }
}

#[test]
fn explicit_tensions_and_suspensions_are_not_silently_dropped_to_fit() {
    let mut request = request(vec![node("G", "dominant", Some(31))]);
    request.nodes[0].chord.degrees = serde_json::from_value(serde_json::json!([
        {"value":9,"alter":-1,"kind":"add"},
        {"value":9,"alter":1,"kind":"add"},
        {"value":11,"alter":1,"kind":"add"}]))
    .unwrap();
    assert_eq!(
        plan_harmony_voicings(&request),
        Err(HarmonyVoicingError::NoVoicing { node: 0 })
    );
    request.voices = 5;
    let voicing = plan_harmony_voicings(&request).unwrap().voicings.remove(0);
    assert_eq!(
        voicing.tones.iter().filter(|tone| tone.degree == 9).count(),
        2
    );
    let mut sus = request.clone();
    sus.voices = 4;
    sus.nodes = vec![node("D", "suspended-fourth", Some(38))];
    let voicing = plan_harmony_voicings(&sus).unwrap().voicings.remove(0);
    assert!(voicing.tones.iter().all(|tone| tone.degree != 3));
    assert!(voicing.tones.iter().any(|tone| tone.degree == 4));
}

#[test]
fn no_voicing_and_no_path_are_distinct_atomic_failures() {
    let mut request = request(vec![
        node("C", "major-seventh", None),
        node("Db", "major-seventh", None),
    ]);
    request.max_leap_semitones = 0;
    assert!(matches!(
        plan_harmony_voicings(&request),
        Err(HarmonyVoicingError::NoPath { .. })
    ));
    request.max_leap_semitones = 12;
    request.nodes[0].melody_ceiling_midi = Some(49);
    assert_eq!(
        plan_harmony_voicings(&request),
        Err(HarmonyVoicingError::NoVoicing { node: 0 })
    );
    request.edges[0].to = 2;
    assert_eq!(
        plan_harmony_voicings(&request),
        Err(HarmonyVoicingError::InvalidRequest)
    );
}

#[test]
fn authored_root_omission_and_compound_degrees_survive_register_selection() {
    let mut request = request(vec![node("D", "minor-ninth", None)]);
    request.root_policy = HarmonyRootPolicy::Include;
    request.nodes[0].chord.degrees = serde_json::from_value(serde_json::json!([
        {"value":1,"alter":0,"kind":"subtract"}]))
    .unwrap();
    let result = plan_harmony_voicings(&request).unwrap();
    assert_eq!(
        result.voicings[0].root_decision,
        RootVoicingDecision::OmittedByChord
    );
    assert!(result.voicings[0].tones.iter().any(|tone| tone.degree == 9));
    assert!(result.voicings[0].tones.iter().all(|tone| tone.degree != 1));
}

#[test]
fn sixty_four_harmonic_positions_stay_within_the_search_budget() {
    let mut request = request(vec![node("C", "major-ninth", Some(36)); 64]);
    request.edges.push(HarmonyVoicingEdge {
        from: 63,
        to: 0,
        repeat_boundary: true,
    });
    let started = std::time::Instant::now();
    let result = plan_harmony_voicings(&request).unwrap();
    assert_eq!(result.voicings.len(), 64);
    assert!(result.search_states_evaluated <= 64 * 64 * 32);
    assert!(
        result
            .transitions
            .iter()
            .all(|edge| edge.voices.iter().all(|voice| voice.semitones == 0))
    );
    eprintln!(
        "64 positions: {} states, {:?}",
        result.search_states_evaluated,
        started.elapsed()
    );
}

#[test]
fn style_penalty_counts_parallel_fifths_without_forbidding_them() {
    let mut request = request(vec![node("C", "major", None), node("D", "major", None)]);
    request.root_policy = HarmonyRootPolicy::Include;
    request.voices = 3;
    request.low_midi = 48;
    request.high_midi = 57;
    request.center_midi = 52;
    let free = plan_harmony_voicings(&request).unwrap();
    assert_eq!(free.transitions[0].parallel_perfects, 1);
    assert!(
        free.transitions[0]
            .voices
            .iter()
            .all(|voice| voice.semitones == 2)
    );
    request.parallel_perfect_penalty = 100;
    let penalized = plan_harmony_voicings(&request).unwrap();
    assert_eq!(penalized.voicings, free.voicings);
    assert_eq!(penalized.cost, free.cost + 100);
}
