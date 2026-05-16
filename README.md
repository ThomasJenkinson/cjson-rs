# cjson-rs

A memory-safe Rust JSON parser with a drop-in C ABI for [cJSON](https://github.com/DaveGamble/cJSON).

> **Status:** functional. 98% pass rate on the public-API subset of
> upstream cJSON's test suite. 77/78 cJSON.h functions implemented.
> Miri-clean on the unsafe FFI shim. Fuzzed (libfuzzer + AddressSanitizer)
> at ~4.5M iterations across two targets with no crashes.

## Quick example

### From C (drop-in libcjson replacement)

Existing code that links against `libcjson.so` keeps working — just link
against the `libcjson.dylib` / `libcjson.so` produced by this project
instead. No source changes:

```c
#include "cjson.h"
#include <stdio.h>

int main(void) {
    cJSON *root = cJSON_Parse("{\"name\":\"alice\",\"age\":30,\"tags\":[\"admin\"]}");
    if (root == NULL) {
        fprintf(stderr, "parse error near: %s\n", cJSON_GetErrorPtr());
        return 1;
    }

    cJSON *name = cJSON_GetObjectItemCaseSensitive(root, "name");
    if (cJSON_IsString(name)) {
        printf("name = %s\n", cJSON_GetStringValue(name));
    }

    char *out = cJSON_PrintUnformatted(root);
    printf("re-serialised: %s\n", out);
    cJSON_free(out);

    cJSON_Delete(root);
    return 0;
}
```

```sh
cc smoke.c -L /path/to/cjson-rs/target/release -lcjson -o smoke
```

### From Rust (safe API)

```rust
use cjson_rs::{parse, serialise, Value};

let v: Value = parse(br#"{"name":"alice","age":30}"#)?;

if let Some(members) = v.as_object() {
    for (key, value) in members {
        println!("{key} = {value:?}");
    }
}

let round_tripped = serialise(&v);
assert_eq!(round_tripped, r#"{"name":"alice","age":30}"#);
```

## What this is

A clean-room Rust implementation of a JSON parser, designed to:

1. **Pass the upstream cJSON test suite** when linked as `libcjson.{so,dylib}`
   (binary-compatible drop-in replacement).
2. **Eliminate the C memory-safety bug class.** Recent cJSON CVEs
   (CVE-2024-31755, CVE-2023-50471, CVE-2023-50472) are null pointer
   dereferences and missing type checks — gone by construction in Rust.
3. **Conform to RFC 8259** (the JSON specification), verified against an
   RFC-driven Rust test suite independent of the upstream tests.

## Who should use this

| You are… | Recommendation |
|---|---|
| Maintaining a C/C++ codebase that links against libcjson and you want to eliminate the memory-safety bug class without touching consumer code | **Use cjson-rs.** Drop in `libcjson.{so,dylib}` from `target/release` and your existing binaries get Rust's safety guarantees. |
| Writing a new Rust application and need a JSON parser | Use [`serde_json`](https://crates.io/crates/serde_json). It has a richer Rust-native API, is faster, and is the de-facto standard. cjson-rs is for *C/C++* consumers. |
| Building a polyglot system (Rust + C) and want a single JSON parser usable from both | **Use cjson-rs.** Safe core for Rust callers, libcjson C ABI for C callers, same parser logic underneath. |
| Embedded or `no_std` constrained | Not yet — both crates depend on `std` (`String`, `Vec`, `libc::malloc`). `no_std` support is under consideration. |
| Looking for the highest possible JSON throughput | Use [`simdjson`](https://github.com/simdjson/simdjson) or [`sonic-rs`](https://github.com/cloudwego/sonic-rs). cjson-rs prioritises correctness, safety, and drop-in compatibility over raw speed. |

## Comparison

| Project | Language | Safe? | Drop-in libcjson ABI? | Use case |
|---|---|---|---|---|
| [cJSON](https://github.com/DaveGamble/cJSON) (upstream) | C | No | (is the original) | Existing C consumers; ~1-2 CVEs/year |
| **cjson-rs** (this project) | Rust | Yes (`#![forbid(unsafe_code)]` core; miri-clean shim) | **Yes** | Same C consumers, no source changes, memory-safe |
| [`serde_json`](https://crates.io/crates/serde_json) | Rust | Yes | No (Rust-native API) | New Rust projects |
| [`simdjson`](https://github.com/simdjson/simdjson) | C++ | Mostly | No | Maximum throughput |
| [`tinyjson`](https://github.com/rhysd/tinyjson) / [`pico-args`](https://crates.io/crates/pico-args) | Rust | Yes | No | Minimal Rust deps |

## Architecture

Two crates with a hard safety boundary:

- **`cjson-rs`** — safe Rust core. Marked `#![forbid(unsafe_code)]`.
  Implements the parser, value model, and serialiser. Exposes a Rust API.
- **`cjson-rs-ffi`** — C ABI shim. The *only* crate containing `unsafe`
  code. Mirrors `cJSON.h` byte-for-byte. Validated under cargo-miri.

The cdylib output is `libcjson.{so,dylib}` — drop-in named.

## What this is not

- **Not a translation of cJSON.** See `METHODOLOGY.md`. The Rust parser
  is built from RFC 8259, not from `cJSON.c`.
- **Not a replacement for `serde_json`.** Rust-native applications should
  use serde_json. This project exists to give existing C consumers of
  cJSON a memory-safe binary they can link instead.

## Status

| Test suite | Tests | Status |
|---|---|---|
| Tokeniser (RFC 8259, internal) | 34 | green |
| Parser (RFC 8259, internal) | 41 | green |
| Serialiser (RFC 8259, internal) | 37 | green |
| FFI smoke (`unsafe` Rust → FFI) | 34 | green |
| C smoke (real `cc -lcjson`) | 11 | green |
| **Upstream cJSON, public-API subset** | **64 / 65** | **98%** |
| Miri (UB detection on FFI) | 34 | clean |
| Fuzz (libfuzzer + ASan, 2 targets, ~4.5M runs) | — | no crashes |

### Known limitations

| Item | Why | Fix |
|---|---|---|
| `cJSON_ParseWithOpts` cannot report parse_end for valid partial input followed by trailing garbage | Our parser eagerly tokenises the whole input. Reporting the parse-end byte position requires lazy tokenisation. | Refactor parser to lazy mode |
| Upstream tests that `#include "../cJSON.c"` directly (using internal `global_hooks` symbol) are not in the runner | These are white-box tests of cJSON's internals, not real drop-in compat tests. | Out of scope for drop-in validation |

## Build & test

```sh
# Builds both crates and produces target/release/libcjson.{so,dylib,a}
cargo build --release --workspace

# Run all Rust tests
cargo test --workspace

# Run the C smoke test (compiles smoke.c with cc, links against libcjson.dylib)
bash cjson-rs-ffi/tests/c/run-c-smoke.sh

# Run the public-API subset of upstream cJSON tests
bash conformance/run-upstream-tests.sh

# Validate the FFI shim under miri (catches aliasing / UB)
cargo +nightly miri test --workspace --test ffi_smoke

# Fuzz (libfuzzer + AddressSanitizer)
cd cjson-rs-ffi
cargo +nightly fuzz run fuzz_parse -- -max_total_time=60
cargo +nightly fuzz run fuzz_ffi_roundtrip -- -max_total_time=60
```

## Layout

```
.
├── cjson-rs/                   # safe Rust core — #![forbid(unsafe_code)]
│   ├── src/
│   │   ├── lib.rs              # public API
│   │   ├── value.rs            # Value enum (cJSON-compatible semantics)
│   │   ├── error.rs            # Error + Position
│   │   ├── token.rs            # Token enum (RFC 8259 productions)
│   │   ├── tokeniser.rs        # tokeniser
│   │   ├── parser.rs           # recursive-descent parser
│   │   └── serialiser.rs       # compact + pretty serialisers
│   └── tests/                  # RFC-driven integration tests
├── cjson-rs-ffi/               # C ABI shim — all unsafe lives here
│   ├── src/
│   │   ├── lib.rs              # public extern "C" functions
│   │   ├── types.rs            # cJSON struct + type constants
│   │   ├── convert.rs          # Value ↔ cJSON tree, alloc helpers
│   │   ├── panic_guard.rs      # catch_unwind wrapper
│   │   └── alloc_hooks.rs      # malloc/free indirection for InitHooks
│   ├── tests/
│   │   ├── ffi_smoke.rs        # Rust-side FFI tests
│   │   └── c/                  # C-side smoke test
│   └── fuzz/                   # cargo-fuzz targets
├── conformance/
│   ├── upstream-tests/         # git submodule → DaveGamble/cJSON
│   ├── shim/common.h           # replaces upstream common.h (public API only)
│   └── run-upstream-tests.sh   # runner — links upstream tests against libcjson.dylib
└── METHODOLOGY.md              # clean-room declaration
```

## License

MIT — same as cJSON, so contributions can flow either way if a consumer
needs a feature in both.
