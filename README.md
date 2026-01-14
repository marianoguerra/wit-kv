# wit-value

A CLI tool and library for lowering and lifting WIT (WebAssembly Interface Types) values using the canonical ABI.

## Overview

`wit-value` provides functionality to:
- **Lower**: Convert WAVE-encoded values to binary format using canonical ABI
- **Lift**: Convert binary data back to WAVE-encoded representation

This is useful for debugging, testing, and understanding how WIT types are encoded in the WebAssembly Component Model.

## Installation

```bash
cargo build --release
```

## Usage

### Lower a value to binary

```bash
wit-value lower --wit types.wit --type-name point --value '{x: 42, y: 100}' --output point.bin
```

### Lift binary to WAVE representation

```bash
wit-value lift --wit types.wit --type-name point --input point.bin
# Output: {x: 42, y: 100}
```

### Variable-length types (strings, lists)

For types containing strings or lists, a `.memory` file is automatically created alongside the binary:

```bash
# Lower a string
wit-value lower --wit types.wit --type-name message --value '{text: "hello"}' --output msg.bin
# Creates: msg.bin (8 bytes) + msg.bin.memory (5 bytes)

# Lift automatically uses the .memory file
wit-value lift --wit types.wit --type-name message --input msg.bin
# Output: {text: "hello"}
```

## Supported Types

| Type | Support | Notes |
|------|---------|-------|
| Primitives (u8-u64, s8-s64, f32, f64, bool, char) | Full | Direct byte encoding |
| Records | Full | Struct layout with alignment padding |
| Tuples | Full | Same as records |
| Enums | Full | Discriminant encoding |
| Flags | Full | Bitfield encoding |
| Options | Full | Discriminant + payload |
| Results | Full | Discriminant + ok/err payload |
| Variants | Full | Discriminant + typed payload |
| Fixed-size lists | Full | Inline array encoding |
| Strings | Full | Requires .memory file |
| Lists | Full | Requires .memory file |
| Handles/Resources | No | Not applicable for standalone encoding |
| Futures/Streams | No | Not applicable for standalone encoding |

## Library Usage

```rust
use wit_value::{CanonicalAbi, LinearMemory};
use wit_parser::Resolve;

// Load WIT types
let mut resolve = Resolve::new();
resolve.push_path("types.wit")?;

// Create ABI encoder
let abi = CanonicalAbi::new(&resolve);

// For fixed-size types (no strings/lists)
let bytes = abi.lower(&value, &wit_type, &wave_type)?;
let (lifted, _) = abi.lift(&bytes, &wit_type, &wave_type)?;

// For variable-length types (strings, lists)
let mut memory = LinearMemory::new();
let bytes = abi.lower_with_memory(&value, &wit_type, &wave_type, &mut memory)?;
let (lifted, _) = abi.lift_with_memory(&bytes, &wit_type, &wave_type, &memory)?;
```

## How It Works

The canonical ABI defines how WIT types are laid out in memory:

- **Fixed-size types** are encoded directly with proper alignment
- **Variable-length types** (strings, lists) use a ptr + len pair (8 bytes), with actual data stored in linear memory

### Memory Layout Example

For a record with a string:
```
record message {
    text: string,
}
```

Main buffer (8 bytes):
```
[ptr: u32][len: u32]
```

Linear memory (.memory file):
```
[string bytes...]
```

## Development

### Run tests

```bash
cargo test
```

### Test coverage

- 32 tests total
- Property-based tests for roundtrip correctness
- Reference tests verifying canonical ABI encoding

## License

MIT
