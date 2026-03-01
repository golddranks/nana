mod common;
use common::*;

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
    // Empty array is valid — element type defaults to Unit.
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
    // Per D5: non-exhaustive branch is a type error
    assert_error("tag(A); tag(B); 1 >> A >> { B(x) -> x }", "non-exhaustive branch");
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

// ── Type-checked REPL persistence ──

#[test]
fn repl_checked_let_persists() {
    let (env, mut ty_env) = nana::env_with_std_and_ty_env().unwrap();
    let (_, env) = nana::run_in_env_checked("let a = [1, 2, 3]", &env, &mut ty_env).unwrap();
    let (val, _) = nana::run_in_env_checked("a", &env, &mut ty_env).unwrap();
    assert_eq!(val.to_string(), "[1, 2, 3]");
}

#[test]
fn repl_checked_type_error_across_lines() {
    let (env, mut ty_env) = nana::env_with_std_and_ty_env().unwrap();
    let (_, env) = nana::run_in_env_checked("let a = [1, 2]", &env, &mut ty_env).unwrap();
    // Concatenating string array with int array should be a type error
    let result = nana::run_in_env_checked(r#"a + ["foo"]"#, &env, &mut ty_env);
    assert!(result.is_err());
}

#[test]
fn repl_checked_empty_array_then_concat() {
    let (env, mut ty_env) = nana::env_with_std_and_ty_env().unwrap();
    let (_, env) = nana::run_in_env_checked("let a = []", &env, &mut ty_env).unwrap();
    // Empty array should unify with any element type
    let (val, _) = nana::run_in_env_checked(r#"a + ["foo"]"#, &env, &mut ty_env).unwrap();
    assert_eq!(val.to_string(), r#"["foo"]"#);
}

// ── Unchecked REPL persistence ──

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
    // [] with let[a, ...rest, b] needs at least 2 elements.
    // Type checker rejects because [] has unresolved element type (D2).
    assert_error("[] >> let[a, ...rest, b]; a", "");
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

