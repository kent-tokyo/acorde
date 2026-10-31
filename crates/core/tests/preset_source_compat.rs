//! Compile the v1.3.4 public preset struct literals from an external crate.

use acorde_core::{
    AccordionBassMode, AccordionMelodicVoice, AccordionPresetCapabilities, AccordionPresetConfig,
    AccordionSwingPolicy, LayeredAccordionOptions,
};

#[test]
fn v1_3_4_preset_literals_remain_source_compatible() {
    let options = LayeredAccordionOptions {
        chords: Vec::new(),
        alternative_chords: None,
        inversion: None,
        accompaniment: None,
        melodic_voice: AccordionMelodicVoice::Unchanged,
        swing: AccordionSwingPolicy::Straight,
        bass_mode: AccordionBassMode::FreeBass,
    };
    let config = AccordionPresetConfig {
        contract_version: 1,
        engine_version: String::new(),
        input_fingerprint: String::new(),
        preset_id: String::new(),
        options,
    };
    let capabilities = AccordionPresetCapabilities {
        contract_version: 1,
        engine_version: String::new(),
        source_profile: String::new(),
        max_shuffled_measures: 0,
        max_shuffled_chords: 0,
        max_shuffled_pool: 0,
        max_shuffled_source_notes: 0,
        max_shuffled_selections: 0,
        presets: Vec::new(),
    };
    assert!(config.input_fingerprint.is_empty());
    assert!(capabilities.presets.is_empty());
}
