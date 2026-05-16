#!/usr/bin/env bash
#
# Build cjson-rs-ffi as a cdylib, then compile + run the C smoke test
# against it. Exits non-zero on failure.

set -euo pipefail

cd "$(dirname "$0")/../.."

# Build libcjson.{dylib,so}
cargo build --release --workspace

# Pick the right shared-library extension.
case "$(uname -s)" in
    Darwin)  LIB="libcjson.dylib"; LDFLAGS="" ;;
    Linux)   LIB="libcjson.so";    LDFLAGS="-Wl,-rpath,\$ORIGIN" ;;
    *)       echo "unsupported OS: $(uname -s)" >&2; exit 1 ;;
esac

REL="../target/release"
[ -f "$REL/$LIB" ] || { echo "missing $REL/$LIB" >&2; exit 1; }

# Compile the C smoke test.
cc -Wall -Wextra -Werror \
   -Itests/c \
   tests/c/smoke.c \
   -L"$REL" -lcjson \
   -o tests/c/smoke

# Run it with the lib path resolvable.
case "$(uname -s)" in
    Darwin) DYLD_LIBRARY_PATH="$REL" tests/c/smoke ;;
    Linux)  LD_LIBRARY_PATH="$REL" tests/c/smoke ;;
esac
