# Conformance

Two independent test suites are run against `cjson-rs`:

1. **RFC 8259 production tests** — Rust tests under `cjson-rs/tests/`, written from the JSON specification. These verify the safe Rust core in isolation.
2. **Upstream cJSON test suite** — the `tests/*.c` files from [DaveGamble/cJSON](https://github.com/DaveGamble/cJSON), linked against `libcjson.so` produced by `cjson-rs-ffi`. These verify binary compatibility.

## Adding the upstream test suite (Week 2)

The upstream tests are not yet checked in. When ready:

```sh
cd conformance
git submodule add https://github.com/DaveGamble/cJSON.git upstream-tests
git submodule update --init --recursive
```

Then build with `cjson-rs-ffi`'s `libcjson.so` on the link path and run the upstream `tests/` binaries against it.
