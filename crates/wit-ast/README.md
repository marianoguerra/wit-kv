# wit-ast

A WebAssembly component for parsing WIT (WebAssembly Interface Types) definitions and working with WAVE (WebAssembly Value Encoding) format values.

## Overview

wit-ast provides a WASM component that can:

- **Parse WIT definitions** into a queryable AST (Abstract Syntax Tree)
- **Lift binary data** from canonical ABI format into structured value trees
- **Convert between WAVE text and value trees** for human-readable serialization

The component exposes a clean interface that can be used from any language with WebAssembly Component Model support, including JavaScript/TypeScript (via [wit-ast-js](./bindings/js/)).

## Features

- **WIT Parsing**: Parse WIT definition strings into an AST with full type information
- **Type Introspection**: Query type definitions by name, list all types, inspect record fields, variant cases, etc.
- **WAVE Format Support**: Parse and generate WAVE text format using the `wasm-wave` crate
- **Value Tree Representation**: Flat, index-based value representation suitable for cross-component boundaries
- **Canonical ABI Lifting**: Decode binary canonical ABI data into structured values

## Supported Types

| Type | Description |
|------|-------------|
| **Primitives** | `bool`, `u8`-`u64`, `s8`-`s64`, `f32`, `f64`, `char`, `string` |
| **Records** | Named fields with heterogeneous types |
| **Tuples** | Ordered, positional elements |
| **Lists** | Homogeneous sequences |
| **Enums** | Named cases without payloads |
| **Variants** | Named cases with optional payloads |
| **Options** | `option<T>` - some or none |
| **Results** | `result<T, E>` - ok or err with optional payloads |
| **Flags** | Bitset of named flags |

## Building

```bash
# Build for wasm32-unknown-unknown target
cargo build --target wasm32-unknown-unknown --release

# Create a WASM component (requires wasm-tools)
wasm-tools component new \
  target/wasm32-unknown-unknown/release/wit_ast.wasm \
  -o wit-ast.wasm
```

## WIT Interface

The component exports four interfaces:

### `wit-kv:wit-ast/types`

Core type definitions including:
- `wit-ast` - Resource for parsed WIT definitions
- `value-tree` - Flat array representation of values
- `wit-value-node` - Individual value nodes (primitives, records, lists, etc.)
- Error types for parsing, lifting, and formatting

### `wit-kv:wit-ast/parser`

```wit
parse-wit: func(definition: string) -> result<wit-ast, parse-error>
```

### `wit-kv:wit-ast/lifter`

```wit
lift: func(
    ast: borrow<wit-ast>,
    type-name: string,
    data: binary-export
) -> result<value-tree, lift-error>
```

### `wit-kv:wit-ast/formatter`

```wit
value-tree-to-wave: func(
    ast: borrow<wit-ast>,
    type-name: string,
    value: value-tree
) -> result<string, format-error>

wave-to-value-tree: func(
    ast: borrow<wit-ast>,
    type-name: string,
    wave-text: string
) -> result<value-tree, format-error>
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         wit-ast                             │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────┐    ┌─────────┐    ┌───────────┐               │
│  │ parser  │    │ lifter  │    │ formatter │               │
│  └────┬────┘    └────┬────┘    └─────┬─────┘               │
│       │              │               │                      │
│       ▼              ▼               ▼                      │
│  ┌─────────────────────────────────────────┐               │
│  │              wit-ast (resource)          │               │
│  │  - wit-parser for WIT parsing            │               │
│  │  - Type definitions and lookup           │               │
│  └─────────────────────────────────────────┘               │
│                      │                                      │
│       ┌──────────────┼──────────────┐                      │
│       ▼              ▼              ▼                      │
│  ┌─────────┐   ┌───────────┐  ┌───────────┐               │
│  │abi/lift │   │value_conv │  │ wasm-wave │               │
│  │abi/lower│   │           │  │ to_string │               │
│  └─────────┘   └───────────┘  └───────────┘               │
│       │              │              │                      │
│       ▼              ▼              ▼                      │
│  ┌─────────────────────────────────────────┐               │
│  │           value-tree (flat array)        │               │
│  │  - Index-based node references           │               │
│  │  - Safe for component boundaries         │               │
│  └─────────────────────────────────────────┘               │
└─────────────────────────────────────────────────────────────┘
```

## Value Tree Format

Values are represented as a flat array of nodes with index-based references, avoiding recursive types across component boundaries:

```
value-tree {
    nodes: [
        RecordVal([{name: "x", idx: 1}, {name: "y", idx: 2}]),  // Node 0 (root)
        Primitive(F64Val(3.14)),                                 // Node 1
        Primitive(F64Val(2.71)),                                 // Node 2
    ]
}
```

## Testing

```bash
# Run roundtrip and property-based tests
cargo test --test wave_roundtrip

# Tests include:
# - WAVE string roundtrip (value -> string -> value)
# - ValueTree conversion roundtrip (WaveValue -> ValueTree -> WaveValue)
# - Property-based tests with proptest for all types
# - Deep nesting tests (options, lists, records up to 5+ levels)
```

## Dependencies

- **wit-bindgen** - Component bindings generation
- **wit-parser** - WIT definition parsing
- **wasm-wave** - WAVE value types and serialization

## Related Projects

- [wit-ast-js](./bindings/js/) - JavaScript/TypeScript bindings with interactive browser demo
- [wasm-wave](https://github.com/bytecodealliance/wasm-tools/tree/main/crates/wasm-wave) - WAVE format specification

## License

MIT
