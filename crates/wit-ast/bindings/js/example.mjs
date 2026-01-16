/**
 * Example usage of witast-decoder WASM component from JavaScript/TypeScript
 */

import { parser, lifter, formatter, types } from "./dist/witast.js";

// Example WIT definition
const witDefinition = `
package example:types@0.1.0;

interface types {
  record person {
    name: string,
    age: u32,
    active: bool,
  }

  record point {
    x: f64,
    y: f64,
  }

  variant status {
    pending,
    complete(u32),
    failed(string),
  }

  enum color {
    red,
    green,
    blue,
  }

  flags permissions {
    read,
    write,
    execute,
  }
}

world example {
  export types;
}
`;

console.log("=== wit-ast-js Example ===\n");

// Parse the WIT definition
console.log("1. Parsing WIT definition...");
const ast = parser.parseWit(witDefinition);
console.log("   WIT parsed successfully!\n");

// Get all type definitions
console.log("2. Type definitions found:");
const typeDefs = ast.types();
for (const typeDef of typeDefs) {
  console.log(`   - ${typeDef.name}: ${formatTypeKind(typeDef.kind)}`);
}
console.log();

// Find a specific type
console.log("3. Looking up 'person' type...");
const personIdx = ast.findType("person");
if (personIdx !== undefined) {
  console.log(`   Found 'person' at index ${personIdx}`);
} else {
  console.log("   'person' type not found");
}
console.log();

// Demonstrate WAVE text parsing
console.log("4. Parsing WAVE text to value-tree...");
const waveText = `{name: "Alice", age: 30, active: true}`;
console.log(`   Input: ${waveText}`);

try {
  const valueTree = formatter.waveToValueTree(ast, "person", waveText);
  console.log(`   Parsed successfully! Tree has ${valueTree.nodes.length} nodes`);
  console.log(`   Root node type: ${formatNodeType(valueTree.nodes[0])}`);
} catch (e) {
  console.log(`   Error: ${e.message}`);
}
console.log();

// Demonstrate value-tree to WAVE conversion
console.log("5. Converting value-tree back to WAVE text...");
try {
  const valueTree = formatter.waveToValueTree(ast, "person", waveText);
  const waveOutput = formatter.valueTreeToWave(ast, "person", valueTree);
  console.log(`   Output: ${waveOutput}`);
} catch (e) {
  console.log(`   Error: ${e.message}`);
}
console.log();

// Test with different types
console.log("6. Testing other types...");

// Point
try {
  const pointWave = `{x: 3.14, y: 2.71}`;
  console.log(`   Point: ${pointWave}`);
  const pointTree = formatter.waveToValueTree(ast, "point", pointWave);
  const pointOut = formatter.valueTreeToWave(ast, "point", pointTree);
  console.log(`   Roundtrip: ${pointOut}`);
} catch (e) {
  console.log(`   Point error: ${e.message}`);
}

// Status variant
try {
  const statusWave = `complete(42)`;
  console.log(`   Status: ${statusWave}`);
  const statusTree = formatter.waveToValueTree(ast, "status", statusWave);
  const statusOut = formatter.valueTreeToWave(ast, "status", statusTree);
  console.log(`   Roundtrip: ${statusOut}`);
} catch (e) {
  console.log(`   Status error: ${e.message}`);
}

// Color enum
try {
  const colorWave = `green`;
  console.log(`   Color: ${colorWave}`);
  const colorTree = formatter.waveToValueTree(ast, "color", colorWave);
  const colorOut = formatter.valueTreeToWave(ast, "color", colorTree);
  console.log(`   Roundtrip: ${colorOut}`);
} catch (e) {
  console.log(`   Color error: ${e.message}`);
}

// Permissions flags
try {
  const flagsWave = `{read, write}`;
  console.log(`   Permissions: ${flagsWave}`);
  const flagsTree = formatter.waveToValueTree(ast, "permissions", flagsWave);
  const flagsOut = formatter.valueTreeToWave(ast, "permissions", flagsTree);
  console.log(`   Roundtrip: ${flagsOut}`);
} catch (e) {
  console.log(`   Permissions error: ${e.message}`);
}

console.log("\n=== Example complete ===");

// Helper functions

function formatTypeKind(kind) {
  switch (kind.tag) {
    case "type-record": {
      const fields = kind.val.map((f) => f.name).join(", ");
      return `record { ${fields} }`;
    }
    case "type-tuple":
      return `tuple<${kind.val.length} elements>`;
    case "type-enum":
      return `enum { ${kind.val.join(", ")} }`;
    case "type-variant": {
      const cases = kind.val.map((c) => c.name).join(", ");
      return `variant { ${cases} }`;
    }
    case "type-flags":
      return `flags { ${kind.val.join(", ")} }`;
    case "type-option":
      return `option<${formatTypeRef(kind.val)}>`;
    case "type-result":
      return `result<${kind.val[0] ? formatTypeRef(kind.val[0]) : "_"}, ${kind.val[1] ? formatTypeRef(kind.val[1]) : "_"}>`;
    case "type-list":
      return `list<${formatTypeRef(kind.val)}>`;
    case "type-alias":
      return `alias -> ${formatTypeRef(kind.val)}`;
    default:
      return JSON.stringify(kind);
  }
}

function formatTypeRef(ref) {
  if (ref.tag === "primitive") {
    return ref.val.replace("prim-", "");
  }
  return `type[${ref.val}]`;
}

function formatNodeType(node) {
  switch (node.tag) {
    case "primitive":
      return formatPrimitiveValue(node.val);
    case "record-val":
      return `record(${node.val.length} fields)`;
    case "tuple-val":
      return `tuple(${node.val.length} elements)`;
    case "list-val":
      return `list(${node.val.length} items)`;
    case "enum-val":
      return `enum(${node.val})`;
    case "variant-val":
      return `variant(${node.val.name})`;
    case "option-val":
      return node.val !== undefined ? `some` : `none`;
    case "result-val":
      return node.val.tag === "ok" ? `ok` : `err`;
    case "flags-val":
      return `flags { ${node.val.join(", ")} }`;
    default:
      return JSON.stringify(node);
  }
}

function formatPrimitiveValue(prim) {
  switch (prim.tag) {
    case "bool-val":
      return `bool(${prim.val})`;
    case "u8-val":
    case "u16-val":
    case "u32-val":
    case "u64-val":
    case "s8-val":
    case "s16-val":
    case "s32-val":
    case "s64-val":
      return `${prim.tag.replace("-val", "")}(${prim.val})`;
    case "f32-val":
    case "f64-val":
      return `${prim.tag.replace("-val", "")}(${prim.val})`;
    case "char-val":
      return `char('${prim.val}')`;
    case "string-val":
      return `string("${prim.val}")`;
    default:
      return JSON.stringify(prim);
  }
}
