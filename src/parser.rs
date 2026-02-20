use crate::ast::*;
use crate::lexer::{StringPart, Token};
use std::sync::atomic::{AtomicU64, Ordering};

static TAG_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_tag_id() -> u64 {
    TAG_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

// Binding power levels for Pratt parsing.
// Each infix operator has a left bp and right bp.
// Higher = tighter binding.
mod bp {
    // Binding power convention: operator consumed when min_bp <= left_bp.
    // Left-associative: right_bp = left_bp + 1 (prevents re-entry at same level).
    // Right-associative: right_bp = left_bp (allows re-entry).

    // Semicolon: right-associative, lowest
    pub const SEMI_L: u8 = 2;
    pub const SEMI_R: u8 = 2; // right-assoc: same as left
    // Pipe >>: left-associative
    pub const PIPE_L: u8 = 6;
    pub const PIPE_R: u8 = 7; // left-assoc: left + 1
    // Comparison: non-associative
    pub const CMP_L: u8 = 8;
    pub const CMP_R: u8 = 9; // non-assoc: treat like left-assoc
    // Add/sub: left-associative
    pub const ADD_L: u8 = 10;
    pub const ADD_R: u8 = 11;
    // Range ..: tighter than add/sub so `a + b..c` is `a + (b..c)`
    pub const RANGE_L: u8 = 12;
    pub const RANGE_R: u8 = 13;
    // Mul/div: left-associative
    pub const MUL_L: u8 = 14;
    pub const MUL_R: u8 = 15;
    // Unary prefix
    pub const UNARY: u8 = 17;
    // Postfix call/field access
    pub const POSTFIX: u8 = 19;
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    pub fn parse_program(&mut self) -> Result<Expr, String> {
        if matches!(self.peek(), Token::Eof) {
            return Ok(Box::new(ExprKind::Unit));
        }
        let expr = self.parse_expr(0)?;
        if !matches!(self.peek(), Token::Eof) {
            return Err(format!(
                "unexpected token after expression: {:?}",
                self.peek()
            ));
        }
        Ok(expr)
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn peek_at(&self, offset: usize) -> &Token {
        self.tokens.get(self.pos + offset).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof);
        self.pos += 1;
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        let tok = self.advance();
        if &tok == expected {
            Ok(())
        } else {
            Err(format!("expected {:?}, got {:?}", expected, tok))
        }
    }

    // ── Main Pratt loop ──────────────────────────────────────────────

    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, String> {
        let mut lhs = self.parse_prefix()?;

        loop {
            lhs = match self.peek() {
                // ── Postfix: call f(...), f{block}, f[array], field a.x ──
                Token::LParen if min_bp <= bp::POSTFIX => {
                    self.advance();
                    let arg = self.parse_call_args()?;
                    Box::new(ExprKind::Call(lhs, arg))
                }
                Token::LBrace if min_bp <= bp::POSTFIX => {
                    // f{ body } — call f with a block argument
                    let block = self.parse_block()?;
                    Box::new(ExprKind::Call(lhs, block))
                }
                Token::LBracket if min_bp <= bp::POSTFIX => {
                    // f[elems] — call f with an array argument
                    let arr = self.parse_array()?;
                    Box::new(ExprKind::Call(lhs, arr))
                }
                Token::Dot if min_bp <= bp::POSTFIX => {
                    self.advance();
                    let field = match self.advance() {
                        Token::Ident(name) => name,
                        Token::Int(n) => n.to_string(),
                        tok => {
                            return Err(format!("expected field name after '.', got {:?}", tok))
                        }
                    };
                    // Check if this is a method call: .name(...), .name{...}, .name[...]
                    match self.peek() {
                        Token::LParen if min_bp <= bp::POSTFIX => {
                            self.advance();
                            let arg = self.parse_call_args()?;
                            Box::new(ExprKind::MethodCall {
                                receiver: lhs,
                                method: field,
                                arg,
                            })
                        }
                        Token::LBrace if min_bp <= bp::POSTFIX => {
                            let block = self.parse_block()?;
                            Box::new(ExprKind::MethodCall {
                                receiver: lhs,
                                method: field,
                                arg: block,
                            })
                        }
                        Token::LBracket if min_bp <= bp::POSTFIX => {
                            let arr = self.parse_array()?;
                            Box::new(ExprKind::MethodCall {
                                receiver: lhs,
                                method: field,
                                arg: arr,
                            })
                        }
                        _ => {
                            // Plain field access
                            Box::new(ExprKind::FieldAccess(lhs, field))
                        }
                    }
                }

                // ── Infix: * / ──
                Token::Star if min_bp <= bp::MUL_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::MUL_R)?;
                    Box::new(ExprKind::BinOp(BinOp::Mul, lhs, rhs))
                }
                Token::Slash if min_bp <= bp::MUL_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::MUL_R)?;
                    if matches!(self.peek(), Token::Star | Token::Slash) {
                        return Err("ambiguous precedence: use parentheses around division".to_string());
                    }
                    Box::new(ExprKind::BinOp(BinOp::Div, lhs, rhs))
                }

                // ── Infix: + - ──
                Token::Plus if min_bp <= bp::ADD_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::ADD_R)?;
                    Box::new(ExprKind::BinOp(BinOp::Add, lhs, rhs))
                }
                Token::Minus if min_bp <= bp::ADD_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::ADD_R)?;
                    Box::new(ExprKind::BinOp(BinOp::Sub, lhs, rhs))
                }

                // ── Infix: comparisons (non-associative) ──
                Token::EqEq if min_bp <= bp::CMP_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::CMP_R)?;
                    self.check_no_chained_comparison()?;
                    Box::new(ExprKind::Compare(CmpOp::Eq, lhs, rhs))
                }
                Token::NotEq if min_bp <= bp::CMP_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::CMP_R)?;
                    self.check_no_chained_comparison()?;
                    Box::new(ExprKind::Compare(CmpOp::NotEq, lhs, rhs))
                }
                Token::Lt if min_bp <= bp::CMP_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::CMP_R)?;
                    self.check_no_chained_comparison()?;
                    Box::new(ExprKind::Compare(CmpOp::Lt, lhs, rhs))
                }
                Token::Gt if min_bp <= bp::CMP_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::CMP_R)?;
                    self.check_no_chained_comparison()?;
                    Box::new(ExprKind::Compare(CmpOp::Gt, lhs, rhs))
                }
                Token::LtEq if min_bp <= bp::CMP_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::CMP_R)?;
                    self.check_no_chained_comparison()?;
                    Box::new(ExprKind::Compare(CmpOp::LtEq, lhs, rhs))
                }
                Token::GtEq if min_bp <= bp::CMP_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::CMP_R)?;
                    self.check_no_chained_comparison()?;
                    Box::new(ExprKind::Compare(CmpOp::GtEq, lhs, rhs))
                }

                // ── Infix: range .. (non-associative) ──
                Token::DotDot if min_bp <= bp::RANGE_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::RANGE_R)?;
                    if matches!(self.peek(), Token::DotDot) {
                        return Err("non-associative: chained ranges require parentheses".to_string());
                    }
                    Box::new(ExprKind::Range(lhs, rhs))
                }

                // ── Infix: pipe >> ──
                Token::Pipe if min_bp <= bp::PIPE_L => {
                    self.advance();
                    self.parse_pipe_rhs(lhs, min_bp)?
                }

                // ── Infix: semicolon (right-associative) ──
                Token::Semicolon if min_bp <= bp::SEMI_L => {
                    self.advance();
                    self.parse_semicolon_rhs(lhs)?
                }

                _ => break,
            };
        }

        Ok(lhs)
    }

    // ── Pipe right-hand side ─────────────────────────────────────────

    fn parse_pipe_rhs(&mut self, lhs: Expr, min_bp: u8) -> Result<Expr, String> {
        match self.peek() {
            // >> let(pattern) — bind and continue
            Token::Let => {
                self.advance();
                self.parse_pipe_let(lhs, min_bp)
            }
            // >> normal_expr
            _ => {
                let rhs = self.parse_expr(bp::PIPE_R)?;
                Ok(Box::new(ExprKind::Pipe(lhs, rhs)))
            }
        }
    }

    /// Parse `let(pat)` after `>>`. The lhs is the value being bound.
    fn parse_pipe_let(&mut self, lhs: Expr, min_bp: u8) -> Result<Expr, String> {
        let (pattern, is_array) = self.parse_let_pattern()?;

        let body = self.parse_let_body(&pattern, is_array, min_bp)?;

        if is_array {
            let array_pats = self.pattern_to_array_pats(pattern)?;
            Ok(Box::new(ExprKind::Pipe(
                lhs,
                Box::new(ExprKind::LetArray {
                    patterns: array_pats,
                    body,
                }),
            )))
        } else {
            Ok(Box::new(ExprKind::Pipe(
                lhs,
                Box::new(ExprKind::Let { pattern, body }),
            )))
        }
    }

    /// After parsing `let(pat)`, parse the body/continuation.
    /// `min_bp` is the minimum binding power from the enclosing parse_expr call.
    /// When min_bp > SEMI_L, we must NOT consume `;` here (it belongs to an outer context).
    /// `is_array` indicates whether this is an array destructuring pattern (let[...]).
    fn parse_let_body(&mut self, pattern: &Pattern, is_array: bool, min_bp: u8) -> Result<Expr, String> {
        match self.peek() {
            Token::Pipe => {
                let mut current = self.identity_expr_for_pattern(pattern, is_array);
                while matches!(self.peek(), Token::Pipe) {
                    self.advance();
                    current = self.parse_pipe_rhs(current, min_bp)?;
                }
                if min_bp <= bp::SEMI_L && matches!(self.peek(), Token::Semicolon) {
                    self.advance();
                    let rest = self.parse_expr(bp::SEMI_R)?;
                    Ok(Box::new(ExprKind::Pipe(
                        current,
                        Box::new(ExprKind::Let {
                            pattern: Pattern::Discard,
                            body: rest,
                        }),
                    )))
                } else {
                    Ok(current)
                }
            }
            Token::Semicolon if min_bp <= bp::SEMI_L => {
                self.advance();
                self.parse_expr(bp::SEMI_R)
            }
            _ => {
                Ok(self.identity_expr_for_pattern(pattern, is_array))
            }
        }
    }

    /// Hidden variable name used to pass through the piped value in multi-field
    /// destructuring. Uses "\0" which the lexer cannot produce, so it can never
    /// collide with user-defined names.
    const LET_INPUT: &'static str = "\0";

    /// Create an expression that returns the bound value(s) for a pattern.
    /// For single-name patterns, returns `Ident(name)` (which IS the piped value).
    /// For multi-field or array patterns, returns `Ident(LET_INPUT)` — a reference
    /// to a hidden variable that eval_pipe will bind to the original piped value.
    fn identity_expr_for_pattern(&self, pattern: &Pattern, _is_array: bool) -> Expr {
        match pattern {
            Pattern::Name(name) => Box::new(ExprKind::Ident(name.clone())),
            Pattern::Discard => Box::new(ExprKind::Unit),
            Pattern::Fields(_) => {
                // For multi-field patterns (struct or array), pass through the
                // original piped value via the hidden LET_INPUT variable.
                Box::new(ExprKind::Ident(Self::LET_INPUT.to_string()))
            }
        }
    }

    fn pattern_to_array_pats(&self, pattern: Pattern) -> Result<Vec<ArrayPat>, String> {
        match pattern {
            Pattern::Fields(fields) => Ok(fields
                .into_iter()
                .map(|f| {
                    if f.is_rest {
                        if f.binding == "_" {
                            ArrayPat::Rest(None)
                        } else {
                            ArrayPat::Rest(Some(f.binding))
                        }
                    } else if f.binding == "_" {
                        ArrayPat::Discard
                    } else {
                        ArrayPat::Name(f.binding)
                    }
                })
                .collect()),
            Pattern::Name(name) => Ok(vec![ArrayPat::Name(name)]),
            Pattern::Discard => Ok(vec![ArrayPat::Discard]),
        }
    }

    // ── Semicolon right-hand side ────────────────────────────────────

    /// Handle `;` — if lhs is a standalone let/tag, attach rhs as its body.
    /// Otherwise, sequence: evaluate lhs (discard), evaluate rhs.
    /// A trailing `;` (followed by EOF or closing delimiter) produces `()`.
    fn parse_semicolon_rhs(&mut self, lhs: Expr) -> Result<Expr, String> {
        // Trailing semicolon: treat as `expr; ()` → evaluates to ()
        if matches!(self.peek(), Token::Eof | Token::RBrace | Token::RParen | Token::RBracket) {
            let rhs = Box::new(ExprKind::Unit);
            return self.attach_body(lhs, rhs);
        }
        let rhs = self.parse_expr(bp::SEMI_R)?;
        self.attach_body(lhs, rhs)
    }

    /// Attach a body to a let-like expression, or create a sequence.
    fn attach_body(&self, lhs: Expr, rhs: Expr) -> Result<Expr, String> {
        // Group (from parenthesized expressions) blocks scope extension.
        // Per spec: "parentheses limit let scope" — bindings inside parens
        // don't leak to the continuation after the closing paren.
        match *lhs {
            ExprKind::Let { pattern, body } if Self::is_replaceable_body(&pattern, &body) => {
                Ok(Box::new(ExprKind::Let {
                    pattern,
                    body: rhs,
                }))
            }
            ExprKind::Let { pattern, body } if Self::has_replaceable_let(&body) => {
                // Recurse into non-replaceable let body to find nested replaceable lets
                let new_body = self.attach_body(body, rhs)?;
                Ok(Box::new(ExprKind::Let {
                    pattern,
                    body: new_body,
                }))
            }
            ExprKind::LetArray { patterns, body } if matches!(*body, ExprKind::Unit) => {
                // LetArray with placeholder body — fill it with the continuation
                Ok(Box::new(ExprKind::LetArray {
                    patterns,
                    body: rhs,
                }))
            }
            ExprKind::LetArray { patterns, body } if Self::has_replaceable_let(&body) => {
                // Recurse into LetArray body to find nested replaceable lets
                let new_body = self.attach_body(body, rhs)?;
                Ok(Box::new(ExprKind::LetArray {
                    patterns,
                    body: new_body,
                }))
            }
            ExprKind::Pipe(pipe_lhs, pipe_rhs) if Self::has_replaceable_let(&pipe_rhs) => {
                let new_rhs = self.attach_body(pipe_rhs, rhs)?;
                Ok(Box::new(ExprKind::Pipe(pipe_lhs, new_rhs)))
            }
            other => {
                let lhs = Box::new(other);
                Ok(Box::new(ExprKind::Pipe(
                    lhs,
                    Box::new(ExprKind::Let {
                        pattern: Pattern::Discard,
                        body: rhs,
                    }),
                )))
            }
        }
    }

    /// Check if a Let body can be replaced by a continuation.
    /// A body is replaceable if it's Unit (placeholder) or if it's just
    /// an Ident matching the pattern name (identity — returns the bound value).
    fn is_replaceable_body(pattern: &Pattern, body: &Expr) -> bool {
        if matches!(body.as_ref(), ExprKind::Unit) {
            return true;
        }
        if let Pattern::Name(name) = pattern {
            if let ExprKind::Ident(body_name) = body.as_ref() {
                return name == body_name;
            }
        }
        false
    }

    /// Check if an expression contains a Let or LetArray with a replaceable body,
    /// recursing through Pipe chains and Let bodies.
    fn has_replaceable_let(expr: &Expr) -> bool {
        match expr.as_ref() {
            ExprKind::Let { pattern, body } => {
                if Self::is_replaceable_body(pattern, body) {
                    true
                } else {
                    // Recurse into non-replaceable let bodies to find nested replaceable lets
                    Self::has_replaceable_let(body)
                }
            }
            ExprKind::LetArray { body, .. } => {
                if matches!(body.as_ref(), ExprKind::Unit) {
                    true
                } else {
                    // Recurse into LetArray body to find nested replaceable lets
                    Self::has_replaceable_let(body)
                }
            }
            ExprKind::Pipe(_, rhs) => Self::has_replaceable_let(rhs),
            _ => false,
        }
    }

    /// Check if an expression contains a Let or LetArray where we can nest a new binding.
    /// This is like `has_replaceable_let` but also matches LetArray with non-Unit bodies,
    /// used by `nest_let_in_expr` for the `let x = expr` sugar.
    fn has_nestable_binding(expr: &Expr) -> bool {
        match expr.as_ref() {
            ExprKind::Let { pattern, body } => {
                if Self::is_replaceable_body(pattern, body) {
                    true
                } else {
                    Self::has_nestable_binding(body)
                }
            }
            ExprKind::LetArray { .. } => true,
            ExprKind::Pipe(_, rhs) => Self::has_nestable_binding(rhs),
            _ => false,
        }
    }

    /// Nest a new let binding inside an expression that ends in a replaceable let.
    /// If `expr` is `Pipe(X, Let{name, Ident(name)})`, produces
    /// `Pipe(X, Let{name, body=Pipe(Ident(name), new_let)})` so that both
    /// bindings are in scope for the continuation.
    /// Otherwise, produces `Pipe(expr, new_let)`.
    fn nest_let_in_expr(expr: Expr, new_let: Expr) -> Expr {
        match expr.as_ref() {
            ExprKind::Pipe(_, rhs) if Self::has_nestable_binding(rhs) => {
                // Restructure: replace the replaceable let's body with a pipe into new_let
                Self::nest_let_in_pipe(expr, new_let)
            }
            _ => {
                Box::new(ExprKind::Pipe(expr, new_let))
            }
        }
    }

    /// Recursively nest a new let inside the deepest replaceable let in a pipe chain.
    fn nest_let_in_pipe(expr: Expr, new_let: Expr) -> Expr {
        match *expr {
            ExprKind::Pipe(pipe_lhs, pipe_rhs) => {
                let new_rhs = Self::nest_let_in_let(pipe_rhs, new_let);
                Box::new(ExprKind::Pipe(pipe_lhs, new_rhs))
            }
            _ => Box::new(ExprKind::Pipe(expr, new_let)),
        }
    }

    /// Replace a replaceable let's body with `Pipe(old_body, new_let)`.
    fn nest_let_in_let(expr: Expr, new_let: Expr) -> Expr {
        match *expr {
            ExprKind::Let { pattern, body } if Self::is_replaceable_body(&pattern, &body) => {
                Box::new(ExprKind::Let {
                    pattern,
                    body: Box::new(ExprKind::Pipe(body, new_let)),
                })
            }
            ExprKind::Let { pattern, body } if Self::has_nestable_binding(&body) => {
                // Recurse into non-replaceable let body to find nested nestable bindings
                let new_body = Self::nest_let_in_let(body, new_let);
                Box::new(ExprKind::Let { pattern, body: new_body })
            }
            ExprKind::LetArray { patterns, body } => {
                // Nest new_let inside LetArray body
                Box::new(ExprKind::LetArray {
                    patterns,
                    body: Box::new(ExprKind::Pipe(body, new_let)),
                })
            }
            ExprKind::Pipe(pipe_lhs, pipe_rhs) if Self::has_nestable_binding(&pipe_rhs) => {
                let new_rhs = Self::nest_let_in_let(pipe_rhs, new_let);
                Box::new(ExprKind::Pipe(pipe_lhs, new_rhs))
            }
            other => Box::new(ExprKind::Pipe(Box::new(other), new_let)),
        }
    }

    // ── Prefix parsing ───────────────────────────────────────────────

    fn parse_prefix(&mut self) -> Result<Expr, String> {
        match self.peek().clone() {
            Token::Int(n) => {
                self.advance();
                Ok(Box::new(ExprKind::Int(n)))
            }
            Token::Float(f) => {
                self.advance();
                Ok(Box::new(ExprKind::Float(f)))
            }
            Token::True => {
                self.advance();
                Ok(Box::new(ExprKind::Bool(true)))
            }
            Token::False => {
                self.advance();
                Ok(Box::new(ExprKind::Bool(false)))
            }
            Token::Str(ref s) => {
                let s = s.clone();
                self.advance();
                Ok(Box::new(ExprKind::Str(s)))
            }
            Token::InterpStr(ref parts) => {
                let parts = parts.clone();
                self.advance();
                self.parse_interp_string(parts)
            }
            Token::Char(c) => {
                self.advance();
                Ok(Box::new(ExprKind::Char(c)))
            }
            Token::Byte(b) => {
                self.advance();
                Ok(Box::new(ExprKind::Byte(b)))
            }
            Token::In => {
                self.advance();
                Ok(Box::new(ExprKind::Ident("in".to_string())))
            }
            Token::NewTag => {
                self.advance();
                Ok(Box::new(ExprKind::NewTag(next_tag_id(), None)))
            }
            Token::Ident(ref name) => {
                let name = name.clone();
                self.advance();
                Ok(Box::new(ExprKind::Ident(name)))
            }
            Token::Underscore => {
                self.advance();
                Ok(Box::new(ExprKind::Ident("_".to_string())))
            }

            // Unary minus
            Token::Minus => {
                self.advance();
                // Parse operand WITHOUT postfix (call/field-access) to detect ambiguity.
                // Use POSTFIX + 1 so postfix operators are not consumed.
                let operand = self.parse_expr(bp::POSTFIX + 1)?;
                // Check: -expr followed by postfix is ambiguous
                if matches!(self.peek(), Token::Dot | Token::LParen | Token::LBrace | Token::LBracket) {
                    return Err("ambiguous: `-x.f()` is a syntax error; write `(-x).f()` or `-(x.f())`".to_string());
                }
                Ok(Box::new(ExprKind::UnaryMinus(operand)))
            }

            // Block { ... } or branching block { pattern -> expr, ... }
            Token::LBrace => self.parse_block(),

            // Array [...]
            Token::LBracket => self.parse_array(),

            // Parenthesized expression, unit, or struct
            Token::LParen => self.parse_paren(),

            // Standalone let or let sugar: let name = expr;
            Token::Let => self.parse_standalone_let(),

            // tag(Name) — sugar for new_tag >> let(Name)
            Token::Tag => self.parse_tag_sugar(),

            // import("name")
            Token::Import => self.parse_import(),

            // use(name) — sugar for import(name) >> let(name)
            Token::Use => self.parse_use(),

            tok => Err(format!("unexpected token in expression: {:?}", tok)),
        }
    }

    // ── Individual prefix parsers ────────────────────────────────────

    fn parse_interp_string(&mut self, parts: Vec<StringPart>) -> Result<Expr, String> {
        let mut ast_parts = Vec::new();
        for part in parts {
            match part {
                StringPart::Literal(s) => {
                    ast_parts.push(StringInterpPart::Literal(s));
                }
                StringPart::Expr(src) => {
                    // Parse the expression source as a sub-program
                    let mut lexer = crate::lexer::Lexer::new(&src);
                    let tokens = lexer.tokenize().map_err(|e| {
                        format!("error in string interpolation: {}", e)
                    })?;
                    let mut parser = Parser::new(tokens);
                    let expr = parser.parse_program().map_err(|e| {
                        format!("error in string interpolation: {}", e)
                    })?;
                    ast_parts.push(StringInterpPart::Expr(expr));
                }
            }
        }
        Ok(Box::new(ExprKind::StringInterp(ast_parts)))
    }

    fn parse_block(&mut self) -> Result<Expr, String> {
        self.advance(); // consume {
        if matches!(self.peek(), Token::RBrace) {
            // {} is a callable block that returns Unit
            self.advance();
            return Ok(Box::new(ExprKind::Block(Box::new(ExprKind::Unit))));
        }

        // Try to detect branching block: look for `pattern -> expr`
        // We need to speculatively check if this is a branching block.
        // Branching blocks have: ident/literal/_ followed eventually by ->
        if self.is_branch_block_start() {
            return self.parse_branch_block();
        }

        // Check for ambiguous `-` at the start of a block:
        // `{ -x }` could mean `{ in - x }` (subtraction sugar) or `{ (-x) }` (unary negation).
        // Reject it and require the user to be explicit.
        if matches!(self.peek(), Token::Minus) {
            return Err("ambiguous: `{ -expr }` could be subtraction or negation; write `{ in - expr }` or `{ (-expr) }`".to_string());
        }

        // Check for block sugar: { op x } => { in op x }
        let body = if self.is_binary_op_token() || matches!(self.peek(), Token::Pipe) {
            let in_expr = Box::new(ExprKind::Ident("in".to_string()));
            self.parse_sugar_body(in_expr)?
        } else {
            self.parse_expr(0)?
        };

        // Ternary sugar: { a | b } => { true -> a, false -> b }
        if matches!(self.peek(), Token::Bar) {
            self.advance();
            let false_branch = self.parse_expr(0)?;
            self.expect(&Token::RBrace)?;
            let arms = vec![
                BranchArm {
                    pattern: BranchPattern::Literal(Box::new(ExprKind::Bool(true))),
                    guard: None,
                    body,
                },
                BranchArm {
                    pattern: BranchPattern::Literal(Box::new(ExprKind::Bool(false))),
                    guard: None,
                    body: false_branch,
                },
            ];
            return Ok(Box::new(ExprKind::BranchBlock(arms)));
        }

        self.expect(&Token::RBrace)?;
        Ok(Box::new(ExprKind::Block(body)))
    }

    /// Check if the current position looks like the start of a branching block.
    /// We look for patterns like:
    /// - `Ident(` ... `) ->` (tag pattern)
    /// - `Ident ->` (binding or tag-no-payload)
    /// - `true ->`, `false ->` (literal bool)
    /// - `_ ->` (discard)
    /// - `Int ->`, `Str ->` etc. (literal patterns)
    fn is_branch_block_start(&self) -> bool {
        let mut offset = 0;
        // Skip past the first "pattern" to see if we find ->
        match self.peek_at(offset) {
            // Ident could be: tag pattern `Tag(x)`, or binding `x`
            Token::Ident(_) => {
                offset += 1;
                match self.peek_at(offset) {
                    Token::Arrow => return true,
                    Token::LParen => {
                        // Tag(x) -> ...  — skip to matching )
                        offset += 1;
                        let mut depth = 1;
                        while depth > 0 {
                            match self.peek_at(offset) {
                                Token::LParen => depth += 1,
                                Token::RParen => depth -= 1,
                                Token::Eof => return false,
                                _ => {}
                            }
                            offset += 1;
                        }
                        // After closing ), check for optional `if` guard then ->
                        if matches!(self.peek_at(offset), Token::Arrow) {
                            return true;
                        }
                        if matches!(self.peek_at(offset), Token::If) {
                            // Skip past guard to find ->
                            return true;
                        }
                    }
                    // Ident followed by `if` (guard) then `->`
                    Token::If => return true,
                    _ => {}
                }
            }
            Token::True | Token::False => {
                offset += 1;
                if matches!(self.peek_at(offset), Token::Arrow | Token::If) {
                    return true;
                }
            }
            Token::Minus => {
                // Negative literal pattern: -1 -> ...
                offset += 1;
                if matches!(self.peek_at(offset), Token::Int(_) | Token::Float(_)) {
                    offset += 1;
                    if matches!(self.peek_at(offset), Token::Arrow | Token::If) {
                        return true;
                    }
                }
            }
            Token::Int(_) | Token::Float(_) | Token::Str(_) | Token::Char(_) | Token::Byte(_) => {
                offset += 1;
                if matches!(self.peek_at(offset), Token::Arrow | Token::If) {
                    return true;
                }
            }
            Token::Underscore => {
                offset += 1;
                if matches!(self.peek_at(offset), Token::Arrow | Token::If) {
                    return true;
                }
            }
            Token::LParen => {
                // () -> ... (unit literal pattern)
                if matches!(self.peek_at(1), Token::RParen) {
                    offset += 2;
                    if matches!(self.peek_at(offset), Token::Arrow | Token::If) {
                        return true;
                    }
                }
            }
            _ => {}
        }
        false
    }

    /// Parse a branching block: { pattern -> expr, ... }
    fn parse_branch_block(&mut self) -> Result<Expr, String> {
        let mut arms = Vec::new();
        loop {
            if matches!(self.peek(), Token::RBrace) {
                break;
            }
            let pattern = self.parse_branch_pattern()?;
            let guard = if matches!(self.peek(), Token::If) {
                self.advance();
                Some(self.parse_expr(bp::PIPE_R)?)
            } else {
                None
            };
            self.expect(&Token::Arrow)?;
            let body = self.parse_expr(bp::SEMI_R)?;
            arms.push(BranchArm {
                pattern,
                guard,
                body,
            });
            if matches!(self.peek(), Token::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        self.expect(&Token::RBrace)?;
        Ok(Box::new(ExprKind::BranchBlock(arms)))
    }

    /// Parse a single branch pattern.
    fn parse_branch_pattern(&mut self) -> Result<BranchPattern, String> {
        match self.peek().clone() {
            Token::Underscore => {
                self.advance();
                Ok(BranchPattern::Discard)
            }
            Token::LParen if matches!(self.peek_at(1), Token::RParen) => {
                self.advance(); // (
                self.advance(); // )
                Ok(BranchPattern::Literal(Box::new(ExprKind::Unit)))
            }
            Token::True => {
                self.advance();
                Ok(BranchPattern::Literal(Box::new(ExprKind::Bool(true))))
            }
            Token::False => {
                self.advance();
                Ok(BranchPattern::Literal(Box::new(ExprKind::Bool(false))))
            }
            Token::Minus => {
                // Negative literal pattern: -1, -3.14, etc.
                self.advance();
                match self.peek().clone() {
                    Token::Int(n) => {
                        self.advance();
                        Ok(BranchPattern::Literal(Box::new(ExprKind::UnaryMinus(
                            Box::new(ExprKind::Int(n)),
                        ))))
                    }
                    Token::Float(f) => {
                        self.advance();
                        Ok(BranchPattern::Literal(Box::new(ExprKind::UnaryMinus(
                            Box::new(ExprKind::Float(f)),
                        ))))
                    }
                    tok => Err(format!("expected number after '-' in pattern, got {:?}", tok)),
                }
            }
            Token::Int(n) => {
                self.advance();
                Ok(BranchPattern::Literal(Box::new(ExprKind::Int(n))))
            }
            Token::Float(f) => {
                self.advance();
                Ok(BranchPattern::Literal(Box::new(ExprKind::Float(f))))
            }
            Token::Str(ref s) => {
                let s = s.clone();
                self.advance();
                Ok(BranchPattern::Literal(Box::new(ExprKind::Str(s))))
            }
            Token::Char(c) => {
                self.advance();
                Ok(BranchPattern::Literal(Box::new(ExprKind::Char(c))))
            }
            Token::Byte(b) => {
                self.advance();
                Ok(BranchPattern::Literal(Box::new(ExprKind::Byte(b))))
            }
            Token::Ident(ref name) => {
                let name = name.clone();
                self.advance();
                if matches!(self.peek(), Token::LParen) {
                    // Tag pattern: TagName(binding)
                    self.advance();
                    let binding = match self.peek() {
                        Token::Underscore => {
                            self.advance();
                            Some(BranchBinding::Discard)
                        }
                        Token::Ident(_) => {
                            let Token::Ident(bname) = self.advance() else {
                                unreachable!()
                            };
                            Some(BranchBinding::Name(bname))
                        }
                        Token::RParen => None,
                        _ => {
                            return Err(format!(
                                "expected binding in tag pattern, got {:?}",
                                self.peek()
                            ))
                        }
                    };
                    self.expect(&Token::RParen)?;
                    Ok(BranchPattern::Tag(name, binding))
                } else {
                    // Could be a tag name (no payload) or a catch-all binding
                    // We distinguish by convention: if it looks like it's followed by
                    // -> or if, it could be either. We'll treat uppercase-starting names
                    // as tag patterns and lowercase as bindings.
                    // Actually, we need to resolve at eval time since we don't know
                    // at parse time whether a name is a tag or a variable.
                    // For now: just store it as a Binding; eval will check if it's a tag.
                    Ok(BranchPattern::Binding(name))
                }
            }
            tok => Err(format!("expected pattern in branch arm, got {:?}", tok)),
        }
    }

    fn is_comparison_token(&self) -> bool {
        matches!(
            self.peek(),
            Token::EqEq | Token::NotEq | Token::Lt | Token::Gt | Token::LtEq | Token::GtEq
        )
    }

    fn check_no_chained_comparison(&self) -> Result<(), String> {
        if self.is_comparison_token() {
            Err("non-associative: chained comparisons require parentheses".to_string())
        } else {
            Ok(())
        }
    }

    fn is_binary_op_token(&self) -> bool {
        // Note: Token::Minus is NOT included here because `-` at the start
        // of a block is ambiguous between unary negation and binary subtraction.
        // We treat it as the start of a regular expression (unary minus).
        // Users should write `{ in - x }` for explicit subtraction sugar.
        matches!(
            self.peek(),
            Token::Plus
                | Token::Star
                | Token::Slash
                | Token::DotDot
                | Token::EqEq
                | Token::NotEq
                | Token::Lt
                | Token::Gt
                | Token::LtEq
                | Token::GtEq
        )
    }

    /// Parse a sugar block body starting with an implicit `in` as lhs.
    fn parse_sugar_body(&mut self, lhs: Expr) -> Result<Expr, String> {
        self.parse_expr_with_lhs(lhs, 0)
    }

    /// Resume Pratt parsing with an already-parsed lhs.
    fn parse_expr_with_lhs(&mut self, mut lhs: Expr, min_bp: u8) -> Result<Expr, String> {
        loop {
            lhs = match self.peek() {
                // ── Postfix: call f(...), f{block}, f[array], field a.x ──
                Token::LParen if min_bp <= bp::POSTFIX => {
                    self.advance();
                    let arg = self.parse_call_args()?;
                    Box::new(ExprKind::Call(lhs, arg))
                }
                Token::LBrace if min_bp <= bp::POSTFIX => {
                    let block = self.parse_block()?;
                    Box::new(ExprKind::Call(lhs, block))
                }
                Token::LBracket if min_bp <= bp::POSTFIX => {
                    let arr = self.parse_array()?;
                    Box::new(ExprKind::Call(lhs, arr))
                }
                Token::Dot if min_bp <= bp::POSTFIX => {
                    self.advance();
                    let field = match self.advance() {
                        Token::Ident(name) => name,
                        Token::Int(n) => n.to_string(),
                        tok => {
                            return Err(format!("expected field name after '.', got {:?}", tok))
                        }
                    };
                    match self.peek() {
                        Token::LParen if min_bp <= bp::POSTFIX => {
                            self.advance();
                            let arg = self.parse_call_args()?;
                            Box::new(ExprKind::MethodCall {
                                receiver: lhs,
                                method: field,
                                arg,
                            })
                        }
                        Token::LBrace if min_bp <= bp::POSTFIX => {
                            let block = self.parse_block()?;
                            Box::new(ExprKind::MethodCall {
                                receiver: lhs,
                                method: field,
                                arg: block,
                            })
                        }
                        Token::LBracket if min_bp <= bp::POSTFIX => {
                            let arr = self.parse_array()?;
                            Box::new(ExprKind::MethodCall {
                                receiver: lhs,
                                method: field,
                                arg: arr,
                            })
                        }
                        _ => Box::new(ExprKind::FieldAccess(lhs, field)),
                    }
                }
                // ── Infix: * / ──
                Token::Star if min_bp <= bp::MUL_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::MUL_R)?;
                    Box::new(ExprKind::BinOp(BinOp::Mul, lhs, rhs))
                }
                Token::Slash if min_bp <= bp::MUL_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::MUL_R)?;
                    if matches!(self.peek(), Token::Star | Token::Slash) {
                        return Err("ambiguous precedence: use parentheses around division".to_string());
                    }
                    Box::new(ExprKind::BinOp(BinOp::Div, lhs, rhs))
                }
                // ── Infix: + - ──
                Token::Plus if min_bp <= bp::ADD_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::ADD_R)?;
                    Box::new(ExprKind::BinOp(BinOp::Add, lhs, rhs))
                }
                Token::Minus if min_bp <= bp::ADD_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::ADD_R)?;
                    Box::new(ExprKind::BinOp(BinOp::Sub, lhs, rhs))
                }
                // ── Infix: comparisons (non-associative) ──
                Token::EqEq if min_bp <= bp::CMP_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::CMP_R)?;
                    self.check_no_chained_comparison()?;
                    Box::new(ExprKind::Compare(CmpOp::Eq, lhs, rhs))
                }
                Token::NotEq if min_bp <= bp::CMP_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::CMP_R)?;
                    self.check_no_chained_comparison()?;
                    Box::new(ExprKind::Compare(CmpOp::NotEq, lhs, rhs))
                }
                Token::Lt if min_bp <= bp::CMP_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::CMP_R)?;
                    self.check_no_chained_comparison()?;
                    Box::new(ExprKind::Compare(CmpOp::Lt, lhs, rhs))
                }
                Token::Gt if min_bp <= bp::CMP_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::CMP_R)?;
                    self.check_no_chained_comparison()?;
                    Box::new(ExprKind::Compare(CmpOp::Gt, lhs, rhs))
                }
                Token::LtEq if min_bp <= bp::CMP_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::CMP_R)?;
                    self.check_no_chained_comparison()?;
                    Box::new(ExprKind::Compare(CmpOp::LtEq, lhs, rhs))
                }
                Token::GtEq if min_bp <= bp::CMP_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::CMP_R)?;
                    self.check_no_chained_comparison()?;
                    Box::new(ExprKind::Compare(CmpOp::GtEq, lhs, rhs))
                }
                // ── Infix: range .. (non-associative) ──
                Token::DotDot if min_bp <= bp::RANGE_L => {
                    self.advance();
                    let rhs = self.parse_expr(bp::RANGE_R)?;
                    if matches!(self.peek(), Token::DotDot) {
                        return Err("non-associative: chained ranges require parentheses".to_string());
                    }
                    Box::new(ExprKind::Range(lhs, rhs))
                }
                // ── Infix: pipe >> ──
                Token::Pipe if min_bp <= bp::PIPE_L => {
                    self.advance();
                    self.parse_pipe_rhs(lhs, min_bp)?
                }
                // ── Infix: semicolon ──
                Token::Semicolon if min_bp <= bp::SEMI_L => {
                    self.advance();
                    self.parse_semicolon_rhs(lhs)?
                }
                _ => break,
            };
        }
        Ok(lhs)
    }

    fn parse_array(&mut self) -> Result<Expr, String> {
        self.advance(); // consume [
        let mut elems = Vec::new();
        if !matches!(self.peek(), Token::RBracket) {
            // Use SEMI_L + 1 so semicolons are NOT consumed inside array elements.
            // Without this, `[1; 2]` would silently discard `1` and produce `[2]`.
            elems.push(self.parse_expr(bp::SEMI_L + 1)?);
            while matches!(self.peek(), Token::Comma) {
                self.advance();
                if matches!(self.peek(), Token::RBracket) {
                    break;
                }
                elems.push(self.parse_expr(bp::SEMI_L + 1)?);
            }
        }
        self.expect(&Token::RBracket)?;
        Ok(Box::new(ExprKind::Array(elems)))
    }

    fn parse_paren(&mut self) -> Result<Expr, String> {
        self.advance(); // consume (
        // () = unit
        if matches!(self.peek(), Token::RParen) {
            self.advance();
            return Ok(Box::new(ExprKind::Unit));
        }

        // Check for labeled field or spread — definitely a struct
        if matches!(self.peek(), Token::Spread) {
            return self.parse_struct_fields(Vec::new());
        }
        if self.is_labeled_field() {
            return self.parse_struct_fields(Vec::new());
        }

        // Parse first expression
        let first = self.parse_expr(0)?;

        match self.peek() {
            Token::RParen => {
                // (expr) — just grouping, wrapped in Group to prevent
                // `let x = (expr)` from nesting into inner let chains
                self.advance();
                Ok(Box::new(ExprKind::Group(first)))
            }
            Token::Comma => {
                // (expr, ...) — tuple/struct
                self.advance();
                // Check for trailing comma: (expr,) — just grouping, not a struct.
                // Per spec: "(1) is just the parenthesized expression 1. There is
                // no distinct single-element tuple type."
                if matches!(self.peek(), Token::RParen) {
                    self.advance();
                    return Ok(Box::new(ExprKind::Group(first)));
                }
                let mut fields = vec![Field {
                    label: None,
                    value: first,
                    is_spread: false,
                }];
                self.parse_remaining_struct_fields(&mut fields)?;
                self.expect(&Token::RParen)?;
                Ok(Box::new(ExprKind::Struct(fields)))
            }
            _ => Err(format!(
                "expected ')', ',', or ';' in parenthesized expression, got {:?}",
                self.peek()
            )),
        }
    }

    fn parse_struct_fields(&mut self, mut fields: Vec<Field>) -> Result<Expr, String> {
        self.parse_remaining_struct_fields(&mut fields)?;
        self.expect(&Token::RParen)?;
        Ok(Box::new(ExprKind::Struct(fields)))
    }

    fn parse_remaining_struct_fields(&mut self, fields: &mut Vec<Field>) -> Result<(), String> {
        loop {
            if matches!(self.peek(), Token::RParen) {
                break;
            }
            if matches!(self.peek(), Token::Spread) {
                self.advance();
                let spread_expr = self.parse_expr(bp::SEMI_L + 1)?;
                fields.push(Field {
                    label: None,
                    value: spread_expr,
                    is_spread: true,
                });
            } else if self.is_labeled_field() {
                let (label, value) = self.parse_labeled_field()?;
                fields.push(Field {
                    label: Some(label),
                    value,
                    is_spread: false,
                });
            } else {
                let expr = self.parse_expr(bp::SEMI_L + 1)?;
                fields.push(Field {
                    label: None,
                    value: expr,
                    is_spread: false,
                });
            }
            if matches!(self.peek(), Token::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        Ok(())
    }

    fn is_labeled_field(&self) -> bool {
        matches!(
            (self.peek(), self.peek_at(1)),
            (Token::Ident(_), Token::Assign)
        )
    }

    fn parse_labeled_field(&mut self) -> Result<(String, Expr), String> {
        let Token::Ident(label) = self.advance() else {
            return Err("expected identifier for field label".to_string());
        };
        self.expect(&Token::Assign)?;
        let value = self.parse_expr(bp::SEMI_L + 1)?;
        Ok((label, value))
    }

    fn parse_standalone_let(&mut self) -> Result<Expr, String> {
        self.advance(); // consume 'let'

        // Check for `let _ = expr;` sugar (discard)
        if matches!(self.peek(), Token::Underscore) && matches!(self.peek_at(1), Token::Assign) {
            self.advance(); // consume _
            self.expect(&Token::Assign)?;
            let value_expr = self.parse_expr(bp::SEMI_L + 1)?;
            let new_let = Box::new(ExprKind::Let {
                pattern: Pattern::Discard,
                body: Box::new(ExprKind::Unit),
            });
            let result = Self::nest_let_in_expr(value_expr, new_let);
            return Ok(result);
        }

        // Check for `let name = expr;` sugar
        if let Token::Ident(_) = self.peek() {
            if matches!(self.peek_at(1), Token::Assign) {
                let Token::Ident(name) = self.advance() else {
                    unreachable!()
                };
                self.expect(&Token::Assign)?;
                // Parse value up to (but not including) the semicolon.
                // SEMI_R == SEMI_L (right-assoc), so use SEMI_L + 1 to stop before ';'.
                let value_expr = self.parse_expr(bp::SEMI_L + 1)?;
                // Desugar: let name = expr → Pipe(expr, Let { Name(name), body })
                // Body always returns the bound value. When `;` follows,
                // attach_body will replace this identity body with the continuation.
                let new_let = Box::new(ExprKind::Let {
                    pattern: Pattern::Name(name.clone()),
                    body: Box::new(ExprKind::Ident(name.clone())),
                });
                // If value_expr ends in a replaceable let (e.g., tag(Foo) →
                // Pipe(NewTag, Let{Foo, Ident})), nest our new let inside it
                // so that both bindings are in scope for the continuation.
                let result = Self::nest_let_in_expr(value_expr, new_let);
                return Ok(result);
            }
        }

        let (pattern, is_array) = self.parse_let_pattern()?;

        // Check for `let [...] = expr;` or `let (...) = expr;` sugar
        if matches!(self.peek(), Token::Assign) {
            self.advance(); // consume '='
            let value_expr = self.parse_expr(bp::SEMI_L + 1)?;
            let new_let = if is_array {
                let array_pats = self.pattern_to_array_pats(pattern)?;
                Box::new(ExprKind::LetArray {
                    patterns: array_pats,
                    body: Box::new(ExprKind::Unit),
                })
            } else {
                Box::new(ExprKind::Let {
                    pattern,
                    body: Box::new(ExprKind::Unit),
                })
            };
            let result = Self::nest_let_in_expr(value_expr, new_let);
            return Ok(result);
        }

        if is_array {
            let array_pats = self.pattern_to_array_pats(pattern)?;
            Ok(Box::new(ExprKind::LetArray {
                patterns: array_pats,
                body: Box::new(ExprKind::Unit), // placeholder, filled by ;
            }))
        } else {
            Ok(Box::new(ExprKind::Let {
                pattern,
                body: Box::new(ExprKind::Unit), // placeholder, filled by ;
            }))
        }
    }

    fn parse_tag_sugar(&mut self) -> Result<Expr, String> {
        self.advance(); // consume 'tag'
        self.expect(&Token::LParen)?;
        let name = match self.advance() {
            Token::Ident(n) => n,
            tok => return Err(format!("expected identifier in tag(), got {:?}", tok)),
        };
        self.expect(&Token::RParen)?;
        let tag_id = next_tag_id();
        // Body always returns the bound value. When `;` follows,
        // attach_body will replace this identity body with the continuation.
        let body = Box::new(ExprKind::Ident(name.clone()));
        Ok(Box::new(ExprKind::Pipe(
            Box::new(ExprKind::NewTag(tag_id, Some(name.clone()))),
            Box::new(ExprKind::Let {
                pattern: Pattern::Name(name),
                body,
            }),
        )))
    }

    fn parse_import(&mut self) -> Result<Expr, String> {
        self.advance(); // consume 'import'
        self.expect(&Token::LParen)?;
        let name = match self.advance() {
            Token::Str(s) => s,
            tok => return Err(format!("expected string in import(), got {:?}", tok)),
        };
        self.expect(&Token::RParen)?;
        Ok(Box::new(ExprKind::Import(name)))
    }

    fn parse_use(&mut self) -> Result<Expr, String> {
        self.advance(); // consume 'use'
        self.expect(&Token::LParen)?;
        let name = match self.advance() {
            Token::Ident(n) => n,
            tok => return Err(format!("expected identifier in use(), got {:?}", tok)),
        };
        self.expect(&Token::RParen)?;
        // Body always returns the bound value. When `;` follows,
        // attach_body will replace this identity body with the continuation.
        let body = Box::new(ExprKind::Ident(name.clone()));
        Ok(Box::new(ExprKind::Pipe(
            Box::new(ExprKind::Import(name.clone())),
            Box::new(ExprKind::Let {
                pattern: Pattern::Name(name),
                body,
            }),
        )))
    }

    // ── Let pattern parsing ──────────────────────────────────────────

    fn parse_let_pattern(&mut self) -> Result<(Pattern, bool), String> {
        match self.peek() {
            Token::LParen => {
                self.advance();
                let pat = self.parse_destructure_pattern()?;
                self.expect(&Token::RParen)?;
                Ok((pat, false))
            }
            Token::LBracket => {
                self.advance();
                let pat = self.parse_destructure_pattern()?;
                self.expect(&Token::RBracket)?;
                Ok((pat, true))
            }
            _ => Err(format!(
                "expected '(' or '[' after 'let', got {:?}",
                self.peek()
            )),
        }
    }

    fn parse_destructure_pattern(&mut self) -> Result<Pattern, String> {
        // Handle underscore
        if matches!(self.peek(), Token::Underscore) {
            self.advance();
            if matches!(self.peek(), Token::RParen | Token::RBracket) {
                return Ok(Pattern::Discard);
            }
            let mut fields = vec![PatField {
                label: None,
                binding: "_".to_string(),
                is_rest: false,
            }];
            if matches!(self.peek(), Token::Comma) {
                self.advance();
                self.parse_remaining_pat_fields(&mut fields)?;
            }
            return Ok(Pattern::Fields(fields));
        }

        // Handle spread
        if matches!(self.peek(), Token::Spread) {
            self.advance();
            let name = if let Token::Ident(_) = self.peek() {
                let Token::Ident(n) = self.advance() else {
                    unreachable!()
                };
                n
            } else {
                "_".to_string()
            };
            let mut fields = vec![PatField {
                label: None,
                binding: name,
                is_rest: true,
            }];
            if matches!(self.peek(), Token::Comma) {
                self.advance();
                self.parse_remaining_pat_fields(&mut fields)?;
            }
            return Ok(Pattern::Fields(fields));
        }

        // Handle labeled destructuring
        if self.is_labeled_pat_field() {
            let mut fields = Vec::new();
            self.parse_remaining_pat_fields(&mut fields)?;
            return Ok(Pattern::Fields(fields));
        }

        // Identifier — single or start of multi-field
        if let Token::Ident(_) = self.peek() {
            let Token::Ident(name) = self.advance() else {
                unreachable!()
            };
            if matches!(self.peek(), Token::Comma) {
                self.advance();
                let mut fields = vec![PatField {
                    label: None,
                    binding: name,
                    is_rest: false,
                }];
                self.parse_remaining_pat_fields(&mut fields)?;
                Ok(Pattern::Fields(fields))
            } else {
                Ok(Pattern::Name(name))
            }
        } else {
            Err(format!(
                "expected pattern in let, got {:?}",
                self.peek()
            ))
        }
    }

    fn is_labeled_pat_field(&self) -> bool {
        matches!(
            (self.peek(), self.peek_at(1)),
            (Token::Ident(_), Token::Assign)
        )
    }

    fn parse_remaining_pat_fields(&mut self, fields: &mut Vec<PatField>) -> Result<(), String> {
        loop {
            if matches!(self.peek(), Token::RParen | Token::RBracket) {
                break;
            }
            if matches!(self.peek(), Token::Spread) {
                if fields.iter().any(|f| f.is_rest) {
                    return Err("multiple rest patterns (...) in one destructuring".to_string());
                }
                self.advance();
                let name = if let Token::Ident(_) = self.peek() {
                    let Token::Ident(n) = self.advance() else {
                        unreachable!()
                    };
                    n
                } else {
                    "_".to_string()
                };
                fields.push(PatField {
                    label: None,
                    binding: name,
                    is_rest: true,
                });
            } else if matches!(self.peek(), Token::Underscore) {
                self.advance();
                fields.push(PatField {
                    label: None,
                    binding: "_".to_string(),
                    is_rest: false,
                });
            } else if self.is_labeled_pat_field() {
                let Token::Ident(label) = self.advance() else {
                    unreachable!()
                };
                self.expect(&Token::Assign)?;
                let binding = match self.peek() {
                    Token::Ident(_) => {
                        let Token::Ident(b) = self.advance() else { unreachable!() };
                        b
                    }
                    Token::Underscore => {
                        self.advance();
                        "_".to_string()
                    }
                    _ => return Err("expected binding name after '=' in pattern".to_string()),
                };
                fields.push(PatField {
                    label: Some(label),
                    binding,
                    is_rest: false,
                });
            } else if let Token::Ident(_) = self.peek() {
                let Token::Ident(name) = self.advance() else {
                    unreachable!()
                };
                fields.push(PatField {
                    label: None,
                    binding: name,
                    is_rest: false,
                });
            } else {
                break;
            }
            if matches!(self.peek(), Token::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        Ok(())
    }

    // ── Call args parsing ────────────────────────────────────────────

    fn parse_call_args(&mut self) -> Result<Expr, String> {
        if matches!(self.peek(), Token::RParen) {
            self.advance();
            return Ok(Box::new(ExprKind::Unit));
        }

        if self.is_labeled_field() || matches!(self.peek(), Token::Spread) {
            return self.parse_struct_fields(Vec::new());
        }

        let first = self.parse_expr(bp::SEMI_L + 1)?;

        match self.peek() {
            Token::RParen => {
                self.advance();
                Ok(first)
            }
            Token::Comma => {
                self.advance();
                // Trailing comma: f(expr,) — pass expr directly, not a struct.
                // Per spec: no single-element tuple type.
                if matches!(self.peek(), Token::RParen) {
                    self.advance();
                    return Ok(first);
                }
                let mut fields = vec![Field {
                    label: None,
                    value: first,
                    is_spread: false,
                }];
                self.parse_remaining_struct_fields(&mut fields)?;
                self.expect(&Token::RParen)?;
                Ok(Box::new(ExprKind::Struct(fields)))
            }
            _ => Err(format!(
                "expected ')' or ',' in function call, got {:?}",
                self.peek()
            )),
        }
    }
}
