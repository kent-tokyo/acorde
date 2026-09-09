#![no_main]

use acorde_io::parse_musicxml_with_report;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| {
    if let Ok(text) = std::str::from_utf8(input) {
        let _ = parse_musicxml_with_report(text);
    }
});
