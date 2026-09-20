//! Arbitrary bytes, read the way `parse_lossy` reads them, and then round-
//! tripped: parse, write USFM, parse again. The property is idempotence on
//! *any* input — the recovered tree, whatever the recovery rules made of it,
//! must survive being written out and read back (ticket 27).
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    usfm_fuzz::check_roundtrip(&String::from_utf8_lossy(data));
});
