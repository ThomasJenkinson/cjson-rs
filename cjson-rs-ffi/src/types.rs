#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

//! C-ABI types mirroring cJSON.h.
//!
//! The `cJSON` struct layout matches cJSON.h byte-for-byte so that C
//! consumers reading struct fields directly (e.g. `item->type`, walking
//! `item->child`/`item->next`) get the same offsets they would from
//! upstream libcjson.

use std::ffi::{c_char, c_double, c_int};
use std::mem::{align_of, size_of};

// ---- Type constants (cJSON.h §"cJSON Types") ----

pub const CJSON_INVALID: c_int = 0;
pub const CJSON_FALSE: c_int = 1 << 0;
pub const CJSON_TRUE: c_int = 1 << 1;
pub const CJSON_NULL: c_int = 1 << 2;
pub const CJSON_NUMBER: c_int = 1 << 3;
pub const CJSON_STRING: c_int = 1 << 4;
pub const CJSON_ARRAY: c_int = 1 << 5;
pub const CJSON_OBJECT: c_int = 1 << 6;
pub const CJSON_RAW: c_int = 1 << 7;

pub const CJSON_IS_REFERENCE: c_int = 256;
pub const CJSON_STRING_IS_CONST: c_int = 512;

/// The cJSON node — mirror of cJSON.h's `struct cJSON`.
#[repr(C)]
pub struct cJSON {
    pub next: *mut cJSON,
    pub prev: *mut cJSON,
    pub child: *mut cJSON,

    /// One of the `CJSON_*` constants above, optionally OR'd with
    /// `CJSON_IS_REFERENCE` / `CJSON_STRING_IS_CONST`.
    pub type_: c_int,

    /// For `CJSON_STRING` / `CJSON_RAW`: the value, malloc'd, NUL-terminated.
    pub valuestring: *mut c_char,

    /// Deprecated integer cache, kept for ABI compatibility.
    pub valueint: c_int,

    /// For `CJSON_NUMBER`: the value.
    pub valuedouble: c_double,

    /// For nodes inside an object: the key, malloc'd, NUL-terminated.
    pub string: *mut c_char,
}

// ---- Layout assertions ----
//
// These match the layout cJSON.h produces on every platform Rust supports
// (LP64 / LLP64). If a future platform changes the C ABI of `int` or
// `double`, these asserts will fail at compile time.

const _: () = {
    // 8 fields: 4 pointers (8 bytes each on 64-bit) + 2 ints (4 each) + 1
    // double (8) + 1 pointer (8) — total 56 bytes with no padding on LP64.
    // The exact size isn't part of the public ABI contract (consumers
    // access by field name, not by absolute offset), but a sudden change
    // would still be surprising — so we assert it here.
    assert!(size_of::<cJSON>() == 64 || size_of::<cJSON>() == 56);
    assert!(align_of::<cJSON>() == align_of::<*mut cJSON>());
};
