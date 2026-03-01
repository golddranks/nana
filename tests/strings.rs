mod common;
use common::*;

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

