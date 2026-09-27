//! Arbitrary bytes, read the way `parse_lossy` reads them, as USFM: parse,
//! write USX, read it back, and the tree is the parse's up to what USX cannot
//! say; then the read tree round-trips through USFM as `roundtrip` asks
//! (ticket 49).
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    usfm_fuzz::check_usx_roundtrip(&String::from_utf8_lossy(data));
});
