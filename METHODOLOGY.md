# cjson-rs — Methodology

## Clean-room declaration

This implementation is built from the JSON specification ([RFC 8259](https://datatracker.ietf.org/doc/html/rfc8259), December 2017), not from cJSON's C source. The parser, the value model, and the serialiser are written in Rust without reference to `cJSON.c`.

The C ABI shim layer (`cjson-rs-ffi`) is the only part of the codebase that follows cJSON's published interface. It mirrors the `cJSON.h` function signatures and the `cJSON` struct layout exactly, so that existing C consumers can link against `libcjson.{so,dylib}` produced by this project without recompiling. The shim layer reads `cJSON.h` only as an API specification — never `cJSON.c`.

## Verification

Four independent layers validate the implementation:

1. **An RFC 8259-driven Rust test suite** (`cjson-rs/tests/`), written from the specification's grammar productions. Each test cites the RFC section it derives from. 112 tests across tokeniser, parser, and serialiser, all spec-cited.
2. **Rust-side FFI smoke tests** (`cjson-rs-ffi/tests/ffi_smoke.rs`), exercising every public `cJSON_*` function through `unsafe` Rust calls.
3. **C-side smoke tests** (`cjson-rs-ffi/tests/c/smoke.c`), compiled with the system `cc` and linked against `libcjson.dylib`. Proves real C consumers can use the shim.
4. **The upstream cJSON test suite** (`tests/*.c` from [DaveGamble/cJSON](https://github.com/DaveGamble/cJSON), included as a git submodule), linked against `libcjson.dylib` produced by `cjson-rs-ffi` instead of upstream's own build. A shim `common.h` substitutes the public API for upstream's internal helpers so the tests genuinely exercise our shim. **100% pass rate (65/65)** on the public-API subset.

The fuzzing harness (`cargo-fuzz` with libfuzzer + AddressSanitizer) reuses the corpus from the upstream `fuzzing/inputs/` directory. Two targets: one for the safe Rust parser, one for the full FFI round-trip (Parse → Print → Delete). ~4.5M iterations across both with no crashes — this is a baseline; extended multi-day runs are planned as part of the funded work.

## What is and isn't borrowed

| Borrowed from cJSON | Re-derived from RFC 8259 |
|---|---|
| The `cJSON.h` header signatures (for ABI compatibility) | Parser logic |
| The `cJSON` struct field layout (for memory-layout compatibility) | Value representation |
| The upstream test suite (used as a verification oracle) | Serialiser logic |
| The fuzz corpus | Error handling |
| The exact byte format of `cJSON_Print` output (derived from observation, not source) | Internal allocation strategy |

No code from `cJSON.c` or `cJSON_Utils.c` is read, copied, transpiled, or AI-translated.

## Safety architecture

- **`cjson-rs`** — the safe Rust core. Marked `#![forbid(unsafe_code)]` at the crate level, so neither the current code nor any future contributor can introduce an unsafe block. Implements the parser, value model, and serialiser.
- **`cjson-rs-ffi`** — the C ABI shim. The *only* crate in the project containing `unsafe` code. Validated under `cargo-miri` (34 FFI tests pass under miri with zero undefined behaviour reported).
- **Panic containment** — every FFI entry point runs its body inside `std::panic::catch_unwind`, so a panic from the safe core becomes a documented failure return (NULL / 0 / false) rather than unwinding into C frames (which is UB on most platforms).
- **Memory model** — all cJSON nodes and their owned strings are allocated via the `cJSON_Hooks` allocator (defaults to `libc::malloc` / `libc::free`), so callers can free them with stdlib `free()` or via `cJSON_free`, matching cJSON's documented behaviour.
- **Struct layout** — the `cJSON` struct in `types.rs` is `#[repr(C)]` with field-by-field correspondence to `cJSON.h`. C consumers that walk `item->child` / `->next` / `->string` directly (the dominant cJSON idiom) get the same offsets they would from upstream.

## CVE classes eliminated by construction

| Recent cJSON CVE | Bug class | Why Rust prevents it |
|---|---|---|
| CVE-2024-31755 (`cJSON_SetValuestring` null deref) | null pointer dereference | All FFI entry points NULL-check before deref |
| CVE-2023-50472 (`cJSON_SetValuestring` null reference) | null pointer dereference | Same |
| CVE-2023-50471 (`cJSON_InsertItemInArray` null reference) | null pointer dereference + bounds | Same; bounds-checked iteration |
| Type confusion in `cJSON_Utils` (most recent upstream fix) | missing type check | Type tag is verified before reading `valuestring`/`valuedouble`/`child` |
| Multiple heap buffer overflows in parsing | unchecked write | Safe Rust core uses slice bounds; no raw pointer arithmetic |
| `realloc` failure paths returning bad pointers | use-after-free | Box/Vec semantics + no realloc in this codebase |
