mod common;
use common::*;

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
    // 1 >> A creates A(1), { A(x) -> x } extracts Int, then { A(y) -> y }
    // receives an Int (not tagged). Per D5: non-exhaustive branch is a type error.
    assert_error(
        "tag(A); 1 >> A >> { A(x) -> x } >> { A(y) -> y }",
        "non-exhaustive branch",
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

