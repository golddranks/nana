//! Tests demonstrating bugs and spec deviations found during code review.
//!
//! Each test documents the expected (correct) behavior per DESIGN.md.
//! Tests that currently fail due to known bugs are marked #[ignore]
//! with a comment referencing the bug. Remove #[ignore] once fixed.

use nana::value::Value;

const T: Value = Value::Bool(true);
const F: Value = Value::Bool(false);
const U: Value = Value::Unit;
fn s(v: &str) -> Value {
    Value::Str(v.to_string())
}
fn int(v: i64) -> Value {
    Value::Int(v)
}
fn float(v: f64) -> Value {
    Value::Float(v)
}
fn ch(v: char) -> Value {
    Value::Char(v)
}
fn byte(v: u8) -> Value {
    Value::Byte(v)
}

const STD_PRELUDE: &str = "\
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
";

/// Create a REPL environment with std + operator method sets applied.
fn repl_env() -> nana::Env {
    let env = nana::env_with_std().unwrap();
    let (_, env) = nana::run_in_env(STD_PRELUDE, &env).unwrap();
    env
}

fn assert_val(input: &str, expected: Value) {
    let full = format!("{}{}", STD_PRELUDE, input);
    let result = nana::run_with_std(&full);
    let val = result.unwrap_or_else(|e| panic!("program failed.\n  input: {input}\n  error: {e}"));
    assert_eq!(
        val, expected,
        "\n  input: {input}\n  expected: {expected}\n  got: {val}"
    );
}

fn assert_output(input: &str, expected: &str) {
    let full = format!("{}{}", STD_PRELUDE, input);
    let result = nana::run_with_std(&full);
    let val = result.unwrap_or_else(|e| panic!("program failed.\n  input: {input}\n  error: {e}"));
    let output = val.to_string();
    assert_eq!(
        output, expected,
        "\n  input: {input}\n  expected: {expected}\n  got: {output}"
    );
}

fn assert_warnings(input: &str, expected_warnings: &[&str]) {
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

fn assert_no_warnings(input: &str) {
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

fn assert_error(input: &str, expected_fragment: &str) {
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

fn assert_parses(input: &str) {
    nana::parse(input).unwrap_or_else(|e| panic!("parse failed.\n  input: {input}\n  error: {e}"));
}

fn assert_parse_error(input: &str, expected_fragment: &str) {
    let result = nana::parse(input);
    let err = result.expect_err(&format!(
        "expected parse error but succeeded.\n  input: {input}"
    ));
    assert!(
        err.contains(expected_fragment),
        "\n  input: {input}\n  expected parse error containing: {expected_fragment}\n  got: {err}"
    );
}

// ═══════════════════════════════════════════════════════════════════
// Passing baseline tests — the things that already work
// ═══════════════════════════════════════════════════════════════════

#[test]
fn baseline_complete_example() {
    assert_val(
        r#"tag(Ok);
tag(Err);

let safe_div = { in >> let(a, b);
  b == 0 >> {
    true -> Err("division by zero"),
    false -> a * 100 / b >> Ok
  }
};

(10, 3) >> safe_div >> {
  Ok(result) -> result,
  Err(_) -> 0
}"#,
        int(333),
    );
}

#[test]
fn baseline_pipe_into_block() {
    assert_val("3 >> { in + 1 }", int(4));
}

#[test]
fn baseline_block_sugar_plus() {
    assert_val("3 >> { + 1 }", int(4));
}

#[test]
fn baseline_block_sugar_mul() {
    assert_val("3 >> { * 2 }", int(6));
}

#[test]
fn baseline_block_sugar_div() {
    assert_val("10 >> { / 2 }", int(5));
}

#[test]
fn baseline_bind_lambda_apply() {
    assert_val("{ in * 2 } >> let(f); 3 >> f", int(6));
}

#[test]
fn baseline_let_binding() {
    assert_val("1 >> let(x); x + 2", int(3));
}

#[test]
fn baseline_array_literal() {
    assert_output("[1, 2, 3]", "[1, 2, 3]");
}

#[test]
fn baseline_struct_field_access() {
    assert_val("(a=1, b=2).a", int(1));
}

#[test]
fn baseline_nested_blocks() {
    assert_val("3 >> { in >> let(outer); 4 >> { outer + in } }", int(7));
}

#[test]
fn baseline_pipe_chain() {
    assert_val("1 >> { in + 1 } >> { in * 3 }", int(6));
}

#[test]
fn baseline_let_passthrough() {
    assert_val("5 >> let(x) >> { in + 1 }", int(6));
}

#[test]
fn baseline_tag_and_branch() {
    assert_val("tag(Foo); 42 >> Foo >> { Foo(x) -> x + 1 }", int(43));
}

#[test]
fn baseline_boolean_builtins() {
    assert_val("and(true, false)", F);
    assert_val("or(true, false)", T);
    assert_val("not(true)", F);
}

#[test]
fn baseline_array_builtins() {
    assert_val("[1, 2, 3].len()", int(3));
    assert_output("[1, 2, 3].map{ * 2 }", "[2, 4, 6]");
    assert_output("[1, 2, 3, 4].filter{ > 2 }", "[3, 4]");
    assert_val("[1, 2, 3].fold(0, { in.acc + in.elem })", int(6));
}

#[test]
fn baseline_string_concat() {
    assert_val(r#""hello" + " " + "world""#, s("hello world"));
}

#[test]
fn baseline_array_concat() {
    assert_output("[1, 2] + [3, 4]", "[1, 2, 3, 4]");
}

#[test]
fn baseline_comparison_operators() {
    assert_val("1 == 1", T);
    assert_val("1 != 2", T);
    assert_val("1 < 2", T);
    assert_val("2 > 1", T);
    assert_val("1 <= 1", T);
    assert_val("1 >= 1", T);
}

#[test]
fn baseline_unit() {
    assert_val("()", U);
}

#[test]
fn baseline_tuple() {
    assert_output("(1, 2, 3)", "(1, 2, 3)");
}

#[test]
fn baseline_branching_block_bool() {
    assert_val("true >> { true -> 1, false -> 2 }", int(1));
    assert_val("false >> { true -> 1, false -> 2 }", int(2));
}

#[test]
fn baseline_division_by_zero() {
    assert_error("1 / 0", "division by zero");
}

#[test]
fn baseline_struct_destructuring() {
    assert_val("(1, 2) >> let(a, b); a + b", int(3));
}

#[test]
fn baseline_array_destructuring() {
    assert_val("[1, 2, 3] >> let[a, b, c]; a + b + c", int(6));
}

#[test]
fn baseline_array_rest_pattern() {
    assert_val("[1, 2, 3, 4] >> let[first, ...rest]; first", int(1));
    assert_output("[1, 2, 3, 4] >> let[first, ...rest]; rest", "[2, 3, 4]");
}

#[test]
fn baseline_branch_with_guard() {
    assert_val(
        "tag(N); 5 >> N >> { N(x) if x > 3 -> x, N(x) -> 0 }",
        int(5),
    );
    assert_val(
        "tag(N); 1 >> N >> { N(x) if x > 3 -> x, N(x) -> 0 }",
        int(0),
    );
}

#[test]
fn baseline_unary_minus() {
    assert_val("-5", int(-5));
    assert_val("3 + -2", int(1));
}

#[test]
fn baseline_float_arithmetic() {
    assert_val("1.5 + 2.5", float(4.0));
    assert_val("3.0 * 2.0", float(6.0));
}

#[test]
fn baseline_char_and_byte_literals() {
    assert_val("'a'", ch('a'));
}

#[test]
fn baseline_string_escape_sequences() {
    assert_val(r#""\n".char_len()"#, int(1));
    assert_val(r#""\t".char_len()"#, int(1));
    assert_val(r#""\\".char_len()"#, int(1));
    assert_val(r#""hello\nworld".char_len()"#, int(11));
}

#[test]
fn baseline_hex_integer() {
    assert_val("0xFF", int(255));
}

#[test]
fn baseline_array_get_method() {
    assert_val("[10, 20, 30].get(1)", int(20));
}

#[test]
fn baseline_range_sugar() {
    assert_val("(1..3).start", int(1));
    assert_val("(1..3).end", int(3));
}

#[test]
fn baseline_spread_in_struct() {
    assert_val("(a=1, b=2) >> let(s); (a=99, ...s).a", int(99));
    assert_val("(a=1, b=2) >> let(s); (a=99, ...s).b", int(2));
}

#[test]
fn baseline_block_sugar_pipe() {
    assert_val("{ in * 2 } >> let(f); 3 >> { >> f }", int(6));
}

#[test]
fn baseline_semicolon_sequencing() {
    assert_val("1; 2; 3", int(3));
}

// ═══════════════════════════════════════════════════════════════════
// BUG-1: Block sugar { - expr } parses as unary negation, not sub
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug1_block_sugar_minus() {
    // `-` at the start of a block is ambiguous (subtraction sugar vs unary negation).
    // It is a parse error. Users must write `{ in - 3 }` or `{ (-3) }`.
    assert_parse_error("10 >> { - 3 }", "ambiguous");
    assert_val("10 >> { in - 3 }", int(7));
    assert_val("10 >> { (-3) }", int(-3));
}

#[test]
fn bug1_block_sugar_minus_complex() {
    assert_parse_error("100 >> { - 30 - 20 }", "ambiguous");
    assert_val("100 >> { in - 30 - 20 }", int(50));
}

// ═══════════════════════════════════════════════════════════════════
// BUG-2: parse_let_body only consumes one pipe step
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug2_multi_pipe_let_chain() {
    assert_val("5 >> let(x) >> { in + 1 } >> let(y); x + y", int(11));
}

#[test]
fn bug2_triple_let_chain() {
    assert_val(
        "1 >> let(a) >> { in + 1 } >> let(b) >> { in + 1 } >> let(c); a + b + c",
        int(6),
    );
}

#[test]
fn bug2_let_chain_with_tag() {
    assert_val(
        "tag(W); 5 >> let(x) >> W >> let(wrapped); wrapped >> { W(v) -> v + x }",
        int(10),
    );
}

// ═══════════════════════════════════════════════════════════════════
// BUG-3: parse_expr_with_lhs missing LBracket and DotDot
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug3_block_sugar_with_range() {
    // Range produces a struct (start=1, end=10). Scalar + struct is not a valid operation.
    assert_error("5 >> { + 1..10 }", "type error");
}

// ═══════════════════════════════════════════════════════════════════
// BUG-4: Integer overflow panics instead of returning error
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug4_add_overflow() {
    assert_error("9223372036854775807 + 1", "overflow");
}

#[test]
fn bug4_mul_overflow() {
    assert_error("9223372036854775807 * 2", "overflow");
}

#[test]
fn bug4_sub_overflow() {
    assert_error("0 - 9223372036854775807 - 2", "overflow");
}

#[test]
fn bug4_unary_minus_overflow() {
    assert_error("0 - 9223372036854775807 - 1 >> { in * -1 }", "overflow");
}

// ═══════════════════════════════════════════════════════════════════
// BUG-5: Negative array index gives confusing error
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug5_negative_index_error_message() {
    assert_error("[1, 2, 3].get(-1)", "negative");
}

// ═══════════════════════════════════════════════════════════════════
// SPEC-1: a / b * c should be a syntax error
// ═══════════════════════════════════════════════════════════════════

#[test]
fn spec1_div_then_mul_is_syntax_error() {
    assert_parse_error("10 / 2 * 3", "");
}

#[test]
fn spec1_mul_then_div_is_valid() {
    assert_val("12 * 2 / 3", int(8));
}

// ═══════════════════════════════════════════════════════════════════
// SPEC-2: Chained comparisons should be syntax error (non-associative)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn spec2_chained_eq_is_syntax_error() {
    assert_parse_error("1 == 1 == true", "");
}

#[test]
fn spec2_chained_lt_is_syntax_error() {
    assert_parse_error("1 < 2 < 3", "");
}

// ═══════════════════════════════════════════════════════════════════
// SPEC-3: {} should be a callable block, not bare Unit
// ═══════════════════════════════════════════════════════════════════

#[test]
fn spec3_empty_block_is_callable() {
    assert_val("5 >> {}", U);
}

#[test]
fn spec3_empty_block_as_lambda() {
    assert_val("{} >> let(f); 5 >> f", U);
}

// ═══════════════════════════════════════════════════════════════════
// SPEC-4: `in` inside a branching block body is the branching block's input.
// To use the outer block's `in`, rebind it with `let`.
// ═══════════════════════════════════════════════════════════════════

#[test]
fn spec4_branch_in_is_scrutinee() {
    // `in` inside a branching arm body refers to the branching block's input
    assert_val("42 >> { x -> in }", int(42));
}

#[test]
fn spec4_outer_in_via_rebind() {
    // To use the outer block's `in`, rebind it before the branch
    assert_val(
        "tag(Ok); 10 >> { in >> let(outer); outer >> Ok >> { Ok(x) -> outer + x } }",
        int(20),
    );
}

// ═══════════════════════════════════════════════════════════════════
// Edge cases and error handling
// ═══════════════════════════════════════════════════════════════════

#[test]
fn minor_empty_array() {
    assert_output("[]", "[]");
}

#[test]
fn minor_trailing_comma_in_array() {
    assert_output("[1, 2, 3,]", "[1, 2, 3]");
}

#[test]
fn minor_trailing_comma_in_struct() {
    assert_val("(a=1, b=2,).a", int(1));
}

#[test]
fn minor_deeply_nested_pipes() {
    assert_val(
        "1 >> { in + 1 } >> { in + 1 } >> { in + 1 } >> { in + 1 } >> { in + 1 }",
        int(6),
    );
}

#[test]
fn minor_let_discard() {
    assert_val("5 >> let(_); 42", int(42));
}

#[test]
fn minor_branch_discard_binding() {
    assert_val("tag(X); 99 >> X >> { X(_) -> 0 }", int(0));
}

#[test]
fn minor_non_exhaustive_branch() {
    assert_error("tag(A); tag(B); 1 >> A >> { B(x) -> x }", "no arm matched");
}

#[test]
fn minor_undefined_variable() {
    assert_error("x + 1", "undefined variable");
}

#[test]
fn minor_type_error_in_binop() {
    assert_error(r#"1 + "hello""#, "type error");
}

#[test]
fn minor_call_non_function() {
    assert_error("42(1)", "cannot call non-function");
}

#[test]
fn minor_field_not_found() {
    assert_error("(a=1).b", "field 'b' not found");
}

// ═══════════════════════════════════════════════════════════════════
// Parse-only tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn parse_empty_block() {
    assert_parses("{}");
}

#[test]
fn parse_nested_blocks() {
    assert_parses("{ { { in } } }");
}

#[test]
fn parse_complex_destructuring() {
    assert_parses("(1, 2, 3) >> let(a, b, c); a");
}

// ═══════════════════════════════════════════════════════════════════
// ROUND 2 — additional bugs and spec deviations
// ═══════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════
// BUG-7: Integer division overflow panics (i64::MIN / -1)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug7_div_overflow() {
    // i64::MIN / -1 overflows because |i64::MIN| > i64::MAX
    assert_error("0 - 9223372036854775807 - 1 >> { in / -1 }", "overflow");
}

// ═══════════════════════════════════════════════════════════════════
// BUG-8: DotDot not in is_binary_op_token, so { ..x } fails
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug8_block_sugar_range() {
    // { ..10 } should be sugar for { in..10 }
    assert_output("5 >> { ..10 }", "(start=5, end=10)");
}

// ═══════════════════════════════════════════════════════════════════
// SPEC-6: Array comparison — spec says comparisons work on arrays
// ═══════════════════════════════════════════════════════════════════

#[test]
fn spec6_array_eq() {
    assert_val("[1, 2, 3] == [1, 2, 3]", T);
}

#[test]
fn spec6_array_neq() {
    assert_val("[1, 2] != [1, 3]", T);
}

// ═══════════════════════════════════════════════════════════════════
// SPEC-7: Struct comparison — spec says comparisons work on structs
// ═══════════════════════════════════════════════════════════════════

#[test]
fn spec7_struct_eq() {
    assert_val("(a=1, b=2) == (a=1, b=2)", T);
}

#[test]
fn spec7_struct_neq() {
    assert_val("(a=1, b=2) != (a=1, b=3)", T);
}

// ═══════════════════════════════════════════════════════════════════
// SPEC-8: Float division by zero should halt, not return inf
// ═══════════════════════════════════════════════════════════════════

#[test]
fn spec8_float_div_by_zero() {
    // DESIGN.md: "Division by zero...halt execution."
    assert_error("1.0 / 0.0", "division by zero");
}

// ═══════════════════════════════════════════════════════════════════
// Round 2 — passing edge case tests (already work)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn r2_closure_captures_env() {
    assert_val("1 >> let(x); { in + x } >> let(f); 10 >> f", int(11));
}

#[test]
fn r2_shadowing() {
    assert_val("1 >> let(x); 2 >> let(x); x", int(2));
}

#[test]
fn r2_block_sugar_comparison() {
    assert_val("5 >> { == 5 }", T);
    assert_val("5 >> { != 5 }", F);
    assert_val("5 >> { > 3 }", T);
    assert_val("5 >> { < 3 }", F);
}

#[test]
fn r2_nested_field_access() {
    assert_val("(a=(b=42)).a.b", int(42));
}

#[test]
fn r2_positional_field_access() {
    assert_val("(10, 20, 30).1", int(20));
}

#[test]
fn r2_struct_as_module() {
    assert_val(
        "(add = { in >> let(a, b); a + b }) >> let(math); (3, 2) >> math.add",
        int(5),
    );
}

#[test]
fn r2_direct_tag_call() {
    assert_output("tag(W); W(42)", "W(42)");
}

#[test]
fn r2_no_payload_tag() {
    assert_output("tag(Done); () >> Done", "Done");
}

#[test]
fn r2_labeled_struct_destructuring() {
    assert_val("(a=1, b=2) >> let(a=x, b=y); x + y", int(3));
}

#[test]
fn r2_struct_rest_pattern() {
    assert_output("(a=1, b=2, c=3) >> let(a=x, ...rest); rest", "(b=2, c=3)");
}

#[test]
fn r2_pipe_prepend_args() {
    assert_output("[1, 2, 3].map({ in * 10 })", "[10, 20, 30]");
}

#[test]
fn r2_comments() {
    assert_val("1 + 2 # comment", int(3));
}

#[test]
fn r2_out_of_bounds_get() {
    assert_error("[1, 2, 3].get(5)", "out of bounds");
}

#[test]
fn r2_paren_scope_limits_let() {
    // DESIGN.md: "parentheses limit let scope; evaluates to 4"
    assert_val("1 >> let(x); (2 >> let(y); y + 1) + x", int(4));
}

// ═══════════════════════════════════════════════════════════════════
// REPL environment persistence (run_in_env)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn repl_let_persists() {
    let env = repl_env();
    let (val, env) = nana::run_in_env("4 >> let(a)", &env).unwrap();
    assert_eq!(val.to_string(), "4");
    let (val, _) = nana::run_in_env("a + 10", &env).unwrap();
    assert_eq!(val.to_string(), "14");
}

#[test]
fn repl_tag_persists() {
    let env = repl_env();
    let (_, env) = nana::run_in_env("tag(Ok)", &env).unwrap();
    let (val, _) = nana::run_in_env("42 >> Ok", &env).unwrap();
    assert_eq!(val.to_string(), "Ok(42)");
}

#[test]
fn repl_function_persists() {
    let env = repl_env();
    let (_, env) = nana::run_in_env("{ in * 2 } >> let(double)", &env).unwrap();
    let (val, _) = nana::run_in_env("5 >> double", &env).unwrap();
    assert_eq!(val.to_string(), "10");
}

#[test]
fn repl_multiple_bindings() {
    let env = repl_env();
    let (_, env) = nana::run_in_env("1 >> let(a)", &env).unwrap();
    let (_, env) = nana::run_in_env("2 >> let(b)", &env).unwrap();
    let (val, _) = nana::run_in_env("a + b", &env).unwrap();
    assert_eq!(val.to_string(), "3");
}

#[test]
fn repl_error_preserves_env() {
    let env = repl_env();
    let (_, env) = nana::run_in_env("42 >> let(x)", &env).unwrap();
    // An error on the next line should not lose x
    let _ = nana::run_in_env("1 / 0", &env);
    let (val, _) = nana::run_in_env("x", &env).unwrap();
    assert_eq!(val.to_string(), "42");
}

// ═══════════════════════════════════════════════════════════════════
// ROUND 3 — bugs found during code review
// ═══════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════
// BUG-10: Byte Display is wrong for non-printable bytes
// value.rs:78 — `*b as char` prints garbage for \0, control chars, etc.
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug10_byte_display_null() {
    // b'\0' should display as b'\0' or b'\x00', not embed a literal NUL char
    let result = nana::run(r"b'\0'").unwrap();
    let s = result.to_string();
    assert!(
        !s.contains('\0'),
        "b'\\0' display embeds a literal NUL byte: {:?}",
        s
    );
}

#[test]
fn bug10_byte_display_newline() {
    // b'\n' should display as b'\n', not embed a literal newline char
    let result = nana::run(r"b'\n'").unwrap();
    let s = result.to_string();
    assert!(
        !s.contains('\n'),
        "b'\\n' display embeds a literal newline: {:?}",
        s
    );
}

// ═══════════════════════════════════════════════════════════════════
// BUG-13: ArrayPat::Discard doesn't check array bounds
// eval.rs:409-411 — increments pos without checking pos < len
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug13_array_discard_out_of_bounds() {
    // [1] has 1 element, but let[a, _] expects 2; should error
    assert_error("[1] >> let[a, _]; a", "not enough elements");
}

#[test]
fn bug13_array_discard_way_out_of_bounds() {
    // [] has 0 elements, but let[_, _] expects 2; should error
    assert_error("[] >> let[_, _]; ()", "not enough elements");
}

// ═══════════════════════════════════════════════════════════════════
// BUG-14: identity_expr_for_pattern uses label instead of binding
// parser.rs:354 — returns Ident(label) instead of Ident(binding)
// for single-field labeled destructuring
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug14_labeled_destructure_pipe_through() {
    // let(a=x, ...) binds field 'a' to variable 'x'; passthrough is the original struct
    assert_val("(a=1, b=2) >> let(a=x, ...) >> { in.a + 10 }", int(11));
}

#[test]
fn bug14_labeled_destructure_pipe_through_different_name() {
    // Passthrough is the original struct
    assert_val(
        "(name=42, other=0) >> let(name=val, ...) >> { in.name * 2 }",
        int(84),
    );
}

// ═══════════════════════════════════════════════════════════════════
// BUG-19: Array rest pattern panics when array is too short
// eval.rs:428 — elems[pos..rest_end] panics when pos > rest_end
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug19_rest_pattern_array_too_short() {
    // [1] has 1 element, but let[first, ...mid, last] needs at least 2
    assert_error("[1] >> let[first, ...mid, last]; last", "not enough");
}

#[test]
fn bug19_rest_pattern_empty_array() {
    // [] with let[a, ...rest, b] needs at least 2
    assert_error("[] >> let[a, ...rest, b]; a", "not enough");
}

// ═══════════════════════════════════════════════════════════════════
// BUG-20: Char Display embeds raw control characters
// value.rs:77 — same issue as BUG-10 but for char values
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug20_char_display_null() {
    let result = nana::run(r"'\0'").unwrap();
    let s = result.to_string();
    assert!(
        !s.contains('\0'),
        "char '\\0' display embeds a literal NUL: {:?}",
        s
    );
}

#[test]
fn bug20_char_display_newline() {
    let result = nana::run(r"'\n'").unwrap();
    let s = result.to_string();
    assert!(
        !s.contains('\n'),
        "char '\\n' display embeds a literal newline: {:?}",
        s
    );
}

// ═══════════════════════════════════════════════════════════════════
// BUG-21: Struct spread doesn't filter positional overrides
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug21_spread_positional_reindex() {
    assert_output("(10, 20) >> let(s); (99, ...s)", "(99, 10, 20)");
}

#[test]
fn bug21_spread_positional_field_access() {
    assert_val("(1, 2, 3) >> let(s); (99, ...s).0", int(99));
    assert_val("(1, 2, 3) >> let(s); (99, ...s).1", int(1));
    assert_val("(1, 2, 3) >> let(s); (99, ...s).2", int(2));
    assert_val("(1, 2, 3) >> let(s); (99, ...s).3", int(3));
}

#[test]
fn bug21_spread_positional_equality() {
    assert_val("(10, 20) >> let(s); (99, ...s) == (99, 10, 20)", T);
}

// ═══════════════════════════════════════════════════════════════════
// BUG-22: Multiple rest patterns in destructuring silently accepted
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug22_multiple_rest_array() {
    assert_error("[1, 2, 3] >> let[...a, ...b]; a", "");
}

#[test]
fn bug22_multiple_rest_struct() {
    assert_error("(a=1, b=2) >> let(...x, ...y); x", "");
}

// ═══════════════════════════════════════════════════════════════════
// BUG-23: Destructuring silently ignores extra elements/fields
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug23_array_extra_elements() {
    assert_error("[1, 2, 3] >> let[a, b]; a", "");
}

#[test]
fn bug23_array_extra_elements_with_rest_ok() {
    assert_val("[1, 2, 3] >> let[a, b, ...]; a + b", int(3));
}

#[test]
fn bug23_array_extra_elements_with_discard_ok() {
    assert_val("[1, 2, 3] >> let[a, b, _]; a + b", int(3));
}

#[test]
fn bug23_array_exact_match_ok() {
    assert_val("[1, 2, 3] >> let[a, b, c]; a + b + c", int(6));
}

#[test]
fn bug23_struct_extra_fields() {
    assert_error("(a=1, b=2, c=3) >> let(a=x, b=y); x", "");
}

#[test]
fn bug23_struct_extra_fields_with_rest_ok() {
    assert_val("(a=1, b=2, c=3) >> let(a=x, ...rest); x", int(1));
}

#[test]
fn bug23_struct_extra_fields_with_discard_rest_ok() {
    assert_val("(a=1, b=2, c=3) >> let(a=x, ...); x", int(1));
}

#[test]
fn bug23_struct_exact_match_ok() {
    assert_val("(a=1, b=2) >> let(a=x, b=y); x + y", int(3));
}

#[test]
fn bug23_positional_struct_extra() {
    assert_error("(1, 2, 3) >> let(a, b); a", "");
}

// ═══════════════════════════════════════════════════════════════════
// BUG-24: Scalar-over-struct distribution should not exist
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug24_scalar_plus_struct_not_allowed() {
    assert_error("10 + (1, 2, 3)", "type error");
}

#[test]
fn bug24_scalar_mul_struct_not_allowed() {
    assert_error("10 * (1, 2, 3)", "type error");
}

#[test]
fn bug24_struct_plus_scalar_not_allowed() {
    assert_error("(1, 2, 3) + 10", "no method 'add'");
}

#[test]
fn bug24_scalar_sub_struct_not_allowed() {
    assert_error("10 - (1, 2)", "type error");
}

#[test]
fn bug24_struct_struct_add_not_allowed() {
    assert_error("(1, 2) + (3, 4)", "no method 'add'");
}

// ═══════════════════════════════════════════════════════════════════
// BUG-26: Named struct equality should be order-insensitive
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug26_named_struct_eq_order_insensitive() {
    assert_val("(a=1, b=2) == (b=2, a=1)", T);
}

#[test]
fn bug26_named_struct_neq_order_insensitive() {
    assert_val("(a=1, b=2) != (b=2, a=1)", F);
}

#[test]
fn bug26_named_struct_different_values() {
    assert_val("(a=1, b=2) == (a=1, b=3)", F);
}

#[test]
fn bug26_named_struct_different_fields() {
    assert_val("(a=1, b=2) == (a=1, c=2)", F);
}

#[test]
fn bug26_positional_struct_still_order_sensitive() {
    assert_val("(1, 2) == (1, 2)", T);
    assert_val("(1, 2) == (2, 1)", F);
}

#[test]
fn bug26_mixed_positional_named_order_sensitive() {
    assert_val("(1, a=2) == (1, a=2)", T);
}

// ═══════════════════════════════════════════════════════════════════
// BUG-27: Rest pattern should re-index positional fields
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug27_rest_positional_reindex() {
    assert_val("(10, 20, 30) >> let(x, ...rest); rest.0", int(20));
}

#[test]
fn bug27_rest_positional_reindex_second() {
    assert_val("(10, 20, 30) >> let(x, ...rest); rest.1", int(30));
}

#[test]
fn bug27_rest_positional_reindex_equality() {
    assert_val("(10, 20, 30) >> let(x, ...rest); rest == (20, 30)", T);
}

#[test]
fn bug27_rest_named_fields_unchanged() {
    assert_val("(a=1, b=2, c=3) >> let(a=x, ...rest); rest.b", int(2));
}

// ═══════════════════════════════════════════════════════════════════
// BUG-28: Duplicate named labels in struct construction
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug28_duplicate_named_label_rejected() {
    assert_error("(a=1, a=2)", "duplicate field");
}

#[test]
fn bug28_no_false_positive_positional() {
    assert_output("(1, 2, 3)", "(1, 2, 3)");
}

#[test]
fn bug28_no_false_positive_named() {
    assert_output("(a=1, b=2)", "(a=1, b=2)");
}

#[test]
fn bug28_duplicate_via_call_args() {
    assert_error("{ in } >> let(f); f(a=1, a=2)", "duplicate field");
}

// ═══════════════════════════════════════════════════════════════════
// BUG-29: Duplicate named fields via multiple spreads
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug29_duplicate_named_via_two_spreads() {
    assert_error(
        "(a=1) >> let(s1); (a=2) >> let(s2); (...s1, ...s2)",
        "duplicate field",
    );
}

#[test]
fn bug29_spread_no_conflict_different_names() {
    assert_output(
        "(a=1) >> let(s1); (b=2) >> let(s2); (...s1, ...s2)",
        "(a=1, b=2)",
    );
}

#[test]
fn bug29_explicit_overrides_spread_still_ok() {
    assert_output("(a=1, b=2) >> let(s); (a=99, ...s)", "(a=99, b=2)");
}

#[test]
fn bug29_positional_spreads_reindex_ok() {
    assert_output(
        "(1, 2) >> let(s1); (3, 4) >> let(s2); (...s1, ...s2)",
        "(1, 2, 3, 4)",
    );
}

// ═══════════════════════════════════════════════════════════════════
// BUG-30: a / b / c should be rejected (like a / b * c)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug30_div_div_is_syntax_error() {
    assert_error("12 / 3 / 2", "ambiguous precedence");
}

#[test]
fn bug30_div_mul_still_error() {
    assert_error("6 / 2 * 3", "ambiguous precedence");
}

#[test]
fn bug30_mul_div_still_valid() {
    assert_val("6 * 2 / 3", int(4));
}

#[test]
fn bug30_parenthesized_div_div_ok() {
    assert_val("(12 / 3) / 2", int(2));
}

// ═══════════════════════════════════════════════════════════════════
// BUG-31: Positional destructuring falls back on named structs
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug31_positional_destruct_on_named_struct_errors() {
    assert_error("(a=1, b=2) >> let(x, y)", "");
}

#[test]
fn bug31_positional_destruct_on_positional_struct_ok() {
    assert_val("(10, 20) >> let(x, y); x + y", int(30));
}

#[test]
fn bug31_named_destruct_on_named_struct_ok() {
    assert_val("(a=1, b=2) >> let(a=x, b=y); x + y", int(3));
}

// ═══════════════════════════════════════════════════════════════════
// New syntax: let name = expr sugar
// ═══════════════════════════════════════════════════════════════════

#[test]
fn let_sugar_basic() {
    assert_val("let x = 1; x + 2", int(3));
}

#[test]
fn let_sugar_complex() {
    assert_val("let x = 1 + 2; x * 3", int(9));
}

#[test]
fn let_sugar_chained() {
    assert_val("let x = 1; let y = 2; x + y", int(3));
}

// ═══════════════════════════════════════════════════════════════════
// New syntax: branching blocks
// ═══════════════════════════════════════════════════════════════════

#[test]
fn branch_bool_true() {
    assert_val("true >> { true -> 1, false -> 2 }", int(1));
}

#[test]
fn branch_bool_false() {
    assert_val("false >> { true -> 1, false -> 2 }", int(2));
}

#[test]
fn branch_literal_int() {
    assert_val("1 >> { 0 -> 10, 1 -> 20, _ -> 30 }", int(20));
}

#[test]
fn branch_literal_string() {
    assert_val(r#""hello" >> { "hello" -> 1, _ -> 0 }"#, int(1));
}

#[test]
fn branch_discard() {
    assert_val("42 >> { _ -> 99 }", int(99));
}

#[test]
fn branch_binding() {
    assert_val("42 >> { x -> x + 1 }", int(43));
}

#[test]
fn branch_tag_pattern() {
    assert_val(
        "tag(Ok); tag(Err); 42 >> Ok >> { Ok(x) -> x, Err(_) -> 0 }",
        int(42),
    );
}

#[test]
fn branch_tag_no_payload() {
    assert_val(
        "tag(Done); tag(NotDone); () >> Done >> { Done -> 1, NotDone -> 0 }",
        int(1),
    );
}

#[test]
fn branch_as_function() {
    // Branching block is a function — can be stored and called
    assert_val("{ true -> 1, false -> 0 } >> let(f); true >> f", int(1));
}

#[test]
fn branch_in_available() {
    // `in` is available in branch arm bodies
    assert_val("42 >> { x -> in }", int(42));
}

// ═══════════════════════════════════════════════════════════════════
// New syntax: method calls
// ═══════════════════════════════════════════════════════════════════

#[test]
fn method_get() {
    assert_val("[10, 20, 30].get(0)", int(10));
    assert_val("[10, 20, 30].get(2)", int(30));
}

#[test]
fn method_slice() {
    assert_output("[10, 20, 30, 40].slice(1..3)", "[20, 30]");
}

#[test]
fn method_len() {
    assert_val("[1, 2, 3].len()", int(3));
}

#[test]
fn method_map() {
    assert_output("[1, 2, 3].map{ * 2 }", "[2, 4, 6]");
}

#[test]
fn method_filter() {
    assert_output("[1, 2, 3, 4].filter{ > 2 }", "[3, 4]");
}

#[test]
fn method_fold() {
    assert_val("[1, 2, 3].fold(0, { in.acc + in.elem })", int(6));
}

#[test]
fn method_zip() {
    assert_output("[1, 2].zip([3, 4])", "[(1, 3), (2, 4)]");
}

#[test]
fn method_string_len() {
    assert_val(r#""hello".char_len()"#, int(5));
}

#[test]
fn method_chained() {
    assert_output("[1, 2, 3, 4].filter{ > 2 }.map{ * 10 }", "[30, 40]");
}

// ═══════════════════════════════════════════════════════════════════
// New syntax: f{block} and f[array] call syntax
// ═══════════════════════════════════════════════════════════════════

#[test]
fn call_with_block() {
    // f{block} calls f with the block as argument
    assert_val("{ in } >> let(f); f{ in + 1 } >> let(g); 5 >> g", int(6));
}

#[test]
fn call_with_array() {
    // f[array] calls f with the array as argument
    assert_output("{ in } >> let(identity); identity[1, 2, 3]", "[1, 2, 3]");
}

// ═══════════════════════════════════════════════════════════════════
// New syntax: unary minus restriction
// ═══════════════════════════════════════════════════════════════════

#[test]
fn unary_minus_restriction() {
    // -a.f() is a syntax error per spec
    assert_parse_error("-a.f()", "ambiguous");
}

// ═══════════════════════════════════════════════════════════════════
// New syntax: dual let(a, b) binding (name or positional)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn dual_let_by_name() {
    // (x=1, y=2) >> let(x, y) — names match struct fields, bind by name
    assert_val("(x=1, y=2) >> let(x, y); x + y", int(3));
}

#[test]
fn dual_let_by_position() {
    // (1, 2) >> let(a, b) — no name match, bind positionally
    assert_val("(1, 2) >> let(a, b); a + b", int(3));
}

#[test]
fn dual_let_partial_name_error() {
    // (x=1, y=2) >> let(x, z) — x matches but z doesn't: all-or-nothing error
    assert_error("(x=1, y=2) >> let(x, z)", "");
}

// ═══════════════════════════════════════════════════════════════════
// ROUND 4 — bugs found during code review
// ═══════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════
// BUG-32: Negative literal patterns in branching blocks fail to parse
// is_branch_block_start and parse_branch_pattern don't handle Minus
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug32_negative_int_pattern() {
    // { -1 -> ... } should work as a branch pattern
    assert_val(
        "0 >> { -1 -> \"neg\", 0 -> \"zero\", _ -> \"pos\" }",
        s("zero"),
    );
}

#[test]
fn bug32_negative_int_pattern_matches() {
    assert_val(
        "-1 >> { -1 -> \"neg\", 0 -> \"zero\", _ -> \"pos\" }",
        s("neg"),
    );
}

#[test]
fn bug32_negative_int_pattern_fallthrough() {
    assert_val(
        "5 >> { -1 -> \"neg\", 0 -> \"zero\", _ -> \"pos\" }",
        s("pos"),
    );
}

#[test]
fn bug32_negative_float_pattern() {
    assert_val("-1.5 >> { -1.5 -> \"match\", _ -> \"no\" }", s("match"));
}

// ═══════════════════════════════════════════════════════════════════
// BUG-33: Builtin fold used positional fields (0/1) but DESIGN.md
// shows .acc/.elem — now both use acc/elem consistently
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug33_fold_consistent_named_fields() {
    // fold method uses acc/elem named fields
    assert_val("[1, 2, 3].fold(0, { in.acc + in.elem })", int(6));
}

#[test]
fn bug33_fold_named_destructuring() {
    // Destructuring with matching names should work
    assert_val(
        "[1, 2, 3].fold(0, { in >> let(acc, elem); acc + elem })",
        int(6),
    );
}

// ═══════════════════════════════════════════════════════════════════
// BUG-34: `let x = expr` without trailing `;` returns () instead
// of the bound value. Per design, let returns the bound value.
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug34_let_sugar_no_semicolon() {
    // `let x = 5` at the end of a program should return 5, not ()
    assert_val("let x = 5", int(5));
}

#[test]
fn bug34_let_sugar_no_semicolon_expr() {
    assert_val("let x = 1 + 2", int(3));
}

#[test]
fn bug34_let_sugar_no_semicolon_in_block() {
    // Inside a block, `{ let x = 5 }` should return 5
    assert_val("3 >> { let x = in + 1 }", int(4));
}

// ═══════════════════════════════════════════════════════════════════
// ROUND 5 — bugs found during code review
// ═══════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════
// BUG-35: Range `..` is left-associative instead of non-associative
// `1..2..3` should be a parse error, like chained comparisons.
// Currently it parses as (1..2)..3 producing nested ranges.
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug35_range_chaining_is_error() {
    // Range operator should be non-associative
    assert_error("1..2..3", "");
}

#[test]
fn bug35_single_range_still_works() {
    assert_val("(1..3).start", int(1));
    assert_val("(1..3).end", int(3));
}

#[test]
fn bug35_parenthesized_range_range_ok() {
    // You can explicitly parenthesize if you really want nested ranges
    assert_val("((1..2)..3).start.start", int(1));
}

// ═══════════════════════════════════════════════════════════════════
// BUG-36: Block sugar `{ -expr }` inserts implicit `in` for Minus,
// turning `{ -1 }` into `{ in - 1 }` instead of `{ -1 }`.
// The parser's is_binary_op_token includes Minus, which triggers
// the sugar path. But `-` at the start of a block is unary negation.
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug36_block_minus_is_ambiguous_error() {
    // { -expr } is ambiguous: subtraction sugar or unary negation?
    // The parser rejects it and requires explicit disambiguation.
    assert_parse_error("5 >> { -1 }", "ambiguous");
    assert_parse_error("5 >> { -in }", "ambiguous");
    assert_parse_error("5 >> { -3 }", "ambiguous");
}

#[test]
fn bug36_explicit_negation_in_block() {
    // Use parentheses for unary negation in a block
    assert_val("5 >> { (-1) }", int(-1));
    assert_val("5 >> { (-in) }", int(-5));
}

#[test]
fn bug36_explicit_subtraction_in_block() {
    // Use explicit `in` for subtraction in a block
    assert_val("5 >> { in - 3 }", int(2));
    assert_val("5 >> { in - 1 }", int(4));
}

// ═══════════════════════════════════════════════════════════════════
// BUG-37: `tag(Foo)` returns () instead of the tag constructor.
// `tag(Foo)` desugars to `new_tag >> let(Foo)`, which should return
// the tag constructor value. But parse_tag_sugar creates a placeholder
// Unit body that never gets filled if there's no trailing `;`.
// Same class of bug as BUG-34.
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug37_tag_returns_constructor() {
    // tag(Foo) at the end should return the tag constructor
    assert_output("tag(Foo)", "<tag Foo>");
}

#[test]
fn bug37_tag_with_semicolon_unchanged() {
    // tag(Foo); expr still works — the constructor is bound, body evaluates
    assert_output("tag(Foo); 42 >> Foo", "Foo(42)");
}

#[test]
fn bug37_tag_constructor_is_callable() {
    // The returned tag constructor can be used
    assert_output("tag(Foo) >> let(f); 42 >> f", "Foo(42)");
}

// ═══════════════════════════════════════════════════════════════════
// ROUND 6 — bugs found during code review
// ═══════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════
// BUG-38: Strings inside collections display without quotes
// Value::Display for Str writes raw string. When nested inside
// Array, Struct, or Tagged, output is ambiguous:
// ["hello", "world"] displays as [hello, world].
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug38_string_in_array_display() {
    let result = nana::run(r#"["hello", "world"]"#).unwrap();
    let display = result.to_string();
    // Strings inside arrays should be quoted so output is unambiguous
    assert!(
        display.contains("\"hello\""),
        "string in array should be quoted: got {}",
        display
    );
}

#[test]
fn bug38_string_in_struct_display() {
    let result = nana::run(r#"("hello", 42)"#).unwrap();
    let display = result.to_string();
    assert!(
        display.contains("\"hello\""),
        "string in struct should be quoted: got {}",
        display
    );
}

#[test]
fn bug38_string_in_tagged_display() {
    let result = nana::run(r#"tag(Wrap); "hello" >> Wrap"#).unwrap();
    let display = result.to_string();
    assert!(
        display.contains("\"hello\""),
        "string in tagged value should be quoted: got {}",
        display
    );
}

#[test]
fn bug38_standalone_string_no_quotes() {
    // A standalone string value
    assert_val(r#""hello""#, s("hello"));
}

// ═══════════════════════════════════════════════════════════════════
// BUG-39: `let x = tag(Foo); body` — tag(Foo) inner placeholder
// body never gets filled. When tag(Foo) is a sub-expression inside
// `let x = ...`, parse_tag_sugar peeks at `;` (which belongs to the
// outer let sugar) and creates a Unit placeholder. attach_body fills
// the outer let(x) but the inner let(Foo) keeps Unit body, so the
// tag constructor is lost and x binds to ().
// Same issue applies to `let x = use(name); body`.
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug39_let_sugar_with_tag() {
    // `let x = tag(Foo); x(42)` should bind x to the tag constructor
    assert_output("let x = tag(Foo); x(42)", "Foo(42)");
}

#[test]
fn bug39_let_sugar_with_tag_then_branch() {
    // After `let x = tag(Foo)`, both Foo and x should refer to the constructor
    assert_val("let x = tag(Foo); 42 >> x >> { Foo(v) -> v + 1 }", int(43));
}

#[test]
fn bug39_tag_in_block_before_semicolon() {
    // tag(Foo) inside a block where the next token is `}`—not `;`
    // This should still work (already worked before this fix)
    assert_output("() >> { tag(Foo); 1 >> Foo }", "Foo(1)");
}

#[test]
fn bug39_nested_tag_in_let_sugar() {
    // Multiple levels of nesting
    assert_val(
        "let f = tag(Ok); let g = tag(Err); 1 >> f >> { Ok(x) -> x, Err(_) -> 0 }",
        int(1),
    );
}

// ═══════════════════════════════════════════════════════════════════
// ROUND 7 — bugs found during code review
// ═══════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════
// BUG-40: `let x = expr >> let(y); body` — parse_let_body steals `;`
// When the value expression in `let x = VALUE` contains an inner
// `>> let(y)`, parse_let_body greedily consumes the `;` and treats
// `body` as let(y)'s continuation rather than let(x)'s. This makes
// x undefined in the continuation.
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug40_let_sugar_with_inner_let_both_in_scope() {
    // `let x = 42 >> let(y); x + y` — both x and y should be 42
    assert_val("let x = 42 >> let(y); x + y", int(84));
}

#[test]
fn bug40_let_sugar_with_inner_let_x_defined() {
    // x should be defined and equal to y
    assert_val("let x = 42 >> let(y); x", int(42));
}

#[test]
fn bug40_let_sugar_with_inner_let_y_defined() {
    // y should also be defined
    assert_val("let x = 42 >> let(y); y", int(42));
}

#[test]
fn bug40_let_sugar_inner_let_no_semicolon() {
    // Without `;`, everything should work (this always worked)
    assert_val("let x = 42 >> let(y)", int(42));
}

#[test]
fn bug40_let_sugar_inner_let_with_pipe() {
    // `let x = 42 >> let(y) >> { in + 1 }; x` — x should be 43
    assert_val("let x = 42 >> let(y) >> { in + 1 }; x", int(43));
}

#[test]
fn bug40_let_sugar_with_inner_let_array() {
    // `let x = [1,2] >> let[a,b]; a + b` — a, b, and x should be in scope
    assert_val("let x = [1, 2] >> let[a, b]; a + b", int(3));
}

#[test]
fn bug40_let_sugar_with_inner_let_array_x_defined() {
    // x should be the passthrough value from let[a,b]
    assert_val("let x = [10, 20] >> let[a, b]; a", int(10));
}

#[test]
fn bug40_existing_pipe_let_chain_unchanged() {
    // `100 >> let(a); 42 >> let(b); a + b` should still work
    assert_val("100 >> let(a); 42 >> let(b); a + b", int(142));
}

// ═══════════════════════════════════════════════════════════════════
// BUG-41: nest_let_in_let doesn't recurse through Let bodies or
// handle LetArray. When `let x = expr >> let(y) >> let(z)`, the
// nest_let_in_let places let(x) as a sibling Pipe instead of
// nesting inside let(z)'s body. This causes x to be undefined
// or produces "cannot call non-function" errors.
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug41_triple_let_chain_in_sugar() {
    // `let x = 42 >> let(y) >> let(z)` — all three should be 42
    assert_val("let x = 42 >> let(y) >> let(z)", int(42));
}

#[test]
fn bug41_triple_let_chain_all_in_scope() {
    // x, y, z should all be in scope for the continuation
    assert_val("let x = 42 >> let(y) >> let(z); x + y + z", int(126));
}

#[test]
fn bug41_let_sugar_tag_chain() {
    // Chained tag and let bindings inside let sugar
    assert_val(
        "let ok = tag(Ok); let err = tag(Err); 42 >> ok >> { Ok(x) -> x }",
        int(42),
    );
}

// ═══════════════════════════════════════════════════════════════════
// ROUND 8 — bugs found during code review
// ═══════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════
// BUG-42: identity_expr_for_pattern includes discard (`_`) bindings.
// When `let[_, b, c]` has no explicit body (passthrough), the
// identity expression tries to reference `_` as a variable, which
// causes "undefined variable: _" at runtime.
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug42_let_array_discard_first() {
    assert_val("[1, 2, 3] >> let[_, b, c]; b + c", int(5));
}

#[test]
fn bug42_let_array_discard_first_passthrough() {
    // Passthrough is the original piped value
    assert_output("[1, 2, 3] >> let[_, b, c]", "[1, 2, 3]");
}

#[test]
fn bug42_let_array_discard_middle() {
    assert_val("[1, 2, 3] >> let[a, _, c]; a + c", int(4));
}

#[test]
fn bug42_let_array_discard_middle_passthrough() {
    // Passthrough is the original piped value
    assert_output("[1, 2, 3] >> let[a, _, c]", "[1, 2, 3]");
}

#[test]
fn bug42_let_struct_discard_passthrough() {
    // Struct destructure with discard also shouldn't crash
    assert_val("(1, 2, 3) >> let(_, b, c); b + c", int(5));
}

// ═══════════════════════════════════════════════════════════════════
// BUG-43: LetArray passthrough creates a named struct instead of
// passing through the original array. `[1, 2] >> let[a, b]` would
// return `(a=1, b=2)` (a struct) instead of `[1, 2]` (an array).
// This broke chaining: `[1, 2] >> let[a, b] >> len` would error
// because the struct has no `len` method.
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug43_let_array_passthrough_is_array() {
    // Passthrough should be an array, not a struct
    assert_val("[1, 2, 3] >> let[a, b, c] >> { in.len() }", int(3));
}

#[test]
fn bug43_let_array_passthrough_get() {
    assert_val("[10, 20, 30] >> let[a, b, c] >> { in.get(0) }", int(10));
}

#[test]
fn bug43_let_array_passthrough_identity() {
    assert_output("[1, 2] >> let[a, b] >> { in }", "[1, 2]");
}

#[test]
fn bug43_let_array_passthrough_with_rest() {
    assert_val("[1, 2, 3, 4] >> let[first, ...rest] >> { in.len() }", int(4));
}

#[test]
fn bug43_let_array_passthrough_with_rest_identity() {
    assert_output(
        "[1, 2, 3, 4] >> let[first, ...rest] >> { in }",
        "[1, 2, 3, 4]",
    );
}

#[test]
fn bug43_let_array_single_passthrough() {
    assert_val("[42] >> let[x] >> { in + 1 }", int(43));
}

#[test]
fn bug43_let_sugar_array_passthrough() {
    // `let x = [1, 2] >> let[a, b]; x` — x should be [1, 2] not (a=1, b=2)
    assert_val("let x = [1, 2] >> let[a, b]; x.len()", int(2));
}

// ═══════════════════════════════════════════════════════════════════
// BUG-44: Trailing semicolons cause parse error.
// `42;` at the end of input (or before `}`, `)`, `]`) fails with
// "unexpected token in expression: Eof". A trailing `;` should
// be valid and evaluate to `()`, treating it as `expr; ()`.
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug44_trailing_semi() {
    assert_val("42;", U);
}

#[test]
fn bug44_trailing_semi_after_let() {
    assert_val("let x = 1;", U);
}

#[test]
fn bug44_trailing_semi_after_tag() {
    assert_val("tag(Ok);", U);
}

#[test]
fn bug44_trailing_semi_sequence() {
    assert_val("1; 2; 3;", U);
}

#[test]
fn bug44_trailing_semi_in_block() {
    // Block with trailing semi — the block is a function, when called body evaluates to ()
    assert_val("5 >> { 42; }", U);
}

#[test]
fn bug44_trailing_semi_preserves_bindings() {
    // Trailing semi after tag should still bind the tag
    assert_output("tag(Ok); 42 >> Ok", "Ok(42)");
}

// ═══════════════════════════════════════════════════════════════════
// BUG-45: .field(args) on structs always parsed as MethodCall
// eval.rs — MethodCall should try struct field access before method dispatch
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug45_struct_field_call_named() {
    // Struct with a function field, called via .field(args)
    assert_val(
        "let math = (add = { in >> let(a, b); a + b }); math.add(3, 4)",
        int(7),
    );
}

#[test]
fn bug45_struct_field_call_positional() {
    // Positional struct field that is a function
    assert_val("let pair = ({ in + 1 }, { in * 2 }); pair.0(5)", int(6));
}

#[test]
fn bug45_struct_field_call_positional_second() {
    assert_val("let pair = ({ in + 1 }, { in * 2 }); pair.1(5)", int(10));
}

#[test]
fn bug45_struct_field_call_block_syntax() {
    // .field{ block } on struct should access field and call with block
    assert_val("let s = (f = { in >> let(x); x + 1 }); s.f(10)", int(11));
}

#[test]
fn bug45_struct_field_call_array_syntax() {
    // .field[array] on struct should access field and call with array
    assert_val("let s = (f = { in.len() }); s.f[1, 2, 3]", int(3));
}

#[test]
fn bug45_method_still_works_on_array() {
    // Method calls on non-struct types should still work
    assert_output("[1, 2, 3].map{ * 2 }", "[2, 4, 6]");
}

#[test]
fn bug45_method_still_works_on_string() {
    assert_val(r#""hello".char_len()"#, int(5));
}

#[test]
fn bug45_struct_as_module() {
    // The "struct as module" pattern from DESIGN.md
    assert_val(
        "let m = (double = { in * 2 }, inc = { in + 1 }); 5 >> m.double >> m.inc",
        int(11),
    );
}

// ═══════════════════════════════════════════════════════════════════
// BUG-46: \{ and \} escape sequences not supported in string literals
// lexer.rs — lex_string must handle \{ and \} escapes
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug46_escaped_open_brace() {
    assert_val(r#""\{""#, s("{"));
}

#[test]
fn bug46_escaped_close_brace() {
    assert_val(r#""\}""#, s("}"));
}

#[test]
fn bug46_escaped_braces_in_context() {
    assert_val(r#""Hello, \{world\}!""#, s("Hello, {world}!"));
}

#[test]
fn bug46_mixed_escapes() {
    assert_val(r#""\{\n\}""#, s("{\n}"));
}

// ═══════════════════════════════════════════════════════════════════
// BUG-47: _ discard in named struct destructuring fails
// eval.rs — bind_pattern must consume a field for _ discards in named mode
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug47_named_struct_discard_last() {
    // Discard the second field of a named struct
    assert_val("(x=1, y=2) >> let(x, _); x", int(1));
}

#[test]
fn bug47_named_struct_discard_first() {
    // Discard the first field of a named struct
    assert_val("(x=1, y=2) >> let(_, y); y", int(2));
}

#[test]
fn bug47_named_struct_discard_middle() {
    // Discard a middle field of a named struct
    assert_val("(x=1, y=2, z=3) >> let(x, _, z); x + z", int(4));
}

#[test]
fn bug47_named_struct_all_discards() {
    // Discard all fields (passthrough returns original value)
    assert_output("(x=1, y=2) >> let(_, _)", "(x=1, y=2)");
}

#[test]
fn bug47_positional_struct_discard_still_works() {
    // Positional discard should still work as before
    assert_val("(1, 2, 3) >> let(_, b, c); b + c", int(5));
}

#[test]
fn bug47_positional_struct_all_discards() {
    // Positional discard all fields
    assert_output("(1, 2) >> let(_, _)", "(1, 2)");
}

// ═══════════════════════════════════════════════════════════════════
// BUG-48: let(field=_) fails — parser rejects _ as binding in labeled pattern
// parser.rs — parse_remaining_pat_fields must accept Underscore after =
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug48_labeled_discard_binding() {
    // Discard a labeled field
    assert_val("(a=1, b=2) >> let(a=x, b=_); x", int(1));
}

#[test]
fn bug48_labeled_discard_first() {
    // Discard the first labeled field
    assert_val("(a=1, b=2) >> let(a=_, b=y); y", int(2));
}

#[test]
fn bug48_labeled_discard_multiple() {
    // Multiple discards in labeled destructure
    assert_val("(a=1, b=2, c=3) >> let(a=_, b=y, c=_); y", int(2));
}

#[test]
fn bug48_labeled_all_discards() {
    // All fields discarded with labels
    assert_output("(a=1, b=2) >> let(a=_, b=_)", "(a=1, b=2)");
}

// ═══════════════════════════════════════════════════════════════════
// BUG-49: () unit literal not recognized as branch pattern
// parser.rs — is_branch_block_start and parse_branch_pattern must handle ()
// eval.rs — literal pattern comparison must handle type mismatches gracefully
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug49_unit_branch_pattern_match() {
    assert_val(r#"() >> { () -> "unit", _ -> "other" }"#, s("unit"));
}

#[test]
fn bug49_unit_branch_pattern_no_match() {
    assert_val(r#"42 >> { () -> "unit", _ -> "other" }"#, s("other"));
}

#[test]
fn bug49_unit_branch_as_closure() {
    assert_val(
        r#"let f = { () -> "empty", x -> "something" }; f()"#,
        s("empty"),
    );
}

#[test]
fn bug49_mixed_type_literal_fallthrough() {
    // Different types in literal patterns should fall through, not error
    assert_val(
        r#""hello" >> { 42 -> "int", "hello" -> "found", _ -> "other" }"#,
        s("found"),
    );
}

#[test]
fn bug49_unit_pattern_with_guard_syntax() {
    // Ensure () pattern doesn't interfere with other patterns
    assert_val(
        r#"true >> { () -> "unit", true -> "yes", false -> "no" }"#,
        s("yes"),
    );
}

// ═══════════════════════════════════════════════════════════════════
// BUG-50: let _ = expr; body fails — parser doesn't handle _ in let sugar
// parser.rs — parse_standalone_let must accept _ before = for discard sugar
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug50_let_discard_sugar() {
    assert_val(r#"let _ = 42; "done""#, s("done"));
}

#[test]
fn bug50_let_discard_preserves_scope() {
    assert_val("let x = 10; let _ = x + 1; x", int(10));
}

#[test]
fn bug50_let_discard_evaluates_expr() {
    // The expression should still be evaluated (e.g., side effects)
    // We verify by ensuring no error occurs
    assert_val("let _ = 1 + 2; 99", int(99));
}

// ═══════════════════════════════════════════════════════════════════
// BUG-51: value >> obj.method(args) doesn't prepend piped value to method args
// eval.rs — eval_pipe must handle MethodCall like Call for arg prepending
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug51_pipe_into_struct_field_method_no_args() {
    // 5 >> obj.transform() should be obj.transform(5)
    assert_val(
        "let obj = (transform = { in * 2 }); 5 >> obj.transform()",
        int(10),
    );
}

#[test]
fn bug51_pipe_into_struct_field_method_with_args() {
    // 5 >> math.add(3) should be math.add(5, 3)
    assert_val(
        "let math = (add = { in >> let(a, b); a + b }); 5 >> math.add(3)",
        int(8),
    );
}

#[test]
fn bug51_pipe_into_builtin_method() {
    // [1, 2, 3] >> [4, 5, 6].zip() should be [4, 5, 6].zip([1, 2, 3])
    assert_output("[1, 2, 3] >> [4, 5, 6].zip()", "[(4, 1), (5, 2), (6, 3)]");
}

#[test]
fn bug51_regular_method_still_works() {
    // Ensure regular (non-piped) method calls are unaffected
    assert_output("[1, 2, 3].map{ * 2 }", "[2, 4, 6]");
}

#[test]
fn bug51_pipe_into_struct_method_block_syntax() {
    // 5 >> obj.f{block} should prepend piped value
    assert_val(
        "let obj = (apply = { in >> let(val, func); func(val) }); 5 >> obj.apply{ in + 1 }",
        int(6),
    );
}

// ── BUG-52: write_nested must escape special chars in strings ───────────────

#[test]
fn bug52_write_nested_escapes_newline() {
    assert_output(r#"["hello\nworld"]"#, r#"["hello\nworld"]"#);
}

#[test]
fn bug52_write_nested_escapes_tab() {
    assert_output(r#"["hello\tworld"]"#, r#"["hello\tworld"]"#);
}

#[test]
fn bug52_write_nested_escapes_quote() {
    assert_output(r#"["a\"b"]"#, r#"["a\"b"]"#);
}

#[test]
fn bug52_write_nested_escapes_in_struct() {
    assert_output(r#"("line1\nline2", 42)"#, r#"("line1\nline2", 42)"#);
}

#[test]
fn bug52_write_nested_escapes_in_tagged() {
    assert_output(r#"tag(W); "a\tb" >> W"#, r#"W("a\tb")"#);
}

// ── BUG-53: destructuring error should name the missing field ───────────────

#[test]
fn bug53_error_names_missing_field() {
    assert_error("(a=1, b=2, c=3) >> let(a, d)", "field 'd' not found");
}

#[test]
fn bug53_error_names_first_missing_when_multiple() {
    assert_error("(a=1, b=2) >> let(a, x)", "field 'x' not found");
}

// ── BUG-55: () should be destructurable as empty struct ─────────────────────

#[test]
fn bug55_unit_destructure_rest() {
    // () is the zero-field struct; ...rest should capture empty struct
    assert_output("() >> let(...rest); rest", "()");
}

#[test]
fn bug55_unit_destructure_discard_rest() {
    // Discarding rest of empty struct should work
    assert_val("() >> let(...); 42", int(42));
}

#[test]
fn bug55_unit_destructure_only_rest() {
    // Just a rest pattern on unit
    assert_val("() >> let(...r); r == ()", T);
}

#[test]
fn bug55_unit_single_name_binds_whole() {
    // let(x) is a single-name pattern, not destructuring — binds the whole value
    assert_val("() >> let(x); x", U);
}

#[test]
fn bug55_unit_destructure_positional_error() {
    // Trying to destructure two fields from () should error
    assert_error("() >> let(a, b)", "");
}

// ── BUG-54: let sugar should not leak bindings from parenthesized exprs ─────

#[test]
fn bug54_paren_scopes_inner_let() {
    assert_error(
        "let x = (42 >> let(y) >> let(z)); y",
        "undefined variable: y",
    );
}

#[test]
fn bug54_paren_scopes_inner_let_z() {
    assert_error(
        "let x = (42 >> let(y) >> let(z)); z",
        "undefined variable: z",
    );
}

#[test]
fn bug54_paren_let_x_still_bound() {
    assert_output("let x = (42 >> let(y) >> let(z)); x", "42");
}

#[test]
fn bug54_nonparen_let_nesting_still_works() {
    // Without parens, inner lets should still be in scope (by design)
    assert_output("let x = 42 >> let(y) >> let(z); x + y + z", "126");
}

#[test]
fn bug54_tag_sugar_still_works_with_let() {
    assert_output("let x = tag(Foo); 42 >> Foo", "Foo(42)");
}

#[test]
fn bug54_paren_tag_scope_limited() {
    // BUG-65 fix: per spec, "parentheses limit let scope".
    // (tag(Foo)) limits Foo to the paren scope, so Foo is undefined after.
    assert_error("(tag(Foo)); 42 >> Foo", "undefined variable");
}

#[test]
fn bug65_paren_let_scope_limited() {
    // BUG-65: (let x = 1; x); x — x should NOT be in scope after parens
    assert_error("(let x = 1; x); x", "undefined variable");
}

#[test]
fn bug65_paren_tag_scope_limited() {
    // Parenthesized tag scope is limited — Foo not visible outside
    assert_error("(tag(Foo)); Foo(1)", "undefined variable");
}

#[test]
fn bug65_nonparen_tag_still_works() {
    // Without parens, tag scope extends normally
    assert_output("tag(Foo); 42 >> Foo", "Foo(42)");
}

// ═══════════════════════════════════════════════════════════════════
// ROUND 9 — edge case testing
// ═══════════════════════════════════════════════════════════════════

// ── Edge case 1: empty string length ────────────────────────────────────────

#[test]
fn edge1_empty_string_length() {
    assert_val(r#""".char_len()"#, int(0));
}

// ── Edge case 2: string + non-string should error ───────────────────────────

#[test]
fn edge2_string_plus_nonstring_errors() {
    assert_error(r#""hello" + 42"#, "type error");
}

// ── Edge case 3: array + non-array should error ─────────────────────────────

#[test]
fn edge3_array_plus_nonarray_errors() {
    assert_error(r#"[] + "hello""#, "type error");
}

// ── Edge case 4: cross-type comparison should error ─────────────────────────

#[test]
fn edge4_cross_type_comparison_errors() {
    assert_error(r#"[1, 2] == "hello""#, "type error");
}

// ── Edge case 5: out of bounds struct field access ──────────────────────────

#[test]
fn edge5_struct_field_out_of_bounds() {
    assert_error("(1, 2).2", "field '2' not found");
}

// ── Edge case 6: missing named field access ─────────────────────────────────

#[test]
fn edge6_missing_named_field() {
    assert_error("(a=1, b=2).c", "field 'c' not found");
}

// ── Edge case 7: comparing tag constructors should error ────────────────────

#[test]
fn edge7_comparing_tag_constructors_equal() {
    // Tag constructors can be compared by identity (BUG-69 fix).
    assert_val("tag(A); tag(B); A == B", F);
}

// ── Edge case 8: matching non-tagged value with tag pattern ─────────────────

#[test]
fn edge8_match_untagged_with_tag_pattern() {
    // 1 >> A creates A(1), { A(x) -> x } extracts 1, then { A(y) -> y } fails
    // because 1 is not tagged. This is correct: non-exhaustive match.
    assert_error(
        "tag(A); 1 >> A >> { A(x) -> x } >> { A(y) -> y }",
        "no arm matched",
    );
}

// ── Edge case 9: float index to get should error ────────────────────────────

#[test]
fn edge9_float_index_errors() {
    assert_error("[1, 2, 3].get(1.5)", "type error");
}

// ── Edge case 10: fold with non-destructuring function ──────────────────────

#[test]
fn edge10_fold_nondestruct_function_errors() {
    // fold passes (acc=0, elem=1) as the struct; { in + 1 } tries to add
    // the whole struct to 1, which is a type error. This is correct behavior:
    // the user must destructure acc/elem.
    assert_error("[1, 2, 3].fold(0, { in + 1 })", "no method 'add'");
}

// ── Edge case 11: calling function with no args (unit) ──────────────────────

#[test]
fn edge11_call_function_with_unit() {
    // f() passes () as the argument. { in } returns its input, so f() returns ().
    assert_val("{ in } >> let(f); f()", U);
}

// ── Edge case 12: pipe into fold method ─────────────────────────────────────

#[test]
fn edge12_pipe_into_fold_method() {
    // 0 >> [1,2,3].fold({ in.acc + in.elem }) — piped value prepends to args,
    // so this becomes [1,2,3].fold(0, { in.acc + in.elem })
    assert_val("0 >> [1,2,3].fold({ in.acc + in.elem })", int(6));
}

// ── Edge case 13: calling tag constructor with unit via () ──────────────────

#[test]
fn edge13_tag_constructor_with_unit() {
    // A() calls the tag constructor with (), creating A(()) which displays as "A"
    assert_output("tag(A); A()", "A");
}

// ── Edge case 14: piping into builtin ───────────────────────────────────────

#[test]
fn edge14_pipe_into_not() {
    assert_val("true >> not", F);
}

// ── Edge case 15: piping tuple into and ─────────────────────────────────────

#[test]
fn edge15_pipe_tuple_into_and() {
    // (true, false) >> and = and(true, false) — but and takes a struct with two
    // boolean fields, so piping the tuple works.
    assert_val("(true, false) >> and", F);
}

// ── Edge case 16: shadowing via let sugar ───────────────────────────────────

#[test]
fn edge16_let_shadowing() {
    assert_val("let x = 1; let x = 2; x", int(2));
}

// ── Edge case 17: chained methods with explicit in ──────────────────────────

#[test]
fn edge17_chained_methods_explicit_in() {
    // [1,2,3].map{ in * 2 } => [2,4,6], then .filter{ in > 4 } => [6]
    assert_output("[1, 2, 3].map{ in * 2 }.filter{ in > 4 }", "[6]");
}

// ── Edge case 18: unit-tagged branch matching ───────────────────────────────
// BUG: `None` (bare tag constructor) piped into a branch fails because the
// branch receives the constructor function, not the tagged value `None(())`.
// The user must write `() >> None` or `None()` to create the tagged value.
// This is a usability issue / design gap: many users expect `None` alone to
// be the tagged value, not the constructor function.

#[test]
fn edge18_bare_tag_constructor_matched() {
    // Tag constructors can be matched in branch patterns (BUG-70 fix).
    assert_val(
        r#"tag(None); tag(Some); None >> { None -> "empty", Some(x) -> x }"#,
        s("empty"),
    );
}

#[test]
fn edge18_unit_tagged_branch_matching_correct_form() {
    // The correct way: use () >> None or None() to create the tagged value.
    assert_val(
        r#"tag(None); tag(Some); () >> None >> { None -> "empty", Some(x) -> x }"#,
        s("empty"),
    );
    assert_val(
        r#"tag(None); tag(Some); None() >> { None -> "empty", Some(x) -> x }"#,
        s("empty"),
    );
}

// ── Edge case 19: empty array length ────────────────────────────────────────

#[test]
fn edge19_empty_array_length() {
    assert_val("[].len()", int(0));
}

// ── Edge case 20: map over empty array ──────────────────────────────────────

#[test]
fn edge20_map_empty_array() {
    assert_output("[].map{ in * 2 }", "[]");
}

// ═══════════════════════════════════════════════════════════════════
// BUG-56: Empty program should return ()
// parser.rs — parse_program should handle empty input (or whitespace-only)
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug56_empty_program() {
    assert_val("", U);
}

#[test]
fn bug56_whitespace_only_program() {
    assert_val("   \n\n  ", U);
}

#[test]
fn bug56_comment_only_program() {
    assert_val("# just a comment", U);
}

// ═══════════════════════════════════════════════════════════════════
// BUG-58: \x hex escapes missing in byte and char literals
// lexer.rs — lex_byte and lex_char must handle \xNN hex escapes
// The display code outputs non-printable bytes as b'\xNN' but the
// lexer couldn't parse them back — round-trip failure.
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug58_byte_hex_escape_ff() {
    assert_output(r"b'\xff'", r"b'\xff'");
}

#[test]
fn bug58_byte_hex_escape_00() {
    // b'\x00' is the same as b'\0'
    assert_output(r"b'\x00'", r"b'\0'");
}

#[test]
fn bug58_byte_hex_escape_7f() {
    // 0x7f is DEL, non-printable
    assert_output(r"b'\x7f'", r"b'\x7f'");
}

#[test]
fn bug58_byte_hex_escape_printable() {
    // 0x41 = 'A', printable — should display as b'A'
    assert_val(r"b'\x41'", byte(b'A'));
}

#[test]
fn bug58_char_hex_escape() {
    // \x41 in char = 'A'
    assert_val(r"'\x41'", ch('A'));
}

#[test]
fn bug58_char_hex_escape_null() {
    // \x00 in char = '\0'
    assert_output(r"'\x00'", r"'\0'");
}

// ═══════════════════════════════════════════════════════════════════
// BUG-59: REPL multi-field destructuring doesn't persist bindings
// eval_pipe_collecting in eval.rs doesn't bind the "\0" passthrough
// variable that identity_expr_for_pattern uses for multi-field
// patterns. Regular eval_pipe (line ~262) binds it, but the REPL's
// eval_pipe_collecting (line ~1092) does not. As a result, any
// multi-field destructuring (struct, array, labeled, or rest) that
// is the LAST expression on a REPL line fails with:
//   "runtime error: undefined variable: \0"
// Single-name let(x) works because its identity expression is
// Ident(name), not Ident("\0").
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug59_repl_struct_destructure() {
    // (1, 2) >> let(a, b) in REPL, then a + b on next line
    let env = repl_env();
    let (_, env) = nana::run_in_env("(1, 2) >> let(a, b)", &env).unwrap();
    let (val, _) = nana::run_in_env("a + b", &env).unwrap();
    assert_eq!(val.to_string(), "3");
}

#[test]
fn bug59_repl_array_destructure() {
    // [10, 20] >> let[a, b] in REPL, then a + b on next line
    let env = repl_env();
    let (_, env) = nana::run_in_env("[10, 20] >> let[a, b]", &env).unwrap();
    let (val, _) = nana::run_in_env("a + b", &env).unwrap();
    assert_eq!(val.to_string(), "30");
}

#[test]
fn bug59_repl_labeled_destructure() {
    // (x=10, y=20) >> let(x, y) in REPL, then x + y on next line
    let env = repl_env();
    let (_, env) = nana::run_in_env("(x=10, y=20) >> let(x, y)", &env).unwrap();
    let (val, _) = nana::run_in_env("x + y", &env).unwrap();
    assert_eq!(val.to_string(), "30");
}

#[test]
fn bug59_repl_explicit_labeled_destructure() {
    // (a=10, b=20) >> let(a=x, b=y) in REPL, then x + y on next line
    let env = repl_env();
    let (_, env) = nana::run_in_env("(a=10, b=20) >> let(a=x, b=y)", &env).unwrap();
    let (val, _) = nana::run_in_env("x + y", &env).unwrap();
    assert_eq!(val.to_string(), "30");
}

#[test]
fn bug59_repl_three_element_destructure() {
    // (1, 2, 3) >> let(a, b, c) in REPL, then a + b + c on next line
    let env = repl_env();
    let (_, env) = nana::run_in_env("(1, 2, 3) >> let(a, b, c)", &env).unwrap();
    let (val, _) = nana::run_in_env("a + b + c", &env).unwrap();
    assert_eq!(val.to_string(), "6");
}

#[test]
fn bug59_repl_destructure_passthrough_value() {
    // (1, 2) >> let(a, b) should return (1, 2) as the passthrough value
    let env = repl_env();
    let (val, _) = nana::run_in_env("(1, 2) >> let(a, b)", &env).unwrap();
    assert_eq!(val.to_string(), "(1, 2)");
}

#[test]
fn bug59_repl_array_rest_pattern() {
    // [1, 2, 3, 4] >> let[first, ...rest] in REPL
    let env = repl_env();
    let (_, env) = nana::run_in_env("[1, 2, 3, 4] >> let[first, ...rest]", &env).unwrap();
    let (val, _) = nana::run_in_env("first", &env).unwrap();
    assert_eq!(val.to_string(), "1");
    let (val, _) = nana::run_in_env("rest", &env).unwrap();
    assert_eq!(val.to_string(), "[2, 3, 4]");
}

#[test]
fn bug59_repl_struct_rest_pattern() {
    // (a=1, b=2, c=3) >> let(a=x, ...rest) in REPL
    let env = repl_env();
    let (_, env) = nana::run_in_env("(a=1, b=2, c=3) >> let(a=x, ...rest)", &env).unwrap();
    let (val, _) = nana::run_in_env("x", &env).unwrap();
    assert_eq!(val.to_string(), "1");
    let (val, _) = nana::run_in_env("rest", &env).unwrap();
    assert_eq!(val.to_string(), "(b=2, c=3)");
}

// ═══════════════════════════════════════════════════════════════════
// BUG-60: \x hex escape missing in string literals
// lexer.rs — lex_string must handle \xNN like lex_byte and lex_char
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug60_string_hex_escape_basic() {
    assert_val(r#""\x41""#, s("A"));
}

#[test]
fn bug60_string_hex_escape_null() {
    assert_val(r#""\x00".char_len()"#, int(1));
}

#[test]
fn bug60_string_hex_escape_mixed() {
    assert_val(r#""\x48\x65\x6c\x6c\x6f""#, s("Hello"));
}

// ═══════════════════════════════════════════════════════════════════
// BUG-61: (1,) creates single-field struct instead of just 1
// parser.rs — trailing comma in single-element paren should be grouping
// Per spec: "(1) is just the parenthesized expression 1. There is no
// distinct single-element tuple type." Trailing commas are allowed.
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug61_trailing_comma_single_is_value() {
    // (1,) should be the same as (1) = just 1
    assert_val("(1,)", int(1));
}

#[test]
fn bug61_trailing_comma_single_in_expr() {
    // (1 + 2,) should be 3
    assert_val("(1 + 2,)", int(3));
}

#[test]
fn bug61_trailing_comma_multi_still_struct() {
    // (1, 2,) should still be a struct with trailing comma
    assert_output("(1, 2,)", "(1, 2)");
}

#[test]
fn bug61_trailing_comma_named_still_struct() {
    // (a=1, b=2,) is a struct with trailing comma
    assert_val("(a=1, b=2,).a", int(1));
}

// ═══════════════════════════════════════════════════════════════════
// BUG-62: f(1,) creates single-field struct instead of passing 1
// parser.rs — parse_call_args must handle trailing comma like parse_paren
// ═══════════════════════════════════════════════════════════════════

#[test]
fn bug62_call_trailing_comma_passes_value() {
    // f(1,) should pass 1 to f, not a struct
    assert_val("{ in } >> let(f); f(1,)", int(1));
}

#[test]
fn bug62_call_trailing_comma_arithmetic() {
    // f(1 + 2,) should pass 3, not a struct
    assert_val("{ in * 2 } >> let(f); f(1 + 2,)", int(6));
}

#[test]
fn bug62_call_multi_arg_trailing_comma_still_struct() {
    // f(1, 2,) should still pass (1, 2) struct
    assert_val("{ in >> let(a, b); a + b } >> let(f); f(1, 2,)", int(3));
}

// ═══════════════════════════════════════════════════════════════════
// STRING FEATURE TESTS — exploring string capabilities and gaps
// ═══════════════════════════════════════════════════════════════════

// ── String Interpolation (DESIGN.md says {expr} inside strings) ──

#[test]
fn string_interpolation_basic() {
    // DESIGN.md says: "String interpolation via {expr} inside string literals:
    // "Hello, {name}!" evaluates the expression and converts the result to a string."
    let name_binding = r#"let name = "world"; "Hello, {name}!""#;
    assert_val(name_binding, s("Hello, world!"));
}

#[test]
fn string_interpolation_with_expr() {
    // {1 + 2} inside a string evaluates the expression
    assert_val(r#""result: {1 + 2}""#, s("result: 3"));
}

#[test]
fn string_interpolation_multiple_parts() {
    assert_val(
        r#"let a = 1; let b = 2; "{a} + {b} = {a + b}""#,
        s("1 + 2 = 3"),
    );
}

#[test]
fn string_interpolation_bool() {
    assert_val(r#""it is {true}""#, s("it is true"));
}

#[test]
fn string_interpolation_nested_string() {
    // Interpolating a string value — should produce unquoted result
    assert_val(r#"let x = "hi"; "say {x}""#, s("say hi"));
}

#[test]
fn string_interpolation_escaped_brace() {
    // \{ produces a literal brace, not interpolation
    assert_val(r#""\{not interpolated\}""#, s("{not interpolated}"));
}

#[test]
fn string_interpolation_empty_string_parts() {
    // Interpolation at start and end of string
    assert_val(r#"let x = 42; "{x}""#, s("42"));
}

#[test]
fn string_interpolation_adjacent() {
    // Two interpolations next to each other
    assert_val(r#"let a = "x"; let b = "y"; "{a}{b}""#, s("xy"));
}

#[test]
fn string_interpolation_with_pipe() {
    // Expression with pipe inside interpolation
    assert_val(r#"let f = { in + 1 }; "result: {3 >> f}""#, s("result: 4"));
}

#[test]
fn string_no_interpolation_plain() {
    // A plain string with no braces stays as Token::Str, not InterpStr
    assert_val(r#""hello world""#, s("hello world"));
}

// ── String Concatenation ─────────────────────────────────────────

#[test]
fn string_concat_basic() {
    assert_val(r#""abc" + "def""#, s("abcdef"));
}

#[test]
fn string_concat_empty_left() {
    assert_val(r#""" + "abc""#, s("abc"));
}

#[test]
fn string_concat_empty_right() {
    assert_val(r#""abc" + """#, s("abc"));
}

#[test]
fn string_concat_both_empty() {
    assert_val(r#""" + """#, s(""));
}

#[test]
fn string_concat_chained() {
    assert_val(r#""a" + "b" + "c""#, s("abc"));
}

// ── String Length ────────────────────────────────────────────────

#[test]
fn string_len_method() {
    assert_val(r#""hello".char_len()"#, int(5));
}

#[test]
fn string_len_piped() {
    // char_len as a method call (no builtin for strings)
    assert_val(r#""hello".char_len()"#, int(5));
}

#[test]
fn string_len_empty() {
    assert_val(r#""".char_len()"#, int(0));
}

#[test]
fn string_len_empty_piped() {
    assert_val(r#""".char_len()"#, int(0));
}

#[test]
fn string_len_with_escapes() {
    // "\n" is one character, so "ab\ncd" has 5 characters
    assert_val(r#""ab\ncd".char_len()"#, int(5));
}

#[test]
fn string_len_null_byte() {
    // "\0" is one character
    assert_val(r#""\0".char_len()"#, int(1));
}

// ── String Comparison: Equality ─────────────────────────────────

#[test]
fn string_eq_same() {
    assert_val(r#""abc" == "abc""#, T);
}

#[test]
fn string_eq_different() {
    assert_val(r#""abc" == "def""#, F);
}

#[test]
fn string_neq_different() {
    assert_val(r#""abc" != "def""#, T);
}

#[test]
fn string_neq_same() {
    assert_val(r#""abc" != "abc""#, F);
}

#[test]
fn string_eq_empty() {
    assert_val(r#""" == """#, T);
}

#[test]
fn string_neq_empty_vs_nonempty() {
    assert_val(r#""" != "a""#, T);
}

// ── String Comparison: Ordering ─────────────────────────────────

#[test]
fn string_lt_true() {
    // Lexicographic: "abc" < "abd" because 'c' < 'd'
    assert_val(r#""abc" < "abd""#, T);
}

#[test]
fn string_lt_false() {
    assert_val(r#""abd" < "abc""#, F);
}

#[test]
fn string_gt_true() {
    assert_val(r#""def" > "abc""#, T);
}

#[test]
fn string_gt_false() {
    assert_val(r#""abc" > "def""#, F);
}

#[test]
fn string_le_equal() {
    assert_val(r#""abc" <= "abc""#, T);
}

#[test]
fn string_le_less() {
    assert_val(r#""abc" <= "abd""#, T);
}

#[test]
fn string_ge_equal() {
    assert_val(r#""abc" >= "abc""#, T);
}

#[test]
fn string_ge_greater() {
    assert_val(r#""abd" >= "abc""#, T);
}

#[test]
fn string_lt_prefix() {
    // "abc" < "abcd" — shorter string is less than longer prefix-match
    assert_val(r#""abc" < "abcd""#, T);
}

#[test]
fn string_gt_prefix() {
    // "abcd" > "abc" — longer string with same prefix is greater
    assert_val(r#""abcd" > "abc""#, T);
}

// ── Empty String ────────────────────────────────────────────────

#[test]
fn empty_string_standalone() {
    // An empty string should display as empty (no quotes in standalone display)
    assert_val(r#""""#, s(""));
}

#[test]
fn empty_string_len() {
    assert_val(r#""".char_len()"#, int(0));
}

#[test]
fn empty_string_concat_left() {
    assert_val(r#""" + "abc""#, s("abc"));
}

#[test]
fn empty_string_in_array() {
    // Empty string inside array should display as ""
    assert_output(r#"["", "a"]"#, r#"["", "a"]"#);
}

// ── Multi-line Strings (Zig-style) ──────────────────────────────

#[test]
fn multiline_string_single_line() {
    assert_val("\\\\hello", s("hello"));
}

#[test]
fn multiline_string_two_lines() {
    assert_val("\\\\hello\n\\\\world", s("hello\nworld"));
}

#[test]
fn multiline_string_with_indentation() {
    // Leading whitespace before \\ on continuation lines is stripped
    assert_val("\\\\first\n    \\\\second", s("first\nsecond"));
}

#[test]
fn multiline_string_len() {
    // "ab\ncd" has 5 characters (2 + newline + 2)
    assert_val("\\\\ab\n\\\\cd\n>> { in.char_len() }", int(5));
}

#[test]
fn multiline_string_in_let() {
    // The \\ lines form the string, then the next non-\\ line continues the program
    assert_val("let s =\n\\\\hello\n\\\\world\n; s", s("hello\nworld"));
}

#[test]
fn multiline_string_as_argument() {
    assert_val("(\n\\\\abc\n).char_len()", int(3));
}

// ── String Escape Sequences ─────────────────────────────────────

#[test]
fn string_escape_newline() {
    // "\n" is a single newline character
    assert_val(r#""\n".char_len()"#, int(1));
}

#[test]
fn string_escape_tab() {
    assert_val(r#""\t".char_len()"#, int(1));
}

#[test]
fn string_escape_carriage_return() {
    assert_val(r#""\r".char_len()"#, int(1));
}

#[test]
fn string_escape_backslash() {
    assert_val(r#""\\".char_len()"#, int(1));
}

#[test]
fn string_escape_quote() {
    assert_val(r#""\"".char_len()"#, int(1));
}

#[test]
fn string_escape_null() {
    assert_val(r#""\0".char_len()"#, int(1));
}

#[test]
fn string_escape_all_combined() {
    // "\n\t\r\\\"\0" should be 6 characters
    assert_val(r#""\n\t\r\\\"\0".char_len()"#, int(6));
}

// ── String Brace Escapes ────────────────────────────────────────

#[test]
fn string_escaped_open_brace() {
    assert_val(r#""\{""#, s("{"));
}

#[test]
fn string_escaped_close_brace() {
    assert_val(r#""\}""#, s("}"));
}

#[test]
fn string_escaped_both_braces() {
    assert_val(r#""\{\}""#, s("{}"));
}

#[test]
fn string_escaped_braces_with_text() {
    assert_val(r#""fn\{body\}""#, s("fn{body}"));
}

#[test]
fn string_interp_variable() {
    // Unescaped { } in a string triggers interpolation.
    // With a defined variable, the expression is evaluated and stringified.
    assert_val(r#"let hello = "world"; "{hello}""#, s("world"));
}

#[test]
fn string_interp_undefined_errors() {
    // Unescaped { } in a string with undefined variable is a runtime error.
    assert_error(r#""{hello}""#, "undefined variable");
}

// ── String Methods ───────────────────────────────────────────────

#[test]
fn string_as_bytes() {
    assert_output(r#""abc".as_bytes()"#, "[b'a', b'b', b'c']");
}

#[test]
fn string_as_bytes_empty() {
    assert_output(r#""".as_bytes()"#, "[]");
}

#[test]
fn string_chars() {
    assert_output(r#""hi".chars()"#, "['h', 'i']");
}

#[test]
fn string_chars_empty() {
    assert_output(r#""".chars()"#, "[]");
}

#[test]
fn string_split_basic() {
    assert_output(r#""a,b,c".split(",")"#, r#"["a", "b", "c"]"#);
}

#[test]
fn string_split_no_match() {
    assert_output(r#""abc".split(",")"#, r#"["abc"]"#);
}

#[test]
fn string_split_empty_delimiter() {
    // Rust's split("") splits between every character (plus edges)
    assert_output(r#""ab".split("")"#, r#"["", "a", "b", ""]"#);
}

#[test]
fn string_trim_spaces() {
    assert_val(r#""  hello  ".trim()"#, s("hello"));
}

#[test]
fn string_trim_no_whitespace() {
    assert_val(r#""hello".trim()"#, s("hello"));
}

#[test]
fn string_contains_true() {
    assert_val(r#""hello world".contains("world")"#, T);
}

#[test]
fn string_contains_false() {
    assert_val(r#""hello".contains("xyz")"#, F);
}

#[test]
fn string_contains_char() {
    assert_val(r#""hello".contains_char('l')"#, T);
}

#[test]
fn string_starts_with_true() {
    assert_val(r#""hello".starts_with("hel")"#, T);
}

#[test]
fn string_starts_with_false() {
    assert_val(r#""hello".starts_with("world")"#, F);
}

#[test]
fn string_ends_with_true() {
    assert_val(r#""hello".ends_with("llo")"#, T);
}

#[test]
fn string_ends_with_false() {
    assert_val(r#""hello".ends_with("hel")"#, F);
}

#[test]
fn string_replace_basic() {
    assert_val(r#""hello world".replace("world", "nana")"#, s("hello nana"));
}

#[test]
fn string_replace_multiple() {
    assert_val(r#""aaa".replace("a", "bb")"#, s("bbbbbb"));
}

#[test]
fn string_slice_basic() {
    assert_val(r#""hello".slice(1..3)"#, s("el"));
}

#[test]
fn string_slice_full() {
    assert_val(r#""abc".slice(0..3)"#, s("abc"));
}

#[test]
fn string_slice_out_of_bounds() {
    assert_error(r#""abc".slice(0..5)"#, "out of bounds");
}

#[test]
fn string_method_chain() {
    // trim then split
    assert_output(r#""  a,b  ".trim().split(",")"#, r#"["a", "b"]"#);
}

// ── String Destructuring ─────────────────────────────────────────

#[test]
fn string_destructure_basic() {
    assert_val(r#""abc" >> let[a, b, c]; a"#, s("a"));
}

#[test]
fn string_destructure_second_char() {
    assert_val(r#""abc" >> let[a, b, c]; b"#, s("b"));
}

#[test]
fn string_destructure_with_rest() {
    assert_val(r#""hello" >> let[first, ...rest]; first"#, s("h"));
}

#[test]
fn string_destructure_rest_is_string() {
    // ...rest captures remaining characters as a string
    assert_val(r#""hi" >> let[h, ...rest]; rest"#, s("i"));
}

#[test]
fn string_destructure_head_tail() {
    assert_val(r#""abc" >> let[_, ...tail]; tail"#, s("bc"));
}

#[test]
fn string_destructure_too_few_elements() {
    assert_error(r#""ab" >> let[a, b, c]"#, "not enough elements");
}

#[test]
fn string_destructure_too_many_elements() {
    assert_error(r#""abcd" >> let[a, b]"#, "too many elements");
}

#[test]
fn string_destructure_empty_string() {
    assert_val(r#""" >> let[...rest]; rest"#, s(""));
}

#[test]
fn string_destructure_let_sugar() {
    assert_val(r#"let s = "xy"; s >> let[x, y]; x"#, s("x"));
}

#[test]
fn string_destructure_concat_parts() {
    // Destructured parts are strings, so they can be concatenated back
    assert_val(r#""abc" >> let[a, b, c]; a + b + c"#, s("abc"));
}

// ── let [...] = expr sugar ──────────────────────────────────────

#[test]
fn let_array_assign_sugar_basic() {
    assert_val(r#"let [a, b, c] = "abc"; b"#, s("b"));
}

#[test]
fn let_array_assign_sugar_rest() {
    assert_val(r#"let [a, ...rest] = "hello"; a"#, s("h"));
}

#[test]
fn let_array_assign_sugar_array() {
    assert_val("let [x, y] = [10, 20]; x + y", int(30));
}

#[test]
fn let_struct_assign_sugar() {
    assert_val("let (a, b) = (1, 2); a + b", int(3));
}

// ── Strings in Structs ──────────────────────────────────────────

#[test]
fn string_in_positional_struct() {
    assert_output(r#"(a="hello", b="world")"#, r#"(a="hello", b="world")"#);
}

#[test]
fn string_in_struct_field_access() {
    assert_val(r#"(a="hello", b="world").a"#, s("hello"));
}

#[test]
fn string_in_struct_field_access_b() {
    assert_val(r#"(a="hello", b="world").b"#, s("world"));
}

// ═══════════════════════════════════════════════════════════════════
// ROUND 20a — Pipe+Let interaction probes and DESIGN.md spec examples
// ═══════════════════════════════════════════════════════════════════

// ── 1. Spec examples from DESIGN.md ─────────────────────────────

#[test]
fn probe20a_spec_basic_binding() {
    // DESIGN.md: `1 >> let(x); x + 2` evaluates to 3
    assert_val("1 >> let(x); x + 2", int(3));
}

#[test]
fn probe20a_spec_let_sugar() {
    // DESIGN.md: `let x = 1; x + 2` (sugar form, same result)
    assert_val("let x = 1; x + 2", int(3));
}

#[test]
fn probe20a_spec_chained_bindings() {
    // DESIGN.md: `1 >> let(x); x >> { in + 1 } >> let(y); x + y` evaluates to 3
    assert_val("1 >> let(x); x >> { in + 1 } >> let(y); x + y", int(3));
}

#[test]
fn probe20a_spec_paren_scope_limits_let() {
    // DESIGN.md: `1 >> let(x); (2 >> let(y); y + 1) + x` evaluates to 4
    assert_val("1 >> let(x); (2 >> let(y); y + 1) + x", int(4));
}

#[test]
fn probe20a_spec_let_returns_value_through_pipe() {
    // DESIGN.md: `value >> let(x) >> log` — let returns its value, pipes onward.
    // Testing with print: "hi" >> let(x) >> print prints "hi" and returns ().
    // Then `; x` evaluates to "hi".
    assert_val(r#""hi" >> let(x) >> print; x"#, s("hi"));
}

// ── 2. let(x) returns x through pipes ───────────────────────────

#[test]
fn probe20a_let_passthrough_into_block() {
    // 5 >> let(x) >> { in + 10 } — let(x) returns 5, then 5 + 10 = 15
    assert_val("5 >> let(x) >> { in + 10 }", int(15));
}

#[test]
fn probe20a_let_passthrough_double_bind() {
    // 5 >> let(x) >> let(y); x + y — both x and y are 5, so 10
    assert_val("5 >> let(x) >> let(y); x + y", int(10));
}

// ── 3. Multiple bindings with pipes ─────────────────────────────

#[test]
fn probe20a_triple_let_chain_in_pipes() {
    // 1 >> let(a) >> { in + 1 } >> let(b) >> { in + 1 } >> let(c); a + b + c
    // a=1, b=2, c=3, sum=6
    assert_output(
        "1 >> let(a) >> { in + 1 } >> let(b) >> { in + 1 } >> let(c); a + b + c",
        "6",
    );
}

// ── 4. Tag in pipes ─────────────────────────────────────────────

#[test]
fn probe20a_tag_in_pipe_chain() {
    // tag(Ok); 42 >> Ok >> { Ok(x) -> x + 1, _ -> 0 } should be 43
    assert_val("tag(Ok); 42 >> Ok >> { Ok(x) -> x + 1, _ -> 0 }", int(43));
}

// ── 5. Nested block scoping ─────────────────────────────────────

#[test]
fn probe20a_nested_block_scoping() {
    // let x = 10; (let x = 20; x) + x — inner x = 20, outer x = 10, result = 30
    assert_val("let x = 10; (let x = 20; x) + x", int(30));
}

// ── 6. Shadowing ────────────────────────────────────────────────

#[test]
fn probe20a_shadowing_basic() {
    // let x = 1; let x = 2; x — shadowed, result = 2
    assert_val("let x = 1; let x = 2; x", int(2));
}

#[test]
fn probe20a_shadowing_preserves_old_capture() {
    // let x = 1; let y = x + 1; let x = 10; y — y captured old x = 1, so y = 2
    assert_val("let x = 1; let y = x + 1; let x = 10; y", int(2));
}

// ── 7. let inside block body ────────────────────────────────────

#[test]
fn probe20a_let_inside_block_body() {
    // 10 >> { let x = in + 1; let y = x * 2; y } — x=11, y=22, result=22
    assert_val("10 >> { let x = in + 1; let y = x * 2; y }", int(22));
}

// ── 8. Array destructuring in pipe ──────────────────────────────

#[test]
fn probe20a_array_destruct_in_pipe() {
    // [10, 20, 30] >> let[a, ...rest]; a + rest.len()
    // a=10, rest=[20, 30], rest.len()=2, result=12
    assert_val("[10, 20, 30] >> let[a, ...rest]; a + rest.len()", int(12));
}

// ── 9. Complex pipe chain ending with branch ────────────────────

#[test]
fn probe20a_complex_pipe_branch() {
    // tag(Ok); tag(Err); 42 >> Ok >> { Ok(x) -> x * 2, Err(_) -> 0 } should be 84
    assert_output(
        "tag(Ok); tag(Err); 42 >> Ok >> { Ok(x) -> x * 2, Err(_) -> 0 }",
        "84",
    );
}

// ── 10. Empty branch body ───────────────────────────────────────

#[test]
fn probe20a_empty_branch_body() {
    // true >> { true -> (), false -> () } should be ()
    assert_val("true >> { true -> (), false -> () }", U);
}

// ── 11. Pipe through multiple functions ─────────────────────────

#[test]
fn probe20a_pipe_through_multiple_functions() {
    // let double = { in * 2 }; let add1 = { in + 1 }; 3 >> double >> add1
    // 3 * 2 = 6, 6 + 1 = 7
    assert_val(
        "let double = { in * 2 }; let add1 = { in + 1 }; 3 >> double >> add1",
        int(7),
    );
}

// ── 12. Block that captures and uses outer binding ──────────────

#[test]
fn probe20a_block_captures_outer_binding() {
    // let x = 5; let f = { in + x }; 10 >> f — should be 15
    assert_val("let x = 5; let f = { in + x }; 10 >> f", int(15));
}

#[test]
fn string_in_struct_len_via_field() {
    assert_val(r#"(a="hello", b="world").a.char_len()"#, int(5));
}

// ── Strings in Arrays ───────────────────────────────────────────

#[test]
fn string_array_literal() {
    assert_output(r#"["a", "b", "c"]"#, r#"["a", "b", "c"]"#);
}

#[test]
fn string_array_get() {
    assert_val(r#"["a", "b", "c"].get(0)"#, s("a"));
}

#[test]
fn string_array_get_last() {
    assert_val(r#"["a", "b", "c"].get(2)"#, s("c"));
}

#[test]
fn string_array_len() {
    assert_val(r#"["a", "b", "c"].len()"#, int(3));
}

#[test]
fn string_array_map_len() {
    // Map over string array getting char_len of each element
    assert_output(r#"["hello", "hi", "hey"].map{ in.char_len() }"#, "[5, 2, 3]");
}

// ── String Method: .char_len() / .byte_len() ────────────────────

#[test]
fn string_method_char_len() {
    assert_val(r#""hello".char_len()"#, int(5));
}

#[test]
fn string_method_char_len_single_char() {
    assert_val(r#""x".char_len()"#, int(1));
}

#[test]
fn string_method_char_len_after_concat() {
    assert_val(r#"("abc" + "def").char_len()"#, int(6));
}

// ── String Comparison Ordering ──────────────────────────────────

#[test]
fn string_order_abc_lt_abd() {
    // Last character differs: 'c' < 'd'
    assert_val(r#""abc" < "abd""#, T);
}

#[test]
fn string_order_case_sensitive() {
    // 'A' (65) < 'a' (97) in byte ordering
    assert_val(r#""A" < "a""#, T);
}

#[test]
fn string_order_empty_lt_nonempty() {
    // Empty string is less than any non-empty string
    assert_val(r#""" < "a""#, T);
}

#[test]
fn string_order_same_not_lt() {
    assert_val(r#""abc" < "abc""#, F);
}

// ── Cross-type String Errors ────────────────────────────────────

#[test]
fn string_plus_int_error() {
    // String + Int should be a type error
    assert_error(r#""abc" + 1"#, "type error");
}

#[test]
fn string_plus_bool_error() {
    assert_error(r#""abc" + true"#, "type error");
}

#[test]
fn string_eq_int_error() {
    // Comparing string to int should error
    assert_error(r#""abc" == 1"#, "type error");
}

// ═══════════════════════════════════════════════════════════════════
// PROBE 17: Edge cases for evaluation and scoping
// ═══════════════════════════════════════════════════════════════════

// ── 1. Let binding returns its value ────────────────────────────

#[test]
fn probe17_let_returns_value_into_block() {
    // Per spec: let(x) returns x, so `1 >> let(x) >> { in + 1 }` should give 2.
    // The piped value 1 is bound to x, then 1 flows into { in + 1 }.
    assert_val("1 >> let(x) >> { in + 1 }", int(2));
}

#[test]
fn probe17_let_returns_value_into_add() {
    // `5 >> let(x) >> { in + x }` — in is 5, x is 5, result is 10
    assert_val("5 >> let(x) >> { in + x }", int(10));
}

#[test]
fn probe17_let_returns_value_into_tag() {
    // `42 >> let(x) >> W` — let passthrough, then wrap in W tag
    assert_val("tag(W); 42 >> let(x) >> W >> { W(v) -> v }", int(42));
}

// ── 2. Semicolon sequencing with complex expressions ────────────

#[test]
fn probe17_semicolon_arithmetic_sequencing() {
    // `1 + 2; 3 + 4` should evaluate 1+2 (discard), then 3+4 = 7
    assert_val("1 + 2; 3 + 4", int(7));
}

#[test]
fn probe17_semicolon_let_chain() {
    // `let x = 1; let y = 2; x + y` should be 3
    assert_val("let x = 1; let y = 2; x + y", int(3));
}

#[test]
fn probe17_semicolon_three_exprs() {
    // `1; 2; 3` should give 3 (last expression)
    assert_val("1; 2; 3", int(3));
}

#[test]
fn probe17_semicolon_with_pipe() {
    // `let x = 10; x >> { in * 2 }` should be 20
    assert_val("let x = 10; x >> { in * 2 }", int(20));
}

// ── 3. Block sugar edge cases ───────────────────────────────────

#[test]
fn probe17_block_sugar_mul() {
    // `5 >> { * 2 }` is sugar for `5 >> { in * 2 }` = 10
    assert_val("5 >> { * 2 }", int(10));
}

#[test]
fn probe17_block_sugar_add() {
    // `5 >> { + 3 }` is sugar for `5 >> { in + 3 }` = 8
    assert_val("5 >> { + 3 }", int(8));
}

#[test]
fn probe17_block_sugar_eq() {
    // `5 >> { == 5 }` is sugar for `5 >> { in == 5 }` = true
    assert_val("5 >> { == 5 }", T);
}

#[test]
fn probe17_block_sugar_neq() {
    // `5 >> { != 3 }` is sugar for `5 >> { in != 3 }` = true
    assert_val("5 >> { != 3 }", T);
}

#[test]
fn probe17_block_sugar_gt() {
    // `5 >> { > 3 }` is sugar for `5 >> { in > 3 }` = true
    assert_val("5 >> { > 3 }", T);
}

#[test]
fn probe17_block_sugar_lt() {
    // `5 >> { < 10 }` is sugar for `5 >> { in < 10 }` = true
    assert_val("5 >> { < 10 }", T);
}

#[test]
fn probe17_block_sugar_gte() {
    // `5 >> { >= 5 }` is sugar for `5 >> { in >= 5 }` = true
    assert_val("5 >> { >= 5 }", T);
}

#[test]
fn probe17_block_sugar_lte() {
    // `5 >> { <= 5 }` is sugar for `5 >> { in <= 5 }` = true
    assert_val("5 >> { <= 5 }", T);
}

#[test]
fn probe17_block_sugar_array_concat() {
    // `[1, 2] >> { + [3] }` is sugar for `[1, 2] >> { in + [3] }` = [1, 2, 3]
    assert_output("[1, 2] >> { + [3] }", "[1, 2, 3]");
}

#[test]
fn probe17_block_sugar_string_concat() {
    // `"hi" >> { + " world" }` is sugar for `"hi" >> { in + " world" }` = "hi world"
    assert_val(r#""hi" >> { + " world" }"#, s("hi world"));
}

// ── 4. Nested blocks ────────────────────────────────────────────

#[test]
fn probe17_nested_block_inner_in() {
    // `3 >> { in >> { in + 1 } }` — outer block receives 3,
    // pipes it to inner block, inner block's `in` is 3, result is 4
    assert_val("3 >> { in >> { in + 1 } }", int(4));
}

#[test]
fn probe17_nested_block_let_then_use() {
    // `3 >> { in >> let(x); x + 1 }` — bind the inner in to x, then x + 1 = 4
    assert_val("3 >> { in >> let(x); x + 1 }", int(4));
}

#[test]
fn probe17_nested_block_outer_in_rebind() {
    // Outer block's `in` is 10, rebind to `outer`, inner block adds `outer` and `in`
    assert_val("10 >> { in >> let(outer); 5 >> { outer + in } }", int(15));
}

// ── 5. Range ────────────────────────────────────────────────────

#[test]
fn probe17_range_produces_struct() {
    // `1..3` should produce `(start=1, end=3)`
    assert_output("1..3", "(start=1, end=3)");
}

#[test]
fn probe17_range_fields_accessible() {
    // Can access start and end fields from a range, and add them
    assert_val("1..3 >> let(r); r.start + r.end", int(4));
}

#[test]
fn probe17_range_in_parentheses() {
    // `(1..3).start` should give 1
    assert_val("(1..3).start", int(1));
}

// ── 6. Pipe into method calls ───────────────────────────────────

#[test]
fn probe17_method_map_with_block_sugar() {
    // `[1, 2, 3] >> { in.map{ * 2 } }` should give [2, 4, 6]
    assert_output("[1, 2, 3] >> { in.map{ * 2 } }", "[2, 4, 6]");
}

#[test]
fn probe17_method_filter_with_block_sugar() {
    // `[1, 2, 3, 4] >> { in.filter{ > 2 } }` should give [3, 4]
    assert_output("[1, 2, 3, 4] >> { in.filter{ > 2 } }", "[3, 4]");
}

#[test]
fn probe17_method_chained_map_filter() {
    // Map then filter in a chain
    assert_output("[1, 2, 3].map{ * 2 }.filter{ > 3 }", "[4, 6]");
}

// ── 7. Division restriction: a / b * c is syntax error ──────────

#[test]
fn probe17_div_then_mul_syntax_error() {
    // Per spec: `a / b * c` is a syntax error
    assert_parse_error("10 / 2 * 5", "");
}

#[test]
fn probe17_div_then_div_syntax_error() {
    // `a / b / c` should also be a syntax error
    assert_parse_error("10 / 2 / 5", "");
}

#[test]
fn probe17_mul_then_div_ok() {
    // `a * b / c` is valid per spec
    assert_val("12 * 2 / 3", int(8));
}

#[test]
fn probe17_div_with_parens_ok() {
    // Parenthesized division is fine
    assert_val("(10 / 2) * 5", int(25));
}

// ── 8. Unary minus restriction: -a.f() is syntax error ──────────

#[test]
fn probe17_unary_minus_dot_syntax_error() {
    // Per spec: `-a.f()` is a syntax error
    // Using a string variable's .char_len() method
    assert_parse_error(r#"let x = "hello"; -x.char_len()"#, "ambiguous");
}

#[test]
fn probe17_unary_minus_call_syntax_error() {
    // `-f(x)` is a syntax error per spec
    assert_parse_error("let f = { in * 2 }; -f(3)", "ambiguous");
}

#[test]
fn probe17_unary_minus_parenthesized_ok() {
    // `-(expr)` is valid
    assert_val("-(3 + 2)", int(-5));
}

// ── 9. tag(Name) produces unique tags ───────────────────────────

#[test]
fn probe17_tags_are_unique() {
    // Two different tag() calls should produce different tags.
    // Wrapping same value in different tags and branching should distinguish them.
    assert_output(
        "tag(A); tag(B); 1 >> A >> { A(x) -> x + 10, B(x) -> x + 20 }",
        "11",
    );
}

#[test]
fn probe17_tags_are_unique_second_arm() {
    // Same as above but matching the second tag
    assert_output(
        "tag(A); tag(B); 1 >> B >> { A(x) -> x + 10, B(x) -> x + 20 }",
        "21",
    );
}

#[test]
fn probe17_same_name_tags_still_unique() {
    // Even if we shadow a tag name, the old and new are different.
    // tag(X) twice: the second X shadows the first, but values tagged
    // with the second X should match the new X pattern.
    assert_val(
        r#"tag(X); 1 >> X >> let(v1); tag(X); 2 >> X >> let(v2); v2 >> { X(n) -> n * 100 }"#,
        int(200),
    );
}

#[test]
fn probe17_shadowed_tag_old_value_no_match() {
    // After shadowing tag X, the old X-tagged value should NOT match the new X pattern
    assert_error(
        r#"tag(X); 1 >> X >> let(v1); tag(X); v1 >> { X(n) -> n * 100 }"#,
        "no arm matched",
    );
}

// ── 10. Empty block {} is a callable block that returns Unit ────

#[test]
fn probe17_empty_block_returns_unit() {
    // `5 >> {}` should evaluate to `()`
    assert_val("5 >> {}", U);
}

#[test]
fn probe17_empty_block_as_stored_function() {
    // Store empty block in a variable, apply it
    assert_val("{} >> let(f); 42 >> f", U);
}

#[test]
fn probe17_empty_block_ignores_input_string() {
    // Empty block ignores any input type
    assert_val(r#""hello" >> {}"#, U);
}

#[test]
fn probe17_empty_block_ignores_input_array() {
    assert_val("[1, 2, 3] >> {}", U);
}

#[test]
fn probe17_empty_block_ignores_input_struct() {
    assert_val("(a=1, b=2) >> {}", U);
}

// ── Additional edge cases discovered during analysis ────────────

#[test]
fn probe17_let_in_pipe_preserves_input_for_later() {
    // `1 >> let(x); 2 >> let(y); x + y` — both x and y should be accessible
    assert_val("1 >> let(x); 2 >> let(y); x + y", int(3));
}

#[test]
fn probe17_pipe_through_multiple_lets() {
    // Value flows through let, into block, into another let
    assert_val("10 >> let(a) >> { in + 5 } >> let(b); a + b", int(25));
}

#[test]
fn probe17_block_sugar_pipe_into_function() {
    // `{ >> f }` is sugar for `{ in >> f }`
    assert_val("{ in * 3 } >> let(f); 5 >> { >> f }", int(15));
}

#[test]
fn probe17_semicolon_discards_first_value() {
    // Semicolon sequences: first expression is evaluated and discarded
    assert_val("42; 99", int(99));
}

#[test]
fn probe17_let_scope_extends_to_block_end() {
    // Let scope extends to the end of the block, not just to the next semicolon
    assert_val("1 >> let(x); 2 >> let(y); 3 >> let(z); x + y + z", int(6));
}

#[test]
fn probe17_nested_block_in_does_not_leak() {
    // `in` in inner block refers to inner block's input, not outer
    assert_val("10 >> { 20 >> { in } }", int(20));
}

#[test]
fn probe17_block_sugar_with_div() {
    // `10 >> { / 2 }` is sugar for `10 >> { in / 2 }` = 5
    assert_val("10 >> { / 2 }", int(5));
}

#[test]
fn probe17_range_field_names() {
    // Range should have exactly `start` and `end` fields
    assert_val("(0..10).start", int(0));
    assert_val("(0..10).end", int(10));
}

#[test]
fn probe17_block_sugar_dotdot_range() {
    // `{ ..5 }` is sugar for `{ in..5 }` — creates a range from `in` to 5
    assert_output("1 >> { ..5 }", "(start=1, end=5)");
}

#[test]
fn probe17_let_binding_complex_expr() {
    // Let sugar with a complex expression on the right
    assert_val("let x = 3 + 4; x * 2", int(14));
}

#[test]
fn probe17_pipe_into_comparison_block() {
    // Pipe into a block that does comparison and branches
    assert_val("5 >> { in > 3 } >> { true -> 1, false -> 0 }", int(1));
}

#[test]
fn probe17_empty_block_in_chain_produces_unit() {
    // Empty block in the middle of a chain produces unit.
    // Piping unit into { in + 1 } should fail since unit is not a number.
    assert_error("5 >> {} >> { in + 1 }", "");
}

// ── Deeper probes: in-reference correctness after semicolons ────

#[test]
fn probe17_in_after_semicolon_in_block() {
    // Inside a block, after a semicolon, `in` should still refer to the block's input.
    // `5 >> { 99; in }` — 99 is discarded, `in` should still be 5.
    assert_val("5 >> { 99; in }", int(5));
}

#[test]
fn probe17_in_after_let_semicolon_in_block() {
    // Inside a block: `5 >> { let x = 10; in + x }` — `in` should be 5, x is 10
    assert_val("5 >> { let x = 10; in + x }", int(15));
}

#[test]
fn probe17_in_not_corrupted_by_semicolon_sequence() {
    // Top-level: `in` refers to program input which is Unit.
    // After semicolon, `in` should remain as program input.
    // In a block: `7 >> { 1; 2; in }` should give 7.
    assert_val("7 >> { 1; 2; in }", int(7));
}

#[test]
fn probe17_let_sugar_preserves_in_in_block() {
    // `8 >> { let y = in + 1; y * 2 }` — `in` is 8, y is 9, result is 18
    assert_val("8 >> { let y = in + 1; y * 2 }", int(18));
}

// ── Probes: closure captures and scoping ────────────────────────

#[test]
fn probe17_closure_captures_outer_let() {
    // A closure should capture variables from its lexical scope
    assert_val("let x = 10; { in + x } >> let(f); 5 >> f", int(15));
}

#[test]
fn probe17_closure_does_not_see_later_bindings() {
    // A closure captures only what's in scope when it's defined
    // `{ x }` is created before x is bound — should fail with undefined variable
    assert_error("{ x } >> let(f); let x = 10; 5 >> f", "undefined variable");
}

#[test]
fn probe17_shadowed_variable_in_closure() {
    // A closure captures the value of x at the time it's defined
    assert_val(
        "let x = 1; { in + x } >> let(f); let x = 100; 5 >> f",
        int(6),
    );
}

// ── Probes: struct/tuple edge cases ─────────────────────────────

#[test]
fn probe17_single_elem_paren_is_not_tuple() {
    // `(1)` is just the parenthesized expression 1, not a tuple
    assert_val("(1)", int(1));
    assert_val("(1) + 2", int(3));
}

#[test]
fn probe17_unit_equality() {
    // () == () should be true
    assert_val("() == ()", T);
    // () != non-unit should be false for ==, true for !=
    assert_val("() != ()", F);
}

#[test]
fn probe17_struct_field_access_positional() {
    // Positional fields should be accessible by number
    assert_val("(10, 20, 30).0", int(10));
    assert_val("(10, 20, 30).1", int(20));
    assert_val("(10, 20, 30).2", int(30));
}

// ── Probes: method calls on piped values ────────────────────────

#[test]
fn probe17_direct_method_on_piped_array() {
    // Direct method call via pipe chain: build array, get length
    assert_val("[1, 2, 3, 4, 5].len()", int(5));
}

#[test]
fn probe17_method_on_let_bound_value() {
    // Bind an array, then call a method on it
    assert_val("[1, 2, 3] >> let(arr); arr.len()", int(3));
}

#[test]
fn probe17_method_map_with_explicit_in() {
    // Map with explicit `in` reference
    assert_output("[1, 2, 3].map{ in * in }", "[1, 4, 9]");
}

// ── Probes: block sugar with complex right-hand sides ───────────

#[test]
fn probe17_block_sugar_add_with_multiplication() {
    // `{ + 3 * 2 }` should be `{ in + 3 * 2 }` = `{ in + 6 }` (due to precedence)
    assert_val("5 >> { + 3 * 2 }", int(11));
}

#[test]
fn probe17_block_sugar_mul_with_addition() {
    // `{ * 2 + 1 }` should be `{ in * 2 + 1 }` = `{ (in * 2) + 1 }` = 11
    assert_val("5 >> { * 2 + 1 }", int(11));
}

// ── Probes: pipe into various expression types ──────────────────

#[test]
fn probe17_pipe_into_variable_holding_function() {
    // `10 >> f` where f is a function
    assert_val("{ in + 5 } >> let(f); 10 >> f", int(15));
}

#[test]
fn probe17_pipe_into_tag_constructor() {
    // `42 >> MyTag` wraps the value
    assert_val("tag(MyTag); 42 >> MyTag >> { MyTag(x) -> x }", int(42));
}

#[test]
fn probe17_pipe_into_builtin() {
    // `[1, 2, 3].len()` should give 3
    assert_val("[1, 2, 3].len()", int(3));
}

// ── Probes: complex let chain interactions ──────────────────────

#[test]
fn probe17_let_destructure_then_use_both() {
    // Destructure a struct, then use both fields
    assert_val("(a=3, b=7) >> let(a, b); a * b", int(21));
}

#[test]
fn probe17_let_array_destructure_then_use() {
    // Destructure an array, use elements
    assert_val("[10, 20, 30] >> let[a, b, c]; a + b + c", int(60));
}

#[test]
fn probe17_let_chain_array_then_struct() {
    // Array destructure followed by struct construction and access
    assert_val("[1, 2] >> let[a, b]; (x=a, y=b).x + (x=a, y=b).y", int(3));
}

// ── Probes: edge cases in branch matching ───────────────────────

#[test]
fn probe17_branch_int_literal() {
    // Branching on integer literals
    assert_val("2 >> { 1 -> 10, 2 -> 20, 3 -> 30 }", int(20));
}

#[test]
fn probe17_branch_string_literal() {
    // Branching on string literals
    assert_val(
        r#""hello" >> { "hello" -> 1, "world" -> 2, _ -> 0 }"#,
        int(1),
    );
}

#[test]
fn probe17_branch_with_guard_and_in() {
    // Guard can use `in`
    assert_val(
        "tag(N); 5 >> N >> { N(x) if in == N(5) -> 99, N(x) -> 0 }",
        int(99),
    );
}

// ── Probes: division edge cases ─────────────────────────────────

#[test]
fn probe17_div_right_requires_parens_for_complex() {
    // Per spec: `/` requires a single operand on the right,
    // so `a / (b * c)` should work
    assert_val("24 / (2 * 3)", int(4));
}

#[test]
fn probe17_div_in_block_sugar() {
    // `{ / 5 }` is sugar for `{ in / 5 }`
    assert_val("100 >> { / 5 }", int(20));
}

// ── Probes: unary minus edge cases ──────────────────────────────

#[test]
fn probe17_double_negation() {
    // `- -5` — double negation: -((-5)) = 5
    // This depends on how the parser handles it
    assert_val("- -5", int(5));
}

#[test]
fn probe17_negative_in_arithmetic() {
    // Negative values in arithmetic: `3 + -2` = 1
    assert_val("3 + -2", int(1));
}

#[test]
fn probe17_unary_minus_in_block() {
    // Unary minus inside a block (explicit form): `{ (-in) }` or `{ in * -1 }`
    assert_val("5 >> { in * -1 }", int(-5));
}

// ── Probes: string interpolation ────────────────────────────────
// NOTE: String interpolation (`{expr}` inside strings) is specified in DESIGN.md
// but not yet implemented. Unescaped { } are treated as literal characters.
// `\{` and `\}` escapes produce literal braces (this part works).

#[test]
fn probe17_string_interpolation_escaped_brace() {
    // Escaped brace: `\{` should produce literal `{`
    assert_val(r#""\{hello\}""#, s("{hello}"));
}

// ── Probes: range interaction with other operators ──────────────

#[test]
fn probe17_range_precedence_with_addition() {
    // Per the parser, range (..) is tighter than add/sub but looser than mul/div
    // So `1 + 2..3` should be parsed as `1 + (2..3)` which is
    // `1 + (start=2, end=3)` — should error since int + struct is invalid
    assert_error("1 + 2..3", "type error");
}

#[test]
fn probe17_range_with_expressions() {
    // Ranges can use expressions: `(1+1)..(2+3)` = 2..5
    assert_output("(1+1)..(2+3)", "(start=2, end=5)");
}

// ── Probes: `in` in nested let scopes ───────────────────────────

#[test]
fn probe17_in_accessible_after_nested_lets() {
    // `in` should still work deep inside nested let scopes
    assert_val("100 >> { let a = 1; let b = 2; in + a + b }", int(103));
}

#[test]
fn probe17_in_not_modified_by_let() {
    // `in` should not be shadowed by let bindings
    assert_val("42 >> { let x = 99; in }", int(42));
}

// ── Probes: pipe into call with args (f(x) prepending) ──────────

#[test]
fn probe17_pipe_into_function_with_args() {
    // `arr.fold(init, f)` should work
    assert_val("[1, 2, 3].fold(0, { in.acc + in.elem })", int(6));
}

#[test]
fn probe17_pipe_into_fold_with_method_syntax() {
    // `[1, 2, 3].fold(0, { in.acc + in.elem })` should be 6
    assert_val("[1, 2, 3].fold(0, { in.acc + in.elem })", int(6));
}

// ── Probes: trailing semicolon behavior ─────────────────────────

#[test]
fn probe17_trailing_semicolon_gives_unit() {
    // Per implementation: trailing semicolon means the expression
    // is followed by Unit
    assert_val("42;", U);
}

#[test]
fn probe17_trailing_semicolon_in_block() {
    // Trailing semicolon inside a block
    assert_val("5 >> { 42; }", U);
}

// ── Probes: complex pipe + let + block chains ───────────────────

#[test]
fn probe17_pipe_let_pipe_block_pipe_let() {
    // Complex chain: 1 >> let(a) >> { in + 1 } >> let(b) >> { in + 1 } >> let(c); a + b + c
    // a=1, b=2, c=3; result=6
    assert_val(
        "1 >> let(a) >> { in + 1 } >> let(b) >> { in + 1 } >> let(c); a + b + c",
        int(6),
    );
}

#[test]
fn probe17_let_sugar_multiple_with_complex_expr() {
    // `let x = 1 + 2; let y = x * 3; y + 1` = let x=3; let y=9; 9+1 = 10
    assert_val("let x = 1 + 2; let y = x * 3; y + 1", int(10));
}

// ═══════════════════════════════════════════════════════════════════
// ROUND 17b — probing branching blocks and pattern matching edge cases
// ═══════════════════════════════════════════════════════════════════

// ── 1. Branching on integers ────────────────────────────────────────

#[test]
fn probe17b_branch_int_match_first() {
    assert_val(r#"1 >> { 1 -> "one", 2 -> "two", _ -> "other" }"#, s("one"));
}

#[test]
fn probe17b_branch_int_match_second() {
    assert_val(r#"2 >> { 1 -> "one", 2 -> "two", _ -> "other" }"#, s("two"));
}

#[test]
fn probe17b_branch_int_match_wildcard() {
    assert_val(
        r#"3 >> { 1 -> "one", 2 -> "two", _ -> "other" }"#,
        s("other"),
    );
}

#[test]
fn probe17b_branch_int_zero() {
    assert_val(r#"0 >> { 0 -> "zero", _ -> "nonzero" }"#, s("zero"));
}

#[test]
fn probe17b_branch_int_large() {
    // Test with large int values to ensure comparison works
    assert_val("999999 >> { 999999 -> 1, _ -> 0 }", int(1));
}

// ── 2. Branching on strings ─────────────────────────────────────────

#[test]
fn probe17b_branch_string_match() {
    assert_val(
        r#""hello" >> { "hello" -> 1, "world" -> 2, _ -> 0 }"#,
        int(1),
    );
}

#[test]
fn probe17b_branch_string_match_second() {
    assert_val(
        r#""world" >> { "hello" -> 1, "world" -> 2, _ -> 0 }"#,
        int(2),
    );
}

#[test]
fn probe17b_branch_string_no_match() {
    assert_val(r#""foo" >> { "hello" -> 1, "world" -> 2, _ -> 0 }"#, int(0));
}

#[test]
fn probe17b_branch_string_empty() {
    // Empty string as pattern
    assert_val(r#""" >> { "" -> "empty", _ -> "nonempty" }"#, s("empty"));
}

// ── 3. Branching on chars ───────────────────────────────────────────

#[test]
fn probe17b_branch_char_match() {
    assert_val("'a' >> { 'a' -> 1, 'b' -> 2, _ -> 0 }", int(1));
}

#[test]
fn probe17b_branch_char_match_second() {
    assert_val("'b' >> { 'a' -> 1, 'b' -> 2, _ -> 0 }", int(2));
}

#[test]
fn probe17b_branch_char_no_match() {
    assert_val("'z' >> { 'a' -> 1, 'b' -> 2, _ -> 0 }", int(0));
}

// ── 4. Branching on bytes ───────────────────────────────────────────

#[test]
fn probe17b_branch_byte_match() {
    assert_val("b'x' >> { b'x' -> 1, b'y' -> 2, _ -> 0 }", int(1));
}

#[test]
fn probe17b_branch_byte_match_second() {
    assert_val("b'y' >> { b'x' -> 1, b'y' -> 2, _ -> 0 }", int(2));
}

#[test]
fn probe17b_branch_byte_no_match() {
    assert_val("b'z' >> { b'x' -> 1, b'y' -> 2, _ -> 0 }", int(0));
}

// ── 5. Guard clauses ────────────────────────────────────────────────

#[test]
fn probe17b_guard_true_path() {
    assert_val(r#"5 >> { x if x > 3 -> "big", x -> "small" }"#, s("big"));
}

#[test]
fn probe17b_guard_false_path() {
    assert_val(r#"1 >> { x if x > 3 -> "big", x -> "small" }"#, s("small"));
}

#[test]
fn probe17b_guard_boundary_value() {
    // x == 3 is NOT > 3, so should fall through
    assert_val(r#"3 >> { x if x > 3 -> "big", x -> "small" }"#, s("small"));
}

#[test]
fn probe17b_guard_with_equality() {
    // Guard using == instead of >
    assert_val(
        r#"42 >> { x if x == 42 -> "found", _ -> "not found" }"#,
        s("found"),
    );
}

#[test]
fn probe17b_guard_multiple_arms() {
    // Multiple guarded arms in sequence
    assert_val(
        r#"5 >> { x if x > 10 -> "big", x if x > 3 -> "medium", x -> "small" }"#,
        s("medium"),
    );
}

#[test]
fn probe17b_guard_non_boolean_error() {
    // Guard that returns non-boolean should error
    assert_error("5 >> { x if x + 1 -> x }", "guard must be boolean");
}

// ── 6. Branching on Unit ────────────────────────────────────────────

#[test]
fn probe17b_branch_unit_match() {
    assert_val(r#"() >> { () -> "unit", _ -> "other" }"#, s("unit"));
}

#[test]
fn probe17b_branch_unit_no_match() {
    assert_val(r#"42 >> { () -> "unit", _ -> "other" }"#, s("other"));
}

// ── 7. Negative literal patterns ────────────────────────────────────

#[test]
fn probe17b_branch_neg_int_match() {
    assert_val(
        r#"-1 >> { -1 -> "neg one", 0 -> "zero", _ -> "other" }"#,
        s("neg one"),
    );
}

#[test]
fn probe17b_branch_neg_int_zero() {
    assert_val(
        r#"0 >> { -1 -> "neg one", 0 -> "zero", _ -> "other" }"#,
        s("zero"),
    );
}

#[test]
fn probe17b_branch_neg_int_positive() {
    assert_val(
        r#"1 >> { -1 -> "neg one", 0 -> "zero", _ -> "other" }"#,
        s("other"),
    );
}

#[test]
fn probe17b_branch_neg_float_match() {
    assert_val(
        r#"-3.14 >> { -3.14 -> "neg pi", _ -> "other" }"#,
        s("neg pi"),
    );
}

// ── 8. Tag patterns with guards ─────────────────────────────────────

#[test]
fn probe17b_tag_guard_match() {
    assert_val(
        r#"tag(Ok); tag(Err); Ok(5) >> { Ok(x) if x > 3 -> "big ok", Ok(x) -> "small ok", Err(_) -> "error" }"#,
        s("big ok"),
    );
}

#[test]
fn probe17b_tag_guard_fallthrough() {
    assert_val(
        r#"tag(Ok); tag(Err); Ok(1) >> { Ok(x) if x > 3 -> "big ok", Ok(x) -> "small ok", Err(_) -> "error" }"#,
        s("small ok"),
    );
}

#[test]
fn probe17b_tag_guard_err_arm() {
    assert_val(
        r#"tag(Ok); tag(Err); Err("fail") >> { Ok(x) if x > 3 -> "big ok", Ok(x) -> "small ok", Err(_) -> "error" }"#,
        s("error"),
    );
}

// ── 9. Non-exhaustive match should error ────────────────────────────

#[test]
fn probe17b_nonexhaustive_int() {
    // Matching 1 against only a 0 pattern should error
    assert_error(r#"1 >> { 0 -> "zero" }"#, "no arm matched");
}

#[test]
fn probe17b_nonexhaustive_string() {
    // Matching "foo" against "bar" with no wildcard
    assert_error(r#""foo" >> { "bar" -> "found" }"#, "no arm matched");
}

#[test]
fn probe17b_nonexhaustive_bool() {
    // Only matching true — false has no arm
    assert_error(r#"false >> { true -> "yes" }"#, "no arm matched");
}

// ── 10. Tag no-payload pattern ──────────────────────────────────────

#[test]
fn probe17b_tag_no_payload_match() {
    // None(()) creates tagged unit; branch pattern `None` matches tagged unit
    assert_val(
        r#"tag(None); tag(Some); None() >> { None -> "nothing", Some(x) -> "something" }"#,
        s("nothing"),
    );
}

#[test]
fn probe17b_tag_no_payload_some_arm() {
    assert_val(
        r#"tag(None); tag(Some); Some(42) >> { None -> "nothing", Some(x) -> "something" }"#,
        s("something"),
    );
}

#[test]
fn probe17b_tag_no_payload_pipe_syntax() {
    // Using () >> None instead of None()
    assert_val(
        r#"tag(None); tag(Some); () >> None >> { None -> "nothing", Some(x) -> "something" }"#,
        s("nothing"),
    );
}

// ── 11. Nested branching ────────────────────────────────────────────

#[test]
fn probe17b_nested_branch_ok_big() {
    assert_val(
        r#"tag(Ok); tag(Err); Ok(5) >> { Ok(x) -> (x > 3 >> { true -> "big", false -> "small" }), Err(_) -> "err" }"#,
        s("big"),
    );
}

#[test]
fn probe17b_nested_branch_ok_small() {
    assert_val(
        r#"tag(Ok); tag(Err); Ok(1) >> { Ok(x) -> (x > 3 >> { true -> "big", false -> "small" }), Err(_) -> "err" }"#,
        s("small"),
    );
}

#[test]
fn probe17b_nested_branch_err() {
    assert_val(
        r#"tag(Ok); tag(Err); Err("oops") >> { Ok(x) -> (x > 3 >> { true -> "big", false -> "small" }), Err(_) -> "err" }"#,
        s("err"),
    );
}

#[test]
fn probe17b_nested_branch_double() {
    // Double-nested: branch inside branch inside branch
    assert_val(
        r#"true >> { true -> (1 >> { 1 -> "one", _ -> "other" }), false -> "no" }"#,
        s("one"),
    );
}

// ── 12. Float literal patterns ──────────────────────────────────────

#[test]
fn probe17b_branch_float_match() {
    assert_val(r#"3.14 >> { 3.14 -> "pi", _ -> "other" }"#, s("pi"));
}

#[test]
fn probe17b_branch_float_no_match() {
    assert_val(r#"2.71 >> { 3.14 -> "pi", _ -> "other" }"#, s("other"));
}

#[test]
fn probe17b_branch_float_zero() {
    assert_val(r#"0.0 >> { 0.0 -> "zero", _ -> "nonzero" }"#, s("zero"));
}

// ── 13. Branching block as standalone lambda ────────────────────────

#[test]
fn probe17b_branch_as_lambda() {
    assert_val(
        r#"let f = { 1 -> "one", 2 -> "two", _ -> "other" }; 2 >> f"#,
        s("two"),
    );
}

#[test]
fn probe17b_branch_as_lambda_wildcard() {
    assert_val(
        r#"let f = { 1 -> "one", 2 -> "two", _ -> "other" }; 99 >> f"#,
        s("other"),
    );
}

#[test]
fn probe17b_branch_as_lambda_call_syntax() {
    // Use f(value) call syntax with a branching lambda
    assert_val(
        r#"let f = { 1 -> "one", 2 -> "two", _ -> "other" }; f(1)"#,
        s("one"),
    );
}

// ── 14. Additional edge cases: binding name shadows tag ─────────────

#[test]
fn probe17b_binding_vs_tag_resolution() {
    // If a name in a branch pattern matches a tag constructor in scope,
    // it should match as a tag pattern (no-payload), not as a binding.
    // This test verifies that `Done` in the pattern matches the tag, not
    // as a catch-all binding — so 42 (untagged) won't match.
    assert_error(r#"tag(Done); 42 >> { Done -> "done" }"#, "no arm matched");
}

#[test]
fn probe17b_binding_name_not_tag_is_catchall() {
    // An identifier that is NOT a tag in scope should be a catch-all binding
    assert_val("42 >> { x -> x + 1 }", int(43));
}

// ── 15. Guard with tag pattern binding value usage ──────────────────

#[test]
fn probe17b_tag_guard_uses_extracted_binding() {
    // Guard references the binding extracted from the tag
    assert_val(
        "tag(N); 10 >> N >> { N(x) if x == 10 -> x * 2, N(x) -> x }",
        int(20),
    );
}

#[test]
fn probe17b_tag_guard_binding_falls_through() {
    assert_val(
        "tag(N); 5 >> N >> { N(x) if x == 10 -> x * 2, N(x) -> x }",
        int(5),
    );
}

// ── 16. Branch arm body uses `in` ───────────────────────────────────

#[test]
fn probe17b_branch_body_uses_in() {
    // `in` in a branch arm body refers to the branching block's input
    assert_val("42 >> { _ -> in + 1 }", int(43));
}

#[test]
fn probe17b_branch_body_in_with_binding() {
    // Both the binding and `in` should be usable
    assert_val("42 >> { x -> x + in }", int(84));
}

// ── 17. Multiple literal types in same branch block ─────────────────

#[test]
fn probe17b_mixed_literal_types_in_branch() {
    // Integer and string patterns in the same branch block
    // The integer pattern should not match a string input; fall through to wildcard
    assert_val(
        r#""hello" >> { 42 -> "int", "hello" -> "str", _ -> "other" }"#,
        s("str"),
    );
}

#[test]
fn probe17b_mixed_literal_bool_and_int() {
    // Bool and int patterns; a boolean should not match an int literal
    assert_val(
        r#"true >> { 1 -> "one", true -> "yes", _ -> "other" }"#,
        s("yes"),
    );
}

// ── 18. Branch with tag extraction and computation ──────────────────

#[test]
fn probe17b_tag_extract_and_compute() {
    assert_val("tag(W); W(10) >> { W(x) -> x * x + 1 }", int(101));
}

// ── 19. Wildcard-only branch ────────────────────────────────────────

#[test]
fn probe17b_wildcard_only_branch() {
    // A branch with only a wildcard pattern — always matches
    assert_val(r#"42 >> { _ -> "always" }"#, s("always"));
}

#[test]
fn probe17b_binding_only_branch() {
    // A branch with only a binding pattern — always matches and binds
    assert_val("42 >> { x -> x + 1 }", int(43));
}

// ── 20. Chained pipe into branching ─────────────────────────────────

#[test]
fn probe17b_chained_pipe_branch() {
    // Result of first branch piped into second branch
    assert_val(
        "1 >> { 1 -> 10, _ -> 0 } >> { 10 -> 100, _ -> 0 }",
        int(100),
    );
}

#[test]
fn probe17b_three_chained_branches() {
    // Triple-chained branching
    assert_val(
        r#"true >> { true -> 1, false -> 0 } >> { 1 -> "one", 0 -> "zero", _ -> "?" } >> { "one" -> 100, _ -> 0 }"#,
        int(100),
    );
}

// ── 21. Branch inside let binding ───────────────────────────────────

#[test]
fn probe17b_branch_result_in_let() {
    assert_val(
        r#"let result = 1 >> { 1 -> "one", _ -> "other" }; result"#,
        s("one"),
    );
}

// ── 22. Tag pattern with discard binding ────────────────────────────

#[test]
fn probe17b_tag_pattern_discard_binding() {
    // Ok(_) discards the payload but still matches the tag
    assert_val(
        r#"tag(Ok); tag(Err); Ok(42) >> { Ok(_) -> "ok", Err(_) -> "err" }"#,
        s("ok"),
    );
}

// ── 23. Tag pattern with empty parens ───────────────────────────────

#[test]
fn probe17b_tag_pattern_empty_parens() {
    // Tag() in branch pattern — parens with no binding
    assert_val("tag(W); W(42) >> { W() -> 1, _ -> 0 }", int(1));
}

// ── 24. Int/float cross-matching in branch patterns ─────────────────

#[test]
fn probe17b_int_vs_float_pattern() {
    // An int value against a float pattern — types don't match, falls through to _
    assert_val(
        r#"1 >> { 1.0 -> "float match", _ -> "no match" }"#,
        s("no match"),
    );
}

#[test]
fn probe17b_float_vs_int_pattern() {
    // A float value against an int pattern — types don't match, falls through to _
    assert_val(
        r#"1.0 >> { 1 -> "int match", _ -> "no match" }"#,
        s("no match"),
    );
}

// ── 25. Branch with expression in body requiring parens ─────────────

#[test]
fn probe17b_branch_body_pipe() {
    // Branch body that itself does a pipe
    assert_val("5 >> { x -> x >> { in * 2 } }", int(10));
}

// ── 26. Guard that references outer scope ───────────────────────────

#[test]
fn probe17b_guard_references_outer_scope() {
    // Guard references a variable from outer scope
    assert_val(
        r#"let threshold = 3; 5 >> { x if x > threshold -> "big", x -> "small" }"#,
        s("big"),
    );
}

#[test]
fn probe17b_guard_references_outer_scope_false() {
    assert_val(
        r#"let threshold = 10; 5 >> { x if x > threshold -> "big", x -> "small" }"#,
        s("small"),
    );
}

// ── 27. Bool literal pattern with non-boolean input ─────────────────

#[test]
fn probe17b_bool_pattern_with_int_input() {
    // Piping an int into a branch with bool patterns — should fall through to wildcard
    assert_val(
        r#"42 >> { true -> "yes", false -> "no", _ -> "neither" }"#,
        s("neither"),
    );
}

// ── 28. Multiple matching arms — first wins ─────────────────────────

#[test]
fn probe17b_first_match_wins() {
    // Two identical patterns: the first one should win
    assert_val("1 >> { 1 -> 100, 1 -> 200, _ -> 0 }", int(100));
}

// ── 29. Branching block captures closure environment ────────────────

#[test]
fn probe17b_branch_closure_captures() {
    // A branching lambda defined inside a let-scope should capture the scope
    assert_val(
        r#"let multiplier = 10; let f = { 1 -> multiplier, _ -> 0 }; 1 >> f"#,
        int(10),
    );
}

// ── 30. Nested tag matching ─────────────────────────────────────────

#[test]
fn probe17b_nested_tag_matching() {
    // Match on tag, extract value, then match on another tag
    assert_val(
        "tag(Ok); tag(Err); tag(W); Ok(W(5)) >> { Ok(inner) -> (inner >> { W(x) -> x * 2 }), Err(_) -> 0 }",
        int(10),
    );
}

// ── 31. Branching block with all literal arms, no wildcard ──────────

#[test]
fn probe17b_all_bool_arms_no_wildcard() {
    // All boolean arms covered — should work
    assert_val("true >> { true -> 1, false -> 0 }", int(1));
    assert_val("false >> { true -> 1, false -> 0 }", int(0));
}

// ── 32. Tag pattern where tag name is not in scope ──────────────────

#[test]
fn probe17b_undefined_tag_in_pattern_is_binding() {
    // Using a tag name in a pattern that hasn't been defined
    // The parser stores it as a Binding, so `Foo` becomes a catch-all binding
    // when the name is not a tag in scope.
    assert_val(r#"42 >> { Foo -> "matched" }"#, s("matched"));
}

#[test]
fn probe17b_undefined_tag_pattern_with_parens() {
    // Using Tag(x) pattern with undefined tag should error at eval time
    assert_error("42 >> { Foo(x) -> x }", "undefined tag");
}

// ── 33. Trailing comma in branch arms ───────────────────────────────

#[test]
fn probe17b_trailing_comma_in_branch() {
    // Trailing comma after last arm
    assert_val(r#"1 >> { 1 -> "one", _ -> "other", }"#, s("one"));
}

// ── 34. Single-arm branch ───────────────────────────────────────────

#[test]
fn probe17b_single_arm_branch_match() {
    assert_val("42 >> { 42 -> 1 }", int(1));
}

#[test]
fn probe17b_single_arm_branch_no_match() {
    assert_error("42 >> { 0 -> 1 }", "no arm matched");
}

// ── 35. Guard with complex expression ───────────────────────────────

#[test]
fn probe17b_guard_with_complex_expr() {
    // Guard with `and` function for logical AND
    assert_val(
        r#"10 >> { x if and(x > 5, x < 20) -> "in range", _ -> "out of range" }"#,
        s("in range"),
    );
}

// ── 36. Branching on tagged values with same-name tags ──────────────

#[test]
fn probe17b_same_name_tags_different_scopes() {
    // Two different `tag(A)` calls create distinct tag identities.
    // The second `tag(A)` shadows the first binding.
    // Matching should use the currently-in-scope tag identity.
    assert_val(
        "tag(A); let a1 = A; tag(A); let a2 = A; 42 >> a2 >> { A(x) -> x }",
        int(42),
    );
}

#[test]
fn probe17b_tag_identity_mismatch() {
    // a1 and a2 are different tag identities despite having the same name.
    // Value tagged with a1 should NOT match a branch pattern using a2's identity
    // (since the second tag(A) shadows A in the branch scope).
    assert_error(
        "tag(A); let a1 = A; tag(A); 42 >> a1 >> { A(x) -> x }",
        "no arm matched",
    );
}

// ── 37. Pipe result of branch directly into another function ────────

#[test]
fn probe17b_branch_result_pipe_into_function() {
    assert_output("tag(W); 1 >> { 1 -> 42, _ -> 0 } >> W", "W(42)");
}

// ── 38. Hex integer as branch pattern ───────────────────────────────

#[test]
fn probe17b_hex_int_branch_pattern() {
    // 0xFF == 255 in pattern position
    assert_val("255 >> { 0xFF -> 1, _ -> 0 }", int(1));
}

// ── 39. Deeply nested branching (stress test) ───────────────────────

#[test]
fn probe17b_deeply_nested_branch() {
    assert_val(
        r#"1 >> { 1 -> (2 >> { 2 -> (3 >> { 3 -> "deep", _ -> "no" }), _ -> "no" }), _ -> "no" }"#,
        s("deep"),
    );
}

// ── 40. Branch pattern with char escape ─────────────────────────────

#[test]
fn probe17b_branch_char_escape() {
    // '\n' as a branch pattern
    assert_val(r"'\n' >> { '\n' -> 1, _ -> 0 }", int(1));
}

// ── 41. Branch on byte zero ─────────────────────────────────────────

#[test]
fn probe17b_branch_byte_zero() {
    assert_val(r"b'\0' >> { b'\0' -> 1, _ -> 0 }", int(1));
}

// ── 42. Guard with `not` function ───────────────────────────────────

#[test]
fn probe17b_guard_with_not() {
    assert_val(
        r#"5 >> { x if not(x == 3) -> "not three", _ -> "three" }"#,
        s("not three"),
    );
}

// ── 43. Branching block applied via method-style pipe ───────────────

#[test]
fn probe17b_branch_via_let_and_pipe() {
    // Store branching block in a variable, use it in a pipe chain
    assert_val(
        r#"let classify = { x if x > 0 -> "positive", x if x < 0 -> "negative", _ -> "zero" }; 0 - 5 >> classify"#,
        s("negative"),
    );
}

// ── 44. Branch matching on tag with struct payload ──────────────────

#[test]
fn probe17b_tag_with_struct_payload() {
    // Branch on a tagged value; the tag has a struct payload
    assert_val("tag(P); P((x=1, y=2)) >> { P(p) -> p.x + p.y }", int(3));
}

// ── 45. Guard that uses `in` ────────────────────────────────────────

#[test]
fn probe17b_guard_uses_in() {
    // The guard should also have access to `in` (the branching block's input)
    assert_val(r#"42 >> { x if in > 40 -> "big", _ -> "small" }"#, s("big"));
}

// ── 46. Branch with array payload in tag ────────────────────────────

#[test]
fn probe17b_tag_with_array_payload() {
    assert_val("tag(V); V([1, 2, 3]) >> { V(arr) -> arr.len() }", int(3));
}

// ── 47. Multiple wildcard/binding arms — first matching wins ────────

#[test]
fn probe17b_multiple_wildcards_first_wins() {
    // If there are multiple catch-all arms, the first one should win
    assert_val("42 >> { _ -> 1, _ -> 2, _ -> 3 }", int(1));
}

// ── 48. Branch on negative zero float ───────────────────────────────

#[test]
fn probe17b_branch_negative_zero_float() {
    // In IEEE 754, -0.0 == 0.0 — should match
    assert_val(
        r#"0.0 >> { -0.0 -> "neg zero match", _ -> "no" }"#,
        s("neg zero match"),
    );
}

// ── 49. Tag guard where guard references payload name from prior failed arm ──

#[test]
fn probe17b_guard_binding_not_leaking() {
    // When Ok(x) if x > 10 fails the guard, the binding x should NOT
    // leak into subsequent arms. The next arm re-binds x.
    assert_val(
        "tag(Ok); Ok(5) >> { Ok(x) if x > 10 -> x * 100, Ok(x) -> x + 1 }",
        int(6),
    );
}

// ── 50. Branch lambda stored and called multiple times ──────────────

#[test]
fn probe17b_branch_lambda_reuse() {
    // A branching block stored as a lambda should work correctly when called multiple times
    assert_val(
        r#"let f = { 1 -> "one", 2 -> "two", _ -> "other" }; (1 >> f) + " " + (2 >> f) + " " + (3 >> f)"#,
        s("one two other"),
    );
}

// ═══════════════════════════════════════════════════════════════════
// ROUND 17b — deeper edge cases: guards on literal patterns
// These test combinations where is_branch_block_start may fail to
// recognize the block as a branching block, causing parse errors.
// ═══════════════════════════════════════════════════════════════════

// ── 51. Int literal pattern with guard ──────────────────────────────

#[test]
fn probe17b_int_literal_with_guard() {
    // { 0 if cond -> body, ... } — is_branch_block_start sees Int, then
    // checks for Arrow, but here the next token after Int is If, not Arrow.
    // BUG: is_branch_block_start doesn't handle Int followed by If.
    assert_val(r#"0 >> { 0 if true -> "zero", _ -> "other" }"#, s("zero"));
}

// ── 52. String literal pattern with guard ───────────────────────────

#[test]
fn probe17b_string_literal_with_guard() {
    // { "hello" if cond -> body, ... }
    // BUG: is_branch_block_start doesn't handle Str followed by If.
    assert_val(
        r#""hello" >> { "hello" if true -> "yes", _ -> "no" }"#,
        s("yes"),
    );
}

// ── 53. Bool literal pattern with guard ─────────────────────────────

#[test]
fn probe17b_bool_literal_with_guard() {
    // { true if cond -> body, ... }
    // BUG: is_branch_block_start for True/False only checks Arrow, not If.
    assert_val(r#"true >> { true if true -> "yes", _ -> "no" }"#, s("yes"));
}

// ── 54. Char literal pattern with guard ─────────────────────────────

#[test]
fn probe17b_char_literal_with_guard() {
    // { 'a' if cond -> body, ... }
    // BUG: is_branch_block_start doesn't handle Char followed by If.
    assert_val(r#"'a' >> { 'a' if true -> "yes", _ -> "no" }"#, s("yes"));
}

// ── 55. Byte literal pattern with guard ─────────────────────────────

#[test]
fn probe17b_byte_literal_with_guard() {
    // { b'x' if cond -> body, ... }
    // BUG: is_branch_block_start doesn't handle Byte followed by If.
    assert_val(r#"b'x' >> { b'x' if true -> "yes", _ -> "no" }"#, s("yes"));
}

// ── 56. Negative literal pattern with guard ─────────────────────────

#[test]
fn probe17b_neg_literal_with_guard() {
    // { -1 if cond -> body, ... }
    // BUG: is_branch_block_start for Minus only checks Arrow after -N, not If.
    // Additionally, `-` at block start triggers the ambiguity check, so this
    // hits the ambiguity error before even reaching the guard detection issue.
    // Two bugs combine: (1) `-` at start triggers ambiguity, (2) even if it
    // didn't, the `if` after `-N` wouldn't be recognized.
    assert_val(r#"-1 >> { -1 if true -> "neg", _ -> "no" }"#, s("neg"));
}

// ── 57. Unit literal pattern with guard ─────────────────────────────

#[test]
fn probe17b_unit_literal_with_guard() {
    // { () if cond -> body, ... }
    // BUG: is_branch_block_start for LParen only checks Arrow after (), not If.
    assert_val(r#"() >> { () if true -> "unit", _ -> "no" }"#, s("unit"));
}

// ── 58. Underscore pattern with guard ───────────────────────────────

#[test]
fn probe17b_underscore_with_guard() {
    // { _ if cond -> body, ... }
    // BUG: is_branch_block_start for Underscore only checks Arrow, not If.
    assert_val(
        r#"42 >> { _ if true -> "matched", _ -> "fallback" }"#,
        s("matched"),
    );
}

#[test]
fn probe17b_underscore_with_guard_false() {
    // Wildcard with guard that evaluates to false — should fall through
    assert_val(
        r#"42 >> { _ if false -> "no", _ -> "fallback" }"#,
        s("fallback"),
    );
}

// ── 59. Float literal pattern with guard ────────────────────────────

#[test]
fn probe17b_float_literal_with_guard() {
    // { 3.14 if cond -> body, ... }
    // BUG: is_branch_block_start doesn't handle Float followed by If.
    assert_val(r#"3.14 >> { 3.14 if true -> "pi", _ -> "no" }"#, s("pi"));
}

// ── 60. Neg float literal with guard ────────────────────────────────

#[test]
fn probe17b_neg_float_literal_with_guard() {
    // { -3.14 if cond -> body, ... }
    assert_val(
        r#"-3.14 >> { -3.14 if true -> "neg pi", _ -> "no" }"#,
        s("neg pi"),
    );
}

// ── 61. Literal guard with meaningful condition ─────────────────────

#[test]
fn probe17b_int_guard_meaningful_condition() {
    // The guard uses the `in` keyword to make the match conditional
    // This is different from just `x if x > 3` — here the pattern is a literal
    // and the guard provides extra check on `in`
    assert_val(
        r#"5 >> { 5 if in > 3 -> "big five", 5 -> "small five", _ -> "other" }"#,
        s("big five"),
    );
}

// ── 62. Tag pattern with guard — `if` comes right after Tag(x) ─────

#[test]
fn probe17b_tag_pattern_guard_detailed() {
    // Make sure is_branch_block_start correctly identifies Tag(x) if guard -> ...
    // The code skips parens then checks for If — this should work.
    assert_val(
        "tag(R); R(42) >> { R(x) if x > 10 -> x, R(x) -> 0 }",
        int(42),
    );
}

// ── 63. Struct as branch scrutinee with binding ─────────────────────

#[test]
fn probe17b_struct_as_scrutinee_binding() {
    // Branch on a struct — the binding gets the whole struct
    assert_val("(1, 2) >> { s -> s.0 + s.1 }", int(3));
}

// ── 64. Array as branch scrutinee with binding ──────────────────────

#[test]
fn probe17b_array_as_scrutinee_binding() {
    // Branch on an array — the binding gets the whole array
    assert_val("[1, 2, 3] >> { arr -> arr.len() }", int(3));
}

// ── 65. Branching where first arm is a guard-only binding ───────────

#[test]
fn probe17b_guard_only_no_literal_first_arm() {
    // First arm is `x if cond -> body`, which is a binding with guard
    // The `Ident` followed by `if` path in is_branch_block_start should detect this
    assert_val(
        r#"5 >> { x if x == 5 -> "five", x -> "not five" }"#,
        s("five"),
    );
}

// ── 66. Branching on function value should not match literal ────────

#[test]
fn probe17b_function_as_scrutinee() {
    // Piping a function into a branch — literal patterns won't match,
    // but a catch-all binding should
    assert_val(
        r#"{ in + 1 } >> { _ -> "got a function" }"#,
        s("got a function"),
    );
}

// ── 67. Empty branch block (no arms) ────────────────────────────────

#[test]
fn probe17b_empty_branch_block_is_expression_block() {
    // {} is an empty expression block, not a branch block.
    // It should return () when applied.
    assert_val("42 >> {}", U);
}

// ── 68. Branch with piped tag constructor as scrutinee ───────────────

#[test]
fn probe17b_piped_tag_constructor_branch() {
    // 42 >> Ok produces Ok(42), then match on it
    assert_val(
        "tag(Ok); tag(Err); 42 >> Ok >> { Ok(x) -> x + 1, Err(_) -> 0 }",
        int(43),
    );
}

// ── 69. Wildcard guard that evaluates expression with side effect ───

#[test]
fn probe17b_guard_evaluates_expression() {
    // Guard evaluates `in > 10`; _ always matches but guard controls flow
    assert_val(
        r#"5 >> { _ if in > 10 -> "big", _ -> "small" }"#,
        s("small"),
    );
}

// ── 70. Branching with let sugar storing result ─────────────────────

#[test]
fn probe17b_let_sugar_branch_result() {
    assert_val(
        r#"let x = "hello" >> { "hello" -> 1, _ -> 0 }; x + 10"#,
        int(11),
    );
}

// ── 71. Negative literal matching against positive scrutinee ────────

#[test]
fn probe17b_neg_pattern_pos_scrutinee() {
    // -1 pattern should NOT match positive 1
    assert_val(r#"1 >> { -1 -> "neg", _ -> "other" }"#, s("other"));
}

// ── 72. Multiple tag constructors, complex matching ─────────────────

#[test]
fn probe17b_three_tag_matching() {
    assert_val(
        r#"tag(A); tag(B); tag(C); B(42) >> { A(x) -> "a", B(x) -> "b", C(x) -> "c", _ -> "?" }"#,
        s("b"),
    );
}

// ── 73. Branching block inside map ──────────────────────────────────

#[test]
fn probe17b_branch_inside_map() {
    // Use a branching block as the function in map
    assert_output(
        r#"[1, 2, 3].map({ 1 -> "one", 2 -> "two", _ -> "other" })"#,
        r#"["one", "two", "other"]"#,
    );
}

// ── 74. Branching block inside filter ───────────────────────────────

#[test]
fn probe17b_branch_inside_filter() {
    // Use a branching block as the function in filter
    assert_output(
        "[1, 2, 3, 4].filter({ x if x > 2 -> true, _ -> false })",
        "[3, 4]",
    );
}

// ── 75. Guarded wildcard followed by unguarded wildcard ─────────────

#[test]
fn probe17b_guarded_wildcard_then_unguarded() {
    // Pattern: _ if cond -> body1, _ -> body2
    // When guard is true, first arm wins; when false, second arm wins
    assert_val(r#"5 >> { _ if in > 3 -> "big", _ -> "small" }"#, s("big"));
    assert_val(r#"1 >> { _ if in > 3 -> "big", _ -> "small" }"#, s("small"));
}

// ═══════════════════════════════════════════════════════════════════
// probe18b — Methods and builtins bug hunt
// ═══════════════════════════════════════════════════════════════════

// ── 1. Array methods ─────────────────────────────────────────────

#[test]
fn probe18b_array_get_empty() {
    assert_error("[].get(0)", "out of bounds");
}

#[test]
fn probe18b_array_get_negative() {
    assert_error("[1, 2, 3].get(-1)", "negative");
}

#[test]
fn probe18b_array_get_out_of_bounds() {
    assert_error("[1, 2, 3].get(100)", "out of bounds");
}

#[test]
fn probe18b_array_slice_basic() {
    assert_output("[1, 2, 3].slice(0..2)", "[1, 2]");
}

#[test]
fn probe18b_array_slice_empty() {
    assert_output("[1, 2, 3].slice(0..0)", "[]");
}

#[test]
fn probe18b_array_slice_out_of_bounds() {
    assert_error("[1, 2, 3].slice(2..5)", "out of bounds");
}

#[test]
fn probe18b_array_len_empty() {
    assert_val("[].len()", int(0));
}

#[test]
fn probe18b_array_map_empty() {
    assert_output("[].map{ * 2 }", "[]");
}

#[test]
fn probe18b_array_filter_empty() {
    assert_output("[].filter{ > 0 }", "[]");
}

#[test]
fn probe18b_array_fold_empty() {
    assert_val("[].fold(0, { in.acc + in.elem })", int(0));
}

#[test]
fn probe18b_array_zip_different_lengths() {
    assert_output("[1, 2, 3].zip([4, 5])", "[(1, 4), (2, 5)]");
}

#[test]
fn probe18b_array_chained_methods() {
    assert_output("[1, 2, 3].map{ * 2 }.filter{ > 3 }", "[4, 6]");
}

// ── 2. Builtin functions via pipe ────────────────────────────────

#[test]
fn probe18b_pipe_len_array() {
    assert_val("[1, 2, 3].len()", int(3));
}

#[test]
fn probe18b_pipe_len_string() {
    assert_val(r#""hello".char_len()"#, int(5));
}

#[test]
fn probe18b_pipe_not_true() {
    assert_val("true >> not", F);
}

#[test]
fn probe18b_pipe_and() {
    assert_val("(true, false) >> and", F);
}

#[test]
fn probe18b_pipe_or() {
    assert_val("(false, true) >> or", T);
}

// ── 3. Builtin functions with wrong arg types ────────────────────

#[test]
fn probe18b_not_wrong_type() {
    assert_error("42 >> not", "cannot unify"); // type checker catches Bool vs IntLiteral
}

#[test]
fn probe18b_len_wrong_type() {
    assert_error("42.len()", "no method");
}

#[test]
fn probe18b_and_wrong_type() {
    assert_error("(1, 2) >> and", "cannot unify"); // type checker catches Struct vs Bool
}

// ── 4. Method on wrong type ──────────────────────────────────────

#[test]
fn probe18b_len_on_int() {
    assert_error("42.len()", "no method");
}

#[test]
fn probe18b_map_on_bool() {
    assert_error("true.map{ * 2 }", "no method");
}

// ── 5. Struct field access vs method conflict ────────────────────

#[test]
fn probe18b_struct_field_shadows_method_not_callable() {
    // (len=42).len() should try to call 42 as a function, which errors
    assert_error("(len=42).len()", "cannot call");
}

#[test]
fn probe18b_struct_field_function_takes_priority() {
    // (len={ in + 1 }).len(5) should call the field function, returning 6
    assert_val("(len={ in + 1 }).len(5)", int(6));
}

// ── 6. fold correctness ──────────────────────────────────────────

#[test]
fn probe18b_fold_sum() {
    assert_val("[1, 2, 3].fold(0, { in.acc + in.elem })", int(6));
}

#[test]
fn probe18b_fold_with_init() {
    assert_val("[1, 2, 3].fold(10, { in.acc + in.elem })", int(16));
}

#[test]
fn probe18b_fold_string_concat() {
    assert_val(
        r#"["a", "b", "c"].fold("", { in.acc + in.elem })"#,
        s("abc"),
    );
}

// ── 7. zip ───────────────────────────────────────────────────────

#[test]
fn probe18b_zip_equal_length() {
    assert_output("[1, 2].zip([3, 4])", "[(1, 3), (2, 4)]");
}

#[test]
fn probe18b_zip_truncate() {
    assert_output("[1, 2, 3].zip([4, 5])", "[(1, 4), (2, 5)]");
}

// ── 8. Pipe prepend into builtin functions ───────────────────────

#[test]
fn probe18b_pipe_into_zip() {
    assert_output("[1, 2].zip([3, 4])", "[(1, 3), (2, 4)]");
}

#[test]
fn probe18b_pipe_into_map() {
    assert_output("[1, 2, 3].map{ * 2 }", "[2, 4, 6]");
}

#[test]
fn probe18b_pipe_into_filter() {
    assert_output("[1, 2, 3].filter{ > 1 }", "[2, 3]");
}

#[test]
fn probe18b_pipe_into_fold() {
    assert_val("[1, 2, 3].fold(0, { in.acc + in.elem })", int(6));
}

// ═══════════════════════════════════════════════════════════════════
// probe18c — Struct construction, spread, destructuring, display
// ═══════════════════════════════════════════════════════════════════

// ── 1. Struct display ──────────────────────────────────────────────

#[test]
fn probe18c_display_positional_struct() {
    assert_output("(1, 2, 3)", "(1, 2, 3)");
}

#[test]
fn probe18c_display_labeled_struct() {
    assert_output("(a=1, b=2)", "(a=1, b=2)");
}

#[test]
fn probe18c_display_mixed_labeled_positional() {
    // Mixed: (a=1, 2) — labeled field a, then positional field.
    // Positional fields get numeric index. a=1 is named so no index consumed?
    // Actually: field a=1 has label Some("a"), field 2 has label None -> pos_index=0.
    // Display: "a=1" (named), "2" (numeric label "0").
    // So display should be "(a=1, 2)".
    assert_output("(a=1, 2)", "(a=1, 2)");
}

#[test]
fn probe18c_display_positional_labeled_positional() {
    // (1, a=2, 3) — pos "0"=1, named "a"=2, pos "1"=3
    // Display: "1, a=2, 3" → "(1, a=2, 3)"
    assert_output("(1, a=2, 3)", "(1, a=2, 3)");
}

// ── 2. Spread creates new struct ───────────────────────────────────

#[test]
fn probe18c_spread_override_field() {
    // (a=10, ...s) where s = (a=1, b=2): a is in explicit_labels, so a from
    // spread is skipped. b from spread is kept. Result: (a=10, b=2).
    assert_output("let s = (a=1, b=2); (a=10, ...s)", "(a=10, b=2)");
}

#[test]
fn probe18c_spread_new_field() {
    // (c=3, ...s) where s = (a=1, b=2): c is explicit, a and b from spread.
    assert_output("let s = (a=1, b=2); (c=3, ...s)", "(c=3, a=1, b=2)");
}

#[test]
fn probe18c_spread_positional_reindex() {
    // (99, ...s) where s = (1, 2): pos 0=99, then spread's "0"=1 → re-indexed "1",
    // spread's "1"=2 → re-indexed "2". Result: (99, 1, 2).
    assert_output("let s = (1, 2); (99, ...s)", "(99, 1, 2)");
}

// ── 3. Spread with duplicate field detection ───────────────────────

#[test]
fn probe18c_spread_explicit_overrides_spread() {
    // (a=2, ...s) where s = (a=1): a is explicit so spread a is skipped. No error.
    assert_output("let s = (a=1); (a=2, ...s)", "(a=2)");
}

#[test]
fn probe18c_spread_then_explicit_same_field() {
    // (...s, a=2) where s = (a=1): explicit_labels = ["a"], so spread's a is skipped.
    // Then explicit a=2 is added. Result should be (a=2).
    // BUT: the spread comes first in field order, and explicit a=2 comes after.
    // Since spread's a is skipped due to explicit_labels, only a=2 appears.
    assert_output("let s = (a=1); (...s, a=2)", "(a=2)");
}

#[test]
fn probe18c_spread_two_spreads_duplicate_named() {
    // (...s, ...t) where s = (a=1), t = (a=2): explicit_labels is empty.
    // First spread adds a=1. Second spread tries to add a=2, but finds a already
    // in result → "duplicate field label 'a' in struct (from spread)" error.
    assert_error("let s = (a=1); let t = (a=2); (...s, ...t)", "duplicate");
}

// ── 4. Empty struct operations ─────────────────────────────────────

#[test]
fn probe18c_empty_struct_display() {
    assert_val("()", U);
}

#[test]
fn probe18c_spread_empty_struct() {
    // BUG-64 fix: () is U, treated as empty struct for spread
    assert_output("let s = (); (...s)", "()");
}

#[test]
fn probe18c_empty_struct_equality() {
    assert_val("() == ()", T);
}

// ── 5. Single-element considerations ───────────────────────────────

#[test]
fn probe18c_single_element_grouping() {
    // (1) is just grouping, evaluates to 1 (not a struct)
    assert_val("(1)", int(1));
}

#[test]
fn probe18c_single_labeled_is_struct() {
    // (a=1) is a labeled field, so it IS a struct
    assert_output("(a=1)", "(a=1)");
}

// ── 6. Field access ────────────────────────────────────────────────

#[test]
fn probe18c_field_access_named() {
    assert_val("(a=1, b=2).a", int(1));
}

#[test]
fn probe18c_field_access_named_second() {
    assert_val("(a=1, b=2).b", int(2));
}

#[test]
fn probe18c_field_access_positional_first() {
    assert_val("(1, 2, 3).0", int(1));
}

#[test]
fn probe18c_field_access_positional_last() {
    assert_val("(1, 2, 3).2", int(3));
}

#[test]
fn probe18c_field_access_positional_out_of_bounds() {
    assert_error("(1, 2, 3).3", "not found");
}

#[test]
fn probe18c_field_access_named_not_found() {
    assert_error("(a=1, b=2).c", "not found");
}

// ── 7. Struct comparison ───────────────────────────────────────────

#[test]
fn probe18c_named_struct_eq_order_insensitive() {
    // Named structs compared by name, not order
    assert_val("(a=1, b=2) == (b=2, a=1)", T);
}

#[test]
fn probe18c_positional_struct_eq() {
    assert_val("(1, 2) == (1, 2)", T);
}

#[test]
fn probe18c_positional_struct_neq() {
    assert_val("(1, 2) != (2, 1)", T);
}

// ── 8. Struct destructuring edge cases ─────────────────────────────

#[test]
fn probe18c_destructure_named() {
    assert_val("(a=1, b=2) >> let(a, b); a + b", int(3));
}

#[test]
fn probe18c_destructure_positional() {
    assert_val("(1, 2) >> let(a, b); a + b", int(3));
}

#[test]
fn probe18c_destructure_named_with_rest() {
    // (a=1, b=2, c=3) >> let(a, ...rest); rest
    // Named binding: a=1 is bound. rest captures (b=2, c=3).
    assert_output("(a=1, b=2, c=3) >> let(a, ...rest); rest", "(b=2, c=3)");
}

#[test]
fn probe18c_destructure_too_many_patterns() {
    // More patterns than fields should error
    assert_error("(1, 2) >> let(a, b, c); a", "not found");
}

#[test]
fn probe18c_destructure_too_few_patterns_no_rest() {
    // Fewer patterns (no rest) should error — unconsumed fields
    assert_error("(1, 2, 3) >> let(a, b); a", "too many fields");
}

// ── 9. Nested struct access ────────────────────────────────────────

#[test]
fn probe18c_nested_struct_field_access() {
    assert_val("(a=(x=1, y=2), b=3).a.x", int(1));
}

#[test]
fn probe18c_nested_struct_field_access_deep() {
    assert_val("(a=(x=1, y=2), b=3).a.y", int(2));
}

// ═══════════════════════════════════════════════════════════════════
// probe18a — eval_collecting / eval_pipe_collecting bug hunting
//
// Focus: top-level binding preservation across statements, especially
// through tag declarations, let sugar, and pipe chains.
// ═══════════════════════════════════════════════════════════════════

// ── 1. Multiple tag declarations followed by usage ────────────────

#[test]
fn probe18a_multiple_tags_then_use() {
    // tag(Ok); tag(Err); Ok(1) — should produce Ok(1)
    assert_output("tag(Ok); tag(Err); Ok(1)", "Ok(1)");
}

#[test]
fn probe18a_multiple_tags_then_branch() {
    // tag(A); tag(B); A(()) >> { A -> "a", B -> "b" } — should be "a"
    assert_val(r#"tag(A); tag(B); A(()) >> { A -> "a", B -> "b" }"#, s("a"));
}

// ── 2. Let sugar with tag sugar in same expression ────────────────

#[test]
fn probe18a_tag_then_let_then_use() {
    // tag(Ok); let x = Ok(5); x — should be Ok(5)
    assert_output("tag(Ok); let x = Ok(5); x", "Ok(5)");
}

#[test]
fn probe18a_let_fn_then_tag_then_apply() {
    // let f = { in + 1 }; tag(Ok); Ok(3 >> f) — should be Ok(4)
    assert_output("let f = { in + 1 }; tag(Ok); Ok(3 >> f)", "Ok(4)");
}

// ── 3. Deeply nested let chains ──────────────────────────────────

#[test]
fn probe18a_triple_let_sum() {
    // let a = 1; let b = 2; let c = 3; a + b + c — should be 6
    assert_val("let a = 1; let b = 2; let c = 3; a + b + c", int(6));
}

#[test]
fn probe18a_triple_let_dependent() {
    // let a = 1; let b = a + 1; let c = b + 1; c — should be 3
    assert_val("let a = 1; let b = a + 1; let c = b + 1; c", int(3));
}

// ── 4. Let with complex RHS ─────────────────────────────────────

#[test]
fn probe18a_let_arithmetic_rhs() {
    // let x = 1 + 2 * 3; x — should be 7
    assert_val("let x = 1 + 2 * 3; x", int(7));
}

#[test]
fn probe18a_let_method_chain_rhs() {
    // let x = [1, 2, 3].map{ * 2 }; x — should be [2, 4, 6]
    assert_output("let x = [1, 2, 3].map{ * 2 }; x", "[2, 4, 6]");
}

// ── 5. Array destructuring in let sugar ──────────────────────────

#[test]
fn probe18a_array_destructure_sum() {
    // [1, 2, 3] >> let[a, b, c]; a + b + c — should be 6
    assert_val("[1, 2, 3] >> let[a, b, c]; a + b + c", int(6));
}

#[test]
fn probe18a_array_destructure_rest() {
    // [1, 2, 3] >> let[first, ...rest]; rest — should be [2, 3]
    assert_output("[1, 2, 3] >> let[first, ...rest]; rest", "[2, 3]");
}

// ── 6. Struct spread in complex expressions ──────────────────────

#[test]
fn probe18a_struct_spread_override() {
    // let s = (a=1, b=2); (a=99, ...s) — explicit a=99 overrides spread a=1
    assert_output("let s = (a=1, b=2); (a=99, ...s)", "(a=99, b=2)");
}

#[test]
fn probe18a_struct_spread_extend() {
    // let s = (a=1, b=2); (...s, c=3) — spread then add c=3
    assert_output("let s = (a=1, b=2); (...s, c=3)", "(a=1, b=2, c=3)");
}

// ── 7. Multiple pipes with let ───────────────────────────────────

#[test]
fn probe18a_pipe_let_pipe_let() {
    // 1 >> let(x) >> { in + 1 } >> let(y); x + y — should be 3
    // x = 1, passthrough = 1, 1 >> { in + 1 } = 2, y = 2, x + y = 3
    assert_val("1 >> let(x) >> { in + 1 } >> let(y); x + y", int(3));
}

// ── 8. Pipe into branching after let ─────────────────────────────

#[test]
fn probe18a_tag_let_branch() {
    // tag(Ok); tag(Err); let v = Ok(42); v >> { Ok(x) -> x, Err(_) -> 0 }
    // should be 42
    assert_val(
        "tag(Ok); tag(Err); let v = Ok(42); v >> { Ok(x) -> x, Err(_) -> 0 }",
        int(42),
    );
}

// ═══════════════════════════════════════════════════════════════════
// probe19a — Lexer edge-case tests
// ═══════════════════════════════════════════════════════════════════

// ── 1. Underscore in numbers ─────────────────────────────────────

#[test]
fn probe19a_underscore_int_1_000() {
    assert_val("1_000", int(1000));
}

#[test]
fn probe19a_underscore_int_1_000_000() {
    assert_val("1_000_000", int(1000000));
}

#[test]
fn probe19a_underscore_hex_0xff_ff() {
    assert_val("0xFF_FF", int(65535));
}

#[test]
fn probe19a_underscore_float_integer_part() {
    // 1_000.5 — underscores in integer part of a float
    assert_val("1_000.5", float(1000.5));
}

#[test]
fn probe19a_underscore_float_fractional_part() {
    // 1.000_5 — underscores in fractional part of a float
    assert_val("1.000_5", float(1.0005));
}

// ── 2. Hex literals ─────────────────────────────────────────────

#[test]
fn probe19a_hex_zero() {
    assert_val("0x0", int(0));
}

#[test]
fn probe19a_hex_ff() {
    assert_val("0xFF", int(255));
}

#[test]
fn probe19a_hex_dead() {
    assert_val("0xDEAD", int(57005));
}

#[test]
fn probe19a_hex_double_zero() {
    assert_val("0x00", int(0));
}

#[test]
fn probe19a_hex_lowercase_ff() {
    assert_val("0xff", int(255));
}

// ── 3. Hex edge cases ───────────────────────────────────────────

#[test]
fn probe19a_hex_no_digits_error() {
    // 0x with nothing after should error
    assert_error("0x", "expected hex digits after 0x");
}

#[test]
fn probe19a_hex_invalid_digit_g() {
    // 0xG — G is not a hex digit, so 0x has no valid digits
    assert_error("0xG", "expected hex digits after 0x");
}

// ── 4. Float edge cases ─────────────────────────────────────────

#[test]
fn probe19a_float_zero() {
    assert_val("0.0", float(0.0));
}

#[test]
fn probe19a_float_one() {
    assert_val("1.0", float(1.0));
}

#[test]
fn probe19a_float_999_999() {
    assert_val("999.999", float(999.999));
}

#[test]
fn probe19a_float_0_1() {
    assert_val("0.1", float(0.1));
}

// ── 5. Integer edge cases ───────────────────────────────────────

#[test]
fn probe19a_int_zero() {
    assert_val("0", int(0));
}

#[test]
fn probe19a_int_i64_max() {
    assert_val("9223372036854775807", int(9223372036854775807));
}

// ── 6. Comments ─────────────────────────────────────────────────

#[test]
fn probe19a_comment_line_before_expr() {
    assert_val("# this is a comment\n1", int(1));
}

#[test]
fn probe19a_comment_trailing() {
    assert_val("1 # trailing comment", int(1));
}

// ── 7. Multiple comments ────────────────────────────────────────

#[test]
fn probe19a_multiple_comment_lines() {
    assert_val("# first\n# second\n42", int(42));
}

// ── 8. Whitespace handling ──────────────────────────────────────

#[test]
fn probe19a_tab_whitespace() {
    assert_val("\t1\t+\t2", int(3));
}

#[test]
fn probe19a_multiple_newlines() {
    assert_val("\n\n\n42\n\n", int(42));
}

#[test]
fn probe19a_mixed_whitespace() {
    assert_val("  \t\n  1  \t +  \n  2  ", int(3));
}

// ── 9. Unknown character errors ─────────────────────────────────

#[test]
fn probe19a_unknown_char_at() {
    assert_error("@", "unexpected character: '@'");
}

#[test]
fn probe19a_unknown_char_tilde() {
    assert_error("~", "unexpected character: '~'");
}

#[test]
fn probe19a_unknown_char_dollar() {
    assert_error("$", "unexpected character: '$'");
}

#[test]
fn probe19a_unknown_char_backtick() {
    assert_error("`", "unexpected character: '`'");
}

// ── 10. Byte literal edge cases ─────────────────────────────────

#[test]
fn probe19a_byte_null() {
    assert_output("b'\\x00'", "b'\\0'");
}

#[test]
fn probe19a_byte_xff() {
    assert_output("b'\\xff'", "b'\\xff'");
}

#[test]
fn probe19a_byte_ascii_a() {
    assert_val("b'A'", byte(b'A'));
}

// ── 11. Char edge cases ─────────────────────────────────────────

#[test]
fn probe19a_char_hex_escape_41() {
    // '\x41' is 'A'
    assert_val("'\\x41'", ch('A'));
}

#[test]
fn probe19a_char_null_escape() {
    // '\0' is the null char
    assert_output("'\\0'", "'\\0'");
}

// ── 12. String escape sequences round-trip ──────────────────────

#[test]
fn probe19a_string_newline_escape() {
    // A string with \n should produce a string containing an actual newline
    // When displayed, the nana Display for Str just prints the raw string
    // so the output should contain a literal newline character
    assert_val(r#""\n""#, s("\n"));
}

#[test]
fn probe19a_string_tab_escape() {
    assert_val(r#""\t""#, s("\t"));
}

#[test]
fn probe19a_string_null_escape() {
    assert_val(r#""\0""#, s("\0"));
}

#[test]
fn probe19a_string_hex_escape() {
    // "\x41" should be "A"
    assert_val(r#""\x41""#, s("A"));
}

#[test]
fn probe19a_string_backslash_escape() {
    assert_val(r#""\\""#, s("\\"));
}

#[test]
fn probe19a_string_quote_escape() {
    assert_val(r#""\"""#, s("\""));
}

// ── 13. `!` alone (not `!=`) ────────────────────────────────────

#[test]
fn probe19a_bang_alone_error() {
    assert_error("!", "did you mean '!='");
}

// ── 14. Operator sequences ──────────────────────────────────────

#[test]
fn probe19a_op_pipe() {
    // >> is the pipe operator; 1 >> { in + 1 } = 2
    assert_val("1 >> { in + 1 }", int(2));
}

#[test]
fn probe19a_op_arrow() {
    // -> is used in branches: true >> { true -> 99, false -> 0 }
    assert_val("true >> { true -> 99, false -> 0 }", int(99));
}

#[test]
fn probe19a_op_dotdot() {
    // .. is the range operator — ensure it parses (range 1..3)
    assert_parses("1..3");
}

#[test]
fn probe19a_op_spread() {
    // ... is the spread operator — test in a destructuring context
    assert_parses("(1,2,3) >> let(a, ...rest)");
}

#[test]
fn probe19a_op_lteq() {
    assert_val("1 <= 2", T);
}

#[test]
fn probe19a_op_gteq() {
    assert_val("2 >= 1", T);
}

#[test]
fn probe19a_op_eqeq() {
    assert_val("1 == 1", T);
}

#[test]
fn probe19a_op_noteq() {
    assert_val("1 != 2", T);
}

// ── 15. Identifiers starting with underscore ────────────────────

#[test]
fn probe19a_ident_underscore_name() {
    // _foo should be a valid identifier
    assert_val("1 >> let(_foo); _foo", int(1));
}

#[test]
fn probe19a_ident_bare_underscore() {
    // _ alone is the discard/wildcard — test in a branch pattern
    assert_val("42 >> { _ -> 99 }", int(99));
}

#[test]
fn probe19a_ident_underscore_digits() {
    // _123 should be a valid identifier
    assert_val("1 >> let(_123); _123", int(1));
}

// ═══════════════════════════════════════════════════════════════════
// probe19c — Display formatting and round-trip correctness
// ═══════════════════════════════════════════════════════════════════

// ── 1. Char display round-trip ──────────────────────────────────

#[test]
fn probe19c_char_display_newline() {
    assert_output(r"'\n'", r"'\n'");
}

#[test]
fn probe19c_char_display_tab() {
    assert_output(r"'\t'", r"'\t'");
}

#[test]
fn probe19c_char_display_carriage_return() {
    assert_output(r"'\r'", r"'\r'");
}

#[test]
fn probe19c_char_display_null() {
    assert_output(r"'\0'", r"'\0'");
}

#[test]
fn probe19c_char_display_backslash() {
    assert_output(r"'\\'", r"'\\'");
}

#[test]
fn probe19c_char_display_single_quote() {
    assert_output(r"'\''", r"'\''");
}

#[test]
fn probe19c_char_display_plain() {
    assert_val("'a'", ch('a'));
}

#[test]
fn probe19c_char_display_hex_41() {
    // '\x41' is hex for 'A', should display as 'A'
    assert_val(r"'\x41'", ch('A'));
}

// ── 2. Byte display round-trip ──────────────────────────────────

#[test]
fn probe19c_byte_display_newline() {
    assert_output(r"b'\n'", r"b'\n'");
}

#[test]
fn probe19c_byte_display_tab() {
    assert_output(r"b'\t'", r"b'\t'");
}

#[test]
fn probe19c_byte_display_null() {
    assert_output(r"b'\0'", r"b'\0'");
}

#[test]
fn probe19c_byte_display_hex_00() {
    // b'\x00' should display as b'\0'
    assert_output(r"b'\x00'", r"b'\0'");
}

#[test]
fn probe19c_byte_display_hex_ff() {
    assert_output(r"b'\xff'", r"b'\xff'");
}

#[test]
fn probe19c_byte_display_plain_a() {
    assert_val("b'A'", byte(b'A'));
}

#[test]
fn probe19c_byte_display_hex_41() {
    // b'\x41' = 0x41 = 65 = 'A', printable, should display as b'A'
    assert_val(r"b'\x41'", byte(b'A'));
}

// ── 3. Float display ────────────────────────────────────────────

#[test]
fn probe19c_float_display_one() {
    assert_val("1.0", float(1.0));
}

#[test]
fn probe19c_float_display_half() {
    assert_val("0.5", float(0.5));
}

#[test]
fn probe19c_float_display_hundred() {
    assert_val("100.0", float(100.0));
}

#[test]
fn probe19c_float_display_negative() {
    assert_val("-1.5", float(-1.5));
}

#[test]
fn probe19c_float_display_addition_result() {
    // 1.0 + 2.0 = 3.0, should display as "3.0" not "3"
    assert_val("1.0 + 2.0", float(3.0));
}

// ── 4. Nested values in display ─────────────────────────────────

#[test]
fn probe19c_nested_strings_in_array() {
    assert_output(r#"["hello", "world"]"#, r#"["hello", "world"]"#);
}

#[test]
fn probe19c_nested_string_in_struct() {
    assert_output(r#"(a="test")"#, r#"(a="test")"#);
}

#[test]
fn probe19c_nested_string_in_tagged() {
    assert_output(r#"tag(Ok); Ok("yes")"#, r#"Ok("yes")"#);
}

// ── 5. String with special chars in nested display ──────────────

#[test]
fn probe19c_nested_string_with_newline() {
    // Array containing a string with a literal newline: should be escaped in display
    assert_output(r#"["hello\nworld"]"#, r#"["hello\nworld"]"#);
}

#[test]
fn probe19c_nested_string_with_tab() {
    assert_output(r#"(a="\t")"#, r#"(a="\t")"#);
}

// ── 6. Tagged value display ─────────────────────────────────────

#[test]
fn probe19c_tagged_unit_payload() {
    // Tag with unit payload should display as just the tag name
    assert_output("tag(None); None(())", "None");
}

#[test]
fn probe19c_tagged_int_payload() {
    assert_output("tag(Some); Some(42)", "Some(42)");
}

#[test]
fn probe19c_tagged_string_payload() {
    assert_output(r#"tag(Ok); Ok("hello")"#, r#"Ok("hello")"#);
}

// ── 7. Complex nested structures ────────────────────────────────

#[test]
fn probe19c_array_of_tagged_values() {
    assert_output("tag(Ok); [Ok(1), Ok(2)]", "[Ok(1), Ok(2)]");
}

#[test]
fn probe19c_nested_struct_with_array() {
    assert_output("(x=[1, 2], y=(a=3, b=4))", "(x=[1, 2], y=(a=3, b=4))");
}

// ── 8. BuiltinFn display ────────────────────────────────────────

#[test]
fn probe19c_builtin_fn_display() {
    assert_output("not", "<builtin not>");
}

// ── 9. Closure display ──────────────────────────────────────────

#[test]
fn probe19c_closure_display() {
    assert_output("{ in + 1 }", "<function>");
}

// ── 10. Char inside nested values ───────────────────────────────

#[test]
fn probe19c_chars_in_array() {
    assert_output("['a', 'b']", "['a', 'b']");
}

#[test]
fn probe19c_char_in_struct() {
    assert_output("(x='a')", "(x='a')");
}

#[test]
fn probe19c_escaped_char_in_array() {
    // '\n' inside an array should still display as '\n'
    assert_output(r"['\n', '\t']", r"['\n', '\t']");
}

// ═══════════════════════════════════════════════════════════════════
// probe19b_ — Semicolon scoping: attach_body / nest_let_in_expr
// ═══════════════════════════════════════════════════════════════════

// ── 1. tag(X) followed by semicolon ─────────────────────────────

#[test]
fn probe19b_tag_semicolon_basic() {
    // tag(Ok) desugars to Pipe(NewTag, Let{Ok, Ident(Ok)}).
    // Semicolon attach_body should replace the identity body with the rest.
    // So tag(Ok); Ok(1) should produce Ok(1).
    assert_output("tag(Ok); Ok(1)", "Ok(1)");
}

#[test]
fn probe19b_tag_semicolon_two_tags() {
    // Two tags in sequence: both should be in scope for the final expression.
    assert_output("tag(Ok); tag(Err); Ok(1)", "Ok(1)");
}

#[test]
fn probe19b_tag_semicolon_two_tags_use_second() {
    // Use the second tag to verify both are in scope.
    assert_output("tag(Ok); tag(Err); Err(42)", "Err(42)");
}

// ── 2. let inside parenthesized expression should NOT leak scope ─

#[test]
fn probe19b_paren_let_contained() {
    // Parens contain the let; result of (let x = 1; x + 1) is 2, then + 3 = 5.
    assert_val("(let x = 1; x + 1) + 3", int(5));
}

#[test]
fn probe19b_paren_let_no_leak() {
    // After the paren, x should NOT be in scope.
    assert_error("(let x = 1; x); x", "undefined variable");
}

// ── 3. let _ = expr (discard) ────────────────────────────────────

#[test]
fn probe19b_let_discard_basic() {
    // let _ = 42; 1 should evaluate 42, discard it, return 1.
    assert_val("let _ = 42; 1", int(1));
}

#[test]
fn probe19b_let_discard_side_effect() {
    // let _ = print("side effect"); 5 should be 5.
    // (print returns (), which is discarded)
    assert_val("let _ = print(\"side effect\"); 5", int(5));
}

// ── 4. Multiple let sugar on one line ────────────────────────────

#[test]
fn probe19b_multi_let_sugar() {
    // Chain of let x = ...; should all be in scope at the end.
    assert_val("let a = 1; let b = 2; let c = 3; a + b + c", int(6));
}

#[test]
fn probe19b_multi_let_sugar_with_tags() {
    // Let bindings that capture tag constructors.
    assert_output("let a = tag(Ok); let b = tag(Err); a(1)", "Ok(1)");
}

// ── 5. let sugar with complex RHS containing pipes ───────────────

#[test]
fn probe19b_let_rhs_pipe() {
    // let x = 1 >> { in + 1 }; x — RHS is a pipe expression, should be 2.
    assert_val("let x = 1 >> { in + 1 }; x", int(2));
}

#[test]
fn probe19b_let_rhs_pipe_map() {
    // let x = [1, 2, 3].map{ * 2 }; x — method call in RHS.
    assert_output("let x = [1, 2, 3].map{ * 2 }; x", "[2, 4, 6]");
}

// ── 6. let sugar with RHS that is a block ────────────────────────

#[test]
fn probe19b_let_rhs_block_closure() {
    // let f = { in + 1 }; 3 >> f — f should be a closure, piping 3 gives 4.
    assert_val("let f = { in + 1 }; 3 >> f", int(4));
}

#[test]
fn probe19b_let_rhs_branch_block() {
    // let g = { true -> "yes", false -> "no" }; true >> g — branching block.
    assert_val(
        r#"let g = { true -> "yes", false -> "no" }; true >> g"#,
        s("yes"),
    );
}

// ── 7. Nested let sugars with tags ───────────────────────────────

#[test]
fn probe19b_nested_let_tag_combo() {
    // tag(Ok) introduces Ok; let f = { in >> Ok } wraps input with Ok.
    // 5 >> f should be Ok(5).
    assert_output("tag(Ok); let f = { in >> Ok }; 5 >> f", "Ok(5)");
}

#[test]
fn probe19b_nested_let_tag_combo_call() {
    // Alternative: { Ok(in) } — explicitly call Ok with in.
    assert_output("tag(Ok); let f = { Ok(in) }; 5 >> f", "Ok(5)");
}

// ── 8. Semicolons in different contexts ──────────────────────────

#[test]
fn probe19b_let_inside_block_body() {
    // { let x = 1; x + 2 } — the block captures a closure.
    // When called with any input, x=1 and returns 3.
    assert_val("99 >> { let x = 1; x + 2 }", int(3));
}

#[test]
fn probe19b_pipe_into_block_with_let() {
    // 3 >> { let x = in; x + 1 } — bind `in` to x, then x + 1 = 4.
    assert_val("3 >> { let x = in; x + 1 }", int(4));
}

#[test]
fn probe19b_block_semicolon_sequence() {
    // { 1; 2 } — block with semicolon. Evaluates 1, discards, returns 2.
    assert_val("0 >> { 1; 2 }", int(2));
}

// ── 9. Trailing semicolons ───────────────────────────────────────

#[test]
fn probe19b_trailing_semi_literal() {
    // 1; — trailing semicolon turns it into () (unit).
    assert_val("1;", U);
}

#[test]
fn probe19b_trailing_semi_let() {
    // let x = 5; — trailing semicolon after let, result is ().
    assert_val("let x = 5;", U);
}

#[test]
fn probe19b_trailing_semi_tag() {
    // tag(Ok); — trailing semicolon after tag, result is ().
    assert_val("tag(Ok);", U);
}

// ── 10. let sugar where RHS contains tag sugar ───────────────────

#[test]
fn probe19b_let_rhs_tag_sugar() {
    // let ok = tag(Ok); ok(1) — tricky: tag(Ok) desugars to Pipe(NewTag, Let{Ok, Ok}).
    // `let ok = ...` via nest_let_in_expr should nest inside so that both
    // Ok and ok are bound. ok should be the tag constructor.
    assert_output("let ok = tag(Ok); ok(1)", "Ok(1)");
}

#[test]
fn probe19b_let_rhs_tag_sugar_pipe() {
    // let ok = tag(Ok); 42 >> ok — pipe into the tag constructor bound to ok.
    assert_output("let ok = tag(Ok); 42 >> ok", "Ok(42)");
}

#[test]
fn probe19b_let_rhs_tag_and_original_name() {
    // After `let ok = tag(Ok)`, both `ok` and `Ok` should be in scope.
    assert_output("let ok = tag(Ok); Ok(1)", "Ok(1)");
}

#[test]
fn probe19b_let_rhs_tag_both_names_same_tag() {
    // ok and Ok should refer to the same tag — tagged values should be equal.
    assert_val("let ok = tag(Ok); ok(1) == Ok(1)", T);
}

// ═══════════════════════════════════════════════════════════════════
// probe20c — REPL multi-line binding persistence tests
// ═══════════════════════════════════════════════════════════════════

/// Helper that simulates the REPL: evaluates each line in sequence,
/// threading the environment so bindings persist across lines.
/// Returns the string representation of the last line's value.
fn eval_repl(lines: &[&str]) -> String {
    let mut env = repl_env();
    let mut last_output = String::from("()");
    for line in lines {
        let (val, new_env) = nana::run_in_env(line, &env)
            .unwrap_or_else(|e| panic!("REPL line failed.\n  line: {line}\n  error: {e}"));
        last_output = format!("{}", val);
        env = new_env;
    }
    last_output
}

#[test]
fn probe20c_basic_binding_persists() {
    // let x = 42 on one REPL line, then x + 1 on the next -> 43
    assert_eq!(eval_repl(&["let x = 42", "x + 1"]), "43");
}

#[test]
fn probe20c_tag_persists() {
    // Define a tag on one REPL line, use it on the next.
    assert_eq!(eval_repl(&["tag(Ok)", "Ok(42)"]), "Ok(42)");
}

#[test]
fn probe20c_multiple_bindings() {
    // Multiple let bindings across REPL lines should all persist.
    assert_eq!(eval_repl(&["let x = 1", "let y = 2", "x + y"]), "3");
}

#[test]
fn probe20c_pipe_with_let_persists() {
    // `42 >> let(x)` in the REPL should persist x for the next line.
    assert_eq!(eval_repl(&["42 >> let(x)", "x"]), "42");
}

#[test]
fn probe20c_array_destructuring_persists() {
    // `[1, 2, 3] >> let[a, b, c]` should persist a, b, c.
    assert_eq!(eval_repl(&["[1, 2, 3] >> let[a, b, c]", "a + b + c"]), "6");
}

#[test]
fn probe20c_struct_destructuring_persists() {
    // `(x=10, y=20) >> let(x, y)` should persist x and y.
    assert_eq!(eval_repl(&["(x=10, y=20) >> let(x, y)", "x + y"]), "30");
}

#[test]
fn probe20c_function_defined_in_repl_persists() {
    // Define a block/lambda on one line, pipe into it on the next.
    assert_eq!(eval_repl(&["let f = { in + 1 }", "5 >> f"]), "6");
}

#[test]
fn probe20c_shadowing_in_repl() {
    // Rebinding the same name should shadow the previous value.
    assert_eq!(eval_repl(&["let x = 1", "let x = 2", "x"]), "2");
}

#[test]
fn probe20c_complex_tag_function_pipe() {
    // Multi-line REPL: define tags, define a function, then use it.
    assert_eq!(
        eval_repl(&[
            "tag(Ok); tag(Err)",
            "let safe_div = { in >> let(a, b); b == 0 >> { true -> Err(\"div/0\"), false -> Ok(a / b) } }",
            "(10, 2) >> safe_div",
        ]),
        "Ok(5)"
    );
}

// ═══════════════════════════════════════════════════════════════════
// probe20b — Error handling edge cases
// ═══════════════════════════════════════════════════════════════════

// ── 1. Type errors in arithmetic ──

#[test]
fn probe20b_type_error_string_plus_int() {
    // String + Int should produce a type error, not silently coerce.
    assert_error(r#""hello" + 42"#, "type error");
}

#[test]
fn probe20b_type_error_bool_plus_bool() {
    // Bool + Bool is not defined.
    assert_error("true + false", "no method 'add'");
}

#[test]
fn probe20b_type_error_array_subtraction() {
    // Array subtraction is not defined (only + for concatenation).
    assert_error("[1] - [2]", "no method 'subtract'");
}

#[test]
fn probe20b_type_error_string_multiply() {
    // String * Int is not defined.
    assert_error(r#""a" * 3"#, "no method 'times'");
}

// ── 2. Type errors in comparisons ──

#[test]
fn probe20b_type_error_string_lt_int() {
    // Comparing string < int should error (incomparable types).
    assert_error(r#""hello" < 42"#, "type error");
}

#[test]
fn probe20b_bool_comparison_works() {
    // Bool has Ord in Rust — false < true should work.
    assert_val("true < false", F);
}

#[test]
fn probe20b_bool_comparison_true_gt_false() {
    assert_val("false < true", T);
}

#[test]
fn probe20b_type_error_array_ordering() {
    // Arrays support == and != but not ordering.
    assert_error("[1, 2] < [1, 3]", "cannot order");
}

// ── 3. Calling non-functions ──

#[test]
fn probe20b_call_int() {
    assert_error("42(1)", "cannot call non-function");
}

#[test]
fn probe20b_call_string() {
    assert_error(r#""hello"(1)"#, "cannot call non-function");
}

#[test]
fn probe20b_call_bool() {
    assert_error("true(1)", "cannot call non-function");
}

// ── 4. Pipe into non-function ──

#[test]
fn probe20b_pipe_into_int() {
    // `1 >> 2` — rhs evaluates to Int(2), then apply(Int(2), Int(1)) should fail.
    assert_error("1 >> 2", "cannot call non-function");
}

#[test]
fn probe20b_pipe_into_string() {
    assert_error(r#"1 >> "hello""#, "cannot call non-function");
}

#[test]
fn probe20b_pipe_into_bool() {
    assert_error("1 >> true", "cannot call non-function");
}

// ── 5. Division errors ──

#[test]
fn probe20b_int_division_by_zero() {
    assert_error("1 / 0", "division by zero");
}

#[test]
fn probe20b_float_division_by_zero() {
    assert_error("1.0 / 0.0", "division by zero");
}

// ── 6. Integer overflow ──

#[test]
fn probe20b_int_overflow_addition() {
    // i64::MAX + 1 should overflow.
    assert_error("9223372036854775807 + 1", "integer overflow");
}

#[test]
fn probe20b_int_underflow_subtraction() {
    // (i64::MAX negated then - 1) gives i64::MIN; subtracting 1 more should underflow.
    // -9223372036854775807 - 1 = i64::MIN = -9223372036854775808
    // i64::MIN - 1 should overflow.
    assert_error("(-9223372036854775807 - 1) - 1", "integer overflow");
}

// ── 7. Undefined variable ──

#[test]
fn probe20b_undefined_variable_bare() {
    assert_error("x", "undefined variable: x");
}

#[test]
fn probe20b_undefined_variable_after_let() {
    assert_error("1 >> let(x); y", "undefined variable: y");
}

// ── 8. Parse errors ──

#[test]
fn probe20b_parse_error_bare_plus() {
    // `+` alone should fail to parse — it's not a valid prefix.
    assert_error("+", "unexpected token");
}

#[test]
fn probe20b_parse_error_trailing_plus() {
    // `1 +` — missing right operand.
    assert_error("1 +", "unexpected token");
}

#[test]
fn probe20b_parse_error_bare_let() {
    // `let` alone without `(` or `[` should fail.
    assert_error("let", "expected");
}

#[test]
fn probe20b_parse_error_block_arrow() {
    // `{ -> }` — `->` is not a valid expression start in a block.
    assert_error("{ -> }", "unexpected token");
}

#[test]
fn probe20b_parse_error_unclosed_paren() {
    // Missing closing paren.
    assert_error("(1, 2", "expected");
}

// ── 9. Spread on non-struct ──

#[test]
fn probe20b_spread_on_int() {
    assert_error("(...42)", "spread on non-struct");
}

#[test]
fn probe20b_spread_on_array() {
    assert_error("(...[1, 2])", "spread on non-struct");
}

#[test]
fn probe20b_spread_on_bool() {
    assert_error("(...true)", "spread on non-struct");
}

// ── 10. Field access on non-struct ──

#[test]
fn probe20b_field_access_on_int() {
    // Space needed: `42.x` lexes as float-like. `42 .x` or `(42).x`.
    assert_error("(42).x", "field access on non-struct");
}

#[test]
fn probe20b_field_access_on_string() {
    // "hello".x — parsed as FieldAccess, should error.
    assert_error(r#""hello".x"#, "field access on non-struct");
}

#[test]
fn probe20b_field_access_on_array() {
    assert_error("[1, 2].x", "field access on non-struct");
}

// ── 11. Duplicate field labels ──

#[test]
fn probe20b_duplicate_field_label() {
    assert_error("(a=1, a=2)", "duplicate field label");
}

// ── 12. Nested error propagation ──

#[test]
fn probe20b_error_in_applied_block() {
    // The block `{ 1 / 0 }` is a valid closure, but applying it should error.
    assert_error("() >> { 1 / 0 }", "division by zero");
}

#[test]
fn probe20b_pipe_error_propagation() {
    // `1 >> { in / 0 }` — division by zero inside the piped block.
    assert_error("1 >> { in / 0 }", "division by zero");
}

// ═══════════════════════════════════════════════════════════════════
// probe21 — Block sugar, branching detection, and tricky edge cases
// ═══════════════════════════════════════════════════════════════════

// ── 1. Block sugar with range operator ──

#[test]
fn probe21_block_sugar_range() {
    // `{ ..10 }` is sugar for `{ in..10 }`, producing a range struct.
    // 5 >> { ..10 } should give (start=5, end=10).
    assert_output("5 >> { ..10 }", "(start=5, end=10)");
}

// ── 2. Block sugar with comparisons returning bools ──

#[test]
fn probe21_block_sugar_eq() {
    // `{ == 5 }` is sugar for `{ in == 5 }`.
    assert_val("5 >> { == 5 }", T);
}

#[test]
fn probe21_block_sugar_eq_false() {
    assert_val("4 >> { == 5 }", F);
}

#[test]
fn probe21_block_sugar_gt() {
    // `{ > 3 }` is sugar for `{ in > 3 }`.
    assert_val("5 >> { > 3 }", T);
}

#[test]
fn probe21_block_sugar_gt_false() {
    assert_val("2 >> { > 3 }", F);
}

#[test]
fn probe21_block_sugar_lt() {
    assert_val("2 >> { < 3 }", T);
}

#[test]
fn probe21_block_sugar_lteq() {
    assert_val("3 >> { <= 3 }", T);
}

#[test]
fn probe21_block_sugar_gteq() {
    assert_val("3 >> { >= 3 }", T);
}

#[test]
fn probe21_block_sugar_neq() {
    assert_val("3 >> { != 4 }", T);
}

// ── 3. Expression blocks that could look like branches ──

#[test]
fn probe21_expr_block_not_branch() {
    // `{ x + 1 }` where `x` is not followed by `->` — should be an expression block.
    assert_val("let x = 5; x >> { x + 1 }", int(6));
}

#[test]
fn probe21_branch_binding_captures_anything() {
    // `x -> "matched"` where `x` is a binding in the pattern (not a lookup).
    // In a branch block, an identifier pattern without a tag constructor in scope
    // is a catch-all binding. It matches anything.
    assert_val(r#"let x = 5; x >> { x -> "matched" }"#, s("matched"));
}

// ── 4. `{ in }` as expression block returning its input ──

#[test]
fn probe21_block_in_returns_input() {
    // `{ in }` is an expression block that returns the block's input.
    assert_val("1 >> { in }", int(1));
}

#[test]
fn probe21_block_in_string_passthrough() {
    assert_val(r#""hello" >> { in }"#, s("hello"));
}

// ── 5. `{ true }` — expression block, not branch ──

#[test]
fn probe21_block_true_is_expr_block() {
    // `{ true }` is NOT a branching block because `true` is not followed by `->`.
    // So `{ true }` is an expression block that evaluates to `true`.
    // When piped `false >> { true }`, the input is `false`, but the body
    // just evaluates the literal `true`.
    assert_val("false >> { true }", T);
}

#[test]
fn probe21_block_false_is_expr_block() {
    // Similarly, `{ false }` is an expression block evaluating to `false`.
    assert_val("true >> { false }", F);
}

#[test]
fn probe21_block_int_literal_is_expr_block() {
    // `{ 42 }` is an expression block (42 not followed by ->).
    assert_val("0 >> { 42 }", int(42));
}

// ── 6. `{ _ -> ... }` as a branch block ──

#[test]
fn probe21_wildcard_branch() {
    // `{ _ -> "matched" }` is a branching block — `_` followed by `->`.
    assert_val(r#"42 >> { _ -> "matched" }"#, s("matched"));
}

#[test]
fn probe21_wildcard_branch_preserves_in() {
    // `in` in the arm body refers to the block input (the scrutinee).
    assert_val("42 >> { _ -> in }", int(42));
}

// ── 7. Identifier that's also a tag in branch patterns ──

#[test]
fn probe21_tag_pattern_vs_binding() {
    // When `Ok` is a tag constructor in scope, `Ok` in a branch pattern
    // matches the tag with unit payload. `42` is not tagged, so `Ok` should NOT match.
    // `x` is a catch-all binding, so it should match.
    assert_val(
        r#"tag(Ok); 42 >> { Ok -> "tag", x -> "binding" }"#,
        s("binding"),
    );
}

#[test]
fn probe21_tag_pattern_matches_tag() {
    // When the scrutinee IS a tagged value, the tag pattern should match.
    assert_val(
        r#"tag(Ok); Ok(()) >> { Ok -> "tag", x -> "binding" }"#,
        s("tag"),
    );
}

#[test]
fn probe21_tag_with_payload_pattern() {
    // Tag with payload pattern: Ok(x) matches Ok(5) and binds x=5.
    assert_val(r#"tag(Ok); Ok(5) >> { Ok(x) -> x, _ -> 0 }"#, int(5));
}

// ── 8. Piped block sugar with chained pipe ──

#[test]
fn probe21_nested_pipe_sugar() {
    // `{ >> { in * 2 } }` is sugar for `{ in >> { in * 2 } }`.
    // 5 >> { in >> { in * 2 } } → inner block receives 5, returns 10.
    assert_val("5 >> { >> { in * 2 } }", int(10));
}

#[test]
fn probe21_nested_pipe_sugar_with_addition() {
    // `{ >> { + 1 } }` is sugar for `{ in >> { in + 1 } }`.
    assert_val("5 >> { >> { + 1 } }", int(6));
}

// ── 9. Complex branch with nested blocks ──

#[test]
fn probe21_branch_with_nested_block_in_arm() {
    // Ok(5) >> { Ok(x) -> x >> { in * 2 }, _ -> 0 } should be 10.
    assert_val(
        "tag(Ok); Ok(5) >> { Ok(x) -> x >> { in * 2 }, _ -> 0 }",
        int(10),
    );
}

#[test]
fn probe21_branch_fallback_arm() {
    // Test the fallback arm when no tag matches.
    assert_val(
        "tag(Ok); tag(Err); Err(99) >> { Ok(x) -> x * 2, _ -> 0 }",
        int(0),
    );
}

// ── 10. Block sugar division ──

#[test]
fn probe21_block_sugar_div() {
    // `{ / 2 }` is sugar for `{ in / 2 }`.
    assert_val("10 >> { / 2 }", int(5));
}

#[test]
fn probe21_block_sugar_div_by_zero() {
    assert_error("10 >> { / 0 }", "division by zero");
}

// ── 11. Method call in block vs block sugar ──

#[test]
fn probe21_method_call_in_block() {
    // `[1,2,3] >> { in.len() }` — method call on the block input.
    assert_val("[1, 2, 3] >> { in.len() }", int(3));
}

#[test]
fn probe21_method_call_string_len() {
    assert_val(r#""hello" >> { in.char_len() }"#, int(5));
}

// ── 12. Block with trailing semicolon ──

#[test]
fn probe21_trailing_semicolon_returns_unit() {
    // A trailing semicolon makes the block return `()`.
    assert_val("1 >> { in + 1; }", U);
}

#[test]
fn probe21_trailing_semicolon_bare_block() {
    // Even a simple literal with trailing semicolon returns unit.
    assert_val("1 >> { 42; }", U);
}

// ── 13. Additional tricky interactions ──

#[test]
fn probe21_block_sugar_mul() {
    // `{ * 2 }` is sugar for `{ in * 2 }`.
    assert_val("3 >> { * 2 }", int(6));
}

#[test]
fn probe21_block_sugar_plus() {
    // `{ + 10 }` is sugar for `{ in + 10 }`.
    assert_val("5 >> { + 10 }", int(15));
}

#[test]
fn probe21_minus_block_is_ambiguous() {
    // `{ -x }` is explicitly rejected as ambiguous.
    assert_error("5 >> { -1 }", "ambiguous");
}

#[test]
fn probe21_explicit_in_minus() {
    // `{ in - 1 }` is unambiguous subtraction.
    assert_val("5 >> { in - 1 }", int(4));
}

#[test]
fn probe21_block_sugar_chained() {
    // Chaining block sugar: 5 >> { + 1 } >> { * 2 } → 12.
    assert_val("5 >> { + 1 } >> { * 2 }", int(12));
}

#[test]
fn probe21_block_sugar_comparison_in_branch() {
    // Using comparison sugar inside a pipe, then branching on the result.
    assert_val(
        r#"5 >> { > 3 } >> { true -> "big", false -> "small" }"#,
        s("big"),
    );
}

#[test]
fn probe21_empty_block_returns_unit() {
    // `{}` is a block that returns `()`.
    assert_val("42 >> {}", U);
}

#[test]
fn probe21_branch_with_literal_int_pattern() {
    // Integer literal as branch pattern.
    assert_val(r#"42 >> { 42 -> "match", _ -> "no match" }"#, s("match"));
}

#[test]
fn probe21_branch_with_literal_string_pattern() {
    // String literal as branch pattern.
    assert_val(
        r#""hello" >> { "hello" -> "match", _ -> "no match" }"#,
        s("match"),
    );
}

#[test]
fn probe21_branch_with_negative_literal() {
    // Negative integer literal as branch pattern.
    assert_val(r#"-1 >> { -1 -> "neg one", _ -> "other" }"#, s("neg one"));
}

#[test]
fn probe21_branch_with_unit_pattern() {
    // `() -> ...` as a branch pattern.
    assert_val(r#"() >> { () -> "unit" }"#, s("unit"));
}

#[test]
fn probe21_block_sugar_range_chain() {
    // Chaining range: create a range and then use it.
    // `5 >> { ..10 }` gives (start=5, end=10)
    // Then we can access the struct fields.
    assert_val("5 >> { ..10 } >> { in.start }", int(5));
}

#[test]
fn probe21_block_sugar_range_end_field() {
    assert_val("5 >> { ..10 } >> { in.end }", int(10));
}

// ═══════════════════════════════════════════════════════════════════
// probe21b — Number literal parsing edge cases and numeric operations
// ═══════════════════════════════════════════════════════════════════

// ── 1. Negative number edge cases ──

#[test]
fn probe21b_neg_zero_int() {
    // -0 is UnaryMinus(Int(0)) → Int(0). Display: "0".
    assert_val("-0", int(0));
}

#[test]
fn probe21b_neg_zero_float() {
    // -0.0 is UnaryMinus(Float(0.0)) → Float(-0.0).
    // Display uses "{}.0" for whole floats; -0.0 should display as "-0.0".
    assert_val("-0.0", float(-0.0));
}

#[test]
fn probe21b_double_negation() {
    // --1 : parser sees Minus, then inside operand parsing sees another Minus.
    // This produces UnaryMinus(UnaryMinus(Int(1))) = 1.
    // DESIGN.md doesn't explicitly ban double negation, so this should work.
    assert_val("--1", int(1));
}

// ── 2. Number followed by dots ──

#[test]
fn probe21b_int_range() {
    // 1..3 is range sugar → (start=1, end=3)
    assert_output("1..3", "(start=1, end=3)");
}

#[test]
fn probe21b_float_range() {
    // 1.0..3.0 — lexer reads 1.0, then DotDot, then 3.0.
    // Should produce a range of floats.
    assert_output("1.0..3.0", "(start=1.0, end=3.0)");
}

#[test]
fn probe21b_int_spread_error() {
    // 1... — lexer reads Int(1), then Spread (...).
    // Spread after a literal is not valid syntax outside struct/array construction.
    assert_error("1...", "");
}

// ── 3. Hexadecimal negation ──

#[test]
fn probe21b_neg_hex() {
    // -0xFF → UnaryMinus(Int(255)) → Int(-255)
    assert_val("-0xFF", int(-255));
}

#[test]
fn probe21b_hex_add() {
    // 0xFF + 1 = 255 + 1 = 256
    assert_val("0xFF + 1", int(256));
}

// ── 4. Large numbers ──

#[test]
fn probe21b_hex_overflow_u64_max() {
    // 0xFFFFFFFFFFFFFFFF is u64::MAX which overflows i64.
    // i64::from_str_radix should fail with an error.
    assert_error("0xFFFFFFFFFFFFFFFF", "hex");
}

#[test]
fn probe21b_hex_i64_max() {
    // 0x7FFFFFFFFFFFFFFF = i64::MAX = 9223372036854775807
    assert_val("0x7FFFFFFFFFFFFFFF", int(9223372036854775807));
}

// ── 4b. Binary literals ──

#[test]
fn binary_literal_basic() {
    assert_val("0b1010", int(10));
}

#[test]
fn binary_literal_zero() {
    assert_val("0b0", int(0));
}

#[test]
fn binary_literal_one() {
    assert_val("0b1", int(1));
}

#[test]
fn binary_literal_byte_size() {
    assert_val("0b11111111", int(255));
}

#[test]
fn binary_literal_with_underscores() {
    assert_val("0b1111_0000", int(240));
}

#[test]
fn binary_literal_neg() {
    assert_val("-0b1010", int(-10));
}

#[test]
fn binary_literal_add() {
    assert_val("0b1010 + 0b0101", int(15));
}

#[test]
fn binary_literal_no_digits_error() {
    assert_error("0b", "expected binary digits");
}

// ── 5. Underscore in number edge cases ──

#[test]
fn probe21b_bare_underscore() {
    // `_` is Token::Underscore → parsed as Ident("_").
    // At runtime, `_` is not bound to anything → error.
    assert_error("_", "");
}

#[test]
fn probe21b_trailing_underscore_number() {
    // `1_` — the lexer's lex_number consumes the trailing underscore
    // (the digit loop accepts '_'). So it becomes Int(1), then Eof.
    assert_val("1_", int(1));
}

#[test]
fn probe21b_underscore_prefix_ident() {
    // `_1` — starts with '_', followed by alphanumeric → Ident("_1").
    // Not a number. Unbound → error.
    assert_error("_1", "undefined");
}

// ── 6. Float precision ──

#[test]
fn probe21b_float_point_one_plus_point_two() {
    // 0.1 + 0.2 in IEEE 754 is 0.30000000000000004, not 0.3.
    // Display for non-whole floats uses "{}".
    assert_val("0.1 + 0.2", float(0.30000000000000004));
}

#[test]
fn probe21b_float_equality_classic() {
    // 0.1 + 0.2 == 0.3 should be false (floating point imprecision).
    assert_val("0.1 + 0.2 == 0.3", F);
}

// ── 7. Negative float operations ──

#[test]
fn probe21b_neg_float_add() {
    // -1.5 + 2.5 = 1.0
    assert_val("-1.5 + 2.5", float(1.0));
}

#[test]
fn probe21b_neg_float_mul() {
    // -1.5 * -2.0 = 3.0
    // Parser: UnaryMinus is prefix, * is infix.
    // So: BinOp(Mul, UnaryMinus(Float(1.5)), UnaryMinus(Float(2.0))) = 3.0
    assert_val("-1.5 * -2.0", float(3.0));
}

// ── 8. Mixed int/float arithmetic is a type error ──

#[test]
fn probe21b_int_plus_float() {
    // 1 + 2.0 → type error: int and float are distinct types
    assert_error("1 + 2.0", "type error");
}

#[test]
fn probe21b_float_times_int() {
    // 2.0 * 3 → type error: float and int are distinct types
    assert_error("2.0 * 3", "type error");
}

#[test]
fn probe21b_int_div_int() {
    // 7 / 2 → integer division → 3 (truncation via checked_div)
    assert_val("7 / 2", int(3));
}

#[test]
fn probe21b_float_div_int() {
    // 7.0 / 2 → type error: float and int are distinct types
    assert_error("7.0 / 2", "type error");
}

// ── 9. Byte arithmetic ──

#[test]
fn probe21b_byte_eq_hex_escape() {
    // b'A' is Byte(65), b'\x41' is Byte(0x41 = 65). They should be equal.
    assert_val("b'A' == b'\\x41'", T);
}

// ── 10. Char comparison ──

#[test]
fn probe21b_char_lt() {
    // 'a' < 'b' — ASCII 97 < 98 → true
    assert_val("'a' < 'b'", T);
}

#[test]
fn probe21b_char_upper_lt_lower() {
    // 'A' < 'a' — ASCII 65 < 97 → true
    assert_val("'A' < 'a'", T);
}

// ═══════════════════════════════════════════════════════════════════
// probe22a — import/use, method calls, pipe+method, struct fields,
//            method chaining, pipe precedence
// ═══════════════════════════════════════════════════════════════════

// ── Area 1: import/use syntax ──

#[test]
fn probe22a_import_string_errors() {
    // import("something") should be a parse error — only identifiers allowed
    assert_parse_error(r#"import("something")"#, "expected identifier in import()");
}

#[test]
fn probe22a_use_errors() {
    // use(something) desugars to import(something) >> let(something)
    // which should error with "module not provided"
    assert_error("use(foo)", "module not provided");
}

// ── Area 2: Method calls piped vs normal ──

#[test]
fn probe22a_map_normal() {
    // [1, 2, 3].map{ * 2 } should be [2, 4, 6]
    assert_output("[1, 2, 3].map{ * 2 }", "[2, 4, 6]");
}

#[test]
fn probe22a_filter_normal() {
    // [1, 2, 3].filter{ > 1 } should be [2, 3]
    assert_output("[1, 2, 3].filter{ > 1 }", "[2, 3]");
}

#[test]
fn probe22a_get_no_arg_error() {
    // .get needs an argument — calling with no args should fail
    // [1, 2, 3].get() passes Unit to get, which expects an int index
    assert_error("[1, 2, 3].get()", "type error");
}

#[test]
fn probe22a_pipe_let_then_get() {
    // [1, 2, 3] >> let(arr); arr.get(0) — should work, returns 1
    assert_val("[1, 2, 3] >> let(arr); arr.get(0)", int(1));
}

// ── Area 3: Pipe into method call ──

#[test]
fn probe22a_pipe_into_get_method() {
    // 0 >> [1, 2, 3].get() — pipe 0 into get with no args.
    // Per eval_pipe MethodCall branch: extra_arg is Unit, so combined = lhs_val = 0.
    // Then eval_method(&[1,2,3], "get", 0) → get(0) → 1
    assert_val("0 >> [1, 2, 3].get()", int(1));
}

// ── Area 4: Edge cases with struct field functions ──

#[test]
fn probe22a_struct_field_function_call() {
    // let s = (f = { in + 1 }); s.f(5) — should be 6
    assert_val("let s = (f = { in + 1 }); s.f(5)", int(6));
}

#[test]
fn probe22a_pipe_into_struct_field_function() {
    // let s = (f = { in + 1 }); 10 >> s.f()
    // In eval_pipe MethodCall branch: recv = struct s, method = "f", extra_arg = Unit.
    // combined = lhs_val (since extra_arg is Unit) = 10.
    // Struct field "f" found → apply(closure, 10) → 10 + 1 = 11.
    assert_val("let s = (f = { in + 1 }); 10 >> s.f()", int(11));
}

// ── Area 5: Multiple method chaining ──

#[test]
fn probe22a_filter_then_map() {
    // [1, 2, 3, 4, 5].filter{ > 2 }.map{ * 10 } should be [30, 40, 50]
    assert_output("[1, 2, 3, 4, 5].filter{ > 2 }.map{ * 10 }", "[30, 40, 50]");
}

#[test]
fn probe22a_map_filter_len() {
    // [3, 1, 2].map{ * 2 }.filter{ > 3 }.len() should be 2
    assert_val("[3, 1, 2].map{ * 2 }.filter{ > 3 }.len()", int(2));
}

// ── Area 6: Pipe precedence with methods ──

#[test]
fn probe22a_pipe_into_block_with_map() {
    // [1, 2, 3] >> { in.map{ * 2 } } should be [2, 4, 6]
    assert_output("[1, 2, 3] >> { in.map{ * 2 } }", "[2, 4, 6]");
}

#[test]
fn probe22a_pipe_into_block_filter_map() {
    // [1, 2, 3] >> { in.filter{ > 1 }.map{ * 10 } } should be [20, 30]
    assert_output("[1, 2, 3] >> { in.filter{ > 1 }.map{ * 10 } }", "[20, 30]");
}

// ═══════════════════════════════════════════════════════════════════
// ROUND probe22b — complex integration tests exercising multiple
// feature interactions simultaneously
// ═══════════════════════════════════════════════════════════════════

// ── 1. Complete DESIGN.md example (safe_div with non-zero divisor) ──

#[test]
fn probe22b_design_example_safe_div_ok() {
    // The canonical example from DESIGN.md: safe_div with (10, 3).
    // 10 * 100 / 3 = 333 (integer division), wrapped in Ok, then unwrapped.
    assert_val(
        r#"tag(Ok);
tag(Err);
let safe_div = { in >> let(a, b);
  b == 0 >> {
    true -> Err("division by zero"),
    false -> a * 100 / b >> Ok
  }
};
(10, 3) >> safe_div >> {
  Ok(result) -> result,
  Err(_) -> 0
}"#,
        int(333),
    );
}

// ── 2. safe_div with zero divisor ──────────────────────────────────

#[test]
fn probe22b_design_example_safe_div_zero() {
    // Same safe_div but (10, 0) triggers the Err branch.
    // The final match on Err(_) returns -1.
    assert_val(
        r#"tag(Ok);
tag(Err);
let safe_div = { in >> let(a, b);
  b == 0 >> {
    true -> Err("division by zero"),
    false -> a * 100 / b >> Ok
  }
};
(10, 0) >> safe_div >> {
  Ok(result) -> result,
  Err(_) -> -1
}"#,
        int(-1),
    );
}

// ── 3. Higher-order function pipeline (compose) ────────────────────

#[test]
fn probe22b_higher_order_compose() {
    // compose takes (f, g) and returns a function that applies f then g.
    // double_then_add1: 5 -> 10 -> 11
    assert_val(
        r#"let compose = { in >> let(f, g); { in >> f >> g } };
let double = { in * 2 };
let add1 = { in + 1 };
(double, add1) >> compose >> let(double_then_add1);
5 >> double_then_add1"#,
        int(11),
    );
}

// ── 4. Tagged option type with map-like operation ──────────────────

#[test]
fn probe22b_tagged_option_map() {
    // Custom map_option that applies f inside Some, passes None through.
    // Some(5) mapped with { in * 2 } => Some(10).
    assert_output(
        r#"tag(Some);
tag(None);
let map_option = { in >> let(opt, f);
  opt >> {
    Some(x) -> Some(x >> f),
    None -> None(())
  }
};
(Some(5), { in * 2 }) >> map_option"#,
        "Some(10)",
    );
}

// ── 5. Array processing pipeline (filter, map, fold) ───────────────

#[test]
fn probe22b_array_filter_map_fold() {
    // [1,2,3,4,5].filter{ > 2 } => [3,4,5]
    // .map{ * 10 } => [30,40,50]
    // .fold(0, { in.acc + in.elem }) => 120
    assert_val(
        r#"[1, 2, 3, 4, 5]
  .filter{ > 2 }
  .map{ * 10 }
  .fold(0, { in.acc + in.elem })"#,
        int(120),
    );
}

// ── 6. Struct as a module pattern ──────────────────────────────────

#[test]
fn probe22b_struct_as_module() {
    // A struct with function fields used as a module.
    // math.square(5) = 25, math.cube(2) = 8, math.abs(-3) = 3.
    // 25 + 8 + 3 = 36.
    assert_val(
        r#"let math = (
  square = { in * in },
  cube = { in * in * in },
  abs = { in >> { x if x >= 0 -> x, x -> 0 - x } }
);
math.square(5) + math.cube(2) + math.abs(-3)"#,
        int(36),
    );
}

// ── 7. Array of mixed tags with branching map ──────────────────────

#[test]
fn probe22b_array_mixed_tags_branch_map() {
    // An array of tagged values mapped through a branching block.
    // Ok(1)->1, Err("bad")->-1, Warn("hmm")->0, Ok(2)->2.
    assert_output(
        r#"tag(Ok);
tag(Err);
tag(Warn);
let results = [Ok(1), Err("bad"), Warn("hmm"), Ok(2)];
results.map{
  Ok(x) -> x,
  Err(_) -> -1,
  Warn(_) -> 0
}"#,
        "[1, -1, 0, 2]",
    );
}

// ── 8. Complex destructuring with spread on named struct ───────────

#[test]
fn probe22b_named_struct_spread_destructure() {
    // Destructure (x=10, y=20, z=30) by pulling out x,
    // rest should be (y=20, z=30). Then build result struct.
    assert_output(
        r#"let point = (x=10, y=20, z=30);
point >> let(x=x, ...rest);
(x=x, sum_rest=rest.y + rest.z)"#,
        "(x=10, sum_rest=50)",
    );
}

// ── 9. Fibonacci-like via fold (iterative state accumulation) ──────

#[test]
fn probe22b_fibonacci_fold() {
    // Compute fib via fold over steps.
    // Starting state: (a=0, b=1)
    // Each step: (a, b) -> (b, a+b)
    // After 7 steps: (0,1)->(1,1)->(1,2)->(2,3)->(3,5)->(5,8)->(8,13)->(13,21)
    assert_output(
        r#"let steps = [1, 2, 3, 4, 5, 6, 7];
steps.fold((a=0, b=1), {
  let state = in;
  (a=state.acc.b, b=state.acc.a + state.acc.b)
})"#,
        "(a=13, b=21)",
    );
}

// ── 10. Chained branching with tags and map ────────────────────────

#[test]
fn probe22b_color_tag_values_map() {
    // Define color tags, a branching block mapping colors to hex values,
    // then map an array of tagged colors through it.
    // Red -> 0xFF0000 = 16711680, Green -> 0x00FF00 = 65280, Blue -> 0x0000FF = 255.
    assert_output(
        r#"tag(Red);
tag(Green);
tag(Blue);
let color_value = {
  Red -> 0xFF0000,
  Green -> 0x00FF00,
  Blue -> 0x0000FF,
  _ -> 0
};
[Red(()), Green(()), Blue(())].map(color_value)"#,
        "[16711680, 65280, 255]",
    );
}

// ═══════════════════════════════════════════════════════════════════
// Probe 23: Deep parser edge cases
// ═══════════════════════════════════════════════════════════════════

// ── 1. parse_expr_with_lhs: block sugar with various operators ──

#[test]
fn probe23_block_sugar_add_chain() {
    // { + 1 + 2 } desugars to { in + 1 + 2 }
    // in + 1 + 2 with left-assoc addition = (5 + 1) + 2 = 8
    assert_val("5 >> { + 1 + 2 }", int(8));
}

#[test]
fn probe23_block_sugar_mul_add_precedence() {
    // { * 2 + 1 } desugars to { in * 2 + 1 }
    // Precedence: (in * 2) + 1 = (5 * 2) + 1 = 11
    assert_val("5 >> { * 2 + 1 }", int(11));
}

#[test]
fn probe23_block_sugar_eq_chained_error() {
    // { == 5 == true } desugars to { in == 5 == true }
    // in == 5 produces a bool, then == true is a chained comparison.
    // Parser should reject chained comparisons.
    assert_error("5 >> { == 5 == true }", "chained comparison");
}

#[test]
fn probe23_block_sugar_comparison() {
    // { > 3 } desugars to { in > 3 }
    assert_val("5 >> { > 3 }", T);
}

#[test]
fn probe23_block_sugar_range() {
    // { .. 10 } desugars to { in .. 10 }
    // 3..10 produces a range struct (start=3, end=10)
    assert_output("3 >> { .. 10 }", "(start=3, end=10)");
}

#[test]
fn probe23_block_sugar_pipe_chain() {
    // { >> { + 1 } } desugars to { in >> { + 1 } }
    assert_val("5 >> { >> { + 1 } }", int(6));
}

// ── 2. Trailing comma in struct fields within call args ──

#[test]
fn probe23_trailing_comma_labeled_call_args() {
    // f(a=1, b=2,) — trailing comma in labeled struct call args
    assert_output("let f = { in }; f(a=1, b=2,)", "(a=1, b=2)");
}

#[test]
fn probe23_trailing_comma_positional_call_args() {
    // f(1, 2,) — trailing comma in positional call args
    // After first expr (1), comma -> second expr (2), comma -> RParen -> trailing comma
    // In parse_remaining_struct_fields: after 2, comma, RParen -> breaks out
    assert_output("let f = { in }; f(1, 2,)", "(1, 2)");
}

#[test]
fn probe23_trailing_comma_single_arg() {
    // f(42,) — trailing comma with single arg; should pass 42 not a struct
    assert_val("let f = { in }; f(42,)", int(42));
}

// ── 3. Spread in call args ──

#[test]
fn probe23_spread_in_call_args() {
    // f(...s) — spread a struct into call args
    assert_output("let s = (a=1, b=2); let f = { in }; f(...s)", "(a=1, b=2)");
}

#[test]
fn probe23_spread_with_extra_fields() {
    // f(...s, c=3) — spread plus additional labeled field
    assert_output(
        "let s = (a=1, b=2); let f = { in }; f(...s, c=3)",
        "(a=1, b=2, c=3)",
    );
}

// ── 4. Empty array operations ──

#[test]
fn probe23_empty_array_map() {
    assert_output("[].map{ * 2 }", "[]");
}

#[test]
fn probe23_empty_array_filter() {
    assert_output("[].filter{ > 0 }", "[]");
}

#[test]
fn probe23_empty_array_concat_left() {
    assert_output("[] + [1, 2]", "[1, 2]");
}

#[test]
fn probe23_empty_array_concat_right() {
    assert_output("[1, 2] + []", "[1, 2]");
}

#[test]
fn probe23_empty_array_concat_both() {
    assert_output("[] + []", "[]");
}

#[test]
fn probe23_empty_array_eq() {
    assert_val("[] == []", T);
}

#[test]
fn probe23_array_neq_empty() {
    assert_val("[1] != []", T);
}

// ── 5. Deeply nested parentheses ──

#[test]
fn probe23_deeply_nested_parens() {
    assert_val("(((((1)))))", int(1));
}

#[test]
fn probe23_nested_parens_arithmetic() {
    assert_val("((1 + 2)) * ((3))", int(9));
}

// ── 6. Spread in array context ──

#[test]
fn probe23_spread_in_array_literal_error() {
    // Array literals don't support spread — `...` is for structs.
    // `[0, ...a]` should produce a parse error since `...` is not a valid prefix.
    assert_error("let a = [1, 2]; [0, ...a]", "unexpected token");
}

// ── 7. Method call on struct field that is not a function ──

#[test]
fn probe23_method_call_non_function_field() {
    // (len=42).len() — len is 42, not a function.
    // The parser parses .len() as MethodCall. Eval sees struct with `len` field,
    // extracts value 42, then tries apply(42, ()) — error: cannot call non-function.
    assert_error("(len=42).len()", "cannot call");
}

// ── 8. Semicolons in array literals ──

#[test]
fn probe23_semicolon_in_array_literal_error() {
    // BUG-66 fix: semicolons are not consumed inside array elements
    assert_error("[1; 2]", "expected");
}

#[test]
fn probe23_semicolon_in_array_literal_multi_error() {
    // BUG-66 fix: semicolons are not consumed inside array elements
    assert_error("[1; 2; 3]", "expected");
}

#[test]
fn probe23_semicolon_in_array_with_comma_error() {
    // BUG-66 fix: semicolons not consumed in array element
    assert_error("[1; 2, 3]", "expected");
}

#[test]
fn bug66_semicolon_in_struct_second_field_error() {
    // BUG-66 fix: semicolons are not consumed inside struct fields (after the first)
    // The first expression in (...) uses parse_expr(0) which can contain semicolons.
    // But subsequent fields use SEMI_L + 1 so semicolons are blocked.
    assert_error("(1, 2; 3)", "expected");
}

#[test]
fn bug66_semicolon_in_call_args_error() {
    // BUG-66 fix: semicolons are not consumed inside call args
    assert_error("let f = { in }; f(1; 2)", "expected");
}

#[test]
fn bug66_semicolon_in_labeled_field_error() {
    // BUG-66 fix: semicolons are not consumed inside labeled struct fields
    assert_error("(a=1; 2)", "expected");
}

// ── 9. Consecutive semicolons ──

#[test]
fn probe23_double_semicolon() {
    // 1;; 2 — after first `;`, parse_semicolon_rhs calls parse_expr
    // which calls parse_prefix and encounters `;` — unexpected token.
    assert_error("1;; 2", "unexpected token");
}

#[test]
fn probe23_triple_semicolon() {
    assert_error("1;;; 2", "unexpected token");
}

// ── 10. Let destructuring with labeled pattern then pipe ──

#[test]
fn probe23_let_destructure_labeled_pipe() {
    // (x=1, y=2) >> let(x=a, y=b); a + b should be 3
    assert_val("(x=1, y=2) >> let(x=a, y=b); a + b", int(3));
}

#[test]
fn probe23_let_destructure_labeled_pipe_more() {
    // Destructure and use in further computation
    assert_val("(x=10, y=3) >> let(x=a, y=b); a * b", int(30));
}

// ── Additional edge cases discovered during analysis ──

#[test]
fn probe23_block_sugar_div() {
    // { / 2 } desugars to { in / 2 }
    assert_val("10 >> { / 2 }", int(5));
}

#[test]
fn probe23_block_sugar_noteq() {
    // { != 5 } desugars to { in != 5 }
    assert_val("3 >> { != 5 }", T);
}

#[test]
fn probe23_block_sugar_lteq() {
    // { <= 5 } desugars to { in <= 5 }
    assert_val("5 >> { <= 5 }", T);
}

#[test]
fn probe23_block_sugar_gteq() {
    // { >= 5 } desugars to { in >= 5 }
    assert_val("5 >> { >= 5 }", T);
}

#[test]
fn probe23_expr_with_lhs_semicolon() {
    // Test that parse_expr_with_lhs handles semicolons identically to parse_expr.
    // { + 1; + 2 } desugars to { in + 1; in + 2 }
    // Wait — semicolon scoping in blocks: the block body is a single expression.
    // { + 1; + 2 } => the sugar only injects `in` before the first operator.
    // So it becomes { in + 1; + 2 } where `; + 2` is continuation.
    // But `+ 2` alone is not valid prefix. Let's test what happens.
    // Actually: parse_sugar_body calls parse_expr_with_lhs(in_expr, 0).
    // With min_bp=0, semicolons are consumed. So `in + 1` is the lhs of `;`.
    // Then parse_semicolon_rhs parses `+ 2` which starts with `+` — not valid prefix.
    assert_error("5 >> { + 1; + 2 }", "unexpected token");
}

#[test]
fn probe23_trailing_semicolon_in_block_is_closure() {
    // { 42; } is a Block (closure). `{...}` always creates a closure.
    // The trailing semicolon inside the block body produces `42; ()` => Unit.
    // But the Block itself is a function value. You have to call it to get the result.
    assert_val("{ 42; }(())", U);
}

#[test]
fn probe23_block_is_always_closure() {
    // { expr } is a closure that must be called. It's not auto-evaluated.
    assert_val("{ 42 }(())", int(42));
}

#[test]
fn probe23_empty_parens_unit() {
    // () is unit
    assert_val("()", U);
}

#[test]
fn probe23_nested_unit() {
    // (()) is grouping around unit
    assert_val("(())", U);
}

#[test]
fn probe23_spread_call_only_spread() {
    // f(...s) where s is a labeled struct — spread is the only arg
    assert_output("let s = (x=10); let f = { in }; f(...s)", "(x=10)");
}

#[test]
fn probe23_method_call_on_empty_struct_field() {
    // Access a nonexistent method on a struct
    assert_error("(a=1).nonexistent()", "no method");
}

#[test]
fn probe23_parse_expr_with_lhs_postfix_call() {
    // Test postfix call in parse_expr_with_lhs context.
    // { + 1 } is sugar, but can we chain postfix calls?
    // Not directly useful but tests the postfix branch in parse_expr_with_lhs.
    // We test method call: { .len() } desugars to... no, `.len()` is not a binary op.
    // Instead test: { * 2 } piped through — ensure postfix in parse_expr_with_lhs via rhs.
    assert_output("[1, 2, 3].map{ * 2 }", "[2, 4, 6]");
}

#[test]
fn probe23_parse_expr_with_lhs_dotdot_range() {
    // { .. 5 } sugar — range operator in parse_expr_with_lhs
    // 1..5 => (start=1, end=5) range struct
    assert_output("1 >> { .. 5 }", "(start=1, end=5)");
}

#[test]
fn probe23_parse_expr_with_lhs_dotdot_range_access() {
    // Access .start and .end fields of range produced by sugar
    assert_val("1 >> { .. 5 } >> { in.start }", int(1));
}

// ═══════════════════════════════════════════════════════════════════
// Probe 24: Final deep probe — obscure and unlikely-to-be-tested scenarios
// ═══════════════════════════════════════════════════════════════════

#[test]
fn probe24_pipe_into_tag_constructor() {
    // `42 >> tag(Ok)` — tag(Ok) desugars to Pipe(NewTag, Let{Ok, Ident(Ok)}).
    // As RHS of pipe, it falls into the catch-all case: eval the rhs expression
    // (which evaluates the inner Pipe, creating a TagConstructor and binding it to Ok,
    // then returning the TagConstructor). Then apply(TagConstructor, 42) => Ok(42).
    assert_output("42 >> tag(Ok)", "Ok(42)");
}

#[test]
fn probe24_method_call_with_block_and_paren_args() {
    // `[1, 2, 3].fold(0){ in.acc + in.elem }` — spec says f(args){body} calls f with args,
    // then calls the result with the body. But .fold(0) expects (init, function) as a
    // 2-element struct. With just 0, fold will error "fold: expected (init, function)".
    // The parser makes MethodCall{fold, 0} then Call(MethodCall, block).
    // MethodCall{fold, 0} is evaluated first and fails.
    assert_error("[1, 2, 3].fold(0){ in.acc + in.elem }", "fold");
}

#[test]
fn probe24_call_with_array_arg() {
    // `let f = { in }; f[1, 2, 3]` — f[...] passes an array to f.
    // The parser sees f followed by [1,2,3] and produces Call(f, Array([1,2,3])).
    // apply(closure, [1,2,3]) passes the array as `in`, and `{ in }` returns it.
    assert_output("let f = { in }; f[1, 2, 3]", "[1, 2, 3]");
}

#[test]
fn probe24_let_destructure_rest_only() {
    // `(a=1, b=2, c=3) >> let(...rest); rest` — rest should capture everything.
    // bind_pattern with just a rest field should capture all fields.
    assert_output("(a=1, b=2, c=3) >> let(...rest); rest", "(a=1, b=2, c=3)");
}

#[test]
fn probe24_let_array_destructure_rest_only() {
    // `[1, 2, 3] >> let[...rest]; rest` — rest should capture entire array.
    assert_output("[1, 2, 3] >> let[...rest]; rest", "[1, 2, 3]");
}

#[test]
fn probe24_empty_struct_let_destructure_rest() {
    // `() >> let(...rest); rest` — rest captures nothing from empty struct (unit).
    // Unit is not a Struct, but bind_pattern may handle it.
    // If Unit goes through bind_pattern with a rest pattern, it likely errors
    // because bind_pattern expects a Struct. Let's see.
    assert_output("() >> let(...rest); rest", "()");
}

#[test]
fn probe24_positional_struct_field_access() {
    // `(10, 20, 30).1` should be 20.
    // Positional fields are stored as "0", "1", "2". Field access with .1 should match.
    assert_val("(10, 20, 30).1", int(20));
}

#[test]
fn probe24_tag_equality_same() {
    // `tag(Ok); Ok(1) == Ok(1)` should be true.
    assert_val("tag(Ok); Ok(1) == Ok(1)", T);
}

#[test]
fn probe24_tag_equality_different_payload() {
    // `tag(Ok); Ok(1) == Ok(2)` should be false.
    assert_val("tag(Ok); Ok(1) == Ok(2)", F);
}

#[test]
fn probe24_tag_equality_different_tags() {
    // `tag(Ok); tag(Err); Ok(1) == Err(1)` should be false (different tag IDs).
    assert_val("tag(Ok); tag(Err); Ok(1) == Err(1)", F);
}

#[test]
fn probe24_deeply_nested_pipe_chains() {
    // `1 >> { + 1 } >> { * 2 } >> { + 3 } >> { * 4 }`
    // = (((1+1)*2)+3)*4 = ((2*2)+3)*4 = (4+3)*4 = 7*4 = 28
    assert_val("1 >> { + 1 } >> { * 2 } >> { + 3 } >> { * 4 }", int(28));
}

#[test]
fn probe24_let_inside_guard() {
    // Can you use `let` inside a branch guard expression?
    // Guards use `parse_expr(bp::PIPE_R)` which should allow semicolons and let.
    // Actually PIPE_R = 7, and SEMI_L = 4, so semicolons are allowed in guards.
    // `(let y = x; y > 3)` — this is a grouped expression containing let and semicolon.
    // Let's see if it works.
    assert_val(
        r#"tag(Ok); Ok(5) >> { Ok(x) if (let y = x; y > 3) -> "big", _ -> "small" }"#,
        s("big"),
    );
}

#[test]
fn probe24_let_inside_guard_false() {
    // Same as above but the guard evaluates to false.
    assert_val(
        r#"tag(Ok); Ok(1) >> { Ok(x) if (let y = x; y > 3) -> "big", _ -> "small" }"#,
        s("small"),
    );
}

#[test]
fn probe24_spread_with_computed_fields() {
    // `let base = (a=1, b=2); let extended = (c=base.a + base.b, ...base); extended`
    // Should have c=3, a=1, b=2.
    assert_output(
        "let base = (a=1, b=2); let extended = (c=base.a + base.b, ...base); extended",
        "(c=3, a=1, b=2)",
    );
}

#[test]
fn probe24_chained_tag_constructors() {
    // `tag(Outer); tag(Inner); Outer(Inner(42))` — nested tags.
    // Inner(42) creates Tagged{Inner, 42}. Outer(Tagged{Inner, 42}) creates Tagged{Outer, Tagged{Inner, 42}}.
    assert_output(
        "tag(Outer); tag(Inner); Outer(Inner(42))",
        "Outer(Inner(42))",
    );
}

#[test]
fn probe24_branching_nested_tags() {
    // `Outer(Inner(42)) >> { Outer(Inner(x)) -> x, _ -> 0 }`
    // Tag patterns only support single-level: Tag(binding). `Inner(x)` in the binding
    // position is not valid syntax — the parser expects an Ident or _ inside Tag().
    // This should be a parse error.
    // The parser reads `Inner` as binding name, then expects `)` but sees `(`.
    assert_parse_error(
        "tag(Outer); tag(Inner); Outer(Inner(42)) >> { Outer(Inner(x)) -> x, _ -> 0 }",
        "expected RParen, got LParen",
    );
}

#[test]
fn probe24_array_of_functions_get() {
    // `let fns = [{ + 1 }, { * 2 }, { + 3 }]; fns.get(0)` — should return `<function>`.
    assert_output(
        "let fns = [{ + 1 }, { * 2 }, { + 3 }]; fns.get(0)",
        "<function>",
    );
}

#[test]
fn probe24_array_of_functions_apply_via_let() {
    // Retrieve the function from the array, then pipe into it.
    // Direct `5 >> fns.get(1)` fails because pipe prepends 5 to get's args.
    assert_val(
        "let fns = [{ + 1 }, { * 2 }, { + 3 }]; let f = fns.get(1); 5 >> f",
        int(10),
    );
}

#[test]
fn probe24_array_of_functions_pipe_bug() {
    // BUG: `5 >> fns.get(1)` — pipe into method call prepends 5 to get's args,
    // producing (0=5, 1=1) which is a struct, not an integer. get errors.
    // This is a semantic mismatch: the user intends to pipe into the RESULT of get,
    // not to prepend to get's args.
    assert_error(
        "let fns = [{ + 1 }, { * 2 }, { + 3 }]; 5 >> fns.get(1)",
        "type error",
    );
}

#[test]
fn probe24_array_len_zero_arg() {
    // `[1, 2, 3].len()` — .len() passes Unit. eval_array_method("len", _) ignores arg.
    assert_val("[1, 2, 3].len()", int(3));
}

#[test]
fn probe24_pipe_into_tag_then_use() {
    // More complete test: tag(Ok) as statement, then use Ok as constructor.
    // Ensure the binding from tag() persists through semicolons.
    assert_val(
        "tag(Ok); tag(Err); Ok(42) >> { Ok(x) -> x + 1, Err(_) -> 0 }",
        int(43),
    );
}

#[test]
fn probe24_fold_proper_syntax() {
    // Proper fold: [1,2,3].fold(0, { in.acc + in.elem })
    // fold expects (init, function) as a 2-element struct.
    assert_val("[1, 2, 3].fold(0, { in.acc + in.elem })", int(6));
}

// ── Ternary sugar: { a | b } ─────────────────────────────────────

#[test]
fn ternary_true_branch() {
    assert_val(r#"true >> { "yes" | "no" }"#, s("yes"));
}

#[test]
fn ternary_false_branch() {
    assert_val(r#"false >> { "yes" | "no" }"#, s("no"));
}

#[test]
fn ternary_with_arithmetic() {
    assert_val("true >> { 1 + 2 | 3 * 4 }", int(3));
    assert_val("false >> { 1 + 2 | 3 * 4 }", int(12));
}

#[test]
fn ternary_with_pipes() {
    assert_val("true >> { 1 >> { + 1 } | 0 }", int(2));
    assert_val("false >> { 1 >> { + 1 } | 0 }", int(0));
}

#[test]
fn ternary_nested() {
    // true branch evaluates the inner ternary
    assert_val(r#"true >> { false >> { "a" | "b" } | "c" }"#, s("b"));
    assert_val(r#"false >> { "a" | true >> { "b" | "c" } }"#, s("b"));
}

#[test]
fn ternary_in_available() {
    // `in` is the bool input — true in true branch, false in false branch
    assert_val("true >> { in | false }", T);
    assert_val("false >> { true | in }", F);
}

#[test]
fn ternary_short_circuits() {
    // Only the taken branch is evaluated; the other should NOT cause a runtime error
    assert_val(r#"true >> { 1 | 1 / 0 }"#, int(1));
    assert_val(r#"false >> { 1 / 0 | 1 }"#, int(1));
}

#[test]
fn ternary_non_bool_error() {
    assert_error(r#"42 >> { "a" | "b" }"#, "");
}

#[test]
fn ternary_with_comparison() {
    assert_val(r#"3 > 0 >> { "positive" | "non-positive" }"#, s("positive"));
    assert_val(
        r#"0 > 0 >> { "positive" | "non-positive" }"#,
        s("non-positive"),
    );
}

#[test]
fn ternary_returns_value() {
    // Ternary result can be used in further computation
    assert_val("1 + (true >> { 10 | 20 })", int(11));
    assert_val("1 + (false >> { 10 | 20 })", int(21));
}

#[test]
fn ternary_with_let_in_branch() {
    // Semicolons work inside branches
    assert_val("true >> { let x = 10; x + 1 | 0 }", int(11));
    assert_val("false >> { 0 | let y = 20; y + 1 }", int(21));
}

#[test]
fn ternary_with_tags() {
    assert_output("tag(Ok); true >> { Ok(1) | Ok(0) }", "Ok(1)");
    assert_output("tag(Ok); false >> { Ok(1) | Ok(0) }", "Ok(0)");
}

#[test]
fn ternary_unit_branches() {
    assert_val("true >> { () | () }", U);
}

#[test]
fn ternary_array_branches() {
    assert_output("true >> { [1, 2] | [3, 4] }", "[1, 2]");
    assert_output("false >> { [1, 2] | [3, 4] }", "[3, 4]");
}

#[test]
fn ternary_struct_branches() {
    assert_output("true >> { (a=1, b=2) | (a=3, b=4) }", "(a=1, b=2)");
}

#[test]
fn ternary_in_pipeline() {
    // Ternary block used in a pipeline
    assert_val(r#"1 > 0 >> { "pos" | "neg" } >> { in.char_len() }"#, int(3));
}

// ── Unused Binding Warnings ─────────────────────────────────────

#[test]
fn warning_unused_binding() {
    assert_warnings("let x = 1; 2", &["unused binding 'x'"]);
}

#[test]
fn warning_no_warning_when_used() {
    assert_no_warnings("let x = 1; x");
}

#[test]
fn warning_underscore_prefix_suppresses() {
    // _x suppresses the unused warning
    assert_no_warnings("let _x = 1; 2");
}

#[test]
fn warning_bare_discard_no_warning() {
    // _ discard never warns
    assert_no_warnings("let _ = 1; 2");
}

#[test]
fn warning_multiple_unused() {
    // Warnings are emitted in binding order (earliest first)
    assert_warnings("let x = 1; let y = 2; 3", &["x", "y"]);
}

#[test]
fn warning_shadowed_used() {
    // x is shadowed; the second x is used, so no warning for it.
    // But the first x IS unused and should produce a warning.
    assert_warnings("let x = 1; let x = 2; x", &["x"]);
}

#[test]
fn warning_tag_binding_used() {
    // tag(Ok) binds Ok — used in the branch
    assert_no_warnings("tag(Ok); 1 >> Ok >> { Ok(x) -> x }");
}

#[test]
fn warning_used_in_interpolation() {
    assert_no_warnings(r#"let name = "world"; "{name}""#);
}

// ═══════════════════════════════════════════════════════════════════
// Bug-finding round 2 — new bugs found
// ═══════════════════════════════════════════════════════════════════

// ── BUG-67: Shadowed unused binding should warn ─────────────────────

#[test]
fn bug67_shadowed_both_unused() {
    // Both x bindings are unused (y is the result).
    // Should warn about both.
    assert_warnings("let x = 1; let x = 2; 3", &["x", "x"]);
}

// ── BUG-68: Int-to-float comparison should be an error ──────────────

#[test]
fn bug68_int_float_comparison_error() {
    // Comparing int to float should be a type error, not silently coerce.
    assert_error("1 == 1.0", "type error");
}

#[test]
fn bug68_float_int_comparison_error() {
    assert_error("1.0 == 1", "type error");
}

#[test]
fn bug68_int_float_lt_error() {
    assert_error("1 < 2.0", "type error");
}

// ── BUG-69: TagConstructor equality comparison missing ───────────────

#[test]
fn bug69_tag_constructor_eq() {
    // Two references to the same tag constructor should be equal.
    assert_val("tag(A); A == A", T);
}

#[test]
fn bug69_tag_constructor_neq_different() {
    // Different tag constructors should not be equal.
    assert_val("tag(A); tag(B); A == B", F);
}

#[test]
fn bug69_tag_constructor_neq() {
    assert_val("tag(A); tag(B); A != B", T);
}

// ── BUG-70: TagConstructor not matchable in branch patterns ─────────

#[test]
fn bug70_tag_constructor_branch_match() {
    // A tag constructor (not a tagged value) should be matchable.
    // `None` is a TagConstructor. Pattern `None ->` should match it.
    assert_val(
        r#"tag(None); tag(Some); None >> { None -> "none", Some -> "some", _ -> "other" }"#,
        s("none"),
    );
}

#[test]
fn bug70_tag_constructor_branch_no_match() {
    // Some (a different constructor) should not match None.
    assert_val(
        r#"tag(None); tag(Some); Some >> { None -> "none", _ -> "other" }"#,
        s("other"),
    );
}

// ── BUG-71: i64::MIN literal rejected ───────────────────────────────

#[test]
fn bug71_i64_min_literal() {
    // -9223372036854775808 is i64::MIN, a valid value.
    // The lexer parses 9223372036854775808 as positive first (overflow),
    // then UnaryMinus is applied. Should handle this edge case.
    assert_val("-9223372036854775808", int(i64::MIN));
}

#[test]
fn bug71_i64_min_in_expression() {
    // Should also work in expressions.
    assert_val("-9223372036854775808 + 1", int(i64::MIN + 1));
}

// ── BUG-72: string.len() should be byte_len()/char_len() ───────────

#[test]
fn bug72_string_byte_len() {
    // "héllo" is 6 bytes in UTF-8 (é is 2 bytes)
    assert_val(r#""héllo".byte_len()"#, int(6));
}

#[test]
fn bug72_string_char_len() {
    // "héllo" has 5 characters
    assert_val(r#""héllo".char_len()"#, int(5));
}

#[test]
fn bug72_string_len_removed() {
    // .len() should no longer exist on strings
    assert_error(r#""hello".len()"#, "no method");
}

#[test]
fn bug72_builtin_len_string_removed() {
    // strings use .char_len(), not .len()
    assert_error(r#""hello".len()"#, "no method");
}

// ── Value::Debug string escaping ──────────────────────────────────

#[test]
fn debug_string_escaping() {
    // Verify that Value::Debug escapes special characters
    let result = nana::run(r#""hello\nworld""#).unwrap();
    let debug_str = format!("{:?}", result);
    assert_eq!(debug_str, r#""hello\nworld""#);
}

#[test]
fn debug_string_escaping_backslash() {
    let result = nana::run(r#""a\\b""#).unwrap();
    let debug_str = format!("{:?}", result);
    assert_eq!(debug_str, r#""a\\b""#);
}

#[test]
fn debug_string_escaping_quote() {
    let result = nana::run(r#""a\"b""#).unwrap();
    let debug_str = format!("{:?}", result);
    assert_eq!(debug_str, r#""a\"b""#);
}

#[test]
fn debug_string_escaping_tab() {
    let result = nana::run(r#""a\tb""#).unwrap();
    let debug_str = format!("{:?}", result);
    assert_eq!(debug_str, r#""a\tb""#);
}

// ── Guard-only `if` sugar ─────────────────────────────────────────

#[test]
fn if_sugar_basic() {
    assert_val(r#"3 >> { if in < 4 -> "small", if in >= 4 -> "big" }"#, s("small"));
    assert_val(r#"5 >> { if in < 4 -> "small", if in >= 4 -> "big" }"#, s("big"));
}

#[test]
fn if_sugar_with_wildcard() {
    assert_val(r#"5 >> { if in == 5 -> "five", _ -> "other" }"#, s("five"));
    assert_val(r#"3 >> { if in == 5 -> "five", _ -> "other" }"#, s("other"));
}

#[test]
fn if_sugar_multiple_guards() {
    assert_val(
        r#"7 >> { if in < 0 -> "neg", if in < 5 -> "small", if in < 10 -> "med", _ -> "big" }"#,
        s("med"),
    );
}

#[test]
fn if_sugar_as_lambda() {
    assert_val(
        r#"let classify = { if in > 0 -> "pos", if in < 0 -> "neg", _ -> "zero" }; -3 >> classify"#,
        s("neg"),
    );
}

#[test]
fn if_sugar_non_exhaustive() {
    assert_error(r#"5 >> { if in == 3 -> "three" }"#, "no arm matched");
}

#[test]
fn if_sugar_guard_not_bool() {
    assert_error(r#"5 >> { if in + 1 -> "bad" }"#, "guard must be boolean");
}

#[test]
fn if_sugar_mixed_with_normal_arms() {
    assert_val(r#"5 >> { 1 -> "one", if in > 3 -> "big", _ -> "other" }"#, s("big"));
    assert_val(r#"1 >> { 1 -> "one", if in > 3 -> "big", _ -> "other" }"#, s("one"));
    assert_val(r#"2 >> { 1 -> "one", if in > 3 -> "big", _ -> "other" }"#, s("other"));
}

// ── Default arm sugar ─────────────────────────────────────────────

#[test]
fn default_arm_basic() {
    assert_val(r#"5 >> { 1 -> "one", 2 -> "two", "other" }"#, s("other"));
    assert_val(r#"1 >> { 1 -> "one", 2 -> "two", "other" }"#, s("one"));
}

#[test]
fn default_arm_with_expression() {
    assert_val(r#"5 >> { 1 -> 100, in * 2 }"#, int(10));
}

#[test]
fn default_arm_with_if_sugar() {
    assert_val(r#"3 >> { if in < 0 -> "neg", if in == 0 -> "zero", "pos" }"#, s("pos"));
    assert_val(r#"-1 >> { if in < 0 -> "neg", if in == 0 -> "zero", "pos" }"#, s("neg"));
    assert_val(r#"0 >> { if in < 0 -> "neg", if in == 0 -> "zero", "pos" }"#, s("zero"));
}

#[test]
fn default_arm_trailing_comma() {
    assert_val(r#"5 >> { 1 -> "one", "default", }"#, s("default"));
}

#[test]
fn default_arm_as_lambda() {
    assert_val(
        r#"let f = { if in > 0 -> "pos", "non-pos" }; -3 >> f"#,
        s("non-pos"),
    );
}

#[test]
fn default_arm_with_nested_block() {
    // Default expression contains a nested branching block
    assert_val(
        r#"5 >> { 1 -> "one", in > 0 >> { "pos" | "neg" } }"#,
        s("pos"),
    );
}

#[test]
fn default_arm_with_function_call() {
    // Default expression has commas inside function args
    assert_val(r#"5 >> { 1 -> and(true, false), and(true, true) }"#, T);
}

#[test]
fn default_arm_not_last_error() {
    // Default arm followed by more arms is an error
    assert_parse_error(
        r#"{ 1 -> "one", "default", 2 -> "two" }"#,
        "default arm must be the last arm",
    );
}

// ── Import / module tests ─────────────────────────────────────────

fn run_with_mods(source: &str, modules: &[(&str, Value)]) -> Value {
    nana::run_with_modules(source, modules)
        .unwrap_or_else(|e| panic!("program failed.\n  input: {source}\n  error: {e}"))
}

fn math_module() -> Value {
    Value::Struct(vec![
        ("pi".to_string(), Value::Int(3)),
        ("e".to_string(), Value::Int(2)),
    ])
}

#[test]
fn import_basic_struct() {
    let val = run_with_mods("import(math)", &[("math", math_module())]);
    assert_eq!(val.to_string(), "(pi=3, e=2)");
}

#[test]
fn use_sugar() {
    // use(math) is sugar for import(math) >> let(math)
    assert_val("let math = (pi=3, e=2); math.pi + math.e", int(5));
}

#[test]
fn import_use_field_access() {
    let val = run_with_mods("use(math); math.pi", &[("math", math_module())]);
    assert_eq!(val, int(3));
}

#[test]
fn import_same_module_twice() {
    // Importing the same module twice returns the same value
    assert_val("let m = (pi=3, e=2); let a = m; let b = m; a == b", T);
}

#[test]
fn import_module_not_provided() {
    assert_error("import(foo)", "module not provided: foo");
}

#[test]
fn import_no_modules_error() {
    // Using plain run() without modules should error on import
    assert_error("import(foo)", "module not provided");
}

#[test]
fn import_module_non_struct() {
    // Module value can be anything — not required to be a struct
    let val = run_with_mods(
        "import(greeting)",
        &[("greeting", Value::Str("hello world".to_string()))],
    );
    assert_eq!(val, s("hello world"));
}

#[test]
fn import_module_with_function() {
    // Module exports a function via a struct field
    let prelude = format!("{}", STD_PRELUDE);
    let double = nana::run_with_std(&format!("{}{{ in * 2 }}", prelude)).unwrap();
    let inc = nana::run_with_std(&format!("{}{{ in + 1 }}", prelude)).unwrap();
    let funcs = Value::Struct(vec![
        ("double".to_string(), double),
        ("inc".to_string(), inc),
    ]);
    let val = run_with_mods("use(funcs); 5 >> funcs.double >> funcs.inc", &[("funcs", funcs)]);
    assert_eq!(val, int(11));
}

#[test]
fn import_string_syntax_rejected() {
    // import("math") with a string should be a parse error
    assert_parse_error(r#"import("math")"#, "expected identifier in import()");
}

#[test]
fn imports_function_extracts_names() {
    let names = nana::imports("use(math); use(utils); import(math)").unwrap();
    assert_eq!(names, vec!["math".to_string(), "utils".to_string()]);
}

#[test]
fn imports_function_empty() {
    let names = nana::imports("1 + 2").unwrap();
    assert!(names.is_empty());
}

#[test]
fn imports_function_nested() {
    // Imports inside blocks are still collected
    let names = nana::imports("{ import(foo) }; import(bar)").unwrap();
    assert_eq!(names, vec!["foo".to_string(), "bar".to_string()]);
}

// ── String indexing methods ───────────────────────────────────────

#[test]
fn byte_get_ascii() {
    assert_val(r#""hello".byte_get(0)"#, byte(b'h'));
    assert_val(r#""hello".byte_get(4)"#, byte(b'o'));
}

#[test]
fn byte_get_multibyte() {
    // 'é' is 0xC3 0xA9 in UTF-8
    assert_val(r#""héllo".byte_get(0)"#, byte(b'h'));
    assert_val(r#""héllo".byte_get(1)"#, byte(0xC3));
    assert_val(r#""héllo".byte_get(2)"#, byte(0xA9));
    assert_val(r#""héllo".byte_get(3)"#, byte(b'l'));
}

#[test]
fn byte_get_out_of_bounds() {
    assert_error(r#""hello".byte_get(5)"#, "out of bounds");
    assert_error(r#""".byte_get(0)"#, "out of bounds");
}

#[test]
fn byte_get_negative() {
    assert_error(r#""hello".byte_get(-1)"#, "negative index");
}

#[test]
fn char_get_ascii() {
    assert_val(r#""hello".char_get(0)"#, ch('h'));
    assert_val(r#""hello".char_get(4)"#, ch('o'));
}

#[test]
fn char_get_multibyte() {
    assert_val(r#""héllo".char_get(0)"#, ch('h'));
    assert_val(r#""héllo".char_get(1)"#, ch('é'));
    assert_val(r#""héllo".char_get(2)"#, ch('l'));
}

#[test]
fn char_get_out_of_bounds() {
    assert_error(r#""hello".char_get(5)"#, "out of bounds");
    assert_error(r#""".char_get(0)"#, "out of bounds");
}

#[test]
fn char_get_negative() {
    assert_error(r#""hello".char_get(-1)"#, "negative index");
}

// ── Type constructor builtins ─────────────────────────────────────

#[test]
fn byte_constructor() {
    assert_val("byte(0)", byte(0));
    assert_val("byte(65)", byte(65));
    assert_val("byte(255)", byte(255));
}

#[test]
fn byte_auto_coercion() {
    // Int literals auto-coerce to byte when used in byte context
    assert_val("b'A' == 65", T);
    assert_val("b'A' != 66", T);
    assert_val("b'A' < 66", T);
    assert_val("b'Z' > 65", T);
}

#[test]
fn byte_constructor_out_of_range() {
    assert_error("byte(256)", "out of range");
    assert_error("byte(-1)", "out of range");
}

#[test]
fn int_constructor() {
    // int() is a literal constructor: IntLiteral → Int
    assert_val("int(42)", int(42));
    // conversions from other types use methods
    assert_error("int(3.14)", "type error");
    assert_error("int(true)", "type error");
}

#[test]
fn float_constructor() {
    // float() is a literal constructor: FloatLiteral → Float
    assert_val("float(3.14)", float(3.14));
    assert_val("float(0.0)", float(0.0));
    // int literals are not float literals
    assert_error("float(42)", "type error");
}

#[test]
fn char_constructor() {
    // char() constructs from an int literal code point
    assert_val("char(65)", ch('A'));
    assert_val("char(0)", ch('\0'));
    // byte literals are not int literals
    assert_error("char(b'z')", "type error");
}

#[test]
fn char_constructor_invalid() {
    assert_error("char(-1)", "negative value");
    assert_error("char(1114112)", "not a valid Unicode scalar value"); // 0x110000
    assert_error("char(55296)", "not a valid Unicode scalar value");  // 0xD800 surrogate
}

// ── Conversion methods ───────────────────────────────────────────

#[test]
fn int_to_float() {
    assert_val("42.to_float()", float(42.0));
    assert_val("(-3).to_float()", float(-3.0));
}

#[test]
fn int_to_byte() {
    assert_val("65.to_byte()", byte(65));
    assert_error("256.to_byte()", "out of range");
    assert_error("(-1).to_byte()", "out of range");
}

#[test]
fn int_to_char() {
    assert_val("65.to_char()", ch('A'));
    assert_val("0.to_char()", ch('\0'));
    assert_error("(-1).to_char()", "negative value");
    assert_error("1114112.to_char()", "not a valid Unicode scalar value");
}

#[test]
fn float_rounding() {
    assert_val("3.7.ceil()", int(4));
    assert_val("3.2.floor()", int(3));
    assert_val("3.5.round()", int(4));
    assert_val("3.9.trunc()", int(3));
    assert_val("(-3.7).ceil()", int(-3));
    assert_val("(-3.7).floor()", int(-4));
    assert_val("(-3.5).round()", int(-4));
    assert_val("(-3.9).trunc()", int(-3));
}

#[test]
fn char_to_int() {
    assert_val("'A'.to_int()", int(65));
    assert_val("'\\0'.to_int()", int(0));
}

#[test]
fn byte_to_int() {
    assert_val("b'A'.to_int()", int(65));
    assert_val("b'\\0'.to_int()", int(0));
}

// ── ref_eq, val_eq, and function comparison ──────────────────────

#[test]
fn ref_eq_same_closure() {
    // A closure compared with its own copy is the same identity
    assert_val("let f = { in + 1 }; ref_eq(f, f)", T);
}

#[test]
fn ref_eq_different_closures() {
    // Two separate evaluations of identical code are different identities
    assert_val("ref_eq({ in + 1 }, { in + 1 })", F);
    assert_val("ref_eq({}, {})", F);
}

#[test]
fn ref_eq_builtins() {
    // Builtins are singletons — same name means same identity
    assert_val("ref_eq(print, print)", T);
    assert_val("ref_eq(print, not)", F);
}

#[test]
fn ref_eq_non_functions() {
    assert_val("ref_eq(1, 1)", T);
    assert_val("ref_eq(1, 2)", F);
    assert_val(r#"ref_eq("a", "a")"#, T);
}

#[test]
fn ref_eq_cross_type() {
    assert_val("ref_eq(1, true)", F);
}

#[test]
fn val_eq_closures() {
    // val_eq compares structure, not identity
    assert_val("val_eq({ in + 1 }, { in + 1 })", T);
    assert_val("val_eq({}, {})", T);
    assert_val("val_eq({ in + 1 }, { in + 2 })", F);
}

#[test]
fn val_eq_builtins() {
    assert_val("val_eq(print, print)", T);
    assert_val("val_eq(print, not)", F);
}

#[test]
fn val_eq_non_functions() {
    // For non-functions, val_eq behaves the same as ref_eq
    assert_val("val_eq(1, 1)", T);
    assert_val("val_eq(1, 2)", F);
}

#[test]
fn function_eq_error() {
    assert_error("{ in + 1 } == { in + 1 }", "cannot compare functions with ==; use ref_eq()");
    assert_error("print == print", "cannot compare functions with ==; use ref_eq()");
    assert_error("{ in + 1 } != { in + 2 }", "cannot compare functions with ==; use ref_eq()");
}

#[test]
fn mismatched_type_compare_still_generic() {
    assert_error(r#"1 == "hello""#, "type error");
}

// ── Method sets ──────────────────────────────────────────────────

#[test]
fn method_set_basic() {
    assert_val(r#"
        tag(Celsius);
        let ms = method_set(Celsius, (
            show = { Celsius(v) -> "{v}°C" }
        ));
        apply(ms);
        Celsius(42).show()
    "#, s("42°C"));
}

#[test]
fn method_set_multiple_methods() {
    assert_val(r#"
        tag(Celsius);
        let ms = method_set(Celsius, (
            show = { Celsius(v) -> "{v}°C" },
            value = { Celsius(v) -> v }
        ));
        apply(ms);
        Celsius(42).value()
    "#, int(42));
}

#[test]
fn method_set_lexical_scope() {
    // Method should not be available outside apply scope
    assert_error(r#"
        tag(Celsius);
        let ms = method_set(Celsius, (
            show = { Celsius(v) -> "{v}°C" }
        ));
        (apply(ms); Celsius(42));
        Celsius(100).show()
    "#, "no method");
}

#[test]
fn method_set_shadowing() {
    assert_val(r#"
        tag(Celsius);
        let ms1 = method_set(Celsius, (
            show = { Celsius(v) -> "{v} degrees" }
        ));
        let ms2 = method_set(Celsius, (
            show = { Celsius(v) -> "{v}°C" }
        ));
        apply(ms1);
        apply(ms2);
        Celsius(42).show()
    "#, s("42°C"));
}

#[test]
fn method_set_error_outside_scope() {
    assert_error(r#"
        tag(Celsius);
        Celsius(42).show()
    "#, "no method");
}

#[test]
fn method_set_with_additional_args() {
    assert_val(r#"
        tag(Vec2);
        let ms = method_set(Vec2, (
            add = { >> let(a, b);
                a >> { Vec2(av) -> av } >> let(ax, ay);
                b >> { Vec2(bv) -> bv } >> let(bx, by);
                Vec2((ax + bx, ay + by))
            }
        ));
        apply(ms);
        Vec2((1, 2)).add(Vec2((3, 4))) >> { Vec2(r) -> r }
    "#, Value::Struct(vec![
        ("0".to_string(), int(4)),
        ("1".to_string(), int(6)),
    ]));
}

#[test]
fn method_set_design_example() {
    // Example from DESIGN.md
    assert_val(r#"
        tag(Celsius);
        let to_string = method_set(Celsius, (
            show = { Celsius(v) -> "{v}°C" }
        ));
        apply(to_string);
        Celsius(42).show()
    "#, s("42°C"));
}

// ── std module tests ──

fn assert_std(input: &str, expected: Value) {
    let result = nana::run_with_std(input);
    let val = result.unwrap_or_else(|e| panic!("program failed.\n  input: {input}\n  error: {e}"));
    assert_eq!(
        val, expected,
        "\n  input: {input}\n  expected: {expected}\n  got: {val}"
    );
}

#[test]
fn std_array_methods_via_method_set() {
    assert_std(r#"
        use(std);
        apply(std.int_methods);
        apply(std.array_methods);
        [1, 2, 3].map{ * 2 }
    "#, Value::Array(vec![int(2), int(4), int(6)]));
}

#[test]
fn std_array_filter_via_method_set() {
    assert_std(r#"
        use(std);
        apply(std.int_methods);
        apply(std.array_methods);
        [1, 2, 3, 4].filter{ > 2 }
    "#, Value::Array(vec![int(3), int(4)]));
}

#[test]
fn std_array_fold_via_method_set() {
    assert_std(r#"
        use(std);
        apply(std.int_methods);
        apply(std.array_methods);
        [1, 2, 3].fold(0, { >> let(acc, elem); acc + elem })
    "#, int(6));
}

#[test]
fn std_array_len_via_method_set() {
    assert_std(r#"
        use(std);
        apply(std.array_methods);
        [1, 2, 3].len()
    "#, int(3));
}

#[test]
fn std_array_get_via_method_set() {
    assert_std(r#"
        use(std);
        apply(std.array_methods);
        [10, 20, 30].get(1)
    "#, int(20));
}

#[test]
fn std_array_zip_via_method_set() {
    assert_std(r#"
        use(std);
        apply(std.array_methods);
        [1, 2].zip([3, 4])
    "#, Value::Array(vec![
        Value::Struct(vec![("0".to_string(), int(1)), ("1".to_string(), int(3))]),
        Value::Struct(vec![("0".to_string(), int(2)), ("1".to_string(), int(4))]),
    ]));
}

#[test]
fn std_array_slice_via_method_set() {
    assert_std(r#"
        use(std);
        apply(std.array_methods);
        [10, 20, 30, 40].slice(1..3)
    "#, Value::Array(vec![int(20), int(30)]));
}

#[test]
fn std_string_methods_via_method_set() {
    assert_std(r#"
        use(std);
        apply(std.string_methods);
        "hello world".contains("world")
    "#, Value::Bool(true));
}

#[test]
fn std_string_char_len_via_method_set() {
    assert_std(r#"
        use(std);
        apply(std.string_methods);
        "hello".char_len()
    "#, int(5));
}

#[test]
fn std_string_split_via_method_set() {
    assert_std(r#"
        use(std);
        apply(std.string_methods);
        "a,b,c".split(",")
    "#, Value::Array(vec![s("a"), s("b"), s("c")]));
}

#[test]
fn std_string_trim_via_method_set() {
    assert_std(r#"
        use(std);
        apply(std.string_methods);
        "  hello  ".trim()
    "#, s("hello"));
}

#[test]
fn std_string_starts_with_via_method_set() {
    assert_std(r#"
        use(std);
        apply(std.string_methods);
        "hello".starts_with("hel")
    "#, Value::Bool(true));
}

#[test]
fn std_string_replace_via_method_set() {
    assert_std(r#"
        use(std);
        apply(std.string_methods);
        "hello world".replace("world", "nana")
    "#, s("hello nana"));
}

#[test]
fn std_builtins_accessible() {
    // Builtins should be accessible through std
    assert_std(r#"
        use(std);
        std.not(true)
    "#, Value::Bool(false));
}

#[test]
fn std_type_constructors() {
    // Type constructors should be in std
    assert_std(r#"
        use(std);
        let ms = std.method_set(std.Array, (
            count = { >> let(arr); arr.len() }
        ));
        apply(ms);
        [1, 2, 3].count()
    "#, int(3));
}

#[test]
fn float_to_string_whole_number() {
    // float_to_string should produce "1.0" for whole-number floats, matching Display
    assert_val(r#""value: {1.0}""#, s("value: 1.0"));
    assert_val(r#""value: {3.14}""#, s("value: 3.14"));
}

// ── Generic array method type checking ──

#[test]
fn generic_map_preserves_type() {
    // [1,2,3].map{+1} should produce Array(Int), not lose type info
    assert_val("[1, 2, 3].map{ + 1 }", Value::Array(vec![int(2), int(3), int(4)]));
}

#[test]
fn generic_map_type_transform() {
    // map with a type-changing function: Int → Bool
    assert_val(
        "[1, 2, 3].map{ > 1 }",
        Value::Array(vec![Value::Bool(false), Value::Bool(true), Value::Bool(true)]),
    );
}

#[test]
fn generic_filter_preserves_type() {
    assert_val("[1, 2, 3, 4].filter{ > 2 }", Value::Array(vec![int(3), int(4)]));
}

#[test]
fn generic_fold_returns_init_type() {
    // fold with int init — result should be int
    assert_val(
        "[1, 2, 3].fold(0, { >> let(acc, elem); acc + elem })",
        int(6),
    );
}

#[test]
fn generic_zip_returns_pair_array() {
    assert_val(
        r#"[1, 2].zip(["a", "b"]).get(0)"#,
        Value::Struct(vec![("0".into(), int(1)), ("1".into(), s("a"))]),
    );
}

#[test]
fn generic_get_returns_element() {
    assert_val("[10, 20, 30].get(1)", int(20));
}

#[test]
fn generic_chained_map_filter() {
    // Chaining should preserve type through operations
    assert_val(
        "[1, 2, 3, 4].map{ * 2 }.filter{ > 4 }",
        Value::Array(vec![int(6), int(8)]),
    );
}

#[test]
fn type_error_non_function_struct_field_call() {
    // Calling a non-function struct field should be a type error
    assert_error("(len=42).len()", "cannot call");
}
