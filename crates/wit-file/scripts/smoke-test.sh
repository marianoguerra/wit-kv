#!/usr/bin/env bash
# Smoke test for wit-file: roundtrip encode/decode of all supported WIT types.
set -euo pipefail

# Use release binary by default, allow override
MODE="${1:-release}"
WIT_FILE="./target/${MODE}/wit-file"
WIT="./crates/wit-file/tests/types.wit"
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

PASSED=0
FAILED=0

# Test a single roundtrip: write WAVE text, read it back, compare.
# Usage: test_roundtrip <test_name> <type_name> <wave_value>
test_roundtrip() {
    local name="$1"
    local type_name="$2"
    local wave="$3"
    local binfile="$TMPDIR/${name}.bin"

    # Write
    if ! "$WIT_FILE" write --wit "$WIT" -t "$type_name" -o "$binfile" --value "$wave" 2>"$TMPDIR/err.txt"; then
        echo "FAIL [$name]: write failed"
        cat "$TMPDIR/err.txt" >&2
        FAILED=$((FAILED + 1))
        return
    fi

    # Read
    local output
    if ! output=$("$WIT_FILE" read --wit "$WIT" -t "$type_name" "$binfile" 2>"$TMPDIR/err.txt"); then
        echo "FAIL [$name]: read failed"
        cat "$TMPDIR/err.txt" >&2
        FAILED=$((FAILED + 1))
        return
    fi

    # Compare (normalize whitespace)
    local expected
    expected=$(echo "$wave" | tr -s ' ')
    output=$(echo "$output" | tr -s ' ')

    if [ "$output" = "$expected" ]; then
        echo "PASS [$name]"
        PASSED=$((PASSED + 1))
    else
        echo "FAIL [$name]: expected '$expected', got '$output'"
        FAILED=$((FAILED + 1))
    fi
}

# Test encode/decode where input and expected output may differ.
# Usage: test_encode_decode <test_name> <type_name> <input_wave> <expected_output>
test_encode_decode() {
    local name="$1"
    local type_name="$2"
    local input_wave="$3"
    local expected_wave="$4"
    local binfile="$TMPDIR/${name}.bin"

    # Write
    if ! "$WIT_FILE" write --wit "$WIT" -t "$type_name" -o "$binfile" --value "$input_wave" 2>"$TMPDIR/err.txt"; then
        echo "FAIL [$name]: write failed"
        cat "$TMPDIR/err.txt" >&2
        FAILED=$((FAILED + 1))
        return
    fi

    # Read
    local output
    if ! output=$("$WIT_FILE" read --wit "$WIT" -t "$type_name" "$binfile" 2>"$TMPDIR/err.txt"); then
        echo "FAIL [$name]: read failed"
        cat "$TMPDIR/err.txt" >&2
        FAILED=$((FAILED + 1))
        return
    fi

    local expected
    expected=$(echo "$expected_wave" | tr -s ' ')
    output=$(echo "$output" | tr -s ' ')

    if [ "$output" = "$expected" ]; then
        echo "PASS [$name]"
        PASSED=$((PASSED + 1))
    else
        echo "FAIL [$name]: expected '$expected', got '$output'"
        FAILED=$((FAILED + 1))
    fi
}

# Test writing from a file instead of --value
test_roundtrip_file() {
    local name="$1"
    local type_name="$2"
    local wave="$3"
    local wavefile="$TMPDIR/${name}.wave"
    local binfile="$TMPDIR/${name}.bin"

    echo "$wave" > "$wavefile"

    # Write from file
    if ! "$WIT_FILE" write --wit "$WIT" -t "$type_name" -o "$binfile" --file "$wavefile" 2>"$TMPDIR/err.txt"; then
        echo "FAIL [$name]: write --file failed"
        cat "$TMPDIR/err.txt" >&2
        FAILED=$((FAILED + 1))
        return
    fi

    # Read
    local output
    if ! output=$("$WIT_FILE" read --wit "$WIT" -t "$type_name" "$binfile" 2>"$TMPDIR/err.txt"); then
        echo "FAIL [$name]: read failed"
        cat "$TMPDIR/err.txt" >&2
        FAILED=$((FAILED + 1))
        return
    fi

    local expected
    expected=$(echo "$wave" | tr -s ' ')
    output=$(echo "$output" | tr -s ' ')

    if [ "$output" = "$expected" ]; then
        echo "PASS [$name]"
        PASSED=$((PASSED + 1))
    else
        echo "FAIL [$name]: expected '$expected', got '$output'"
        FAILED=$((FAILED + 1))
    fi
}

# Test writing from stdin
test_roundtrip_stdin() {
    local name="$1"
    local type_name="$2"
    local wave="$3"
    local binfile="$TMPDIR/${name}.bin"

    # Write from stdin
    if ! echo "$wave" | "$WIT_FILE" write --wit "$WIT" -t "$type_name" -o "$binfile" 2>"$TMPDIR/err.txt"; then
        echo "FAIL [$name]: write from stdin failed"
        cat "$TMPDIR/err.txt" >&2
        FAILED=$((FAILED + 1))
        return
    fi

    # Read
    local output
    if ! output=$("$WIT_FILE" read --wit "$WIT" -t "$type_name" "$binfile" 2>"$TMPDIR/err.txt"); then
        echo "FAIL [$name]: read failed"
        cat "$TMPDIR/err.txt" >&2
        FAILED=$((FAILED + 1))
        return
    fi

    local expected
    expected=$(echo "$wave" | tr -s ' ')
    output=$(echo "$output" | tr -s ' ')

    if [ "$output" = "$expected" ]; then
        echo "PASS [$name]"
        PASSED=$((PASSED + 1))
    else
        echo "FAIL [$name]: expected '$expected', got '$output'"
        FAILED=$((FAILED + 1))
    fi
}

# Test that read from --output writes to file
test_read_to_file() {
    local name="$1"
    local type_name="$2"
    local wave="$3"
    local binfile="$TMPDIR/${name}.bin"
    local outfile="$TMPDIR/${name}.out"

    "$WIT_FILE" write --wit "$WIT" -t "$type_name" -o "$binfile" --value "$wave"
    "$WIT_FILE" read --wit "$WIT" -t "$type_name" "$binfile" -o "$outfile"

    local output
    output=$(cat "$outfile" | tr -s ' ')
    local expected
    expected=$(echo "$wave" | tr -s ' ')

    if [ "$output" = "$expected" ]; then
        echo "PASS [$name]"
        PASSED=$((PASSED + 1))
    else
        echo "FAIL [$name]: expected '$expected', got '$output'"
        FAILED=$((FAILED + 1))
    fi
}

echo "========================================"
echo "  SMOKE TEST: wit-file"
echo "========================================"
echo ""

# --- Fixed-size types (no memory segment) ---
echo "--- Fixed-size types ---"

test_roundtrip "point-basic" point '{x: 10, y: 20}'
test_roundtrip "point-negative" point '{x: -100, y: -200}'
test_roundtrip "point-zero" point '{x: 0, y: 0}'
test_roundtrip "point-max" point '{x: 2147483647, y: -2147483648}'

echo ""
echo "--- All numeric primitives ---"

test_roundtrip "all-numbers" all-numbers '{a-bool: true, a-u8: 255, a-u16: 65535, a-u32: 4294967295, a-u64: 18446744073709551615, a-s8: -128, a-s16: -32768, a-s32: -2147483648, a-s64: -9223372036854775808, a-f32: 3.14, a-f64: 2.718281828459045, a-char: '\''A'\''}'
test_roundtrip "all-numbers-zero" all-numbers '{a-bool: false, a-u8: 0, a-u16: 0, a-u32: 0, a-u64: 0, a-s8: 0, a-s16: 0, a-s32: 0, a-s64: 0, a-f32: 0, a-f64: 0, a-char: '\''0'\''}'

echo ""
echo "--- Nested records (fixed-size) ---"

test_roundtrip "line-segment" line-segment '{start: {x: 1, y: 2}, end: {x: 3, y: 4}}'

echo ""
echo "--- Enum ---"

test_roundtrip "color-red" color 'red'
test_roundtrip "color-green" color 'green'
test_roundtrip "color-blue" color 'blue'

echo ""
echo "--- Flags ---"

test_roundtrip "perms-all" perm-holder '{perms: {read, write, execute}}'
test_roundtrip "perms-some" perm-holder '{perms: {read, execute}}'
test_roundtrip "perms-none" perm-holder '{perms: {}}'
test_roundtrip "perms-single" perm-holder '{perms: {write}}'

echo ""
echo "--- Variant ---"

test_roundtrip "shape-circle" shape 'circle(42)'
test_roundtrip "shape-rectangle" shape 'rectangle({x: 5, y: 10})'
test_roundtrip "shape-empty" shape 'empty'

echo ""
echo "--- Tuple ---"

test_roundtrip "pair" pair '{value: (100, -200)}'

echo ""
echo "--- Option (fixed-size payload) ---"

test_roundtrip "option-some" maybe-point '{value: some({x: 42, y: 99})}'
# WAVE omits none fields in records, so the output is {: }
test_encode_decode "option-none" maybe-point '{value: none}' '{:}'

echo ""
echo "--- Result (mixed payloads) ---"

test_roundtrip "result-ok" operation-result '{outcome: ok(42)}'
test_roundtrip "result-err" operation-result '{outcome: err("something went wrong")}'

echo ""
echo "--- Variable-length types (with memory segment) ---"

test_roundtrip "greeting" greeting '{message: "hello world", count: 5}'
test_roundtrip "greeting-empty-string" greeting '{message: "", count: 0}'
test_roundtrip "greeting-unicode" greeting '{message: "こんにちは世界", count: 1}'

echo ""
echo "--- Lists ---"

test_roundtrip "scores" scores '{values: [10, 20, 30, 40, 50], label: "test"}'
test_roundtrip "scores-empty" scores '{values: [], label: ""}'
test_roundtrip "scores-single" scores '{values: [42], label: "one"}'

echo ""
echo "--- List of strings ---"

test_roundtrip "tags" tag-set '{tags: ["alpha", "beta", "gamma"]}'
test_roundtrip "tags-empty" tag-set '{tags: []}'
test_roundtrip "tags-single" tag-set '{tags: ["solo"]}'

echo ""
echo "--- Optional string ---"

test_roundtrip "user-with-email" user '{name: "Alice", email: some("alice@example.com")}'
# WAVE omits none optional fields
test_encode_decode "user-no-email" user '{name: "Bob", email: none}' '{name: "Bob"}'

echo ""
echo "--- List of records ---"

test_roundtrip "point-list" point-list '{points: [{x: 1, y: 2}, {x: 3, y: 4}, {x: 5, y: 6}]}'
test_roundtrip "point-list-empty" point-list '{points: []}'

echo ""
echo "--- Deeply nested: list of records with strings ---"

test_encode_decode "contacts" contacts '{entries: [{name: "Alice", email: some("a@b.com")}, {name: "Bob", email: none}]}' '{entries: [{name: "Alice", email: some("a@b.com")}, {name: "Bob"}]}'
test_roundtrip "contacts-empty" contacts '{entries: []}'

echo ""
echo "--- Input modes (file, stdin) ---"

test_roundtrip_file "file-input" point '{x: 77, y: 88}'
test_roundtrip_stdin "stdin-input" point '{x: 99, y: 11}'

echo ""
echo "--- Output to file ---"

test_read_to_file "output-file" point '{x: 55, y: 66}'

echo ""
echo "========================================"
echo "  Results: $PASSED passed, $FAILED failed"
echo "========================================"

if [ "$FAILED" -gt 0 ]; then
    exit 1
fi
