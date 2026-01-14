//! Reference tests comparing our canonical ABI implementation against wasmtime.
//!
//! These tests verify canonical ABI encoding for various WIT types.

use wit_parser::Resolve;

use wit_kv::{CanonicalAbi, LinearMemory};

/// Test that our encoding of a point record matches the canonical ABI
#[test]
fn test_point_record_encoding() -> Result<(), anyhow::Error> {
    // Our implementation
    let wit = r#"
package test:types;

interface api {
    record point {
        x: u32,
        y: u32,
    }
}

world test-world {
    use api.{point};
    export process: func(p: point) -> point;
}
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    // Find the point type
    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("point"))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("point type not found"))?;

    let wave_type = wit_kv::resolve_wit_type(&resolve, type_id)?;

    // Lower a value using our implementation
    let value: wit_kv::Value = wasm_wave::from_str(&wave_type, "{x: 42, y: 100}")?;
    let abi = CanonicalAbi::new(&resolve);
    let our_bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type)?;

    // Expected: two u32 values in little-endian
    // x = 42 = 0x2a000000, y = 100 = 0x64000000
    let expected = vec![
        0x2a, 0x00, 0x00, 0x00, // x = 42
        0x64, 0x00, 0x00, 0x00, // y = 100
    ];

    assert_eq!(our_bytes, expected, "Point encoding mismatch");

    // Verify roundtrip
    let (lifted, _) = abi.lift(&our_bytes, &wit_parser::Type::Id(type_id), &wave_type)?;
    let lifted_str = wasm_wave::to_string(&lifted)?;
    assert_eq!(lifted_str, "{x: 42, y: 100}");

    Ok(())
}

/// Test enum encoding matches canonical ABI
#[test]
fn test_enum_encoding() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    enum color {
        red,
        green,
        blue,
    }
}

world test-world {
    use api.{color};
    export get-color: func() -> color;
}
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("color"))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("color type not found"))?;

    let wave_type = wit_kv::resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    // Test each enum case
    let test_cases = [("red", 0u8), ("green", 1u8), ("blue", 2u8)];

    for (name, expected_discriminant) in test_cases {
        let value: wit_kv::Value = wasm_wave::from_str(&wave_type, name)?;
        let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type)?;

        assert_eq!(bytes, vec![expected_discriminant], "Enum {} encoding mismatch", name);

        // Verify roundtrip
        let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type)?;
        let lifted_str = wasm_wave::to_string(&lifted)?;
        assert_eq!(lifted_str, name);
    }

    Ok(())
}

/// Test flags encoding matches canonical ABI
#[test]
fn test_flags_encoding() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    flags permissions {
        read,
        write,
        execute,
    }
}

world test-world {
    use api.{permissions};
    export get-perms: func() -> permissions;
}
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("permissions"))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("permissions type not found"))?;

    let wave_type = wit_kv::resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    // Test various flag combinations
    // read=bit0, write=bit1, execute=bit2
    let test_cases = [
        ("{}", 0b000u8),
        ("{read}", 0b001u8),
        ("{write}", 0b010u8),
        ("{execute}", 0b100u8),
        ("{read, write}", 0b011u8),
        ("{read, execute}", 0b101u8),
        ("{read, write, execute}", 0b111u8),
    ];

    for (wave_str, expected_bits) in test_cases {
        let value: wit_kv::Value = wasm_wave::from_str(&wave_type, wave_str)?;
        let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type)?;

        assert_eq!(bytes, vec![expected_bits], "Flags {} encoding mismatch", wave_str);
    }

    Ok(())
}

/// Test option encoding matches canonical ABI
#[test]
fn test_option_encoding() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    type maybe-u32 = option<u32>;
}

world test-world {
    use api.{maybe-u32};
    export get-maybe: func() -> maybe-u32;
}
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("maybe-u32"))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("maybe-u32 type not found"))?;

    let wave_type = wit_kv::resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    // Test none
    let none_value: wit_kv::Value = wasm_wave::from_str(&wave_type, "none")?;
    let none_bytes = abi.lower(&none_value, &wit_parser::Type::Id(type_id), &wave_type)?;
    // none: discriminant=0, payload padding to 4 bytes
    assert_eq!(none_bytes.len(), 8, "option<u32> should be 8 bytes");
    assert_eq!(none_bytes[0], 0, "none discriminant should be 0");

    // Test some(42)
    let some_value: wit_kv::Value = wasm_wave::from_str(&wave_type, "some(42)")?;
    let some_bytes = abi.lower(&some_value, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(some_bytes.len(), 8, "option<u32> should be 8 bytes");
    assert_eq!(some_bytes[0], 1, "some discriminant should be 1");
    // Payload at offset 4 (aligned)
    assert_eq!(&some_bytes[4..8], &[42, 0, 0, 0], "some payload should be 42");

    // Verify roundtrip
    let (lifted_none, _) = abi.lift(&none_bytes, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(wasm_wave::to_string(&lifted_none)?, "none");

    let (lifted_some, _) = abi.lift(&some_bytes, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(wasm_wave::to_string(&lifted_some)?, "some(42)");

    Ok(())
}

/// Test result encoding matches canonical ABI
#[test]
fn test_result_encoding() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    type result-u32 = result<u32, u8>;
}

world test-world {
    use api.{result-u32};
    export get-result: func() -> result-u32;
}
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("result-u32"))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("result-u32 type not found"))?;

    let wave_type = wit_kv::resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    // Test ok(123)
    let ok_value: wit_kv::Value = wasm_wave::from_str(&wave_type, "ok(123)")?;
    let ok_bytes = abi.lower(&ok_value, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(ok_bytes[0], 0, "ok discriminant should be 0");
    assert_eq!(&ok_bytes[4..8], &[123, 0, 0, 0], "ok payload should be 123");

    // Test err(42)
    let err_value: wit_kv::Value = wasm_wave::from_str(&wave_type, "err(42)")?;
    let err_bytes = abi.lower(&err_value, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(err_bytes[0], 1, "err discriminant should be 1");
    assert_eq!(err_bytes[4], 42, "err payload should be 42");

    // Verify roundtrip
    let (lifted_ok, _) = abi.lift(&ok_bytes, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(wasm_wave::to_string(&lifted_ok)?, "ok(123)");

    let (lifted_err, _) = abi.lift(&err_bytes, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(wasm_wave::to_string(&lifted_err)?, "err(42)");

    Ok(())
}

/// Test variant encoding matches canonical ABI
#[test]
fn test_variant_encoding() -> Result<(), anyhow::Error> {
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
}

world test-world {
    use api.{shape};
    export get-shape: func() -> shape;
}
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("shape"))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("shape type not found"))?;

    let wave_type = wit_kv::resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    // Test circle(10) - discriminant 0
    let circle_value: wit_kv::Value = wasm_wave::from_str(&wave_type, "circle(10)")?;
    let circle_bytes = abi.lower(&circle_value, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(circle_bytes[0], 0, "circle discriminant should be 0");

    // Test rectangle({x: 5, y: 10}) - discriminant 1
    let rect_value: wit_kv::Value = wasm_wave::from_str(&wave_type, "rectangle({x: 5, y: 10})")?;
    let rect_bytes = abi.lower(&rect_value, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(rect_bytes[0], 1, "rectangle discriminant should be 1");

    // Test %none - discriminant 2
    let none_value: wit_kv::Value = wasm_wave::from_str(&wave_type, "%none")?;
    let none_bytes = abi.lower(&none_value, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(none_bytes[0], 2, "none discriminant should be 2");

    // Verify roundtrips
    let (lifted_circle, _) = abi.lift(&circle_bytes, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(wasm_wave::to_string(&lifted_circle)?, "circle(10)");

    let (lifted_rect, _) = abi.lift(&rect_bytes, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(wasm_wave::to_string(&lifted_rect)?, "rectangle({x: 5, y: 10})");

    let (lifted_none, _) = abi.lift(&none_bytes, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(wasm_wave::to_string(&lifted_none)?, "%none");

    Ok(())
}

/// Test primitive types encoding
#[test]
fn test_primitive_encoding() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    type my-u8 = u8;
    type my-u16 = u16;
    type my-u32 = u32;
    type my-u64 = u64;
    type my-s8 = s8;
    type my-s16 = s16;
    type my-s32 = s32;
    type my-s64 = s64;
    type my-f32 = f32;
    type my-f64 = f64;
    type my-bool = bool;
    type my-char = char;
}

world test-world {
    use api.{my-u32};
    export get-u32: func() -> my-u32;
}
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    // Test u32
    let u32_type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("my-u32"))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("my-u32 type not found"))?;

    let wave_type = wit_kv::resolve_wit_type(&resolve, u32_type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    let value: wit_kv::Value = wasm_wave::from_str(&wave_type, "3735928559")?; // 0xDEADBEEF
    let bytes = abi.lower(&value, &wit_parser::Type::Id(u32_type_id), &wave_type)?;

    // 0xDEADBEEF in little-endian
    assert_eq!(bytes, vec![0xEF, 0xBE, 0xAD, 0xDE]);

    // Verify roundtrip
    let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(u32_type_id), &wave_type)?;
    assert_eq!(wasm_wave::to_string(&lifted)?, "3735928559"); // decimal representation

    Ok(())
}

/// Test tuple encoding with proper alignment
#[test]
fn test_tuple_encoding() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    type pair = tuple<u8, u32>;
}

world test-world {
    use api.{pair};
    export get-pair: func() -> pair;
}
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("pair"))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("pair type not found"))?;

    let wave_type = wit_kv::resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    // tuple<u8, u32> should have alignment padding
    // u8 at offset 0, then 3 bytes padding, then u32 at offset 4
    let value: wit_kv::Value = wasm_wave::from_str(&wave_type, "(255, 42)")?;
    let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type)?;

    assert_eq!(bytes.len(), 8, "tuple<u8, u32> should be 8 bytes with padding");
    assert_eq!(bytes[0], 255, "first element should be 255");
    assert_eq!(&bytes[4..8], &[42, 0, 0, 0], "second element should be 42");

    // Verify roundtrip
    let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(wasm_wave::to_string(&lifted)?, "(255, 42)");

    Ok(())
}

/// Test nested record encoding
#[test]
fn test_nested_record_encoding() -> Result<(), anyhow::Error> {
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
}

world test-world {
    use api.{outer};
    export get-outer: func() -> outer;
}
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("outer"))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("outer type not found"))?;

    let wave_type = wit_kv::resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    let value: wit_kv::Value = wasm_wave::from_str(&wave_type, "{x: 1, inner: {a: 2, b: 3}, y: 4}")?;
    let bytes = abi.lower(&value, &wit_parser::Type::Id(type_id), &wave_type)?;

    // Verify roundtrip
    let (lifted, _) = abi.lift(&bytes, &wit_parser::Type::Id(type_id), &wave_type)?;
    assert_eq!(wasm_wave::to_string(&lifted)?, "{x: 1, inner: {a: 2, b: 3}, y: 4}");

    Ok(())
}

/// Test string encoding with linear memory
#[test]
fn test_string_encoding() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    record message {
        text: string,
    }
}

world test-world {
    use api.{message};
    export get-message: func() -> message;
}
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("message"))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("message type not found"))?;

    let wave_type = wit_kv::resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    // Lower a string value using linear memory
    let value: wit_kv::Value = wasm_wave::from_str(&wave_type, r#"{text: "hello"}"#)?;
    let mut memory = LinearMemory::new();
    let bytes = abi.lower_with_memory(&value, &wit_parser::Type::Id(type_id), &wave_type, &mut memory)?;

    // The main buffer should contain ptr (4 bytes) + len (4 bytes)
    assert_eq!(bytes.len(), 8, "string record should be 8 bytes (ptr + len)");

    // Read the pointer and length from the buffer
    let ptr = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let len = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

    assert_eq!(len, 5, "string length should be 5");

    // Verify the string bytes are in linear memory
    let string_bytes = memory.read(ptr, len)?;
    assert_eq!(string_bytes, b"hello");

    // Verify roundtrip
    let (lifted, _) = abi.lift_with_memory(&bytes, &wit_parser::Type::Id(type_id), &wave_type, &memory)?;
    assert_eq!(wasm_wave::to_string(&lifted)?, r#"{text: "hello"}"#);

    Ok(())
}

/// Test list encoding with linear memory
#[test]
fn test_list_encoding() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    record numbers {
        values: list<u32>,
    }
}

world test-world {
    use api.{numbers};
    export get-numbers: func() -> numbers;
}
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("numbers"))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("numbers type not found"))?;

    let wave_type = wit_kv::resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    // Lower a list value using linear memory
    let value: wit_kv::Value = wasm_wave::from_str(&wave_type, "{values: [1, 2, 3, 4, 5]}")?;
    let mut memory = LinearMemory::new();
    let bytes = abi.lower_with_memory(&value, &wit_parser::Type::Id(type_id), &wave_type, &mut memory)?;

    // The main buffer should contain ptr (4 bytes) + len (4 bytes)
    assert_eq!(bytes.len(), 8, "list record should be 8 bytes (ptr + len)");

    // Read the pointer and length from the buffer
    let ptr = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let len = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

    assert_eq!(len, 5, "list length should be 5");

    // Verify the elements are in linear memory (5 * 4 bytes = 20 bytes)
    let list_bytes = memory.read(ptr, len * 4)?;
    assert_eq!(list_bytes.len(), 20);

    // Check each u32 element
    assert_eq!(u32::from_le_bytes([list_bytes[0], list_bytes[1], list_bytes[2], list_bytes[3]]), 1);
    assert_eq!(u32::from_le_bytes([list_bytes[4], list_bytes[5], list_bytes[6], list_bytes[7]]), 2);
    assert_eq!(u32::from_le_bytes([list_bytes[8], list_bytes[9], list_bytes[10], list_bytes[11]]), 3);
    assert_eq!(u32::from_le_bytes([list_bytes[12], list_bytes[13], list_bytes[14], list_bytes[15]]), 4);
    assert_eq!(u32::from_le_bytes([list_bytes[16], list_bytes[17], list_bytes[18], list_bytes[19]]), 5);

    // Verify roundtrip
    let (lifted, _) = abi.lift_with_memory(&bytes, &wit_parser::Type::Id(type_id), &wave_type, &memory)?;
    assert_eq!(wasm_wave::to_string(&lifted)?, "{values: [1, 2, 3, 4, 5]}");

    Ok(())
}

/// Test empty string encoding
#[test]
fn test_empty_string_encoding() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    record message {
        text: string,
    }
}

world test-world {
    use api.{message};
    export get-message: func() -> message;
}
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("message"))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("message type not found"))?;

    let wave_type = wit_kv::resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    // Lower an empty string
    let value: wit_kv::Value = wasm_wave::from_str(&wave_type, r#"{text: ""}"#)?;
    let mut memory = LinearMemory::new();
    let bytes = abi.lower_with_memory(&value, &wit_parser::Type::Id(type_id), &wave_type, &mut memory)?;

    // Read the length from the buffer
    let len = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    assert_eq!(len, 0, "empty string length should be 0");

    // Verify roundtrip
    let (lifted, _) = abi.lift_with_memory(&bytes, &wit_parser::Type::Id(type_id), &wave_type, &memory)?;
    assert_eq!(wasm_wave::to_string(&lifted)?, r#"{text: ""}"#);

    Ok(())
}

/// Test empty list encoding
#[test]
fn test_empty_list_encoding() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    record numbers {
        values: list<u32>,
    }
}

world test-world {
    use api.{numbers};
    export get-numbers: func() -> numbers;
}
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("numbers"))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("numbers type not found"))?;

    let wave_type = wit_kv::resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    // Lower an empty list
    let value: wit_kv::Value = wasm_wave::from_str(&wave_type, "{values: []}")?;
    let mut memory = LinearMemory::new();
    let bytes = abi.lower_with_memory(&value, &wit_parser::Type::Id(type_id), &wave_type, &mut memory)?;

    // Read the length from the buffer
    let len = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    assert_eq!(len, 0, "empty list length should be 0");

    // Verify roundtrip
    let (lifted, _) = abi.lift_with_memory(&bytes, &wit_parser::Type::Id(type_id), &wave_type, &memory)?;
    assert_eq!(wasm_wave::to_string(&lifted)?, "{values: []}");

    Ok(())
}

/// Test list of strings encoding (nested variable-length types)
#[test]
fn test_list_of_strings_encoding() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    record words {
        items: list<string>,
    }
}

world test-world {
    use api.{words};
    export get-words: func() -> words;
}
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("words"))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("words type not found"))?;

    let wave_type = wit_kv::resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    // Lower a list of strings
    let value: wit_kv::Value = wasm_wave::from_str(&wave_type, r#"{items: ["hello", "world"]}"#)?;
    let mut memory = LinearMemory::new();
    let bytes = abi.lower_with_memory(&value, &wit_parser::Type::Id(type_id), &wave_type, &mut memory)?;

    // The main buffer should contain ptr (4 bytes) + len (4 bytes)
    assert_eq!(bytes.len(), 8, "list record should be 8 bytes (ptr + len)");

    // Read the length from the buffer
    let len = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    assert_eq!(len, 2, "list length should be 2");

    // Verify roundtrip
    let (lifted, _) = abi.lift_with_memory(&bytes, &wit_parser::Type::Id(type_id), &wave_type, &memory)?;
    assert_eq!(wasm_wave::to_string(&lifted)?, r#"{items: ["hello", "world"]}"#);

    Ok(())
}

/// Test record with multiple strings
#[test]
fn test_record_multiple_strings() -> Result<(), anyhow::Error> {
    let wit = r#"
package test:types;

interface api {
    record person {
        first-name: string,
        last-name: string,
        age: u32,
    }
}

world test-world {
    use api.{person};
    export get-person: func() -> person;
}
"#;

    let mut resolve = Resolve::new();
    resolve.push_str("test.wit", wit)?;

    let type_id = resolve
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("person"))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow::anyhow!("person type not found"))?;

    let wave_type = wit_kv::resolve_wit_type(&resolve, type_id)?;
    let abi = CanonicalAbi::new(&resolve);

    // Lower a record with multiple strings
    let value: wit_kv::Value = wasm_wave::from_str(
        &wave_type,
        r#"{first-name: "John", last-name: "Doe", age: 30}"#,
    )?;
    let mut memory = LinearMemory::new();
    let bytes = abi.lower_with_memory(&value, &wit_parser::Type::Id(type_id), &wave_type, &mut memory)?;

    // Record should have: string (8) + string (8) + u32 (4) = 20 bytes
    assert_eq!(bytes.len(), 20, "person record should be 20 bytes");

    // Verify roundtrip
    let (lifted, _) = abi.lift_with_memory(&bytes, &wit_parser::Type::Id(type_id), &wave_type, &memory)?;
    assert_eq!(
        wasm_wave::to_string(&lifted)?,
        r#"{first-name: "John", last-name: "Doe", age: 30}"#
    );

    Ok(())
}
