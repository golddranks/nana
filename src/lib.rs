pub mod ast;
pub mod eval;
pub mod lexer;
pub mod parser;
pub mod value;

use ast::Expr;
pub use value::Env;
use value::Value;

/// Parse source code into an AST.
pub fn parse(source: &str) -> Result<Expr, String> {
    let mut lex = lexer::Lexer::new(source);
    let tokens = lex.tokenize().map_err(|e| format!("lexer error: {}", e))?;
    let mut par = parser::Parser::new(tokens);
    par.parse_program().map_err(|e| format!("parse error: {}", e))
}

/// Run source code and return the resulting value.
pub fn run(source: &str) -> Result<Value, String> {
    let ast = parse(source)?;
    let env = eval::default_env();
    eval::eval(&ast, &env, &Value::Unit)
        .map_err(|e| format!("runtime error: {}", e))
}

/// Run source code with a given environment, returning both the value
/// and the updated environment. Used by the REPL to persist bindings.
pub fn run_in_env(source: &str, env: &Env) -> Result<(Value, Env), String> {
    let ast = parse(source)?;
    eval::eval_toplevel(&ast, env, &Value::Unit)
        .map_err(|e| format!("runtime error: {}", e))
}

/// Create the default environment with builtins.
pub fn default_env() -> Env {
    eval::default_env()
}
