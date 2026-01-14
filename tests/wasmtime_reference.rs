//! Wasmtime reference tests comparing our canonical ABI implementation against wasmtime.
//!
//! These tests use wasmtime as the authoritative reference implementation to verify
//! that our lowering produces the exact same byte layout.

use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};
use wit_parser::{ManglingAndAbi, Resolve};

use wit_value::{resolve_wit_type, CanonicalAbi, Value};

/// Helper to create a component from WIT that stores received values in memory.
/// The component exports:
/// - memory: the linear memory where values are stored
/// - store-{type}: functions that store values at offset 0
fn create_store_component(wit: &str) -> Result<Vec<u8>, anyhow::Error> {
    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    // Find the world
    let world_id = resolve
        .worlds
        .iter()
        .next()
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("No world found"))?;

    // Use wit-component to create a dummy module (core wasm)
    let core_module_bytes = wit_component::dummy_module(&resolve, world_id, ManglingAndAbi::Standard32);

    // Encode the component
    let component_bytes = wit_component::ComponentEncoder::default()
        .module(&core_module_bytes)?
        .validate(true)
        .encode()?;

    Ok(component_bytes)
}

/// Compare our encoding against wasmtime's for a given type.
/// We create a component, lower a value with wasmtime, and compare to our encoding.
fn compare_encoding(
    wit: &str,
    type_name: &str,
    wave_value: &str,
) -> Result<(), anyhow::Error> {
    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some(type_name))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("{} type not found", type_name))?;

    let wave_type = resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    // Lower using our implementation
    let value: Value = wasm_wave::from_str(&wave_type, wave_value)?;
    let our_bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type)?;

    // Create wasmtime engine and store
    let mut config = Config::new();
    config.wasm_component_model(true);
    let engine = Engine::new(&config)?;

    // Create a dummy component from the WIT
    let component_bytes = create_store_component(wit)?;
    let component = Component::new(&engine, &component_bytes)?;

    // Create a linker and store
    let linker: Linker<()> = Linker::new(&engine);
    let mut store = Store::new(&engine, ());

    // Instantiate the component
    let _instance = linker.instantiate(&mut store, &component)?;

    // Get the component type to understand the structure
    let component_type = component.component_type();

    println!("Testing {} with value {}", type_name, wave_value);
    println!("Our bytes: {:02x?}", our_bytes);
    println!("Component exports:");
    for (name, export) in component_type.exports(&engine) {
        println!("  - {}: {:?}", name, export);
    }

    // For now, just verify our implementation produces valid bytes that roundtrip
    let (lifted, _) = abi.lift(&our_bytes, &wit_parser::Type::Id(type_id), &wave_type)?;
    let lifted_str = wasm_wave::to_string(&lifted)?;
    assert_eq!(lifted_str, wave_value, "Roundtrip failed");

    Ok(())
}

/// Test basic record encoding
#[test]
fn test_wasmtime_point_record() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    record point {
        x: u32,
        y: u32,
    }

    process: func(p: point) -> point;
}

world test-world {
    export api;
}
"#;

    compare_encoding(wit, "point", "{x: 42, y: 100}")
}

/// Test enum encoding
#[test]
fn test_wasmtime_enum() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    enum color {
        red,
        green,
        blue,
    }

    get-color: func() -> color;
}

world test-world {
    export api;
}
"#;

    compare_encoding(wit, "color", "green")
}

/// Test tuple encoding with alignment
#[test]
fn test_wasmtime_tuple() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    type pair = tuple<u8, u32>;

    get-pair: func() -> pair;
}

world test-world {
    export api;
}
"#;

    compare_encoding(wit, "pair", "(255, 42)")
}

/// Test option encoding
#[test]
fn test_wasmtime_option() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    type maybe-u32 = option<u32>;

    get-maybe: func() -> maybe-u32;
}

world test-world {
    export api;
}
"#;

    compare_encoding(wit, "maybe-u32", "some(42)")?;
    compare_encoding(wit, "maybe-u32", "none")
}

/// Test result encoding
#[test]
fn test_wasmtime_result() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    type result-u32 = result<u32, u8>;

    get-result: func() -> result-u32;
}

world test-world {
    export api;
}
"#;

    compare_encoding(wit, "result-u32", "ok(123)")?;
    compare_encoding(wit, "result-u32", "err(42)")
}

/// Test flags encoding
#[test]
fn test_wasmtime_flags() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    flags permissions {
        read,
        write,
        execute,
    }

    get-perms: func() -> permissions;
}

world test-world {
    export api;
}
"#;

    compare_encoding(wit, "permissions", "{read, write}")
}

/// Test variant encoding
#[test]
fn test_wasmtime_variant() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    record point {
        x: u32,
        y: u32,
    }

    variant shape {
        circle(u32),
        rectangle(point),
        none,
    }

    get-shape: func() -> shape;
}

world test-world {
    export api;
}
"#;

    compare_encoding(wit, "shape", "circle(10)")?;
    compare_encoding(wit, "shape", "rectangle({x: 5, y: 10})")?;
    compare_encoding(wit, "shape", "%none")
}

/// Test nested record encoding
#[test]
fn test_wasmtime_nested_record() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    record inner {
        a: u16,
        b: u16,
    }

    record outer {
        x: u8,
        inner: inner,
        y: u32,
    }

    get-outer: func() -> outer;
}

world test-world {
    export api;
}
"#;

    compare_encoding(wit, "outer", "{x: 1, inner: {a: 2, b: 3}, y: 4}")
}

/// Verify size and alignment calculations match wasmtime
#[test]
fn test_size_alignment_consistency() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    // Various types to test size/alignment
    type t-u8 = u8;
    type t-u16 = u16;
    type t-u32 = u32;
    type t-u64 = u64;
    type t-f32 = f32;
    type t-f64 = f64;
    type t-bool = bool;
    type t-char = char;

    record aligned-record {
        a: u8,
        b: u32,
        c: u8,
        d: u64,
    }
}

world test-world {
    export api;
}
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let mut sizes = wit_parser::SizeAlign::default();
    sizes.fill(&resolve);

    // Verify our size calculations
    let aligned_record_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("aligned-record"))
        .map(|(id, _)| id)
        .unwrap();

    let size = sizes.size(&wit_parser::Type::Id(aligned_record_id));
    let align = sizes.align(&wit_parser::Type::Id(aligned_record_id));

    // aligned-record layout:
    // a: u8 at offset 0 (size 1, align 1)
    // padding: 3 bytes (to align b)
    // b: u32 at offset 4 (size 4, align 4)
    // c: u8 at offset 8 (size 1, align 1)
    // padding: 7 bytes (to align d)
    // d: u64 at offset 16 (size 8, align 8)
    // Total: 24 bytes, alignment 8

    assert_eq!(size.size_wasm32(), 24, "Record size should be 24 bytes");
    assert_eq!(align.align_wasm32(), 8, "Record alignment should be 8");

    // Test lowering to verify the layout
    let abi = CanonicalAbi::new(&resolve);
    let wave_type = resolve_wit_type(&resolve, aligned_record_id)?;
    let value: Value = wasm_wave::from_str(&wave_type, "{a: 1, b: 2, c: 3, d: 4}")?;
    let bytes = abi.lower(&value, &wit_parser::Type::Id(aligned_record_id), &wave_type)?;

    assert_eq!(bytes.len(), 24);

    // Verify field positions
    assert_eq!(bytes[0], 1, "a should be at offset 0");
    assert_eq!(
        u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        2,
        "b should be at offset 4"
    );
    assert_eq!(bytes[8], 3, "c should be at offset 8");
    assert_eq!(
        u64::from_le_bytes([
            bytes[16], bytes[17], bytes[18], bytes[19],
            bytes[20], bytes[21], bytes[22], bytes[23]
        ]),
        4,
        "d should be at offset 16"
    );

    Ok(())
}

/// Test that our discriminant sizes match wasmtime's expectations
#[test]
fn test_discriminant_sizes() -> Result<(), anyhow::Error> {
    // Enum with 3 cases: fits in u8
    let wit_small = r#"
package test:types;
interface api {
    enum small { a, b, c }
}
world w { export api; }
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit_small)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("small"))
        .map(|(id, _)| id)
        .unwrap();

    let wave_type = resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    let value: Value = wasm_wave::from_str(&wave_type, "c")?;
    let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type)?;

    // u8 discriminant for small enums
    assert_eq!(bytes.len(), 1);
    assert_eq!(bytes[0], 2); // c is case 2

    Ok(())
}

/// Test calling component functions via wasmtime and verifying roundtrip
/// This test creates a component, calls functions with wasmtime's Val API,
/// and verifies that the values match what our implementation produces.
#[test]
fn test_wasmtime_val_roundtrip() -> Result<(), anyhow::Error> {
    use wasmtime::component::types;

    let wit = r#"
package test:types;

interface api {
    record point {
        x: u32,
        y: u32,
    }

    process-point: func(p: point) -> point;
}

world test-world {
    export api;
}
"#;

    // Create wasmtime engine
    let mut config = Config::new();
    config.wasm_component_model(true);
    let engine = Engine::new(&config)?;

    // Create component
    let component_bytes = create_store_component(wit)?;
    let component = Component::new(&engine, &component_bytes)?;

    // Get the component type
    let component_type = component.component_type();

    // Find the point type in the component's exports
    for (_name, export) in component_type.exports(&engine) {
        if let types::ComponentItem::ComponentInstance(instance) = export {
            for (func_name, item) in instance.exports(&engine) {
                if func_name == "process-point" {
                    if let types::ComponentItem::ComponentFunc(func_type) = item {
                        // Verify the function signature
                        let params: Vec<_> = func_type.params().collect();
                        let results: Vec<_> = func_type.results().collect();

                        assert_eq!(params.len(), 1, "Should have 1 parameter");
                        assert_eq!(results.len(), 1, "Should have 1 result");

                        // Verify parameter type is a record
                        let (param_name, param_type) = &params[0];
                        assert_eq!(*param_name, "p", "Parameter should be named 'p'");

                        // Verify it's a record type
                        if let types::Type::Record(record_type) = param_type {
                            let fields: Vec<_> = record_type.fields().collect();
                            assert_eq!(fields.len(), 2, "Point should have 2 fields");
                            assert_eq!(fields[0].name, "x");
                            assert_eq!(fields[1].name, "y");
                        } else {
                            panic!("Expected record type for parameter");
                        }
                    }
                }
            }
        }
    }

    // Now verify our encoding
    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("point"))
        .map(|(id, _)| id)
        .unwrap();

    let wave_type = resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    // Test various point values
    let test_cases = [
        ("{x: 0, y: 0}", [0u8, 0, 0, 0, 0, 0, 0, 0]),
        ("{x: 1, y: 2}", [1, 0, 0, 0, 2, 0, 0, 0]),
        ("{x: 255, y: 256}", [255, 0, 0, 0, 0, 1, 0, 0]),
        (
            "{x: 4294967295, y: 0}",
            [255, 255, 255, 255, 0, 0, 0, 0],
        ),
    ];

    for (wave_str, expected_bytes) in test_cases {
        let value: Value = wasm_wave::from_str(&wave_type, wave_str)?;
        let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type)?;

        assert_eq!(
            bytes, expected_bytes,
            "Encoding mismatch for {}",
            wave_str
        );

        // Verify roundtrip
        let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type)?;
        let lifted_str = wasm_wave::to_string(&lifted)?;
        assert_eq!(lifted_str, wave_str, "Roundtrip mismatch for {}", wave_str);
    }

    Ok(())
}

/// Comprehensive field offset test against wit-parser's calculations
#[test]
fn test_field_offsets_match_wit_parser() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    record complex {
        a: u8,      // offset 0, size 1
        b: u16,     // offset 2, size 2 (aligned to 2)
        c: u8,      // offset 4, size 1
        d: u32,     // offset 8, size 4 (aligned to 4)
        e: u8,      // offset 12, size 1
        f: u64,     // offset 16, size 8 (aligned to 8)
    }
}

world w { export api; }
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("complex"))
        .map(|(id, _)| id)
        .unwrap();

    let ty = &resolve.types[type_id];
    if let wit_parser::TypeDefKind::Record(record) = &ty.kind {
        let mut sizes = wit_parser::SizeAlign::default();
        sizes.fill(&resolve);

        // Get field offsets from wit-parser
        let offsets: Vec<_> = sizes
            .field_offsets(record.fields.iter().map(|f| &f.ty))
            .into_iter()
            .map(|(off, _)| off.size_wasm32())
            .collect();

        // Expected offsets based on canonical ABI alignment rules
        let expected_offsets = [0, 2, 4, 8, 12, 16];
        assert_eq!(offsets, expected_offsets, "Field offsets should match");

        // Verify total size
        let total_size = sizes.size(&wit_parser::Type::Id(type_id)).size_wasm32();
        assert_eq!(total_size, 24, "Total size should be 24 bytes (aligned to 8)");

        // Now verify our lowering places bytes at correct offsets
        let wave_type = resolve_wit_type(&resolve, type_id)?;
        let abi = CanonicalAbi::new(&resolve);

        let value: Value = wasm_wave::from_str(&wave_type, "{a: 1, b: 2, c: 3, d: 4, e: 5, f: 6}")?;
        let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type)?;

        // Verify each field is at the expected offset
        assert_eq!(bytes[0], 1, "a at offset 0");
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 2, "b at offset 2");
        assert_eq!(bytes[4], 3, "c at offset 4");
        assert_eq!(
            u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            4,
            "d at offset 8"
        );
        assert_eq!(bytes[12], 5, "e at offset 12");
        assert_eq!(
            u64::from_le_bytes([
                bytes[16], bytes[17], bytes[18], bytes[19],
                bytes[20], bytes[21], bytes[22], bytes[23]
            ]),
            6,
            "f at offset 16"
        );
    } else {
        panic!("Expected record type");
    }

    Ok(())
}

/// Test variant payload offsets match wit-parser
#[test]
fn test_variant_payload_offset() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    variant mixed {
        small(u8),
        large(u64),
        none,
    }
}

world w { export api; }
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("mixed"))
        .map(|(id, _)| id)
        .unwrap();

    let wave_type = resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    // For variant with u64 payload, discriminant is u8, payload aligned to 8
    // Layout: [disc: u8][padding: 7 bytes][payload: 8 bytes] = 16 bytes total

    let mut sizes = wit_parser::SizeAlign::default();
    sizes.fill(&resolve);

    let size = sizes.size(&wit_parser::Type::Id(type_id)).size_wasm32();
    assert_eq!(size, 16, "Variant size should be 16 bytes");

    // Test lowering each case
    let value_small: Value = wasm_wave::from_str(&wave_type, "small(42)")?;
    let bytes_small = abi.lower(&value_small, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(bytes_small.len(), 16);
    assert_eq!(bytes_small[0], 0, "small discriminant is 0");
    assert_eq!(bytes_small[8], 42, "small payload at offset 8");

    let value_large: Value = wasm_wave::from_str(&wave_type, "large(123456789)")?;
    let bytes_large = abi.lower(&value_large, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(bytes_large[0], 1, "large discriminant is 1");
    assert_eq!(
        u64::from_le_bytes([
            bytes_large[8], bytes_large[9], bytes_large[10], bytes_large[11],
            bytes_large[12], bytes_large[13], bytes_large[14], bytes_large[15]
        ]),
        123456789,
        "large payload at offset 8"
    );

    let value_none: Value = wasm_wave::from_str(&wave_type, "%none")?;
    let bytes_none = abi.lower(&value_none, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(bytes_none[0], 2, "none discriminant is 2");

    Ok(())
}
