//! The HTML writer over arbitrary bytes, read the way a tool that refuses to
//! reject a file would: invalid UTF-8 becomes U+FFFD. The parse is the same
//! one `parse_lossy` makes; what is checked here is what `to_html_string`
//! writes — that it does not panic and that the markup it produces is
//! well formed (`usfm_fuzz::check_html`).
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    usfm_fuzz::check_html(&String::from_utf8_lossy(data));
});
