# wit-ast-js

JavaScript/TypeScript bindings for the [wit-ast](../../) WebAssembly component.

## Overview

wit-ast-js provides JavaScript bindings for parsing WIT (WebAssembly Interface Types) definitions and working with WAVE (WebAssembly Value Encoding) format values. It uses [jco](https://github.com/bytecodealliance/jco) to transpile the WebAssembly component into JavaScript.

## Features

- **Parse WIT definitions** into a queryable AST
- **Convert WAVE text to structured values** and back
- **Full TypeScript type definitions** for all interfaces
- **Works in Node.js** (browser support depends on WASM capabilities)

## Installation

```bash
npm install
```

## Building

```bash
# Build everything (Rust -> WASM -> JS)
npm run build

# Or step by step:
npm run build:wasm      # Compile Rust to WASM (requires cargo)
npm run build:component # Create WASM component (requires wasm-tools)
npm run transpile       # Generate JS/TS bindings (requires jco)
```

### Prerequisites

- **Rust** with `wasm32-unknown-unknown` target
- **wasm-tools** (`cargo install wasm-tools`)
- **Node.js** 18+

## Quick Start

```javascript
import { parser, formatter } from "./dist/witast.js";

// Define a WIT interface
const witDefinition = `
package example:types@0.1.0;

interface types {
  record person {
    name: string,
    age: u32,
    active: bool,
  }
}

world example {
  export types;
}
`;

// Parse the WIT definition
const ast = parser.parseWit(witDefinition);

// Parse WAVE text to value tree
const waveText = `{name: "Alice", age: 30, active: true}`;
const valueTree = formatter.waveToValueTree(ast, "person", waveText);

// Convert value tree back to WAVE text
const output = formatter.valueTreeToWave(ast, "person", valueTree);
console.log(output); // {name: "Alice", age: 30, active: true}
```

## API Reference

### Parser

#### `parser.parseWit(definition: string): WitAst`

Parse a WIT definition string into an AST resource.

```javascript
const ast = parser.parseWit(`
  package my:types;
  interface types {
    record point { x: f64, y: f64 }
  }
  world example { export types; }
`);
```

**Throws**: `ParseError` with `message`, `line`, and `column` on syntax errors.

### WitAst Resource

#### `ast.types(): TypeDef[]`

Get all type definitions from the parsed AST.

```javascript
const types = ast.types();
// [{ name: "point", kind: { tag: "type-record", val: [...] } }]
```

#### `ast.findType(name: string): number | undefined`

Find a type index by name.

```javascript
const idx = ast.findType("point"); // 0
const missing = ast.findType("unknown"); // undefined
```

### Formatter

#### `formatter.waveToValueTree(ast: WitAst, typeName: string, waveText: string): ValueTree`

Parse WAVE text into a value tree.

```javascript
const tree = formatter.waveToValueTree(ast, "point", "{x: 1.0, y: 2.0}");
// { nodes: [{ tag: "record-val", val: [...] }, ...] }
```

#### `formatter.valueTreeToWave(ast: WitAst, typeName: string, valueTree: ValueTree): string`

Convert a value tree to WAVE text.

```javascript
const wave = formatter.valueTreeToWave(ast, "point", tree);
// "{x: 1, y: 2}"
```

### Lifter

#### `lifter.lift(ast: WitAst, typeName: string, data: BinaryExport): ValueTree`

Lift canonical ABI binary data into a value tree.

```javascript
const data = {
  value: new Uint8Array([...]), // Canonical ABI encoded value
  memory: new Uint8Array([...]) // Optional: linear memory for strings/lists
};
const tree = lifter.lift(ast, "person", data);
```

## Type Definitions

### TypeDef

```typescript
interface TypeDef {
  name: string;
  kind: TypeDefKind;
}

type TypeDefKind =
  | { tag: "type-record"; val: TypeField[] }
  | { tag: "type-tuple"; val: TypeRef[] }
  | { tag: "type-enum"; val: string[] }
  | { tag: "type-variant"; val: TypeCase[] }
  | { tag: "type-flags"; val: string[] }
  | { tag: "type-option"; val: TypeRef }
  | { tag: "type-result"; val: [TypeRef?, TypeRef?] }
  | { tag: "type-list"; val: TypeRef }
  | { tag: "type-alias"; val: TypeRef };
```

### ValueTree

```typescript
interface ValueTree {
  nodes: WitValueNode[];
}

type WitValueNode =
  | { tag: "primitive"; val: PrimitiveValue }
  | { tag: "record-val"; val: FieldRef[] }
  | { tag: "tuple-val"; val: Uint32Array }
  | { tag: "list-val"; val: Uint32Array }
  | { tag: "enum-val"; val: string }
  | { tag: "variant-val"; val: VariantRef }
  | { tag: "option-val"; val: number | undefined }
  | { tag: "result-val"; val: Result<number?, number?> }
  | { tag: "flags-val"; val: string[] };
```

### PrimitiveValue

```typescript
type PrimitiveValue =
  | { tag: "bool-val"; val: boolean }
  | { tag: "u8-val"; val: number }
  | { tag: "u16-val"; val: number }
  | { tag: "u32-val"; val: number }
  | { tag: "u64-val"; val: bigint }
  | { tag: "s8-val"; val: number }
  | { tag: "s16-val"; val: number }
  | { tag: "s32-val"; val: number }
  | { tag: "s64-val"; val: bigint }
  | { tag: "f32-val"; val: number }
  | { tag: "f64-val"; val: number }
  | { tag: "char-val"; val: string }
  | { tag: "string-val"; val: string };
```

## Examples

### Working with Records

```javascript
const wit = `
package example:types;
interface types {
  record user {
    id: u64,
    name: string,
    email: option<string>,
  }
}
world example { export types; }
`;

const ast = parser.parseWit(wit);
const wave = `{id: 12345, name: "alice", email: some("alice@example.com")}`;
const tree = formatter.waveToValueTree(ast, "user", wave);

// Access the root record
const root = tree.nodes[0]; // { tag: "record-val", val: [...] }
```

### Working with Variants

```javascript
const wit = `
package example:types;
interface types {
  variant result-status {
    pending,
    success(u32),
    error(string),
  }
}
world example { export types; }
`;

const ast = parser.parseWit(wit);

// Parse different variant cases
formatter.waveToValueTree(ast, "result-status", "pending");
formatter.waveToValueTree(ast, "result-status", "success(200)");
formatter.waveToValueTree(ast, "result-status", `error("not found")`);
```

### Working with Flags

```javascript
const wit = `
package example:types;
interface types {
  flags permissions {
    read,
    write,
    execute,
  }
}
world example { export types; }
`;

const ast = parser.parseWit(wit);
const tree = formatter.waveToValueTree(ast, "permissions", "{read, write}");
// { nodes: [{ tag: "flags-val", val: ["read", "write"] }] }
```

## Running the Example

```bash
npm run all
# or
npm run build && npm run example
```

## Interactive Demo

An interactive browser-based demo is included to explore all library features:

```bash
# Build and start the demo server
npm run demo

# Or if already built:
npm run serve
```

Then open http://localhost:3000 in your browser.

### Demo Features

- **Template Selector**: Pre-defined examples for all supported types
  - Basic types: primitives, records, tuples
  - Algebraic types: enums, variants, flags
  - Container types: options, results, lists
  - Complex nested structures
- **WIT Parser**: Edit and parse WIT definitions interactively
- **Type Inspector**: View all parsed type definitions
- **WAVE Converter**: Parse WAVE text to value trees and back
- **Roundtrip Testing**: Verify WAVE -> ValueTree -> WAVE conversions

## Project Structure

```
wit-ast-js/
├── package.json      # Build scripts
├── index.html        # Interactive browser demo
├── example.mjs       # Node.js usage example
├── README.md
├── .gitignore
└── dist/             # Generated (after build)
    ├── witast.js           # Main JS module
    ├── witast.d.ts         # TypeScript definitions
    ├── witast.core.wasm    # WASM binary
    └── interfaces/         # Interface type definitions
```

## Related Projects

- [wit-ast](../../) - The Rust WASM component this package wraps
- [jco](https://github.com/bytecodealliance/jco) - JavaScript Component tooling
- [wasm-wave](https://github.com/bytecodealliance/wasm-tools) - WAVE format specification

## License

MIT
