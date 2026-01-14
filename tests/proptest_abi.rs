//! Property-based tests for canonical ABI roundtrip correctness.
//!
//! These tests verify that lower(lift(x)) == x for random inputs.

use proptest::prelude::*;
use wit_parser::Resolve;
use wit_value::{CanonicalAbi, LinearMemory, resolve_wit_type, Value};

/// Helper to create a resolve with a specific type
fn create_resolve_with_type(wit: &str) -> Result<(Resolve, wit_parser::TypeId), anyhow::Error> {
    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    // Find the first named type
    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.is_some())
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("No named type found"))?;

    Ok((resolve, type_id))
}

// Test roundtrip for u8 values
proptest! {
    #[test]
    fn roundtrip_u8(val in 0u8..=255u8) {
        let wit = r#"
package test:types;
interface api { type my-u8 = u8; }
world w { use api.{my-u8}; export f: func() -> my-u8; }
"#;
        let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
        let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
        let abi = CanonicalAbi::new(&resolve);

        let value: Value = wasm_wave::from_str(&wave_type, &val.to_string()).unwrap();
        let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let lifted_str = wasm_wave::to_string(&lifted).unwrap();

        prop_assert_eq!(lifted_str, val.to_string());
    }

    #[test]
    fn roundtrip_u16(val in 0u16..=65535u16) {
        let wit = r#"
package test:types;
interface api { type my-u16 = u16; }
world w { use api.{my-u16}; export f: func() -> my-u16; }
"#;
        let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
        let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
        let abi = CanonicalAbi::new(&resolve);

        let value: Value = wasm_wave::from_str(&wave_type, &val.to_string()).unwrap();
        let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let lifted_str = wasm_wave::to_string(&lifted).unwrap();

        prop_assert_eq!(lifted_str, val.to_string());
    }

    #[test]
    fn roundtrip_u32(val in any::<u32>()) {
        let wit = r#"
package test:types;
interface api { type my-u32 = u32; }
world w { use api.{my-u32}; export f: func() -> my-u32; }
"#;
        let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
        let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
        let abi = CanonicalAbi::new(&resolve);

        let value: Value = wasm_wave::from_str(&wave_type, &val.to_string()).unwrap();
        let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let lifted_str = wasm_wave::to_string(&lifted).unwrap();

        prop_assert_eq!(lifted_str, val.to_string());
    }

    #[test]
    fn roundtrip_u64(val in any::<u64>()) {
        let wit = r#"
package test:types;
interface api { type my-u64 = u64; }
world w { use api.{my-u64}; export f: func() -> my-u64; }
"#;
        let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
        let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
        let abi = CanonicalAbi::new(&resolve);

        let value: Value = wasm_wave::from_str(&wave_type, &val.to_string()).unwrap();
        let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let lifted_str = wasm_wave::to_string(&lifted).unwrap();

        prop_assert_eq!(lifted_str, val.to_string());
    }

    #[test]
    fn roundtrip_s8(val in -128i8..=127i8) {
        let wit = r#"
package test:types;
interface api { type my-s8 = s8; }
world w { use api.{my-s8}; export f: func() -> my-s8; }
"#;
        let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
        let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
        let abi = CanonicalAbi::new(&resolve);

        let value: Value = wasm_wave::from_str(&wave_type, &val.to_string()).unwrap();
        let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let lifted_str = wasm_wave::to_string(&lifted).unwrap();

        prop_assert_eq!(lifted_str, val.to_string());
    }

    #[test]
    fn roundtrip_s32(val in any::<i32>()) {
        let wit = r#"
package test:types;
interface api { type my-s32 = s32; }
world w { use api.{my-s32}; export f: func() -> my-s32; }
"#;
        let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
        let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
        let abi = CanonicalAbi::new(&resolve);

        let value: Value = wasm_wave::from_str(&wave_type, &val.to_string()).unwrap();
        let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let lifted_str = wasm_wave::to_string(&lifted).unwrap();

        prop_assert_eq!(lifted_str, val.to_string());
    }

    #[test]
    fn roundtrip_s64(val in any::<i64>()) {
        let wit = r#"
package test:types;
interface api { type my-s64 = s64; }
world w { use api.{my-s64}; export f: func() -> my-s64; }
"#;
        let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
        let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
        let abi = CanonicalAbi::new(&resolve);

        let value: Value = wasm_wave::from_str(&wave_type, &val.to_string()).unwrap();
        let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let lifted_str = wasm_wave::to_string(&lifted).unwrap();

        prop_assert_eq!(lifted_str, val.to_string());
    }

    #[test]
    fn roundtrip_bool(val in any::<bool>()) {
        let wit = r#"
package test:types;
interface api { type my-bool = bool; }
world w { use api.{my-bool}; export f: func() -> my-bool; }
"#;
        let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
        let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
        let abi = CanonicalAbi::new(&resolve);

        let wave_str = if val { "true" } else { "false" };
        let value: Value = wasm_wave::from_str(&wave_type, wave_str).unwrap();
        let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let lifted_str = wasm_wave::to_string(&lifted).unwrap();

        prop_assert_eq!(lifted_str, wave_str);
    }

    #[test]
    fn roundtrip_point(x in any::<u32>(), y in any::<u32>()) {
        let wit = r#"
package test:types;
interface api { record point { x: u32, y: u32, } }
world w { use api.{point}; export f: func() -> point; }
"#;
        let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
        let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
        let abi = CanonicalAbi::new(&resolve);

        let wave_str = format!("{{x: {}, y: {}}}", x, y);
        let value: Value = wasm_wave::from_str(&wave_type, &wave_str).unwrap();
        let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let lifted_str = wasm_wave::to_string(&lifted).unwrap();

        prop_assert_eq!(lifted_str, wave_str);
    }

    #[test]
    fn roundtrip_option_u32(val in proptest::option::of(any::<u32>())) {
        let wit = r#"
package test:types;
interface api { type maybe-u32 = option<u32>; }
world w { use api.{maybe-u32}; export f: func() -> maybe-u32; }
"#;
        let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
        let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
        let abi = CanonicalAbi::new(&resolve);

        let wave_str = match val {
            Some(v) => format!("some({})", v),
            None => "none".to_string(),
        };
        let value: Value = wasm_wave::from_str(&wave_type, &wave_str).unwrap();
        let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let lifted_str = wasm_wave::to_string(&lifted).unwrap();

        prop_assert_eq!(lifted_str, wave_str);
    }

    #[test]
    fn roundtrip_result_u32_u8(ok in any::<bool>(), ok_val in any::<u32>(), err_val in any::<u8>()) {
        let wit = r#"
package test:types;
interface api { type result-u32 = result<u32, u8>; }
world w { use api.{result-u32}; export f: func() -> result-u32; }
"#;
        let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
        let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
        let abi = CanonicalAbi::new(&resolve);

        let wave_str = if ok {
            format!("ok({})", ok_val)
        } else {
            format!("err({})", err_val)
        };
        let value: Value = wasm_wave::from_str(&wave_type, &wave_str).unwrap();
        let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let lifted_str = wasm_wave::to_string(&lifted).unwrap();

        prop_assert_eq!(lifted_str, wave_str);
    }

    #[test]
    fn roundtrip_tuple_u8_u32(a in any::<u8>(), b in any::<u32>()) {
        let wit = r#"
package test:types;
interface api { type pair = tuple<u8, u32>; }
world w { use api.{pair}; export f: func() -> pair; }
"#;
        let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
        let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
        let abi = CanonicalAbi::new(&resolve);

        let wave_str = format!("({}, {})", a, b);
        let value: Value = wasm_wave::from_str(&wave_type, &wave_str).unwrap();
        let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type).unwrap();
        let lifted_str = wasm_wave::to_string(&lifted).unwrap();

        prop_assert_eq!(lifted_str, wave_str);
    }
}

/// Test specific edge cases
#[test]
fn test_u32_edge_cases() {
    let wit = r#"
package test:types;
interface api { type my-u32 = u32; }
world w { use api.{my-u32}; export f: func() -> my-u32; }
"#;
    let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
    let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
    let abi = CanonicalAbi::new(&resolve);

    // Test edge values
    let edge_cases = [0u32, 1, u32::MAX, u32::MAX - 1, 0x80000000];

    for val in edge_cases {
        let value: Value = wasm_wave::from_str(&wave_type, &val.to_string()).unwrap();
        let bytes = abi
            .lower(&value, &wit_parser::Type::Id(type_id), &wave_type)
            .unwrap();
        let (lifted, _) = abi
            .lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type)
            .unwrap();
        let lifted_str = wasm_wave::to_string(&lifted).unwrap();
        assert_eq!(lifted_str, val.to_string(), "Failed for value {}", val);
    }
}

/// Test f32 special values
#[test]
fn test_f32_special_values() {
    let wit = r#"
package test:types;
interface api { type my-f32 = f32; }
world w { use api.{my-f32}; export f: func() -> my-f32; }
"#;
    let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
    let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
    let abi = CanonicalAbi::new(&resolve);

    // Test regular values (not NaN since NaN != NaN)
    let test_cases = [0.0f32, 1.0, -1.0, f32::INFINITY, f32::NEG_INFINITY];

    for val in test_cases {
        let wave_str = format!("{}", val);
        let value: Value = wasm_wave::from_str(&wave_type, &wave_str).unwrap();
        let bytes = abi
            .lower(&value, &wit_parser::Type::Id(type_id), &wave_type)
            .unwrap();
        let (lifted, _) = abi
            .lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type)
            .unwrap();

        // Compare the bytes directly for floating point
        let value2: Value = wasm_wave::from_str(&wave_type, &wasm_wave::to_string(&lifted).unwrap()).unwrap();
        let bytes2 = abi
            .lower(&value2, &wit_parser::Type::Id(type_id), &wave_type)
            .unwrap();
        assert_eq!(bytes, bytes2, "Roundtrip failed for f32 value {}", val);
    }
}

/// Test char values including unicode
#[test]
fn test_char_values() {
    let wit = r#"
package test:types;
interface api { type my-char = char; }
world w { use api.{my-char}; export f: func() -> my-char; }
"#;
    let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
    let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
    let abi = CanonicalAbi::new(&resolve);

    // Test various characters
    let test_chars = ['a', 'Z', '0', ' ', '\n', '\t', '中', '🎉', '\u{10FFFF}'];

    for c in test_chars {
        let wave_str = format!("'{}'", c.escape_default());
        let value: Value = wasm_wave::from_str(&wave_type, &wave_str).unwrap();
        let bytes = abi
            .lower(&value, &wit_parser::Type::Id(type_id), &wave_type)
            .unwrap();
        let _ = abi
            .lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type)
            .unwrap();

        // Verify the character was preserved
        assert_eq!(bytes.len(), 4, "char should be 4 bytes");
        let code = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert_eq!(code, c as u32, "Character code mismatch for '{}'", c);
    }
}

proptest! {
    // Test roundtrip for string values
    #[test]
    fn roundtrip_string(s in ".*") {
        let wit = r#"
package test:types;

interface api {
    record message {
        text: string,
    }
}

world test-world {
    use api.{message};
    export get: func() -> message;
}
"#;

        let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
        let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
        let abi = CanonicalAbi::new(&resolve);

        // Escape special characters in the string for WAVE format
        let escaped = s
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");
        let wave_value = format!(r#"{{text: "{}"}}"#, escaped);

        // Parse, lower, and lift
        if let Ok(value) = wasm_wave::from_str::<Value>(&wave_type, &wave_value) {
            let mut memory = LinearMemory::new();
            let bytes = abi
                .lower_with_memory(&value, &wit_parser::Type::Id(type_id), &wave_type, &mut memory)
                .unwrap();
            let (lifted, _) = abi
                .lift_with_memory(&bytes, &wit_parser::Type::Id(type_id), &wave_type, &memory)
                .unwrap();

            // Compare the original and lifted values
            let original_str = wasm_wave::to_string(&value).unwrap();
            let lifted_str = wasm_wave::to_string(&lifted).unwrap();
            prop_assert_eq!(original_str, lifted_str);
        }
        // If parsing fails (due to invalid escape sequences), skip this test case
    }

    // Test roundtrip for list of u32 values
    #[test]
    fn roundtrip_list_u32(values in prop::collection::vec(any::<u32>(), 0..20)) {
        let wit = r#"
package test:types;

interface api {
    record numbers {
        values: list<u32>,
    }
}

world test-world {
    use api.{numbers};
    export get: func() -> numbers;
}
"#;

        let (resolve, type_id) = create_resolve_with_type(wit).unwrap();
        let wave_type = resolve_wit_type(&resolve, type_id).unwrap();
        let abi = CanonicalAbi::new(&resolve);

        // Build WAVE string
        let values_str = values
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let wave_value = format!("{{values: [{}]}}", values_str);

        let value: Value = wasm_wave::from_str(&wave_type, &wave_value).unwrap();
        let mut memory = LinearMemory::new();
        let bytes = abi
            .lower_with_memory(&value, &wit_parser::Type::Id(type_id), &wave_type, &mut memory)
            .unwrap();
        let (lifted, _) = abi
            .lift_with_memory(&bytes, &wit_parser::Type::Id(type_id), &wave_type, &memory)
            .unwrap();

        let original_str = wasm_wave::to_string(&value).unwrap();
        let lifted_str = wasm_wave::to_string(&lifted).unwrap();
        prop_assert_eq!(original_str, lifted_str);
    }
}
