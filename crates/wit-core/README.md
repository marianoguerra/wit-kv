# wit-core

Shared WIT utilities for type resolution, canonical ABI encoding, and WAVE helpers.

## Overview

wit-core extracts the common functionality used across the wit-kv project into a standalone library with no database or runtime dependencies. It provides:

- **WIT type lookup** - Find types by name in a parsed WIT resolve
- **WIT loading** - Parse WIT definitions from files or strings
- **Canonical ABI** - Encoding/decoding values via the `abi` module
- **WAVE helpers** - Re-exports from `wasm-wave` for text serialization

## Usage

```rust
use wit_core::{
    CanonicalAbi, LinearMemory, Type,
    load_wit_type_from_path, resolve_wit_type, wave_from_str, wave_to_string,
};
use std::path::Path;

// Load a WIT type from file
let (resolve, type_id) = load_wit_type_from_path(
    Path::new("types.wit"),
    Some("point"),
)?;
let wave_type = resolve_wit_type(&resolve, type_id)?;

// Encode WAVE text to canonical ABI binary
let value = wave_from_str(&wave_type, "{x: 10, y: 20}")?;
let abi = CanonicalAbi::new(&resolve);
let ty = Type::Id(type_id);
let mut memory = LinearMemory::new();
let buffer = abi.lower_with_memory(&value, &ty, &wave_type, &mut memory)?;

// Decode binary back to WAVE text
let (decoded, _) = abi.lift_with_memory(&buffer, &ty, &wave_type, &memory)?;
let text = wave_to_string(&decoded)?;
```

## API

### Type Lookup

| Function | Description |
|----------|-------------|
| `load_wit_type_from_path(path, type_name)` | Parse a WIT file and resolve a type by name |
| `load_wit_type_from_string(wit_text, type_name)` | Parse WIT from a string and resolve a type |
| `find_type_by_name(resolve, name)` | Find a type by name in a resolve |
| `find_first_named_type(resolve)` | Find the first named type in a resolve |

### Re-exports

| Source | Types |
|--------|-------|
| `abi` | `CanonicalAbi`, `LinearMemory`, `EncodedValue`, `CanonicalAbiError` |
| `wit-parser` | `Resolve`, `Type`, `TypeId` |
| `wasm-wave` | `Value`, `WaveType`, `resolve_wit_type`, `wave_from_str`, `wave_to_string` |

## Dependencies

- **wit-parser** - WIT definition parsing
- **wasm-wave** - WAVE value types and serialization
- **wasmtime** - Optional, for direct `Val` conversion (`val` feature)

## License

MIT
