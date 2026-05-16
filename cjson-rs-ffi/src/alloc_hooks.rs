//! Allocator-hook indirection.
//!
//! cJSON.h exposes `cJSON_InitHooks` which lets a caller substitute their
//! own `malloc`/`free` for cJSON's internal allocations. Routing every
//! allocation through these hooks lets callers — and the upstream test
//! suite — inject failing allocators to exercise error paths.
//!
//! Implementation: two `AtomicPtr<()>` slots, lazily-initialised to
//! `libc::malloc` / `libc::free` on first use. `cJSON_InitHooks` atomic-
//! stores caller-supplied pointers (or reverts to defaults if the caller
//! passes NULL).
//!
//! The fn-pointer-via-AtomicPtr pattern is sound: `extern "C" fn(...)`
//! pointers are thin, single-word, and round-trip through `*mut ()`
//! losslessly via `transmute`. Ordering is `Relaxed` — readers see
//! either the old or the new hook on each call, never a torn value.

use libc::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};

pub type MallocFn = unsafe extern "C" fn(usize) -> *mut c_void;
pub type FreeFn = unsafe extern "C" fn(*mut c_void);

static MALLOC_HOOK: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());
static FREE_HOOK: AtomicPtr<()> = AtomicPtr::new(std::ptr::null_mut());

/// Install caller-supplied allocator functions. Passing a NULL function
/// pointer (or for either slot) reverts that slot to the libc default.
pub fn set_hooks(malloc: Option<MallocFn>, free: Option<FreeFn>) {
    let m: *mut () = malloc.map_or(std::ptr::null_mut(), |f| f as *mut ());
    let f: *mut () = free.map_or(std::ptr::null_mut(), |f| f as *mut ());
    MALLOC_HOOK.store(m, Ordering::Relaxed);
    FREE_HOOK.store(f, Ordering::Relaxed);
}

/// Allocate `size` bytes via the current hook (defaults to `libc::malloc`).
pub unsafe fn hook_malloc(size: usize) -> *mut c_void {
    let p = MALLOC_HOOK.load(Ordering::Relaxed);
    if p.is_null() {
        libc::malloc(size)
    } else {
        let f: MallocFn = std::mem::transmute(p);
        f(size)
    }
}

/// Free `ptr` via the current hook (defaults to `libc::free`). NULL-safe.
pub unsafe fn hook_free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let p = FREE_HOOK.load(Ordering::Relaxed);
    if p.is_null() {
        libc::free(ptr);
    } else {
        let f: FreeFn = std::mem::transmute(p);
        f(ptr);
    }
}
