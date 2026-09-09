#![no_main]

use acorde_core::{Score, validate};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| {
    if input.len() > 16 * 1024 * 1024 {
        return;
    }

    if let Ok(score) = serde_json::from_slice::<Score>(input) {
        let _ = validate(&score);
    }
});
