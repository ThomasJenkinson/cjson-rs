# Changelog

All notable changes to this project are recorded here.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html) (with the usual pre-1.0 caveat that breaking changes can land in any minor version while we're in `0.x`).

## [0.2.0] — 2026-05-19

### Added

- `cJSON_PrintPreallocated` is now a real implementation. Prints the value into the caller-supplied buffer; returns `true` on success, `false` on NULL inputs, non-positive length, or undersized buffer. Brings the FFI surface from 77/78 to 78/78 cJSON.h functions implemented.
- `cjson_rs::parse_prefix` — new public Rust API. Parses a single JSON value from the prefix of a buffer that may contain trailing whitespace, garbage, or further values. Returns the value plus the byte offset immediately after its last consumed byte.

### Changed

- `cJSON_ParseWithOpts` and `cJSON_ParseWithLengthOpts` now honour the `require_null_terminated` flag. When `false`, the parser accepts valid-prefix-then-trailing-bytes input (the documented headline use case of `WithOpts`) and reports `parse_end` correctly. Previously the flag was ignored and trailing bytes were always rejected.
- Upstream cJSON conformance: **64/65 → 65/65 (100%)** on the public-API subset.

### Documentation

- README and METHODOLOGY now reflect 100% conformance and 78/78 functions.
- Fuzz claim reframed as a baseline (~4.5M iterations is modest; extended multi-day runs are planned with funded work).
- `cjson-rs-ffi/src/panic_guard.rs` documents the invariant that the safe core must remain stateless per-parse for the `AssertUnwindSafe` wrap to remain sound.

## [0.0.1] — Initial release

Initial public release of the clean-room Rust implementation with C ABI shim.
