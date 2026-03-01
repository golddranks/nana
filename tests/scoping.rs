mod common;
use common::*;

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
    // After shadowing tag X, the old X-tagged value carries a different tag identity.
    // Per D5: the branch pattern uses the new X identity, which doesn't match — type error.
    assert_error(
        r#"tag(X); 1 >> X >> let(v1); tag(X); v1 >> { X(n) -> n * 100 }"#,
        "non-exhaustive branch",
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
    // Using Tag(x) pattern with undefined tag is a type error (per D5)
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
    // Value tagged with a1 should NOT match a branch pattern using a2's identity.
    // Per D5: non-exhaustive branch is a type error.
    assert_error(
        "tag(A); let a1 = A; tag(A); 42 >> a1 >> { A(x) -> x }",
        "non-exhaustive branch",
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
    // Type checker rejects because [] has unresolved element type (D2).
    // Would be runtime "out of bounds" if it passed type checking.
    assert_error("[].get(0)", "");
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
    // Per D1: standalone closures as program results are rejected (unresolved inference)
    assert_error("{ in + 1 }", "unresolved inference");
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

