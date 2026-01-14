#!/bin/bash
#
# wit-value Usage Example and Smoke Test
#
# This script demonstrates all CLI features of wit-value and can be run
# as a smoke test to verify the tool is working correctly.
#
# Usage:
#   ./scripts/usage-example.sh          # Run with cargo
#   ./scripts/usage-example.sh release  # Run with release binary
#
# Exit on any error
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Get the directory where the script is located and find project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Determine the wit-value command
if [ "$1" = "release" ]; then
    WIT_VALUE="$PROJECT_ROOT/target/release/wit-value"
    echo -e "${BLUE}Using release binary: $WIT_VALUE${NC}"
else
    WIT_VALUE="cargo run --quiet --manifest-path $PROJECT_ROOT/Cargo.toml --"
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

# Helper function to run a test
run_test() {
    local name="$1"
    local cmd="$2"
    echo -e "\n${BLUE}=== $name ===${NC}"
    echo -e "$ $cmd"
    if eval "$cmd"; then
        echo -e "${GREEN}PASSED${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}FAILED${NC}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
}

# Helper function to check output
check_output() {
    local name="$1"
    local cmd="$2"
    local expected="$3"
    echo -e "\n${BLUE}=== $name ===${NC}"
    echo -e "$ $cmd"
    local output
    output=$(eval "$cmd")
    echo "$output"
    if echo "$output" | grep -q "$expected"; then
        echo -e "${GREEN}PASSED (found: $expected)${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "${RED}FAILED (expected to find: $expected)${NC}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
}

echo ""
echo "=============================================="
echo "  PART 1: Lower and Lift Commands"
echo "=============================================="
echo ""
echo "The lower/lift commands convert between WAVE text format and"
echo "canonical ABI binary format."

# Create a simple WIT file with various types
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

    flags permissions {
        read,
        write,
        execute,
    }
}
EOF

echo -e "\n${BLUE}Created types.wit:${NC}"
cat types.wit

# Test 1: Lower a simple record
run_test "Lower a point record to binary" \
    "$WIT_VALUE lower --wit types.wit --type-name point --value '{x: 42, y: 100}' --output point.bin"

# Verify the binary file was created
run_test "Verify point.bin was created" \
    "test -f point.bin && echo 'point.bin exists (size: '$(wc -c < point.bin)' bytes)'"

# Test 2: Lift the binary back to WAVE format
check_output "Lift point.bin back to WAVE" \
    "$WIT_VALUE lift --wit types.wit --type-name point --input point.bin" \
    "x: 42"

# Test 3: Lower a type with a string (creates .memory file)
run_test "Lower a message with string (creates .memory file)" \
    "$WIT_VALUE lower --wit types.wit --type-name message --value '{text: \"hello world\", count: 5}' --output msg.bin"

# Verify both files were created
run_test "Verify msg.bin and msg.bin.memory were created" \
    "test -f msg.bin && test -f msg.bin.memory && echo 'msg.bin ('$(wc -c < msg.bin)' bytes) + msg.bin.memory ('$(wc -c < msg.bin.memory)' bytes)'"

# Test 4: Lift the message back
check_output "Lift msg.bin back to WAVE (uses .memory file automatically)" \
    "$WIT_VALUE lift --wit types.wit --type-name message --input msg.bin" \
    "hello world"

# Test 5: Lower an enum value
run_test "Lower an enum value" \
    "$WIT_VALUE lower --wit types.wit --type-name color --value 'green' --output color.bin"

check_output "Lift enum back" \
    "$WIT_VALUE lift --wit types.wit --type-name color --input color.bin" \
    "green"

# Test 6: Lower a variant
run_test "Lower a variant (circle)" \
    "$WIT_VALUE lower --wit types.wit --type-name shape --value 'circle(50)' --output shape.bin"

check_output "Lift variant back" \
    "$WIT_VALUE lift --wit types.wit --type-name shape --input shape.bin" \
    "circle(50)"

# Test 7: Lower flags
run_test "Lower flags value" \
    "$WIT_VALUE lower --wit types.wit --type-name permissions --value '{read, write}' --output perms.bin"

check_output "Lift flags back" \
    "$WIT_VALUE lift --wit types.wit --type-name permissions --input perms.bin" \
    "read"

echo ""
echo "=============================================="
echo "  PART 2: Key-Value Store Commands"
echo "=============================================="
echo ""
echo "The kv subcommand provides a typed persistent key-value store"
echo "where each keyspace is associated with a WIT type."

# Initialize a new KV store
run_test "Initialize a new KV store" \
    "$WIT_VALUE kv --path ./test-kv init"

run_test "Verify KV store was created" \
    "test -d ./test-kv && echo 'test-kv directory exists'"

# Create a WIT file for tasks
cat > task.wit << 'EOF'
package app:tasks;
interface types {
    record task {
        title: string,
        completed: bool,
        priority: u8,
    }
}
EOF

echo -e "\n${BLUE}Created task.wit:${NC}"
cat task.wit

# Register a type for a keyspace
run_test "Register 'task' type for 'tasks' keyspace" \
    "$WIT_VALUE kv --path ./test-kv set-type tasks --wit task.wit --type-name task"

# List registered types
check_output "List registered types" \
    "$WIT_VALUE kv --path ./test-kv list-types" \
    "tasks"

# Get type definition
check_output "Get type definition for 'tasks' keyspace" \
    "$WIT_VALUE kv --path ./test-kv get-type tasks" \
    "record task"

# Set some values
run_test "Set task-1" \
    "$WIT_VALUE kv --path ./test-kv set tasks task-1 --value '{title: \"Buy groceries\", completed: false, priority: 1}'"

run_test "Set task-2" \
    "$WIT_VALUE kv --path ./test-kv set tasks task-2 --value '{title: \"Walk the dog\", completed: true, priority: 2}'"

run_test "Set task-3" \
    "$WIT_VALUE kv --path ./test-kv set tasks task-3 --value '{title: \"Review PR\", completed: false, priority: 1}'"

# Get values back
check_output "Get task-1" \
    "$WIT_VALUE kv --path ./test-kv get tasks task-1" \
    "Buy groceries"

check_output "Get task-2" \
    "$WIT_VALUE kv --path ./test-kv get tasks task-2" \
    "completed: true"

# List keys
check_output "List all keys in 'tasks' keyspace" \
    "$WIT_VALUE kv --path ./test-kv list tasks" \
    "task-1"

check_output "List keys with prefix 'task-1'" \
    "$WIT_VALUE kv --path ./test-kv list tasks --prefix task-1" \
    "task-1"

check_output "List keys with limit" \
    "$WIT_VALUE kv --path ./test-kv list tasks --limit 2" \
    "task"

# Update a value
run_test "Update task-1 (mark as completed)" \
    "$WIT_VALUE kv --path ./test-kv set tasks task-1 --value '{title: \"Buy groceries\", completed: true, priority: 1}'"

check_output "Verify task-1 was updated" \
    "$WIT_VALUE kv --path ./test-kv get tasks task-1" \
    "completed: true"

# Delete a value
run_test "Delete task-3" \
    "$WIT_VALUE kv --path ./test-kv delete tasks task-3"

# Verify deletion
echo -e "\n${BLUE}=== Verify task-3 was deleted ===${NC}"
echo "$ $WIT_VALUE kv --path ./test-kv get tasks task-3"
if $WIT_VALUE kv --path ./test-kv get tasks task-3 2>&1; then
    echo -e "${RED}FAILED (task-3 should not exist)${NC}"
    TESTS_FAILED=$((TESTS_FAILED + 1))
else
    echo -e "${GREEN}PASSED (task-3 correctly not found)${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi

# Test value from file
echo '{title: "From file", completed: false, priority: 3}' > task-from-file.wave
run_test "Set value from file" \
    "$WIT_VALUE kv --path ./test-kv set tasks task-from-file --file task-from-file.wave"

check_output "Verify value from file" \
    "$WIT_VALUE kv --path ./test-kv get tasks task-from-file" \
    "From file"

echo ""
echo "=============================================="
echo "  PART 3: Multiple Keyspaces"
echo "=============================================="

# Create another type
cat > user.wit << 'EOF'
package app:users;
interface types {
    record user {
        name: string,
        email: string,
        active: bool,
    }
}
EOF

run_test "Register 'user' type for 'users' keyspace" \
    "$WIT_VALUE kv --path ./test-kv set-type users --wit user.wit --type-name user"

run_test "Set a user value" \
    "$WIT_VALUE kv --path ./test-kv set users alice --value '{name: \"Alice\", email: \"alice@example.com\", active: true}'"

check_output "Get user value" \
    "$WIT_VALUE kv --path ./test-kv get users alice" \
    "alice@example.com"

check_output "List all keyspace types" \
    "$WIT_VALUE kv --path ./test-kv list-types" \
    "users"

echo ""
echo "=============================================="
echo "  PART 4: Cleanup Operations"
echo "=============================================="

# Delete type without data
run_test "Delete 'users' type (keeping data)" \
    "$WIT_VALUE kv --path ./test-kv delete-type users"

# Verify type was deleted but we can re-register
run_test "Re-register 'users' type" \
    "$WIT_VALUE kv --path ./test-kv set-type users --wit user.wit --type-name user"

# Delete type with data
run_test "Delete 'users' type with --delete-data" \
    "$WIT_VALUE kv --path ./test-kv delete-type users --delete-data"

echo ""
echo "=============================================="
echo "  PART 5: Environment Variable"
echo "=============================================="

export WIT_KV_PATH="./test-kv"
check_output "Use WIT_KV_PATH environment variable" \
    "$WIT_VALUE kv list-types" \
    "tasks"
unset WIT_KV_PATH

echo ""
echo "=============================================="
echo "  SUMMARY"
echo "=============================================="
echo ""
echo -e "Tests passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Tests failed: ${RED}$TESTS_FAILED${NC}"
echo ""

if [ $TESTS_FAILED -gt 0 ]; then
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
else
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
fi
