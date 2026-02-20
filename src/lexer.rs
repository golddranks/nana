#[derive(Debug, Clone, PartialEq)]
pub enum StringPart {
    Literal(String),
    Expr(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    Int(i64),
    /// Integer literal that overflowed i64 but fits in u64.
    /// Only valid when preceded by unary minus (e.g., -9223372036854775808).
    BigInt(u64),
    Float(f64),
    Str(String),
    Char(char),
    Byte(u8),

    // Interpolated string: "hello, {name}!"
    InterpStr(Vec<StringPart>),

    // Delimiters
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    EqEq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    Pipe,   // >>
    Bar,    // |
    Arrow,  // ->
    DotDot, // ..
    Dot,
    Spread, // ...

    // Punctuation
    Comma,
    Semicolon,
    Assign, // =

    // Underscore
    Underscore,

    // Keywords
    Let,
    In,
    If, // only used for guards in branching arms
    Tag,
    NewTag,
    True,
    False,
    Import,
    Use,

    // Identifier
    Ident(String),

    Eof,
}

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.input.len() {
                tokens.push(Token::Eof);
                break;
            }
            let tok = self.next_token()?;
            tokens.push(tok);
        }
        Ok(tokens)
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.input.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.input.get(self.pos).copied();
        self.pos += 1;
        ch
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip whitespace
            while self.pos < self.input.len() && self.input[self.pos].is_whitespace() {
                self.pos += 1;
            }
            // Skip comments
            if self.peek() == Some('#') {
                while self.pos < self.input.len() && self.input[self.pos] != '\n' {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn next_token(&mut self) -> Result<Token, String> {
        let ch = self.peek().unwrap();

        match ch {
            '(' => {
                self.advance();
                // Check for () — unit
                if self.peek() == Some(')') {
                    // Don't consume — let the parser handle unit detection
                    // Actually, we should let parser decide if () is unit or empty parens
                    Ok(Token::LParen)
                } else {
                    Ok(Token::LParen)
                }
            }
            ')' => {
                self.advance();
                Ok(Token::RParen)
            }
            '[' => {
                self.advance();
                Ok(Token::LBracket)
            }
            ']' => {
                self.advance();
                Ok(Token::RBracket)
            }
            '{' => {
                self.advance();
                Ok(Token::LBrace)
            }
            '}' => {
                self.advance();
                Ok(Token::RBrace)
            }
            '+' => {
                self.advance();
                Ok(Token::Plus)
            }
            '*' => {
                self.advance();
                Ok(Token::Star)
            }
            '/' => {
                self.advance();
                Ok(Token::Slash)
            }
            '-' => {
                self.advance();
                if self.peek() == Some('>') {
                    self.advance();
                    Ok(Token::Arrow)
                } else {
                    Ok(Token::Minus)
                }
            }
            '>' => {
                self.advance();
                if self.peek() == Some('>') {
                    self.advance();
                    Ok(Token::Pipe)
                } else if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::GtEq)
                } else {
                    Ok(Token::Gt)
                }
            }
            '<' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::LtEq)
                } else {
                    Ok(Token::Lt)
                }
            }
            '=' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::EqEq)
                } else {
                    Ok(Token::Assign)
                }
            }
            '!' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Token::NotEq)
                } else {
                    Err("unexpected '!', did you mean '!='?".to_string())
                }
            }
            '.' => {
                self.advance();
                if self.peek() == Some('.') {
                    self.advance();
                    if self.peek() == Some('.') {
                        self.advance();
                        Ok(Token::Spread)
                    } else {
                        Ok(Token::DotDot)
                    }
                } else {
                    Ok(Token::Dot)
                }
            }
            '|' => {
                self.advance();
                Ok(Token::Bar)
            }
            ',' => {
                self.advance();
                Ok(Token::Comma)
            }
            ';' => {
                self.advance();
                Ok(Token::Semicolon)
            }
            '"' => self.lex_string(),
            '\'' => self.lex_char(),
            'b' if self.peek_at(1) == Some('\'') => self.lex_byte(),
            '0'..='9' => self.lex_number(),
            '_' => {
                self.advance();
                if self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    // _name — lex as identifier
                    let mut name = String::from('_');
                    while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        name.push(self.advance().unwrap());
                    }
                    Ok(Token::Ident(name))
                } else {
                    Ok(Token::Underscore)
                }
            }
            '\\' if self.peek_at(1) == Some('\\') => self.lex_multiline_string(),
            c if c.is_alphabetic() => self.lex_ident_or_keyword(),
            _ => Err(format!("unexpected character: '{}'", ch)),
        }
    }

    fn lex_string(&mut self) -> Result<Token, String> {
        self.advance(); // consume opening "
        let mut current = String::new();
        let mut parts: Vec<StringPart> = Vec::new();
        let mut has_interp = false;
        loop {
            match self.advance() {
                None => return Err("unterminated string literal".to_string()),
                Some('"') => {
                    if has_interp {
                        if !current.is_empty() {
                            parts.push(StringPart::Literal(current));
                        }
                        return Ok(Token::InterpStr(parts));
                    } else {
                        return Ok(Token::Str(current));
                    }
                }
                Some('{') => {
                    // Start of interpolated expression
                    has_interp = true;
                    if !current.is_empty() {
                        parts.push(StringPart::Literal(current));
                        current = String::new();
                    }
                    // Collect expression source until matching '}'
                    let expr_src = self.lex_interp_expr()?;
                    parts.push(StringPart::Expr(expr_src));
                }
                Some('\\') => match self.advance() {
                    Some('n') => current.push('\n'),
                    Some('t') => current.push('\t'),
                    Some('r') => current.push('\r'),
                    Some('\\') => current.push('\\'),
                    Some('"') => current.push('"'),
                    Some('0') => current.push('\0'),
                    Some('{') => current.push('{'),
                    Some('}') => current.push('}'),
                    Some('x') => {
                        let b = self.lex_hex_byte()?;
                        current.push(b as char);
                    }
                    Some(c) => return Err(format!("unknown escape sequence: \\{}", c)),
                    None => return Err("unterminated escape in string".to_string()),
                },
                Some(c) => current.push(c),
            }
        }
    }

    /// Collect characters for an interpolated expression inside `{...}` in a string.
    /// Handles nested braces, strings, and char literals so that `}` inside those
    /// doesn't prematurely end the expression.
    fn lex_interp_expr(&mut self) -> Result<String, String> {
        let mut expr = String::new();
        let mut depth = 1; // we already consumed the opening '{'
        loop {
            match self.advance() {
                None => return Err("unterminated interpolation in string".to_string()),
                Some('{') => {
                    depth += 1;
                    expr.push('{');
                }
                Some('}') => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(expr);
                    }
                    expr.push('}');
                }
                Some('"') => {
                    // Nested string literal — consume until closing "
                    expr.push('"');
                    loop {
                        match self.advance() {
                            None => return Err("unterminated string inside interpolation".to_string()),
                            Some('\\') => {
                                expr.push('\\');
                                if let Some(c) = self.advance() {
                                    expr.push(c);
                                }
                            }
                            Some('"') => {
                                expr.push('"');
                                break;
                            }
                            Some(c) => expr.push(c),
                        }
                    }
                }
                Some('\'') => {
                    // Char literal — consume until closing '
                    expr.push('\'');
                    match self.advance() {
                        None => return Err("unterminated char inside interpolation".to_string()),
                        Some('\\') => {
                            expr.push('\\');
                            if let Some(c) = self.advance() {
                                expr.push(c);
                            }
                        }
                        Some(c) => expr.push(c),
                    }
                    match self.advance() {
                        Some('\'') => expr.push('\''),
                        _ => return Err("expected closing ' in char literal inside interpolation".to_string()),
                    }
                }
                Some('#') => {
                    // Comment inside interpolation — consume until newline
                    expr.push('#');
                    while self.pos < self.input.len() && self.input[self.pos] != '\n' {
                        expr.push(self.advance().unwrap());
                    }
                }
                Some(c) => expr.push(c),
            }
        }
    }

    /// Lex a Zig-style multi-line string: each line starts with `\\`.
    /// Lines are joined with newlines. Leading whitespace before `\\` on
    /// continuation lines is stripped.
    fn lex_multiline_string(&mut self) -> Result<Token, String> {
        let mut s = String::new();
        loop {
            // Consume the `\\` prefix
            if self.peek() == Some('\\') && self.peek_at(1) == Some('\\') {
                self.advance(); // first '\'
                self.advance(); // second '\'
            } else {
                break;
            }
            // Consume the rest of the line (until newline or EOF)
            while self.pos < self.input.len() && self.input[self.pos] != '\n' {
                s.push(self.advance().unwrap());
            }
            // Consume the newline if present
            if self.peek() == Some('\n') {
                self.advance();
                s.push('\n');
            }
            // Skip whitespace on the next line to find the next `\\`
            while self.peek().is_some_and(|c| c == ' ' || c == '\t') {
                self.advance();
            }
        }
        // Remove trailing newline (the last line's newline is an artifact)
        if s.ends_with('\n') {
            s.pop();
        }
        Ok(Token::Str(s))
    }

    fn lex_char(&mut self) -> Result<Token, String> {
        self.advance(); // consume opening '
        let c = match self.advance() {
            Some('\\') => match self.advance() {
                Some('n') => '\n',
                Some('t') => '\t',
                Some('r') => '\r',
                Some('\\') => '\\',
                Some('\'') => '\'',
                Some('0') => '\0',
                Some('x') => {
                    let b = self.lex_hex_byte()?;
                    b as char
                }
                Some(c) => return Err(format!("unknown escape in char literal: \\{}", c)),
                None => return Err("unterminated char literal".to_string()),
            },
            Some(c) => c,
            None => return Err("unterminated char literal".to_string()),
        };
        match self.advance() {
            Some('\'') => Ok(Token::Char(c)),
            _ => Err("expected closing ' in char literal".to_string()),
        }
    }

    fn lex_byte(&mut self) -> Result<Token, String> {
        self.advance(); // consume 'b'
        self.advance(); // consume opening '
        let b = match self.advance() {
            Some('\\') => match self.advance() {
                Some('n') => b'\n',
                Some('t') => b'\t',
                Some('r') => b'\r',
                Some('\\') => b'\\',
                Some('\'') => b'\'',
                Some('0') => 0,
                Some('x') => self.lex_hex_byte()?,
                Some(c) => return Err(format!("unknown escape in byte literal: \\{}", c)),
                None => return Err("unterminated byte literal".to_string()),
            },
            Some(c) if c.is_ascii() => c as u8,
            Some(c) => return Err(format!("non-ASCII character in byte literal: '{}'", c)),
            None => return Err("unterminated byte literal".to_string()),
        };
        match self.advance() {
            Some('\'') => Ok(Token::Byte(b)),
            _ => Err("expected closing ' in byte literal".to_string()),
        }
    }

    /// Parse two hex digits after `\x` and return the byte value.
    fn lex_hex_byte(&mut self) -> Result<u8, String> {
        let hi = self.advance().ok_or("unterminated \\x escape")?;
        let lo = self.advance().ok_or("unterminated \\x escape")?;
        if !hi.is_ascii_hexdigit() || !lo.is_ascii_hexdigit() {
            return Err(format!("invalid hex escape: \\x{}{}", hi, lo));
        }
        let val = (hex_digit(hi) << 4) | hex_digit(lo);
        Ok(val)
    }

    fn lex_number(&mut self) -> Result<Token, String> {
        let mut num_str = String::new();

        // Check for hex: 0x...
        if self.peek() == Some('0') && self.peek_at(1) == Some('x') {
            self.advance(); // 0
            self.advance(); // x
            let mut hex = String::new();
            while self.peek().is_some_and(|c| c.is_ascii_hexdigit() || c == '_') {
                let c = self.advance().unwrap();
                if c != '_' {
                    hex.push(c);
                }
            }
            if hex.is_empty() {
                return Err("expected hex digits after 0x".to_string());
            }
            let val =
                i64::from_str_radix(&hex, 16).map_err(|e| format!("invalid hex literal: {}", e))?;
            return Ok(Token::Int(val));
        }

        // Check for binary: 0b...
        if self.peek() == Some('0') && self.peek_at(1) == Some('b') {
            // Make sure it's not b' (byte literal prefix — but that's handled separately
            // in next_token before lex_number, so 0b here is always binary)
            self.advance(); // 0
            self.advance(); // b
            let mut bin = String::new();
            while self.peek().is_some_and(|c| c == '0' || c == '1' || c == '_') {
                let c = self.advance().unwrap();
                if c != '_' {
                    bin.push(c);
                }
            }
            if bin.is_empty() {
                return Err("expected binary digits after 0b".to_string());
            }
            let val =
                i64::from_str_radix(&bin, 2).map_err(|e| format!("invalid binary literal: {}", e))?;
            return Ok(Token::Int(val));
        }

        while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '_') {
            let c = self.advance().unwrap();
            if c != '_' {
                num_str.push(c);
            }
        }

        // Check for float: digits followed by '.' followed by digit (not '..')
        if self.peek() == Some('.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
            num_str.push('.');
            self.advance(); // consume '.'
            while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '_') {
                let c = self.advance().unwrap();
                if c != '_' {
                    num_str.push(c);
                }
            }
            let val: f64 = num_str
                .parse()
                .map_err(|e| format!("invalid float literal: {}", e))?;
            return Ok(Token::Float(val));
        }

        match num_str.parse::<i64>() {
            Ok(val) => Ok(Token::Int(val)),
            Err(_) => {
                // Overflows i64 — try u64 so the parser can handle -BigInt
                match num_str.parse::<u64>() {
                    Ok(val) => Ok(Token::BigInt(val)),
                    Err(_) => Err(format!("integer literal too large: {}", num_str)),
                }
            }
        }
    }

    fn lex_ident_or_keyword(&mut self) -> Result<Token, String> {
        let mut name = String::new();
        while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
            name.push(self.advance().unwrap());
        }
        let tok = match name.as_str() {
            "let" => Token::Let,
            "in" => Token::In,
            "if" => Token::If,
            "tag" => Token::Tag,
            "new_tag" => Token::NewTag,
            "true" => Token::True,
            "false" => Token::False,
            "import" => Token::Import,
            "use" => Token::Use,
            _ => Token::Ident(name),
        };
        Ok(tok)
    }
}

fn hex_digit(c: char) -> u8 {
    match c {
        '0'..='9' => c as u8 - b'0',
        'a'..='f' => c as u8 - b'a' + 10,
        'A'..='F' => c as u8 - b'A' + 10,
        _ => unreachable!(),
    }
}
