//! Arbitrary bytes, read the way a tool that refuses to reject a file would:
//! invalid UTF-8 becomes U+FFFD. Every input reaches the parser, so the
//! fuzzer's bytes are never wasted, and the replacement characters exercise
//! the multi-byte paths in the lexer.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    usfm_fuzz::check_source(&String::from_utf8_lossy(data));
});
