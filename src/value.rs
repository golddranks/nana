use std::cell::Cell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::mir::{Mir, MirBranchArm};

pub type TagId = u64;

// Reserved TagIds for built-in primitive types (0–17).
// User-generated tag IDs start at 18 (see parser.rs TAG_COUNTER).
pub const TAG_ID_I64: TagId = 0;
pub const TAG_ID_F64: TagId = 1;
pub const TAG_ID_BOOL: TagId = 2;
pub const TAG_ID_STRING: TagId = 3;
pub const TAG_ID_CHAR: TagId = 4;
pub const TAG_ID_U8: TagId = 5;
pub const TAG_ID_ARRAY: TagId = 6;
pub const TAG_ID_UNIT: TagId = 7;
pub const TAG_ID_STRUCT: TagId = 8;
pub const TAG_ID_I32: TagId = 9;
pub const TAG_ID_F32: TagId = 10;
pub const TAG_ID_I8: TagId = 11;
pub const TAG_ID_I16: TagId = 12;
pub const TAG_ID_U16: TagId = 13;
pub const TAG_ID_U32: TagId = 14;
pub const TAG_ID_U64: TagId = 15;
pub const TAG_ID_I128: TagId = 16;
pub const TAG_ID_U128: TagId = 17;

/// Map a primitive Value to its built-in TagId for method set dispatch.
pub fn builtin_tag_id(value: &Value) -> Option<TagId> {
    match value {
        Value::I64(_) => Some(TAG_ID_I64),
        Value::F64(_) => Some(TAG_ID_F64),
        Value::Bool(_) => Some(TAG_ID_BOOL),
        Value::Str(_) => Some(TAG_ID_STRING),
        Value::Char(_) => Some(TAG_ID_CHAR),
        Value::U8(_) => Some(TAG_ID_U8),
        Value::I8(_) => Some(TAG_ID_I8),
        Value::I16(_) => Some(TAG_ID_I16),
        Value::U16(_) => Some(TAG_ID_U16),
        Value::I32(_) => Some(TAG_ID_I32),
        Value::U32(_) => Some(TAG_ID_U32),
        Value::F32(_) => Some(TAG_ID_F32),
        Value::U64(_) => Some(TAG_ID_U64),
        Value::I128(_) => Some(TAG_ID_I128),
        Value::U128(_) => Some(TAG_ID_U128),
        Value::Array(_) => Some(TAG_ID_ARRAY),
        Value::Unit => Some(TAG_ID_UNIT),
        Value::Struct(_) => Some(TAG_ID_STRUCT),
        _ => None,
    }
}

// ── Environment ──────────────────────────────────────────────────

#[derive(Clone)]
struct Binding {
    name: String,
    value: Value,
    used: Rc<Cell<bool>>,
}

#[derive(Clone)]
pub struct Env {
    bindings: Vec<Binding>,
    modules: Rc<HashMap<String, Value>>,
}

impl Env {
    pub fn new() -> Self {
        Env {
            bindings: Vec::new(),
            modules: Rc::new(HashMap::new()),
        }
    }

    pub fn with_modules(modules: HashMap<String, Value>) -> Self {
        Env {
            bindings: Vec::new(),
            modules: Rc::new(modules),
        }
    }

    pub fn get_module(&self, name: &str) -> Option<&Value> {
        self.modules.get(name)
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.bindings.iter().rev().find_map(|b| {
            if b.name == name {
                b.used.set(true);
                Some(&b.value)
            } else {
                None
            }
        })
    }

    pub fn bind(&self, name: String, value: Value) -> Env {
        let mut new_bindings = self.bindings.clone();
        new_bindings.push(Binding {
            name,
            value,
            used: Rc::new(Cell::new(false)),
        });
        Env {
            bindings: new_bindings,
            modules: self.modules.clone(),
        }
    }

    /// Bind a name that is considered pre-used (no unused warning).
    pub fn bind_used(&self, name: String, value: Value) -> Env {
        let mut new_bindings = self.bindings.clone();
        new_bindings.push(Binding {
            name,
            value,
            used: Rc::new(Cell::new(true)),
        });
        Env {
            bindings: new_bindings,
            modules: self.modules.clone(),
        }
    }

    /// Return warnings for unused bindings that don't start with `_`.
    /// Skips builtins and internal names (like `\0`).
    pub fn unused_warnings(&self) -> Vec<String> {
        self.unused_warnings_from(0)
    }

    /// Return warnings for unused bindings starting from the given index.
    /// Used by the REPL to only warn about newly introduced bindings.
    pub fn unused_warnings_from(&self, from: usize) -> Vec<String> {
        let mut warnings = Vec::new();
        for b in self.bindings.iter().skip(from) {
            if b.name.starts_with('\0') || b.name == "_" {
                continue;
            }
            // Builtins are pre-bound and always "used" implicitly
            if matches!(b.value, Value::BuiltinFn(_)) {
                continue;
            }
            if !b.used.get() && !b.name.starts_with('_') {
                warnings.push(format!("warning: unused binding '{}'", b.name));
            }
        }
        warnings
    }

    /// Return the number of bindings in the environment.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Iterate over bindings added after position `from`.
    /// Returns (name, value) pairs for each new binding.
    pub fn bindings_from(&self, from: usize) -> impl Iterator<Item = (&str, &Value)> {
        self.bindings.iter().skip(from).map(|b| (b.name.as_str(), &b.value))
    }

    /// Find a method in active method sets for a given tag ID.
    /// Only considers method sets activated via `apply` (bound with `\0ms` prefix).
    /// Scans backwards (most recent first) for shadowing semantics.
    pub fn find_method_in_method_sets(&self, tag_id: TagId, method_name: &str) -> Option<&Value> {
        for b in self.bindings.iter().rev() {
            if !b.name.starts_with("\0ms") {
                continue;
            }
            if let Value::MethodSet { tag_id: ms_tag_id, methods, .. } = &b.value {
                if *ms_tag_id == tag_id {
                    if let Some((_, func)) = methods.iter().find(|(name, _)| name == method_name) {
                        b.used.set(true);
                        return Some(func);
                    }
                }
            }
        }
        None
    }
}

#[derive(Clone)]
pub enum Value {
    I64(i64),
    F64(f64),
    Bool(bool),
    Str(String),
    Char(char),
    U8(u8),
    I8(i8),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    F32(f32),
    U64(u64),
    I128(i128),
    U128(u128),
    Unit,
    Array(Vec<Value>),
    Struct(Vec<(String, Value)>),
    Closure {
        id: u64,
        body: Mir,
        env: Env,
    },
    BranchClosure {
        id: u64,
        arms: Vec<MirBranchArm>,
        env: Env,
    },
    TagConstructor {
        id: TagId,
        name: String,
    },
    Tagged {
        id: TagId,
        name: String,
        payload: Box<Value>,
    },
    BuiltinFn(String),
    MethodSet {
        id: u64,
        tag_id: TagId,
        methods: Vec<(String, Value)>,
    },
}

impl Value {
    /// For `print`: format the value for human consumption (strings without quotes).
    pub fn print_string(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            other => other.to_string(),
        }
    }

    /// Structural equality: compares function bodies and environments,
    /// not just identity. For non-function values, same as `PartialEq`.
    pub fn val_eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Closure { body: b1, env: e1, .. }, Value::Closure { body: b2, env: e2, .. }) => {
                b1 == b2 && e1 == e2
            }
            (
                Value::BranchClosure { arms: a1, env: e1, .. },
                Value::BranchClosure { arms: a2, env: e2, .. },
            ) => a1 == a2 && e1 == e2,
            _ => self == other,
        }
    }

    /// Write a string value with quotes and escape sequences.
    fn write_quoted_str(s: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"")?;
        for c in s.chars() {
            match c {
                '\0' => write!(f, "\\0")?,
                '\n' => write!(f, "\\n")?,
                '\r' => write!(f, "\\r")?,
                '\t' => write!(f, "\\t")?,
                '\\' => write!(f, "\\\\")?,
                '"' => write!(f, "\\\"")?,
                c => write!(f, "{}", c)?,
            }
        }
        write!(f, "\"")
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::I64(n) => write!(f, "{}", n),
            Value::F64(n) => {
                if n.fract() == 0.0 {
                    write!(f, "{}.0", n)
                } else {
                    write!(f, "{}", n)
                }
            }
            Value::Bool(b) => write!(f, "{}", b),
            Value::Str(s) => Self::write_quoted_str(s, f),
            Value::Char(c) => match c {
                '\0' => write!(f, "'\\0'"),
                '\n' => write!(f, "'\\n'"),
                '\r' => write!(f, "'\\r'"),
                '\t' => write!(f, "'\\t'"),
                '\\' => write!(f, "'\\\\'"),
                '\'' => write!(f, "'\\''"),
                _ => write!(f, "'{}'", c),
            },
            Value::U8(b) => match b {
                b'\0' => write!(f, "b'\\0'"),
                b'\n' => write!(f, "b'\\n'"),
                b'\r' => write!(f, "b'\\r'"),
                b'\t' => write!(f, "b'\\t'"),
                b'\\' => write!(f, "b'\\\\'"),
                b'\'' => write!(f, "b'\\''"),
                0x20..=0x7e => write!(f, "b'{}'", *b as char),
                _ => write!(f, "b'\\x{:02x}'", b),
            },
            Value::I8(n) => write!(f, "{}i8", n),
            Value::I16(n) => write!(f, "{}i16", n),
            Value::U16(n) => write!(f, "{}u16", n),
            Value::I32(n) => write!(f, "{}i32", n),
            Value::U32(n) => write!(f, "{}u32", n),
            Value::F32(n) => {
                if n.fract() == 0.0 {
                    write!(f, "{}.0f32", n)
                } else {
                    write!(f, "{}f32", n)
                }
            }
            Value::U64(n) => write!(f, "{}u64", n),
            Value::I128(n) => write!(f, "{}i128", n),
            Value::U128(n) => write!(f, "{}u128", n),
            Value::Unit => write!(f, "()"),
            Value::Array(elems) => {
                write!(f, "[")?;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", elem)?;
                }
                write!(f, "]")
            }
            Value::Struct(fields) => {
                write!(f, "(")?;
                for (i, (label, val)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    // Check if label is numeric (positional)
                    if label.parse::<usize>().is_ok() {
                        write!(f, "{}", val)?;
                    } else {
                        write!(f, "{}=", label)?;
                        write!(f, "{}", val)?;
                    }
                }
                write!(f, ")")
            }
            Value::Closure { .. } => write!(f, "<function>"),
            Value::BranchClosure { .. } => write!(f, "<function>"),
            Value::TagConstructor { name, .. } => write!(f, "<tag {}>", name),
            Value::Tagged {
                name, payload, ..
            } => {
                if matches!(**payload, Value::Unit) {
                    write!(f, "{}", name)
                } else {
                    write!(f, "{}(", name)?;
                    write!(f, "{}", payload)?;
                    write!(f, ")")
                }
            }
            Value::BuiltinFn(name) => write!(f, "<builtin {}>", name),
            Value::MethodSet { .. } => write!(f, "<method_set>"),
        }
    }
}

impl PartialEq for Env {
    fn eq(&self, other: &Self) -> bool {
        if self.bindings.len() != other.bindings.len() {
            return false;
        }
        self.bindings
            .iter()
            .zip(other.bindings.iter())
            .all(|(a, b)| a.name == b.name && a.value == b.value)
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::I64(a), Value::I64(b)) => a == b,
            (Value::F64(a), Value::F64(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::U8(a), Value::U8(b)) => a == b,
            (Value::I8(a), Value::I8(b)) => a == b,
            (Value::I16(a), Value::I16(b)) => a == b,
            (Value::U16(a), Value::U16(b)) => a == b,
            (Value::I32(a), Value::I32(b)) => a == b,
            (Value::U32(a), Value::U32(b)) => a == b,
            (Value::F32(a), Value::F32(b)) => a == b,
            (Value::U64(a), Value::U64(b)) => a == b,
            (Value::I128(a), Value::I128(b)) => a == b,
            (Value::U128(a), Value::U128(b)) => a == b,
            (Value::Unit, Value::Unit) => true,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Struct(a), Value::Struct(b)) => a == b,
            (Value::Closure { id: id1, .. }, Value::Closure { id: id2, .. }) => id1 == id2,
            (Value::BranchClosure { id: id1, .. }, Value::BranchClosure { id: id2, .. }) => {
                id1 == id2
            }
            (
                Value::Tagged {
                    id: id1,
                    payload: p1,
                    ..
                },
                Value::Tagged {
                    id: id2,
                    payload: p2,
                    ..
                },
            ) => id1 == id2 && p1 == p2,
            (Value::TagConstructor { id: id1, .. }, Value::TagConstructor { id: id2, .. }) => {
                id1 == id2
            }
            (Value::BuiltinFn(a), Value::BuiltinFn(b)) => a == b,
            (Value::MethodSet { id: id1, .. }, Value::MethodSet { id: id2, .. }) => id1 == id2,
            _ => false,
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Str(s) => {
                write!(f, "\"")?;
                for c in s.chars() {
                    match c {
                        '\\' => write!(f, "\\\\")?,
                        '"' => write!(f, "\\\"")?,
                        '\n' => write!(f, "\\n")?,
                        '\t' => write!(f, "\\t")?,
                        '\r' => write!(f, "\\r")?,
                        '\0' => write!(f, "\\0")?,
                        c => write!(f, "{}", c)?,
                    }
                }
                write!(f, "\"")
            }
            other => write!(f, "{}", other),
        }
    }
}
