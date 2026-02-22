pub type Expr = Box<ExprKind>;

#[derive(Debug, Clone, PartialEq)]
pub enum StringInterpPart {
    Literal(String),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    // Literals
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Char(char),
    Byte(u8),
    Unit,

    // String interpolation: "hello, {name}!"
    StringInterp(Vec<StringInterpPart>),

    // References
    Ident(String),

    // Block / lambda: { body } — `in` refers to the block's input
    Block(Expr),

    // Branching block: { pattern -> expr, ... } — matches `in` against arms
    BranchBlock(Vec<BranchArm>),

    // Collections
    Array(Vec<Expr>),
    Struct(Vec<Field>),

    // Access
    FieldAccess(Expr, String),

    // Function call: f(arg), f{block}, f[array]
    Call(Expr, Expr),

    // Method call: value.method(arg) — type-based dispatch
    MethodCall {
        receiver: Expr,
        method: String,
        arg: Expr,
    },

    // Operators
    UnaryMinus(Expr),
    BinOp(BinOp, Expr, Expr),
    Compare(CmpOp, Expr, Expr),

    // Pipe: lhs >> rhs
    Pipe(Expr, Expr),

    // Let binding: bind input to pattern, then evaluate body.
    // In `value >> let(x); body`, this becomes Pipe(value, Let(Name("x"), body)).
    // The "input" to Let is the value from the pipe. body can reference the bound name.
    // `let(x)` returns x — if body is None, the let returns the bound value.
    Let {
        pattern: Pattern,
        body: Expr,
    },

    // Array destructuring: value >> let[a, b, ...rest]; body
    LetArray {
        patterns: Vec<ArrayPat>,
        body: Expr,
    },

    // Tag constructor (generative, each lexical occurrence gets unique id)
    // (tag_id, optional display name from tag(Name) sugar)
    NewTag(u64, Option<String>),

    // Range sugar: a..b → (start=a, end=b)
    Range(Expr, Expr),

    // Import
    Import(String),

    // apply(ms); body — activate method set in lexical scope
    Apply {
        expr: Expr,
        body: Expr,
    },

    // Grouping: `(expr)` — semantically transparent, but prevents `let x = (expr)`
    // desugaring from nesting into the inner expression's let chain.
    Group(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub label: Option<String>,
    pub value: Expr,
    pub is_spread: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CmpOp {
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Name(String),
    Discard,
    Fields(Vec<PatField>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PatField {
    pub label: Option<String>,
    pub binding: String,
    pub is_rest: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayPat {
    Name(String),
    Discard,
    Rest(Option<String>),
}

/// An arm in a branching block: { pattern -> expr, ... }
#[derive(Debug, Clone, PartialEq)]
pub struct BranchArm {
    pub pattern: BranchPattern,
    pub guard: Option<Expr>,
    pub body: Expr,
}

/// Pattern in a branching arm.
#[derive(Debug, Clone, PartialEq)]
pub enum BranchPattern {
    /// Match a literal value: true, false, 0, "hello", etc.
    Literal(Expr),
    /// Match a tag constructor: Ok(x), Err(msg)
    Tag(String, Option<BranchBinding>),
    /// Catch-all binding: x (binds the whole value)
    Binding(String),
    /// Discard: _
    Discard,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BranchBinding {
    Name(String),
    Discard,
}

/// Collect all module names referenced by `import(name)` in an AST.
/// Returns a deduplicated list in first-occurrence order.
pub fn collect_imports(expr: &Expr) -> Vec<String> {
    let mut names = Vec::new();
    collect_imports_inner(expr, &mut names);
    names
}

fn collect_imports_inner(expr: &Expr, names: &mut Vec<String>) {
    match expr.as_ref() {
        ExprKind::Import(name) => {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        ExprKind::Block(body) => collect_imports_inner(body, names),
        ExprKind::BranchBlock(arms) => {
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_imports_inner(guard, names);
                }
                collect_imports_inner(&arm.body, names);
                if let BranchPattern::Literal(lit) = &arm.pattern {
                    collect_imports_inner(lit, names);
                }
            }
        }
        ExprKind::Array(elems) => {
            for e in elems {
                collect_imports_inner(e, names);
            }
        }
        ExprKind::Struct(fields) => {
            for f in fields {
                collect_imports_inner(&f.value, names);
            }
        }
        ExprKind::StringInterp(parts) => {
            for part in parts {
                if let StringInterpPart::Expr(e) = part {
                    collect_imports_inner(e, names);
                }
            }
        }
        ExprKind::FieldAccess(e, _) => collect_imports_inner(e, names),
        ExprKind::Call(func, arg) => {
            collect_imports_inner(func, names);
            collect_imports_inner(arg, names);
        }
        ExprKind::MethodCall { receiver, arg, .. } => {
            collect_imports_inner(receiver, names);
            collect_imports_inner(arg, names);
        }
        ExprKind::UnaryMinus(e) => collect_imports_inner(e, names),
        ExprKind::BinOp(_, lhs, rhs) => {
            collect_imports_inner(lhs, names);
            collect_imports_inner(rhs, names);
        }
        ExprKind::Compare(_, lhs, rhs) => {
            collect_imports_inner(lhs, names);
            collect_imports_inner(rhs, names);
        }
        ExprKind::Pipe(lhs, rhs) => {
            collect_imports_inner(lhs, names);
            collect_imports_inner(rhs, names);
        }
        ExprKind::Let { body, .. } => collect_imports_inner(body, names),
        ExprKind::LetArray { body, .. } => collect_imports_inner(body, names),
        ExprKind::Apply { expr, body } => {
            collect_imports_inner(expr, names);
            collect_imports_inner(body, names);
        }
        ExprKind::Range(start, end) => {
            collect_imports_inner(start, names);
            collect_imports_inner(end, names);
        }
        ExprKind::Group(inner) => collect_imports_inner(inner, names),
        // Leaves — no children to recurse into
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::Str(_)
        | ExprKind::Char(_)
        | ExprKind::Byte(_)
        | ExprKind::Unit
        | ExprKind::Ident(_)
        | ExprKind::NewTag(_, _) => {}
    }
}
