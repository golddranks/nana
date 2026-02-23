//! Type checker for nana.
//!
//! A forward-only type checker operating on the MIR. No bidirectional inference,
//! no unification, no type variables — just concrete type propagation.
//!
//! The primary use case is validating method set construction: each lexical
//! `method_set(...)` call produces a generative type (like tags) that doesn't
//! unify with other method set types.

use std::collections::HashMap;

use crate::ast::{ArrayPat, BranchBinding, Pattern};
use crate::mir::{Mir, MirBranchPattern, MirKind};
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
    Array(Box<Ty>),
    Struct(Vec<(std::string::String, Ty)>),
    Fn {
        param: Box<Ty>,
        ret: Box<Ty>,
    },
    TagConstructor(TagId),
    Tagged {
        tag_id: TagId,
        payload: Box<Ty>,
    },
    MethodSet {
        id: u64,
        tag_id: TagId,
    },
    /// Escape hatch for constructs we don't type yet.
    Unknown,
}

// ── Type environment ────────────────────────────────────────────

/// Registry entry for a method set: tag_id + methods struct type.
#[derive(Debug, Clone)]
pub struct MethodSetInfo {
    pub tag_id: TagId,
    pub methods: Ty, // always Ty::Struct(...)
}

pub struct TyEnv {
    bindings: Vec<(std::string::String, Ty)>,
    modules: HashMap<std::string::String, Ty>,
    next_ms_id: u64,
    /// Global registry: method set id → info (tag_id + methods struct).
    method_sets: HashMap<u64, MethodSetInfo>,
}

impl TyEnv {
    pub fn new() -> Self {
        TyEnv {
            bindings: Vec::new(),
            modules: HashMap::new(),
            next_ms_id: 0,
            method_sets: HashMap::new(),
        }
    }

    pub fn with_module(mut self, name: impl Into<std::string::String>, ty: Ty) -> Self {
        self.modules.insert(name.into(), ty);
        self
    }

    /// Create a new TyEnv inheriting the method set registry from another env.
    pub fn with_method_sets_from(mut self, other: &TyEnv) -> Self {
        self.method_sets = other.method_sets.clone();
        self.next_ms_id = other.next_ms_id;
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

    fn register_method_set(&mut self, id: u64, tag_id: TagId, methods: Ty) {
        self.method_sets.insert(id, MethodSetInfo { tag_id, methods });
    }

    /// Find a method's type by searching active method sets in scope.
    /// Scans backwards (most recent first) for shadowing semantics,
    /// mirroring `Env::find_method_in_method_sets`.
    fn find_method_type(&self, tag_id: TagId, method_name: &str) -> Option<Ty> {
        for (name, ty) in self.bindings.iter().rev() {
            if !name.starts_with("\0ms") {
                continue;
            }
            if let Ty::MethodSet { id, tag_id: ms_tag_id } = ty {
                if *ms_tag_id == tag_id {
                    if let Some(info) = self.method_sets.get(id) {
                        if let Ty::Struct(fields) = &info.methods {
                            if let Some((_, method_ty)) = fields.iter().find(|(n, _)| n == method_name) {
                                return Some(method_ty.clone());
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

/// Map a primitive Ty to its built-in TagId for method set dispatch.
fn ty_to_tag_id(ty: &Ty) -> Option<TagId> {
    match ty {
        Ty::Int => Some(TAG_ID_INT),
        Ty::Float => Some(TAG_ID_FLOAT),
        Ty::Bool => Some(TAG_ID_BOOL),
        Ty::String => Some(TAG_ID_STRING),
        Ty::Char => Some(TAG_ID_CHAR),
        Ty::Byte => Some(TAG_ID_BYTE),
        Ty::Array(_) => Some(TAG_ID_ARRAY),
        Ty::Unit => Some(TAG_ID_UNIT),
        Ty::Tagged { tag_id, .. } => Some(*tag_id),
        _ => None,
    }
}

/// Specialize a generic (Unknown) method return type based on the receiver type.
/// For example, `Array(Int).get(i)` returns `Int` (not `Unknown`).
fn specialize_method_return(recv_ty: &Ty, method: &str, ret: Ty) -> Ty {
    if ret != Ty::Unknown {
        return ret; // already specific, no need to specialize
    }
    match recv_ty {
        Ty::Array(elem) => match method {
            "get" => *elem.clone(),
            "filter" | "slice" => recv_ty.clone(),
            "add" => recv_ty.clone(), // array concatenation
            "map" | "zip" => ret, // depends on callback, keep Unknown
            _ => ret,
        },
        _ => ret,
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

    // Builtins with precise return types.
    // Params are Unknown (not checked yet); return types enable type propagation.
    let f = |ret: Ty| Ty::Fn {
        param: Box::new(Ty::Unknown),
        ret: Box::new(ret),
    };

    // Logical builtins
    fields.push(("not".into(), f(Ty::Bool)));
    fields.push(("and".into(), f(Ty::Bool)));
    fields.push(("or".into(), f(Ty::Bool)));

    // Collection builtins
    fields.push(("len".into(), f(Ty::Int)));
    fields.push(("print".into(), f(Ty::Unit)));
    fields.push(("map".into(), f(Ty::Array(Box::new(Ty::Unknown)))));
    fields.push(("filter".into(), f(Ty::Array(Box::new(Ty::Unknown)))));
    fields.push(("fold".into(), f(Ty::Unknown))); // return type depends on accumulator
    fields.push(("zip".into(), f(Ty::Array(Box::new(Ty::Unknown)))));

    // Conversion builtins
    fields.push(("byte".into(), f(Ty::Byte)));
    fields.push(("int".into(), f(Ty::Int)));
    fields.push(("float".into(), f(Ty::Float)));
    fields.push(("char".into(), f(Ty::Char)));

    // Equality builtins
    fields.push(("ref_eq".into(), f(Ty::Bool)));
    fields.push(("val_eq".into(), f(Ty::Bool)));
    fields.push(("method_set".into(), fn_ty())); // generative, handled specially

    // Array methods
    fields.push(("array_get".into(), f(Ty::Unknown))); // element type unknown
    fields.push(("array_slice".into(), f(Ty::Array(Box::new(Ty::Unknown)))));
    fields.push(("array_len".into(), f(Ty::Int)));
    fields.push(("array_map".into(), f(Ty::Array(Box::new(Ty::Unknown)))));
    fields.push(("array_filter".into(), f(Ty::Array(Box::new(Ty::Unknown)))));
    fields.push(("array_fold".into(), f(Ty::Unknown)));
    fields.push(("array_zip".into(), f(Ty::Array(Box::new(Ty::Unknown)))));
    fields.push(("array_add".into(), f(Ty::Array(Box::new(Ty::Unknown)))));
    fields.push(("array_eq".into(), f(Ty::Bool)));
    fields.push(("array_not_eq".into(), f(Ty::Bool)));

    // String methods
    fields.push(("string_byte_len".into(), f(Ty::Int)));
    fields.push(("string_char_len".into(), f(Ty::Int)));
    fields.push(("string_byte_get".into(), f(Ty::Byte)));
    fields.push(("string_char_get".into(), f(Ty::Char)));
    fields.push(("string_as_bytes".into(), f(Ty::Array(Box::new(Ty::Byte)))));
    fields.push(("string_chars".into(), f(Ty::Array(Box::new(Ty::Char)))));
    fields.push(("string_split".into(), f(Ty::Array(Box::new(Ty::String)))));
    fields.push(("string_trim".into(), f(Ty::String)));
    fields.push(("string_contains".into(), f(Ty::Bool)));
    fields.push(("string_slice".into(), f(Ty::String)));
    fields.push(("string_starts_with".into(), f(Ty::Bool)));
    fields.push(("string_ends_with".into(), f(Ty::Bool)));
    fields.push(("string_replace".into(), f(Ty::String)));
    fields.push(("string_add".into(), f(Ty::String)));
    fields.push(("string_eq".into(), f(Ty::Bool)));
    fields.push(("string_not_eq".into(), f(Ty::Bool)));
    fields.push(("string_lt".into(), f(Ty::Bool)));
    fields.push(("string_gt".into(), f(Ty::Bool)));
    fields.push(("string_lt_eq".into(), f(Ty::Bool)));
    fields.push(("string_gt_eq".into(), f(Ty::Bool)));
    fields.push(("string_to_string".into(), f(Ty::String)));

    // Int methods
    fields.push(("int_add".into(), f(Ty::Int)));
    fields.push(("int_subtract".into(), f(Ty::Int)));
    fields.push(("int_times".into(), f(Ty::Int)));
    fields.push(("int_divided_by".into(), f(Ty::Int)));
    fields.push(("int_negate".into(), f(Ty::Int)));
    fields.push(("int_eq".into(), f(Ty::Bool)));
    fields.push(("int_not_eq".into(), f(Ty::Bool)));
    fields.push(("int_lt".into(), f(Ty::Bool)));
    fields.push(("int_gt".into(), f(Ty::Bool)));
    fields.push(("int_lt_eq".into(), f(Ty::Bool)));
    fields.push(("int_gt_eq".into(), f(Ty::Bool)));
    fields.push(("int_to_string".into(), f(Ty::String)));

    // Float methods
    fields.push(("float_add".into(), f(Ty::Float)));
    fields.push(("float_subtract".into(), f(Ty::Float)));
    fields.push(("float_times".into(), f(Ty::Float)));
    fields.push(("float_divided_by".into(), f(Ty::Float)));
    fields.push(("float_negate".into(), f(Ty::Float)));
    fields.push(("float_eq".into(), f(Ty::Bool)));
    fields.push(("float_not_eq".into(), f(Ty::Bool)));
    fields.push(("float_lt".into(), f(Ty::Bool)));
    fields.push(("float_gt".into(), f(Ty::Bool)));
    fields.push(("float_lt_eq".into(), f(Ty::Bool)));
    fields.push(("float_gt_eq".into(), f(Ty::Bool)));
    fields.push(("float_to_string".into(), f(Ty::String)));

    // Bool methods
    fields.push(("bool_eq".into(), f(Ty::Bool)));
    fields.push(("bool_not_eq".into(), f(Ty::Bool)));
    fields.push(("bool_to_string".into(), f(Ty::String)));

    // Char methods
    fields.push(("char_eq".into(), f(Ty::Bool)));
    fields.push(("char_not_eq".into(), f(Ty::Bool)));
    fields.push(("char_lt".into(), f(Ty::Bool)));
    fields.push(("char_gt".into(), f(Ty::Bool)));
    fields.push(("char_lt_eq".into(), f(Ty::Bool)));
    fields.push(("char_gt_eq".into(), f(Ty::Bool)));
    fields.push(("char_to_string".into(), f(Ty::String)));

    // Byte methods
    fields.push(("byte_eq".into(), f(Ty::Bool)));
    fields.push(("byte_not_eq".into(), f(Ty::Bool)));
    fields.push(("byte_lt".into(), f(Ty::Bool)));
    fields.push(("byte_gt".into(), f(Ty::Bool)));
    fields.push(("byte_lt_eq".into(), f(Ty::Bool)));
    fields.push(("byte_gt_eq".into(), f(Ty::Bool)));
    fields.push(("byte_to_string".into(), f(Ty::String)));

    // Unit methods
    fields.push(("unit_eq".into(), f(Ty::Bool)));
    fields.push(("unit_not_eq".into(), f(Ty::Bool)));

    Ty::Struct(fields)
}

// ── Unification ─────────────────────────────────────────────────

/// Check if two types are compatible and return the unified type.
/// `Unknown` is compatible with any type (acts as a wildcard).
pub fn unify(a: &Ty, b: &Ty) -> Result<Ty, std::string::String> {
    match (a, b) {
        // Unknown unifies with anything
        (Ty::Unknown, other) | (other, Ty::Unknown) => Ok(other.clone()),

        // Primitives: must match exactly
        (Ty::Int, Ty::Int) => Ok(Ty::Int),
        (Ty::Float, Ty::Float) => Ok(Ty::Float),
        (Ty::Bool, Ty::Bool) => Ok(Ty::Bool),
        (Ty::String, Ty::String) => Ok(Ty::String),
        (Ty::Char, Ty::Char) => Ok(Ty::Char),
        (Ty::Byte, Ty::Byte) => Ok(Ty::Byte),
        (Ty::Unit, Ty::Unit) => Ok(Ty::Unit),
        (Ty::Array(e1), Ty::Array(e2)) => {
            let elem = unify(e1, e2)?;
            Ok(Ty::Array(Box::new(elem)))
        }

        // Functions: unify param and return types
        (Ty::Fn { param: p1, ret: r1 }, Ty::Fn { param: p2, ret: r2 }) => {
            let param = unify(p1, p2)?;
            let ret = unify(r1, r2)?;
            Ok(Ty::Fn {
                param: Box::new(param),
                ret: Box::new(ret),
            })
        }

        // Structs: must have same fields in same order, unify each
        (Ty::Struct(f1), Ty::Struct(f2)) => {
            if f1.len() != f2.len() {
                return Err(format!(
                    "type error: cannot unify structs with {} and {} fields",
                    f1.len(),
                    f2.len()
                ));
            }
            let mut unified = Vec::with_capacity(f1.len());
            for ((n1, t1), (n2, t2)) in f1.iter().zip(f2.iter()) {
                if n1 != n2 {
                    return Err(format!(
                        "type error: cannot unify struct fields '{}' and '{}'",
                        n1, n2
                    ));
                }
                let t = unify(t1, t2)?;
                unified.push((n1.clone(), t));
            }
            Ok(Ty::Struct(unified))
        }

        // TagConstructor: must have same tag ID
        (Ty::TagConstructor(id1), Ty::TagConstructor(id2)) => {
            if id1 == id2 {
                Ok(Ty::TagConstructor(*id1))
            } else {
                Err("type error: cannot unify different type constructors".to_string())
            }
        }

        // Tagged: same tag ID → unify payloads; different tags → Unknown (sum type)
        (Ty::Tagged { tag_id: id1, payload: p1 }, Ty::Tagged { tag_id: id2, payload: p2 }) => {
            if id1 == id2 {
                let payload = unify(p1, p2)?;
                Ok(Ty::Tagged { tag_id: *id1, payload: Box::new(payload) })
            } else {
                // Different tags in a branch — this is a sum type (Ok | Err, etc.)
                // We don't have union types yet, so fall back to Unknown.
                Ok(Ty::Unknown)
            }
        }

        // Tagged with non-tagged — can't unify structurally, but in branches
        // a tagged and non-tagged arm indicate a heterogeneous result.
        // Fall back to Unknown rather than erroring.
        (Ty::Tagged { .. }, _) | (_, Ty::Tagged { .. }) => Ok(Ty::Unknown),

        // MethodSet: must have same generative ID (same lexical site)
        (Ty::MethodSet { id: id1, tag_id: t1 }, Ty::MethodSet { id: id2, tag_id: t2 }) => {
            if id1 == id2 && t1 == t2 {
                Ok(Ty::MethodSet { id: *id1, tag_id: *t1 })
            } else {
                Err("type error: cannot unify different method set types".to_string())
            }
        }

        // Everything else: mismatch
        _ => Err(format!(
            "type error: cannot unify {:?} with {:?}",
            a, b
        )),
    }
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
                .ok_or_else(|| format!("type error: undefined variable: {}", name))
        }

        // ── Import ──
        MirKind::Import(name) => {
            env.modules
                .get(name)
                .cloned()
                .ok_or_else(|| format!("type error: module not provided: {}", name))
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
                        .ok_or_else(|| format!("type error: field '{}' not found in struct", field))
                }
                Ty::Unknown => Ok(Ty::Unknown),
                _ => Err(format!("type error: field access on non-struct value")),
            }
        }

        // ── Struct literal ──
        MirKind::Struct(fields) => {
            let mut typed_fields = Vec::new();
            let mut positional_idx = 0u64;
            for field in fields {
                let ty = check(&field.value, env)?;
                if field.is_spread {
                    // Spread: merge fields from the spread value
                    match &ty {
                        Ty::Struct(spread_fields) => {
                            for (name, fty) in spread_fields {
                                if name.parse::<u64>().is_ok() {
                                    // Re-index positional fields
                                    typed_fields.push((positional_idx.to_string(), fty.clone()));
                                    positional_idx += 1;
                                } else {
                                    typed_fields.push((name.clone(), fty.clone()));
                                }
                            }
                        }
                        Ty::Unit => {} // spreading unit is a no-op
                        _ => {
                            // Unknown or non-struct spread — can't track fields
                            typed_fields.push((positional_idx.to_string(), ty));
                            positional_idx += 1;
                        }
                    }
                } else {
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
                Ty::MethodSet { .. } => {
                    env.bind("\0ms".to_string(), ms_ty);
                    let result = check(body, env);
                    env.pop_binding();
                    result
                }
                Ty::Unknown => check(body, env),
                _ => Err("type error: apply expects a method set".to_string()),
            }
        }

        // ── NewTag ──
        MirKind::NewTag(id, _name) => Ok(Ty::TagConstructor(*id)),

        // ── Block (lambda) ──
        MirKind::Block(body) => {
            env.bind("in".to_string(), Ty::Unknown);
            let body_ty = check(body, env)?;
            env.pop_binding();
            Ok(Ty::Fn {
                param: Box::new(Ty::Unknown),
                ret: Box::new(body_ty),
            })
        }

        // ── BranchBlock (pattern-matching lambda) ──
        // When not called via check_call, input type is Unknown.
        MirKind::BranchBlock(arms) => {
            let input_ty = Ty::Unknown;
            env.bind("in".to_string(), input_ty.clone());
            let mut result_ty: Option<Ty> = None;
            for arm in arms {
                let bindings_added = bind_branch_pattern(&arm.pattern, &input_ty, env);
                if let Some(guard) = &arm.guard {
                    let _ = check(guard, env)?;
                }
                let arm_ty = check(&arm.body, env)?;
                for _ in 0..bindings_added {
                    env.pop_binding();
                }
                result_ty = Some(match result_ty {
                    None => arm_ty,
                    Some(prev) => unify(&prev, &arm_ty)?,
                });
            }
            env.pop_binding(); // pop "in"
            let ret = result_ty.unwrap_or(Ty::Unknown);
            Ok(Ty::Fn {
                param: Box::new(Ty::Unknown),
                ret: Box::new(ret),
            })
        }

        // ── Array ──
        MirKind::Array(elems) => {
            let mut elem_ty = Ty::Unknown;
            for elem in elems {
                let ty = check(elem, env)?;
                elem_ty = unify(&elem_ty, &ty)?;
            }
            Ok(Ty::Array(Box::new(elem_ty)))
        }

        // ── LetArray (standalone, not via pipe) ──
        MirKind::LetArray { patterns, body } => {
            check_let_array(patterns, body, &Ty::Unknown, env)
        }

        // ── MethodCall ──
        MirKind::MethodCall { receiver, method, arg } => {
            let recv_ty = check(receiver, env)?;
            let _arg_ty = check(arg, env)?;

            // Stage 1: struct field access (method stored as field)
            if let Ty::Struct(fields) = &recv_ty {
                if let Some((_, field_ty)) = fields.iter().find(|(n, _)| n == method) {
                    return match field_ty {
                        Ty::Fn { ret, .. } => Ok(*ret.clone()),
                        Ty::Unknown => Ok(Ty::Unknown),
                        _ => Ok(Ty::Unknown),
                    };
                }
            }

            // Stage 2: method set lookup
            if let Some(tag_id) = ty_to_tag_id(&recv_ty) {
                if let Some(method_ty) = env.find_method_type(tag_id, method) {
                    let ret = match method_ty {
                        Ty::Fn { ret, .. } => *ret,
                        _ => Ty::Unknown,
                    };
                    // Specialize generic return types based on receiver
                    let ret = specialize_method_return(&recv_ty, method, ret);
                    return Ok(ret);
                }
            }

            // Stage 3: fallback methods for types without explicit method sets
            if matches!(method.as_str(), "eq" | "not_eq" | "lt" | "gt" | "lt_eq" | "gt_eq") {
                return Ok(Ty::Bool);
            }
            if method == "to_string" {
                return Ok(Ty::String);
            }

            // Unknown receiver or method not found — don't error, return Unknown
            Ok(Ty::Unknown)
        }
    }
}

fn check_pipe(lhs: &Mir, rhs: &Mir, env: &mut TyEnv) -> Result<Ty, std::string::String> {
    let lhs_ty = check(lhs, env)?;
    match rhs.as_ref() {
        // Pipe into let: `expr >> let(pattern); body`
        // The lhs value is bound to the pattern.
        // Also bind \0 (passthrough variable) like eval_pipe does.
        MirKind::Let { pattern, body } => {
            check_let_with_passthrough(pattern, body, &lhs_ty, env)
        }
        // Pipe into let array: `expr >> let[a, b, c]; body`
        MirKind::LetArray { patterns, body } => {
            check_let_array_with_passthrough(patterns, body, &lhs_ty, env)
        }
        // Pipe into apply: `expr >> apply(ms); body`
        MirKind::Apply { expr, body } => {
            let ms_ty = check(expr, env)?;
            match &ms_ty {
                Ty::MethodSet { .. } => {
                    env.bind("\0ms".to_string(), ms_ty);
                    let result = check(body, env);
                    env.pop_binding();
                    result
                }
                Ty::Unknown => check(body, env),
                _ => Err("type error: apply expects a method set".to_string()),
            }
        }
        // All other pipe RHS patterns are lowered to Call/MethodCall by MIR.
        // This branch should be unreachable.
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
        Pattern::Fields(fields) => {
            let mut bindings_added = 0usize;
            let mut positional_idx = 0u64;
            for field in fields {
                if field.binding == "_" && !field.is_rest {
                    // Discard — don't bind, but consume positional index
                    if field.label.is_none() {
                        positional_idx += 1;
                    }
                    continue;
                }
                if field.is_rest {
                    // Rest pattern — compute remaining fields from the input struct
                    if field.binding != "_" && !field.binding.is_empty() {
                        let rest_ty = match input_ty {
                            Ty::Struct(struct_fields) => {
                                // Collect field keys consumed by non-rest patterns
                                let mut consumed = Vec::new();
                                let mut pos = 0u64;
                                for f in fields {
                                    if f.is_rest { continue; }
                                    match &f.label {
                                        Some(label) => consumed.push(label.clone()),
                                        None => {
                                            consumed.push(pos.to_string());
                                            pos += 1;
                                        }
                                    }
                                }
                                let remaining: Vec<(std::string::String, Ty)> = struct_fields
                                    .iter()
                                    .filter(|(n, _)| !consumed.contains(n))
                                    .cloned()
                                    .collect();
                                if remaining.is_empty() {
                                    Ty::Unit
                                } else {
                                    // Re-index positional fields starting from 0
                                    let mut re_pos = 0u64;
                                    let remaining = remaining.into_iter().map(|(n, ty)| {
                                        if n.parse::<u64>().is_ok() {
                                            let new_n = re_pos.to_string();
                                            re_pos += 1;
                                            (new_n, ty)
                                        } else {
                                            (n, ty)
                                        }
                                    }).collect();
                                    Ty::Struct(remaining)
                                }
                            }
                            _ => Ty::Unknown,
                        };
                        env.bind(field.binding.clone(), rest_ty);
                        bindings_added += 1;
                    }
                    continue;
                }
                // Look up field type from input struct if known
                let field_ty = match input_ty {
                    Ty::Struct(struct_fields) => {
                        let lookup_key = match &field.label {
                            Some(label) => label.clone(),
                            None => {
                                let key = positional_idx.to_string();
                                positional_idx += 1;
                                key
                            }
                        };
                        struct_fields
                            .iter()
                            .find(|(n, _)| *n == lookup_key)
                            .map(|(_, ty)| ty.clone())
                            .unwrap_or(Ty::Unknown)
                    }
                    _ => {
                        if field.label.is_none() {
                            positional_idx += 1;
                        }
                        Ty::Unknown
                    }
                };
                env.bind(field.binding.clone(), field_ty);
                bindings_added += 1;
            }
            let body_ty = check(body, env)?;
            for _ in 0..bindings_added {
                env.pop_binding();
            }
            Ok(body_ty)
        }
    }
}

fn check_let_array(
    patterns: &[ArrayPat],
    body: &Mir,
    input_ty: &Ty,
    env: &mut TyEnv,
) -> Result<Ty, std::string::String> {
    // Extract element type from Array input
    let elem_ty = match input_ty {
        Ty::Array(elem) => elem.as_ref().clone(),
        _ => Ty::Unknown,
    };
    let mut bindings_added = 0usize;
    for pat in patterns {
        match pat {
            ArrayPat::Name(name) => {
                env.bind(name.clone(), elem_ty.clone());
                bindings_added += 1;
            }
            ArrayPat::Discard => {}
            ArrayPat::Rest(Some(name)) => {
                env.bind(name.clone(), Ty::Array(Box::new(elem_ty.clone())));
                bindings_added += 1;
            }
            ArrayPat::Rest(None) => {}
        }
    }
    let body_ty = check(body, env)?;
    for _ in 0..bindings_added {
        env.pop_binding();
    }
    Ok(body_ty)
}

/// Like check_let but also binds the \0 passthrough variable (used by pipe >> let).
/// Also mirrors `apply_prelude` from eval: if the input has a `prelude` field
/// containing method sets, auto-apply them (handles `use(std)` pattern).
fn check_let_with_passthrough(
    pattern: &Pattern,
    body: &Mir,
    input_ty: &Ty,
    env: &mut TyEnv,
) -> Result<Ty, std::string::String> {
    // Auto-apply prelude method sets from the input value (mirrors eval::apply_prelude)
    let mut prelude_count = 0usize;
    if let Ty::Struct(fields) = input_ty {
        for (label, ty) in fields {
            if label == "prelude" {
                if let Ty::Struct(prelude_fields) = ty {
                    for (_, ms_ty) in prelude_fields {
                        if matches!(ms_ty, Ty::MethodSet { .. }) {
                            env.bind("\0ms".to_string(), ms_ty.clone());
                            prelude_count += 1;
                        }
                    }
                }
            }
        }
    }
    // Bind \0 passthrough variable like eval_pipe does
    env.bind("\0".to_string(), input_ty.clone());
    let result = check_let(pattern, body, input_ty, env);
    env.pop_binding(); // pop \0
    for _ in 0..prelude_count {
        env.pop_binding(); // pop prelude method sets
    }
    result
}

/// Like check_let_array but also binds the \0 passthrough variable.
fn check_let_array_with_passthrough(
    patterns: &[ArrayPat],
    body: &Mir,
    input_ty: &Ty,
    env: &mut TyEnv,
) -> Result<Ty, std::string::String> {
    env.bind("\0".to_string(), input_ty.clone());
    let result = check_let_array(patterns, body, input_ty, env);
    env.pop_binding(); // pop \0
    result
}

fn check_call(func: &Mir, arg: &Mir, env: &mut TyEnv) -> Result<Ty, std::string::String> {
    // Special case: method_set call
    if let MirKind::Ident(name) = func.as_ref() {
        if name == "method_set" {
            return check_method_set_call(arg, env);
        }
    }

    // Bidirectional: when calling a Block or BranchBlock, check the arg first
    // so we can bind `in` to the arg's type instead of Unknown.
    match func.as_ref() {
        MirKind::Block(body) => {
            let arg_ty = check(arg, env)?;
            env.bind("in".to_string(), arg_ty);
            let body_ty = check(body, env)?;
            env.pop_binding();
            Ok(body_ty)
        }
        MirKind::BranchBlock(arms) => {
            let arg_ty = check(arg, env)?;
            check_branch_block_with_input(arms, &arg_ty, env)
        }
        _ => {
            let func_ty = check(func, env)?;
            let arg_ty = check(arg, env)?;
            match func_ty {
                Ty::Fn { ret, .. } => Ok(*ret),
                Ty::TagConstructor(tag_id) => Ok(Ty::Tagged {
                    tag_id,
                    payload: Box::new(arg_ty),
                }),
                Ty::Unknown => Ok(Ty::Unknown),
                _ => Err(format!("type error: cannot call non-function")),
            }
        }
    }
}

/// Check a BranchBlock with a known input type (from bidirectional Call inference).
/// Binds `in` to the input type and unifies all arm body types.
fn check_branch_block_with_input(
    arms: &[crate::mir::MirBranchArm],
    input_ty: &Ty,
    env: &mut TyEnv,
) -> Result<Ty, std::string::String> {
    env.bind("in".to_string(), input_ty.clone());
    let mut result_ty: Option<Ty> = None;
    for arm in arms {
        let bindings_added = bind_branch_pattern(&arm.pattern, input_ty, env);
        if let Some(guard) = &arm.guard {
            let _ = check(guard, env)?;
        }
        let arm_ty = check(&arm.body, env)?;
        for _ in 0..bindings_added {
            env.pop_binding();
        }
        result_ty = Some(match result_ty {
            None => arm_ty,
            Some(prev) => unify(&prev, &arm_ty)?,
        });
    }
    env.pop_binding(); // pop "in"
    Ok(result_ty.unwrap_or(Ty::Unknown))
}

/// Bind variables introduced by a branch pattern. Returns the number of bindings added.
fn bind_branch_pattern(pattern: &MirBranchPattern, input_ty: &Ty, env: &mut TyEnv) -> usize {
    match pattern {
        MirBranchPattern::Literal(_) => 0,
        MirBranchPattern::Tag(_, binding) => match binding {
            Some(BranchBinding::Name(n)) => {
                // Tag payload type is Unknown — the input is a sum type and we
                // can't resolve tag names to IDs to extract the right payload.
                env.bind(n.clone(), Ty::Unknown);
                1
            }
            _ => 0,
        },
        MirBranchPattern::Binding(n) => {
            // Catch-all binding gets the input type
            env.bind(n.clone(), input_ty.clone());
            1
        }
        MirBranchPattern::Discard => 0,
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
            env.register_method_set(id, tag_id, methods_ty.clone());
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

    // ── Unification tests ──

    #[test]
    fn unify_same_primitives() {
        assert_eq!(unify(&Ty::Int, &Ty::Int).unwrap(), Ty::Int);
        assert_eq!(unify(&Ty::Float, &Ty::Float).unwrap(), Ty::Float);
        assert_eq!(unify(&Ty::Bool, &Ty::Bool).unwrap(), Ty::Bool);
        assert_eq!(unify(&Ty::String, &Ty::String).unwrap(), Ty::String);
    }

    #[test]
    fn unify_different_primitives_error() {
        assert!(unify(&Ty::Int, &Ty::Float).is_err());
        assert!(unify(&Ty::Int, &Ty::String).is_err());
        assert!(unify(&Ty::Bool, &Ty::Byte).is_err());
    }

    #[test]
    fn unify_unknown_with_anything() {
        assert_eq!(unify(&Ty::Unknown, &Ty::Int).unwrap(), Ty::Int);
        assert_eq!(unify(&Ty::Float, &Ty::Unknown).unwrap(), Ty::Float);
        assert_eq!(unify(&Ty::Unknown, &Ty::Unknown).unwrap(), Ty::Unknown);
    }

    #[test]
    fn unify_structs_same_shape() {
        let a = Ty::Struct(vec![("x".into(), Ty::Int), ("y".into(), Ty::Float)]);
        let b = Ty::Struct(vec![("x".into(), Ty::Int), ("y".into(), Ty::Float)]);
        assert_eq!(unify(&a, &b).unwrap(), a);
    }

    #[test]
    fn unify_structs_different_field_count() {
        let a = Ty::Struct(vec![("x".into(), Ty::Int)]);
        let b = Ty::Struct(vec![("x".into(), Ty::Int), ("y".into(), Ty::Float)]);
        assert!(unify(&a, &b).is_err());
    }

    #[test]
    fn unify_structs_different_field_names() {
        let a = Ty::Struct(vec![("x".into(), Ty::Int)]);
        let b = Ty::Struct(vec![("y".into(), Ty::Int)]);
        assert!(unify(&a, &b).is_err());
    }

    #[test]
    fn unify_structs_different_field_types() {
        let a = Ty::Struct(vec![("x".into(), Ty::Int)]);
        let b = Ty::Struct(vec![("x".into(), Ty::Float)]);
        assert!(unify(&a, &b).is_err());
    }

    #[test]
    fn unify_method_sets_same_id() {
        let a = Ty::MethodSet { id: 0, tag_id: TAG_ID_INT };
        let b = Ty::MethodSet { id: 0, tag_id: TAG_ID_INT };
        assert_eq!(unify(&a, &b).unwrap(), a);
    }

    #[test]
    fn unify_method_sets_different_id() {
        let a = Ty::MethodSet { id: 0, tag_id: TAG_ID_INT };
        let b = Ty::MethodSet { id: 1, tag_id: TAG_ID_INT };
        assert!(unify(&a, &b).is_err());
    }

    #[test]
    fn unify_functions() {
        let a = Ty::Fn { param: Box::new(Ty::Int), ret: Box::new(Ty::Bool) };
        let b = Ty::Fn { param: Box::new(Ty::Int), ret: Box::new(Ty::Bool) };
        assert_eq!(unify(&a, &b).unwrap(), a);
    }

    #[test]
    fn unify_functions_unknown_fills() {
        let a = Ty::Fn { param: Box::new(Ty::Unknown), ret: Box::new(Ty::Unknown) };
        let b = Ty::Fn { param: Box::new(Ty::Int), ret: Box::new(Ty::Bool) };
        assert_eq!(unify(&a, &b).unwrap(), b);
    }

    // ── Block and branch tests ──

    #[test]
    fn block_types_as_fn() {
        let mut env = TyEnv::new();
        // { 42 } is a lambda that returns Int
        let expr = mir(MirKind::Block(mir(MirKind::Int(42))));
        let ty = check(&expr, &mut env).unwrap();
        assert_eq!(
            ty,
            Ty::Fn { param: Box::new(Ty::Unknown), ret: Box::new(Ty::Int) }
        );
    }

    #[test]
    fn branch_same_type_arms() {
        use crate::mir::{MirBranchArm, MirBranchPattern};
        let mut env = TyEnv::new();
        // { true -> 1, false -> 2 }
        let expr = mir(MirKind::BranchBlock(vec![
            MirBranchArm {
                pattern: MirBranchPattern::Literal(mir(MirKind::Bool(true))),
                guard: None,
                body: mir(MirKind::Int(1)),
            },
            MirBranchArm {
                pattern: MirBranchPattern::Literal(mir(MirKind::Bool(false))),
                guard: None,
                body: mir(MirKind::Int(2)),
            },
        ]));
        let ty = check(&expr, &mut env).unwrap();
        assert_eq!(
            ty,
            Ty::Fn { param: Box::new(Ty::Unknown), ret: Box::new(Ty::Int) }
        );
    }

    #[test]
    fn branch_different_type_arms_error() {
        use crate::mir::{MirBranchArm, MirBranchPattern};
        let mut env = TyEnv::new();
        // { true -> 1, false -> "hello" }
        let expr = mir(MirKind::BranchBlock(vec![
            MirBranchArm {
                pattern: MirBranchPattern::Literal(mir(MirKind::Bool(true))),
                guard: None,
                body: mir(MirKind::Int(1)),
            },
            MirBranchArm {
                pattern: MirBranchPattern::Literal(mir(MirKind::Bool(false))),
                guard: None,
                body: mir(MirKind::Str("hello".into())),
            },
        ]));
        assert!(check(&expr, &mut env).is_err());
    }

    #[test]
    fn branch_with_binding_pattern() {
        use crate::mir::{MirBranchArm, MirBranchPattern};
        let mut env = TyEnv::new();
        // { x -> x } — catch-all returns Unknown (input type unknown)
        let expr = mir(MirKind::BranchBlock(vec![
            MirBranchArm {
                pattern: MirBranchPattern::Binding("x".into()),
                guard: None,
                body: mir(MirKind::Ident("x".into())),
            },
        ]));
        let ty = check(&expr, &mut env).unwrap();
        assert_eq!(
            ty,
            Ty::Fn { param: Box::new(Ty::Unknown), ret: Box::new(Ty::Unknown) }
        );
    }

    // ── Q5: Two lexical method_set calls in branches → error ──

    #[test]
    fn q5_two_method_set_calls_in_branch_error() {
        use crate::mir::{MirBranchArm, MirBranchPattern};
        let mut env = TyEnv::new();
        env.bind("Int".into(), Ty::TagConstructor(TAG_ID_INT));
        let fn_ty = Ty::Fn {
            param: Box::new(Ty::Unknown),
            ret: Box::new(Ty::Unknown),
        };
        env.bind("int_to_string".into(), fn_ty.clone());
        env.bind("int_to_string_other".into(), fn_ty);
        env.bind(
            "method_set".into(),
            Ty::Fn { param: Box::new(Ty::Unknown), ret: Box::new(Ty::Unknown) },
        );

        // Helper to build method_set(Int, (to_string = f))
        let make_ms = |f: &str| {
            mir(MirKind::Call(
                mir(MirKind::Ident("method_set".into())),
                mir(MirKind::Struct(vec![
                    MirField { label: None, value: mir(MirKind::Ident("Int".into())), is_spread: false },
                    MirField {
                        label: None,
                        value: mir(MirKind::Struct(vec![MirField {
                            label: Some("to_string".into()),
                            value: mir(MirKind::Ident(f.into())),
                            is_spread: false,
                        }])),
                        is_spread: false,
                    },
                ])),
            ))
        };

        // { true -> method_set(Int, ...), false -> method_set(Int, ...) }
        let branch = mir(MirKind::BranchBlock(vec![
            MirBranchArm {
                pattern: MirBranchPattern::Literal(mir(MirKind::Bool(true))),
                guard: None,
                body: make_ms("int_to_string"),
            },
            MirBranchArm {
                pattern: MirBranchPattern::Literal(mir(MirKind::Bool(false))),
                guard: None,
                body: make_ms("int_to_string_other"),
            },
        ]));

        // The two method_set calls produce different generative IDs → unification fails
        let result = check(&branch, &mut env);
        assert!(result.is_err(), "expected error from two different method_set types in branch");
    }

    // ── Q4: One method_set with varying struct arg → OK ──

    #[test]
    fn q4_one_method_set_with_varying_struct_ok() {
        let mut env = TyEnv::new();
        let fn_ty = Ty::Fn { param: Box::new(Ty::Unknown), ret: Box::new(Ty::Unknown) };
        env.bind("Int".into(), Ty::TagConstructor(TAG_ID_INT));
        env.bind("int_to_string".into(), fn_ty.clone());
        env.bind("int_to_string_other".into(), fn_ty.clone());
        env.bind(
            "method_set".into(),
            Ty::Fn { param: Box::new(Ty::Unknown), ret: Box::new(Ty::Unknown) },
        );

        // let int_ms1 = (to_string = int_to_string)
        // let int_ms2 = (to_string = int_to_string_other)
        // method_set(Int, in >> { int_ms1 | int_ms2 })
        //
        // The branch { int_ms1 | int_ms2 } unifies because both are
        // Struct([(to_string, Fn(Unknown->Unknown))]) — same shape, same types.
        // Then method_set sees the branch result (a function returning a struct) and accepts.

        // Build: { true -> int_ms1, false -> int_ms2 }
        use crate::mir::{MirBranchArm, MirBranchPattern};
        let branch = mir(MirKind::BranchBlock(vec![
            MirBranchArm {
                pattern: MirBranchPattern::Literal(mir(MirKind::Bool(true))),
                guard: None,
                body: mir(MirKind::Struct(vec![MirField {
                    label: Some("to_string".into()),
                    value: mir(MirKind::Ident("int_to_string".into())),
                    is_spread: false,
                }])),
            },
            MirBranchArm {
                pattern: MirBranchPattern::Literal(mir(MirKind::Bool(false))),
                guard: None,
                body: mir(MirKind::Struct(vec![MirField {
                    label: Some("to_string".into()),
                    value: mir(MirKind::Ident("int_to_string_other".into())),
                    is_spread: false,
                }])),
            },
        ]));

        // The branch should typecheck fine — both arms are same-shaped structs
        let branch_ty = check(&branch, &mut env).unwrap();
        match &branch_ty {
            Ty::Fn { ret, .. } => {
                assert!(matches!(ret.as_ref(), Ty::Struct(_)));
            }
            other => panic!("expected Fn, got {:?}", other),
        }
    }

    // ── MethodCall resolution tests ──

    #[test]
    fn method_call_resolves_via_method_set() {
        let mut env = TyEnv::new();
        // Set up: method_set(Int, (add = fn(Unknown->Int)))
        env.bind("Int".into(), Ty::TagConstructor(TAG_ID_INT));
        let int_add_ty = Ty::Fn {
            param: Box::new(Ty::Unknown),
            ret: Box::new(Ty::Int),
        };
        env.bind("int_add".into(), int_add_ty);
        env.bind(
            "method_set".into(),
            Ty::Fn { param: Box::new(Ty::Unknown), ret: Box::new(Ty::Unknown) },
        );

        // Create the method set: method_set(Int, (add = int_add))
        let ms_call = mir(MirKind::Call(
            mir(MirKind::Ident("method_set".into())),
            mir(MirKind::Struct(vec![
                MirField { label: None, value: mir(MirKind::Ident("Int".into())), is_spread: false },
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
        let ms_ty = check(&ms_call, &mut env).unwrap();
        assert!(matches!(ms_ty, Ty::MethodSet { tag_id, .. } if tag_id == TAG_ID_INT));

        // Apply the method set and call 1.add(2)
        let expr = mir(MirKind::Apply {
            expr: ms_call.clone(),
            body: mir(MirKind::MethodCall {
                receiver: mir(MirKind::Int(1)),
                method: "add".into(),
                arg: mir(MirKind::Int(2)),
            }),
        });
        // Note: ms_call is checked again inside Apply — this allocates a second ms id,
        // but the Apply handler binds it. We need a fresh env for this test.
        let mut env2 = TyEnv::new();
        env2.bind("Int".into(), Ty::TagConstructor(TAG_ID_INT));
        env2.bind("int_add".into(), Ty::Fn {
            param: Box::new(Ty::Unknown),
            ret: Box::new(Ty::Int),
        });
        env2.bind(
            "method_set".into(),
            Ty::Fn { param: Box::new(Ty::Unknown), ret: Box::new(Ty::Unknown) },
        );
        let ty = check(&expr, &mut env2).unwrap();
        assert_eq!(ty, Ty::Int);
    }

    #[test]
    fn method_call_on_struct_field() {
        let mut env = TyEnv::new();
        // A struct with a method-like field: (add = fn(Unknown->Int))
        let struct_ty = Ty::Struct(vec![(
            "add".into(),
            Ty::Fn { param: Box::new(Ty::Unknown), ret: Box::new(Ty::Int) },
        )]);
        env.bind("s".into(), struct_ty);
        // s.add(1)
        let expr = mir(MirKind::MethodCall {
            receiver: mir(MirKind::Ident("s".into())),
            method: "add".into(),
            arg: mir(MirKind::Int(1)),
        });
        let ty = check(&expr, &mut env).unwrap();
        assert_eq!(ty, Ty::Int);
    }

    #[test]
    fn struct_rest_pattern_types() {
        use crate::ast::{PatField, Pattern};
        let mut env = TyEnv::new();
        // (x=1, y=2.0, z=true) >> let(x=x, ...rest); rest
        // rest should be Struct([(y, Float), (z, Bool)])
        let input = Ty::Struct(vec![
            ("x".into(), Ty::Int),
            ("y".into(), Ty::Float),
            ("z".into(), Ty::Bool),
        ]);
        let pattern = Pattern::Fields(vec![
            PatField { label: Some("x".into()), binding: "x".into(), is_rest: false },
            PatField { label: None, binding: "rest".into(), is_rest: true },
        ]);
        let body = mir(MirKind::Ident("rest".into()));
        let ty = check_let(&pattern, &body, &input, &mut env).unwrap();
        assert_eq!(
            ty,
            Ty::Struct(vec![("y".into(), Ty::Float), ("z".into(), Ty::Bool)])
        );
    }

    #[test]
    fn method_call_to_string_fallback() {
        let mut env = TyEnv::new();
        // x.to_string() where x is Unknown — should still return String
        let expr = mir(MirKind::MethodCall {
            receiver: mir(MirKind::Int(42)),
            method: "to_string".into(),
            arg: mir(MirKind::Unit),
        });
        let ty = check(&expr, &mut env).unwrap();
        assert_eq!(ty, Ty::String);
    }

    #[test]
    fn array_get_returns_element_type() {
        // [1, 2, 3].get(0) should return Int, not Unknown
        let mut env = TyEnv::new();
        // Set up array method set
        env.bind("Array".into(), Ty::TagConstructor(TAG_ID_ARRAY));
        let get_ty = Ty::Fn {
            param: Box::new(Ty::Unknown),
            ret: Box::new(Ty::Unknown), // generic return
        };
        env.bind(
            "method_set".into(),
            Ty::Fn { param: Box::new(Ty::Unknown), ret: Box::new(Ty::Unknown) },
        );
        // Register a method set for Array with get method
        let ms_id = env.fresh_method_set_id();
        env.register_method_set(
            ms_id,
            TAG_ID_ARRAY,
            Ty::Struct(vec![("get".into(), get_ty)]),
        );
        env.bind("\0ms".to_string(), Ty::MethodSet { id: ms_id, tag_id: TAG_ID_ARRAY });

        let expr = mir(MirKind::MethodCall {
            receiver: mir(MirKind::Array(vec![
                mir(MirKind::Int(1)),
                mir(MirKind::Int(2)),
            ])),
            method: "get".into(),
            arg: mir(MirKind::Int(0)),
        });
        let ty = check(&expr, &mut env).unwrap();
        assert_eq!(ty, Ty::Int);
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
