# wit-kv

A typed key-value store and CLI tool for working with [WIT](https://component-model.bytecodealliance.org/design/wit.html) (WebAssembly Interface Types) values using the canonical ABI.

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
# Binary at ./target/release/wit-kv
```

## Getting Started

### Define your types in WIT

```wit
// types.wit
package myapp:types;

interface types {
    record user {
        name: string,
        email: string,
        active: bool,
    }
}
```

### Store typed values

```bash
# Initialize a store
wit-kv init

# Register a type for a keyspace
wit-kv set-type users --wit types.wit --type-name user

# Store values using WAVE syntax
wit-kv set users alice --value '{name: "Alice", email: "alice@example.com", active: true}'
wit-kv set users bob --value '{name: "Bob", email: "bob@example.com", active: false}'

# Retrieve values
wit-kv get users alice
# {name: "Alice", email: "alice@example.com", active: true}

# List keys
wit-kv list users
# alice
# bob
```

### Convert between formats

```bash
# Lower WAVE text to canonical ABI binary
wit-kv lower --wit types.wit --type-name user \
  --value '{name: "Alice", email: "alice@example.com", active: true}' \
  --output alice.bin

# Lift binary back to WAVE text
wit-kv lift --wit types.wit --type-name user --input alice.bin
# {name: "Alice", email: "alice@example.com", active: true}
```

## CLI Reference

### Key-Value Store Commands

| Command | Description |
|---------|-------------|
| `init` | Initialize a new store |
| `set-type <keyspace> --wit <file>` | Register a WIT type for a keyspace (add `--force` to overwrite) |
| `get-type <keyspace>` | Show the WIT definition for a keyspace (add `--binary` for raw format) |
| `delete-type <keyspace>` | Remove a keyspace type (add `--delete-data` to remove values too) |
| `list-types` | List all registered keyspaces and their types |
| `set <keyspace> <key> --value <wave>` | Store a value (or use `--file <path>` to read from file) |
| `get <keyspace> <key>` | Retrieve a value (add `--binary` for canonical ABI format) |
| `delete <keyspace> <key>` | Delete a value |
| `list <keyspace>` | List keys (supports `--prefix` and `--limit`) |

All KV commands accept `--path <dir>` to specify the store location (default: `.wit-kv/`).

### Encoding Commands

| Command | Description |
|---------|-------------|
| `lower --wit <file> --value <wave> --output <file>` | Convert WAVE text to canonical ABI binary |
| `lift --wit <file> --input <file>` | Convert canonical ABI binary to WAVE text (add `--output <file>` for file output) |

Both commands accept `--type-name <name>` (or `-t`) to select a specific type from the WIT file.

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `WIT_KV_PATH` | Store directory path | `.wit-kv/` |

### Map/Reduce Commands

wit-kv supports executing WebAssembly Components to filter, transform, and aggregate values in a keyspace.

| Command | Description |
|---------|-------------|
| `map <keyspace> --module <wasm> --module-wit <wit> --input-type <type>` | **Typed** map: components receive actual WIT types |
| `reduce <keyspace> --module <wasm> --module-wit <wit> --input-type <type> --state-type <type>` | **Typed** reduce: fold with typed state |
| `map-low <keyspace> --module <wasm>` | **Low-level** map: components receive `binary-export` blobs |
| `reduce-low <keyspace> --module <wasm>` | **Low-level** reduce: fold with `binary-export` state |

**Typed vs Low-level:**
- **Typed (`map`, `reduce`)**: Components use actual WIT types like `filter(value: point) -> bool` or `reduce(state: total, value: person) -> total`. Clean, type-safe, direct field access.
- **Low-level (`map-low`, `reduce-low`)**: Components receive opaque `binary-export` bytes. More flexible, but requires manual parsing.

See [examples/](examples/) for sample components.

### Examples

**Working with complex types:**

```wit
// shapes.wit
package demo:shapes;

interface types {
    record point { x: s32, y: s32 }

    variant shape {
        circle(u32),
        rectangle(point),
        triangle(tuple<point, point, point>),
    }

    flags permissions { read, write, execute }

    enum color { red, green, blue }
}
```

```bash
# Enums
wit-kv lower --wit shapes.wit -t color --value 'green' -o color.bin

# Variants
wit-kv lower --wit shapes.wit -t shape --value 'circle(50)' -o shape.bin
wit-kv lower --wit shapes.wit -t shape --value 'rectangle({x: 10, y: 20})' -o rect.bin

# Flags
wit-kv lower --wit shapes.wit -t permissions --value '{read, write}' -o perms.bin

# Records with nested types
wit-kv lower --wit shapes.wit -t point --value '{x: -5, y: 100}' -o point.bin
```

**Reading values from files:**

```bash
# Store WAVE value in a file
echo '{name: "Charlie", email: "charlie@example.com", active: true}' > charlie.wave

# Set from file
wit-kv set users charlie --file charlie.wave
```

**Binary output for pipelines:**

```bash
# Output as binary-export format (canonical ABI, can be decoded with kv.wit types)
wit-kv get users alice --binary > alice.bin
```

**Map/reduce with WebAssembly Components:**

```bash
# Typed map: filter points within radius 100, double coordinates
wit-kv map points \
  --module ./examples/typed-point-filter/target/wasm32-unknown-unknown/release/typed_point_filter.wasm \
  --module-wit ./examples/typed-point-filter/wit/typed-map.wit \
  --input-type point

# Typed reduce: sum all scores with typed state
wit-kv reduce users \
  --module ./examples/typed-sum-scores/target/wasm32-unknown-unknown/release/typed_sum_scores.wasm \
  --module-wit ./examples/typed-sum-scores/wit/typed-reduce.wit \
  --input-type person \
  --state-type total
# Output: {sum: 305, count: 3}

# Low-level map: filter users with score >= 100
wit-kv map-low users --module ./examples/high-score-filter/target/high_score_filter.component.wasm

# Low-level reduce: sum all scores
wit-kv reduce-low users \
  --module ./examples/sum-scores/target/sum_scores.component.wasm \
  --state-wit ./examples/sum-scores/state.wit \
  --state-type total
```

---

## Design

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         wit-kv CLI                          │
├─────────────────────────────────────────────────────────────┤
│  lower/lift commands  │  KV store commands                  │
├───────────────────────┴─────────────────────────────────────┤
│                    CanonicalAbi encoder                     │
│              (lower_with_memory / lift_with_memory)         │
├─────────────────────────────────────────────────────────────┤
│  wit-parser (WIT types)  │  wasm-wave (WAVE text format)    │
├──────────────────────────┴──────────────────────────────────┤
│                     fjall (persistent KV)                   │
└─────────────────────────────────────────────────────────────┘
```

### Canonical ABI Encoding

The [canonical ABI](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md) defines how WIT types are laid out in linear memory. wit-kv implements this encoding for standalone use outside of WebAssembly.

**Fixed-size types** are encoded directly with proper alignment:

```
record point { x: u32, y: u32 }

Binary layout (8 bytes):
┌────────────┬────────────┐
│ x: u32 (4) │ y: u32 (4) │
└────────────┴────────────┘
```

**Variable-length types** (strings, lists) use a pointer+length pair in the main buffer, with data stored separately in linear memory:

```
record message { text: string }

Main buffer (8 bytes):         Linear memory:
┌────────────┬────────────┐    ┌─────────────────┐
│ ptr: u32   │ len: u32   │───▶│ "hello world"   │
└────────────┴────────────┘    └─────────────────┘
```

The `lower`/`lift` commands and `--binary` flag use the `binary-export` format (defined in `kv.wit`) which packages both the main buffer and linear memory into a single file.

### Storage Format

The KV store uses WIT-defined types for its internal format (defined in `kv.wit`):

```wit
record stored-value {
    version: u8,              // Format version for migrations
    type-version: u32,        // Schema version at write time
    value: list<u8>,          // Canonical ABI encoded bytes
    memory: option<list<u8>>, // Linear memory (for strings/lists)
}

record keyspace-metadata {
    name: string,             // Keyspace name
    qualified-name: string,   // e.g., "myapp:types/types#user"
    wit-definition: string,   // Full WIT source
    type-name: string,        // Type within the WIT file
    type-version: u32,        // Incremented on schema changes
    type-hash: u32,           // CRC32 of WIT definition
    created-at: u64,          // Unix timestamp
}

record binary-export {
    value: list<u8>,          // Canonical ABI encoded bytes
    memory: option<list<u8>>, // Linear memory (for strings/lists)
}
```

This self-describing format enables:
- **Schema evolution**: Type version tracking for migrations
- **Cross-language interop**: Any language implementing canonical ABI can read the data
- **Debugging**: Values can be lifted back to human-readable WAVE format

### Type Support

| WIT Type | Status | Encoding |
|----------|--------|----------|
| `bool` | Full | 1 byte (0 or 1) |
| `u8`/`s8` | Full | 1 byte |
| `u16`/`s16` | Full | 2 bytes, aligned |
| `u32`/`s32` | Full | 4 bytes, aligned |
| `u64`/`s64` | Full | 8 bytes, aligned |
| `f32`/`f64` | Full | IEEE 754 |
| `char` | Full | Unicode scalar (4 bytes) |
| `string` | Full | ptr+len, data in memory |
| `list<T>` | Full | ptr+len, elements in memory |
| `record` | Full | Fields with alignment padding |
| `tuple` | Full | Same as record |
| `variant` | Full | Discriminant + payload |
| `enum` | Full | Discriminant only |
| `option<T>` | Full | Discriminant + optional payload |
| `result<T, E>` | Full | Discriminant + ok/err payload |
| `flags` | Full | Bitfield |
| `resource` | No | Requires runtime context |
| `stream`/`future` | No | Requires async runtime |

---

## Development

### Project Structure

```
wit-kv/
├── src/
│   ├── main.rs      # CLI entry point
│   ├── lib.rs       # Library exports
│   ├── abi.rs       # Canonical ABI implementation
│   ├── kv/          # Key-value store module
│   │   ├── store.rs # KvStore implementation
│   │   ├── types.rs # StoredValue, KeyspaceMetadata
│   │   └── format.rs# WIT-based binary encoding
│   └── wasm/        # WebAssembly execution module
│       ├── runner.rs       # Low-level WasmRunner (binary-export)
│       ├── typed_runner.rs # TypedRunner (actual WIT types)
│       ├── map.rs          # MapOperation
│       └── reduce.rs       # ReduceOperation
├── examples/
│   ├── typed-point-filter/   # Typed map example (filter by radius)
│   ├── typed-person-filter/  # Typed map example (filter by score)
│   ├── typed-sum-scores/     # Typed reduce example (sum aggregation)
│   ├── identity-map/         # Low-level map (pass-through)
│   ├── high-score-filter/    # Low-level map (filter by score)
│   └── sum-scores/           # Low-level reduce (sum aggregation)
├── kv.wit           # Storage format type definitions
├── mapreduce.wit    # Map/reduce interface definitions
├── test.wit         # Test type definitions
└── scripts/
    └── usage-example.sh  # Smoke test / tutorial
```

### Library Usage

```rust
use wit_kv::{CanonicalAbi, LinearMemory, Resolve, Value};
use wit_kv::kv::KvStore;

// Direct encoding/decoding
let mut resolve = Resolve::new();
resolve.push_path("types.wit")?;

let abi = CanonicalAbi::new(&resolve);
let mut memory = LinearMemory::new();

let bytes = abi.lower_with_memory(&value, &wit_type, &wave_type, &mut memory)?;
let (lifted, _) = abi.lift_with_memory(&bytes, &wit_type, &wave_type, &memory)?;

// Key-value store
let store = KvStore::open(".wit-kv")?;
store.set("users", "alice", "{name: \"Alice\", ...}")?;
let value = store.get("users", "alice")?;
```

### Running Tests

```bash
# Unit and integration tests
cargo test

# Full smoke test (unit tests + usage examples + map/reduce examples)
just smoke-test

# Or run individual test scripts
./scripts/usage-example.sh
```

### Test Coverage

- **Unit tests**: Roundtrip encoding for all WIT types
- **Property tests**: Randomized roundtrip verification
- **Reference tests**: Wasmtime-based canonical ABI validation
- **Smoke tests**: End-to-end CLI verification including typed and low-level map/reduce

## License

MIT
