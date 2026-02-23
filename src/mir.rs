//! MIR (Mid-level Intermediate Representation) — a desugared AST.
//!
//! The MIR has fewer node types than the AST. Syntactic sugar is expanded:
//! - Binary operators → method calls (.add, .subtract, .times, .divided_by)
//! - Comparisons → method calls (.eq, .not_eq, .lt, .gt, .lt_eq, .gt_eq)
//! - Unary minus → method call (.negate)
//! - Pipes → inlined into calls / binds
//! - String interpolation → .add() / .to_string() chains
//! - Range → struct literal (start=a, end=b)
//! - Group → stripped
//! - Let patterns → chains of Bind (single name only)
//! - LetArray → chains of Bind + .get() calls

use crate::ast;
use crate::ast::{ExprKind, BinOp, CmpOp, Pattern, ArrayPat, BranchBinding};

pub type Mir = Box<MirKind>;

#[derive(Debug, Clone, PartialEq)]
pub enum MirKind {
    // Literals
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Char(char),
    Byte(u8),
    Unit,

    // Reference
    Ident(String),

    // Block / lambda
    Block(Mir),
    BranchBlock(Vec<MirBranchArm>),

    // Collections
    Array(Vec<Mir>),
    Struct(Vec<MirField>),

    // Access + calls
    FieldAccess(Mir, String),
    Call(Mir, Mir),
    MethodCall {
        receiver: Mir,
        method: String,
        arg: Mir,
    },

    // Binding — single name, carries its own value
    Bind {
        name: String,
        value: Mir,
        body: Mir,
    },

    // Let binding with pattern (not yet fully desugared — requires runtime heuristics)
    Let {
        pattern: Pattern,
        body: Mir,
    },

    // Let array destructuring (not yet fully desugared)
    LetArray {
        patterns: Vec<ArrayPat>,
        body: Mir,
    },

    // Pipe (kept for pipe-into-let/letarray which need special input handling)
    Pipe(Mir, Mir),

    // Tag constructor
    NewTag(u64, Option<String>),

    // Import
    Import(String),

    // Method set scope
    Apply {
        expr: Mir,
        body: Mir,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirField {
    pub label: Option<String>,
    pub value: Mir,
    pub is_spread: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirBranchArm {
    pub pattern: MirBranchPattern,
    pub guard: Option<Mir>,
    pub body: Mir,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MirBranchPattern {
    Literal(Mir),
    Tag(String, Option<BranchBinding>),
    Binding(String),
    Discard,
}

// ── Lowering: AST → MIR ────────────────────────────────────────

/// Lower an AST expression to MIR.
pub fn lower(expr: &ast::Expr) -> Mir {
    match expr.as_ref() {
        // ── Literals (pass through) ──
        ExprKind::Int(n) => Box::new(MirKind::Int(*n)),
        ExprKind::Float(f) => Box::new(MirKind::Float(*f)),
        ExprKind::Bool(b) => Box::new(MirKind::Bool(*b)),
        ExprKind::Str(s) => Box::new(MirKind::Str(s.clone())),
        ExprKind::Char(c) => Box::new(MirKind::Char(*c)),
        ExprKind::Byte(b) => Box::new(MirKind::Byte(*b)),
        ExprKind::Unit => Box::new(MirKind::Unit),
        ExprKind::Ident(name) => Box::new(MirKind::Ident(name.clone())),
        ExprKind::NewTag(id, name) => Box::new(MirKind::NewTag(*id, name.clone())),
        ExprKind::Import(name) => Box::new(MirKind::Import(name.clone())),

        // ── Block / lambda ──
        ExprKind::Block(body) => Box::new(MirKind::Block(lower(body))),
        ExprKind::BranchBlock(arms) => Box::new(MirKind::BranchBlock(
            arms.iter().map(lower_branch_arm).collect(),
        )),

        // ── Collections ──
        ExprKind::Array(elems) => Box::new(MirKind::Array(
            elems.iter().map(lower).collect(),
        )),
        ExprKind::Struct(fields) => Box::new(MirKind::Struct(
            fields.iter().map(lower_field).collect(),
        )),

        // ── Access + calls ──
        ExprKind::FieldAccess(expr, field) => Box::new(MirKind::FieldAccess(
            lower(expr),
            field.clone(),
        )),
        ExprKind::Call(func, arg) => Box::new(MirKind::Call(
            lower(func),
            lower(arg),
        )),
        ExprKind::MethodCall { receiver, method, arg } => Box::new(MirKind::MethodCall {
            receiver: lower(receiver),
            method: method.clone(),
            arg: lower(arg),
        }),

        // ── Binary operators → method calls ──
        ExprKind::BinOp(op, lhs, rhs) => {
            let method = match op {
                BinOp::Add => "add",
                BinOp::Sub => "subtract",
                BinOp::Mul => "times",
                BinOp::Div => "divided_by",
            };
            Box::new(MirKind::MethodCall {
                receiver: lower(lhs),
                method: method.to_string(),
                arg: lower(rhs),
            })
        }

        // ── Comparisons → method calls ──
        ExprKind::Compare(op, lhs, rhs) => {
            let method = match op {
                CmpOp::Eq => "eq",
                CmpOp::NotEq => "not_eq",
                CmpOp::Lt => "lt",
                CmpOp::Gt => "gt",
                CmpOp::LtEq => "lt_eq",
                CmpOp::GtEq => "gt_eq",
            };
            Box::new(MirKind::MethodCall {
                receiver: lower(lhs),
                method: method.to_string(),
                arg: lower(rhs),
            })
        }

        // ── Unary minus → method call ──
        ExprKind::UnaryMinus(expr) => Box::new(MirKind::MethodCall {
            receiver: lower(expr),
            method: "negate".to_string(),
            arg: Box::new(MirKind::Unit),
        }),

        // ── Pipe → inline into target ──
        ExprKind::Pipe(lhs, rhs) => lower_pipe(lhs, rhs),

        // ── Let → pass through (pattern destructuring requires runtime heuristics) ──
        ExprKind::Let { pattern, body } => Box::new(MirKind::Let {
            pattern: pattern.clone(),
            body: lower(body),
        }),

        // ── LetArray → pass through (destructuring requires runtime logic) ──
        ExprKind::LetArray { patterns, body } => Box::new(MirKind::LetArray {
            patterns: patterns.clone(),
            body: lower(body),
        }),

        // ── String interpolation → add + to_string chain ──
        ExprKind::StringInterp(parts) => lower_string_interp(parts),

        // ── Range → struct literal ──
        ExprKind::Range(start, end) => Box::new(MirKind::Struct(vec![
            MirField { label: Some("start".to_string()), value: lower(start), is_spread: false },
            MirField { label: Some("end".to_string()), value: lower(end), is_spread: false },
        ])),

        // ── Group → stripped ──
        ExprKind::Group(inner) => lower(inner),

        // ── Apply ──
        ExprKind::Apply { expr, body } => Box::new(MirKind::Apply {
            expr: lower(expr),
            body: lower(body),
        }),
    }
}

// ── Pipe lowering ──────────────────────────────────────────────

fn lower_pipe(lhs: &ast::Expr, rhs: &ast::Expr) -> Mir {
    match rhs.as_ref() {
        // a >> f(arg) → f(prepend(a, arg))
        ExprKind::Call(func, arg) => {
            Box::new(MirKind::Call(
                lower(func),
                prepend_to_struct(lower(lhs), lower(arg)),
            ))
        }

        // a >> let(...); body — keep as Pipe (needs special input handling)
        ExprKind::Let { .. } => {
            Box::new(MirKind::Pipe(lower(lhs), lower(rhs)))
        }

        // a >> let[...]; body — keep as Pipe
        ExprKind::LetArray { .. } => {
            Box::new(MirKind::Pipe(lower(lhs), lower(rhs)))
        }

        // a >> receiver.method(args) → receiver.method(prepend(a, args))
        ExprKind::MethodCall { receiver, method, arg } => {
            Box::new(MirKind::MethodCall {
                receiver: lower(receiver),
                method: method.clone(),
                arg: prepend_to_struct(lower(lhs), lower(arg)),
            })
        }

        // a >> apply(ms); body — keep as Pipe (needs special scope handling)
        ExprKind::Apply { .. } => {
            Box::new(MirKind::Pipe(lower(lhs), lower(rhs)))
        }

        // a >> other → Call(other, a)
        _ => Box::new(MirKind::Call(lower(rhs), lower(lhs))),
    }
}

/// Prepend a value to a struct argument.
/// If arg is Unit → just val.
/// If arg is Struct → prepend val as first positional field.
/// Otherwise → Struct([val, arg]).
fn prepend_to_struct(val: Mir, arg: Mir) -> Mir {
    match arg.as_ref() {
        MirKind::Unit => val,
        MirKind::Struct(fields) => {
            let mut new_fields = vec![MirField {
                label: None,
                value: val,
                is_spread: false,
            }];
            new_fields.extend(fields.iter().cloned());
            Box::new(MirKind::Struct(new_fields))
        }
        _ => Box::new(MirKind::Struct(vec![
            MirField { label: None, value: val, is_spread: false },
            MirField { label: None, value: arg, is_spread: false },
        ])),
    }
}

// ── String interpolation lowering ──────────────────────────────

fn lower_string_interp(parts: &[ast::StringInterpPart]) -> Mir {
    // Build a chain of .add() calls starting from the first part.
    // "hello {name}!" → "hello ".add(name.to_string()).add("!")
    let mut result: Option<Mir> = None;

    for part in parts {
        let piece = match part {
            ast::StringInterpPart::Literal(s) => {
                Box::new(MirKind::Str(s.clone()))
            }
            ast::StringInterpPart::Expr(expr) => {
                // Convert expression result to string via .to_string()
                Box::new(MirKind::MethodCall {
                    receiver: lower(expr),
                    method: "to_string".to_string(),
                    arg: Box::new(MirKind::Unit),
                })
            }
        };

        result = Some(match result {
            None => piece,
            Some(acc) => Box::new(MirKind::MethodCall {
                receiver: acc,
                method: "add".to_string(),
                arg: piece,
            }),
        });
    }

    result.unwrap_or_else(|| Box::new(MirKind::Str(String::new())))
}

// ── Helpers ────────────────────────────────────────────────────

fn lower_branch_arm(arm: &ast::BranchArm) -> MirBranchArm {
    MirBranchArm {
        pattern: lower_branch_pattern(&arm.pattern),
        guard: arm.guard.as_ref().map(lower),
        body: lower(&arm.body),
    }
}

fn lower_branch_pattern(pat: &ast::BranchPattern) -> MirBranchPattern {
    match pat {
        ast::BranchPattern::Literal(expr) => MirBranchPattern::Literal(lower(expr)),
        ast::BranchPattern::Tag(name, binding) => {
            MirBranchPattern::Tag(name.clone(), binding.clone())
        }
        ast::BranchPattern::Binding(name) => MirBranchPattern::Binding(name.clone()),
        ast::BranchPattern::Discard => MirBranchPattern::Discard,
    }
}

fn lower_field(field: &ast::Field) -> MirField {
    MirField {
        label: field.label.clone(),
        value: lower(&field.value),
        is_spread: field.is_spread,
    }
}
