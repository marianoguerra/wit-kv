# wit-kv

A typed key-value store for WIT values using the canonical ABI.

## Overview

wit-kv enforces schemas at the storage layer: each keyspace is bound to a WIT type, and values are validated on every read and write. This brings the type safety of the WebAssembly Component Model to persistent storage, enabling cross-language interoperability.

**Capabilities:**

- **Typed storage** - Schema enforcement per keyspace with semantic versioning
- **Canonical ABI encoding** - Binary format compatible with WebAssembly components
- **WASM map/reduce** - Execute components directly on stored data with full type safety
- **WAVE text format** - Human-readable syntax for all WIT types

## Features

| Feature | Description |
|---------|-------------|
| `kv` (default) | Key-value store (fjall backend) |
| `wasm` (default) | WebAssembly component execution (wasmtime) |
| `logging` | Tracing-based logging |

```toml
[dependencies]
wit-kv = { version = "0.1", features = ["kv"] }
```

## API Usage

```rust
use wit_kv::{CanonicalAbi, LinearMemory, Resolve};
use wit_kv::kv::KvStore;

// Key-value store
let store = KvStore::init(".wit-kv")?;
store.set_type("users", "resources/types.wit", Some("user"), false)?;
store.set("users", "alice", "{name: \"Alice\", email: \"a@example.com\", active: true}")?;

let value = store.get("users", "alice")?;
let keys = store.list("users", Some("a"), None, None, Some(100))?;
store.delete("users", "alice")?;

// Direct canonical ABI encoding
let mut resolve = Resolve::new();
resolve.push_path("resources/types.wit")?;

let abi = CanonicalAbi::new(&resolve);
let mut memory = LinearMemory::new();

// Encode WAVE value to binary
let bytes = abi.lower_with_memory(&value, &wit_type, &wave_type, &mut memory)?;

// Decode binary to WAVE value
let (decoded, _) = abi.lift_with_memory(&bytes, &wit_type, &wave_type, &memory)?;
```

## CLI

The command-line interface is provided by the [`wit-kv-cli`](../wit-kv-cli/) crate.

```bash
# Initialize store and register a type
wit-kv init
wit-kv set-type users --wit resources/types.wit --type-name user

# Store and retrieve values
wit-kv set users alice --value '{name: "Alice", email: "alice@example.com", active: true}'
wit-kv get users alice

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
| `lower --wit <file> -t <type> --value <wave> -o <file>` | WAVE to binary |
| `lift --wit <file> -t <type> --input <file>` | Binary to WAVE |

**Environment:** `WIT_KV_PATH` sets the store directory (default: `.wit-kv/`)

### Map/Reduce

Execute WebAssembly components to filter, transform, and aggregate stored data:

```bash
# Map: filter and transform points
wit-kv map points \
  --module ./examples/point-filter/target/.../point_filter.wasm \
  --module-wit ./examples/point-filter/wit/map.wit \
  --input-type point

# Reduce: aggregate values
wit-kv reduce users \
  --module ./examples/sum-scores/target/.../sum_scores.wasm \
  --module-wit ./examples/sum-scores/wit/reduce.wit \
  --input-type person --state-type total
```

## Server

The HTTP API server is provided by the [`wit-kv-server`](../wit-kv-server/) crate.

### API Endpoints

Base path: `/api/v1`

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| GET | `/databases` | List all databases |
| GET | `/db/{db}/types` | List all keyspaces |
| GET | `/db/{db}/types/{keyspace}` | Get type metadata |
| PUT | `/db/{db}/types/{keyspace}?type_name=T` | Register type |
| DELETE | `/db/{db}/types/{keyspace}?delete_data=bool` | Delete type |
| GET | `/db/{db}/kv/{keyspace}?prefix=&limit=` | List keys |
| GET | `/db/{db}/kv/{keyspace}/{key}` | Get value |
| PUT | `/db/{db}/kv/{keyspace}/{key}` | Set value |
| DELETE | `/db/{db}/kv/{keyspace}/{key}` | Delete value |
| POST | `/db/{db}/map/{keyspace}` | Execute map operation |
| POST | `/db/{db}/reduce/{keyspace}` | Execute reduce operation |

### Content Negotiation

- `application/x-wasm-wave` or `text/plain` - WAVE text (default)
- `application/octet-stream` - Binary canonical ABI

### TypeScript Client

```typescript
import { WitKvClient } from 'wit-kv-client';

const client = new WitKvClient('http://localhost:8080');
await client.setType('points', witDefinition, { typeName: 'point' });
await client.set('points', 'p1', '{x: 10, y: 20}');
const value = await client.get('points', 'p1');
```

### Playground

An interactive web UI for exploring wit-kv features. See `playground/` for details.

## Storage Format

Defined in `kv.wit`:

```wit
record stored-value {
    version: u8,
    type-version: semantic-version,
    value: list<u8>,
    memory: option<list<u8>>,
}

record keyspace-metadata {
    name: string,
    qualified-name: string,
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

## Dependencies

- **wit-core** - Shared WIT utilities and canonical ABI encoding/decoding
- **fjall** - Persistent KV storage (optional, `kv` feature)
- **wasmtime** - WebAssembly runtime (optional, `wasm` feature)

## License

MIT
