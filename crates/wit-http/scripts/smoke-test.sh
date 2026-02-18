#!/usr/bin/env bash
# Smoke test for wit-http: end-to-end REST API testing with the user_store example.
set -euo pipefail

MODE="${1:-release}"
EXAMPLE_BIN="./target/${MODE}/examples/user_store"
PORT=3847  # Use uncommon port to avoid conflicts
BASE_URL="http://127.0.0.1:${PORT}/api/v1/users"
TMPDIR=$(mktemp -d)
SERVER_PID=""

PASSED=0
FAILED=0

cleanup() {
    if [ -n "$SERVER_PID" ]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    rm -rf "$TMPDIR"
}
trap cleanup EXIT

# Wait for the server to be ready (up to 5 seconds).
wait_for_server() {
    local retries=50
    while [ $retries -gt 0 ]; do
        if curl -s -o /dev/null -w '' "${BASE_URL}" 2>/dev/null; then
            return 0
        fi
        sleep 0.1
        retries=$((retries - 1))
    done
    echo "FAIL: server did not start within 5 seconds"
    exit 1
}

# Run a test: execute a curl command and check the HTTP status code.
# Usage: run_test <test_name> <expected_status> <curl_args...>
run_test() {
    local name="$1"
    local expected_status="$2"
    shift 2

    local status
    local body
    body=$(curl -s -o "$TMPDIR/body.txt" -w "%{http_code}" "$@" 2>"$TMPDIR/err.txt") || true
    status="$body"
    body=$(cat "$TMPDIR/body.txt")

    if [ "$status" = "$expected_status" ]; then
        echo "PASS [$name] (status $status)"
        PASSED=$((PASSED + 1))
    else
        echo "FAIL [$name]: expected status $expected_status, got $status"
        if [ -n "$body" ]; then
            echo "  body: $body"
        fi
        FAILED=$((FAILED + 1))
    fi
}

# Run a test and also check that the response body contains a string.
# Usage: check_body <test_name> <expected_status> <body_pattern> <curl_args...>
check_body() {
    local name="$1"
    local expected_status="$2"
    local pattern="$3"
    shift 3

    local status
    local body
    body=$(curl -s -o "$TMPDIR/body.txt" -w "%{http_code}" "$@" 2>"$TMPDIR/err.txt") || true
    status="$body"
    body=$(cat "$TMPDIR/body.txt")

    if [ "$status" != "$expected_status" ]; then
        echo "FAIL [$name]: expected status $expected_status, got $status"
        FAILED=$((FAILED + 1))
        return
    fi

    if echo "$body" | grep -q "$pattern"; then
        echo "PASS [$name] (status $status, body contains '$pattern')"
        PASSED=$((PASSED + 1))
    else
        echo "FAIL [$name]: body does not contain '$pattern'"
        echo "  body: $body"
        FAILED=$((FAILED + 1))
    fi
}

echo "========================================"
echo "  SMOKE TEST: wit-http"
echo "========================================"
echo ""

# --- Build if needed ---
if [ ! -f "$EXAMPLE_BIN" ]; then
    echo "Building user_store example ($MODE)..."
    if [ "$MODE" = "release" ]; then
        cargo build --example user_store -p wit-http --release
    else
        cargo build --example user_store -p wit-http
    fi
fi

if [ ! -f "$EXAMPLE_BIN" ]; then
    echo "FAIL: example binary not found at $EXAMPLE_BIN"
    echo "  Try: cargo build --example user_store -p wit-http --release"
    exit 1
fi

# --- Start the server ---
echo "Starting user_store example on port $PORT..."
PORT=$PORT "$EXAMPLE_BIN" &
SERVER_PID=$!
wait_for_server
echo ""

# --- PUT: Create users ---
echo "--- PUT (create/update) ---"

run_test "put-alice" "204" \
    -X PUT "${BASE_URL}/alice" \
    -H 'Content-Type: application/x-wasm-wave' \
    -d '{name: "Alice", email: "alice@example.com", age: 30}'

run_test "put-bob" "204" \
    -X PUT "${BASE_URL}/bob" \
    -H 'Content-Type: application/x-wasm-wave' \
    -d '{name: "Bob", email: "bob@example.com", age: 25}'

run_test "put-charlie" "204" \
    -X PUT "${BASE_URL}/charlie" \
    -H 'Content-Type: application/x-wasm-wave' \
    -d '{name: "Charlie", email: "charlie@example.com", age: 35}'

echo ""

# --- GET: Retrieve users (WAVE text) ---
echo "--- GET (wave text) ---"

check_body "get-alice-wave" "200" "Alice" \
    "${BASE_URL}/alice" \
    -H 'Accept: application/x-wasm-wave'

check_body "get-bob-wave" "200" "bob@example.com" \
    "${BASE_URL}/bob" \
    -H 'Accept: application/x-wasm-wave'

echo ""

# --- GET: Default format is WAVE ---
echo "--- GET (default format) ---"

check_body "get-default-format" "200" "Alice" \
    "${BASE_URL}/alice"

echo ""

# --- GET: Binary format ---
echo "--- GET (binary) ---"

# Fetch binary, save to file
curl -s -o "$TMPDIR/alice.bin" \
    "${BASE_URL}/alice" \
    -H 'Accept: application/octet-stream'

if [ -s "$TMPDIR/alice.bin" ]; then
    echo "PASS [get-alice-binary] (non-empty binary response)"
    PASSED=$((PASSED + 1))
else
    echo "FAIL [get-alice-binary]: empty binary response"
    FAILED=$((FAILED + 1))
fi

# PUT binary back as a new user
run_test "put-binary-roundtrip" "204" \
    -X PUT "${BASE_URL}/alice-copy" \
    -H 'Content-Type: application/octet-stream' \
    --data-binary @"$TMPDIR/alice.bin"

# Verify the copy matches
check_body "get-binary-copy" "200" "Alice" \
    "${BASE_URL}/alice-copy" \
    -H 'Accept: application/x-wasm-wave'

echo ""

# --- LIST: Collection endpoint ---
echo "--- LIST ---"

check_body "list-all" "200" "Alice" \
    "${BASE_URL}" \
    -H 'Accept: application/x-wasm-wave'

check_body "list-contains-bob" "200" "Bob" \
    "${BASE_URL}" \
    -H 'Accept: application/x-wasm-wave'

check_body "list-contains-charlie" "200" "Charlie" \
    "${BASE_URL}" \
    -H 'Accept: application/x-wasm-wave'

echo ""

# --- LIST: Pagination ---
echo "--- LIST (pagination) ---"

check_body "list-limit-1" "200" "Alice" \
    "${BASE_URL}?limit=1" \
    -H 'Accept: application/x-wasm-wave'

echo ""

# --- PUT: Update existing user ---
echo "--- PUT (update) ---"

run_test "update-alice" "204" \
    -X PUT "${BASE_URL}/alice" \
    -H 'Content-Type: application/x-wasm-wave' \
    -d '{name: "Alice Updated", email: "alice-new@example.com", age: 31}'

check_body "get-updated-alice" "200" "Alice Updated" \
    "${BASE_URL}/alice" \
    -H 'Accept: application/x-wasm-wave'

echo ""

# --- DELETE ---
echo "--- DELETE ---"

run_test "delete-bob" "204" \
    -X DELETE "${BASE_URL}/bob"

run_test "get-deleted-bob" "404" \
    "${BASE_URL}/bob"

echo ""

# --- Error cases ---
echo "--- Error cases ---"

run_test "get-nonexistent" "404" \
    "${BASE_URL}/nobody"

run_test "delete-nonexistent" "404" \
    -X DELETE "${BASE_URL}/nobody"

run_test "put-unsupported-content-type" "415" \
    -X PUT "${BASE_URL}/test" \
    -H 'Content-Type: application/json' \
    -d '{"name": "Test"}'

run_test "put-invalid-wave" "400" \
    -X PUT "${BASE_URL}/test" \
    -H 'Content-Type: application/x-wasm-wave' \
    -d 'not valid wave {{{'

echo ""
echo "========================================"
echo "  Results: $PASSED passed, $FAILED failed"
echo "========================================"

if [ "$FAILED" -gt 0 ]; then
    exit 1
fi
