//! Shared test helpers for nana e2e tests.

#![allow(dead_code)]

pub use nana::value::Value;

pub const T: Value = Value::Bool(true);
pub const F: Value = Value::Bool(false);
pub const U: Value = Value::Unit;

pub fn s(v: &str) -> Value {
    Value::Str(v.to_string())
}
pub fn int(v: i64) -> Value {
    Value::Int(v)
}
pub fn float(v: f64) -> Value {
    Value::Float(v)
}
pub fn ch(v: char) -> Value {
    Value::Char(v)
}
pub fn byte(v: u8) -> Value {
    Value::Byte(v)
}
pub fn i32_val(v: i32) -> Value {
    Value::I32(v)
}
pub fn f32_val(v: f32) -> Value {
    Value::F32(v)
}

pub const STD_PRELUDE: &str = "\
let not = std.not; \
let and = std.and; \
let or = std.or; \
let print = std.print; \
let byte = std.byte; \
let int = std.int; \
let float = std.float; \
let char = std.char; \
let ref_eq = std.ref_eq; \
let val_eq = std.val_eq; \
let method_set = std.method_set; \
let i32 = std.i32; \
let f32 = std.f32; \
";

/// Create a REPL environment with std + operator method sets applied.
pub fn repl_env() -> nana::Env {
    let env = nana::env_with_std().unwrap();
    let (_, env) = nana::run_in_env(STD_PRELUDE, &env).unwrap();
    env
}

pub fn assert_val(input: &str, expected: Value) {
    let full = format!("{}{}", STD_PRELUDE, input);
    let result = nana::run_with_std(&full);
    let val = result.unwrap_or_else(|e| panic!("program failed.\n  input: {input}\n  error: {e}"));
    assert_eq!(
        val, expected,
        "\n  input: {input}\n  expected: {expected}\n  got: {val}"
    );
}

pub fn assert_output(input: &str, expected: &str) {
    let full = format!("{}{}", STD_PRELUDE, input);
    let result = nana::run_with_std(&full);
    let val = result.unwrap_or_else(|e| panic!("program failed.\n  input: {input}\n  error: {e}"));
    let output = val.to_string();
    assert_eq!(
        output, expected,
        "\n  input: {input}\n  expected: {expected}\n  got: {output}"
    );
}

pub fn assert_warnings(input: &str, expected_warnings: &[&str]) {
    let full = format!("{}{}", STD_PRELUDE, input);
    let result = nana::run_with_std_and_warnings(&full);
    let (_val, warnings) =
        result.unwrap_or_else(|e| panic!("program failed.\n  input: {input}\n  error: {e}"));
    assert_eq!(
        warnings.len(),
        expected_warnings.len(),
        "\n  input: {input}\n  expected {} warnings but got {}: {:?}",
        expected_warnings.len(),
        warnings.len(),
        warnings
    );
    for (w, expected) in warnings.iter().zip(expected_warnings.iter()) {
        assert!(
            w.contains(expected),
            "\n  input: {input}\n  expected warning containing: {expected}\n  got: {w}"
        );
    }
}

pub fn assert_no_warnings(input: &str) {
    let full = format!("{}{}", STD_PRELUDE, input);
    let result = nana::run_with_std_and_warnings(&full);
    let (_val, warnings) =
        result.unwrap_or_else(|e| panic!("program failed.\n  input: {input}\n  error: {e}"));
    assert!(
        warnings.is_empty(),
        "\n  input: {input}\n  expected no warnings but got: {:?}",
        warnings
    );
}

pub fn assert_error(input: &str, expected_fragment: &str) {
    let full = format!("{}{}", STD_PRELUDE, input);
    let result = nana::run_with_std(&full);
    let err = result.expect_err(&format!(
        "expected error but program succeeded.\n  input: {input}"
    ));
    assert!(
        err.contains(expected_fragment),
        "\n  input: {input}\n  expected error containing: {expected_fragment}\n  got: {err}"
    );
}

pub fn assert_parses(input: &str) {
    nana::parse(input).unwrap_or_else(|e| panic!("parse failed.\n  input: {input}\n  error: {e}"));
}

pub fn assert_parse_error(input: &str, expected_fragment: &str) {
    let result = nana::parse(input);
    let err = result.expect_err(&format!(
        "expected parse error but succeeded.\n  input: {input}"
    ));
    assert!(
        err.contains(expected_fragment),
        "\n  input: {input}\n  expected parse error containing: {expected_fragment}\n  got: {err}"
    );
}

/// Assert with std but without the prelude bindings (for testing raw std access).
pub fn assert_std(input: &str, expected: Value) {
    let result = nana::run_with_std(input);
    let val = result.unwrap_or_else(|e| panic!("program failed.\n  input: {input}\n  error: {e}"));
    assert_eq!(
        val, expected,
        "\n  input: {input}\n  expected: {expected}\n  got: {val}"
    );
}
