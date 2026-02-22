pub mod ast;
pub mod eval;
pub mod lexer;
pub mod parser;
pub mod value;

use std::collections::HashMap;

use ast::Expr;
pub use value::Env;
use value::Value;

/// The std module source code, written in nana.
const STD_SOURCE: &str = r#"
use(core);

let _array_methods = method_set(core.Array, (
    get = core.array_get,
    slice = core.array_slice,
    len = core.array_len,
    map = core.array_map,
    filter = core.array_filter,
    fold = core.array_fold,
    zip = core.array_zip,
));

let _string_methods = method_set(core.String, (
    byte_len = core.string_byte_len,
    char_len = core.string_char_len,
    byte_get = core.string_byte_get,
    char_get = core.string_char_get,
    as_bytes = core.string_as_bytes,
    chars = core.string_chars,
    split = core.string_split,
    trim = core.string_trim,
    contains = core.string_contains,
    slice = core.string_slice,
    starts_with = core.string_starts_with,
    ends_with = core.string_ends_with,
    replace = core.string_replace,
));

(
    Array = core.Array,
    String = core.String,
    Int = core.Int,
    Float = core.Float,
    Bool = core.Bool,
    Char = core.Char,
    Byte = core.Byte,
    Unit = core.Unit,

    array_methods = _array_methods,
    string_methods = _string_methods,

    not = core.not,
    and = core.and,
    or = core.or,
    len = core.len,
    print = core.print,
    map = core.map,
    filter = core.filter,
    fold = core.fold,
    zip = core.zip,
    byte = core.byte,
    int = core.int,
    float = core.float,
    char = core.char,
    ref_eq = core.ref_eq,
    val_eq = core.val_eq,
    method_set = core.method_set,
)
"#;

/// Evaluate the std module source with core available, returning the std module value.
fn eval_std_module() -> Result<Value, String> {
    let core = eval::build_core_module();
    let mut modules = HashMap::new();
    modules.insert("core".to_string(), core);
    // std needs method_set builtin to build method sets
    let env = eval::default_env_with_modules(modules);
    let ast = parse(STD_SOURCE)?;
    let (val, _) = eval::eval_toplevel(&ast, &env, &Value::Unit)
        .map_err(|e| format!("std module error: {}", e))?;
    Ok(val)
}

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

/// Create an environment with core and std modules available.
/// Programs can `use(std)` to access all builtins and method sets.
pub fn env_with_std() -> Result<Env, String> {
    let core = eval::build_core_module();
    let std_val = eval_std_module()?;
    let mut modules = HashMap::new();
    modules.insert("core".to_string(), core);
    modules.insert("std".to_string(), std_val);
    Ok(eval::default_env_with_modules(modules))
}

/// Run source code with core and std modules available.
pub fn run_with_std(source: &str) -> Result<Value, String> {
    let (val, _warnings) = run_with_std_and_warnings(source)?;
    Ok(val)
}

/// Run source code with core and std modules, returning value + warnings.
pub fn run_with_std_and_warnings(source: &str) -> Result<(Value, Vec<String>), String> {
    let ast = parse(source)?;
    let env = env_with_std()?;
    let (val, final_env) = eval::eval_toplevel(&ast, &env, &Value::Unit)
        .map_err(|e| format!("runtime error: {}", e))?;
    let warnings = final_env.unused_warnings();
    Ok((val, warnings))
}
