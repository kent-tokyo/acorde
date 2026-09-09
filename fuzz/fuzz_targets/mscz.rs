#![no_main]

use acorde_io::parse_mscz_with_report;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| {
    let _ = parse_mscz_with_report(input);
});
