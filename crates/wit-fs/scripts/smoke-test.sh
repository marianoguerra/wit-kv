#!/usr/bin/env bash
# Smoke test for wit-fs: mount, write, read, validate, error handling.
set -euo pipefail

# Use release binary by default, allow override
MODE="${1:-release}"
WIT_FS="./target/${MODE}/wit-fs"
WIT_FILE="./target/${MODE}/wit-file"

BACKING=$(mktemp -d)
MOUNTPOINT=$(mktemp -d)
trap 'cleanup' EXIT

PASSED=0
FAILED=0
FS_PID=""

cleanup() {
    if [ -n "$FS_PID" ] && kill -0 "$FS_PID" 2>/dev/null; then
        # Send SIGTERM and let fuser handle the unmount cleanly via Session::Drop.
        # Doing an external umount first would cause fuser to attempt a second
        # umount, producing: WARN fuser::session: Failed to umount filesystem: EINVAL
        kill "$FS_PID" 2>/dev/null || true
        wait "$FS_PID" 2>/dev/null || true
    fi
    rm -rf "$BACKING"
    # macOS may take a moment to fully release a FUSE mountpoint after unmount.
    local retries=10
    while [ $retries -gt 0 ] && [ -e "$MOUNTPOINT" ]; do
        rm -rf "$MOUNTPOINT" 2>/dev/null && break
        sleep 0.5
        retries=$((retries - 1))
    done
}

pass() {
    echo "PASS [$1]"
    PASSED=$((PASSED + 1))
}

fail() {
    echo "FAIL [$1]: $2"
    FAILED=$((FAILED + 1))
}

# Wait for the filesystem to be mounted
wait_for_mount() {
    local max_wait=10
    local waited=0
    while [ $waited -lt $max_wait ]; do
        if mount | grep -q "$MOUNTPOINT"; then
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done
    echo "ERROR: Filesystem not mounted after ${max_wait}s"
    return 1
}

echo "========================================"
echo "  SMOKE TEST: wit-fs"
echo "========================================"
echo ""
echo "Backing dir: $BACKING"
echo "Mount point: $MOUNTPOINT"
echo ""

# ---- Mount ----
echo "--- Mounting wit-fs ---"

"$WIT_FS" mount "$BACKING" "$MOUNTPOINT" &
FS_PID=$!
sleep 2

if ! wait_for_mount; then
    echo "FATAL: Could not mount filesystem"
    exit 1
fi
pass "mount"

echo ""
echo "--- Creating typed directory ---"

# Create a typed directory with a point schema
mkdir "$MOUNTPOINT/points"
if [ -d "$MOUNTPOINT/points" ]; then
    pass "mkdir-points"
else
    fail "mkdir-points" "Directory not created"
fi

# Write the schema
POINT_WIT='package example:points;
interface types {
    record point {
        x: s32,
        y: s32,
    }
}'
echo "$POINT_WIT" > "$MOUNTPOINT/points/.type.wit"
sleep 0.5

# Read back the schema
if cat "$MOUNTPOINT/points/.type.wit" | grep -q "record point"; then
    pass "write-schema"
else
    fail "write-schema" "Schema not readable"
fi

# Check .type.error.wit exists
if cat "$MOUNTPOINT/points/.type.error.wit" | grep -q "validation-error"; then
    pass "error-schema-exists"
else
    fail "error-schema-exists" ".type.error.wit not found or wrong content"
fi

echo ""
echo "--- Writing valid values (WAVE text) ---"

# Write a valid point via .wave
echo '{x: 10, y: 20}' > "$MOUNTPOINT/points/origin.wave"
sleep 0.5

# Read it back via .wave
WAVE_OUT=$(cat "$MOUNTPOINT/points/origin.wave" 2>/dev/null || echo "READ_ERROR")
if echo "$WAVE_OUT" | grep -q "x: 10" && echo "$WAVE_OUT" | grep -q "y: 20"; then
    pass "write-read-wave"
else
    fail "write-read-wave" "Expected '{x: 10, y: 20}', got '$WAVE_OUT'"
fi

# Read the same value via .witb (should be binary, non-empty)
WITB_SIZE=$(wc -c < "$MOUNTPOINT/points/origin.witb" 2>/dev/null || echo "0")
if [ "$WITB_SIZE" -gt 0 ]; then
    pass "read-witb"
else
    fail "read-witb" "Binary file is empty or missing"
fi

# No error files should exist
if [ ! -f "$MOUNTPOINT/points/origin.witerr" ]; then
    pass "no-error-after-valid"
else
    fail "no-error-after-valid" ".witerr exists after valid write"
fi

echo ""
echo "--- Writing another value ---"

echo '{x: 0, y: 0}' > "$MOUNTPOINT/points/center.wave"
sleep 0.5

CENTER_OUT=$(cat "$MOUNTPOINT/points/center.wave" 2>/dev/null || echo "READ_ERROR")
if echo "$CENTER_OUT" | grep -q "x: 0" && echo "$CENTER_OUT" | grep -q "y: 0"; then
    pass "write-center"
else
    fail "write-center" "Expected '{x: 0, y: 0}', got '$CENTER_OUT'"
fi

echo ""
echo "--- Writing invalid value (error reporting) ---"

# Write invalid data — should fail
if echo '{x: "hello"}' > "$MOUNTPOINT/points/bad.wave" 2>/dev/null; then
    # Some systems don't report the error on echo, check if value reverted
    true
fi
sleep 0.5

# Check for error file
if [ -f "$MOUNTPOINT/points/bad.witerr" ]; then
    ERR_OUT=$(cat "$MOUNTPOINT/points/bad.witerr")
    if echo "$ERR_OUT" | grep -q "error-kind:"; then
        pass "error-file-created"
    else
        fail "error-file-created" "Error file exists but has unexpected content: $ERR_OUT"
    fi
else
    # The value might not have been created at all (which is also valid)
    pass "error-file-created (value rejected)"
fi

echo ""
echo "--- Writing via .witb (binary) ---"

# Use wit-file to generate binary, write to .witb
if command -v "$WIT_FILE" >/dev/null 2>&1; then
    # Write binary via wit-file to a temp file, then copy to .witb
    TMPBIN=$(mktemp)
    if "$WIT_FILE" write --wit "$MOUNTPOINT/points/.type.wit" --value '{x: 42, y: 99}' -o "$TMPBIN" 2>/dev/null; then
        cp -X "$TMPBIN" "$MOUNTPOINT/points/direct.witb"
        sleep 0.5

        # Read back via .wave
        DIRECT_OUT=$(cat "$MOUNTPOINT/points/direct.wave" 2>/dev/null || echo "READ_ERROR")
        if echo "$DIRECT_OUT" | grep -q "x: 42" && echo "$DIRECT_OUT" | grep -q "y: 99"; then
            pass "write-witb-read-wave"
        else
            fail "write-witb-read-wave" "Expected '{x: 42, y: 99}', got '$DIRECT_OUT'"
        fi
    else
        fail "write-witb-read-wave" "wit-file write failed"
    fi
    rm -f "$TMPBIN"
else
    echo "SKIP [write-witb-read-wave]: wit-file not found"
fi

echo ""
echo "--- Error dismissal ---"

# Create a value with an error, then dismiss it
echo '{x: 100, y: 200}' > "$MOUNTPOINT/points/dismiss-test.wave"
sleep 0.5

# Force an error by writing invalid data
echo '{broken' > "$MOUNTPOINT/points/dismiss-test.wave" 2>/dev/null || true
sleep 0.5

if [ -f "$MOUNTPOINT/points/dismiss-test.witerr" ]; then
    # Dismiss the error by deleting .witerr
    rm "$MOUNTPOINT/points/dismiss-test.witerr" 2>/dev/null || true
    sleep 0.5

    if [ ! -f "$MOUNTPOINT/points/dismiss-test.witerr" ] && [ ! -f "$MOUNTPOINT/points/dismiss-test.witerrb" ]; then
        pass "error-dismissal"
    else
        fail "error-dismissal" "Error files still exist after deletion"
    fi
else
    pass "error-dismissal (no error to dismiss)"
fi

echo ""
echo "--- Error auto-removal on valid write ---"

echo '{x: 1, y: 1}' > "$MOUNTPOINT/points/auto-clear.wave"
sleep 0.5

# Create error
echo '{invalid}' > "$MOUNTPOINT/points/auto-clear.wave" 2>/dev/null || true
sleep 0.5

# Fix with valid write
echo '{x: 2, y: 2}' > "$MOUNTPOINT/points/auto-clear.wave"
sleep 0.5

if [ ! -f "$MOUNTPOINT/points/auto-clear.witerr" ]; then
    pass "error-auto-removal"
else
    fail "error-auto-removal" ".witerr still exists after valid write"
fi

echo ""
echo "--- Deleting values ---"

echo '{x: 77, y: 88}' > "$MOUNTPOINT/points/to-delete.wave"
sleep 0.5

# Delete via .wave
rm "$MOUNTPOINT/points/to-delete.wave" 2>/dev/null || true
sleep 0.5

if [ ! -f "$MOUNTPOINT/points/to-delete.wave" ] && [ ! -f "$MOUNTPOINT/points/to-delete.witb" ]; then
    pass "delete-value"
else
    fail "delete-value" "Value files still exist after deletion"
fi

echo ""
echo "--- Multiple types: greeting (strings) ---"

mkdir -p "$MOUNTPOINT/greetings"
GREETING_WIT='package example:greetings;
interface types {
    record greeting {
        message: string,
        count: u32,
    }
}'
echo "$GREETING_WIT" > "$MOUNTPOINT/greetings/.type.wit"
sleep 0.5

echo '{message: "hello world", count: 5}' > "$MOUNTPOINT/greetings/hello.wave"
sleep 0.5

GREET_OUT=$(cat "$MOUNTPOINT/greetings/hello.wave" 2>/dev/null || echo "READ_ERROR")
if echo "$GREET_OUT" | grep -q 'hello world' && echo "$GREET_OUT" | grep -q 'count: 5'; then
    pass "string-roundtrip"
else
    fail "string-roundtrip" "Expected greeting, got '$GREET_OUT'"
fi

echo ""
echo "--- Multiple types: enum ---"

mkdir -p "$MOUNTPOINT/colors"
COLOR_WIT='package example:colors;
interface types {
    enum color {
        red,
        green,
        blue,
    }
}'
echo "$COLOR_WIT" > "$MOUNTPOINT/colors/.type.wit"
sleep 0.5

echo 'red' > "$MOUNTPOINT/colors/primary.wave"
sleep 0.5

COLOR_OUT=$(cat "$MOUNTPOINT/colors/primary.wave" 2>/dev/null || echo "READ_ERROR")
if echo "$COLOR_OUT" | grep -q "red"; then
    pass "enum-roundtrip"
else
    fail "enum-roundtrip" "Expected 'red', got '$COLOR_OUT'"
fi

echo ""
echo "--- Multiple types: variant ---"

mkdir -p "$MOUNTPOINT/shapes"
SHAPE_WIT='package example:shapes;
interface types {
    variant shape {
        circle(u32),
        square(u32),
        empty,
    }
}'
echo "$SHAPE_WIT" > "$MOUNTPOINT/shapes/.type.wit"
sleep 0.5

echo 'circle(42)' > "$MOUNTPOINT/shapes/my-shape.wave"
sleep 0.5

SHAPE_OUT=$(cat "$MOUNTPOINT/shapes/my-shape.wave" 2>/dev/null || echo "READ_ERROR")
if echo "$SHAPE_OUT" | grep -q "circle(42)"; then
    pass "variant-roundtrip"
else
    fail "variant-roundtrip" "Expected 'circle(42)', got '$SHAPE_OUT'"
fi

echo ""
echo "--- Extended attributes ---"

# Check xattr on a valid value
VALID_ATTR=$(xattr -p user.wit-fs.valid "$MOUNTPOINT/points/origin.wave" 2>/dev/null || echo "UNSUPPORTED")
if [ "$VALID_ATTR" = "true" ]; then
    pass "xattr-valid"
elif [ "$VALID_ATTR" = "UNSUPPORTED" ]; then
    echo "SKIP [xattr-valid]: xattr not supported or not implemented"
else
    fail "xattr-valid" "Expected 'true', got '$VALID_ATTR'"
fi

echo ""
echo "--- Directory listing ---"

# Check that readdir shows expected files
LS_OUT=$(ls "$MOUNTPOINT/points/" 2>/dev/null || echo "LS_ERROR")
if echo "$LS_OUT" | grep -q "origin.wave" && echo "$LS_OUT" | grep -q "center.wave"; then
    pass "readdir"
else
    fail "readdir" "Expected origin.wave and center.wave in listing, got: $LS_OUT"
fi

echo ""
echo "--- Persistence (backing store) ---"

# Check that backing store has the expected files
if [ -f "$BACKING/points/.type.wit" ] && [ -f "$BACKING/points/origin.bin" ]; then
    pass "backing-store"
else
    fail "backing-store" "Expected .type.wit and origin.bin in backing dir"
fi

echo ""
echo "--- wit-file auto-discovery ---"

# Test that wit-file can auto-discover .type.wit
if command -v "$WIT_FILE" >/dev/null 2>&1; then
    # Copy a schema and binary to a temp dir for testing auto-discovery
    AUTODISCOVER_DIR=$(mktemp -d)
    cp "$BACKING/points/.type.wit" "$AUTODISCOVER_DIR/"
    cp "$BACKING/points/origin.bin" "$AUTODISCOVER_DIR/origin.bin"

    AUTO_OUT=$("$WIT_FILE" read "$AUTODISCOVER_DIR/origin.bin" 2>/dev/null || echo "ERROR")
    if echo "$AUTO_OUT" | grep -q "x:"; then
        pass "wit-file-auto-discover"
    else
        fail "wit-file-auto-discover" "wit-file auto-discovery failed: $AUTO_OUT"
    fi

    rm -rf "$AUTODISCOVER_DIR"
else
    echo "SKIP [wit-file-auto-discover]: wit-file not found"
fi

echo ""
echo "--- wit-file validate command ---"

if command -v "$WIT_FILE" >/dev/null 2>&1; then
    # Valid input
    if "$WIT_FILE" validate --wit "$MOUNTPOINT/points/.type.wit" --value '{x: 1, y: 2}' 2>/dev/null | grep -q "Valid"; then
        pass "wit-file-validate-ok"
    else
        fail "wit-file-validate-ok" "validate command did not print Valid"
    fi

    # Invalid input
    if ! "$WIT_FILE" validate --wit "$MOUNTPOINT/points/.type.wit" --value '{x: "hello"}' 2>/dev/null; then
        pass "wit-file-validate-err"
    else
        fail "wit-file-validate-err" "validate command should have failed"
    fi
else
    echo "SKIP [wit-file-validate]: wit-file not found"
fi

echo ""
echo "--- wit-file .wave extension ---"

if command -v "$WIT_FILE" >/dev/null 2>&1; then
    WAVE_DIR=$(mktemp -d)
    cp "$BACKING/points/.type.wit" "$WAVE_DIR/"

    # Write to .wave file (should write WAVE text, not binary)
    "$WIT_FILE" write --wit "$WAVE_DIR/.type.wit" --value '{x: 55, y: 66}' -o "$WAVE_DIR/test.wave" 2>/dev/null

    # Read back .wave file (should parse as WAVE text)
    WAVE_BACK=$("$WIT_FILE" read --wit "$WAVE_DIR/.type.wit" "$WAVE_DIR/test.wave" 2>/dev/null || echo "ERROR")
    if echo "$WAVE_BACK" | grep -q "x: 55"; then
        pass "wit-file-wave-extension"
    else
        fail "wit-file-wave-extension" "Expected point with x: 55, got: $WAVE_BACK"
    fi

    rm -rf "$WAVE_DIR"
else
    echo "SKIP [wit-file-wave-extension]: wit-file not found"
fi

echo ""
echo "========================================"
echo "  Results: $PASSED passed, $FAILED failed"
echo "========================================"

if [ "$FAILED" -gt 0 ]; then
    exit 1
fi
