//! Type checker for nana.
//!
//! A forward-only type checker operating on the MIR. No bidirectional inference,
//! no unification, no type variables — just concrete type propagation.
//!
//! The primary use case is validating method set construction: each lexical
//! `method_set(...)` call produces a generative type (like tags) that doesn't
//! unify with other method set types.

use std::collections::HashMap;

use crate::ast::Pattern;
use crate::mir::{Mir, MirKind};
use crate::value::{
    TagId, TAG_ID_ARRAY, TAG_ID_BOOL, TAG_ID_BYTE, TAG_ID_CHAR, TAG_ID_FLOAT, TAG_ID_INT,
    TAG_ID_STRING, TAG_ID_UNIT,
};

// ── Types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Int,
    Float,
    Bool,
    String,
    Char,
    Byte,
    Unit,
    Array,
    Struct(Vec<(std::string::String, Ty)>),
    Fn {
        param: Box<Ty>,
        ret: Box<Ty>,
    },
    TagConstructor(TagId),
    MethodSet {
        id: u64,
        tag_id: TagId,
    },
    /// Escape hatch for constructs we don't type yet.
    Unknown,
}

// ── Type environment ────────────────────────────────────────────

pub struct TyEnv {
    bindings: Vec<(std::string::String, Ty)>,
    modules: HashMap<std::string::String, Ty>,
    next_ms_id: u64,
}

impl TyEnv {
    pub fn new() -> Self {
        TyEnv {
            bindings: Vec::new(),
            modules: HashMap::new(),
            next_ms_id: 0,
        }
    }

    pub fn with_module(mut self, name: impl Into<std::string::String>, ty: Ty) -> Self {
        self.modules.insert(name.into(), ty);
        self
    }

    /// Add a binding from outside the checker (e.g., for pre-bound builtins).
    pub fn bind_external(&mut self, name: std::string::String, ty: Ty) {
        self.bindings.push((name, ty));
    }

    fn get(&self, name: &str) -> Option<&Ty> {
        self.bindings.iter().rev().find_map(|(n, ty)| {
            if n == name {
                Some(ty)
            } else {
                None
            }
        })
    }

    fn bind(&mut self, name: std::string::String, ty: Ty) {
        self.bindings.push((name, ty));
    }

    fn pop_binding(&mut self) {
        self.bindings.pop();
    }

    fn fresh_method_set_id(&mut self) -> u64 {
        let id = self.next_ms_id;
        self.next_ms_id += 1;
        id
    }
}

// ── Core module type ────────────────────────────────────────────

/// The type of `build_core_module()` from eval.rs.
pub fn core_module_type() -> Ty {
    let fn_ty = || Ty::Fn {
        param: Box::new(Ty::Unknown),
        ret: Box::new(Ty::Unknown),
    };

    let mut fields: Vec<(std::string::String, Ty)> = Vec::new();

    // Type constructors (same order as build_core_module)
    let type_constructors: &[(&str, TagId)] = &[
        ("Int", TAG_ID_INT),
        ("Float", TAG_ID_FLOAT),
        ("Bool", TAG_ID_BOOL),
        ("String", TAG_ID_STRING),
        ("Char", TAG_ID_CHAR),
        ("Byte", TAG_ID_BYTE),
        ("Array", TAG_ID_ARRAY),
        ("Unit", TAG_ID_UNIT),
    ];
    for (name, id) in type_constructors {
        fields.push((name.to_string(), Ty::TagConstructor(*id)));
    }

    // All builtins as Fn(Unknown -> Unknown)
    let builtins: &[&str] = &[
        "not", "and", "or", "len", "print", "map", "filter", "fold", "zip",
        "byte", "int", "float", "char", "ref_eq", "val_eq", "method_set",
        "array_get", "array_slice", "array_len", "array_map", "array_filter",
        "array_fold", "array_zip",
        "array_add", "array_eq", "array_not_eq",
        "string_byte_len", "string_char_len", "string_byte_get", "string_char_get",
        "string_as_bytes", "string_chars", "string_split", "string_trim",
        "string_contains", "string_slice", "string_starts_with", "string_ends_with",
        "string_replace",
        "string_add", "string_eq", "string_not_eq", "string_lt", "string_gt",
        "string_lt_eq", "string_gt_eq", "string_to_string",
        "int_add", "int_subtract", "int_times", "int_divided_by", "int_negate",
        "int_eq", "int_not_eq", "int_lt", "int_gt", "int_lt_eq", "int_gt_eq",
        "int_to_string",
        "float_add", "float_subtract", "float_times", "float_divided_by", "float_negate",
        "float_eq", "float_not_eq", "float_lt", "float_gt", "float_lt_eq", "float_gt_eq",
        "float_to_string",
        "bool_eq", "bool_not_eq", "bool_to_string",
        "char_eq", "char_not_eq", "char_lt", "char_gt", "char_lt_eq", "char_gt_eq",
        "char_to_string",
        "byte_eq", "byte_not_eq", "byte_lt", "byte_gt", "byte_lt_eq", "byte_gt_eq",
        "byte_to_string",
        "unit_eq", "unit_not_eq",
    ];
    for name in builtins {
        fields.push((name.to_string(), fn_ty()));
    }

    Ty::Struct(fields)
}

// ── Type checker ────────────────────────────────────────────────

pub fn check(mir: &Mir, env: &mut TyEnv) -> Result<Ty, std::string::String> {
    match mir.as_ref() {
        // ── Literals ──
        MirKind::Int(_) => Ok(Ty::Int),
        MirKind::Float(_) => Ok(Ty::Float),
        MirKind::Bool(_) => Ok(Ty::Bool),
        MirKind::Str(_) => Ok(Ty::String),
        MirKind::Char(_) => Ok(Ty::Char),
        MirKind::Byte(_) => Ok(Ty::Byte),
        MirKind::Unit => Ok(Ty::Unit),

        // ── Ident ──
        MirKind::Ident(name) => {
            env.get(name)
                .cloned()
                .ok_or_else(|| format!("type error: undefined variable '{}'", name))
        }

        // ── Import ──
        MirKind::Import(name) => {
            env.modules
                .get(name)
                .cloned()
                .ok_or_else(|| format!("type error: module '{}' not found", name))
        }

        // ── FieldAccess ──
        MirKind::FieldAccess(expr, field) => {
            let ty = check(expr, env)?;
            match ty {
                Ty::Struct(fields) => {
                    fields
                        .iter()
                        .find(|(name, _)| name == field)
                        .map(|(_, ty)| ty.clone())
                        .ok_or_else(|| format!("type error: no field '{}' in struct", field))
                }
                Ty::Unknown => Ok(Ty::Unknown),
                _ => Err(format!("type error: field access on non-struct type")),
            }
        }

        // ── Struct literal ──
        MirKind::Struct(fields) => {
            let mut typed_fields = Vec::new();
            let mut positional_idx = 0u64;
            for field in fields {
                let ty = check(&field.value, env)?;
                let label = match &field.label {
                    Some(name) => name.clone(),
                    None => {
                        let label = positional_idx.to_string();
                        positional_idx += 1;
                        label
                    }
                };
                typed_fields.push((label, ty));
            }
            Ok(Ty::Struct(typed_fields))
        }

        // ── Bind ──
        MirKind::Bind { name, value, body } => {
            let val_ty = check(value, env)?;
            env.bind(name.clone(), val_ty);
            let body_ty = check(body, env)?;
            env.pop_binding();
            Ok(body_ty)
        }

        // ── Pipe (for let/letarray/apply that weren't desugared) ──
        MirKind::Pipe(lhs, rhs) => {
            check_pipe(lhs, rhs, env)
        }

        // ── Let (pattern destructuring — standalone, not via pipe) ──
        MirKind::Let { pattern, body } => {
            check_let(pattern, body, &Ty::Unknown, env)
        }

        // ── Call ──
        MirKind::Call(func, arg) => {
            check_call(func, arg, env)
        }

        // ── Apply (method set scope) ──
        MirKind::Apply { expr, body } => {
            let ms_ty = check(expr, env)?;
            match &ms_ty {
                Ty::MethodSet { .. } => check(body, env),
                Ty::Unknown => check(body, env),
                _ => Err("type error: apply expects a method set".to_string()),
            }
        }

        // ── NewTag ──
        MirKind::NewTag(id, _name) => Ok(Ty::TagConstructor(*id)),

        // ── Everything else → Unknown ──
        MirKind::Block(_)
        | MirKind::BranchBlock(_)
        | MirKind::Array(_)
        | MirKind::MethodCall { .. }
        | MirKind::LetArray { .. } => Ok(Ty::Unknown),
    }
}

fn check_pipe(lhs: &Mir, rhs: &Mir, env: &mut TyEnv) -> Result<Ty, std::string::String> {
    let lhs_ty = check(lhs, env)?;
    match rhs.as_ref() {
        // Pipe into let: `expr >> let(pattern); body`
        // The lhs value is bound to the pattern.
        MirKind::Let { pattern, body } => {
            check_let(pattern, body, &lhs_ty, env)
        }
        // Pipe into apply: `expr >> apply(ms); body`
        MirKind::Apply { expr, body } => {
            let ms_ty = check(expr, env)?;
            match &ms_ty {
                Ty::MethodSet { .. } => check(body, env),
                Ty::Unknown => check(body, env),
                _ => Err("type error: apply expects a method set".to_string()),
            }
        }
        _ => Ok(Ty::Unknown),
    }
}

fn check_let(
    pattern: &Pattern,
    body: &Mir,
    input_ty: &Ty,
    env: &mut TyEnv,
) -> Result<Ty, std::string::String> {
    match pattern {
        Pattern::Name(name) => {
            env.bind(name.clone(), input_ty.clone());
            let body_ty = check(body, env)?;
            env.pop_binding();
            Ok(body_ty)
        }
        Pattern::Discard => check(body, env),
        Pattern::Fields(_) => {
            // Destructuring not yet supported
            check(body, env)
        }
    }
}

fn check_call(func: &Mir, arg: &Mir, env: &mut TyEnv) -> Result<Ty, std::string::String> {
    // Special case: method_set call
    if let MirKind::Ident(name) = func.as_ref() {
        if name == "method_set" {
            return check_method_set_call(arg, env);
        }
    }

    let func_ty = check(func, env)?;
    let _arg_ty = check(arg, env)?;

    match func_ty {
        Ty::Fn { ret, .. } => Ok(*ret),
        Ty::TagConstructor(_) => Ok(Ty::Unknown),
        Ty::Unknown => Ok(Ty::Unknown),
        _ => Err(format!("type error: calling a non-function")),
    }
}

fn check_method_set_call(arg: &Mir, env: &mut TyEnv) -> Result<Ty, std::string::String> {
    // method_set(constructor, struct_of_methods)
    // The parser produces Call(Ident("method_set"), Struct([positional_ctor, positional_methods]))
    let arg_ty = check(arg, env)?;
    match arg_ty {
        Ty::Struct(ref fields) if fields.len() == 2 => {
            let ctor_ty = &fields[0].1;
            let methods_ty = &fields[1].1;

            let tag_id = match ctor_ty {
                Ty::TagConstructor(id) => *id,
                _ => {
                    return Err(
                        "type error: method_set first argument must be a type constructor"
                            .to_string(),
                    )
                }
            };

            match methods_ty {
                Ty::Struct(_) => {}
                _ => {
                    return Err(
                        "type error: method_set second argument must be a struct of functions"
                            .to_string(),
                    )
                }
            }

            let id = env.fresh_method_set_id();
            Ok(Ty::MethodSet { id, tag_id })
        }
        _ => Err("type error: method_set expects (constructor, struct_of_functions)".to_string()),
    }
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::MirField;

    fn mir(kind: MirKind) -> Mir {
        Box::new(kind)
    }

    #[test]
    fn literal_types() {
        let mut env = TyEnv::new();
        assert_eq!(check(&mir(MirKind::Int(42)), &mut env).unwrap(), Ty::Int);
        assert_eq!(check(&mir(MirKind::Float(1.0)), &mut env).unwrap(), Ty::Float);
        assert_eq!(check(&mir(MirKind::Bool(true)), &mut env).unwrap(), Ty::Bool);
        assert_eq!(
            check(&mir(MirKind::Str("hi".into())), &mut env).unwrap(),
            Ty::String
        );
        assert_eq!(check(&mir(MirKind::Char('a')), &mut env).unwrap(), Ty::Char);
        assert_eq!(check(&mir(MirKind::Byte(0)), &mut env).unwrap(), Ty::Byte);
        assert_eq!(check(&mir(MirKind::Unit), &mut env).unwrap(), Ty::Unit);
    }

    #[test]
    fn ident_lookup() {
        let mut env = TyEnv::new();
        env.bind("x".into(), Ty::Int);
        assert_eq!(check(&mir(MirKind::Ident("x".into())), &mut env).unwrap(), Ty::Int);
    }

    #[test]
    fn ident_undefined() {
        let mut env = TyEnv::new();
        assert!(check(&mir(MirKind::Ident("x".into())), &mut env).is_err());
    }

    #[test]
    fn import_module() {
        let mut env = TyEnv::new().with_module("core", Ty::Int);
        assert_eq!(
            check(&mir(MirKind::Import("core".into())), &mut env).unwrap(),
            Ty::Int
        );
    }

    #[test]
    fn field_access() {
        let mut env = TyEnv::new();
        let struct_ty = Ty::Struct(vec![("x".into(), Ty::Int), ("y".into(), Ty::Float)]);
        env.bind("s".into(), struct_ty);
        let expr = mir(MirKind::FieldAccess(
            mir(MirKind::Ident("s".into())),
            "x".into(),
        ));
        assert_eq!(check(&expr, &mut env).unwrap(), Ty::Int);
    }

    #[test]
    fn field_access_missing() {
        let mut env = TyEnv::new();
        let struct_ty = Ty::Struct(vec![("x".into(), Ty::Int)]);
        env.bind("s".into(), struct_ty);
        let expr = mir(MirKind::FieldAccess(
            mir(MirKind::Ident("s".into())),
            "z".into(),
        ));
        assert!(check(&expr, &mut env).is_err());
    }

    #[test]
    fn struct_literal() {
        let mut env = TyEnv::new();
        let expr = mir(MirKind::Struct(vec![
            MirField { label: Some("a".into()), value: mir(MirKind::Int(1)), is_spread: false },
            MirField { label: None, value: mir(MirKind::Bool(true)), is_spread: false },
        ]));
        let ty = check(&expr, &mut env).unwrap();
        assert_eq!(
            ty,
            Ty::Struct(vec![("a".into(), Ty::Int), ("0".into(), Ty::Bool)])
        );
    }

    #[test]
    fn bind_simple() {
        let mut env = TyEnv::new();
        let expr = mir(MirKind::Bind {
            name: "x".into(),
            value: mir(MirKind::Int(42)),
            body: mir(MirKind::Ident("x".into())),
        });
        assert_eq!(check(&expr, &mut env).unwrap(), Ty::Int);
    }

    #[test]
    fn pipe_let_name() {
        // use(core) desugars to: Pipe(Import("core"), Let { Name("core"), body })
        let mut env = TyEnv::new().with_module("test", Ty::Int);
        let expr = mir(MirKind::Pipe(
            mir(MirKind::Import("test".into())),
            mir(MirKind::Let {
                pattern: Pattern::Name("t".into()),
                body: mir(MirKind::Ident("t".into())),
            }),
        ));
        assert_eq!(check(&expr, &mut env).unwrap(), Ty::Int);
    }

    #[test]
    fn method_set_call() {
        let mut env = TyEnv::new();
        env.bind("Int".into(), Ty::TagConstructor(TAG_ID_INT));
        let fn_ty = Ty::Fn {
            param: Box::new(Ty::Unknown),
            ret: Box::new(Ty::Unknown),
        };
        env.bind("int_add".into(), fn_ty);
        env.bind(
            "method_set".into(),
            Ty::Fn {
                param: Box::new(Ty::Unknown),
                ret: Box::new(Ty::Unknown),
            },
        );

        let expr = mir(MirKind::Call(
            mir(MirKind::Ident("method_set".into())),
            mir(MirKind::Struct(vec![
                MirField {
                    label: None,
                    value: mir(MirKind::Ident("Int".into())),
                    is_spread: false,
                },
                MirField {
                    label: None,
                    value: mir(MirKind::Struct(vec![MirField {
                        label: Some("add".into()),
                        value: mir(MirKind::Ident("int_add".into())),
                        is_spread: false,
                    }])),
                    is_spread: false,
                },
            ])),
        ));

        let ty = check(&expr, &mut env).unwrap();
        assert!(matches!(ty, Ty::MethodSet { id: 0, tag_id } if tag_id == TAG_ID_INT));
    }

    #[test]
    fn method_set_non_constructor_errors() {
        let mut env = TyEnv::new();
        env.bind("not_a_ctor".into(), Ty::Int);
        env.bind(
            "method_set".into(),
            Ty::Fn {
                param: Box::new(Ty::Unknown),
                ret: Box::new(Ty::Unknown),
            },
        );

        let expr = mir(MirKind::Call(
            mir(MirKind::Ident("method_set".into())),
            mir(MirKind::Struct(vec![
                MirField {
                    label: None,
                    value: mir(MirKind::Ident("not_a_ctor".into())),
                    is_spread: false,
                },
                MirField {
                    label: None,
                    value: mir(MirKind::Struct(vec![])),
                    is_spread: false,
                },
            ])),
        ));

        assert!(check(&expr, &mut env).is_err());
    }

    #[test]
    fn method_set_non_struct_methods_errors() {
        let mut env = TyEnv::new();
        env.bind("Int".into(), Ty::TagConstructor(TAG_ID_INT));
        env.bind(
            "method_set".into(),
            Ty::Fn {
                param: Box::new(Ty::Unknown),
                ret: Box::new(Ty::Unknown),
            },
        );

        let expr = mir(MirKind::Call(
            mir(MirKind::Ident("method_set".into())),
            mir(MirKind::Struct(vec![
                MirField {
                    label: None,
                    value: mir(MirKind::Ident("Int".into())),
                    is_spread: false,
                },
                MirField {
                    label: None,
                    value: mir(MirKind::Int(42)),
                    is_spread: false,
                },
            ])),
        ));

        assert!(check(&expr, &mut env).is_err());
    }

    #[test]
    fn typecheck_std_nana() {
        let source = include_str!("std.nana");
        let ast = crate::parse(source).expect("parse failed");
        let mir = crate::mir::lower(&ast);

        let mut env = TyEnv::new()
            .with_module("core", core_module_type());
        // std.nana needs method_set as a bare binding
        env.bind(
            "method_set".into(),
            Ty::Fn {
                param: Box::new(Ty::Unknown),
                ret: Box::new(Ty::Unknown),
            },
        );

        let ty = check(&mir, &mut env).expect("typecheck failed");

        // The result should be a struct with known fields
        match &ty {
            Ty::Struct(fields) => {
                let field_names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
                assert!(field_names.contains(&"Int"));
                assert!(field_names.contains(&"int_methods"));
                assert!(field_names.contains(&"prelude"));
                assert!(field_names.contains(&"not"));

                // int_methods should be a MethodSet for Int
                let int_methods = fields.iter().find(|(n, _)| n == "int_methods").unwrap();
                match &int_methods.1 {
                    Ty::MethodSet { tag_id, .. } => {
                        assert_eq!(*tag_id, TAG_ID_INT);
                    }
                    other => panic!("expected MethodSet, got {:?}", other),
                }

                // prelude should be a struct of method sets
                let prelude = fields.iter().find(|(n, _)| n == "prelude").unwrap();
                match &prelude.1 {
                    Ty::Struct(prelude_fields) => {
                        assert!(prelude_fields.len() >= 8); // 8 primitive method sets
                        for (_, ty) in prelude_fields {
                            assert!(matches!(ty, Ty::MethodSet { .. }));
                        }
                    }
                    other => panic!("expected Struct for prelude, got {:?}", other),
                }
            }
            other => panic!("expected Struct, got {:?}", other),
        }
    }
}
