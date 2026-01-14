# wit-kv justfile
# Run tasks with: just <target>

# Default target
default: build

# Build the main wit-kv binary
build:
    cargo build --release

# Build all example wasm components
build-examples: build-identity-map build-high-score-filter build-sum-scores

# Build identity-map component (pure wasm, no WASI)
build-identity-map:
    cd examples/identity-map && cargo build --release --target wasm32-unknown-unknown
    wasm-tools component new examples/identity-map/target/wasm32-unknown-unknown/release/identity_map.wasm \
        -o examples/identity-map/target/identity_map.component.wasm

# Build high-score-filter component (pure wasm, no WASI)
build-high-score-filter:
    cd examples/high-score-filter && cargo build --release --target wasm32-unknown-unknown
    wasm-tools component new examples/high-score-filter/target/wasm32-unknown-unknown/release/high_score_filter.wasm \
        -o examples/high-score-filter/target/high_score_filter.component.wasm

# Build sum-scores reduce component (pure wasm, no WASI)
build-sum-scores:
    cd examples/sum-scores && cargo build --release --target wasm32-unknown-unknown
    wasm-tools component new examples/sum-scores/target/wasm32-unknown-unknown/release/sum_scores.wasm \
        -o examples/sum-scores/target/sum_scores.component.wasm

# Run all tests
test:
    cargo test

# Run clippy
clippy:
    cargo clippy

# Run the usage example script
usage-example: build
    ./scripts/usage-example.sh release

# Run map-low examples
test-map-examples: build build-identity-map build-high-score-filter
    #!/usr/bin/env bash
    set -e
    echo "=== Setting up test environment ==="
    rm -rf /tmp/smoke-test-kv
    mkdir -p /tmp/smoke-test-kv
    ./target/release/wit-kv init --path /tmp/smoke-test-kv
    ./target/release/wit-kv set-type users --wit test.wit --type-name person --path /tmp/smoke-test-kv
    ./target/release/wit-kv set users alice --value "{age: 30, score: 100}" --path /tmp/smoke-test-kv
    ./target/release/wit-kv set users bob --value "{age: 25, score: 85}" --path /tmp/smoke-test-kv
    ./target/release/wit-kv set users charlie --value "{age: 35, score: 120}" --path /tmp/smoke-test-kv
    echo ""
    echo "=== Test identity-map (should return all 3 records) ==="
    ./target/release/wit-kv map-low users \
        --module ./examples/identity-map/target/identity_map.component.wasm \
        --path /tmp/smoke-test-kv
    echo ""
    echo "=== Test high-score-filter (should return 2 records with score >= 100) ==="
    ./target/release/wit-kv map-low users \
        --module ./examples/high-score-filter/target/high_score_filter.component.wasm \
        --path /tmp/smoke-test-kv
    echo ""
    echo "=== Map examples passed ==="

# Run reduce-low example
test-reduce-example: build build-sum-scores
    #!/usr/bin/env bash
    set -e
    echo "=== Setting up test environment ==="
    rm -rf /tmp/smoke-test-kv
    mkdir -p /tmp/smoke-test-kv
    ./target/release/wit-kv init --path /tmp/smoke-test-kv
    ./target/release/wit-kv set-type users --wit test.wit --type-name person --path /tmp/smoke-test-kv
    ./target/release/wit-kv set users alice --value "{age: 30, score: 100}" --path /tmp/smoke-test-kv
    ./target/release/wit-kv set users bob --value "{age: 25, score: 85}" --path /tmp/smoke-test-kv
    ./target/release/wit-kv set users charlie --value "{age: 35, score: 120}" --path /tmp/smoke-test-kv
    echo ""
    echo "=== Test sum-scores reduce (should return 305 = 100 + 85 + 120) ==="
    ./target/release/wit-kv reduce-low users \
        --module ./examples/sum-scores/target/sum_scores.component.wasm \
        --state-wit ./examples/sum-scores/state.wit \
        --state-type total \
        --path /tmp/smoke-test-kv
    echo ""
    echo "=== Reduce example passed ==="

# Run all smoke tests (usage example + map/reduce examples + unit tests)
smoke-test: build build-examples
    #!/usr/bin/env bash
    set -e
    echo "========================================"
    echo "  SMOKE TEST: wit-kv"
    echo "========================================"
    echo ""

    echo ">>> Running unit tests..."
    cargo test
    echo ""

    echo ">>> Running usage example script..."
    ./scripts/usage-example.sh release
    echo ""

    echo ">>> Setting up map/reduce test environment..."
    rm -rf /tmp/smoke-test-kv
    mkdir -p /tmp/smoke-test-kv
    ./target/release/wit-kv init --path /tmp/smoke-test-kv
    ./target/release/wit-kv set-type users --wit test.wit --type-name person --path /tmp/smoke-test-kv
    ./target/release/wit-kv set users alice --value "{age: 30, score: 100}" --path /tmp/smoke-test-kv
    ./target/release/wit-kv set users bob --value "{age: 25, score: 85}" --path /tmp/smoke-test-kv
    ./target/release/wit-kv set users charlie --value "{age: 35, score: 120}" --path /tmp/smoke-test-kv
    echo ""

    echo ">>> Testing map-low with identity-map..."
    OUTPUT=$(./target/release/wit-kv map-low users \
        --module ./examples/identity-map/target/identity_map.component.wasm \
        --path /tmp/smoke-test-kv 2>&1)
    echo "$OUTPUT"
    if echo "$OUTPUT" | grep -q "3 mapped"; then
        echo "PASSED: identity-map returned 3 records"
    else
        echo "FAILED: identity-map should return 3 records"
        exit 1
    fi
    echo ""

    echo ">>> Testing map-low with high-score-filter..."
    OUTPUT=$(./target/release/wit-kv map-low users \
        --module ./examples/high-score-filter/target/high_score_filter.component.wasm \
        --path /tmp/smoke-test-kv 2>&1)
    echo "$OUTPUT"
    if echo "$OUTPUT" | grep -q "2 mapped"; then
        echo "PASSED: high-score-filter returned 2 records"
    else
        echo "FAILED: high-score-filter should return 2 records"
        exit 1
    fi
    echo ""

    echo ">>> Testing reduce-low with sum-scores..."
    OUTPUT=$(./target/release/wit-kv reduce-low users \
        --module ./examples/sum-scores/target/sum_scores.component.wasm \
        --state-wit ./examples/sum-scores/state.wit \
        --state-type total \
        --path /tmp/smoke-test-kv 2>&1)
    echo "$OUTPUT"
    if echo "$OUTPUT" | grep -q "305"; then
        echo "PASSED: sum-scores returned 305 (100 + 85 + 120)"
    else
        echo "FAILED: sum-scores should return 305"
        exit 1
    fi
    echo ""

    echo "========================================"
    echo "  ALL SMOKE TESTS PASSED"
    echo "========================================"

# Clean build artifacts
clean:
    cargo clean
    rm -rf examples/identity-map/target
    rm -rf examples/high-score-filter/target
    rm -rf examples/sum-scores/target
