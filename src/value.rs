use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use crate::ast::{BranchArm, Expr};

pub type TagId = u64;

#[derive(Clone)]
struct Binding {
    name: String,
    value: Value,
    used: Rc<Cell<bool>>,
}

#[derive(Clone)]
pub struct Env {
    bindings: Vec<Binding>,
}

impl Env {
    pub fn new() -> Self {
        Env {
            bindings: Vec::new(),
        }
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
        let mut new_env = self.clone();
        new_env.bindings.push(Binding {
            name,
            value,
            used: Rc::new(Cell::new(false)),
        });
        new_env
    }

    /// Return warnings for unused bindings that don't start with `_`.
    /// Skips builtins and internal names (like `\0`).
    pub fn unused_warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        let mut seen = std::collections::HashSet::new();
        // Walk in reverse so we only warn about the most recent binding for each name.
        // (Shadowed bindings that are unused are expected.)
        for b in self.bindings.iter().rev() {
            if seen.contains(&b.name) {
                continue;
            }
            seen.insert(b.name.clone());
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
}

#[derive(Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Char(char),
    Byte(u8),
    Unit,
    Array(Vec<Value>),
    Struct(Vec<(String, Value)>),
    Closure {
        body: Expr,
        env: Env,
    },
    BranchClosure {
        arms: Vec<BranchArm>,
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
}

impl Value {
    /// For `print`: format the value for human consumption (strings without quotes).
    pub fn print_string(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            other => other.to_string(),
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
            Value::Int(n) => write!(f, "{}", n),
            Value::Float(n) => {
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
            Value::Byte(b) => match b {
                b'\0' => write!(f, "b'\\0'"),
                b'\n' => write!(f, "b'\\n'"),
                b'\r' => write!(f, "b'\\r'"),
                b'\t' => write!(f, "b'\\t'"),
                b'\\' => write!(f, "b'\\\\'"),
                b'\'' => write!(f, "b'\\''"),
                0x20..=0x7e => write!(f, "b'{}'", *b as char),
                _ => write!(f, "b'\\x{:02x}'", b),
            },
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
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Byte(a), Value::Byte(b)) => a == b,
            (Value::Unit, Value::Unit) => true,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Struct(a), Value::Struct(b)) => a == b,
            (Value::Closure { body: b1, env: e1 }, Value::Closure { body: b2, env: e2 }) => {
                b1 == b2 && e1 == e2
            }
            (
                Value::BranchClosure { arms: a1, env: e1 },
                Value::BranchClosure { arms: a2, env: e2 },
            ) => a1 == a2 && e1 == e2,
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
            _ => false,
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Str(s) => write!(f, "\"{}\"", s),
            other => write!(f, "{}", other),
        }
    }
}
