#!/bin/bash
#
# wit-kv Error Conditions Test Script
#
# This script exercises all error conditions in wit-kv CLI commands
# to verify error messages are user-friendly and informative.
#
# Usage:
#   ./scripts/test-errors.sh          # Run with cargo
#   ./scripts/test-errors.sh release  # Run with release binary
#
# Exit on any error (we'll handle expected errors ourselves)
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# Get the directory where the script is located and find project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Determine the wit-kv command
if [ "$1" = "release" ]; then
    WIT_KV="$PROJECT_ROOT/target/release/wit-kv"
    echo -e "${BLUE}Using release binary: $WIT_KV${NC}"
else
    WIT_KV="cargo run --quiet --manifest-path $PROJECT_ROOT/Cargo.toml --"
    echo -e "${BLUE}Using cargo run${NC}"
fi

# Create a temporary directory for test files
TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

echo -e "${BLUE}Working in: $TMPDIR${NC}"
cd "$TMPDIR"

# Track test results
TESTS_PASSED=0
TESTS_FAILED=0

# Helper function to check that a command fails with expected error pattern
expect_error() {
    local name="$1"
    local expected_pattern="$2"
    shift 2
    local cmd="$*"

    echo -e "\n${BLUE}=== $name ===${NC}"
    echo -e "$ $cmd"

    local output
    local exit_code
    set +e
    output=$(eval "$cmd" 2>&1)
    exit_code=$?
    set -e

    echo "$output"

    if [ $exit_code -eq 0 ]; then
        echo -e "${RED}FAILED (expected error, but command succeeded)${NC}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi

    if echo "$output" | grep -qiE "$expected_pattern"; then
        echo -e "${GREEN}PASSED${NC} (error matched: $expected_pattern)"
        TESTS_PASSED=$((TESTS_PASSED + 1))
        return 0
    else
        echo -e "${RED}FAILED${NC} (error did not match pattern: $expected_pattern)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi
}

# Create a valid WIT file for tests that need it
create_test_wit() {
    cat > types.wit << 'EOF'
package test:types;
interface types {
    record point {
        x: u32,
        y: u32,
    }

    record message {
        text: string,
        count: u32,
    }

    enum color {
        red,
        green,
        blue,
    }

    variant shape {
        circle(u32),
        rectangle(point),
        none,
    }
}
EOF
}

echo ""
echo "=============================================="
echo "  CATEGORY 1: WIT Syntax Errors"
echo "=============================================="

# 1.1 Invalid WIT syntax - missing closing brace
cat > broken.wit << 'EOF'
package test:broken;
interface types {
    record point {
        x: u32,
        y: u32,
EOF
expect_error "Invalid WIT: missing closing brace" \
    "expected|parse|syntax|error" \
    "$WIT_KV lower --wit broken.wit --type-name point --value '{x: 1, y: 2}' --output out.bin"

# 1.2 Invalid WIT syntax - unknown type reference
cat > broken2.wit << 'EOF'
package test:broken;
interface types {
    record point {
        x: nonexistent-type,
        y: u32,
    }
}
EOF
expect_error "Invalid WIT: unknown type reference" \
    "nonexistent|unknown|not found|error" \
    "$WIT_KV lower --wit broken2.wit --type-name point --value '{x: 1, y: 2}' --output out.bin"

# 1.3 Empty WIT file
echo "" > empty.wit
expect_error "Empty WIT file" \
    "no types|empty|error" \
    "$WIT_KV lower --wit empty.wit --value '{x: 1}' --output out.bin"

echo ""
echo "=============================================="
echo "  CATEGORY 2: WAVE Value Syntax Errors"
echo "=============================================="

create_test_wit

# 2.1 Unclosed brace in WAVE value
expect_error "WAVE: unclosed brace" \
    "parse|syntax|expected|error" \
    "$WIT_KV lower --wit types.wit --type-name point --value '{x: 42' --output out.bin"

# 2.2 Wrong field name
expect_error "WAVE: wrong field name" \
    "wrong|unknown|field|mismatch|error" \
    "$WIT_KV lower --wit types.wit --type-name point --value '{wrong: 42, y: 1}' --output out.bin"

# 2.3 Wrong value type (string where number expected)
expect_error "WAVE: wrong value type" \
    "type|mismatch|expected|error" \
    "$WIT_KV lower --wit types.wit --type-name point --value '{x: \"hello\", y: 1}' --output out.bin"

# 2.4 Invalid enum value
expect_error "WAVE: invalid enum value" \
    "purple|unknown|variant|case|error" \
    "$WIT_KV lower --wit types.wit --type-name color --value 'purple' --output out.bin"

# 2.5 Invalid variant case
expect_error "WAVE: invalid variant case" \
    "triangle|unknown|variant|case|error" \
    "$WIT_KV lower --wit types.wit --type-name shape --value 'triangle(5)' --output out.bin"

echo ""
echo "=============================================="
echo "  CATEGORY 3: Corrupted Binary Data"
echo "=============================================="

# 3.1 Truncated binary file
echo -n "short" > truncated.bin
expect_error "LIFT: truncated binary file" \
    "buffer|small|truncated|invalid|error" \
    "$WIT_KV lift --wit types.wit --type-name point --input truncated.bin"

# 3.2 Random garbage data
head -c 100 /dev/urandom > random.bin
expect_error "LIFT: random garbage data" \
    "invalid|error|format|decode" \
    "$WIT_KV lift --wit types.wit --type-name message --input random.bin"

# 3.3 Empty binary file
touch empty.bin
expect_error "LIFT: empty binary file" \
    "buffer|empty|small|invalid|error" \
    "$WIT_KV lift --wit types.wit --type-name point --input empty.bin"

echo ""
echo "=============================================="
echo "  CATEGORY 4: Type Mismatches"
echo "=============================================="

# 4.1 Type not found in WIT file
expect_error "Type not found in WIT" \
    "type.*not found|not found.*type|nonexistent" \
    "$WIT_KV lower --wit types.wit --type-name nonexistent --value '{x: 1}' --output out.bin"

# 4.2 Lower as point, lift as message (type mismatch)
$WIT_KV lower --wit types.wit --type-name point --value '{x: 100, y: 200}' --output point.bin 2>/dev/null || true
# This may or may not error depending on implementation - it could produce garbage
# We'll just verify the command works with correct types first

echo ""
echo "=============================================="
echo "  CATEGORY 5: Missing Resources"
echo "=============================================="

# 5.1 Store not initialized
expect_error "Store not initialized" \
    "not initialized|no such|does not exist|error" \
    "$WIT_KV get tasks task-1 --path ./nonexistent-store"

# 5.2 Keyspace not found
$WIT_KV init --path ./test-store 2>/dev/null || true
expect_error "Keyspace not found" \
    "keyspace.*not found|not found.*keyspace|error" \
    "$WIT_KV get nonexistent-keyspace some-key --path ./test-store"

# 5.3 Key not found (need to set up keyspace first)
$WIT_KV set-type tasks --wit types.wit --type-name point --path ./test-store 2>/dev/null || true
expect_error "Key not found" \
    "key.*not found|not found.*key|error" \
    "$WIT_KV get tasks nonexistent-key --path ./test-store"

# 5.4 WIT file not found
expect_error "WIT file not found" \
    "no such file|not found|error|does not exist" \
    "$WIT_KV lower --wit ./nonexistent.wit --value '{x: 1}' --output out.bin"

# 5.5 Input binary file not found
expect_error "Input binary file not found" \
    "no such file|not found|error|does not exist" \
    "$WIT_KV lift --wit types.wit --type-name point --input ./nonexistent.bin"

echo ""
echo "=============================================="
echo "  CATEGORY 6: WASM Module Errors"
echo "=============================================="

# 6.1 WASM module file not found
expect_error "WASM module not found" \
    "no such file|not found|error|load" \
    "$WIT_KV map tasks --module ./nonexistent.wasm --module-wit types.wit --input-type point --path ./test-store"

# 6.2 Invalid WASM module (not a valid wasm file)
echo "not a wasm module" > fake.wasm
expect_error "Invalid WASM module" \
    "invalid|error|component|wasm|magic" \
    "$WIT_KV map tasks --module fake.wasm --module-wit types.wit --input-type point --path ./test-store"

# 6.3 Wrong input type name for module
# First create a simple valid store setup
expect_error "Wrong input type in module WIT" \
    "type.*not found|not found|nonexistent|error" \
    "$WIT_KV map tasks --module fake.wasm --module-wit types.wit --input-type nonexistent --path ./test-store"

echo ""
echo "=============================================="
echo "  CATEGORY 7: Store Constraint Violations"
echo "=============================================="

# 7.1 Keyspace already exists (without --force)
# Reset the store
rm -rf ./test-store
$WIT_KV init --path ./test-store 2>/dev/null || true
$WIT_KV set-type tasks --wit types.wit --type-name point --path ./test-store 2>/dev/null || true
expect_error "Keyspace already exists" \
    "already exists|exists|error" \
    "$WIT_KV set-type tasks --wit types.wit --type-name point --path ./test-store"

# 7.2 Set value without keyspace type registered
expect_error "Set without keyspace type" \
    "keyspace.*not found|not found|not registered|error" \
    "$WIT_KV set unregistered-keyspace some-key --value '{x: 1, y: 2}' --path ./test-store"

echo ""
echo "=============================================="
echo "  CATEGORY 8: Invalid Arguments"
echo "=============================================="

# 8.1 Neither --value nor --file specified
# Note: This may be caught by clap if configured, or by our code
expect_error "Missing --value or --file" \
    "value|file|required|must|either|error" \
    "$WIT_KV set tasks some-key --path ./test-store"

# 8.2 --file points to nonexistent file
expect_error "--file points to nonexistent file" \
    "no such file|not found|error|does not exist" \
    "$WIT_KV set tasks some-key --file ./nonexistent.wave --path ./test-store"

echo ""
echo "=============================================="
echo "  SUMMARY"
echo "=============================================="
echo ""
echo -e "Tests passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Tests failed: ${RED}$TESTS_FAILED${NC}"
echo ""

if [ $TESTS_FAILED -gt 0 ]; then
    echo -e "${YELLOW}Note: Some tests may fail if error messages don't match expected patterns.${NC}"
    echo -e "${YELLOW}Review failed tests to improve error messages.${NC}"
    exit 1
else
    echo -e "${GREEN}All error tests passed!${NC}"
    exit 0
fi
