//! Panic containment at the FFI boundary.
//!
//! Rust unwinding across an `extern "C"` boundary is undefined behaviour
//! on most platforms. Every public FFI function in this crate runs its
//! body inside `catch_unwind` so that a panic from the safe Rust core
//! becomes a clean error return instead of unwinding into C frames.
//!
//! On panic, the function returns the caller-supplied "panic value"
//! (typically `NULL` or `0`), matching cJSON's "return NULL on failure"
//! convention.

use std::panic::{catch_unwind, AssertUnwindSafe};

/// Run `f` inside `catch_unwind`. On panic, return `on_panic`.
///
/// `f` is wrapped in `AssertUnwindSafe` because all FFI bodies operate
/// on caller-owned C pointers — we have no Rust state that could be
/// left in an inconsistent observable state by an unwind.
pub(crate) fn guard<T, F>(on_panic: T, f: F) -> T
where
    F: FnOnce() -> T,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(_) => on_panic,
    }
}
