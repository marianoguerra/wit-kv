# wit-kv

A typed key-value store for [WIT](https://component-model.bytecodealliance.org/design/wit.html) (WebAssembly Interface Types) values using the canonical ABI.

wit-kv enforces schemas at the storage layer: each keyspace is bound to a WIT type, and values are validated on every read and write. This brings the type safety of the WebAssembly Component Model to persistent storage, enabling cross-language interoperability—any language that implements the canonical ABI can read and write your data.

**Core capabilities:**

- **Typed storage** — Schema enforcement per keyspace with semantic versioning
- **Three interfaces** — HTTP API server, command-line tool, and Rust library
- **Canonical ABI encoding** — Binary format compatible with WebAssembly components
- **WASM map/reduce** — Execute components directly on stored data with full type safety
- **WAVE text format** — Human-readable syntax for all WIT types

---

## Installation

```bash
# Install CLI and server
cargo install --path . --features full

# Or build specific binaries
cargo build --release --features cli    # CLI only
cargo build --release --features server # Server only
```

## Defining Types

Types are defined using WIT syntax. Create a `.wit` file with your schema:

```wit
// types.wit
package myapp:types;

interface types {
    record user {
        name: string,
        email: string,
        active: bool,
    }

    record point {
        x: s32,
        y: s32,
    }

    variant shape {
        circle(u32),
        rectangle(point),
    }

    enum color { red, green, blue }

    flags permissions { read, write, execute }
}
```

Values use [WAVE syntax](https://github.com/bytecodealliance/wasm-tools/tree/main/crates/wasm-wave):

```
# Records
{name: "Alice", email: "alice@example.com", active: true}

# Variants
circle(50)
rectangle({x: 10, y: 20})

# Enums
green

# Flags
{read, write}

# Lists
[1, 2, 3]

# Options
some("value")
none
```

---

## Server

The HTTP API server provides a RESTful interface to wit-kv with content negotiation for WAVE text and binary formats.

### Running the Server

```bash
# Create configuration file
cat > wit-kv-server.toml << 'EOF'
[server]
bind = "127.0.0.1"
port = 8080

[[databases]]
name = "default"
path = ".wit-kv"
EOF

# Start server
wit-kv-server --config wit-kv-server.toml
```

### API Endpoints

Base path: `/api/v1`

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| **Types** |
| GET | `/db/{db}/types` | List all types |
| GET | `/db/{db}/types/{keyspace}` | Get type metadata |
| PUT | `/db/{db}/types/{keyspace}?type_name=T` | Register type |
| DELETE | `/db/{db}/types/{keyspace}?delete_data=bool` | Delete type |
| **Key-Value** |
| GET | `/db/{db}/kv/{keyspace}?prefix=&limit=` | List keys |
| GET | `/db/{db}/kv/{keyspace}/{key}` | Get value |
| PUT | `/db/{db}/kv/{keyspace}/{key}` | Set value |
| DELETE | `/db/{db}/kv/{keyspace}/{key}` | Delete value |

### Content Negotiation

**Request Content-Type:**
- `application/x-wasm-wave` or `text/plain` — WAVE text (default)
- `application/octet-stream` — Binary canonical ABI

**Response Accept header:**
- `application/x-wasm-wave` — WAVE text (default)
- `application/octet-stream` — Binary format

### Example Usage

```bash
# Register a type
curl -X PUT "http://localhost:8080/api/v1/db/default/types/points?type_name=point" \
  -H "Content-Type: text/plain" \
  -d 'package app:types;
interface types {
  record point { x: s32, y: s32 }
}'

# Store a value
curl -X PUT "http://localhost:8080/api/v1/db/default/kv/points/origin" \
  -H "Content-Type: application/x-wasm-wave" \
  -d '{x: 0, y: 0}'

# Retrieve as WAVE text
curl "http://localhost:8080/api/v1/db/default/kv/points/origin"
# {x: 0, y: 0}

# Retrieve as binary
curl "http://localhost:8080/api/v1/db/default/kv/points/origin" \
  -H "Accept: application/octet-stream" -o point.bin

# List keys
curl "http://localhost:8080/api/v1/db/default/kv/points?prefix=o&limit=10"
# ["origin"]
```

### TypeScript Client

A TypeScript client is included in the `client/` directory:

```typescript
import { WitKvClient } from 'wit-kv-client';

const client = new WitKvClient('http://localhost:8080');

// Type management
await client.setType('points', witDefinition, { typeName: 'point' });
const types = await client.listTypes();

// Key-value operations
await client.set('points', 'p1', '{x: 10, y: 20}');
const value = await client.get('points', 'p1');           // WAVE text
const binary = await client.get('points', 'p1', { format: 'binary' }); // ArrayBuffer
const keys = await client.list('points', { prefix: 'p', limit: 100 });
await client.delete('points', 'p1');
```

```bash
cd client && npm install && npm run build
```

---

## CLI

The command-line interface provides full access to wit-kv functionality including map/reduce operations.

### Quick Start

```bash
# Initialize store and register a type
wit-kv init
wit-kv set-type users --wit types.wit --type-name user

# Store and retrieve values
wit-kv set users alice --value '{name: "Alice", email: "alice@example.com", active: true}'
wit-kv get users alice
# {name: "Alice", email: "alice@example.com", active: true}

# List and delete
wit-kv list users --prefix a
wit-kv delete users alice
```

### Command Reference

**Store Management**

| Command | Description |
|---------|-------------|
| `init` | Initialize a new store |
| `set-type <keyspace> --wit <file> -t <type>` | Register a WIT type |
| `get-type <keyspace>` | Show type definition |
| `delete-type <keyspace> [--delete-data]` | Remove type |
| `list-types` | List all keyspaces |

**Key-Value Operations**

| Command | Description |
|---------|-------------|
| `set <keyspace> <key> --value <wave>` | Store a value |
| `set <keyspace> <key> --file <path>` | Store from file |
| `get <keyspace> <key>` | Retrieve as WAVE text |
| `get <keyspace> <key> --binary` | Retrieve as binary |
| `delete <keyspace> <key>` | Delete a value |
| `list <keyspace> [--prefix P] [--limit N]` | List keys |

**Encoding (without store)**

| Command | Description |
|---------|-------------|
| `lower --wit <file> -t <type> --value <wave> -o <file>` | WAVE → binary |
| `lift --wit <file> -t <type> --input <file>` | Binary → WAVE |

**Environment:** `WIT_KV_PATH` sets the store directory (default: `.wit-kv/`)

### Map/Reduce Operations

Execute WebAssembly components to filter, transform, and aggregate stored data. Components receive actual WIT types with direct field access—no binary parsing required.

```bash
# Setup test data
wit-kv set-type points --wit ./examples/point-filter/wit/map.wit -t point
wit-kv set points p1 --value '{x: 3, y: 4}'
wit-kv set points p2 --value '{x: 10, y: 20}'

# Map: filter points (same type in/out)
wit-kv map points \
  --module ./examples/point-filter/target/wasm32-unknown-unknown/release/point_filter.wasm \
  --module-wit ./examples/point-filter/wit/map.wit \
  --input-type point

# Map: transform to different type (Point → Magnitude)
wit-kv map points \
  --module ./examples/point-to-magnitude/target/wasm32-unknown-unknown/release/point_to_magnitude.wasm \
  --module-wit ./examples/point-to-magnitude/wit/map.wit \
  --input-type point \
  --output-type magnitude
# {distance-squared: 25, quadrant: 1}

# Reduce: aggregate values
wit-kv reduce users \
  --module ./examples/sum-scores/target/wasm32-unknown-unknown/release/sum_scores.wasm \
  --module-wit ./examples/sum-scores/wit/reduce.wit \
  --input-type person \
  --state-type total
# {sum: 305, count: 3}
```

See `examples/` for sample components.

---

## Library

The Rust library provides direct access to the canonical ABI encoder and key-value store.

### Feature Flags

```toml
[dependencies]
wit-kv = { version = "0.1", features = ["kv"] }
```

| Feature | Description |
|---------|-------------|
| `kv` | Key-value store (default) |
| `wasm` | WebAssembly component execution (default) |
| `cli` | Command-line binary |
| `server` | HTTP API server |
| `full` | All features |

### API Usage

```rust
use wit_kv::{CanonicalAbi, LinearMemory, Resolve};
use wit_kv::kv::KvStore;

// Key-value store
let store = KvStore::init(".wit-kv")?;
store.set_type("users", "types.wit", Some("user"), false)?;
store.set("users", "alice", "{name: \"Alice\", email: \"a@example.com\", active: true}")?;

let value = store.get("users", "alice")?;
let keys = store.list("users", Some("a"), None, None, Some(100))?;
store.delete("users", "alice")?;

// Direct canonical ABI encoding
let mut resolve = Resolve::new();
resolve.push_path("types.wit")?;

let abi = CanonicalAbi::new(&resolve);
let mut memory = LinearMemory::new();

// Encode WAVE value to binary
let bytes = abi.lower_with_memory(&value, &wit_type, &wave_type, &mut memory)?;

// Decode binary to WAVE value
let (decoded, _) = abi.lift_with_memory(&bytes, &wit_type, &wave_type, &memory)?;
```

### Project Structure

```
wit-kv/
├── src/
│   ├── lib.rs              # Library exports
│   ├── main.rs             # CLI entry point
│   ├── abi/                # Canonical ABI implementation
│   │   ├── mod.rs          # CanonicalAbi, EncodedValue
│   │   ├── memory.rs       # LinearMemory allocator
│   │   ├── wave_lower.rs   # WAVE → binary
│   │   └── wave_lift.rs    # Binary → WAVE
│   ├── kv/                 # Key-value store
│   │   ├── store.rs        # KvStore implementation
│   │   ├── types.rs        # StoredValue, KeyspaceMetadata
│   │   └── format.rs       # WIT-based binary encoding
│   ├── wasm/               # WebAssembly execution
│   │   └── typed_runner.rs # TypedRunner for map/reduce
│   ├── server/             # HTTP API server
│   │   ├── config.rs       # TOML configuration
│   │   ├── routes/         # API handlers
│   │   └── content.rs      # Content negotiation
│   └── bin/
│       └── wit-kv-server.rs
├── client/                 # TypeScript client
├── examples/               # Map/reduce component examples
└── kv.wit                  # Storage format types
```

---

## Technical Design

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│              wit-kv-server (HTTP API)                       │
│              wit-kv (CLI)                                   │
├─────────────────────────────────────────────────────────────┤
│                    KvStore                                  │
│         (typed keyspaces, schema versioning)                │
├─────────────────────────────────────────────────────────────┤
│                  CanonicalAbi encoder                       │
│    WAVE text ←→ canonical ABI binary ←→ wasmtime::Val       │
├─────────────────────────────────────────────────────────────┤
│  wit-parser     │  wasm-wave      │  wasmtime               │
│  (WIT types)    │  (WAVE format)  │  (Wasm runtime)         │
├─────────────────┴─────────────────┴─────────────────────────┤
│                    fjall (persistent KV)                    │
└─────────────────────────────────────────────────────────────┘
```

### Canonical ABI Encoding

The [canonical ABI](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md) defines binary layout for WIT types.

**Fixed-size types** encode directly with alignment:

```
record point { x: u32, y: u32 }

Binary (8 bytes):
┌────────────┬────────────┐
│ x: u32 (4) │ y: u32 (4) │
└────────────┴────────────┘
```

**Variable-length types** use pointer+length with data in linear memory:

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
record stored-value {
    version: u8,                      // Format version
    type-version: semantic-version,   // Schema version at write time
    value: list<u8>,                  // Canonical ABI bytes
    memory: option<list<u8>>,         // Linear memory for strings/lists
}

record keyspace-metadata {
    name: string,
    qualified-name: string,           // "myapp:types/types#user"
    wit-definition: string,
    type-name: string,
    type-version: semantic-version,
    type-hash: u32,
    created-at: u64,
}
```

**Version compatibility:**
- Pre-1.0 (`0.x.y`): Patch-level compatible (`0.1.1` reads `0.1.0`)
- Post-1.0: Same major, higher minor/patch reads older

### Type Support

| WIT Type | Status | Encoding |
|----------|--------|----------|
| `bool` | ✓ | 1 byte |
| `u8`/`s8` | ✓ | 1 byte |
| `u16`/`s16` | ✓ | 2 bytes |
| `u32`/`s32` | ✓ | 4 bytes |
| `u64`/`s64` | ✓ | 8 bytes |
| `f32`/`f64` | ✓ | IEEE 754 |
| `char` | ✓ | 4 bytes |
| `string` | ✓ | ptr+len |
| `list<T>` | ✓ | ptr+len |
| `record` | ✓ | Aligned fields |
| `tuple` | ✓ | Same as record |
| `variant` | ✓ | Discriminant + payload |
| `enum` | ✓ | Discriminant |
| `option<T>` | ✓ | Discriminant + payload |
| `result<T,E>` | ✓ | Discriminant + payload |
| `flags` | ✓ | Bitfield |
| `resource` | ✗ | Requires runtime |
| `stream`/`future` | ✗ | Requires async |

---

## Development

```bash
cargo test              # Unit and integration tests
just smoke-test         # Full suite with map/reduce
```

## License

MIT
