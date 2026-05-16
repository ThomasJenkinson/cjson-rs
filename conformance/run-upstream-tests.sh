#!/usr/bin/env bash
#
# Run the subset of upstream cJSON tests that exercise only the public
# API surface (i.e. don't reach into upstream-internal symbols like
# global_hooks). Each test is linked against libcjson.dylib produced
# by cjson-rs-ffi rather than upstream's cJSON.c.
#
# The shim/common.h is pre-included so that the test files' own
# `#include "common.h"` is shadowed.

set -euo pipefail

cd "$(dirname "$0")/.."

# Build our libcjson.{dylib,so}.
cargo build --release --workspace >/dev/null

UPSTREAM=conformance/upstream-tests
SHIM=conformance/shim
UNITY="$UPSTREAM/tests/unity/src"
TESTSDIR="$UPSTREAM/tests"
OUT=conformance/build
mkdir -p "$OUT"

case "$(uname -s)" in
    Darwin) LIB_VAR=DYLD_LIBRARY_PATH ;;
    Linux)  LIB_VAR=LD_LIBRARY_PATH ;;
    *)      echo "unsupported OS: $(uname -s)" >&2; exit 1 ;;
esac

# Candidate tests: those that don't reference upstream-internal symbols
# (verified by grep for global_hooks / cjson_min / similar).
TESTS=(
    cjson_add
    compare_tests
    parse_examples
    parse_with_opts
    readme_examples
)

# Compile + run each one. Per-test status accumulates in pass/fail.
declare -i pass=0
declare -i fail=0
declare -i compile_fail=0
declare -a failures=()

for t in "${TESTS[@]}"; do
    src="$TESTSDIR/${t}.c"
    bin="$OUT/${t}"

    # -include forces our shim common.h to be processed before the
    # source file. The test's own `#include "common.h"` becomes a
    # header-guard no-op.
    # -I$UPSTREAM puts cJSON.h on the search path.
    # -I$UNITY puts unity.h on the search path.
    if cc \
        -Wno-implicit-function-declaration \
        -include "$SHIM/common.h" \
        -I"$UPSTREAM" \
        -I"$UNITY" \
        -I"$TESTSDIR" \
        "$src" "$TESTSDIR/unity_setup.c" "$UNITY/unity.c" \
        -L"target/release" -lcjson \
        -o "$bin" 2>"$OUT/${t}.cc.log"; then
        : # compiled OK
    else
        compile_fail+=1
        failures+=("$t (compile error — see $OUT/${t}.cc.log)")
        continue
    fi

    # Run it. Many upstream tests need to be run from the tests/ dir
    # because they read inputs/*.json by relative path.
    BIN_ABS="$PWD/$bin"
    LOG_ABS="$PWD/$OUT/${t}.run.log"
    LIB_ABS="$PWD/target/release"
    pushd "$TESTSDIR" >/dev/null
    if env "$LIB_VAR=$LIB_ABS" "$BIN_ABS" >"$LOG_ABS" 2>&1; then
        pass+=1
    else
        fail+=1
        failures+=("$t (test failure — see $OUT/${t}.run.log)")
    fi
    popd >/dev/null
done

echo "================================================================"
echo "Upstream cJSON test suite — public-API subset"
echo "================================================================"
printf "%-22s %8s %8s %8s\n" "FILE" "TOTAL" "PASS" "FAIL"
printf "%-22s %8s %8s %8s\n" "----" "-----" "----" "----"

declare -i total_indiv=0
declare -i pass_indiv=0
declare -i fail_indiv=0

for t in "${TESTS[@]}"; do
    log="$OUT/${t}.run.log"
    if [ -f "$log" ]; then
        line=$(grep -E "^[0-9]+ Tests " "$log" | tail -1 || true)
        if [ -n "$line" ]; then
            tot=$(echo "$line" | awk '{print $1}')
            fl=$(echo "$line" | awk '{print $3}')
            ps=$((tot - fl))
            total_indiv+=$tot
            pass_indiv+=$ps
            fail_indiv+=$fl
            printf "%-22s %8d %8d %8d\n" "$t" "$tot" "$ps" "$fl"
        else
            printf "%-22s %8s %8s %8s\n" "$t" "?" "?" "?"
        fi
    fi
done

printf "%-22s %8s %8s %8s\n" "----" "-----" "----" "----"
printf "%-22s %8d %8d %8d\n" "TOTAL" "$total_indiv" "$pass_indiv" "$fail_indiv"
echo ""
if [ "$total_indiv" -gt 0 ]; then
    pct=$((pass_indiv * 100 / total_indiv))
    echo "Pass rate: ${pct}% (${pass_indiv}/${total_indiv})"
fi
echo "Test files: $pass passed cleanly, $fail had failures, $compile_fail did not compile"
echo "================================================================"

# Exit non-zero if any compile errors. (Test failures aren't fatal — we
# expect some until the implementation surface is complete.)
[ "$compile_fail" -eq 0 ]
