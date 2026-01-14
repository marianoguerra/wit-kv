# wit-kv justfile
# Run tasks with: just <target>

# Default target
default: build

# Build the main wit-kv binary
build:
    cargo build --release

# Build all example wasm components
build-examples: build-identity-map build-high-score-filter build-sum-scores build-typed-point-filter build-typed-person-filter

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

# Build typed-point-filter component (uses cargo-component)
build-typed-point-filter:
    cd examples/typed-point-filter && cargo component build --release

# Build typed-person-filter component (uses cargo-component)
build-typed-person-filter:
    cd examples/typed-person-filter && cargo component build --release

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

    echo ">>> Setting up typed map test environment..."
    # Set up points keyspace for typed-point-filter
    ./target/release/wit-kv set-type points \
        --wit examples/typed-point-filter/wit/typed-map.wit \
        --type-name point \
        --path /tmp/smoke-test-kv
    ./target/release/wit-kv set points p1 --value "{x: 10, y: 20}" --path /tmp/smoke-test-kv
    ./target/release/wit-kv set points p2 --value "{x: 50, y: 50}" --path /tmp/smoke-test-kv
    ./target/release/wit-kv set points p3 --value "{x: 150, y: 0}" --path /tmp/smoke-test-kv
    ./target/release/wit-kv set points p4 --value "{x: 3, y: 4}" --path /tmp/smoke-test-kv
    echo ""

    echo ">>> Testing typed map with typed-point-filter..."
    OUTPUT=$(./target/release/wit-kv map points \
        --module ./examples/typed-point-filter/target/wasm32-wasip1/release/typed_point_filter.wasm \
        --module-wit ./examples/typed-point-filter/wit/typed-map.wit \
        --input-type point \
        --path /tmp/smoke-test-kv 2>&1)
    echo "$OUTPUT"
    # Should filter out p3 (150,0 is outside radius 100) and transform others (double coords)
    if echo "$OUTPUT" | grep -q "3 transformed" && echo "$OUTPUT" | grep -q "1 filtered"; then
        echo "PASSED: typed-point-filter returned 3 transformed, 1 filtered"
    else
        echo "FAILED: typed-point-filter should return 3 transformed, 1 filtered"
        exit 1
    fi
    echo ""

    echo ">>> Testing typed map with typed-person-filter..."
    # Reuse the users keyspace (already has person type compatible data)
    OUTPUT=$(./target/release/wit-kv map users \
        --module ./examples/typed-person-filter/target/wasm32-wasip1/release/typed_person_filter.wasm \
        --module-wit ./examples/typed-person-filter/wit/typed-map.wit \
        --input-type person \
        --path /tmp/smoke-test-kv 2>&1)
    echo "$OUTPUT"
    # alice (score 100) and charlie (score 120) pass filter, bob (85) filtered out
    if echo "$OUTPUT" | grep -q "2 transformed" && echo "$OUTPUT" | grep -q "1 filtered"; then
        echo "PASSED: typed-person-filter returned 2 transformed, 1 filtered"
    else
        echo "FAILED: typed-person-filter should return 2 transformed, 1 filtered"
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
    rm -rf examples/typed-point-filter/target
    rm -rf examples/typed-person-filter/target
