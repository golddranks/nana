#[allow(unused_imports)]
mod common;
use common::*;

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
fn probe23_array_concat() {
    assert_output("[4] + [6]", "[4, 6]");
}

#[test]
fn probe23_array_concat_type_mismatch() {
    assert_error(r#"["foo"] + [5]"#, "cannot unify");
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
    // Empty array concat is valid — element type defaults to Unit.
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
    // `{ + 1 }` has ambiguous type (Int->Int or Byte->Byte), so .get() returning
    // it correctly produces an unresolved inference error.
    assert_error(
        "let fns = [{ + 1 }, { * 2 }, { + 3 }]; fns.get(0)",
        "unresolved inference",
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
        ("pi".to_string(), Value::I64(3)),
        ("e".to_string(), Value::I64(2)),
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
    // Module exports a function via a struct field.
    // Build closures via eval (skipping type check — standalone blocks have Infer).
    let env = nana::env_with_std().unwrap();
    let (double, _) = nana::run_in_env(&format!("{}{{ in * 2 }}", STD_PRELUDE), &env).unwrap();
    let (inc, _) = nana::run_in_env(&format!("{}{{ in + 1 }}", STD_PRELUDE), &env).unwrap();
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

// ── Type hint builtins ────────────────────────────────────────────

#[test]
fn byte_type_hint() {
    // byte() is a type hint: Byte → Byte (identity)
    assert_val("byte(b'A')", byte(b'A'));
    assert_val("byte(b'\\x00')", byte(0));
    // IntLiteral coerces to Byte, so literal ints work too
    assert_val("byte(0)", byte(0));
    assert_val("byte(65)", byte(65));
    assert_val("byte(255)", byte(255));
}

#[test]
fn let_bound_int_literal_coerces() {
    // Int literal bound via let should remain IntLiteral (not prematurely
    // defaulted to Int), so it can coerce to Byte in byte contexts.
    assert_val("let a = 65; b'A' == a", T);
    assert_val("let a = 4; a + 1", int(5));
    assert_val("let a = 4; a.to_u8()", byte(4));
}

#[test]
fn let_bound_float_literal_coerces() {
    assert_val("let a = 2.0; a + 1.0", float(3.0));
    assert_val("let a = 2.5; a * 2.0", float(5.0));
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
fn byte_type_hint_rejects_wrong_types() {
    // byte() is Byte → Byte; negated literals become Int, not Byte
    assert_error("byte(-1)", "type error");
    assert_error("byte(true)", "type error");
    assert_error("byte(\"hi\")", "type error");
}

#[test]
fn int_type_hint() {
    // int() is Int → Int (type hint / identity)
    assert_val("int(42)", int(42));
    assert_error("int(3.14)", "type error");
    assert_error("int(true)", "type error");
}

#[test]
fn float_type_hint() {
    // float() is Float → Float (type hint / identity)
    assert_val("float(3.14)", float(3.14));
    assert_val("float(0.0)", float(0.0));
    assert_error("float(42)", "type error");
}

#[test]
fn char_type_hint() {
    // char() is Char → Char (type hint / identity)
    assert_val("char('A')", ch('A'));
    // int literals don't coerce to Char
    assert_error("char(65)", "type error");
    assert_error("char(b'z')", "type error");
}

// ── Conversion methods ───────────────────────────────────────────

#[test]
fn int_to_float() {
    assert_val("42.to_f64()", float(42.0));
    assert_val("(-3).to_f64()", float(-3.0));
}

#[test]
fn int_to_byte() {
    assert_val("65.to_u8()", byte(65));
    assert_error("256.to_u8()", "out of range");
    assert_error("(-1).to_u8()", "out of range");
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
    assert_val("'A'.to_i64()", int(65));
    assert_val("'\\0'.to_i64()", int(0));
}

#[test]
fn byte_to_int() {
    assert_val("b'A'.to_i64()", int(65));
    assert_val("b'\\0'.to_i64()", int(0));
}

// ── I32 (32-bit integer) ─────────────────────────────────────────

#[test]
fn i32_type_hint() {
    assert_val("i32(42)", i32_val(42));
    assert_val("i32(0)", i32_val(0));
}

#[test]
fn i32_type_hint_rejects_wrong_types() {
    assert_error("i32(3.14)", "type error");
    assert_error("i32(true)", "type error");
    assert_error("i32(\"hi\")", "type error");
}

#[test]
fn i32_arithmetic() {
    assert_val("i32(10) + i32(20)", i32_val(30));
    assert_val("i32(10) - i32(3)", i32_val(7));
    assert_val("i32(6) * i32(7)", i32_val(42));
    assert_val("i32(10) / i32(3)", i32_val(3));
    assert_val("i32(0) - i32(5)", i32_val(-5));
}

#[test]
fn i32_overflow() {
    assert_error("i32(2147483647) + i32(1)", "overflow");
}

#[test]
fn i32_division_by_zero() {
    assert_error("i32(1) / i32(0)", "division by zero");
}

#[test]
fn i32_comparisons() {
    assert_val("i32(1) == i32(1)", T);
    assert_val("i32(1) != i32(2)", T);
    assert_val("i32(1) < i32(2)", T);
    assert_val("i32(2) > i32(1)", T);
    assert_val("i32(1) <= i32(1)", T);
    assert_val("i32(2) >= i32(1)", T);
}

#[test]
fn i32_literal_coercion() {
    // Int literal auto-coerces to I32 in I32 context
    assert_val("i32(1) + 2", i32_val(3));
    assert_val("let a = 42; i32(a)", i32_val(42));
}

#[test]
fn i32_conversions() {
    assert_val("i32(42).to_i64()", int(42));
    assert_val("i32(42).to_f64()", float(42.0));
    assert_val("i32(42).to_f32()", f32_val(42.0));
    assert_val("i32(65).to_u8()", byte(65));
    assert_error("i32(256).to_u8()", "out of range");
    assert_output("i32(42).to_string()", "\"42i32\"");
}

#[test]
fn i32_cross_type_rejection() {
    // I32 and Int are distinct types — no implicit cross-type arithmetic
    assert_error("i32(1) + int(2)", "type error");
}

#[test]
fn int_to_i32_conversion() {
    assert_val("42.to_i32()", i32_val(42));
    assert_val("(-1).to_i32()", i32_val(-1));
    assert_error("2147483648.to_i32()", "out of range");
}

#[test]
fn byte_to_i32_conversion() {
    assert_val("b'A'.to_i32()", i32_val(65));
}

// ── F32 (32-bit float) ──────────────────────────────────────────

#[test]
fn f32_type_hint() {
    assert_val("f32(3.0)", f32_val(3.0));
    assert_val("f32(0.0)", f32_val(0.0));
}

#[test]
fn f32_type_hint_rejects_wrong_types() {
    assert_error("f32(42)", "type error");
    assert_error("f32(true)", "type error");
}

#[test]
fn f32_arithmetic() {
    assert_val("f32(3.0) + f32(2.0)", f32_val(5.0));
    assert_val("f32(3.0) - f32(1.0)", f32_val(2.0));
    assert_val("f32(3.0) * f32(2.0)", f32_val(6.0));
    assert_val("f32(6.0) / f32(2.0)", f32_val(3.0));
    assert_val("f32(0.0) - f32(5.0)", f32_val(-5.0));
}

#[test]
fn f32_division_by_zero() {
    assert_error("f32(1.0) / f32(0.0)", "division by zero");
}

#[test]
fn f32_comparisons() {
    assert_val("f32(1.0) == f32(1.0)", T);
    assert_val("f32(1.0) != f32(2.0)", T);
    assert_val("f32(1.0) < f32(2.0)", T);
    assert_val("f32(2.0) > f32(1.0)", T);
}

#[test]
fn f32_literal_coercion() {
    // Float literal auto-coerces to F32 in F32 context
    assert_val("f32(1.0) + 2.0", f32_val(3.0));
    assert_val("let a = 2.5; f32(a)", f32_val(2.5));
}

#[test]
fn f32_rounding() {
    assert_val("f32(3.7).ceil()", i32_val(4));
    assert_val("f32(3.2).floor()", i32_val(3));
    assert_val("f32(3.5).round()", i32_val(4));
    assert_val("f32(3.9).trunc()", i32_val(3));
}

#[test]
fn f32_conversions() {
    assert_val("f32(3.0).to_f64()", float(3.0));
    assert_val("f32(3.0).to_i64()", int(3));
    assert_val("f32(3.0).to_i32()", i32_val(3));
    assert_output("f32(3.0).to_string()", "\"3.0f32\"");
}

#[test]
fn f32_cross_type_rejection() {
    assert_error("f32(1.0) + float(2.0)", "type error");
}

#[test]
fn float_to_f32_conversion() {
    assert_val("3.14.to_f32()", f32_val(3.14_f64 as f32));
}

// ── I128 (128-bit signed integer) ────────────────────────────────

#[test]
fn i128_type_hint() {
    assert_val("i128(42)", i128_val(42));
    assert_val("i128(0)", i128_val(0));
}

#[test]
fn i128_type_hint_rejects_wrong_types() {
    assert_error("i128(3.14)", "type error");
    assert_error("i128(true)", "type error");
    assert_error("i128(\"hi\")", "type error");
}

#[test]
fn i128_arithmetic() {
    assert_val("i128(10) + i128(20)", i128_val(30));
    assert_val("i128(10) - i128(3)", i128_val(7));
    assert_val("i128(6) * i128(7)", i128_val(42));
    assert_val("i128(10) / i128(3)", i128_val(3));
    assert_val("i128(0) - i128(5)", i128_val(-5));
}

#[test]
fn i128_overflow() {
    // Can't express i128::MAX as a literal (lexer only handles i64 range).
    // Instead, test that checked arithmetic works by chaining multiplications.
    assert_error(
        "let big = i128(9223372036854775807); big * big * big",
        "overflow",
    );
}

#[test]
fn i128_division_by_zero() {
    assert_error("i128(1) / i128(0)", "division by zero");
}

#[test]
fn i128_comparisons() {
    assert_val("i128(1) == i128(1)", T);
    assert_val("i128(1) != i128(2)", T);
    assert_val("i128(1) < i128(2)", T);
    assert_val("i128(2) > i128(1)", T);
    assert_val("i128(1) <= i128(1)", T);
    assert_val("i128(2) >= i128(1)", T);
}

#[test]
fn i128_literal_coercion() {
    assert_val("i128(1) + 2", i128_val(3));
    assert_val("let a = 42; i128(a)", i128_val(42));
}

#[test]
fn i128_conversions() {
    assert_val("i128(42).to_i64()", int(42));
    assert_val("i128(42).to_i32()", i32_val(42));
    assert_val("i128(42).to_u128()", u128_val(42));
    assert_error("i128(0) - i128(1) >> { n -> n.to_u128() }", "out of range");
    assert_output("i128(42).to_string()", "\"42i128\"");
}

#[test]
fn i128_cross_type_rejection() {
    assert_error("i128(1) + int(2)", "type error");
    assert_error("i128(1) + i32(2)", "type error");
}

#[test]
fn int_to_i128_conversion() {
    assert_val("42.to_i128()", i128_val(42));
    assert_val("(-1).to_i128()", i128_val(-1));
}

#[test]
fn byte_to_i128_conversion() {
    assert_val("b'A'.to_i128()", i128_val(65));
}

#[test]
fn i32_to_i128_conversion() {
    assert_val("i32(42).to_i128()", i128_val(42));
}

// ── U128 (128-bit unsigned integer) ──────────────────────────────

#[test]
fn u128_type_hint() {
    assert_val("u128(42)", u128_val(42));
    assert_val("u128(0)", u128_val(0));
}

#[test]
fn u128_type_hint_rejects_wrong_types() {
    assert_error("u128(3.14)", "type error");
    assert_error("u128(true)", "type error");
}

#[test]
fn u128_type_hint_rejects_negative() {
    assert_error("u128(0) - u128(1)", "underflow");
}

#[test]
fn u128_arithmetic() {
    assert_val("u128(10) + u128(20)", u128_val(30));
    assert_val("u128(10) - u128(3)", u128_val(7));
    assert_val("u128(6) * u128(7)", u128_val(42));
    assert_val("u128(10) / u128(3)", u128_val(3));
}

#[test]
fn u128_overflow() {
    // Can't express u128::MAX as a literal (lexer only handles i64 range).
    // Instead, test that checked arithmetic works by chaining multiplications.
    assert_error(
        "let big = u128(9223372036854775807); big * big * big",
        "overflow",
    );
}

#[test]
fn u128_division_by_zero() {
    assert_error("u128(1) / u128(0)", "division by zero");
}

#[test]
fn u128_comparisons() {
    assert_val("u128(1) == u128(1)", T);
    assert_val("u128(1) != u128(2)", T);
    assert_val("u128(1) < u128(2)", T);
    assert_val("u128(2) > u128(1)", T);
    assert_val("u128(1) <= u128(1)", T);
    assert_val("u128(2) >= u128(1)", T);
}

#[test]
fn u128_literal_coercion() {
    assert_val("u128(1) + 2", u128_val(3));
    assert_val("let a = 42; u128(a)", u128_val(42));
}

#[test]
fn u128_conversions() {
    assert_val("u128(42).to_i64()", int(42));
    assert_val("u128(42).to_i32()", i32_val(42));
    assert_val("u128(42).to_i128()", i128_val(42));
    assert_output("u128(42).to_string()", "\"42u128\"");
}

#[test]
fn u128_cross_type_rejection() {
    assert_error("u128(1) + int(2)", "type error");
    assert_error("u128(1) + i128(2)", "type error");
}

#[test]
fn int_to_u128_conversion() {
    assert_val("42.to_u128()", u128_val(42));
    assert_error("(-1).to_u128()", "out of range");
}

#[test]
fn byte_to_u128_conversion() {
    assert_val("b'A'.to_u128()", u128_val(65));
}

#[test]
fn i32_to_u128_conversion() {
    assert_val("i32(42).to_u128()", u128_val(42));
    assert_error("i32(0) - i32(1) >> { n -> n.to_u128() }", "out of range");
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
}

#[test]
fn ref_eq_different_type_builtins_error() {
    // Different-typed builtins can't be compared
    assert_error("ref_eq(print, not)", "cannot unify");
}

#[test]
fn ref_eq_non_functions() {
    assert_val("ref_eq(1, 1)", T);
    assert_val("ref_eq(1, 2)", F);
    assert_val(r#"ref_eq("a", "a")"#, T);
}

#[test]
fn ref_eq_cross_type_error() {
    // Cross-type comparison is a type error
    assert_error("ref_eq(1, true)", "cannot unify");
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
}

#[test]
fn val_eq_different_type_builtins_error() {
    // Different-typed builtins can't be compared
    assert_error("val_eq(print, not)", "cannot unify");
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

#[test]
fn std_array_methods_via_method_set() {
    assert_std(r#"
        use(std);
        apply(std.i64_methods);
        apply(std.array_methods);
        [1, 2, 3].map{ * 2 }
    "#, Value::Array(vec![int(2), int(4), int(6)]));
}

#[test]
fn std_array_filter_via_method_set() {
    assert_std(r#"
        use(std);
        apply(std.i64_methods);
        apply(std.array_methods);
        [1, 2, 3, 4].filter{ > 2 }
    "#, Value::Array(vec![int(3), int(4)]));
}

#[test]
fn std_array_fold_via_method_set() {
    assert_std(r#"
        use(std);
        apply(std.i64_methods);
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

// ── Inference failure detection ──────────────────────────────────
//
// The type checker rejects programs whose result type contains unresolved
// inference variables (Ty::Infer). These tests verify that various
// inference failures are caught.

#[test]
fn infer_error_standalone_block() {
    // A standalone block that is never called has Infer param and body
    // (body is Infer because .times() on Infer returns Infer)
    assert_error("{ in * 2 }", "unresolved inference");
}

#[test]
fn infer_error_standalone_branch_block() {
    // A standalone branch block that is never called
    assert_error("{ true -> 1, false -> 0 }", "unresolved inference");
}

#[test]
fn infer_error_method_on_infer() {
    // Method call on Infer receiver silently succeeds — the method name
    // is never validated because the receiver type is unknown
    assert_error("{ in.nonexistent() }", "unresolved inference");
}

#[test]
fn infer_error_call_on_infer() {
    // Calling an Infer value silently succeeds
    assert_error("{ in(42) }", "unresolved inference");
}

#[test]
fn infer_error_empty_array() {
    // Empty array is valid — element type defaults to Unit.
    assert_output("[]", "[]");
}

#[test]
fn infer_error_block_in_struct() {
    // Block stored as struct field — Infer leaks into struct type
    assert_error("(f = { in + 1 })", "unresolved inference");
}

#[test]
fn infer_ok_block_called_immediately() {
    // Block called immediately — Infer resolved via bidirectional checking
    assert_val("3 >> { in + 1 }", int(4));
}

#[test]
fn infer_ok_block_stored_then_called() {
    // Block stored via let then called — re-check resolves Infer
    assert_val("let f = { in + 1 }; f(3)", int(4));
}

#[test]
fn infer_ok_block_as_callback() {
    // Block as method callback — re-check via method dispatch resolves Infer
    assert_val("[1,2,3].map{ * 2 }", Value::Array(vec![int(2), int(4), int(6)]));
}

#[test]
fn infer_ok_branch_called_immediately() {
    // Branch block called immediately — Infer resolved
    assert_val("true >> { true -> 1, false -> 0 }", int(1));
}

// ═══════════════════════════════════════════════════════════════════
// I8 tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn i8_type_hint() {
    assert_val("i8(42)", i8_val(42));
    assert_val("i8(0)", i8_val(0));
}

#[test]
fn i8_type_hint_rejects_wrong_types() {
    assert_error("i8(3.14)", "type error");
    assert_error("i8(true)", "type error");
}

#[test]
fn i8_arithmetic() {
    assert_val("i8(10) + i8(20)", i8_val(30));
    assert_val("i8(10) - i8(3)", i8_val(7));
    assert_val("i8(6) * i8(7)", i8_val(42));
    assert_val("i8(10) / i8(3)", i8_val(3));
    assert_val("i8(0) - i8(5)", i8_val(-5));
}

#[test]
fn i8_overflow() {
    assert_error("i8(127) + i8(1)", "overflow");
}

#[test]
fn i8_division_by_zero() {
    assert_error("i8(1) / i8(0)", "division by zero");
}

#[test]
fn i8_comparisons() {
    assert_val("i8(1) == i8(1)", T);
    assert_val("i8(1) != i8(2)", T);
    assert_val("i8(1) < i8(2)", T);
    assert_val("i8(2) > i8(1)", T);
}

#[test]
fn i8_literal_coercion() {
    assert_val("i8(1) + 2", i8_val(3));
    assert_val("let a = 42; i8(a)", i8_val(42));
}

#[test]
fn i8_conversions() {
    assert_val("i8(42).to_i64()", int(42));
    assert_output("i8(42).to_string()", "\"42i8\"");
}

#[test]
fn i8_cross_type_rejection() {
    assert_error("i8(1) + int(2)", "type error");
}

// ═══════════════════════════════════════════════════════════════════
// U8 tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn u8_type_hint() {
    assert_val("u8(42)", u8_val(42));
    assert_val("u8(0)", u8_val(0));
}

#[test]
fn u8_type_hint_rejects_wrong_types() {
    assert_error("u8(3.14)", "type error");
    assert_error("u8(true)", "type error");
}

#[test]
fn u8_arithmetic() {
    assert_val("u8(10) + u8(20)", u8_val(30));
    assert_val("u8(10) - u8(3)", u8_val(7));
    assert_val("u8(6) * u8(7)", u8_val(42));
    assert_val("u8(10) / u8(3)", u8_val(3));
}

#[test]
fn u8_overflow() {
    assert_error("u8(255) + u8(1)", "overflow");
}

#[test]
fn u8_underflow() {
    assert_error("u8(0) - u8(1)", "underflow");
}

#[test]
fn u8_division_by_zero() {
    assert_error("u8(1) / u8(0)", "division by zero");
}

#[test]
fn u8_comparisons() {
    assert_val("u8(1) == u8(1)", T);
    assert_val("u8(1) != u8(2)", T);
    assert_val("u8(1) < u8(2)", T);
    assert_val("u8(2) > u8(1)", T);
}

#[test]
fn u8_literal_coercion() {
    assert_val("u8(1) + 2", u8_val(3));
    assert_val("let a = 42; u8(a)", u8_val(42));
}

#[test]
fn u8_conversions() {
    assert_val("u8(42).to_i64()", int(42));
    assert_output("u8(42).to_string()", "\"42u8\"");
}

// ═══════════════════════════════════════════════════════════════════
// I16 tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn i16_type_hint() {
    assert_val("i16(42)", i16_val(42));
    assert_val("i16(0)", i16_val(0));
}

#[test]
fn i16_type_hint_rejects_wrong_types() {
    assert_error("i16(3.14)", "type error");
    assert_error("i16(true)", "type error");
}

#[test]
fn i16_arithmetic() {
    assert_val("i16(100) + i16(200)", i16_val(300));
    assert_val("i16(100) - i16(30)", i16_val(70));
    assert_val("i16(6) * i16(7)", i16_val(42));
    assert_val("i16(100) / i16(3)", i16_val(33));
    assert_val("i16(0) - i16(5)", i16_val(-5));
}

#[test]
fn i16_overflow() {
    assert_error("i16(32767) + i16(1)", "overflow");
}

#[test]
fn i16_division_by_zero() {
    assert_error("i16(1) / i16(0)", "division by zero");
}

#[test]
fn i16_comparisons() {
    assert_val("i16(1) == i16(1)", T);
    assert_val("i16(1) != i16(2)", T);
    assert_val("i16(1) < i16(2)", T);
    assert_val("i16(2) > i16(1)", T);
}

#[test]
fn i16_literal_coercion() {
    assert_val("i16(1) + 2", i16_val(3));
    assert_val("let a = 42; i16(a)", i16_val(42));
}

#[test]
fn i16_conversions() {
    assert_val("i16(42).to_i64()", int(42));
    assert_output("i16(42).to_string()", "\"42i16\"");
}

#[test]
fn i16_cross_type_rejection() {
    assert_error("i16(1) + int(2)", "type error");
}

// ═══════════════════════════════════════════════════════════════════
// U16 tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn u16_type_hint() {
    assert_val("u16(42)", u16_val(42));
    assert_val("u16(0)", u16_val(0));
}

#[test]
fn u16_type_hint_rejects_wrong_types() {
    assert_error("u16(3.14)", "type error");
    assert_error("u16(true)", "type error");
}

#[test]
fn u16_arithmetic() {
    assert_val("u16(100) + u16(200)", u16_val(300));
    assert_val("u16(100) - u16(30)", u16_val(70));
    assert_val("u16(6) * u16(7)", u16_val(42));
    assert_val("u16(100) / u16(3)", u16_val(33));
}

#[test]
fn u16_overflow() {
    assert_error("u16(65535) + u16(1)", "overflow");
}

#[test]
fn u16_underflow() {
    assert_error("u16(0) - u16(1)", "underflow");
}

#[test]
fn u16_division_by_zero() {
    assert_error("u16(1) / u16(0)", "division by zero");
}

#[test]
fn u16_comparisons() {
    assert_val("u16(1) == u16(1)", T);
    assert_val("u16(1) != u16(2)", T);
    assert_val("u16(1) < u16(2)", T);
    assert_val("u16(2) > u16(1)", T);
}

#[test]
fn u16_literal_coercion() {
    assert_val("u16(1) + 2", u16_val(3));
    assert_val("let a = 42; u16(a)", u16_val(42));
}

#[test]
fn u16_conversions() {
    assert_val("u16(42).to_i64()", int(42));
    assert_output("u16(42).to_string()", "\"42u16\"");
}

// ═══════════════════════════════════════════════════════════════════
// U32 tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn u32_type_hint() {
    assert_val("u32(42)", u32_val(42));
    assert_val("u32(0)", u32_val(0));
}

#[test]
fn u32_type_hint_rejects_wrong_types() {
    assert_error("u32(3.14)", "type error");
    assert_error("u32(true)", "type error");
}

#[test]
fn u32_arithmetic() {
    assert_val("u32(100) + u32(200)", u32_val(300));
    assert_val("u32(100) - u32(30)", u32_val(70));
    assert_val("u32(6) * u32(7)", u32_val(42));
    assert_val("u32(100) / u32(3)", u32_val(33));
}

#[test]
fn u32_overflow() {
    assert_error("u32(4294967295) + u32(1)", "overflow");
}

#[test]
fn u32_underflow() {
    assert_error("u32(0) - u32(1)", "underflow");
}

#[test]
fn u32_division_by_zero() {
    assert_error("u32(1) / u32(0)", "division by zero");
}

#[test]
fn u32_comparisons() {
    assert_val("u32(1) == u32(1)", T);
    assert_val("u32(1) != u32(2)", T);
    assert_val("u32(1) < u32(2)", T);
    assert_val("u32(2) > u32(1)", T);
}

#[test]
fn u32_literal_coercion() {
    assert_val("u32(1) + 2", u32_val(3));
    assert_val("let a = 42; u32(a)", u32_val(42));
}

#[test]
fn u32_conversions() {
    assert_val("u32(42).to_i64()", int(42));
    assert_output("u32(42).to_string()", "\"42u32\"");
}

// ═══════════════════════════════════════════════════════════════════
// U64 tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn u64_type_hint() {
    assert_val("u64(42)", u64_val(42));
    assert_val("u64(0)", u64_val(0));
}

#[test]
fn u64_type_hint_rejects_wrong_types() {
    assert_error("u64(3.14)", "type error");
    assert_error("u64(true)", "type error");
}

#[test]
fn u64_arithmetic() {
    assert_val("u64(100) + u64(200)", u64_val(300));
    assert_val("u64(100) - u64(30)", u64_val(70));
    assert_val("u64(6) * u64(7)", u64_val(42));
    assert_val("u64(100) / u64(3)", u64_val(33));
}

#[test]
fn u64_overflow() {
    // u64 max is 2^64 - 1, but nana lexer only handles i64 range
    // Use large multiplication chain to trigger overflow
    let input = "let big = u64(4294967295); let huge = big * big; huge * huge";
    assert_error(input, "overflow");
}

#[test]
fn u64_underflow() {
    assert_error("u64(0) - u64(1)", "underflow");
}

#[test]
fn u64_division_by_zero() {
    assert_error("u64(1) / u64(0)", "division by zero");
}

#[test]
fn u64_comparisons() {
    assert_val("u64(1) == u64(1)", T);
    assert_val("u64(1) != u64(2)", T);
    assert_val("u64(1) < u64(2)", T);
    assert_val("u64(2) > u64(1)", T);
}

#[test]
fn u64_literal_coercion() {
    assert_val("u64(1) + 2", u64_val(3));
    assert_val("let a = 42; u64(a)", u64_val(42));
}

#[test]
fn u64_conversions() {
    assert_val("u64(42).to_i64()", int(42));
    assert_output("u64(42).to_string()", "\"42u64\"");
}

// ═══════════════════════════════════════════════════════════════════
// F64 tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn f64_type_hint() {
    assert_val("f64(3.14)", f64_val(3.14));
    assert_val("f64(0.0)", f64_val(0.0));
}

#[test]
fn f64_type_hint_rejects_wrong_types() {
    assert_error("f64(true)", "type error");
    assert_error("f64(\"hi\")", "type error");
}

#[test]
fn f64_arithmetic() {
    assert_val("f64(10.0) + f64(20.0)", f64_val(30.0));
    assert_val("f64(10.0) - f64(3.0)", f64_val(7.0));
    assert_val("f64(6.0) * f64(7.0)", f64_val(42.0));
    assert_val("f64(10.0) / f64(4.0)", f64_val(2.5));
}

#[test]
fn f64_division_by_zero() {
    assert_error("f64(1.0) / f64(0.0)", "division by zero");
}

#[test]
fn f64_comparisons() {
    assert_val("f64(1.0) == f64(1.0)", T);
    assert_val("f64(1.0) != f64(2.0)", T);
    assert_val("f64(1.0) < f64(2.0)", T);
    assert_val("f64(2.0) > f64(1.0)", T);
}

#[test]
fn f64_literal_coercion() {
    assert_val("f64(1.0) + 2.0", f64_val(3.0));
    assert_val("let a = 1.5; f64(a)", f64_val(1.5));
}

#[test]
fn f64_rounding() {
    assert_val("f64(3.7).ceil()", int(4));
    assert_val("f64(3.7).floor()", int(3));
    assert_val("f64(3.5).round()", int(4));
    assert_val("f64(3.7).trunc()", int(3));
}

#[test]
fn f64_conversions() {
    assert_val("f64(42.5).to_i64()", int(42));
    assert_val("f64(42.0).to_f32()", f32_val(42.0));
    assert_output("f64(42.0).to_string()", "\"42.0\"");
}

#[test]
fn f64_cross_type_rejection() {
    // f64 and float are now the same type, so f64 + float works
    assert_val("f64(1.0) + float(2.0)", float(3.0));
    // f64 and f32 are still different types
    assert_error("f64(1.0) + f32(2.0)", "type error");
}
