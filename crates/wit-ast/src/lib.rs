//! wit-ast - WIT AST WebAssembly Component
//!
//! A WASM component that exposes a WIT interface for:
//! - Parsing WIT definitions into an AST
//! - Lifting canonical ABI binary data into a value-tree AST
//! - Converting value-tree to/from WAVE text format

mod abi;
mod ast;
mod value_convert;

use std::cell::RefCell;

use wasm_wave::value::Type as WaveType;
use wit_parser::Type;

use crate::abi::{CanonicalAbi, EncodedValue, LinearMemory};
use crate::ast::WitAst;
use crate::value_convert::{value_tree_to_wave, wave_to_value_tree};

// Generate bindings for the wit-ast-decoder world
wit_bindgen::generate!({
    world: "wit-ast-decoder",
    path: "wit",
});

// Import types and traits from the generated bindings
use exports::wit_kv::wit_ast::types::{
    BinaryExport, FormatError, GuestWitAst, LiftError, TypeDef, ValueTree,
};
use exports::wit_kv::wit_ast::formatter::Guest as FormatterGuest;
use exports::wit_kv::wit_ast::lifter::Guest as LifterGuest;
use exports::wit_kv::wit_ast::parser::Guest as ParserGuest;

/// Resource wrapper for WitAst
pub struct WitAstResource {
    inner: RefCell<WitAst>,
}

impl GuestWitAst for WitAstResource {
    fn types(&self) -> Vec<TypeDef> {
        self.inner.borrow().types()
    }

    fn find_type(&self, name: String) -> Option<u32> {
        self.inner.borrow().find_type(&name)
    }
}

// Export the component
export!(Component);

struct Component;

impl exports::wit_kv::wit_ast::types::Guest for Component {
    type WitAst = WitAstResource;
}

impl ParserGuest for Component {
    fn parse_wit(
        definition: String,
    ) -> Result<exports::wit_kv::wit_ast::types::WitAst, exports::wit_kv::wit_ast::types::ParseError>
    {
        match WitAst::parse(&definition) {
            Ok(ast) => Ok(exports::wit_kv::wit_ast::types::WitAst::new(WitAstResource {
                inner: RefCell::new(ast),
            })),
            Err(e) => Err(exports::wit_kv::wit_ast::types::ParseError {
                message: e.message,
                line: e.line,
                column: e.column,
            }),
        }
    }
}

impl LifterGuest for Component {
    fn lift(
        ast: exports::wit_kv::wit_ast::types::WitAstBorrow<'_>,
        type_name: String,
        data: BinaryExport,
    ) -> Result<ValueTree, LiftError> {
        let ast_resource = ast.get::<WitAstResource>();
        let wit_ast = ast_resource.inner.borrow();

        // Look up the WIT type by name
        let wit_ty = wit_ast.get_wit_type(&type_name).ok_or_else(|| LiftError {
            message: format!("Type '{}' not found in WIT definition", type_name),
            context: None,
        })?;

        // Get the resolve and create the canonical ABI
        let resolve = wit_ast.resolve();
        let abi = CanonicalAbi::new(resolve);

        // Build the wave type from the WIT type
        let wave_ty = build_wave_type(resolve, &wit_ty).map_err(|e| LiftError {
            message: e,
            context: Some("building wave type".to_string()),
        })?;

        // Create encoded value from the binary export
        let encoded = EncodedValue::new(data.value, data.memory);

        // Lift the value
        let memory = LinearMemory::from_option(encoded.memory.clone());
        let (wave_value, _) = abi
            .lift_with_memory(&encoded.buffer, &wit_ty, &wave_ty, &memory)
            .map_err(|e| LiftError {
                message: e.to_string(),
                context: Some("lifting value".to_string()),
            })?;

        // Convert wave value to value-tree
        Ok(wave_to_value_tree(&wave_value))
    }
}

impl FormatterGuest for Component {
    fn value_tree_to_wave(
        ast: exports::wit_kv::wit_ast::types::WitAstBorrow<'_>,
        type_name: String,
        value: ValueTree,
    ) -> Result<String, FormatError> {
        let ast_resource = ast.get::<WitAstResource>();
        let wit_ast = ast_resource.inner.borrow();

        // Look up the WIT type by name
        let wit_ty = wit_ast.get_wit_type(&type_name).ok_or_else(|| FormatError {
            message: format!("Type '{}' not found in WIT definition", type_name),
        })?;

        // Get the resolve and build the wave type
        let resolve = wit_ast.resolve();
        let wave_ty = build_wave_type(resolve, &wit_ty).map_err(|e| FormatError { message: e })?;

        // Convert value tree to wave value
        let wave_value = value_tree_to_wave(&value, &wave_ty);

        // Use wasm_wave's to_string for formatting
        wasm_wave::to_string(&wave_value).map_err(|e| FormatError {
            message: e.to_string(),
        })
    }

    fn wave_to_value_tree(
        ast: exports::wit_kv::wit_ast::types::WitAstBorrow<'_>,
        type_name: String,
        wave_text: String,
    ) -> Result<ValueTree, FormatError> {
        let ast_resource = ast.get::<WitAstResource>();
        let wit_ast = ast_resource.inner.borrow();

        // Look up the WIT type by name
        let wit_ty = wit_ast.get_wit_type(&type_name).ok_or_else(|| FormatError {
            message: format!("Type '{}' not found in WIT definition", type_name),
        })?;

        // Get the resolve and build the wave type
        let resolve = wit_ast.resolve();
        let wave_ty = build_wave_type(resolve, &wit_ty).map_err(|e| FormatError { message: e })?;

        // Parse the WAVE text using wasm_wave
        let wave_value: wasm_wave::value::Value =
            wasm_wave::from_str(&wave_ty, &wave_text).map_err(|e| FormatError {
                message: e.to_string(),
            })?;

        // Convert to value-tree
        Ok(wave_to_value_tree(&wave_value))
    }
}

/// Build a wasm_wave::Type from a wit_parser::Type
fn build_wave_type(resolve: &wit_parser::Resolve, wit_ty: &Type) -> Result<WaveType, String> {
    use wit_parser::TypeDefKind;

    match wit_ty {
        Type::Bool => Ok(WaveType::BOOL),
        Type::U8 => Ok(WaveType::U8),
        Type::U16 => Ok(WaveType::U16),
        Type::U32 => Ok(WaveType::U32),
        Type::U64 => Ok(WaveType::U64),
        Type::S8 => Ok(WaveType::S8),
        Type::S16 => Ok(WaveType::S16),
        Type::S32 => Ok(WaveType::S32),
        Type::S64 => Ok(WaveType::S64),
        Type::F32 => Ok(WaveType::F32),
        Type::F64 => Ok(WaveType::F64),
        Type::Char => Ok(WaveType::CHAR),
        Type::String => Ok(WaveType::STRING),
        Type::Id(id) => {
            let ty_def = resolve
                .types
                .get(*id)
                .ok_or_else(|| format!("Type ID {:?} not found", id))?;

            match &ty_def.kind {
                TypeDefKind::Type(inner) => build_wave_type(resolve, inner),
                TypeDefKind::Record(r) => {
                    let fields: Result<Vec<(String, WaveType)>, String> = r
                        .fields
                        .iter()
                        .map(|f| {
                            let field_ty = build_wave_type(resolve, &f.ty)?;
                            Ok((f.name.clone(), field_ty))
                        })
                        .collect();
                    WaveType::record(fields?)
                        .ok_or_else(|| "Cannot create empty record type".to_string())
                }
                TypeDefKind::Tuple(t) => {
                    let types: Result<Vec<_>, _> =
                        t.types.iter().map(|ty| build_wave_type(resolve, ty)).collect();
                    WaveType::tuple(types?)
                        .ok_or_else(|| "Cannot create empty tuple type".to_string())
                }
                TypeDefKind::Flags(f) => {
                    let names: Vec<_> = f.flags.iter().map(|fl| fl.name.clone()).collect();
                    WaveType::flags(names)
                        .ok_or_else(|| "Cannot create empty flags type".to_string())
                }
                TypeDefKind::Enum(e) => {
                    let cases: Vec<_> = e.cases.iter().map(|c| c.name.clone()).collect();
                    WaveType::enum_ty(cases)
                        .ok_or_else(|| "Cannot create empty enum type".to_string())
                }
                TypeDefKind::Variant(v) => {
                    let cases: Result<Vec<(String, Option<WaveType>)>, String> = v
                        .cases
                        .iter()
                        .map(|c| {
                            let payload =
                                c.ty.as_ref().map(|t| build_wave_type(resolve, t)).transpose()?;
                            Ok((c.name.clone(), payload))
                        })
                        .collect();
                    WaveType::variant(cases?)
                        .ok_or_else(|| "Cannot create empty variant type".to_string())
                }
                TypeDefKind::Option(inner) => {
                    let inner_ty = build_wave_type(resolve, inner)?;
                    Ok(WaveType::option(inner_ty))
                }
                TypeDefKind::Result(r) => {
                    let ok_ty = r
                        .ok
                        .as_ref()
                        .map(|t| build_wave_type(resolve, t))
                        .transpose()?;
                    let err_ty = r
                        .err
                        .as_ref()
                        .map(|t| build_wave_type(resolve, t))
                        .transpose()?;
                    Ok(WaveType::result(ok_ty, err_ty))
                }
                TypeDefKind::List(elem) => {
                    let elem_ty = build_wave_type(resolve, elem)?;
                    Ok(WaveType::list(elem_ty))
                }
                TypeDefKind::FixedSizeList(elem, len) => {
                    let elem_ty = build_wave_type(resolve, elem)?;
                    Ok(WaveType::fixed_size_list(elem_ty, *len))
                }
                TypeDefKind::Handle(_)
                | TypeDefKind::Resource
                | TypeDefKind::Future(_)
                | TypeDefKind::Stream(_)
                | TypeDefKind::Map(_, _)
                | TypeDefKind::Unknown => Err(format!("Unsupported type kind: {:?}", ty_def.kind)),
            }
        }
        Type::ErrorContext => Err("ErrorContext type not supported".to_string()),
    }
}
