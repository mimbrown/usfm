//! Arbitrary bytes that already are valid UTF-8; anything else is skipped.
//! Slower to make progress than `parse_lossy`, but the source here is the
//! exact bytes libFuzzer chose, so a span is checked against them rather than
//! against a repaired copy.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(source) = std::str::from_utf8(data) {
        usfm_fuzz::check_source(source);
    }
});
