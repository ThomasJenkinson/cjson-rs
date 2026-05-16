#![no_main]
//! Fuzz the safe Rust parser directly. Every input is a byte slice;
//! the parser must either return a Value or an Error, never panic
//! and never UB. The safe core is `#![forbid(unsafe_code)]`, so the
//! only way this target can fail is via panic (which libfuzzer reports).

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = cjson_rs::parse(data);
});
