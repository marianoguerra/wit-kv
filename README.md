# wit-kv

A suite of tools for working with [WIT](https://component-model.bytecodealliance.org/design/wit.html) (WebAssembly Interface Types) values using the canonical ABI.

The project brings the type safety of the WebAssembly Component Model to storage, file encoding, and data processing. Values are validated against WIT schemas, encoded using the [canonical ABI](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md) binary format, and represented in [WAVE](https://github.com/bytecodealliance/wasm-tools/tree/main/crates/wasm-wave) text for human readability.

---

## Projects

| Crate | Description |
|-------|-------------|
| [`wit-core`](crates/wit-core/) | Shared WIT utilities: type resolution, canonical ABI encoding, WAVE helpers |
| [`wit-kv`](crates/wit-kv/) | Typed key-value store library with WASM map/reduce |
| [`wit-kv-cli`](crates/wit-kv-cli/) | CLI for the key-value store |
| [`wit-kv-server`](crates/wit-kv-server/) | HTTP API server with content negotiation |
| [`wit-file`](crates/wit-file/) | CLI for reading/writing raw canonical ABI binary files |
| [`wit-fs`](crates/wit-fs/) | FUSE filesystem with WAVE text and canonical ABI binary views |
| [`wit-ast`](crates/wit-ast/) | Standalone WASM component for WIT parsing in browser/edge runtimes |

### How they relate

```
                        ┌──────────────┐
                        │   wit-core   │  Canonical ABI, type resolution, WAVE helpers
                        └──┬─────┬─────┬──┘
                           │     │     │
              ┌────────────┘     │     └──────────────┐
              │                  │                    │
       ┌──────┴───────┐  ┌──────┴──────┐  ┌──────────┴───┐
       │    wit-kv     │  │   wit-file  │  │    wit-fs    │
       │  (KV store +  │  │ (raw binary │  │    (FUSE     │
       │   WASM exec)  │  │    files)   │  │  filesystem) │
       └──┬────────┬───┘  └─────────────┘  └──────────────┘
          │        │
   ┌──────┴──┐  ┌──┴──────────┐
   │wit-kv-  │  │ wit-kv-     │
   │cli      │  │ server      │
   └─────────┘  └─────────────┘
```

- **wit-core** is the encoding engine and type resolution layer, shared by `wit-kv`, `wit-file`, and `wit-fs`
- **wit-kv** adds the persistent KV store (fjall) and WASM execution (wasmtime)
- **wit-file** is a lightweight CLI that only needs `wit-core` for raw file I/O
- **wit-fs** is a FUSE filesystem that exposes WIT-typed files with WAVE text and binary views
- **wit-ast** is independent — a WASM component for in-browser WIT parsing

---

## Quick Start

### wit-file: Encode/decode binary files

```bash
# Install
cargo install --path crates/wit-file

# Define a type
cat > point.wit << 'EOF'
package app:types;
interface types { record point { x: s32, y: s32 } }
EOF

# Write WAVE text to binary
wit-file write --wit point.wit -t point -o point.bin --value '{x: 10, y: 20}'

# Read binary back to WAVE text
wit-file read --wit point.wit -t point point.bin
# {x: 10, y: 20}
```

### wit-kv: Typed key-value store

```bash
# Install
cargo install --path crates/wit-kv-cli

# Initialize and use
wit-kv init
wit-kv set-type users --wit types.wit -t user
wit-kv set users alice --value '{name: "Alice", email: "alice@example.com", active: true}'
wit-kv get users alice
```

### wit-kv-server: HTTP API

```bash
cargo install --path crates/wit-kv-server
wit-kv-server --config server.toml
```

---

## Defining Types

Types are defined using WIT syntax:

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

## Canonical ABI Encoding

The [canonical ABI](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md) defines binary layout for WIT types.

**Fixed-size types** encode directly with alignment:

```
record point { x: u32, y: u32 }

Binary (8 bytes):
+-----------+-----------+
| x: u32(4) | y: u32(4) |
+-----------+-----------+
```

**Variable-length types** use pointer+length with data in linear memory:

```
record message { text: string }

Main buffer (8 bytes):         Linear memory:
+-----------+-----------+      +----------------+
| ptr: u32  | len: u32  | ---> | "hello world"  |
+-----------+-----------+      +----------------+
```

### Type Support

| WIT Type | Encoding |
|----------|----------|
| `bool` | 1 byte |
| `u8`/`s8` | 1 byte |
| `u16`/`s16` | 2 bytes |
| `u32`/`s32` | 4 bytes |
| `u64`/`s64` | 8 bytes |
| `f32`/`f64` | IEEE 754 |
| `char` | 4 bytes |
| `string` | ptr+len |
| `list<T>` | ptr+len |
| `record` | Aligned fields |
| `tuple` | Same as record |
| `variant` | Discriminant + payload |
| `enum` | Discriminant |
| `option<T>` | Discriminant + payload |
| `result<T,E>` | Discriminant + payload |
| `flags` | Bitfield |

---

## Development

```bash
# Build everything
just dist

# Run all tests
just test

# Run wit-file smoke tests
just smoke-test-wit-file

# Run full smoke tests (requires example WASM components)
just smoke-test

# Clippy
cargo clippy --workspace
```

### Project Structure

```
wit-kv/
├── Cargo.toml              # Workspace manifest
├── crates/
│   ├── wit-core/           # Canonical ABI, type resolution + WAVE helpers
│   ├── wit-kv/             # KV store library (fjall + wasmtime)
│   │   └── examples/       # Map/reduce example WASM components
│   ├── wit-kv-cli/         # CLI binary
│   ├── wit-kv-server/      # HTTP server binary
│   │   ├── client/         # TypeScript client
│   │   └── playground/     # Interactive web UI
│   ├── wit-file/           # Raw binary file CLI
│   ├── wit-fs/             # FUSE filesystem
│   └── wit-ast/            # Standalone WASM component for WIT parsing
```

## License

MIT
