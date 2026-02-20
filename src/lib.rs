pub mod ast;
pub mod eval;
pub mod lexer;
pub mod parser;
pub mod value;

use std::collections::HashMap;

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

/// Parse source code and return the list of module names referenced by `import()`.
pub fn imports(source: &str) -> Result<Vec<String>, String> {
    let ast = parse(source)?;
    Ok(ast::collect_imports(&ast))
}

/// Run source code and return the resulting value.
/// No modules are available — `import()` will error.
pub fn run(source: &str) -> Result<Value, String> {
    let (val, _warnings) = run_with_warnings(source)?;
    Ok(val)
}

/// Run source code and return the value plus any warnings.
/// No modules are available — `import()` will error.
pub fn run_with_warnings(source: &str) -> Result<(Value, Vec<String>), String> {
    let ast = parse(source)?;
    let env = eval::default_env();
    let (val, final_env) = eval::eval_toplevel(&ast, &env, &Value::Unit)
        .map_err(|e| format!("runtime error: {}", e))?;
    let warnings = final_env.unused_warnings();
    Ok((val, warnings))
}

/// Run source code with provided modules.
/// Modules are name-value pairs that can be referenced via `import(name)`.
pub fn run_with_modules(source: &str, modules: &[(&str, Value)]) -> Result<Value, String> {
    let (val, _warnings) = run_with_modules_and_warnings(source, modules)?;
    Ok(val)
}

/// Run source code with provided modules, returning value + warnings.
pub fn run_with_modules_and_warnings(
    source: &str,
    modules: &[(&str, Value)],
) -> Result<(Value, Vec<String>), String> {
    let ast = parse(source)?;
    let module_map: HashMap<String, Value> = modules
        .iter()
        .map(|(name, val)| (name.to_string(), val.clone()))
        .collect();
    let env = eval::default_env_with_modules(module_map);
    let (val, final_env) = eval::eval_toplevel(&ast, &env, &Value::Unit)
        .map_err(|e| format!("runtime error: {}", e))?;
    let warnings = final_env.unused_warnings();
    Ok((val, warnings))
}

/// Run source code with a given environment, returning both the value
/// and the updated environment. Used by the REPL to persist bindings.
pub fn run_in_env(source: &str, env: &Env) -> Result<(Value, Env), String> {
    let ast = parse(source)?;
    eval::eval_toplevel(&ast, env, &Value::Unit)
        .map_err(|e| format!("runtime error: {}", e))
}

/// Create the default environment with builtins (no modules).
pub fn default_env() -> Env {
    eval::default_env()
}

/// Create the default environment with builtins and provided modules.
pub fn default_env_with_modules(modules: HashMap<String, Value>) -> Env {
    eval::default_env_with_modules(modules)
}
