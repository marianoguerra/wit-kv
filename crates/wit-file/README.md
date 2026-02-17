# wit-file

Read and write raw canonical ABI binary files using WIT type definitions.

## Overview

wit-file is a standalone CLI for encoding and decoding files in the [canonical ABI](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md) binary format. It uses WIT type definitions to determine the binary layout and converts between [WAVE](https://github.com/bytecodealliance/wasm-tools/tree/main/crates/wasm-wave) text and raw binary with no metadata overhead.

## File Format

Files contain the raw canonical ABI encoding:

```
[buffer bytes (flat_size)] [memory bytes (remaining)]
```

The type's flat size (from canonical ABI `SizeAlign`) determines where the buffer ends and memory begins. Types without variable-length data (strings, lists) have no memory segment.

## Installation

```bash
cargo install --path crates/wit-file
```

## Usage

### Write: WAVE text to binary

```bash
# From --value
wit-file write --wit types.wit -t point -o point.bin --value '{x: 10, y: 20}'

# From file
wit-file write --wit types.wit -t point -o point.bin --file point.wave

# From stdin
echo '{x: 10, y: 20}' | wit-file write --wit types.wit -t point -o point.bin
```

### Read: binary to WAVE text

```bash
# To stdout
wit-file read --wit types.wit -t point point.bin
# {x: 10, y: 20}

# To file
wit-file read --wit types.wit -t point point.bin -o point.wave
```

### Type inference

If the WIT file contains a single named type, the `-t` flag can be omitted:

```bash
wit-file write --wit point.wit -o point.bin --value '{x: 10, y: 20}'
```

## Supported Types

| Type | Description | Memory |
|------|-------------|--------|
| `bool` | 1 byte | No |
| `u8`/`s8` through `u64`/`s64` | Fixed-width integers | No |
| `f32`/`f64` | IEEE 754 floats | No |
| `char` | Unicode scalar (4 bytes) | No |
| `string` | UTF-8 text | Yes |
| `list<T>` | Homogeneous sequence | Yes |
| `record` | Named fields | Depends on fields |
| `tuple` | Positional elements | Depends on elements |
| `variant` | Discriminant + payload | Depends on payload |
| `enum` | Discriminant only | No |
| `option<T>` | Discriminant + payload | Depends on T |
| `result<T, E>` | Discriminant + payload | Depends on T/E |
| `flags` | Bitfield | No |

## Example: Defining a Type

```wit
// types.wit
package myapp:types;

interface types {
    record user {
        name: string,
        email: option<string>,
        active: bool,
    }
}
```

```bash
wit-file write --wit types.wit -t user -o user.bin \
  --value '{name: "Alice", email: some("alice@example.com"), active: true}'

wit-file read --wit types.wit -t user user.bin
# {name: "Alice", email: some("alice@example.com"), active: true}
```

## Dependencies

- **wit-core** - Shared WIT utilities (type resolution, canonical ABI)
- **clap** - CLI argument parsing

## License

MIT
