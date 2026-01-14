# wit-kv

A typed key-value store and CLI tool for working with [WIT](https://component-model.bytecodealliance.org/design/wit.html) (WebAssembly Interface Types) values using the canonical ABI.

## Features

- **Typed storage** — Store values with WIT schema enforcement per keyspace
- **Canonical ABI encoding** — Convert between human-readable WAVE text and binary format
- **WebAssembly map/reduce** — Execute Wasm components to filter, transform, and aggregate data
- **Schema versioning** — Track type versions and detect incompatibilities
- **Cross-language interop** — Any language implementing canonical ABI can read/write data

## Quick Start

```bash
# Install
cargo install --path .

# Initialize store and register a type
wit-kv init
wit-kv set-type users --wit types.wit --type-name user

# Store and retrieve values (WAVE syntax)
wit-kv set users alice --value '{name: "Alice", email: "alice@example.com", active: true}'
wit-kv get users alice
# {name: "Alice", email: "alice@example.com", active: true}
```

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
# Binary at ./target/release/wit-kv
```

## Usage Guide

### Working with Types

Define your types in a WIT file:

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

Register types for keyspaces:

```bash
wit-kv set-type users --wit types.wit --type-name user
wit-kv get-type users          # Show WIT definition
wit-kv list-types              # List all keyspaces
wit-kv delete-type users       # Remove type (add --delete-data to remove values)
```

### Storing and Retrieving Values

```bash
# Store values using WAVE syntax
wit-kv set users alice --value '{name: "Alice", email: "alice@example.com", active: true}'
wit-kv set users bob --file bob.wave    # Or read from file

# Retrieve values
wit-kv get users alice                  # WAVE text output
wit-kv get users alice --binary         # Binary export format

# List and delete
wit-kv list users                       # All keys
wit-kv list users --prefix a            # Keys starting with "a"
wit-kv delete users alice
```

### Encoding and Decoding

Convert between WAVE text and canonical ABI binary without using the store:

```bash
# Lower: WAVE text → canonical ABI binary
wit-kv lower --wit types.wit -t user \
  --value '{name: "Alice", email: "alice@example.com", active: true}' \
  --output alice.bin

# Lift: canonical ABI binary → WAVE text
wit-kv lift --wit types.wit -t user --input alice.bin
# {name: "Alice", email: "alice@example.com", active: true}
```

### Map/Reduce Operations

Execute WebAssembly Components to filter, transform, and aggregate stored values.

Components receive actual WIT types with direct field access:

```bash
# Map: filter and transform points (same type in/out)
wit-kv map points \
  --module ./examples/point-filter/target/wasm32-unknown-unknown/release/point_filter.wasm \
  --module-wit ./examples/point-filter/wit/map.wit \
  --input-type point

# Map: transform to different type (T -> T1)
wit-kv map points \
  --module ./examples/point-to-magnitude/target/wasm32-unknown-unknown/release/point_to_magnitude.wasm \
  --module-wit ./examples/point-to-magnitude/wit/map.wit \
  --input-type point \
  --output-type magnitude
# Input:  {x: 3, y: 4}
# Output: {distance-squared: 25, quadrant: 1}

# Reduce: aggregate with typed state
wit-kv reduce users \
  --module ./examples/sum-scores/target/wasm32-unknown-unknown/release/sum_scores.wasm \
  --module-wit ./examples/sum-scores/wit/reduce.wit \
  --input-type person \
  --state-type total
# Output: {sum: 305, count: 3}
```

See [examples/](examples/) for sample components.

## CLI Reference

### Store Commands

| Command | Description |
|---------|-------------|
| `init` | Initialize a new store |
| `set-type <keyspace> --wit <file> -t <type>` | Register a WIT type for a keyspace |
| `get-type <keyspace>` | Show the WIT definition for a keyspace |
| `delete-type <keyspace>` | Remove a keyspace type |
| `list-types` | List all registered keyspaces and their types |
| `set <keyspace> <key> --value <wave>` | Store a value |
| `get <keyspace> <key>` | Retrieve a value |
| `delete <keyspace> <key>` | Delete a value |
| `list <keyspace>` | List keys in a keyspace |

**Flags:** `--path <dir>` (store location), `--force` (overwrite type), `--delete-data` (with delete-type), `--binary` (binary output), `--file <path>` (read value from file), `--prefix`/`--start`/`--end`/`--limit` (for list)

### Encoding Commands

| Command | Description |
|---------|-------------|
| `lower --wit <file> -t <type> --value <wave> -o <file>` | WAVE text → canonical ABI binary |
| `lift --wit <file> -t <type> --input <file>` | Canonical ABI binary → WAVE text |

### Map/Reduce Commands

| Command | Description |
|---------|-------------|
| `map <keyspace> --module <wasm> --module-wit <wit> --input-type <type> [--output-type <type>]` | Typed map operation (output-type enables T -> T1) |
| `reduce <keyspace> --module <wasm> --module-wit <wit> --input-type <type> --state-type <type>` | Typed reduce operation |

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `WIT_KV_PATH` | Store directory path | `.wit-kv/` |

## Examples

### Complex WIT Types

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

# Records
wit-kv lower --wit shapes.wit -t point --value '{x: -5, y: 100}' -o point.bin
```

### Reading Values from Files

```bash
echo '{name: "Charlie", email: "charlie@example.com", active: true}' > charlie.wave
wit-kv set users charlie --file charlie.wave
```

### Binary Output for Pipelines

```bash
# Output as binary-export format (can be decoded with kv.wit types)
wit-kv get users alice --binary > alice.bin
```

## Technical Design

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         wit-kv CLI                          │
├─────────────────────────────────────────────────────────────┤
│  lower/lift commands  │  KV store commands  │  map/reduce   │
├───────────────────────┴─────────────────────┴───────────────┤
│                    CanonicalAbi encoder                     │
│              (lower_with_memory / lift_with_memory)         │
├─────────────────────────────────────────────────────────────┤
│  wit-parser (WIT types)  │  wasm-wave (WAVE text format)    │
├──────────────────────────┴──────────────────────────────────┤
│         fjall (persistent KV)  │  wasmtime (Wasm runtime)   │
└─────────────────────────────────────────────────────────────┘
```

### Canonical ABI Encoding

The [canonical ABI](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md) defines how WIT types are laid out in linear memory.

**Fixed-size types** are encoded directly with proper alignment:

```
record point { x: u32, y: u32 }

Binary layout (8 bytes):
┌────────────┬────────────┐
│ x: u32 (4) │ y: u32 (4) │
└────────────┴────────────┘
```

**Variable-length types** (strings, lists) use a pointer+length pair in the main buffer, with data in linear memory:

```
record message { text: string }

Main buffer (8 bytes):         Linear memory:
┌────────────┬────────────┐    ┌─────────────────┐
│ ptr: u32   │ len: u32   │───▶│ "hello world"   │
└────────────┴────────────┘    └─────────────────┘
```

### Storage Format

Defined in `kv.wit`:

```wit
record semantic-version {
    major: u32,
    minor: u32,
    patch: u32,
}

record stored-value {
    version: u8,                      // Format version for migrations
    type-version: semantic-version,   // Schema version at write time
    value: list<u8>,                  // Canonical ABI encoded bytes
    memory: option<list<u8>>,         // Linear memory (for strings/lists)
}

record keyspace-metadata {
    name: string,
    qualified-name: string,           // e.g., "myapp:types/types#user"
    wit-definition: string,           // Full WIT source
    type-name: string,
    type-version: semantic-version,   // Semantic version (0.1.0, 1.2.3, etc.)
    type-hash: u32,                   // CRC32 of WIT definition
    created-at: u64,
}

record binary-export {
    value: list<u8>,
    memory: option<list<u8>>,
}
```

Type versions use semantic versioning with compatibility rules:
- Pre-1.0 (`0.x.y`): Only patch-level changes are compatible (e.g., `0.1.1` can read `0.1.0`)
- Post-1.0: Same major version with higher minor/patch can read older versions

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

## Development

### Project Structure

```
wit-kv/
├── src/
│   ├── main.rs           # CLI entry point
│   ├── lib.rs            # Library exports
│   ├── abi.rs            # Canonical ABI implementation
│   ├── kv/               # Key-value store module
│   │   ├── store.rs      # KvStore implementation
│   │   ├── types.rs      # StoredValue, KeyspaceMetadata
│   │   └── format.rs     # WIT-based binary encoding
│   └── wasm/             # WebAssembly execution module
│       ├── typed_runner.rs # TypedRunner (actual WIT types)
│       └── error.rs      # WasmError types
├── examples/
│   ├── point-filter/       # Typed map: Point -> Point (filter by radius)
│   ├── person-filter/      # Typed map: Person -> Person (filter by score)
│   ├── point-to-magnitude/ # Typed map: Point -> Magnitude (T -> T1 transformation)
│   └── sum-scores/         # Typed reduce: Person -> Total (sum aggregation)
├── kv.wit                # Storage format types
├── mapreduce.wit         # Map/reduce interfaces
└── test.wit              # Test type definitions
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

### Testing

```bash
cargo test              # Unit and integration tests
just smoke-test         # Full suite: tests + usage demo + map/reduce
./scripts/usage-example.sh  # Interactive CLI demonstration
```

**Test coverage:**
- Unit tests: Roundtrip encoding for all WIT types
- Property tests: Randomized roundtrip verification
- Reference tests: Wasmtime-based canonical ABI validation
- Smoke tests: End-to-end CLI verification

## License

MIT
