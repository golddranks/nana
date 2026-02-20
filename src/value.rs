use std::fmt;

use crate::ast::{BranchArm, Expr};

pub type TagId = u64;

#[derive(Clone)]
pub struct Env {
    bindings: Vec<(String, Value)>,
}

impl Env {
    pub fn new() -> Self {
        Env {
            bindings: Vec::new(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.bindings.iter().rev().find_map(|(k, v)| {
            if k == name {
                Some(v)
            } else {
                None
            }
        })
    }

    pub fn bind(&self, name: String, value: Value) -> Env {
        let mut new_env = self.clone();
        new_env.bindings.push((name, value));
        new_env
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
    /// Write the value in "nested" form, where strings are quoted.
    /// Used when displaying values inside composite types (arrays, structs, tagged).
    fn write_nested(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Str(s) => {
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
            other => write!(f, "{}", other),
        }
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
            Value::Str(s) => write!(f, "{}", s),
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
                    elem.write_nested(f)?;
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
                        val.write_nested(f)?;
                    } else {
                        write!(f, "{}=", label)?;
                        val.write_nested(f)?;
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
                    payload.write_nested(f)?;
                    write!(f, ")")
                }
            }
            Value::BuiltinFn(name) => write!(f, "<builtin {}>", name),
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
