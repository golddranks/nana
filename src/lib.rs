pub mod ast;
pub mod eval;
pub mod lexer;
pub mod mir;
pub mod parser;
pub mod value;

use std::collections::HashMap;

use ast::Expr;
pub use value::Env;
use value::Value;

/// The std module source code, written in nana.
const STD_SOURCE: &str = include_str!("std.nana");

/// Evaluate the std module source with core available, returning the std module value.
fn eval_std_module() -> Result<Value, String> {
    let core = eval::build_core_module();
    let mut modules = HashMap::new();
    modules.insert("core".to_string(), core);
    let mut env = eval::default_env_with_modules(modules);
    // std source needs method_set as a bare builtin to build method sets
    env = env.bind(
        "method_set".to_string(),
        Value::BuiltinFn("method_set".to_string()),
    );
    let ast = parse(STD_SOURCE)?;
    let mir = mir::lower(&ast);
    let (val, _) = eval::eval_toplevel(&mir, &env, &Value::Unit)
        .map_err(|e| format!("std module error: {}", e))?;
    Ok(val)
}

/// Parse source code into an AST.
pub fn parse(source: &str) -> Result<Expr, String> {
    let mut lex = lexer::Lexer::new(source);
    let tokens = lex.tokenize().map_err(|e| format!("lexer error: {}", e))?;
    let mut par = parser::Parser::new(tokens);
    par.parse_program()
        .map_err(|e| format!("parse error: {}", e))
}

/// Parse and lower source code into MIR.
pub fn parse_and_lower(source: &str) -> Result<mir::Mir, String> {
    let ast = parse(source)?;
    Ok(mir::lower(&ast))
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
    let mir = parse_and_lower(source)?;
    let env = eval::default_env();
    let (val, final_env) = eval::eval_toplevel(&mir, &env, &Value::Unit)
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
    let mir = parse_and_lower(source)?;
    let module_map: HashMap<String, Value> = modules
        .iter()
        .map(|(name, val)| (name.to_string(), val.clone()))
        .collect();
    let env = eval::default_env_with_modules(module_map);
    let (val, final_env) = eval::eval_toplevel(&mir, &env, &Value::Unit)
        .map_err(|e| format!("runtime error: {}", e))?;
    let warnings = final_env.unused_warnings();
    Ok((val, warnings))
}

/// Run source code with a given environment, returning both the value
/// and the updated environment. Used by the REPL to persist bindings.
pub fn run_in_env(source: &str, env: &Env) -> Result<(Value, Env), String> {
    let mir = parse_and_lower(source)?;
    eval::eval_toplevel(&mir, env, &Value::Unit).map_err(|e| format!("runtime error: {}", e))
}

/// Create the default environment with builtins (no modules).
pub fn default_env() -> Env {
    eval::default_env()
}

/// Create the default environment with builtins and provided modules.
pub fn default_env_with_modules(modules: HashMap<String, Value>) -> Env {
    eval::default_env_with_modules(modules)
}

/// Create an environment with core and std modules available.
/// `std` is automatically bound and its prelude method sets are applied,
/// so operators (+, ==, etc.) work without explicit `use(std)` or `apply()`.
pub fn env_with_std() -> Result<Env, String> {
    let core = eval::build_core_module();
    let std_val = eval_std_module()?;
    let mut modules = HashMap::new();
    modules.insert("core".to_string(), core);
    modules.insert("std".to_string(), std_val.clone());
    let mut env = eval::default_env_with_modules(modules);
    // Bind std so programs can access std.map, std.filter, etc.
    // Use bind_used so implicit std doesn't trigger unused-binding warnings.
    env = env.bind_used("std".to_string(), std_val.clone());
    // Auto-apply prelude method sets so operators work out of the box
    if let Value::Struct(fields) = &std_val {
        for (label, val) in fields {
            if label == "prelude" {
                if let Value::Struct(prelude_fields) = val {
                    for (_, ms_val) in prelude_fields {
                        if matches!(ms_val, Value::MethodSet { .. }) {
                            env = env.bind(format!("\0ms"), ms_val.clone());
                        }
                    }
                }
            }
        }
    }
    Ok(env)
}

/// Run source code with core and std modules available.
pub fn run_with_std(source: &str) -> Result<Value, String> {
    let (val, _warnings) = run_with_std_and_warnings(source)?;
    Ok(val)
}

/// Run source code with core and std modules, returning value + warnings.
pub fn run_with_std_and_warnings(source: &str) -> Result<(Value, Vec<String>), String> {
    let mir = parse_and_lower(source)?;
    let env = env_with_std()?;
    let (val, final_env) = eval::eval_toplevel(&mir, &env, &Value::Unit)
        .map_err(|e| format!("runtime error: {}", e))?;
    let warnings = final_env.unused_warnings();
    Ok((val, warnings))
}
