//! Type checker for nana.
//!
//! A forward type checker operating on the MIR with unification and bidirectional
//! inference. Validates all expressions: literals, bindings, calls, method dispatch,
//! branch arm consistency, struct/array construction, destructuring, and method set
//! generativity. Numeric literals create constrained inference variables that
//! resolve to concrete types (Int/Byte for int literals, Float for float literals)
//! via unification.

use std::collections::HashMap;

use crate::ast::{ArrayPat, BranchBinding, Pattern};
use crate::mir::{Mir, MirBranchPattern, MirKind};
use crate::value::{
    TAG_ID_ARRAY, TAG_ID_BOOL, TAG_ID_CHAR, TAG_ID_F32, TAG_ID_F64,
    TAG_ID_I128, TAG_ID_I16, TAG_ID_I32, TAG_ID_I64, TAG_ID_I8, TAG_ID_STRING, TAG_ID_U128,
    TAG_ID_U16, TAG_ID_U32, TAG_ID_U64, TAG_ID_U8, TAG_ID_UNIT, TagId,
};

// ── Types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    I64,
    F64,
    Bool,
    String,
    Char,
    U8,
    I8,
    I16,
    U16,
    I32,
    U32,
    F32,
    U64,
    I128,
    U128,
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
    /// The builtin `method_set` constructor function. Tracked as a distinct
    /// type so it can be intercepted at call sites regardless of the name
    /// it's bound to (e.g. `let ms = std.method_set; ms(Int, ...)`).
    MethodSetConstructor,
    /// Sum type: one of several possible types (e.g. `Ok(Int) | Err(String)`).
    /// Variants are always flattened (no nested unions).
    Union(Vec<Ty>),
    /// Inference variable: type not yet determined. Each use site gets a
    /// unique ID via `TyEnv::fresh_infer()`. Unifies with any concrete type,
    /// resolving to that type. Not a type in the language — only exists
    /// during type checking for deferred inference (standalone blocks,
    /// empty arrays, pattern fallbacks).
    Infer(u64),
    /// Generic type variable, instantiated per call site.
    /// Used in parametric signatures (e.g. array methods, ref_eq/val_eq).
    /// `Generic(0)` = first type variable T, `Generic(1)` = second U.
    Generic(u64),
}

/// Constraint on an inference variable, restricting which concrete types it can resolve to.
/// Numeric literals create constrained Infer variables: int literals can become Int or Byte,
/// float literals can become Float.
#[derive(Debug, Clone, PartialEq)]
enum InferConstraint {
    /// Can resolve to Int or Byte. Defaults to Int if unconstrained at end.
    IntLiteral,
    /// Can resolve to Float only. Defaults to Float if unconstrained at end.
    FloatLiteral,
}

impl Ty {
    /// Returns true if this type contains any unresolved inference variables.
    /// An inference variable in the final result type indicates an inference failure.
    pub fn contains_infer(&self) -> bool {
        match self {
            Ty::Infer(_) => true,
            Ty::Array(elem) => elem.contains_infer(),
            Ty::Fn { param, ret } => param.contains_infer() || ret.contains_infer(),
            Ty::Struct(fields) => fields.iter().any(|(_, ty)| ty.contains_infer()),
            Ty::Tagged { payload, .. } => payload.contains_infer(),
            Ty::Union(variants) => variants.iter().any(|ty| ty.contains_infer()),
            _ => false,
        }
    }

    /// Default unresolved `Infer` variables inside arrays to `Unit`.
    /// Empty arrays have unknowable element types — this is benign since
    /// no elements exist. Other Infer positions (closures, structs) are
    /// left as-is so the `contains_infer` check can reject them.
    pub fn default_infer_in_arrays(&self) -> Ty {
        match self {
            Ty::Array(elem) => Ty::Array(Box::new(elem.default_infer_recursive())),
            Ty::Fn { param, ret } => Ty::Fn {
                param: Box::new(param.default_infer_in_arrays()),
                ret: Box::new(ret.default_infer_in_arrays()),
            },
            Ty::Struct(fields) => Ty::Struct(
                fields.iter().map(|(n, t)| (n.clone(), t.default_infer_in_arrays())).collect(),
            ),
            Ty::Tagged { tag_id, payload } => Ty::Tagged {
                tag_id: *tag_id,
                payload: Box::new(payload.default_infer_in_arrays()),
            },
            Ty::Union(variants) => Ty::Union(
                variants.iter().map(|t| t.default_infer_in_arrays()).collect(),
            ),
            other => other.clone(),
        }
    }

    /// Replace all `Infer` with `Unit` recursively. Used inside arrays
    /// where unconstrained element types are acceptable.
    fn default_infer_recursive(&self) -> Ty {
        match self {
            Ty::Infer(_) => Ty::Unit,
            Ty::Array(elem) => Ty::Array(Box::new(elem.default_infer_recursive())),
            Ty::Fn { param, ret } => Ty::Fn {
                param: Box::new(param.default_infer_recursive()),
                ret: Box::new(ret.default_infer_recursive()),
            },
            Ty::Struct(fields) => Ty::Struct(
                fields.iter().map(|(n, t)| (n.clone(), t.default_infer_recursive())).collect(),
            ),
            Ty::Tagged { tag_id, payload } => Ty::Tagged {
                tag_id: *tag_id,
                payload: Box::new(payload.default_infer_recursive()),
            },
            Ty::Union(variants) => Ty::Union(
                variants.iter().map(|t| t.default_infer_recursive()).collect(),
            ),
            other => other.clone(),
        }
    }
}

/// Infer a Ty from a runtime Value.
/// Used by the REPL to persist type bindings across lines.
/// Needs `env` to allocate fresh Infer IDs for unknown types (empty arrays, closures).
pub fn ty_from_value(val: &crate::value::Value, env: &mut TyEnv) -> Ty {
    use crate::value::Value;
    match val {
        Value::I64(_) => Ty::I64,
        Value::F64(_) => Ty::F64,
        Value::Bool(_) => Ty::Bool,
        Value::Str(_) => Ty::String,
        Value::Char(_) => Ty::Char,
        Value::U8(_) => Ty::U8,
        Value::I8(_) => Ty::I8,
        Value::I16(_) => Ty::I16,
        Value::U16(_) => Ty::U16,
        Value::I32(_) => Ty::I32,
        Value::U32(_) => Ty::U32,
        Value::F32(_) => Ty::F32,
        Value::U64(_) => Ty::U64,
        Value::I128(_) => Ty::I128,
        Value::U128(_) => Ty::U128,
        Value::Unit => Ty::Unit,
        Value::Array(elems) => {
            if let Some(first) = elems.first() {
                Ty::Array(Box::new(ty_from_value(first, env)))
            } else {
                Ty::Array(Box::new(env.fresh_infer()))
            }
        }
        Value::Struct(fields) => Ty::Struct(
            fields.iter().map(|(n, v)| (n.clone(), ty_from_value(v, env))).collect(),
        ),
        Value::Tagged { id, payload, .. } => Ty::Tagged {
            tag_id: *id,
            payload: Box::new(ty_from_value(payload, env)),
        },
        Value::TagConstructor { id, .. } => Ty::TagConstructor(*id),
        Value::Closure { .. } | Value::BranchClosure { .. } | Value::BuiltinFn(_) => Ty::Fn {
            param: Box::new(env.fresh_infer()),
            ret: Box::new(env.fresh_infer()),
        },
        Value::MethodSet { .. } => Ty::Unit, // opaque at type level
    }
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
    next_infer_id: u64,
    /// Global registry: method set id → info (tag_id + methods struct).
    method_sets: HashMap<u64, MethodSetInfo>,
    /// Block bodies stored for deferred re-checking on call.
    /// When `let f = { body }`, we store body here keyed by binding name.
    /// On `f(x)`, we re-check body with `in` bound to arg type.
    block_bodies: HashMap<std::string::String, Mir>,
    /// Union-find substitution: Infer(id) → resolved type.
    /// When unify encounters Infer(id), it records the binding here.
    /// resolve() chases these links to find the concrete type.
    infer_subst: HashMap<u64, Ty>,
    /// Constraints on inference variables: id → constraint.
    /// Numeric literals create constrained Infer vars that can only resolve
    /// to certain concrete types (e.g. IntLiteral → Int or Byte).
    infer_constraints: HashMap<u64, InferConstraint>,
}

impl TyEnv {
    pub fn new() -> Self {
        TyEnv {
            bindings: Vec::new(),
            modules: HashMap::new(),
            next_ms_id: 0,
            next_infer_id: 0,
            method_sets: HashMap::new(),
            block_bodies: HashMap::new(),
            infer_subst: HashMap::new(),
            infer_constraints: HashMap::new(),
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
        self.next_infer_id = other.next_infer_id;
        self.infer_subst = other.infer_subst.clone();
        self.infer_constraints = other.infer_constraints.clone();
        self
    }

    /// Add a binding from outside the checker (e.g., for pre-bound builtins).
    pub fn bind_external(&mut self, name: std::string::String, ty: Ty) {
        self.bindings.push((name, ty));
    }

    fn get(&self, name: &str) -> Option<&Ty> {
        self.bindings
            .iter()
            .rev()
            .find_map(|(n, ty)| if n == name { Some(ty) } else { None })
    }

    fn bind(&mut self, name: std::string::String, ty: Ty) {
        self.bindings.push((name, ty));
    }

    fn pop_binding(&mut self) {
        self.bindings.pop();
    }

    fn fresh_infer(&mut self) -> Ty {
        let id = self.next_infer_id;
        self.next_infer_id += 1;
        Ty::Infer(id)
    }

    /// Create a fresh inference variable with a constraint.
    /// Used for numeric literals: int literals get IntLiteral constraint,
    /// float literals get FloatLiteral constraint.
    fn fresh_constrained_infer(&mut self, constraint: InferConstraint) -> Ty {
        let id = self.next_infer_id;
        self.next_infer_id += 1;
        self.infer_constraints.insert(id, constraint);
        Ty::Infer(id)
    }

    /// Look up the constraint on an inference variable, if any.
    fn infer_constraint(&self, id: u64) -> Option<&InferConstraint> {
        self.infer_constraints.get(&id)
    }

    /// Default constrained Infer variables to their concrete defaults.
    /// IntLiteral-constrained → Int, FloatLiteral-constrained → Float.
    /// Unconstrained Infer is left as-is (for `contains_infer` to catch).
    pub fn default_constrained_infer(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Infer(id) => {
                if let Some(resolved) = self.infer_subst.get(id) {
                    return self.default_constrained_infer(&self.resolve(resolved));
                }
                match self.infer_constraint(*id) {
                    Some(InferConstraint::IntLiteral) => Ty::I64,
                    Some(InferConstraint::FloatLiteral) => Ty::F64,
                    None => ty.clone(),
                }
            }
            Ty::Array(elem) => Ty::Array(Box::new(self.default_constrained_infer(elem))),
            Ty::Fn { param, ret } => Ty::Fn {
                param: Box::new(self.default_constrained_infer(param)),
                ret: Box::new(self.default_constrained_infer(ret)),
            },
            Ty::Struct(fields) => Ty::Struct(
                fields.iter().map(|(n, t)| (n.clone(), self.default_constrained_infer(t))).collect(),
            ),
            Ty::Tagged { tag_id, payload } => Ty::Tagged {
                tag_id: *tag_id,
                payload: Box::new(self.default_constrained_infer(payload)),
            },
            Ty::Union(variants) => Ty::Union(
                variants.iter().map(|t| self.default_constrained_infer(t)).collect(),
            ),
            other => other.clone(),
        }
    }

    /// Default constrained Infer only inside Fn types.
    /// Used at binding boundaries to freeze function signatures while keeping
    /// plain values flexible for later unification.
    fn default_constrained_infer_in_fn(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Fn { param, ret } => Ty::Fn {
                param: Box::new(self.default_constrained_infer(param)),
                ret: Box::new(self.default_constrained_infer(ret)),
            },
            Ty::Array(elem) => Ty::Array(Box::new(self.default_constrained_infer_in_fn(elem))),
            Ty::Struct(fields) => Ty::Struct(
                fields.iter().map(|(n, t)| (n.clone(), self.default_constrained_infer_in_fn(t))).collect(),
            ),
            other => other.clone(),
        }
    }

    /// Resolve a type by chasing Infer links through the substitution table.
    /// Recursively resolves compound types so that all nested Infer variables
    /// are replaced with their concrete types (if known).
    pub fn resolve(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Infer(id) => {
                if let Some(resolved) = self.infer_subst.get(id) {
                    self.resolve(resolved)
                } else {
                    ty.clone()
                }
            }
            Ty::Array(elem) => Ty::Array(Box::new(self.resolve(elem))),
            Ty::Fn { param, ret } => Ty::Fn {
                param: Box::new(self.resolve(param)),
                ret: Box::new(self.resolve(ret)),
            },
            Ty::Struct(fields) => Ty::Struct(
                fields.iter().map(|(n, t)| (n.clone(), self.resolve(t))).collect(),
            ),
            Ty::Tagged { tag_id, payload } => Ty::Tagged {
                tag_id: *tag_id,
                payload: Box::new(self.resolve(payload)),
            },
            Ty::Union(variants) => Ty::Union(
                variants.iter().map(|v| self.resolve(v)).collect(),
            ),
            other => other.clone(),
        }
    }

    fn fresh_method_set_id(&mut self) -> u64 {
        let id = self.next_ms_id;
        self.next_ms_id += 1;
        id
    }

    fn register_method_set(&mut self, id: u64, tag_id: TagId, methods: Ty) {
        self.method_sets
            .insert(id, MethodSetInfo { tag_id, methods });
    }

    /// Find a method's type by searching active method sets in scope.
    /// Scans backwards (most recent first) for shadowing semantics,
    /// mirroring `Env::find_method_in_method_sets`.
    fn find_method_type_with_id(&self, tag_id: TagId, method_name: &str) -> Option<(u64, Ty)> {
        for (name, ty) in self.bindings.iter().rev() {
            if !name.starts_with("\0ms") {
                continue;
            }
            if let Ty::MethodSet {
                id,
                tag_id: ms_tag_id,
            } = ty
            {
                if *ms_tag_id == tag_id {
                    if let Some(info) = self.method_sets.get(id) {
                        if let Ty::Struct(fields) = &info.methods {
                            if let Some((_, method_ty)) =
                                fields.iter().find(|(n, _)| n == method_name)
                            {
                                return Some((*id, method_ty.clone()));
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
fn ty_to_tag_id(ty: &Ty, env: &TyEnv) -> Option<TagId> {
    match ty {
        Ty::I64 => Some(TAG_ID_I64),
        Ty::F64 => Some(TAG_ID_F64),
        Ty::Bool => Some(TAG_ID_BOOL),
        Ty::String => Some(TAG_ID_STRING),
        Ty::Char => Some(TAG_ID_CHAR),
        Ty::U8 => Some(TAG_ID_U8),
        Ty::I8 => Some(TAG_ID_I8),
        Ty::I16 => Some(TAG_ID_I16),
        Ty::U16 => Some(TAG_ID_U16),
        Ty::I32 => Some(TAG_ID_I32),
        Ty::U32 => Some(TAG_ID_U32),
        Ty::F32 => Some(TAG_ID_F32),
        Ty::U64 => Some(TAG_ID_U64),
        Ty::I128 => Some(TAG_ID_I128),
        Ty::U128 => Some(TAG_ID_U128),
        Ty::Array(_) => Some(TAG_ID_ARRAY),
        Ty::Unit => Some(TAG_ID_UNIT),
        Ty::Tagged { tag_id, .. } => Some(*tag_id),
        Ty::Infer(id) => match env.infer_constraint(*id) {
            Some(InferConstraint::IntLiteral) => Some(TAG_ID_I64),
            Some(InferConstraint::FloatLiteral) => Some(TAG_ID_F64),
            None => None,
        },
        _ => None,
    }
}

/// Mirror of `prepend_arg` from eval.rs, but for types.
/// `recv.method(arg)` at runtime calls `method_fn(prepend_arg(recv, arg))`.
fn prepend_arg_ty(recv_ty: &Ty, arg_ty: &Ty) -> Ty {
    match arg_ty {
        Ty::Unit => recv_ty.clone(),
        Ty::Struct(fields) => {
            let mut new_fields = vec![("0".into(), recv_ty.clone())];
            for (label, ty) in fields {
                if let Ok(n) = label.parse::<u64>() {
                    new_fields.push(((n + 1).to_string(), ty.clone()));
                } else {
                    new_fields.push((label.clone(), ty.clone()));
                }
            }
            Ty::Struct(new_fields)
        }
        _ => Ty::Struct(vec![
            ("0".into(), recv_ty.clone()),
            ("1".into(), arg_ty.clone()),
        ]),
    }
}

// ── Core module type ────────────────────────────────────────────

/// The type of `build_core_module()` from eval.rs.
pub fn core_module_type() -> Ty {
    let mut fields: Vec<(std::string::String, Ty)> = Vec::new();

    // Type constructors (same order as build_core_module)
    let type_constructors: &[(&str, TagId)] = &[
        ("I64", TAG_ID_I64),
        ("F64", TAG_ID_F64),
        ("Bool", TAG_ID_BOOL),
        ("String", TAG_ID_STRING),
        ("Char", TAG_ID_CHAR),
        ("U8", TAG_ID_U8),
        ("Array", TAG_ID_ARRAY),
        ("Unit", TAG_ID_UNIT),
        ("I8", TAG_ID_I8),
        ("I16", TAG_ID_I16),
        ("U16", TAG_ID_U16),
        ("I32", TAG_ID_I32),
        ("U32", TAG_ID_U32),
        ("F32", TAG_ID_F32),
        ("U64", TAG_ID_U64),
        ("I128", TAG_ID_I128),
        ("U128", TAG_ID_U128),
    ];
    for (name, id) in type_constructors {
        fields.push((name.to_string(), Ty::TagConstructor(*id)));
    }

    // Helpers for building function types.
    // fn(param) -> ret
    let f = |param: Ty, ret: Ty| Ty::Fn {
        param: Box::new(param),
        ret: Box::new(ret),
    };
    // Binary method: prepend_arg(T, T) = Struct([("0", T), ("1", T)])
    let binop = |t: Ty, ret: Ty| {
        f(
            Ty::Struct(vec![("0".into(), t.clone()), ("1".into(), t)]),
            ret,
        )
    };
    // Binary method with different arg types
    let binop2 =
        |t1: Ty, t2: Ty, ret: Ty| f(Ty::Struct(vec![("0".into(), t1), ("1".into(), t2)]), ret);
    // Unary method: prepend_arg(T, Unit) = T
    let unary = |t: Ty, ret: Ty| f(t, ret);

    // Logical builtins (standalone functions, not methods — no prepend)
    fields.push(("not".into(), f(Ty::Bool, Ty::Bool)));
    fields.push((
        "and".into(),
        f(
            Ty::Struct(vec![("0".into(), Ty::Bool), ("1".into(), Ty::Bool)]),
            Ty::Bool,
        ),
    ));
    fields.push((
        "or".into(),
        f(
            Ty::Struct(vec![("0".into(), Ty::Bool), ("1".into(), Ty::Bool)]),
            Ty::Bool,
        ),
    ));

    // Standalone builtins
    fields.push(("print".into(), f(Ty::String, Ty::Unit)));

    // Type hints — identity functions that assert the type at compile time
    fields.push(("byte".into(), f(Ty::U8, Ty::U8)));
    fields.push(("int".into(), f(Ty::I64, Ty::I64)));
    fields.push(("i64".into(), f(Ty::I64, Ty::I64)));
    fields.push(("float".into(), f(Ty::F64, Ty::F64)));
    fields.push(("char".into(), f(Ty::Char, Ty::Char)));
    fields.push(("i8".into(), f(Ty::I8, Ty::I8)));
    fields.push(("u8".into(), f(Ty::U8, Ty::U8)));
    fields.push(("i16".into(), f(Ty::I16, Ty::I16)));
    fields.push(("u16".into(), f(Ty::U16, Ty::U16)));
    fields.push(("i32".into(), f(Ty::I32, Ty::I32)));
    fields.push(("u32".into(), f(Ty::U32, Ty::U32)));
    fields.push(("f32".into(), f(Ty::F32, Ty::F32)));
    fields.push(("u64".into(), f(Ty::U64, Ty::U64)));
    fields.push(("f64".into(), f(Ty::F64, Ty::F64)));
    fields.push(("i128".into(), f(Ty::I128, Ty::I128)));
    fields.push(("u128".into(), f(Ty::U128, Ty::U128)));

    // Equality builtins (standalone, generic — both args must be same type)
    let g0 = Ty::Generic(0);
    let eq_param = Ty::Struct(vec![("0".into(), g0.clone()), ("1".into(), g0.clone())]);
    fields.push(("ref_eq".into(), f(eq_param.clone(), Ty::Bool)));
    fields.push(("val_eq".into(), f(eq_param, Ty::Bool)));
    fields.push(("method_set".into(), Ty::MethodSetConstructor));

    // Array methods — parametric with Generic type variables.
    // G0 = element type T, G1 = second type variable U.
    // Signatures use prepended receiver: Struct([("0", Array(G0)), ("1", user_arg)]).
    let g0 = Ty::Generic(0);
    let g1 = Ty::Generic(1);
    let arr_g0 = Ty::Array(Box::new(g0.clone()));
    let arr_g1 = Ty::Array(Box::new(g1.clone()));
    // array_get: Array(G0) × Int → G0
    fields.push((
        "array_get".into(),
        binop2(arr_g0.clone(), Ty::I64, g0.clone()),
    ));
    // array_slice: Array(G0) × Range → Array(G0)
    // After prepend: (0=Array(G0), start=Int, end=Int)
    let slice_param = Ty::Struct(vec![
        ("0".into(), arr_g0.clone()),
        ("start".into(), Ty::I64),
        ("end".into(), Ty::I64),
    ]);
    fields.push(("array_slice".into(), f(slice_param, arr_g0.clone())));
    // array_len: Array(G0) → Int
    fields.push(("array_len".into(), unary(arr_g0.clone(), Ty::I64)));
    // array_map: Array(G0) × (G0 → G1) → Array(G1)
    let map_cb = Ty::Fn {
        param: Box::new(g0.clone()),
        ret: Box::new(g1.clone()),
    };
    fields.push((
        "array_map".into(),
        binop2(arr_g0.clone(), map_cb, arr_g1.clone()),
    ));
    // array_filter: Array(G0) × (G0 → Bool) → Array(G0)
    let filter_cb = Ty::Fn {
        param: Box::new(g0.clone()),
        ret: Box::new(Ty::Bool),
    };
    fields.push((
        "array_filter".into(),
        binop2(arr_g0.clone(), filter_cb, arr_g0.clone()),
    ));
    // array_fold: Array(G0) × (init: G1, f: (acc: G1, elem: G0) → G1) → G1
    // After prepend: (0=Array(G0), 1=G1, 2=Fn((acc: G1, elem: G0) → G1))
    let fold_f = Ty::Fn {
        param: Box::new(Ty::Struct(vec![
            ("acc".into(), g1.clone()),
            ("elem".into(), g0.clone()),
        ])),
        ret: Box::new(g1.clone()),
    };
    let fold_param = Ty::Struct(vec![
        ("0".into(), arr_g0.clone()),
        ("1".into(), g1.clone()),
        ("2".into(), fold_f),
    ]);
    fields.push(("array_fold".into(), f(fold_param, g1.clone())));
    // array_zip: Array(G0) × Array(G1) → Array((G0, G1))
    let zip_elem = Ty::Struct(vec![("0".into(), g0.clone()), ("1".into(), g1.clone())]);
    fields.push((
        "array_zip".into(),
        binop2(
            arr_g0.clone(),
            arr_g1.clone(),
            Ty::Array(Box::new(zip_elem)),
        ),
    ));
    // array_add: Array(G0) × Array(G0) → Array(G0)
    fields.push(("array_add".into(), binop(arr_g0.clone(), arr_g0.clone())));
    // array_eq / array_not_eq: Array(G0) × Array(G0) → Bool
    fields.push(("array_eq".into(), binop(arr_g0.clone(), Ty::Bool)));
    fields.push(("array_not_eq".into(), binop(arr_g0.clone(), Ty::Bool)));

    // String methods
    fields.push(("string_byte_len".into(), unary(Ty::String, Ty::I64)));
    fields.push(("string_char_len".into(), unary(Ty::String, Ty::I64)));
    fields.push((
        "string_byte_get".into(),
        binop2(Ty::String, Ty::I64, Ty::U8),
    ));
    fields.push((
        "string_char_get".into(),
        binop2(Ty::String, Ty::I64, Ty::Char),
    ));
    fields.push((
        "string_as_bytes".into(),
        unary(Ty::String, Ty::Array(Box::new(Ty::U8))),
    ));
    fields.push((
        "string_chars".into(),
        unary(Ty::String, Ty::Array(Box::new(Ty::Char))),
    ));
    fields.push((
        "string_split".into(),
        binop2(Ty::String, Ty::String, Ty::Array(Box::new(Ty::String))),
    ));
    fields.push(("string_trim".into(), unary(Ty::String, Ty::String)));
    fields.push((
        "string_contains".into(),
        binop2(Ty::String, Ty::String, Ty::Bool),
    ));
    fields.push((
        "string_contains_char".into(),
        binop2(Ty::String, Ty::Char, Ty::Bool),
    ));
    // string_slice: String × Range → String
    // After prepend: (0=String, start=Int, end=Int)
    let string_slice_param = Ty::Struct(vec![
        ("0".into(), Ty::String),
        ("start".into(), Ty::I64),
        ("end".into(), Ty::I64),
    ]);
    fields.push(("string_slice".into(), f(string_slice_param, Ty::String)));
    fields.push(("string_starts_with".into(), binop(Ty::String, Ty::Bool)));
    fields.push(("string_ends_with".into(), binop(Ty::String, Ty::Bool)));
    // string_replace: String × (pattern, replacement) → String
    // After prepend: (0=String, 1=String, 2=String)
    let replace_param = Ty::Struct(vec![
        ("0".into(), Ty::String),
        ("1".into(), Ty::String),
        ("2".into(), Ty::String),
    ]);
    fields.push(("string_replace".into(), f(replace_param, Ty::String)));
    fields.push(("string_add".into(), binop(Ty::String, Ty::String)));
    fields.push(("string_eq".into(), binop(Ty::String, Ty::Bool)));
    fields.push(("string_not_eq".into(), binop(Ty::String, Ty::Bool)));
    fields.push(("string_lt".into(), binop(Ty::String, Ty::Bool)));
    fields.push(("string_gt".into(), binop(Ty::String, Ty::Bool)));
    fields.push(("string_lt_eq".into(), binop(Ty::String, Ty::Bool)));
    fields.push(("string_gt_eq".into(), binop(Ty::String, Ty::Bool)));
    fields.push(("string_to_string".into(), unary(Ty::String, Ty::String)));

    // Int methods — arithmetic is Int × Int → Int (no cross-type promotion)
    fields.push(("int_add".into(), binop(Ty::I64, Ty::I64)));
    fields.push(("int_subtract".into(), binop(Ty::I64, Ty::I64)));
    fields.push(("int_times".into(), binop(Ty::I64, Ty::I64)));
    fields.push(("int_divided_by".into(), binop(Ty::I64, Ty::I64)));
    fields.push(("int_negate".into(), unary(Ty::I64, Ty::I64)));
    fields.push(("int_eq".into(), binop(Ty::I64, Ty::Bool)));
    fields.push(("int_not_eq".into(), binop(Ty::I64, Ty::Bool)));
    fields.push(("int_lt".into(), binop(Ty::I64, Ty::Bool)));
    fields.push(("int_gt".into(), binop(Ty::I64, Ty::Bool)));
    fields.push(("int_lt_eq".into(), binop(Ty::I64, Ty::Bool)));
    fields.push(("int_gt_eq".into(), binop(Ty::I64, Ty::Bool)));
    fields.push(("int_to_string".into(), unary(Ty::I64, Ty::String)));
    fields.push(("int_to_char".into(), unary(Ty::I64, Ty::Char)));
    fields.push(("int_to_i32".into(), unary(Ty::I64, Ty::I32)));
    fields.push(("int_to_i8".into(), unary(Ty::I64, Ty::I8)));
    fields.push(("int_to_u8".into(), unary(Ty::I64, Ty::U8)));
    fields.push(("int_to_i16".into(), unary(Ty::I64, Ty::I16)));
    fields.push(("int_to_u16".into(), unary(Ty::I64, Ty::U16)));
    fields.push(("int_to_u32".into(), unary(Ty::I64, Ty::U32)));
    fields.push(("int_to_u64".into(), unary(Ty::I64, Ty::U64)));
    fields.push(("int_to_f64".into(), unary(Ty::I64, Ty::F64)));
    fields.push(("int_to_f32".into(), unary(Ty::I64, Ty::F32)));
    fields.push(("int_to_i128".into(), unary(Ty::I64, Ty::I128)));
    fields.push(("int_to_u128".into(), unary(Ty::I64, Ty::U128)));

    // Float methods — arithmetic is Float × Float → Float (no cross-type promotion)
    fields.push(("float_add".into(), binop(Ty::F64, Ty::F64)));
    fields.push(("float_subtract".into(), binop(Ty::F64, Ty::F64)));
    fields.push(("float_times".into(), binop(Ty::F64, Ty::F64)));
    fields.push(("float_divided_by".into(), binop(Ty::F64, Ty::F64)));
    fields.push(("float_negate".into(), unary(Ty::F64, Ty::F64)));
    fields.push(("float_eq".into(), binop(Ty::F64, Ty::Bool)));
    fields.push(("float_not_eq".into(), binop(Ty::F64, Ty::Bool)));
    fields.push(("float_lt".into(), binop(Ty::F64, Ty::Bool)));
    fields.push(("float_gt".into(), binop(Ty::F64, Ty::Bool)));
    fields.push(("float_lt_eq".into(), binop(Ty::F64, Ty::Bool)));
    fields.push(("float_gt_eq".into(), binop(Ty::F64, Ty::Bool)));
    fields.push(("float_to_string".into(), unary(Ty::F64, Ty::String)));
    fields.push(("float_ceil".into(), unary(Ty::F64, Ty::I64)));
    fields.push(("float_floor".into(), unary(Ty::F64, Ty::I64)));
    fields.push(("float_round".into(), unary(Ty::F64, Ty::I64)));
    fields.push(("float_trunc".into(), unary(Ty::F64, Ty::I64)));
    fields.push(("float_to_i64".into(), unary(Ty::F64, Ty::I64)));
    fields.push(("float_to_f32".into(), unary(Ty::F64, Ty::F32)));
    fields.push(("float_to_f64".into(), unary(Ty::F64, Ty::F64)));

    // Bool methods
    fields.push(("bool_eq".into(), binop(Ty::Bool, Ty::Bool)));
    fields.push(("bool_not_eq".into(), binop(Ty::Bool, Ty::Bool)));
    fields.push(("bool_to_string".into(), unary(Ty::Bool, Ty::String)));

    // Char methods
    fields.push(("char_eq".into(), binop(Ty::Char, Ty::Bool)));
    fields.push(("char_not_eq".into(), binop(Ty::Char, Ty::Bool)));
    fields.push(("char_lt".into(), binop(Ty::Char, Ty::Bool)));
    fields.push(("char_gt".into(), binop(Ty::Char, Ty::Bool)));
    fields.push(("char_lt_eq".into(), binop(Ty::Char, Ty::Bool)));
    fields.push(("char_gt_eq".into(), binop(Ty::Char, Ty::Bool)));
    fields.push(("char_to_string".into(), unary(Ty::Char, Ty::String)));
    fields.push(("char_to_i64".into(), unary(Ty::Char, Ty::I64)));

    // Byte methods
    fields.push(("byte_eq".into(), binop(Ty::U8, Ty::Bool)));
    fields.push(("byte_not_eq".into(), binop(Ty::U8, Ty::Bool)));
    fields.push(("byte_lt".into(), binop(Ty::U8, Ty::Bool)));
    fields.push(("byte_gt".into(), binop(Ty::U8, Ty::Bool)));
    fields.push(("byte_lt_eq".into(), binop(Ty::U8, Ty::Bool)));
    fields.push(("byte_gt_eq".into(), binop(Ty::U8, Ty::Bool)));
    fields.push(("byte_to_string".into(), unary(Ty::U8, Ty::String)));
    fields.push(("byte_to_i64".into(), unary(Ty::U8, Ty::I64)));
    fields.push(("u8_to_i32".into(), unary(Ty::U8, Ty::I32)));
    fields.push(("u8_to_i128".into(), unary(Ty::U8, Ty::I128)));
    fields.push(("u8_to_u128".into(), unary(Ty::U8, Ty::U128)));

    // I32 methods — arithmetic is I32 × I32 → I32
    fields.push(("i32_add".into(), binop(Ty::I32, Ty::I32)));
    fields.push(("i32_subtract".into(), binop(Ty::I32, Ty::I32)));
    fields.push(("i32_times".into(), binop(Ty::I32, Ty::I32)));
    fields.push(("i32_divided_by".into(), binop(Ty::I32, Ty::I32)));
    fields.push(("i32_negate".into(), unary(Ty::I32, Ty::I32)));
    fields.push(("i32_eq".into(), binop(Ty::I32, Ty::Bool)));
    fields.push(("i32_not_eq".into(), binop(Ty::I32, Ty::Bool)));
    fields.push(("i32_lt".into(), binop(Ty::I32, Ty::Bool)));
    fields.push(("i32_gt".into(), binop(Ty::I32, Ty::Bool)));
    fields.push(("i32_lt_eq".into(), binop(Ty::I32, Ty::Bool)));
    fields.push(("i32_gt_eq".into(), binop(Ty::I32, Ty::Bool)));
    fields.push(("i32_to_string".into(), unary(Ty::I32, Ty::String)));
    fields.push(("i32_to_i64".into(), unary(Ty::I32, Ty::I64)));
    fields.push(("i32_to_f64".into(), unary(Ty::I32, Ty::F64)));
    fields.push(("i32_to_f32".into(), unary(Ty::I32, Ty::F32)));
    fields.push(("i32_to_u8".into(), unary(Ty::I32, Ty::U8)));
    fields.push(("i32_to_i128".into(), unary(Ty::I32, Ty::I128)));
    fields.push(("i32_to_u128".into(), unary(Ty::I32, Ty::U128)));

    // F32 methods — arithmetic is F32 × F32 → F32
    fields.push(("f32_add".into(), binop(Ty::F32, Ty::F32)));
    fields.push(("f32_subtract".into(), binop(Ty::F32, Ty::F32)));
    fields.push(("f32_times".into(), binop(Ty::F32, Ty::F32)));
    fields.push(("f32_divided_by".into(), binop(Ty::F32, Ty::F32)));
    fields.push(("f32_negate".into(), unary(Ty::F32, Ty::F32)));
    fields.push(("f32_eq".into(), binop(Ty::F32, Ty::Bool)));
    fields.push(("f32_not_eq".into(), binop(Ty::F32, Ty::Bool)));
    fields.push(("f32_lt".into(), binop(Ty::F32, Ty::Bool)));
    fields.push(("f32_gt".into(), binop(Ty::F32, Ty::Bool)));
    fields.push(("f32_lt_eq".into(), binop(Ty::F32, Ty::Bool)));
    fields.push(("f32_gt_eq".into(), binop(Ty::F32, Ty::Bool)));
    fields.push(("f32_to_string".into(), unary(Ty::F32, Ty::String)));
    fields.push(("f32_to_f64".into(), unary(Ty::F32, Ty::F64)));
    fields.push(("f32_to_i64".into(), unary(Ty::F32, Ty::I64)));
    fields.push(("f32_to_i32".into(), unary(Ty::F32, Ty::I32)));
    fields.push(("f32_ceil".into(), unary(Ty::F32, Ty::I32)));
    fields.push(("f32_floor".into(), unary(Ty::F32, Ty::I32)));
    fields.push(("f32_round".into(), unary(Ty::F32, Ty::I32)));
    fields.push(("f32_trunc".into(), unary(Ty::F32, Ty::I32)));

    // I8 methods
    fields.push(("i8_add".into(), binop(Ty::I8, Ty::I8)));
    fields.push(("i8_subtract".into(), binop(Ty::I8, Ty::I8)));
    fields.push(("i8_times".into(), binop(Ty::I8, Ty::I8)));
    fields.push(("i8_divided_by".into(), binop(Ty::I8, Ty::I8)));
    fields.push(("i8_negate".into(), unary(Ty::I8, Ty::I8)));
    fields.push(("i8_eq".into(), binop(Ty::I8, Ty::Bool)));
    fields.push(("i8_not_eq".into(), binop(Ty::I8, Ty::Bool)));
    fields.push(("i8_lt".into(), binop(Ty::I8, Ty::Bool)));
    fields.push(("i8_gt".into(), binop(Ty::I8, Ty::Bool)));
    fields.push(("i8_lt_eq".into(), binop(Ty::I8, Ty::Bool)));
    fields.push(("i8_gt_eq".into(), binop(Ty::I8, Ty::Bool)));
    fields.push(("i8_to_string".into(), unary(Ty::I8, Ty::String)));
    fields.push(("i8_to_i64".into(), unary(Ty::I8, Ty::I64)));

    // U8 methods
    fields.push(("u8_add".into(), binop(Ty::U8, Ty::U8)));
    fields.push(("u8_subtract".into(), binop(Ty::U8, Ty::U8)));
    fields.push(("u8_times".into(), binop(Ty::U8, Ty::U8)));
    fields.push(("u8_divided_by".into(), binop(Ty::U8, Ty::U8)));
    fields.push(("u8_eq".into(), binop(Ty::U8, Ty::Bool)));
    fields.push(("u8_not_eq".into(), binop(Ty::U8, Ty::Bool)));
    fields.push(("u8_lt".into(), binop(Ty::U8, Ty::Bool)));
    fields.push(("u8_gt".into(), binop(Ty::U8, Ty::Bool)));
    fields.push(("u8_lt_eq".into(), binop(Ty::U8, Ty::Bool)));
    fields.push(("u8_gt_eq".into(), binop(Ty::U8, Ty::Bool)));
    fields.push(("u8_to_string".into(), unary(Ty::U8, Ty::String)));
    fields.push(("u8_to_i64".into(), unary(Ty::U8, Ty::I64)));

    // I16 methods
    fields.push(("i16_add".into(), binop(Ty::I16, Ty::I16)));
    fields.push(("i16_subtract".into(), binop(Ty::I16, Ty::I16)));
    fields.push(("i16_times".into(), binop(Ty::I16, Ty::I16)));
    fields.push(("i16_divided_by".into(), binop(Ty::I16, Ty::I16)));
    fields.push(("i16_negate".into(), unary(Ty::I16, Ty::I16)));
    fields.push(("i16_eq".into(), binop(Ty::I16, Ty::Bool)));
    fields.push(("i16_not_eq".into(), binop(Ty::I16, Ty::Bool)));
    fields.push(("i16_lt".into(), binop(Ty::I16, Ty::Bool)));
    fields.push(("i16_gt".into(), binop(Ty::I16, Ty::Bool)));
    fields.push(("i16_lt_eq".into(), binop(Ty::I16, Ty::Bool)));
    fields.push(("i16_gt_eq".into(), binop(Ty::I16, Ty::Bool)));
    fields.push(("i16_to_string".into(), unary(Ty::I16, Ty::String)));
    fields.push(("i16_to_i64".into(), unary(Ty::I16, Ty::I64)));

    // U16 methods
    fields.push(("u16_add".into(), binop(Ty::U16, Ty::U16)));
    fields.push(("u16_subtract".into(), binop(Ty::U16, Ty::U16)));
    fields.push(("u16_times".into(), binop(Ty::U16, Ty::U16)));
    fields.push(("u16_divided_by".into(), binop(Ty::U16, Ty::U16)));
    fields.push(("u16_eq".into(), binop(Ty::U16, Ty::Bool)));
    fields.push(("u16_not_eq".into(), binop(Ty::U16, Ty::Bool)));
    fields.push(("u16_lt".into(), binop(Ty::U16, Ty::Bool)));
    fields.push(("u16_gt".into(), binop(Ty::U16, Ty::Bool)));
    fields.push(("u16_lt_eq".into(), binop(Ty::U16, Ty::Bool)));
    fields.push(("u16_gt_eq".into(), binop(Ty::U16, Ty::Bool)));
    fields.push(("u16_to_string".into(), unary(Ty::U16, Ty::String)));
    fields.push(("u16_to_i64".into(), unary(Ty::U16, Ty::I64)));

    // U32 methods
    fields.push(("u32_add".into(), binop(Ty::U32, Ty::U32)));
    fields.push(("u32_subtract".into(), binop(Ty::U32, Ty::U32)));
    fields.push(("u32_times".into(), binop(Ty::U32, Ty::U32)));
    fields.push(("u32_divided_by".into(), binop(Ty::U32, Ty::U32)));
    fields.push(("u32_eq".into(), binop(Ty::U32, Ty::Bool)));
    fields.push(("u32_not_eq".into(), binop(Ty::U32, Ty::Bool)));
    fields.push(("u32_lt".into(), binop(Ty::U32, Ty::Bool)));
    fields.push(("u32_gt".into(), binop(Ty::U32, Ty::Bool)));
    fields.push(("u32_lt_eq".into(), binop(Ty::U32, Ty::Bool)));
    fields.push(("u32_gt_eq".into(), binop(Ty::U32, Ty::Bool)));
    fields.push(("u32_to_string".into(), unary(Ty::U32, Ty::String)));
    fields.push(("u32_to_i64".into(), unary(Ty::U32, Ty::I64)));

    // U64 methods
    fields.push(("u64_add".into(), binop(Ty::U64, Ty::U64)));
    fields.push(("u64_subtract".into(), binop(Ty::U64, Ty::U64)));
    fields.push(("u64_times".into(), binop(Ty::U64, Ty::U64)));
    fields.push(("u64_divided_by".into(), binop(Ty::U64, Ty::U64)));
    fields.push(("u64_eq".into(), binop(Ty::U64, Ty::Bool)));
    fields.push(("u64_not_eq".into(), binop(Ty::U64, Ty::Bool)));
    fields.push(("u64_lt".into(), binop(Ty::U64, Ty::Bool)));
    fields.push(("u64_gt".into(), binop(Ty::U64, Ty::Bool)));
    fields.push(("u64_lt_eq".into(), binop(Ty::U64, Ty::Bool)));
    fields.push(("u64_gt_eq".into(), binop(Ty::U64, Ty::Bool)));
    fields.push(("u64_to_string".into(), unary(Ty::U64, Ty::String)));
    fields.push(("u64_to_i64".into(), unary(Ty::U64, Ty::I64)));

    // F64 methods
    fields.push(("f64_add".into(), binop(Ty::F64, Ty::F64)));
    fields.push(("f64_subtract".into(), binop(Ty::F64, Ty::F64)));
    fields.push(("f64_times".into(), binop(Ty::F64, Ty::F64)));
    fields.push(("f64_divided_by".into(), binop(Ty::F64, Ty::F64)));
    fields.push(("f64_negate".into(), unary(Ty::F64, Ty::F64)));
    fields.push(("f64_eq".into(), binop(Ty::F64, Ty::Bool)));
    fields.push(("f64_not_eq".into(), binop(Ty::F64, Ty::Bool)));
    fields.push(("f64_lt".into(), binop(Ty::F64, Ty::Bool)));
    fields.push(("f64_gt".into(), binop(Ty::F64, Ty::Bool)));
    fields.push(("f64_lt_eq".into(), binop(Ty::F64, Ty::Bool)));
    fields.push(("f64_gt_eq".into(), binop(Ty::F64, Ty::Bool)));
    fields.push(("f64_to_string".into(), unary(Ty::F64, Ty::String)));
    fields.push(("f64_to_f64".into(), unary(Ty::F64, Ty::F64)));
    fields.push(("f64_to_i64".into(), unary(Ty::F64, Ty::I64)));
    fields.push(("f64_ceil".into(), unary(Ty::F64, Ty::I64)));
    fields.push(("f64_floor".into(), unary(Ty::F64, Ty::I64)));
    fields.push(("f64_round".into(), unary(Ty::F64, Ty::I64)));
    fields.push(("f64_trunc".into(), unary(Ty::F64, Ty::I64)));

    // I128 methods — arithmetic is I128 × I128 → I128
    fields.push(("i128_add".into(), binop(Ty::I128, Ty::I128)));
    fields.push(("i128_subtract".into(), binop(Ty::I128, Ty::I128)));
    fields.push(("i128_times".into(), binop(Ty::I128, Ty::I128)));
    fields.push(("i128_divided_by".into(), binop(Ty::I128, Ty::I128)));
    fields.push(("i128_negate".into(), unary(Ty::I128, Ty::I128)));
    fields.push(("i128_eq".into(), binop(Ty::I128, Ty::Bool)));
    fields.push(("i128_not_eq".into(), binop(Ty::I128, Ty::Bool)));
    fields.push(("i128_lt".into(), binop(Ty::I128, Ty::Bool)));
    fields.push(("i128_gt".into(), binop(Ty::I128, Ty::Bool)));
    fields.push(("i128_lt_eq".into(), binop(Ty::I128, Ty::Bool)));
    fields.push(("i128_gt_eq".into(), binop(Ty::I128, Ty::Bool)));
    fields.push(("i128_to_string".into(), unary(Ty::I128, Ty::String)));
    fields.push(("i128_to_i64".into(), unary(Ty::I128, Ty::I64)));
    fields.push(("i128_to_i32".into(), unary(Ty::I128, Ty::I32)));
    fields.push(("i128_to_u128".into(), unary(Ty::I128, Ty::U128)));

    // U128 methods — arithmetic is U128 × U128 → U128
    fields.push(("u128_add".into(), binop(Ty::U128, Ty::U128)));
    fields.push(("u128_subtract".into(), binop(Ty::U128, Ty::U128)));
    fields.push(("u128_times".into(), binop(Ty::U128, Ty::U128)));
    fields.push(("u128_divided_by".into(), binop(Ty::U128, Ty::U128)));
    fields.push(("u128_eq".into(), binop(Ty::U128, Ty::Bool)));
    fields.push(("u128_not_eq".into(), binop(Ty::U128, Ty::Bool)));
    fields.push(("u128_lt".into(), binop(Ty::U128, Ty::Bool)));
    fields.push(("u128_gt".into(), binop(Ty::U128, Ty::Bool)));
    fields.push(("u128_lt_eq".into(), binop(Ty::U128, Ty::Bool)));
    fields.push(("u128_gt_eq".into(), binop(Ty::U128, Ty::Bool)));
    fields.push(("u128_to_string".into(), unary(Ty::U128, Ty::String)));
    fields.push(("u128_to_i64".into(), unary(Ty::U128, Ty::I64)));
    fields.push(("u128_to_i32".into(), unary(Ty::U128, Ty::I32)));
    fields.push(("u128_to_i128".into(), unary(Ty::U128, Ty::I128)));

    // Unit methods — after prepend_arg(Unit, Unit) = Unit, so param is just Unit
    fields.push(("unit_eq".into(), unary(Ty::Unit, Ty::Bool)));
    fields.push(("unit_not_eq".into(), unary(Ty::Unit, Ty::Bool)));

    Ty::Struct(fields)
}

// ── Unification ─────────────────────────────────────────────────

/// Add a type to a union, merging same-tag variants. Returns the new union.
fn union_add(mut variants: Vec<Ty>, ty: Ty, env: &mut TyEnv) -> Result<Ty, std::string::String> {
    // Flatten: if ty is itself a Union, merge all its variants
    let to_add = match ty {
        Ty::Union(inner) => inner,
        other => vec![other],
    };
    for t in to_add {
        // Check if this tag already exists in the union — unify payloads
        let mut merged = false;
        for existing in variants.iter_mut() {
            if let (
                Ty::Tagged {
                    tag_id: id1,
                    payload: p1,
                },
                Ty::Tagged {
                    tag_id: id2,
                    payload: p2,
                },
            ) = (existing, &t)
            {
                if id1 == id2 {
                    let unified_payload = unify(p1, p2, env)?;
                    *p1 = Box::new(unified_payload);
                    merged = true;
                    break;
                }
            }
        }
        if !merged {
            // Check for structural duplicates
            if !variants.contains(&t) {
                variants.push(t);
            }
        }
    }
    if variants.len() == 1 {
        Ok(variants.into_iter().next().unwrap())
    } else {
        Ok(Ty::Union(variants))
    }
}

/// Merge two inference constraints. Returns error if incompatible.
fn merge_constraints(
    c1: Option<&InferConstraint>,
    c2: Option<&InferConstraint>,
) -> Result<Option<InferConstraint>, std::string::String> {
    match (c1, c2) {
        (None, None) => Ok(None),
        (Some(c), None) | (None, Some(c)) => Ok(Some(c.clone())),
        (Some(InferConstraint::IntLiteral), Some(InferConstraint::IntLiteral)) => {
            Ok(Some(InferConstraint::IntLiteral))
        }
        (Some(InferConstraint::FloatLiteral), Some(InferConstraint::FloatLiteral)) => {
            Ok(Some(InferConstraint::FloatLiteral))
        }
        _ => Err("type error: cannot unify int literal with float literal".to_string()),
    }
}

/// Check that a concrete type is allowed by the constraint on an Infer variable.
fn check_constraint_allows(env: &TyEnv, id: u64, ty: &Ty) -> Result<(), std::string::String> {
    if let Some(constraint) = env.infer_constraint(id) {
        match constraint {
            InferConstraint::IntLiteral => match ty {
                Ty::I64 | Ty::U8
                | Ty::I8 | Ty::I16 | Ty::U16
                | Ty::I32 | Ty::U32 | Ty::U64
                | Ty::I128 | Ty::U128 => Ok(()),
                _ => Err(format!("type error: cannot unify {:?} with {:?}", Ty::I64, ty)),
            },
            InferConstraint::FloatLiteral => match ty {
                Ty::F64 | Ty::F32 => Ok(()),
                _ => Err(format!("type error: cannot unify {:?} with {:?}", Ty::F64, ty)),
            },
        }
    } else {
        Ok(())
    }
}

/// Check if two types are compatible and return the unified type.
/// Records Infer bindings in env.infer_subst (union-find).
pub fn unify(a: &Ty, b: &Ty, env: &mut TyEnv) -> Result<Ty, std::string::String> {
    let a = env.resolve(a);
    let b = env.resolve(b);
    match (&a, &b) {
        // Infer: constraint-aware union-find
        (Ty::Infer(id), Ty::Infer(id2)) if id == id2 => Ok(a.clone()),
        (Ty::Infer(id1), Ty::Infer(id2)) => {
            // Merge constraints from both Infer vars
            let c1 = env.infer_constraint(*id1).cloned();
            let c2 = env.infer_constraint(*id2).cloned();
            let merged = merge_constraints(c1.as_ref(), c2.as_ref())?;
            // Point id1 → id2 (union-find)
            env.infer_subst.insert(*id1, Ty::Infer(*id2));
            if let Some(c) = merged {
                env.infer_constraints.insert(*id2, c);
            }
            Ok(Ty::Infer(*id2))
        }
        (Ty::Infer(id), other) => {
            check_constraint_allows(env, *id, other)?;
            env.infer_subst.insert(*id, other.clone());
            Ok(other.clone())
        }
        (other, Ty::Infer(id)) => {
            check_constraint_allows(env, *id, other)?;
            env.infer_subst.insert(*id, other.clone());
            Ok(other.clone())
        }
        (Ty::Generic(_), other) | (other, Ty::Generic(_)) => Ok(other.clone()),

        // Primitives: must match exactly
        (Ty::I64, Ty::I64) => Ok(Ty::I64),
        (Ty::F64, Ty::F64) => Ok(Ty::F64),
        (Ty::Bool, Ty::Bool) => Ok(Ty::Bool),
        (Ty::String, Ty::String) => Ok(Ty::String),
        (Ty::Char, Ty::Char) => Ok(Ty::Char),
        (Ty::U8, Ty::U8) => Ok(Ty::U8),
        (Ty::I8, Ty::I8) => Ok(Ty::I8),
        (Ty::I16, Ty::I16) => Ok(Ty::I16),
        (Ty::U16, Ty::U16) => Ok(Ty::U16),
        (Ty::I32, Ty::I32) => Ok(Ty::I32),
        (Ty::U32, Ty::U32) => Ok(Ty::U32),
        (Ty::F32, Ty::F32) => Ok(Ty::F32),
        (Ty::U64, Ty::U64) => Ok(Ty::U64),
        (Ty::I128, Ty::I128) => Ok(Ty::I128),
        (Ty::U128, Ty::U128) => Ok(Ty::U128),
        (Ty::Unit, Ty::Unit) => Ok(Ty::Unit),
        (Ty::Array(e1), Ty::Array(e2)) => {
            let elem = unify(e1, e2, env)?;
            Ok(Ty::Array(Box::new(elem)))
        }

        // Functions: unify param and return types
        (Ty::Fn { param: p1, ret: r1 }, Ty::Fn { param: p2, ret: r2 }) => {
            let param = unify(p1, p2, env)?;
            let ret = unify(r1, r2, env)?;
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
                let t = unify(t1, t2, env)?;
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

        // Tagged: same tag ID → unify payloads; different tags → union
        (
            Ty::Tagged {
                tag_id: id1,
                payload: p1,
            },
            Ty::Tagged {
                tag_id: id2,
                payload: p2,
            },
        ) => {
            if id1 == id2 {
                let payload = unify(p1, p2, env)?;
                Ok(Ty::Tagged {
                    tag_id: *id1,
                    payload: Box::new(payload),
                })
            } else {
                Ok(Ty::Union(vec![a.clone(), b.clone()]))
            }
        }

        // Union with anything → merge into the union
        (Ty::Union(variants), other) | (other, Ty::Union(variants)) => {
            union_add(variants.clone(), other.clone(), env)
        }

        // Tagged with non-tagged → union
        (Ty::Tagged { .. }, _) | (_, Ty::Tagged { .. }) => {
            Ok(Ty::Union(vec![a.clone(), b.clone()]))
        }

        (Ty::MethodSetConstructor, Ty::MethodSetConstructor) => Ok(Ty::MethodSetConstructor),

        // MethodSet: must have same generative ID (same lexical site)
        (
            Ty::MethodSet {
                id: id1,
                tag_id: t1,
            },
            Ty::MethodSet {
                id: id2,
                tag_id: t2,
            },
        ) => {
            if id1 == id2 && t1 == t2 {
                Ok(Ty::MethodSet {
                    id: *id1,
                    tag_id: *t1,
                })
            } else {
                Err("type error: cannot unify different method set types".to_string())
            }
        }

        // Everything else: mismatch
        _ => Err(format!("type error: cannot unify {:?} with {:?}", a, b)),
    }
}

/// Unify two types while collecting Generic substitutions.
/// When a Generic(id) meets a concrete type, record the mapping.
/// If Generic(id) is already mapped, unify the existing mapping with the new type.
fn unify_with_generics(
    a: &Ty,
    b: &Ty,
    subst: &mut HashMap<u64, Ty>,
    env: &mut TyEnv,
) -> Result<Ty, std::string::String> {
    let a = env.resolve(a);
    let b = env.resolve(b);
    match (&a, &b) {
        (Ty::Generic(id), Ty::Generic(id2)) if id == id2 => Ok(Ty::Generic(*id)),
        (Ty::Generic(id), other) | (other, Ty::Generic(id)) => {
            if let Some(existing) = subst.get(id).cloned() {
                let unified = unify_with_generics(&existing, other, subst, env)?;
                subst.insert(*id, unified.clone());
                Ok(unified)
            } else {
                subst.insert(*id, other.clone());
                Ok(other.clone())
            }
        }
        // Recurse into compound types
        (Ty::Array(e1), Ty::Array(e2)) => {
            let elem = unify_with_generics(e1, e2, subst, env)?;
            Ok(Ty::Array(Box::new(elem)))
        }
        (Ty::Fn { param: p1, ret: r1 }, Ty::Fn { param: p2, ret: r2 }) => {
            let param = unify_with_generics(p1, p2, subst, env)?;
            let ret = unify_with_generics(r1, r2, subst, env)?;
            Ok(Ty::Fn {
                param: Box::new(param),
                ret: Box::new(ret),
            })
        }
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
                let t = unify_with_generics(t1, t2, subst, env)?;
                unified.push((n1.clone(), t));
            }
            Ok(Ty::Struct(unified))
        }
        // Delegate all other cases to regular unify
        _ => unify(&a, &b, env),
    }
}

/// Apply a substitution map, replacing Generic(id) with its concrete type.
fn substitute_generics(ty: &Ty, subst: &HashMap<u64, Ty>) -> Ty {
    match ty {
        Ty::Generic(id) => subst.get(id).cloned().unwrap_or_else(|| ty.clone()),
        Ty::Array(elem) => Ty::Array(Box::new(substitute_generics(elem, subst))),
        Ty::Fn { param, ret } => Ty::Fn {
            param: Box::new(substitute_generics(param, subst)),
            ret: Box::new(substitute_generics(ret, subst)),
        },
        Ty::Struct(fields) => Ty::Struct(
            fields
                .iter()
                .map(|(n, t)| (n.clone(), substitute_generics(t, subst)))
                .collect(),
        ),
        Ty::Tagged { tag_id, payload } => Ty::Tagged {
            tag_id: *tag_id,
            payload: Box::new(substitute_generics(payload, subst)),
        },
        Ty::Union(variants) => Ty::Union(
            variants
                .iter()
                .map(|v| substitute_generics(v, subst))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Find a block body for a struct field method call.
/// Checks: 1) block_bodies["name.field"] for Ident receivers
///         2) inline struct literal fields for Struct receivers
fn find_field_block_body(receiver: &Mir, field: &str, env: &TyEnv) -> Option<Mir> {
    match receiver.as_ref() {
        MirKind::Ident(recv_name) => {
            let key = format!("{}.{}", recv_name, field);
            env.block_bodies.get(&key).cloned()
        }
        MirKind::Struct(fields) => {
            let mut positional_idx = 0u64;
            for f in fields {
                if f.is_spread {
                    continue;
                }
                let label = match &f.label {
                    Some(name) => name.clone(),
                    None => {
                        let l = positional_idx.to_string();
                        positional_idx += 1;
                        l
                    }
                };
                if label == field {
                    match f.value.as_ref() {
                        MirKind::Block(_) | MirKind::BranchBlock(_) => {
                            return Some(f.value.clone());
                        }
                        _ => return None,
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Store block/branch bodies from struct literal fields under "structname.fieldname" keys.
/// This enables re-checking when the field is later called: `s.f(arg)`.
fn store_struct_field_bodies(struct_name: &str, fields: &[crate::mir::MirField], env: &mut TyEnv) {
    let mut positional_idx = 0u64;
    for field in fields {
        if field.is_spread {
            continue;
        }
        let label = match &field.label {
            Some(name) => name.clone(),
            None => {
                let label = positional_idx.to_string();
                positional_idx += 1;
                label
            }
        };
        let key = format!("{}.{}", struct_name, label);
        match field.value.as_ref() {
            MirKind::Block(_) | MirKind::BranchBlock(_) => {
                env.block_bodies.insert(key, field.value.clone());
            }
            _ => {
                // Try to resolve the field value to a block (e.g., Ident referencing a block)
                if let Some(resolved) = resolve_to_block_mir(&field.value, env) {
                    env.block_bodies.insert(key, resolved);
                }
            }
        }
    }
}

/// Propagate block bodies from a struct LHS to destructured pattern fields.
/// When `(a, { block }) >> let(x, f); f(5)`, we store `block_bodies["f"]` = the block MIR.
/// Handles both inline struct MIR and Ident references with stored field block_bodies.
fn propagate_block_bodies_to_fields(
    lhs: &Mir,
    pat_fields: &[crate::ast::PatField],
    env: &mut TyEnv,
) {
    // Collect (position_index, block_mir) from the LHS
    match lhs.as_ref() {
        MirKind::Struct(mir_fields) => {
            // Direct struct literal — extract blocks from fields
            for pat_field in pat_fields {
                if pat_field.is_rest || pat_field.binding == "_" {
                    continue;
                }
                let mir_field = find_mir_field_for_pattern(mir_fields, pat_field, pat_fields);
                if let Some(mf) = mir_field {
                    match mf.value.as_ref() {
                        MirKind::Block(_) | MirKind::BranchBlock(_) => {
                            env.block_bodies
                                .insert(pat_field.binding.clone(), mf.value.clone());
                        }
                        MirKind::Struct(inner_fields) => {
                            // Struct of blocks — store field-level block bodies
                            store_struct_field_bodies(&pat_field.binding, inner_fields, env);
                        }
                        _ => {}
                    }
                }
            }
        }
        MirKind::Ident(name) => {
            // Ident reference — look up stored block_bodies with "name.field" keys
            let mut positional_idx = 0u64;
            for pat_field in pat_fields {
                if pat_field.is_rest || pat_field.binding == "_" {
                    if !pat_field.is_rest && pat_field.label.is_none() {
                        positional_idx += 1;
                    }
                    continue;
                }
                // Try looking up by explicit label, by binding name, or by positional index
                let lookup_key = if let Some(label) = &pat_field.label {
                    label.clone()
                } else {
                    pat_field.binding.clone()
                };
                let key_by_name = format!("{}.{}", name, lookup_key);
                let key_by_pos = format!("{}.{}", name, positional_idx);
                if let Some(body) = env
                    .block_bodies
                    .get(&key_by_name)
                    .or_else(|| env.block_bodies.get(&key_by_pos))
                    .cloned()
                {
                    env.block_bodies.insert(pat_field.binding.clone(), body);
                }
                if pat_field.label.is_none() {
                    positional_idx += 1;
                }
            }
        }
        _ => {}
    }
}

/// Find the MIR field corresponding to a pattern field in a struct destructuring.
fn find_mir_field_for_pattern<'a>(
    mir_fields: &'a [crate::mir::MirField],
    pat_field: &crate::ast::PatField,
    all_pat_fields: &[crate::ast::PatField],
) -> Option<&'a crate::mir::MirField> {
    if let Some(label) = &pat_field.label {
        // Explicit label — match by name
        mir_fields.iter().find(|f| f.label.as_deref() == Some(label))
    } else {
        // Positional — find the position of this pat_field among non-rest fields
        let pos = all_pat_fields
            .iter()
            .filter(|f| !f.is_rest)
            .position(|f| std::ptr::eq(f, pat_field));
        pos.and_then(|idx| {
            let mut positional_idx = 0usize;
            for mf in mir_fields {
                if mf.label.is_none() {
                    if positional_idx == idx {
                        return Some(mf);
                    }
                    positional_idx += 1;
                }
            }
            // Also try matching by stringified index for labeled fields
            let key = idx.to_string();
            mir_fields.iter().find(|f| f.label.as_deref() == Some(&key[..]))
        })
    }
}

/// Try to resolve a MIR expression to a Block/BranchBlock, following simple patterns.
/// This enables propagation of block bodies through calls and binds.
fn resolve_to_block_mir(mir: &Mir, env: &TyEnv) -> Option<Mir> {
    match mir.as_ref() {
        MirKind::Block(_) | MirKind::BranchBlock(_) => Some(mir.clone()),
        MirKind::Ident(name) => env.block_bodies.get(name).cloned(),
        MirKind::Call(func, arg) => {
            // Get the function's block body (if known)
            let func_body = match func.as_ref() {
                MirKind::Ident(name) => env.block_bodies.get(name).cloned(),
                MirKind::Block(_) | MirKind::BranchBlock(_) => Some(func.clone()),
                _ => None,
            };
            if let Some(body_mir) = func_body {
                if let MirKind::Block(inner) = body_mir.as_ref() {
                    if matches!(inner.as_ref(), MirKind::Ident(n) if n == "in") {
                        // Identity function: f(x) = x
                        return resolve_to_block_mir(arg, env);
                    }
                    // Check if the block body's tail expression is a Block
                    // (function that returns a closure, like compose)
                    if let Some(tail) = find_tail_expr(inner) {
                        return resolve_to_block_mir(tail, env);
                    }
                }
            }
            None
        }
        MirKind::FieldAccess(expr, field) => {
            if let MirKind::Ident(name) = expr.as_ref() {
                let key = format!("{}.{}", name, field);
                env.block_bodies.get(&key).cloned()
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Find the tail (final) expression in a MIR chain.
/// Follows through Bind, Pipe, and Let to find what the expression ultimately evaluates to.
fn find_tail_expr(mir: &Mir) -> Option<&Mir> {
    match mir.as_ref() {
        MirKind::Bind { body, .. } => find_tail_expr(body),
        MirKind::Pipe(_, rhs) => {
            match rhs.as_ref() {
                MirKind::Let { body, .. } => find_tail_expr(body),
                MirKind::LetArray { body, .. } => find_tail_expr(body),
                MirKind::Apply { body, .. } => find_tail_expr(body),
                _ => Some(rhs),
            }
        }
        MirKind::Block(_) | MirKind::BranchBlock(_) => Some(mir),
        _ => None,
    }
}

/// Re-check a stored block/branch body with a known arg type and MIR.
/// Binds `in` to arg_ty and propagates block_bodies from arg_mir.
fn recheck_block_body(
    stored_body: &Mir,
    arg_ty: Ty,
    arg_mir: &Mir,
    env: &mut TyEnv,
) -> Result<Ty, String> {
    // Propagate block bodies from the arg MIR to "in" scope
    match arg_mir.as_ref() {
        MirKind::Block(_) | MirKind::BranchBlock(_) => {
            env.block_bodies.insert("in".to_string(), arg_mir.clone());
        }
        MirKind::Struct(fields) => {
            store_struct_field_bodies("in", fields, env);
        }
        _ => {}
    }
    match stored_body.as_ref() {
        MirKind::Block(body) => {
            env.bind("in".to_string(), arg_ty);
            let body_ty = check(body, env)?;
            env.pop_binding();
            Ok(body_ty)
        }
        MirKind::BranchBlock(arms) => check_branch_block_with_input(arms, &arg_ty, env),
        _ => unreachable!("block_bodies only stores Block/BranchBlock"),
    }
}

// ── Type checker ────────────────────────────────────────────────

pub fn check(mir: &Mir, env: &mut TyEnv) -> Result<Ty, std::string::String> {
    match mir.as_ref() {
        // ── Literals ──
        MirKind::Int(_) => Ok(env.fresh_constrained_infer(InferConstraint::IntLiteral)),
        MirKind::Float(_) => Ok(env.fresh_constrained_infer(InferConstraint::FloatLiteral)),
        MirKind::Bool(_) => Ok(Ty::Bool),
        MirKind::Str(_) => Ok(Ty::String),
        MirKind::Char(_) => Ok(Ty::Char),
        MirKind::Byte(_) => Ok(Ty::U8),
        MirKind::Unit => Ok(Ty::Unit),

        // ── Ident ──
        MirKind::Ident(name) => {
            let ty = env
                .get(name)
                .cloned()
                .ok_or_else(|| format!("type error: undefined variable: {}", name))?;
            let resolved = env.resolve(&ty);
            Ok(resolved)
        }

        // ── Import ──
        MirKind::Import(name) => env
            .modules
            .get(name)
            .cloned()
            .ok_or_else(|| format!("type error: module not provided: {}", name)),

        // ── FieldAccess ──
        MirKind::FieldAccess(expr, field) => {
            let ty = check(expr, env)?;
            match ty {
                Ty::Struct(fields) => fields
                    .iter()
                    .find(|(name, _)| name == field)
                    .map(|(_, ty)| ty.clone())
                    .ok_or_else(|| format!("type error: field '{}' not found in struct", field)),
                Ty::Infer(id) if env.infer_constraint(id).is_none() => Err(format!(
                    "type error: field access '{}' on value of unknown type",
                    field
                )),
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
            // Store block/branch bodies for deferred re-checking at call sites.
            // Remove stale entries when a name is rebound to a non-block value.
            match value.as_ref() {
                MirKind::Block(_) | MirKind::BranchBlock(_) => {
                    env.block_bodies.insert(name.clone(), value.clone());
                }
                MirKind::Struct(fields) => {
                    env.block_bodies.remove(name);
                    store_struct_field_bodies(name, fields, env);
                }
                MirKind::Array(elems) => {
                    // Store a representative block body from the first block element.
                    // When .get() returns an element, we can re-check it at the call site.
                    env.block_bodies.remove(name);
                    for elem in elems {
                        match elem.as_ref() {
                            MirKind::Block(_) | MirKind::BranchBlock(_) => {
                                let key = format!("\0arr:{}", name);
                                env.block_bodies.insert(key, elem.clone());
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                MirKind::Ident(other_name) => {
                    // Alias: `let g = f` — copy block body if available
                    env.block_bodies.remove(name);
                    if let Some(body_mir) = env.block_bodies.get(other_name).cloned() {
                        env.block_bodies.insert(name.clone(), body_mir);
                    }
                    // Also copy struct field block bodies ("other.field" → "name.field")
                    let prefix = format!("{}.", other_name);
                    let copies: Vec<(String, Mir)> = env
                        .block_bodies
                        .iter()
                        .filter(|(k, _)| k.starts_with(&prefix))
                        .map(|(k, v)| {
                            let suffix = &k[prefix.len()..];
                            (format!("{}.{}", name, suffix), v.clone())
                        })
                        .collect();
                    for (k, v) in copies {
                        env.block_bodies.insert(k, v);
                    }
                    // Copy array element block body if aliasing an array
                    let arr_key = format!("\0arr:{}", other_name);
                    if let Some(arr_body) = env.block_bodies.get(&arr_key).cloned() {
                        env.block_bodies.insert(format!("\0arr:{}", name), arr_body);
                    }
                }
                MirKind::MethodCall { receiver, method, .. } if method == "get" => {
                    // .get() on an array — propagate the array's element block body
                    env.block_bodies.remove(name);
                    if let MirKind::Ident(arr_name) = receiver.as_ref() {
                        let arr_key = format!("\0arr:{}", arr_name);
                        if let Some(arr_body) = env.block_bodies.get(&arr_key).cloned() {
                            env.block_bodies.insert(name.clone(), arr_body);
                        }
                    }
                }
                _ => {
                    env.block_bodies.remove(name);
                }
            }
            let val_ty = check(value, env)?;
            let val_ty = env.default_constrained_infer_in_fn(&val_ty);
            env.bind(name.clone(), val_ty);
            let body_ty = check(body, env)?;
            env.pop_binding();
            Ok(body_ty)
        }

        // ── Pipe (for let/letarray/apply that weren't desugared) ──
        MirKind::Pipe(lhs, rhs) => check_pipe(lhs, rhs, env),

        // ── Let (pattern destructuring — standalone, not via pipe) ──
        MirKind::Let { pattern, body } => {
            let input = env.fresh_infer();
            check_let(pattern, body, &input, env)
        }

        // ── Call ──
        MirKind::Call(func, arg) => check_call(func, arg, env),

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
                _ => Err("type error: apply expects a method set".to_string()),
            }
        }

        // ── NewTag ──
        MirKind::NewTag(id, _name) => Ok(Ty::TagConstructor(*id)),

        // ── Block (lambda) ──
        MirKind::Block(body) => {
            let param = env.fresh_infer();
            env.bind("in".to_string(), param.clone());
            let body_ty = check(body, env)?;
            env.pop_binding();
            Ok(Ty::Fn {
                param: Box::new(param),
                ret: Box::new(body_ty),
            })
        }

        // ── BranchBlock (pattern-matching lambda) ──
        // When not called via check_call, input type is not yet known.
        MirKind::BranchBlock(arms) => {
            let input_ty = env.fresh_infer();
            env.bind("in".to_string(), input_ty.clone());
            let mut result_ty: Option<Ty> = None;
            for arm in arms {
                let bindings_added = bind_branch_pattern(&arm.pattern, &input_ty, env)?;
                if let Some(guard) = &arm.guard {
                    let _ = check(guard, env)?;
                }
                let arm_ty = check(&arm.body, env)?;
                for _ in 0..bindings_added {
                    env.pop_binding();
                }
                result_ty = Some(match result_ty {
                    None => arm_ty,
                    Some(prev) => unify(&prev, &arm_ty, env)?,
                });
            }
            env.pop_binding(); // pop "in"
            let ret = result_ty.unwrap_or_else(|| env.fresh_infer());
            Ok(Ty::Fn {
                param: Box::new(input_ty),
                ret: Box::new(ret),
            })
        }

        // ── Array ──
        MirKind::Array(elems) => {
            let mut elem_ty: Option<Ty> = None;
            for elem in elems {
                let ty = check(elem, env)?;
                elem_ty = Some(match elem_ty {
                    None => ty,
                    Some(prev) => unify(&prev, &ty, env)?,
                });
            }
            Ok(Ty::Array(Box::new(elem_ty.unwrap_or_else(|| env.fresh_infer()))))
        }

        // ── LetArray (standalone, not via pipe) ──
        MirKind::LetArray { patterns, body } => {
            let input = env.fresh_infer();
            check_let_array(patterns, body, &input, env)
        }

        // ── MethodCall ──
        MirKind::MethodCall {
            receiver,
            method,
            arg,
        } => {
            let recv_ty = check(receiver, env)?;

            // Stage 1: struct field access (method stored as field)
            if let Ty::Struct(fields) = &recv_ty {
                if let Some((_, field_ty)) = fields.iter().find(|(n, _)| n == method) {
                    return match field_ty {
                        Ty::MethodSetConstructor => check_method_set_call(arg, env),
                        _ => {
                            // Check for stored block body (struct field function)
                            let block_body = find_field_block_body(receiver, method, env);
                            if let Some(stored_body) = block_body {
                                let arg_ty = check(arg, env)?;
                                return recheck_block_body(&stored_body, arg_ty, arg, env);
                            }
                            let arg_ty = check(arg, env)?;
                            match field_ty {
                                Ty::Fn { param, ret } => {
                                    unify(&arg_ty, param, env)?;
                                    Ok(env.resolve(ret))
                                }
                                Ty::Infer(id) if env.infer_constraint(*id).is_none() => {
                                    Ok(env.fresh_infer())
                                }
                                _ => Err(format!(
                                    "type error: cannot call non-function field '{}'",
                                    method
                                )),
                            }
                        }
                    };
                }
            }

            // Stage 2: method set lookup
            if let Some(tag_id) = ty_to_tag_id(&recv_ty, env) {
                if let Some((ms_id, method_ty)) = env.find_method_type_with_id(tag_id, method) {
                    // Check if we have a stored block body for this method
                    let ms_key = format!("\0ms{}.{}", ms_id, method);
                    if let Some(stored_body) = env.block_bodies.get(&ms_key).cloned() {
                        // Re-check the method body with the actual receiver as arg
                        let arg_ty = check(arg, env)?;
                        let actual_arg = prepend_arg_ty(&recv_ty, &arg_ty);
                        return match stored_body.as_ref() {
                            MirKind::Block(body) => {
                                env.bind("in".to_string(), actual_arg);
                                let body_ty = check(body, env)?;
                                env.pop_binding();
                                Ok(body_ty)
                            }
                            MirKind::BranchBlock(arms) => {
                                check_branch_block_with_input(arms, &actual_arg, env)
                            }
                            _ => unreachable!("block_bodies only stores Block/BranchBlock"),
                        };
                    }

                    let ret = match &method_ty {
                        Ty::Fn { param, ret } => {
                            // Phase 1: Check arg with blocks deferred (placeholder types).
                            let arg_ty = check_arg_defer_blocks(arg, env)?;
                            let actual_prepended = prepend_arg_ty(&recv_ty, &arg_ty);
                            let mut subst = HashMap::new();
                            unify_with_generics(&actual_prepended, param, &mut subst, env)
                                .map_err(|e| format!("type error in .{}(): {}", method, e))?;

                            // Phase 2: Re-check deferred blocks with resolved param types.
                            let refined = recheck_callback_args(arg, param, &subst, env)?;
                            if let Some(refined_arg_ty) = refined {
                                let refined_prepended = prepend_arg_ty(&recv_ty, &refined_arg_ty);
                                subst.clear();
                                unify_with_generics(&refined_prepended, param, &mut subst, env)
                                    .map_err(|e| format!("type error in .{}(): {}", method, e))?;
                            }

                            substitute_generics(ret, &subst)
                        }
                        _ => {
                            return Err(format!(
                                "type error: method '{}' is not a function",
                                method
                            ));
                        }
                    };
                    return Ok(env.resolve(&ret));
                }
            }

            // Unconstrained Infer: receiver type not yet known — check arg but return Infer.
            // The block will be re-checked with a concrete type at the call site.
            // Constrained Infer (IntLiteral/FloatLiteral) should NOT use this path —
            // they have a known tag_id and should have found a method set above.
            if let Ty::Infer(id) = &recv_ty {
                if env.infer_constraint(*id).is_none() {
                    let _arg_ty = check(arg, env)?;
                    return Ok(env.fresh_infer());
                }
            }

            // Fallback: built-in comparison for types without explicit method sets.
            // The runtime supports eq/not_eq on structs, tags, tag constructors,
            // and arrays, plus ordering on primitives — all via eval_compare.
            let _arg_ty = check(arg, env)?;
            if matches!(
                method.as_str(),
                "eq" | "not_eq" | "lt" | "gt" | "lt_eq" | "gt_eq"
            ) {
                return Ok(Ty::Bool);
            }

            Err(format!(
                "type error: no method '{}' on type {:?}",
                method, recv_ty
            ))
        }
    }
}

/// Check a method arg, but give Block/BranchBlock nodes placeholder types.
/// This avoids checking block bodies before we know their input type.
fn check_arg_defer_blocks(arg: &Mir, env: &mut TyEnv) -> Result<Ty, String> {
    match arg.as_ref() {
        MirKind::Block(_) => Ok(Ty::Fn {
            param: Box::new(env.fresh_infer()),
            ret: Box::new(env.fresh_infer()),
        }),
        MirKind::BranchBlock(_) => Ok(Ty::Fn {
            param: Box::new(env.fresh_infer()),
            ret: Box::new(env.fresh_infer()),
        }),
        MirKind::Struct(fields) => {
            let mut typed_fields = Vec::new();
            let mut positional_idx = 0u64;
            for field in fields {
                let ty = check_arg_defer_blocks(&field.value, env)?;
                if field.is_spread {
                    match &ty {
                        Ty::Struct(spread_fields) => {
                            for (name, fty) in spread_fields {
                                if name.parse::<u64>().is_ok() {
                                    typed_fields.push((positional_idx.to_string(), fty.clone()));
                                    positional_idx += 1;
                                } else {
                                    typed_fields.push((name.clone(), fty.clone()));
                                }
                            }
                        }
                        Ty::Unit => {}
                        _ => {
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
        // For non-block, non-struct args, check normally
        _ => check(arg, env),
    }
}

/// Bidirectional inference for method call callbacks.
/// After generic substitution resolves the expected param types for callback arguments,
/// re-check any Block/BranchBlock args with the resolved param type bound to `in`.
/// Returns Some(refined_arg_ty) if any callbacks were re-checked, None otherwise.
fn recheck_callback_args(
    arg_mir: &Mir,
    method_param: &Ty,
    subst: &HashMap<u64, Ty>,
    env: &mut TyEnv,
) -> Result<Option<Ty>, String> {
    // The method_param (after prepend) is either:
    // - A primitive type (unary method like len) → no callbacks
    // - Struct([("0", recv), ("1", arg1), ...]) → callbacks are Fn-typed fields at index > 0

    let param_fields = match method_param {
        Ty::Struct(fields) => fields,
        _ => return Ok(None),
    };

    // Find Fn-typed fields in the param (skip "0" which is the receiver)
    let mut callback_positions: Vec<(String, Ty)> = Vec::new();
    for (label, ty) in param_fields {
        if label == "0" {
            continue; // skip receiver
        }
        let resolved = substitute_generics(ty, subst);
        if matches!(resolved, Ty::Fn { .. }) {
            callback_positions.push((label.clone(), resolved));
        }
    }

    if callback_positions.is_empty() {
        return Ok(None);
    }

    // Map param field labels back to MIR arg positions.
    // Prepend shifts positional indices by 1: param "1" → MIR field 0, param "2" → MIR field 1, etc.
    // Named fields keep their labels.
    let mut any_refined = false;
    let mut refined_field_types: Vec<(String, Ty)> = Vec::new();

    // Build refined types for all arg fields
    match arg_mir.as_ref() {
        MirKind::Struct(fields) => {
            let mut pos_idx = 0u64;
            for field in fields {
                let label = match &field.label {
                    Some(l) => l.clone(),
                    None => {
                        let l = pos_idx.to_string();
                        pos_idx += 1;
                        l
                    }
                };
                // The param label for this field: positional fields are shifted by 1
                let param_label = if let Ok(n) = label.parse::<u64>() {
                    (n + 1).to_string()
                } else {
                    label.clone()
                };

                if let Some((
                    _,
                    Ty::Fn {
                        param: cb_param, ..
                    },
                )) = callback_positions.iter().find(|(l, _)| *l == param_label)
                {
                    // This field is a callback — try bidirectional re-check
                    if let Some(refined_ty) = recheck_single_callback(&field.value, cb_param, env)?
                    {
                        refined_field_types.push((label, refined_ty));
                        any_refined = true;
                        continue;
                    }
                }
                // Not a callback or couldn't re-check — use original type
                let ty = check(&field.value, env)?;
                refined_field_types.push((label, ty));
            }
        }
        _ => {
            // Single arg (not a struct) — check if it's a callback
            // param label would be "1" (prepend shifts single arg to position 1)
            if let Some((
                _,
                Ty::Fn {
                    param: cb_param, ..
                },
            )) = callback_positions.iter().find(|(l, _)| l == "1")
            {
                if let Some(refined_ty) = recheck_single_callback(arg_mir, cb_param, env)? {
                    return Ok(Some(refined_ty));
                }
            }
            return Ok(None);
        }
    }

    if any_refined {
        Ok(Some(Ty::Struct(refined_field_types)))
    } else {
        Ok(None)
    }
}

/// Re-check a single Block or BranchBlock with a known input type.
/// Returns Some(Fn { param, ret }) with the refined return type, or None if not a block.
fn recheck_single_callback(
    mir: &Mir,
    expected_param: &Ty,
    env: &mut TyEnv,
) -> Result<Option<Ty>, String> {
    match mir.as_ref() {
        MirKind::Block(body) => {
            env.bind("in".to_string(), expected_param.clone());
            let body_ty = check(body, env)?;
            env.pop_binding();
            Ok(Some(Ty::Fn {
                param: expected_param.clone().into(),
                ret: Box::new(body_ty),
            }))
        }
        MirKind::BranchBlock(arms) => {
            let ret_ty = check_branch_block_with_input(arms, expected_param, env)?;
            Ok(Some(Ty::Fn {
                param: expected_param.clone().into(),
                ret: Box::new(ret_ty),
            }))
        }
        _ => Ok(None),
    }
}

fn check_pipe(lhs: &Mir, rhs: &Mir, env: &mut TyEnv) -> Result<Ty, std::string::String> {
    // Store block/branch bodies for deferred re-checking at call sites.
    // When `{ body } >> let(name); ...`, store the block under `name`
    // so that `name(arg)` can re-check the body with a known arg type.
    if let MirKind::Let { pattern, .. } = rhs.as_ref() {
        match pattern {
            Pattern::Name(name) => {
                match lhs.as_ref() {
                    MirKind::Block(_) | MirKind::BranchBlock(_) => {
                        env.block_bodies.insert(name.clone(), lhs.clone());
                    }
                    MirKind::Struct(fields) => {
                        env.block_bodies.remove(name);
                        store_struct_field_bodies(name, fields, env);
                    }
                    MirKind::Array(elems) => {
                        // Store a representative block body from the first block element.
                        // When .get() returns an element, we can re-check it at the call site.
                        env.block_bodies.remove(name);
                        for elem in elems {
                            match elem.as_ref() {
                                MirKind::Block(_) | MirKind::BranchBlock(_) => {
                                    let key = format!("\0arr:{}", name);
                                    env.block_bodies.insert(key, elem.clone());
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    MirKind::Ident(other) => {
                        // Alias: propagate block body from the other name
                        env.block_bodies.remove(name);
                        if let Some(body) = env.block_bodies.get(other).cloned() {
                            env.block_bodies.insert(name.clone(), body);
                        }
                        // Copy array element block body if aliasing an array
                        let arr_key = format!("\0arr:{}", other);
                        if let Some(arr_body) = env.block_bodies.get(&arr_key).cloned() {
                            env.block_bodies.insert(format!("\0arr:{}", name), arr_body);
                        }
                    }
                    MirKind::MethodCall { receiver, method, .. } if method == "get" => {
                        // .get() on an array — propagate the array's element block body
                        env.block_bodies.remove(name);
                        if let MirKind::Ident(arr_name) = receiver.as_ref() {
                            let arr_key = format!("\0arr:{}", arr_name);
                            if let Some(arr_body) = env.block_bodies.get(&arr_key).cloned() {
                                env.block_bodies.insert(name.clone(), arr_body);
                            }
                        }
                    }
                    _ => {
                        // Try to resolve the LHS MIR to find an underlying block
                        env.block_bodies.remove(name);
                        if let Some(resolved) = resolve_to_block_mir(lhs, env) {
                            env.block_bodies.insert(name.clone(), resolved);
                        }
                    }
                }
            }
            Pattern::Fields(pat_fields) => {
                // Propagate block bodies from struct LHS fields to destructured names.
                propagate_block_bodies_to_fields(lhs, pat_fields, env);
            }
            _ => {}
        }
    }
    let lhs_ty = check(lhs, env)?;
    let lhs_ty = env.default_constrained_infer_in_fn(&lhs_ty);
    match rhs.as_ref() {
        // Pipe into let: `expr >> let(pattern); body`
        // The lhs value is bound to the pattern.
        // Also bind \0 (passthrough variable) like eval_pipe does.
        MirKind::Let { pattern, body } => check_let_with_passthrough(pattern, body, &lhs_ty, env),
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
                _ => Err("type error: apply expects a method set".to_string()),
            }
        }
        // All other pipe RHS patterns are lowered to Call/MethodCall by MIR.
        _ => unreachable!("pipe RHS should be lowered to Call/MethodCall by MIR"),
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
            // Determine bind_by_name vs positional, mirroring eval::bind_pattern
            let struct_fields = match input_ty {
                Ty::Struct(f) => f.as_slice(),
                Ty::Unit => &[][..],
                _ => &[][..],
            };
            let has_explicit_labels = fields.iter().any(|f| f.label.is_some());
            let unlabeled_fields: Vec<&crate::ast::PatField> = fields
                .iter()
                .filter(|f| !f.is_rest && f.label.is_none() && f.binding != "_")
                .collect();
            let bind_by_name = if has_explicit_labels {
                true
            } else if unlabeled_fields.is_empty() {
                struct_fields.iter().any(|(l, _)| l.parse::<u64>().is_err())
            } else {
                let all_match = unlabeled_fields.iter().all(|pf| {
                    struct_fields.iter().any(|(l, _)| l == &pf.binding)
                });
                if all_match {
                    true
                } else {
                    // No name match — bind positionally
                    false
                }
            };

            let mut bindings_added = 0usize;
            let mut positional_idx = 0u64;
            let mut used_keys = Vec::new();
            for field in fields {
                if field.binding == "_" && !field.is_rest {
                    // Discard — don't bind, but consume positional index
                    if field.label.is_none() {
                        if bind_by_name {
                            // consume any unused field
                        } else {
                            let key = positional_idx.to_string();
                            used_keys.push(key);
                            positional_idx += 1;
                        }
                    }
                    continue;
                }
                if field.is_rest {
                    // Rest pattern — compute remaining fields from the input struct
                    if field.binding != "_" && !field.binding.is_empty() {
                        let rest_ty = match input_ty {
                            Ty::Struct(sf) => {
                                let remaining: Vec<(std::string::String, Ty)> = sf
                                    .iter()
                                    .filter(|(n, _)| !used_keys.contains(n))
                                    .cloned()
                                    .collect();
                                if remaining.is_empty() {
                                    Ty::Unit
                                } else {
                                    // Re-index positional fields starting from 0
                                    let mut re_pos = 0u64;
                                    let remaining = remaining
                                        .into_iter()
                                        .map(|(n, ty)| {
                                            if n.parse::<u64>().is_ok() {
                                                let new_n = re_pos.to_string();
                                                re_pos += 1;
                                                (new_n, ty)
                                            } else {
                                                (n, ty)
                                            }
                                        })
                                        .collect();
                                    Ty::Struct(remaining)
                                }
                            }
                            Ty::Unit => Ty::Unit,
                            _ => env.fresh_infer(),
                        };
                        env.bind(field.binding.clone(), rest_ty);
                        bindings_added += 1;
                    }
                    continue;
                }
                // Look up field type from input struct/unit
                let field_ty = match input_ty {
                    Ty::Struct(sf) => {
                        let lookup_key = if let Some(label) = &field.label {
                            label.clone()
                        } else if bind_by_name {
                            field.binding.clone()
                        } else {
                            let key = positional_idx.to_string();
                            positional_idx += 1;
                            key
                        };
                        used_keys.push(lookup_key.clone());
                        sf.iter()
                            .find(|(n, _)| *n == lookup_key)
                            .map(|(_, ty)| ty.clone())
                            .unwrap_or_else(|| env.fresh_infer())
                    }
                    _ => {
                        if field.label.is_none() && !bind_by_name {
                            positional_idx += 1;
                        }
                        env.fresh_infer()
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
    // Extract element type and rest type from input
    let (elem_ty, rest_ty) = match input_ty {
        Ty::Array(elem) => (elem.as_ref().clone(), Ty::Array(elem.clone())),
        Ty::String => (Ty::String, Ty::String),
        _ => {
            let e = env.fresh_infer();
            (e.clone(), Ty::Array(Box::new(e)))
        }
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
                env.bind(name.clone(), rest_ty.clone());
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
    // Bidirectional: when calling a Block or BranchBlock, check the arg first
    // so we can bind `in` to the arg's type instead of Unknown.
    match func.as_ref() {
        MirKind::Block(_) | MirKind::BranchBlock(_) => {
            let arg_ty = check(arg, env)?;
            recheck_block_body(func, arg_ty, arg, env)
        }
        // Ident referring to a stored block/branch — re-check with known arg type
        MirKind::Ident(name) if env.block_bodies.contains_key(name) => {
            let stored_body = env.block_bodies.get(name).unwrap().clone();
            let arg_ty = check(arg, env)?;
            recheck_block_body(&stored_body, arg_ty, arg, env)
        }
        // FieldAccess on a struct where the field has a stored block body
        MirKind::FieldAccess(expr, field) => {
            if let MirKind::Ident(struct_name) = expr.as_ref() {
                let key = format!("{}.{}", struct_name, field);
                if let Some(stored_body) = env.block_bodies.get(&key).cloned() {
                    let arg_ty = check(arg, env)?;
                    return recheck_block_body(&stored_body, arg_ty, arg, env);
                }
            }
            // Fall through to general case
            let func_ty = check(func, env)?;
            let arg_ty = check(arg, env)?;
            check_call_by_type(func_ty, arg_ty, arg, env)
        }
        _ => {
            let func_ty = check(func, env)?;
            let arg_ty = check(arg, env)?;
            check_call_by_type(func_ty, arg_ty, arg, env)
        }
    }
}

/// Given a resolved function type and arg type, produce the call result.
/// Unconstrained Infer is allowed (type not yet known); constrained Infer
/// (IntLiteral/FloatLiteral) is treated as a concrete non-function type.
fn check_call_by_type(func_ty: Ty, arg_ty: Ty, arg: &Mir, env: &mut TyEnv) -> Result<Ty, String> {
    match func_ty {
        Ty::Fn { param, ret } => {
            let mut subst = HashMap::new();
            unify_with_generics(&arg_ty, &param, &mut subst, env)?;
            let ret = substitute_generics(&ret, &subst);
            Ok(env.resolve(&ret))
        }
        Ty::TagConstructor(tag_id) => Ok(Ty::Tagged {
            tag_id,
            payload: Box::new(arg_ty),
        }),
        Ty::MethodSetConstructor => check_method_set_call(arg, env),
        Ty::Infer(id) if env.infer_constraint(id).is_none() => Ok(env.fresh_infer()),
        _ => Err(format!("type error: cannot call non-function")),
    }
}

/// Check a BranchBlock with a known input type (from bidirectional Call inference).
/// Binds `in` to the input type and unifies all arm body types.
fn check_branch_block_with_input(
    arms: &[crate::mir::MirBranchArm],
    input_ty: &Ty,
    env: &mut TyEnv,
) -> Result<Ty, std::string::String> {
    // Per D5: when the input type is known (non-Infer), check for undefined tags
    // and exhaustiveness. Constrained Infer (IntLiteral/FloatLiteral) counts as
    // known — only unconstrained Infer is truly unknown.
    let is_unconstrained_infer = matches!(input_ty, Ty::Infer(id) if env.infer_constraint(*id).is_none());
    if !is_unconstrained_infer {
        // First: check for undefined tag names in patterns
        for arm in arms {
            if let crate::mir::MirBranchPattern::Tag(tag_name, _) = &arm.pattern {
                if !matches!(env.get(tag_name), Some(Ty::TagConstructor(_))) {
                    return Err(format!(
                        "type error: undefined tag '{}' in branch pattern",
                        tag_name
                    ));
                }
            }
        }
        // Second: check exhaustiveness — at least one arm must be able to match
        let any_can_match = arms.iter().any(|arm| arm_pattern_can_match(&arm.pattern, input_ty, env));
        if !any_can_match {
            let tag_name = arms.iter().find_map(|arm| {
                if let crate::mir::MirBranchPattern::Tag(name, _) = &arm.pattern {
                    Some(name.as_str())
                } else {
                    None
                }
            }).unwrap_or("?");
            return Err(format!(
                "type error: non-exhaustive branch — no arm matches input type (first tag: '{}')",
                tag_name
            ));
        }
    }

    env.bind("in".to_string(), input_ty.clone());
    let mut result_ty: Option<Ty> = None;
    for arm in arms {
        let bindings_added = bind_branch_pattern(&arm.pattern, input_ty, env)?;
        if let Some(guard) = &arm.guard {
            let _ = check(guard, env)?;
        }
        let arm_ty = check(&arm.body, env)?;
        for _ in 0..bindings_added {
            env.pop_binding();
        }
        result_ty = Some(match result_ty {
            None => arm_ty,
            Some(prev) => unify(&prev, &arm_ty, env)?,
        });
    }
    env.pop_binding(); // pop "in"
    Ok(result_ty.unwrap_or_else(|| env.fresh_infer()))
}

/// Returns true if a branch pattern can potentially match the given input type.
fn arm_pattern_can_match(pattern: &crate::mir::MirBranchPattern, input_ty: &Ty, env: &TyEnv) -> bool {
    match pattern {
        crate::mir::MirBranchPattern::Binding(_) => true,
        crate::mir::MirBranchPattern::Discard => true,
        crate::mir::MirBranchPattern::Literal(_) => true,
        crate::mir::MirBranchPattern::Tag(tag_name, _) => {
            let tag_id = match env.get(tag_name) {
                Some(Ty::TagConstructor(id)) => *id,
                _ => return false,
            };
            extract_payload_for_tag(tag_id, input_ty).is_some()
        }
    }
}

/// Bind variables introduced by a branch pattern. Returns the number of bindings added.
/// When input_ty is known (non-Infer), errors on undefined tags or tag/type mismatches.
fn bind_branch_pattern(
    pattern: &MirBranchPattern,
    input_ty: &Ty,
    env: &mut TyEnv,
) -> Result<usize, String> {
    match pattern {
        MirBranchPattern::Literal(_) => Ok(0),
        MirBranchPattern::Tag(tag_name, binding) => {
            let input_is_infer = matches!(input_ty, Ty::Infer(_));
            if !input_is_infer {
                // Check that the tag name is defined in scope
                match env.get(tag_name) {
                    Some(Ty::TagConstructor(_)) => {}
                    _ => {
                        return Err(format!(
                            "type error: undefined tag '{}' in branch pattern",
                            tag_name
                        ))
                    }
                }
            }
            match binding {
                Some(BranchBinding::Name(n)) => {
                    let payload_ty = resolve_tag_payload(tag_name, input_ty, env)
                        .unwrap_or_else(|| env.fresh_infer());
                    env.bind(n.clone(), payload_ty);
                    Ok(1)
                }
                _ => Ok(0),
            }
        }
        MirBranchPattern::Binding(n) => {
            // The parser stores bare identifiers as Binding — but they might be
            // tag names without payload (e.g., `None -> ...`). Check the env first.
            if matches!(env.get(n), Some(Ty::TagConstructor(_))) {
                // It's a tag pattern with no payload — don't bind, just match.
                Ok(0)
            } else {
                // Catch-all binding gets the input type
                env.bind(n.clone(), input_ty.clone());
                Ok(1)
            }
        }
        MirBranchPattern::Discard => Ok(0),
    }
}

/// Resolve the payload type for a tag pattern by looking up the tag name
/// in the environment and matching against the input type (Tagged or Union).
fn resolve_tag_payload(tag_name: &str, input_ty: &Ty, env: &TyEnv) -> Option<Ty> {
    // Step 1: resolve tag name → tag ID via the type environment
    let tag_id = match env.get(tag_name) {
        Some(Ty::TagConstructor(id)) => *id,
        _ => return None,
    };
    // Step 2: extract payload from input type
    extract_payload_for_tag(tag_id, input_ty)
}

/// Extract the payload type for a specific tag ID from a type.
fn extract_payload_for_tag(tag_id: TagId, ty: &Ty) -> Option<Ty> {
    match ty {
        Ty::Tagged {
            tag_id: id,
            payload,
        } if *id == tag_id => Some(*payload.clone()),
        Ty::Union(variants) => {
            for v in variants {
                if let Ty::Tagged {
                    tag_id: id,
                    payload,
                } = v
                {
                    if *id == tag_id {
                        return Some(*payload.clone());
                    }
                }
            }
            None
        }
        _ => None,
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
                    );
                }
            };

            match methods_ty {
                Ty::Struct(_) => {}
                _ => {
                    return Err(
                        "type error: method_set second argument must be a struct of functions"
                            .to_string(),
                    );
                }
            }

            let id = env.fresh_method_set_id();
            env.register_method_set(id, tag_id, methods_ty.clone());

            // Store block bodies from the methods struct for deferred re-checking.
            // The method_set arg MIR is Struct([ctor, methods_struct]).
            // Extract block bodies from methods_struct fields.
            if let MirKind::Struct(arg_fields) = arg.as_ref() {
                if arg_fields.len() == 2 {
                    let methods_mir = &arg_fields[1].value;
                    if let MirKind::Struct(method_fields) = methods_mir.as_ref() {
                        let ms_key = format!("\0ms{}", id);
                        for field in method_fields {
                            if field.is_spread {
                                continue;
                            }
                            if let Some(label) = &field.label {
                                match field.value.as_ref() {
                                    MirKind::Block(_) | MirKind::BranchBlock(_) => {
                                        let key = format!("{}.{}", ms_key, label);
                                        env.block_bodies.insert(key, field.value.clone());
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }

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
        let int_ty = check(&mir(MirKind::Int(42)), &mut env).unwrap();
        assert!(matches!(int_ty, Ty::Infer(id) if env.infer_constraint(id) == Some(&InferConstraint::IntLiteral)));
        let float_ty = check(&mir(MirKind::Float(1.0)), &mut env).unwrap();
        assert!(matches!(float_ty, Ty::Infer(id) if env.infer_constraint(id) == Some(&InferConstraint::FloatLiteral)));
        assert_eq!(
            check(&mir(MirKind::Bool(true)), &mut env).unwrap(),
            Ty::Bool
        );
        assert_eq!(
            check(&mir(MirKind::Str("hi".into())), &mut env).unwrap(),
            Ty::String
        );
        assert_eq!(check(&mir(MirKind::Char('a')), &mut env).unwrap(), Ty::Char);
        assert_eq!(check(&mir(MirKind::Byte(0)), &mut env).unwrap(), Ty::U8);
        assert_eq!(check(&mir(MirKind::Unit), &mut env).unwrap(), Ty::Unit);
    }

    #[test]
    fn ident_lookup() {
        let mut env = TyEnv::new();
        env.bind("x".into(), Ty::I64);
        assert_eq!(
            check(&mir(MirKind::Ident("x".into())), &mut env).unwrap(),
            Ty::I64
        );
    }

    #[test]
    fn ident_undefined() {
        let mut env = TyEnv::new();
        assert!(check(&mir(MirKind::Ident("x".into())), &mut env).is_err());
    }

    #[test]
    fn import_module() {
        let mut env = TyEnv::new().with_module("core", Ty::I64);
        assert_eq!(
            check(&mir(MirKind::Import("core".into())), &mut env).unwrap(),
            Ty::I64
        );
    }

    #[test]
    fn field_access() {
        let mut env = TyEnv::new();
        let struct_ty = Ty::Struct(vec![("x".into(), Ty::I64), ("y".into(), Ty::F64)]);
        env.bind("s".into(), struct_ty);
        let expr = mir(MirKind::FieldAccess(
            mir(MirKind::Ident("s".into())),
            "x".into(),
        ));
        assert_eq!(check(&expr, &mut env).unwrap(), Ty::I64);
    }

    #[test]
    fn field_access_missing() {
        let mut env = TyEnv::new();
        let struct_ty = Ty::Struct(vec![("x".into(), Ty::I64)]);
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
            MirField {
                label: Some("a".into()),
                value: mir(MirKind::Int(1)),
                is_spread: false,
            },
            MirField {
                label: None,
                value: mir(MirKind::Bool(true)),
                is_spread: false,
            },
        ]));
        let ty = check(&expr, &mut env).unwrap();
        if let Ty::Struct(fields) = &ty {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].0, "a");
            assert!(matches!(&fields[0].1, Ty::Infer(id) if env.infer_constraint(*id) == Some(&InferConstraint::IntLiteral)));
            assert_eq!(fields[1], ("0".into(), Ty::Bool));
        } else {
            panic!("expected Struct, got {:?}", ty);
        }
    }

    #[test]
    fn bind_simple() {
        let mut env = TyEnv::new();
        let expr = mir(MirKind::Bind {
            name: "x".into(),
            value: mir(MirKind::Int(42)),
            body: mir(MirKind::Ident("x".into())),
        });
        // Constrained Infer stays as-is — defaulted only at final result level
        let ty = check(&expr, &mut env).unwrap();
        assert!(matches!(ty, Ty::Infer(id) if env.infer_constraint(id) == Some(&InferConstraint::IntLiteral)));
    }

    #[test]
    fn pipe_let_name() {
        // use(core) desugars to: Pipe(Import("core"), Let { Name("core"), body })
        let mut env = TyEnv::new().with_module("test", Ty::I64);
        let expr = mir(MirKind::Pipe(
            mir(MirKind::Import("test".into())),
            mir(MirKind::Let {
                pattern: Pattern::Name("t".into()),
                body: mir(MirKind::Ident("t".into())),
            }),
        ));
        assert_eq!(check(&expr, &mut env).unwrap(), Ty::I64);
    }

    #[test]
    fn method_set_call() {
        let mut env = TyEnv::new();
        env.bind("Int".into(), Ty::TagConstructor(TAG_ID_I64));
        let fn_ty = Ty::Fn {
            param: Box::new(Ty::Infer(0)),
            ret: Box::new(Ty::Infer(0)),
        };
        env.bind("int_add".into(), fn_ty);
        env.bind("method_set".into(), Ty::MethodSetConstructor);

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
        assert!(matches!(ty, Ty::MethodSet { id: 0, tag_id } if tag_id == TAG_ID_I64));
    }

    #[test]
    fn method_set_via_field_access() {
        // std.method_set(Int, (add = int_add)) should work the same as bare method_set(...)
        let mut env = TyEnv::new();
        env.bind("Int".into(), Ty::TagConstructor(TAG_ID_I64));
        let fn_ty = Ty::Fn {
            param: Box::new(Ty::Infer(0)),
            ret: Box::new(Ty::Infer(0)),
        };
        env.bind("int_add".into(), fn_ty);
        // Simulate a module with method_set as a field
        let module_ty = Ty::Struct(vec![("method_set".into(), Ty::MethodSetConstructor)]);
        env.bind("mymod".into(), module_ty);

        // mymod.method_set(Int, (add = int_add))
        let expr = mir(MirKind::Call(
            mir(MirKind::FieldAccess(
                mir(MirKind::Ident("mymod".into())),
                "method_set".into(),
            )),
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
        assert!(matches!(ty, Ty::MethodSet { id: 0, tag_id } if tag_id == TAG_ID_I64));
    }

    #[test]
    fn method_set_aliased() {
        // let ms = method_set; ms(Int, (add = int_add)) should also work
        let mut env = TyEnv::new();
        env.bind("Int".into(), Ty::TagConstructor(TAG_ID_I64));
        let fn_ty = Ty::Fn {
            param: Box::new(Ty::Infer(0)),
            ret: Box::new(Ty::Infer(0)),
        };
        env.bind("int_add".into(), fn_ty);
        // Bind under an arbitrary name
        env.bind("hoge".into(), Ty::MethodSetConstructor);

        // hoge(Int, (add = int_add))
        let expr = mir(MirKind::Call(
            mir(MirKind::Ident("hoge".into())),
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
        assert!(matches!(ty, Ty::MethodSet { id: 0, tag_id } if tag_id == TAG_ID_I64));
    }

    #[test]
    fn method_set_non_constructor_errors() {
        let mut env = TyEnv::new();
        env.bind("not_a_ctor".into(), Ty::I64);
        env.bind("method_set".into(), Ty::MethodSetConstructor);

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
        env.bind("Int".into(), Ty::TagConstructor(TAG_ID_I64));
        env.bind("method_set".into(), Ty::MethodSetConstructor);

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
        let mut env = TyEnv::new();
        assert_eq!(unify(&Ty::I64, &Ty::I64, &mut env).unwrap(), Ty::I64);
        assert_eq!(unify(&Ty::F64, &Ty::F64, &mut env).unwrap(), Ty::F64);
        assert_eq!(unify(&Ty::Bool, &Ty::Bool, &mut env).unwrap(), Ty::Bool);
        assert_eq!(unify(&Ty::String, &Ty::String, &mut env).unwrap(), Ty::String);
    }

    #[test]
    fn unify_different_primitives_error() {
        let mut env = TyEnv::new();
        assert!(unify(&Ty::I64, &Ty::F64, &mut env).is_err());
        assert!(unify(&Ty::I64, &Ty::String, &mut env).is_err());
        assert!(unify(&Ty::Bool, &Ty::U8, &mut env).is_err());
    }

    #[test]
    fn unify_unknown_with_anything() {
        let mut env = TyEnv::new();
        assert_eq!(unify(&Ty::Infer(0), &Ty::I64, &mut env).unwrap(), Ty::I64);
        // Use fresh env per case to avoid prior bindings interfering
        let mut env2 = TyEnv::new();
        assert_eq!(unify(&Ty::F64, &Ty::Infer(0), &mut env2).unwrap(), Ty::F64);
        let mut env3 = TyEnv::new();
        assert_eq!(unify(&Ty::Infer(0), &Ty::Infer(0), &mut env3).unwrap(), Ty::Infer(0));
    }

    #[test]
    fn unify_structs_same_shape() {
        let mut env = TyEnv::new();
        let a = Ty::Struct(vec![("x".into(), Ty::I64), ("y".into(), Ty::F64)]);
        let b = Ty::Struct(vec![("x".into(), Ty::I64), ("y".into(), Ty::F64)]);
        assert_eq!(unify(&a, &b, &mut env).unwrap(), a);
    }

    #[test]
    fn unify_structs_different_field_count() {
        let mut env = TyEnv::new();
        let a = Ty::Struct(vec![("x".into(), Ty::I64)]);
        let b = Ty::Struct(vec![("x".into(), Ty::I64), ("y".into(), Ty::F64)]);
        assert!(unify(&a, &b, &mut env).is_err());
    }

    #[test]
    fn unify_structs_different_field_names() {
        let mut env = TyEnv::new();
        let a = Ty::Struct(vec![("x".into(), Ty::I64)]);
        let b = Ty::Struct(vec![("y".into(), Ty::I64)]);
        assert!(unify(&a, &b, &mut env).is_err());
    }

    #[test]
    fn unify_structs_different_field_types() {
        let mut env = TyEnv::new();
        let a = Ty::Struct(vec![("x".into(), Ty::I64)]);
        let b = Ty::Struct(vec![("x".into(), Ty::F64)]);
        assert!(unify(&a, &b, &mut env).is_err());
    }

    #[test]
    fn unify_method_sets_same_id() {
        let mut env = TyEnv::new();
        let a = Ty::MethodSet {
            id: 0,
            tag_id: TAG_ID_I64,
        };
        let b = Ty::MethodSet {
            id: 0,
            tag_id: TAG_ID_I64,
        };
        assert_eq!(unify(&a, &b, &mut env).unwrap(), a);
    }

    #[test]
    fn unify_method_sets_different_id() {
        let mut env = TyEnv::new();
        let a = Ty::MethodSet {
            id: 0,
            tag_id: TAG_ID_I64,
        };
        let b = Ty::MethodSet {
            id: 1,
            tag_id: TAG_ID_I64,
        };
        assert!(unify(&a, &b, &mut env).is_err());
    }

    #[test]
    fn unify_functions() {
        let mut env = TyEnv::new();
        let a = Ty::Fn {
            param: Box::new(Ty::I64),
            ret: Box::new(Ty::Bool),
        };
        let b = Ty::Fn {
            param: Box::new(Ty::I64),
            ret: Box::new(Ty::Bool),
        };
        assert_eq!(unify(&a, &b, &mut env).unwrap(), a);
    }

    #[test]
    fn unify_functions_unknown_fills() {
        let mut env = TyEnv::new();
        let a = Ty::Fn {
            param: Box::new(Ty::Infer(0)),
            ret: Box::new(Ty::Infer(1)),
        };
        let b = Ty::Fn {
            param: Box::new(Ty::I64),
            ret: Box::new(Ty::Bool),
        };
        assert_eq!(unify(&a, &b, &mut env).unwrap(), b);
    }

    #[test]
    fn unify_int_literal_coercion() {
        let mut env = TyEnv::new();
        // IntLiteral + Int = Int
        let int_lit1 = env.fresh_constrained_infer(InferConstraint::IntLiteral);
        assert_eq!(unify(&int_lit1, &Ty::I64, &mut env).unwrap(), Ty::I64);
        let int_lit2 = env.fresh_constrained_infer(InferConstraint::IntLiteral);
        assert_eq!(unify(&Ty::I64, &int_lit2, &mut env).unwrap(), Ty::I64);
        // IntLiteral + Byte = Byte
        let int_lit3 = env.fresh_constrained_infer(InferConstraint::IntLiteral);
        assert_eq!(unify(&int_lit3, &Ty::U8, &mut env).unwrap(), Ty::U8);
        let int_lit4 = env.fresh_constrained_infer(InferConstraint::IntLiteral);
        assert_eq!(unify(&Ty::U8, &int_lit4, &mut env).unwrap(), Ty::U8);
        // IntLiteral + IntLiteral = Infer with IntLiteral constraint (via union-find)
        let int_lit5 = env.fresh_constrained_infer(InferConstraint::IntLiteral);
        let int_lit6 = env.fresh_constrained_infer(InferConstraint::IntLiteral);
        let result = unify(&int_lit5, &int_lit6, &mut env).unwrap();
        assert!(matches!(result, Ty::Infer(id) if env.infer_constraint(id) == Some(&InferConstraint::IntLiteral)));
        // IntLiteral + Float = error (int and float are distinct)
        let int_lit7 = env.fresh_constrained_infer(InferConstraint::IntLiteral);
        assert!(unify(&int_lit7, &Ty::F64, &mut env).is_err());
        // IntLiteral + FloatLiteral = error
        let int_lit8 = env.fresh_constrained_infer(InferConstraint::IntLiteral);
        let float_lit1 = env.fresh_constrained_infer(InferConstraint::FloatLiteral);
        assert!(unify(&int_lit8, &float_lit1, &mut env).is_err());
        // IntLiteral + String = error
        let int_lit9 = env.fresh_constrained_infer(InferConstraint::IntLiteral);
        assert!(unify(&int_lit9, &Ty::String, &mut env).is_err());
    }

    #[test]
    fn unify_float_literal_coercion() {
        let mut env = TyEnv::new();
        // FloatLiteral + Float = Float
        let float_lit1 = env.fresh_constrained_infer(InferConstraint::FloatLiteral);
        assert_eq!(unify(&float_lit1, &Ty::F64, &mut env).unwrap(), Ty::F64);
        // FloatLiteral + FloatLiteral = Infer with FloatLiteral constraint (via union-find)
        let float_lit2 = env.fresh_constrained_infer(InferConstraint::FloatLiteral);
        let float_lit3 = env.fresh_constrained_infer(InferConstraint::FloatLiteral);
        let result = unify(&float_lit2, &float_lit3, &mut env).unwrap();
        assert!(matches!(result, Ty::Infer(id) if env.infer_constraint(id) == Some(&InferConstraint::FloatLiteral)));
        // FloatLiteral + Int = error (can't coerce float to int)
        let float_lit4 = env.fresh_constrained_infer(InferConstraint::FloatLiteral);
        assert!(unify(&float_lit4, &Ty::I64, &mut env).is_err());
        // FloatLiteral + Byte = error
        let float_lit5 = env.fresh_constrained_infer(InferConstraint::FloatLiteral);
        assert!(unify(&float_lit5, &Ty::U8, &mut env).is_err());
    }

    #[test]
    fn branch_int_literal_coerces_to_byte() {
        use crate::mir::{MirBranchArm, MirBranchPattern};
        let mut env = TyEnv::new();
        // { true -> b'A', false -> 4 }
        // Arm 1: Byte, Arm 2: IntLiteral → coerces to Byte
        let expr = mir(MirKind::BranchBlock(vec![
            MirBranchArm {
                pattern: MirBranchPattern::Literal(mir(MirKind::Bool(true))),
                guard: None,
                body: mir(MirKind::Byte(b'A')),
            },
            MirBranchArm {
                pattern: MirBranchPattern::Literal(mir(MirKind::Bool(false))),
                guard: None,
                body: mir(MirKind::Int(4)),
            },
        ]));
        let ty = check(&expr, &mut env).unwrap();
        match ty {
            Ty::Fn { param, ret } => {
                assert!(matches!(*param, Ty::Infer(_)));
                assert_eq!(*ret, Ty::U8);
            }
            _ => panic!("expected Fn, got {:?}", ty),
        }
    }

    #[test]
    fn array_int_literal_coerces_to_byte() {
        let mut env = TyEnv::new();
        // [b'A', 4, 5] — element 0 is Byte, elements 1-2 are IntLiteral → Array(Byte)
        let expr = mir(MirKind::Array(vec![
            mir(MirKind::Byte(b'A')),
            mir(MirKind::Int(4)),
            mir(MirKind::Int(5)),
        ]));
        let ty = check(&expr, &mut env).unwrap();
        assert_eq!(ty, Ty::Array(Box::new(Ty::U8)));
    }

    #[test]
    fn unify_tagged_different_ids_produces_union() {
        let mut env = TyEnv::new();
        let ok = Ty::Tagged {
            tag_id: 100,
            payload: Box::new(Ty::I64),
        };
        let err = Ty::Tagged {
            tag_id: 101,
            payload: Box::new(Ty::String),
        };
        let result = unify(&ok, &err, &mut env).unwrap();
        assert_eq!(result, Ty::Union(vec![ok, err]));
    }

    #[test]
    fn unify_tagged_same_id_unifies_payload() {
        let mut env = TyEnv::new();
        let a = Ty::Tagged {
            tag_id: 100,
            payload: Box::new(Ty::Infer(0)),
        };
        let b = Ty::Tagged {
            tag_id: 100,
            payload: Box::new(Ty::I64),
        };
        let result = unify(&a, &b, &mut env).unwrap();
        assert_eq!(
            result,
            Ty::Tagged {
                tag_id: 100,
                payload: Box::new(Ty::I64)
            }
        );
    }

    #[test]
    fn unify_union_with_new_tag() {
        let mut env = TyEnv::new();
        let ok = Ty::Tagged {
            tag_id: 100,
            payload: Box::new(Ty::I64),
        };
        let err = Ty::Tagged {
            tag_id: 101,
            payload: Box::new(Ty::String),
        };
        let union = Ty::Union(vec![ok.clone(), err.clone()]);
        let none = Ty::Tagged {
            tag_id: 102,
            payload: Box::new(Ty::Unit),
        };
        let result = unify(&union, &none, &mut env).unwrap();
        assert_eq!(result, Ty::Union(vec![ok, err, none]));
    }

    #[test]
    fn unify_union_with_existing_tag_merges() {
        let mut env = TyEnv::new();
        let ok = Ty::Tagged {
            tag_id: 100,
            payload: Box::new(Ty::Infer(0)),
        };
        let err = Ty::Tagged {
            tag_id: 101,
            payload: Box::new(Ty::String),
        };
        let union = Ty::Union(vec![ok, err.clone()]);
        let ok2 = Ty::Tagged {
            tag_id: 100,
            payload: Box::new(Ty::I64),
        };
        let result = unify(&union, &ok2, &mut env).unwrap();
        let expected_ok = Ty::Tagged {
            tag_id: 100,
            payload: Box::new(Ty::I64),
        };
        assert_eq!(result, Ty::Union(vec![expected_ok, err]));
    }

    #[test]
    fn unify_tagged_with_non_tagged_produces_union() {
        let mut env = TyEnv::new();
        let ok = Ty::Tagged {
            tag_id: 100,
            payload: Box::new(Ty::I64),
        };
        let result = unify(&ok, &Ty::I64, &mut env).unwrap();
        assert_eq!(result, Ty::Union(vec![ok, Ty::I64]));
    }

    // ── Generic type variable tests ──

    #[test]
    fn unify_with_generics_binds_variable() {
        let mut env = TyEnv::new();
        let mut subst = HashMap::new();
        let result = unify_with_generics(&Ty::Generic(0), &Ty::I64, &mut subst, &mut env).unwrap();
        assert_eq!(result, Ty::I64);
        assert_eq!(subst.get(&0), Some(&Ty::I64));
    }

    #[test]
    fn unify_with_generics_consistent_binding() {
        let mut env = TyEnv::new();
        let mut subst = HashMap::new();
        // First bind G0 = Int
        unify_with_generics(&Ty::Generic(0), &Ty::I64, &mut subst, &mut env).unwrap();
        // Second use of G0 must also be Int
        let result = unify_with_generics(&Ty::Generic(0), &Ty::I64, &mut subst, &mut env).unwrap();
        assert_eq!(result, Ty::I64);
    }

    #[test]
    fn unify_with_generics_inconsistent_binding_error() {
        let mut env = TyEnv::new();
        let mut subst = HashMap::new();
        unify_with_generics(&Ty::Generic(0), &Ty::I64, &mut subst, &mut env).unwrap();
        // G0 is already Int, unifying with Float should error
        assert!(unify_with_generics(&Ty::Generic(0), &Ty::F64, &mut subst, &mut env).is_err());
    }

    #[test]
    fn unify_with_generics_two_variables() {
        let mut env = TyEnv::new();
        let mut subst = HashMap::new();
        // G0 = Int, G1 = String
        unify_with_generics(&Ty::Generic(0), &Ty::I64, &mut subst, &mut env).unwrap();
        unify_with_generics(&Ty::Generic(1), &Ty::String, &mut subst, &mut env).unwrap();
        assert_eq!(subst.get(&0), Some(&Ty::I64));
        assert_eq!(subst.get(&1), Some(&Ty::String));
    }

    #[test]
    fn unify_with_generics_in_array() {
        let mut env = TyEnv::new();
        let mut subst = HashMap::new();
        // Array(G0) vs Array(Int) -> G0 = Int
        let a = Ty::Array(Box::new(Ty::Generic(0)));
        let b = Ty::Array(Box::new(Ty::I64));
        let result = unify_with_generics(&a, &b, &mut subst, &mut env).unwrap();
        assert_eq!(result, Ty::Array(Box::new(Ty::I64)));
        assert_eq!(subst.get(&0), Some(&Ty::I64));
    }

    #[test]
    fn unify_with_generics_in_fn() {
        let mut env = TyEnv::new();
        let mut subst = HashMap::new();
        // Fn(G0 -> G1) vs Fn(Int -> String) -> G0=Int, G1=String
        let a = Ty::Fn {
            param: Box::new(Ty::Generic(0)),
            ret: Box::new(Ty::Generic(1)),
        };
        let b = Ty::Fn {
            param: Box::new(Ty::I64),
            ret: Box::new(Ty::String),
        };
        let result = unify_with_generics(&a, &b, &mut subst, &mut env).unwrap();
        assert_eq!(
            result,
            Ty::Fn {
                param: Box::new(Ty::I64),
                ret: Box::new(Ty::String),
            }
        );
        assert_eq!(subst.get(&0), Some(&Ty::I64));
        assert_eq!(subst.get(&1), Some(&Ty::String));
    }

    #[test]
    fn substitute_generics_replaces() {
        let mut subst = HashMap::new();
        subst.insert(0, Ty::I64);
        subst.insert(1, Ty::String);
        // Array(G1) with {1: String} -> Array(String)
        let ty = Ty::Array(Box::new(Ty::Generic(1)));
        assert_eq!(
            substitute_generics(&ty, &subst),
            Ty::Array(Box::new(Ty::String))
        );
    }

    #[test]
    fn substitute_generics_in_fn() {
        let mut subst = HashMap::new();
        subst.insert(0, Ty::I64);
        subst.insert(1, Ty::Bool);
        let ty = Ty::Fn {
            param: Box::new(Ty::Generic(0)),
            ret: Box::new(Ty::Generic(1)),
        };
        assert_eq!(
            substitute_generics(&ty, &subst),
            Ty::Fn {
                param: Box::new(Ty::I64),
                ret: Box::new(Ty::Bool),
            }
        );
    }

    #[test]
    fn substitute_generics_unbound_stays() {
        let subst = HashMap::new();
        let ty = Ty::Generic(99);
        assert_eq!(substitute_generics(&ty, &subst), Ty::Generic(99));
    }

    // ── Block and branch tests ──

    #[test]
    fn block_types_as_fn() {
        let mut env = TyEnv::new();
        // { 42 } is a lambda that returns a constrained Infer (IntLiteral)
        let expr = mir(MirKind::Block(mir(MirKind::Int(42))));
        let ty = check(&expr, &mut env).unwrap();
        match ty {
            Ty::Fn { param, ret } => {
                assert!(matches!(*param, Ty::Infer(_)));
                assert!(matches!(*ret, Ty::Infer(id) if env.infer_constraint(id) == Some(&InferConstraint::IntLiteral)));
            }
            _ => panic!("expected Fn, got {:?}", ty),
        }
    }

    #[test]
    fn branch_same_type_arms() {
        use crate::mir::{MirBranchArm, MirBranchPattern};
        let mut env = TyEnv::new();
        // { true -> 1, false -> 2 } — both arms are constrained Infer (IntLiteral), unifies to Infer with IntLiteral constraint
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
        match ty {
            Ty::Fn { param, ret } => {
                assert!(matches!(*param, Ty::Infer(_)));
                assert!(matches!(*ret, Ty::Infer(id) if env.infer_constraint(id) == Some(&InferConstraint::IntLiteral)));
            }
            _ => panic!("expected Fn, got {:?}", ty),
        }
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
        // { x -> x } — catch-all returns the input type (both param and ret are inferred)
        let expr = mir(MirKind::BranchBlock(vec![MirBranchArm {
            pattern: MirBranchPattern::Binding("x".into()),
            guard: None,
            body: mir(MirKind::Ident("x".into())),
        }]));
        let ty = check(&expr, &mut env).unwrap();
        match ty {
            Ty::Fn { param, ret } => {
                assert!(matches!(*param, Ty::Infer(_)));
                assert!(matches!(*ret, Ty::Infer(_)));
            }
            _ => panic!("expected Fn, got {:?}", ty),
        }
    }

    #[test]
    fn branch_tag_payload_resolution() {
        use crate::mir::MirBranchPattern;
        let mut env = TyEnv::new();
        // Simulate: tag(Ok); tag(Err);
        // Ok and Err are tag constructors with IDs 100 and 101
        env.bind("Ok".into(), Ty::TagConstructor(100));
        env.bind("Err".into(), Ty::TagConstructor(101));

        // Input type: Union(Tagged{Ok, Int}, Tagged{Err, String})
        let input_ty = Ty::Union(vec![
            Ty::Tagged {
                tag_id: 100,
                payload: Box::new(Ty::I64),
            },
            Ty::Tagged {
                tag_id: 101,
                payload: Box::new(Ty::String),
            },
        ]);

        // Test check_branch_block_with_input directly with the union input.
        // Ok(x) should bind x to Int; Err(e) should bind e to String.
        // Arm bodies: x (Int), 0 (IntLiteral) → unifies to Int.
        let ret = check_branch_block_with_input(
            &[
                crate::mir::MirBranchArm {
                    pattern: MirBranchPattern::Tag(
                        "Ok".into(),
                        Some(BranchBinding::Name("x".into())),
                    ),
                    guard: None,
                    body: mir(MirKind::Ident("x".into())),
                },
                crate::mir::MirBranchArm {
                    pattern: MirBranchPattern::Tag(
                        "Err".into(),
                        Some(BranchBinding::Name("e".into())),
                    ),
                    guard: None,
                    body: mir(MirKind::Int(0)),
                },
            ],
            &input_ty,
            &mut env,
        )
        .unwrap();
        // x has type Int (from Ok payload), 0 is Int → unifies to Int
        assert_eq!(ret, Ty::I64);
    }

    #[test]
    fn branch_tag_payload_err_arm_types() {
        use crate::mir::MirBranchPattern;
        let mut env = TyEnv::new();
        env.bind("Ok".into(), Ty::TagConstructor(100));
        env.bind("Err".into(), Ty::TagConstructor(101));

        let input_ty = Ty::Union(vec![
            Ty::Tagged {
                tag_id: 100,
                payload: Box::new(Ty::I64),
            },
            Ty::Tagged {
                tag_id: 101,
                payload: Box::new(Ty::String),
            },
        ]);

        // { Ok(x) -> x, Err(e) -> e }
        // Ok(x) → x is Int, Err(e) → e is String → unification error!
        let result = check_branch_block_with_input(
            &[
                crate::mir::MirBranchArm {
                    pattern: MirBranchPattern::Tag(
                        "Ok".into(),
                        Some(BranchBinding::Name("x".into())),
                    ),
                    guard: None,
                    body: mir(MirKind::Ident("x".into())),
                },
                crate::mir::MirBranchArm {
                    pattern: MirBranchPattern::Tag(
                        "Err".into(),
                        Some(BranchBinding::Name("e".into())),
                    ),
                    guard: None,
                    body: mir(MirKind::Ident("e".into())),
                },
            ],
            &input_ty,
            &mut env,
        );
        assert!(
            result.is_err(),
            "Ok(Int) and Err(String) arm bodies don't unify"
        );
    }

    // ── Q5: Two lexical method_set calls in branches → error ──

    #[test]
    fn q5_two_method_set_calls_in_branch_error() {
        use crate::mir::{MirBranchArm, MirBranchPattern};
        let mut env = TyEnv::new();
        env.bind("Int".into(), Ty::TagConstructor(TAG_ID_I64));
        let fn_ty = Ty::Fn {
            param: Box::new(Ty::Infer(0)),
            ret: Box::new(Ty::Infer(0)),
        };
        env.bind("int_to_string".into(), fn_ty.clone());
        env.bind("int_to_string_other".into(), fn_ty);
        env.bind("method_set".into(), Ty::MethodSetConstructor);

        // Helper to build method_set(Int, (to_string = f))
        let make_ms = |f: &str| {
            mir(MirKind::Call(
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
        assert!(
            result.is_err(),
            "expected error from two different method_set types in branch"
        );
    }

    // ── Q4: One method_set with varying struct arg → OK ──

    #[test]
    fn q4_one_method_set_with_varying_struct_ok() {
        let mut env = TyEnv::new();
        let fn_ty = Ty::Fn {
            param: Box::new(Ty::Infer(0)),
            ret: Box::new(Ty::Infer(0)),
        };
        env.bind("Int".into(), Ty::TagConstructor(TAG_ID_I64));
        env.bind("int_to_string".into(), fn_ty.clone());
        env.bind("int_to_string_other".into(), fn_ty.clone());
        env.bind("method_set".into(), Ty::MethodSetConstructor);

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
        env.bind("Int".into(), Ty::TagConstructor(TAG_ID_I64));
        let int_add_param = env.fresh_infer();
        let int_add_ty = Ty::Fn {
            param: Box::new(int_add_param),
            ret: Box::new(Ty::I64),
        };
        env.bind("int_add".into(), int_add_ty);
        env.bind("method_set".into(), Ty::MethodSetConstructor);

        // Create the method set: method_set(Int, (add = int_add))
        let ms_call = mir(MirKind::Call(
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
        let ms_ty = check(&ms_call, &mut env).unwrap();
        assert!(matches!(ms_ty, Ty::MethodSet { tag_id, .. } if tag_id == TAG_ID_I64));

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
        env2.bind("Int".into(), Ty::TagConstructor(TAG_ID_I64));
        let int_add_param2 = env2.fresh_infer();
        env2.bind(
            "int_add".into(),
            Ty::Fn {
                param: Box::new(int_add_param2),
                ret: Box::new(Ty::I64),
            },
        );
        env2.bind("method_set".into(), Ty::MethodSetConstructor);
        let ty = check(&expr, &mut env2).unwrap();
        assert_eq!(ty, Ty::I64);
    }

    #[test]
    fn method_call_on_struct_field() {
        let mut env = TyEnv::new();
        // A struct with a method-like field: (add = fn(Unknown->Int))
        let struct_ty = Ty::Struct(vec![(
            "add".into(),
            Ty::Fn {
                param: Box::new(Ty::Infer(0)),
                ret: Box::new(Ty::I64),
            },
        )]);
        env.bind("s".into(), struct_ty);
        // s.add(1)
        let expr = mir(MirKind::MethodCall {
            receiver: mir(MirKind::Ident("s".into())),
            method: "add".into(),
            arg: mir(MirKind::Int(1)),
        });
        let ty = check(&expr, &mut env).unwrap();
        assert_eq!(ty, Ty::I64);
    }

    #[test]
    fn call_param_type_mismatch_error() {
        let mut env = TyEnv::new();
        // f: Fn(Int -> Bool)
        env.bind(
            "f".into(),
            Ty::Fn {
                param: Box::new(Ty::I64),
                ret: Box::new(Ty::Bool),
            },
        );
        // f("hello") — String arg doesn't match Int param
        let expr = mir(MirKind::Call(
            mir(MirKind::Ident("f".into())),
            mir(MirKind::Str("hello".into())),
        ));
        assert!(check(&expr, &mut env).is_err());
    }

    #[test]
    fn call_param_type_ok() {
        let mut env = TyEnv::new();
        // f: Fn(Int -> Bool)
        env.bind(
            "f".into(),
            Ty::Fn {
                param: Box::new(Ty::I64),
                ret: Box::new(Ty::Bool),
            },
        );
        // f(42) — IntLiteral coerces to Int
        let expr = mir(MirKind::Call(
            mir(MirKind::Ident("f".into())),
            mir(MirKind::Int(42)),
        ));
        let ty = check(&expr, &mut env).unwrap();
        assert_eq!(ty, Ty::Bool);
    }

    #[test]
    fn struct_rest_pattern_types() {
        use crate::ast::{PatField, Pattern};
        let mut env = TyEnv::new();
        // (x=1, y=2.0, z=true) >> let(x=x, ...rest); rest
        // rest should be Struct([(y, Float), (z, Bool)])
        let input = Ty::Struct(vec![
            ("x".into(), Ty::I64),
            ("y".into(), Ty::F64),
            ("z".into(), Ty::Bool),
        ]);
        let pattern = Pattern::Fields(vec![
            PatField {
                label: Some("x".into()),
                binding: "x".into(),
                is_rest: false,
            },
            PatField {
                label: None,
                binding: "rest".into(),
                is_rest: true,
            },
        ]);
        let body = mir(MirKind::Ident("rest".into()));
        let ty = check_let(&pattern, &body, &input, &mut env).unwrap();
        assert_eq!(
            ty,
            Ty::Struct(vec![("y".into(), Ty::F64), ("z".into(), Ty::Bool)])
        );
    }

    #[test]
    fn method_call_without_method_set_errors() {
        let mut env = TyEnv::new();
        // 42.to_string() without any method set in scope is a type error
        let expr = mir(MirKind::MethodCall {
            receiver: mir(MirKind::Int(42)),
            method: "to_string".into(),
            arg: mir(MirKind::Unit),
        });
        assert!(check(&expr, &mut env).is_err());
    }

    #[test]
    fn array_get_returns_element_type() {
        // [1, 2].get(0) should return IntLiteral (element type), not Unknown
        let mut env = TyEnv::new();
        // array_get: Array(G0) × Int → G0
        let g0 = Ty::Generic(0);
        let get_ty = Ty::Fn {
            param: Box::new(Ty::Struct(vec![
                ("0".into(), Ty::Array(Box::new(g0.clone()))),
                ("1".into(), Ty::I64),
            ])),
            ret: Box::new(g0),
        };
        let ms_id = env.fresh_method_set_id();
        env.register_method_set(
            ms_id,
            TAG_ID_ARRAY,
            Ty::Struct(vec![("get".into(), get_ty)]),
        );
        env.bind(
            "\0ms".to_string(),
            Ty::MethodSet {
                id: ms_id,
                tag_id: TAG_ID_ARRAY,
            },
        );

        let expr = mir(MirKind::MethodCall {
            receiver: mir(MirKind::Array(vec![
                mir(MirKind::Int(1)),
                mir(MirKind::Int(2)),
            ])),
            method: "get".into(),
            arg: mir(MirKind::Int(0)),
        });
        let ty = check(&expr, &mut env).unwrap();
        assert!(matches!(ty, Ty::Infer(id) if env.infer_constraint(id) == Some(&InferConstraint::IntLiteral)));
    }

    #[test]
    fn array_map_returns_mapped_type() {
        let mut env = TyEnv::new();
        // array_map: Array(G0) × (G0 → G1) → Array(G1)
        let g0 = Ty::Generic(0);
        let g1 = Ty::Generic(1);
        let map_cb = Ty::Fn {
            param: Box::new(g0.clone()),
            ret: Box::new(g1.clone()),
        };
        let map_ty = Ty::Fn {
            param: Box::new(Ty::Struct(vec![
                ("0".into(), Ty::Array(Box::new(g0))),
                ("1".into(), map_cb),
            ])),
            ret: Box::new(Ty::Array(Box::new(g1))),
        };
        let ms_id = env.fresh_method_set_id();
        env.register_method_set(
            ms_id,
            TAG_ID_ARRAY,
            Ty::Struct(vec![("map".into(), map_ty)]),
        );
        env.bind(
            "\0ms".to_string(),
            Ty::MethodSet {
                id: ms_id,
                tag_id: TAG_ID_ARRAY,
            },
        );

        // [1, 2].map(f) where f: Any → String
        let callback = Ty::Fn {
            param: Box::new(Ty::Infer(0)),
            ret: Box::new(Ty::String),
        };
        env.bind("f".into(), callback);

        let expr = mir(MirKind::MethodCall {
            receiver: mir(MirKind::Array(vec![
                mir(MirKind::Int(1)),
                mir(MirKind::Int(2)),
            ])),
            method: "map".into(),
            arg: mir(MirKind::Ident("f".into())),
        });
        let ty = check(&expr, &mut env).unwrap();
        assert_eq!(ty, Ty::Array(Box::new(Ty::String)));
    }

    #[test]
    fn array_zip_returns_pair_array() {
        let mut env = TyEnv::new();
        // array_zip: Array(G0) × Array(G1) → Array((G0, G1))
        let g0 = Ty::Generic(0);
        let g1 = Ty::Generic(1);
        let zip_ty = Ty::Fn {
            param: Box::new(Ty::Struct(vec![
                ("0".into(), Ty::Array(Box::new(g0.clone()))),
                ("1".into(), Ty::Array(Box::new(g1.clone()))),
            ])),
            ret: Box::new(Ty::Array(Box::new(Ty::Struct(vec![
                ("0".into(), g0),
                ("1".into(), g1),
            ])))),
        };
        let ms_id = env.fresh_method_set_id();
        env.register_method_set(
            ms_id,
            TAG_ID_ARRAY,
            Ty::Struct(vec![("zip".into(), zip_ty)]),
        );
        env.bind(
            "\0ms".to_string(),
            Ty::MethodSet {
                id: ms_id,
                tag_id: TAG_ID_ARRAY,
            },
        );

        // [1, 2].zip(["a", "b"]) → Array(Struct([(0, IntLiteral), (1, String)]))
        let expr = mir(MirKind::MethodCall {
            receiver: mir(MirKind::Array(vec![
                mir(MirKind::Int(1)),
                mir(MirKind::Int(2)),
            ])),
            method: "zip".into(),
            arg: mir(MirKind::Array(vec![
                mir(MirKind::Str("a".into())),
                mir(MirKind::Str("b".into())),
            ])),
        });
        let ty = check(&expr, &mut env).unwrap();
        if let Ty::Array(elem) = &ty {
            if let Ty::Struct(fields) = elem.as_ref() {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].0, "0");
                assert!(matches!(&fields[0].1, Ty::Infer(id) if env.infer_constraint(*id) == Some(&InferConstraint::IntLiteral)));
                assert_eq!(fields[1], ("1".into(), Ty::String));
            } else {
                panic!("expected Struct element, got {:?}", elem);
            }
        } else {
            panic!("expected Array, got {:?}", ty);
        }
    }

    // ── Literal defaulting tests ──

    #[test]
    fn bind_keeps_literal_in_struct() {
        let mut env = TyEnv::new();
        // let s = (x=1, y=2.0); s
        let expr = mir(MirKind::Bind {
            name: "s".into(),
            value: mir(MirKind::Struct(vec![
                MirField {
                    label: Some("x".into()),
                    value: mir(MirKind::Int(1)),
                    is_spread: false,
                },
                MirField {
                    label: Some("y".into()),
                    value: mir(MirKind::Float(2.0)),
                    is_spread: false,
                },
            ])),
            body: mir(MirKind::Ident("s".into())),
        });
        let ty = check(&expr, &mut env).unwrap();
        // Constrained Infer vars stay as-is in bindings — defaulted only at final result
        if let Ty::Struct(fields) = &ty {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].0, "x");
            assert!(matches!(&fields[0].1, Ty::Infer(id) if env.infer_constraint(*id) == Some(&InferConstraint::IntLiteral)));
            assert_eq!(fields[1].0, "y");
            assert!(matches!(&fields[1].1, Ty::Infer(id) if env.infer_constraint(*id) == Some(&InferConstraint::FloatLiteral)));
        } else {
            panic!("expected Struct, got {:?}", ty);
        }
    }

    #[test]
    fn bind_keeps_literal_in_array() {
        let mut env = TyEnv::new();
        // let a = [1, 2, 3]; a
        let expr = mir(MirKind::Bind {
            name: "a".into(),
            value: mir(MirKind::Array(vec![
                mir(MirKind::Int(1)),
                mir(MirKind::Int(2)),
                mir(MirKind::Int(3)),
            ])),
            body: mir(MirKind::Ident("a".into())),
        });
        let ty = check(&expr, &mut env).unwrap();
        if let Ty::Array(elem) = &ty {
            assert!(matches!(elem.as_ref(), Ty::Infer(id) if env.infer_constraint(*id) == Some(&InferConstraint::IntLiteral)));
        } else {
            panic!("expected Array, got {:?}", ty);
        }
    }

    #[test]
    fn bind_defaults_literal_in_fn() {
        let mut env = TyEnv::new();
        // let f = { 42 }; f
        // Function param/return types get defaulted at binding time
        let expr = mir(MirKind::Bind {
            name: "f".into(),
            value: mir(MirKind::Block(mir(MirKind::Int(42)))),
            body: mir(MirKind::Ident("f".into())),
        });
        let ty = check(&expr, &mut env).unwrap();
        assert_eq!(
            ty,
            Ty::Fn {
                param: Box::new(Ty::Infer(0)),
                ret: Box::new(Ty::I64)
            }
        );
    }

    // ── Deferred block re-checking tests ──

    #[test]
    fn stored_block_rechecked_on_call() {
        let mut env = TyEnv::new();
        // let f = { in }; f(42)
        // Without re-checking, f(42) returns Unknown (from initial block check).
        // With re-checking, f(42) should return Int (from re-checking with in=IntLiteral).
        // But since it goes through default_literals in Bind... the block is stored before defaulting.
        let expr = mir(MirKind::Bind {
            name: "f".into(),
            value: mir(MirKind::Block(mir(MirKind::Ident("in".into())))),
            body: mir(MirKind::Call(
                mir(MirKind::Ident("f".into())),
                mir(MirKind::Int(42)),
            )),
        });
        let ty = check(&expr, &mut env).unwrap();
        assert!(matches!(ty, Ty::Infer(id) if env.infer_constraint(id) == Some(&InferConstraint::IntLiteral)));
    }

    #[test]
    fn stored_block_with_body_expr() {
        let mut env = TyEnv::new();
        env.bind("Int".into(), Ty::TagConstructor(TAG_ID_I64));
        // let f = { Int(in) }; f(42) should return Tagged{Int, IntLiteral}
        let expr = mir(MirKind::Bind {
            name: "f".into(),
            value: mir(MirKind::Block(mir(MirKind::Call(
                mir(MirKind::Ident("Int".into())),
                mir(MirKind::Ident("in".into())),
            )))),
            body: mir(MirKind::Call(
                mir(MirKind::Ident("f".into())),
                mir(MirKind::Int(42)),
            )),
        });
        let ty = check(&expr, &mut env).unwrap();
        if let Ty::Tagged { tag_id, payload } = &ty {
            assert_eq!(*tag_id, TAG_ID_I64);
            assert!(matches!(payload.as_ref(), Ty::Infer(id) if env.infer_constraint(*id) == Some(&InferConstraint::IntLiteral)));
        } else {
            panic!("expected Tagged, got {:?}", ty);
        }
    }

    #[test]
    fn stored_block_cleared_on_rebind() {
        let mut env = TyEnv::new();
        // let f = { in }; let f = 42; f
        // After rebinding f to 42, f should be IntLiteral (no premature defaulting),
        // not a block. And calling f should fail because IntLiteral is not callable.
        let expr = mir(MirKind::Bind {
            name: "f".into(),
            value: mir(MirKind::Block(mir(MirKind::Ident("in".into())))),
            body: mir(MirKind::Bind {
                name: "f".into(),
                value: mir(MirKind::Int(42)),
                body: mir(MirKind::Ident("f".into())),
            }),
        });
        let ty = check(&expr, &mut env).unwrap();
        assert!(matches!(ty, Ty::Infer(id) if env.infer_constraint(id) == Some(&InferConstraint::IntLiteral)));
    }

    // ── prepend_arg_ty tests ──

    #[test]
    fn prepend_arg_ty_unit() {
        // recv.method() — arg is unit, prepend = recv
        assert_eq!(prepend_arg_ty(&Ty::I64, &Ty::Unit), Ty::I64);
    }

    #[test]
    fn prepend_arg_ty_single() {
        // recv.method(x) — arg is single value, prepend = (recv, x)
        assert_eq!(
            prepend_arg_ty(&Ty::I64, &Ty::String),
            Ty::Struct(vec![("0".into(), Ty::I64), ("1".into(), Ty::String)])
        );
    }

    #[test]
    fn prepend_arg_ty_struct() {
        // recv.method(a, b) — arg is struct (0=a, 1=b), prepend = (0=recv, 1=a, 2=b)
        let arg = Ty::Struct(vec![("0".into(), Ty::String), ("1".into(), Ty::Bool)]);
        assert_eq!(
            prepend_arg_ty(&Ty::I64, &arg),
            Ty::Struct(vec![
                ("0".into(), Ty::I64),
                ("1".into(), Ty::String),
                ("2".into(), Ty::Bool),
            ])
        );
    }

    #[test]
    fn prepend_arg_ty_named_struct() {
        // recv.fold(init=0, f=cb) — named fields stay named, recv is prepended as 0
        let arg = Ty::Struct(vec![("init".into(), Ty::I64), ("f".into(), Ty::Infer(0))]);
        assert_eq!(
            prepend_arg_ty(&Ty::Array(Box::new(Ty::I64)), &arg),
            Ty::Struct(vec![
                ("0".into(), Ty::Array(Box::new(Ty::I64))),
                ("init".into(), Ty::I64),
                ("f".into(), Ty::Infer(0)),
            ])
        );
    }

    // ── Tag constructor call with literal ──

    #[test]
    fn tag_constructor_wraps_literal() {
        let mut env = TyEnv::new();
        env.bind("Ok".into(), Ty::TagConstructor(100));
        // Ok(42) should return Tagged{100, constrained Infer (IntLiteral)}
        let expr = mir(MirKind::Call(
            mir(MirKind::Ident("Ok".into())),
            mir(MirKind::Int(42)),
        ));
        let ty = check(&expr, &mut env).unwrap();
        if let Ty::Tagged { tag_id, payload } = &ty {
            assert_eq!(*tag_id, 100);
            assert!(matches!(payload.as_ref(), Ty::Infer(id) if env.infer_constraint(*id) == Some(&InferConstraint::IntLiteral)));
        } else {
            panic!("expected Tagged, got {:?}", ty);
        }
    }

    // ── default_literals function ──

    #[test]
    fn default_literals_recursive() {
        let mut env = TyEnv::new();
        let int_lit = env.fresh_constrained_infer(InferConstraint::IntLiteral);
        let float_lit1 = env.fresh_constrained_infer(InferConstraint::FloatLiteral);
        let int_lit2 = env.fresh_constrained_infer(InferConstraint::IntLiteral);
        let float_lit2 = env.fresh_constrained_infer(InferConstraint::FloatLiteral);
        let ty = Ty::Struct(vec![
            ("a".into(), int_lit),
            ("b".into(), Ty::Array(Box::new(float_lit1))),
            (
                "c".into(),
                Ty::Fn {
                    param: Box::new(int_lit2),
                    ret: Box::new(float_lit2),
                },
            ),
        ]);
        let defaulted = env.default_constrained_infer(&ty);
        assert_eq!(
            defaulted,
            Ty::Struct(vec![
                ("a".into(), Ty::I64),
                ("b".into(), Ty::Array(Box::new(Ty::F64))),
                (
                    "c".into(),
                    Ty::Fn {
                        param: Box::new(Ty::I64),
                        ret: Box::new(Ty::F64),
                    }
                ),
            ])
        );
    }

    #[test]
    fn default_literals_in_tagged() {
        let mut env = TyEnv::new();
        let int_lit = env.fresh_constrained_infer(InferConstraint::IntLiteral);
        let ty = Ty::Tagged {
            tag_id: 100,
            payload: Box::new(int_lit),
        };
        assert_eq!(
            env.default_constrained_infer(&ty),
            Ty::Tagged {
                tag_id: 100,
                payload: Box::new(Ty::I64)
            }
        );
    }

    #[test]
    fn default_literals_in_union() {
        let mut env = TyEnv::new();
        let int_lit = env.fresh_constrained_infer(InferConstraint::IntLiteral);
        let float_lit = env.fresh_constrained_infer(InferConstraint::FloatLiteral);
        let ty = Ty::Union(vec![
            Ty::Tagged {
                tag_id: 100,
                payload: Box::new(int_lit),
            },
            Ty::Tagged {
                tag_id: 101,
                payload: Box::new(float_lit),
            },
        ]);
        assert_eq!(
            env.default_constrained_infer(&ty),
            Ty::Union(vec![
                Ty::Tagged {
                    tag_id: 100,
                    payload: Box::new(Ty::I64)
                },
                Ty::Tagged {
                    tag_id: 101,
                    payload: Box::new(Ty::F64)
                },
            ])
        );
    }

    #[test]
    fn typecheck_std_nana() {
        let source = include_str!("std.nana");
        let ast = crate::parse(source).expect("parse failed");
        let mir = crate::mir::lower(&ast);

        let mut env = TyEnv::new().with_module("core", core_module_type());
        // std.nana does `let method_set = core.method_set` which gets
        // MethodSetConstructor from the core module, but we also bind it
        // here so the name is available before the let-binding is processed.
        env.bind("method_set".into(), Ty::MethodSetConstructor);

        let ty = check(&mir, &mut env).expect("typecheck failed");

        // The result should be a struct with known fields
        match &ty {
            Ty::Struct(fields) => {
                let field_names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
                assert!(field_names.contains(&"I64"));
                assert!(field_names.contains(&"i64_methods"));
                assert!(field_names.contains(&"prelude"));
                assert!(field_names.contains(&"not"));

                // i64_methods should be a MethodSet for I64
                let int_methods = fields.iter().find(|(n, _)| n == "i64_methods").unwrap();
                match &int_methods.1 {
                    Ty::MethodSet { tag_id, .. } => {
                        assert_eq!(*tag_id, TAG_ID_I64);
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
