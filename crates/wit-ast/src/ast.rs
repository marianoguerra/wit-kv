//! WIT AST resource implementation.
//!
//! This module wraps wit_parser::Resolve and provides conversion to the WIT-defined
//! type-def structures that can be exposed across the component boundary.

use std::collections::HashMap;

use wit_parser::{Resolve, Type, TypeDefKind, TypeId, UnresolvedPackageGroup};

// Import types from the generated bindings
pub use crate::exports::wit_kv::wit_ast::types::{
    PrimitiveKind, TypeCase, TypeDef, TypeDefKind as WitTypeDefKind, TypeField, TypeRef,
};

/// Parsed WIT AST wrapping a wit_parser::Resolve.
pub struct WitAst {
    resolve: Resolve,
    /// Map from type name to index in our type list
    name_to_index: HashMap<String, u32>,
    /// Cached type definitions converted to our WIT format
    type_defs: Vec<TypeDef>,
}

impl WitAst {
    /// Parse a WIT definition string into a WitAst.
    pub fn parse(definition: &str) -> Result<Self, ParseError> {
        let group = UnresolvedPackageGroup::parse("input.wit", definition)
            .map_err(|e| ParseError::new(e.to_string()))?;

        let mut resolve = Resolve::default();
        let pkg_id = resolve
            .push_group(group)
            .map_err(|e| ParseError::new(e.to_string()))?;

        // Build type mappings
        let mut name_to_index = HashMap::new();
        let mut type_defs = Vec::new();

        // Iterate through all types in the package we just added
        if let Some(pkg) = resolve.packages.get(pkg_id) {
            // Collect types from all interfaces in the package
            for (_iface_name, iface_id) in &pkg.interfaces {
                if let Some(iface) = resolve.interfaces.get(*iface_id) {
                    for (type_name, type_id) in &iface.types {
                        let index = type_defs.len() as u32;
                        name_to_index.insert(type_name.clone(), index);

                        if let Some(type_def) =
                            convert_type_def(&resolve, *type_id, type_name.clone())
                        {
                            type_defs.push(type_def);
                        }
                    }
                }
            }
        }

        Ok(Self {
            resolve,
            name_to_index,
            type_defs,
        })
    }

    /// Get all type definitions.
    pub fn types(&self) -> Vec<TypeDef> {
        self.type_defs.clone()
    }

    /// Find type index by name.
    pub fn find_type(&self, name: &str) -> Option<u32> {
        self.name_to_index.get(name).copied()
    }

    /// Get the underlying wit_parser::Resolve.
    pub fn resolve(&self) -> &Resolve {
        &self.resolve
    }

    /// Look up a wit_parser::Type by name.
    pub fn get_wit_type(&self, name: &str) -> Option<Type> {
        // Search through all packages and interfaces for the type
        for (_pkg_id, pkg) in &self.resolve.packages {
            for (_iface_name, iface_id) in &pkg.interfaces {
                if let Some(iface) = self.resolve.interfaces.get(*iface_id)
                    && let Some(type_id) = iface.types.get(name)
                {
                    return Some(Type::Id(*type_id));
                }
            }
        }
        None
    }
}

/// Error type for parsing WIT definitions.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

impl ParseError {
    pub fn new(message: String) -> Self {
        // Try to extract line/column from error message if present
        Self {
            message,
            line: None,
            column: None,
        }
    }
}

/// Convert a wit_parser type definition to our WIT format.
fn convert_type_def(resolve: &Resolve, type_id: TypeId, name: String) -> Option<TypeDef> {
    let ty = resolve.types.get(type_id)?;
    let kind = convert_type_def_kind(resolve, &ty.kind)?;

    Some(TypeDef { name, kind })
}

/// Convert a wit_parser TypeDefKind to our WIT format.
fn convert_type_def_kind(resolve: &Resolve, kind: &TypeDefKind) -> Option<WitTypeDefKind> {
    match kind {
        TypeDefKind::Type(inner) => {
            let type_ref = convert_type_to_ref(resolve, inner)?;
            Some(WitTypeDefKind::TypeAlias(type_ref))
        }
        TypeDefKind::Record(r) => {
            let fields: Vec<_> = r
                .fields
                .iter()
                .filter_map(|f| {
                    Some(TypeField {
                        name: f.name.clone(),
                        ty: convert_type_to_ref(resolve, &f.ty)?,
                    })
                })
                .collect();
            Some(WitTypeDefKind::TypeRecord(fields))
        }
        TypeDefKind::Tuple(t) => {
            let types: Vec<_> = t
                .types
                .iter()
                .filter_map(|ty| convert_type_to_ref(resolve, ty))
                .collect();
            Some(WitTypeDefKind::TypeTuple(types))
        }
        TypeDefKind::Flags(f) => {
            let names: Vec<_> = f.flags.iter().map(|flag| flag.name.clone()).collect();
            Some(WitTypeDefKind::TypeFlags(names))
        }
        TypeDefKind::Enum(e) => {
            let names: Vec<_> = e.cases.iter().map(|c| c.name.clone()).collect();
            Some(WitTypeDefKind::TypeEnum(names))
        }
        TypeDefKind::Variant(v) => {
            let cases: Vec<_> = v
                .cases
                .iter()
                .map(|c| TypeCase {
                    name: c.name.clone(),
                    ty: c.ty.as_ref().and_then(|t| convert_type_to_ref(resolve, t)),
                })
                .collect();
            Some(WitTypeDefKind::TypeVariant(cases))
        }
        TypeDefKind::Option(inner) => {
            let type_ref = convert_type_to_ref(resolve, inner)?;
            Some(WitTypeDefKind::TypeOption(type_ref))
        }
        TypeDefKind::Result(r) => {
            let ok_ref = r.ok.as_ref().and_then(|t| convert_type_to_ref(resolve, t));
            let err_ref = r.err.as_ref().and_then(|t| convert_type_to_ref(resolve, t));
            Some(WitTypeDefKind::TypeResult((ok_ref, err_ref)))
        }
        TypeDefKind::List(inner) => {
            let type_ref = convert_type_to_ref(resolve, inner)?;
            Some(WitTypeDefKind::TypeList(type_ref))
        }
        // Unsupported types
        TypeDefKind::Handle(_)
        | TypeDefKind::Resource
        | TypeDefKind::Future(_)
        | TypeDefKind::Stream(_)
        | TypeDefKind::FixedSizeList(_, _)
        | TypeDefKind::Map(_, _)
        | TypeDefKind::Unknown => None,
    }
}

/// Convert a wit_parser::Type to our TypeRef format.
fn convert_type_to_ref(resolve: &Resolve, ty: &Type) -> Option<TypeRef> {
    match ty {
        Type::Bool => Some(TypeRef::Primitive(PrimitiveKind::PrimBool)),
        Type::U8 => Some(TypeRef::Primitive(PrimitiveKind::PrimU8)),
        Type::U16 => Some(TypeRef::Primitive(PrimitiveKind::PrimU16)),
        Type::U32 => Some(TypeRef::Primitive(PrimitiveKind::PrimU32)),
        Type::U64 => Some(TypeRef::Primitive(PrimitiveKind::PrimU64)),
        Type::S8 => Some(TypeRef::Primitive(PrimitiveKind::PrimS8)),
        Type::S16 => Some(TypeRef::Primitive(PrimitiveKind::PrimS16)),
        Type::S32 => Some(TypeRef::Primitive(PrimitiveKind::PrimS32)),
        Type::S64 => Some(TypeRef::Primitive(PrimitiveKind::PrimS64)),
        Type::F32 => Some(TypeRef::Primitive(PrimitiveKind::PrimF32)),
        Type::F64 => Some(TypeRef::Primitive(PrimitiveKind::PrimF64)),
        Type::Char => Some(TypeRef::Primitive(PrimitiveKind::PrimChar)),
        Type::String => Some(TypeRef::Primitive(PrimitiveKind::PrimString)),
        Type::Id(id) => {
            // Look up the type to get its name and find its index
            let type_def = resolve.types.get(*id)?;
            if let Some(_name) = &type_def.name {
                // For now, we return the type id's index as u32
                // This should be mapped to our type_defs index in practice
                Some(TypeRef::Defined(id.index() as u32))
            } else {
                // Anonymous type - try to inline it
                match &type_def.kind {
                    TypeDefKind::Option(inner) => {
                        // This is an anonymous option, convert recursively
                        convert_type_to_ref(resolve, inner)
                    }
                    TypeDefKind::Result(_r) => {
                        // Anonymous result - we'd need to handle this specially
                        // For now, return the id
                        Some(TypeRef::Defined(id.index() as u32))
                    }
                    TypeDefKind::List(inner) => {
                        // Anonymous list
                        convert_type_to_ref(resolve, inner)
                    }
                    TypeDefKind::Tuple(_t) => {
                        // Anonymous tuple - return as defined type
                        Some(TypeRef::Defined(id.index() as u32))
                    }
                    _ => Some(TypeRef::Defined(id.index() as u32)),
                }
            }
        }
        Type::ErrorContext => None,
    }
}
