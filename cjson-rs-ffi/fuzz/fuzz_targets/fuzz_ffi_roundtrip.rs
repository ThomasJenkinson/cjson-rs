#![no_main]
//! Fuzz the full FFI round-trip: Parse → Print → Delete.
//!
//! This exercises:
//! - cJSON_Parse + the unsafe NUL-terminated byte handling
//! - value_to_cjson + tree allocation
//! - print_cjson_pretty (cJSON-compatible formatter)
//! - cJSON_Delete + recursive free of the tree
//!
//! Any leak, double-free, or use-after-free would be caught by AddressSanitizer
//! (enabled by default in cargo-fuzz). Panics escape to libfuzzer.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The FFI takes a NUL-terminated string; require fuzz input to be
    // suitable as a C string. Reject inputs with embedded NULs.
    if data.contains(&0) {
        return;
    }
    // Build a NUL-terminated copy on the stack-ish (Vec on heap).
    let mut buf = Vec::with_capacity(data.len() + 1);
    buf.extend_from_slice(data);
    buf.push(0);

    unsafe {
        let root = cjson::cJSON_Parse(buf.as_ptr() as *const std::ffi::c_char);
        if !root.is_null() {
            let printed = cjson::cJSON_Print(root);
            if !printed.is_null() {
                cjson::cJSON_free(printed as *mut std::ffi::c_void);
            }
            let printed_compact = cjson::cJSON_PrintUnformatted(root);
            if !printed_compact.is_null() {
                cjson::cJSON_free(printed_compact as *mut std::ffi::c_void);
            }
            cjson::cJSON_Delete(root);
        }
    }
});
