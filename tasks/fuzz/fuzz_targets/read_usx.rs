//! Arbitrary bytes, read the way `parse_lossy` reads them, as USX: the reader
//! does not panic, keeps its spans in its source, and builds a tree the writer
//! turns into well-formed USX (ticket 49). The seeds are real USX, so the
//! mutations start from the vocabulary rather than from nothing.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    usfm_fuzz::check_read_usx(&String::from_utf8_lossy(data));
});
