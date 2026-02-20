# Language Design: Functional, Expression-Based, Pipeable DSL

## Core Principles

- Pure, statically typed, expression-oriented.
- Every ordinary expression is clonable and droppable (resource types are the exception — see Resource Types).
- Functions are first-class, unary, with `{ … }` blocks; input is accessed via `in`.
- Pipe operator `>>` pipes the left value into the right expression.
- Blocks return the last expression as their value; `{}` evaluates to `()`.
- Shadowing allowed with `let`, introducing new lexical scopes.
- Compilation and evaluation are the same process; type errors and runtime errors both halt.

---

## Value Types

- **Primitives:** `int` (64-bit), `float` (64-bit), `bool` (`true`/`false`), `byte` (unsigned 8-bit integer; literal `b'a'`, `0xF4`, or `4`), `char` (Unicode scalar value, as in Rust; literal `'a'`), `string` (UTF-8 bytes). Other bit sizes available via separate constructors. Numeric literals are literal-typed and convert automatically to the required type.
- **Unit:** `()` is the zero-field struct, used as the unit value.
- **Arrays:** single-type, created with `[1, 2, 3]`. Element access via `arr.get(0)`, slicing via `arr.slice(1..3)`, concatenation (`+`), destructuring via `let[...]` with `...` for rest capture. An empty array `[]` has an undecided element type, refined on first use.
- **Ranges:** `1..3` is sugar for `(start=1, end=3)`.
- **Strings:** UTF-8 byte sequences. Not directly indexable; use `as_bytes` to convert to a byte array, then index. Standard library provides `chars()` to iterate Unicode codepoints. Char literals `'a'` denote a single Unicode codepoint. Concatenation via `+`. Literals use `"…"` with escape sequences. String interpolation via `{expr}` inside string literals: `"Hello, {name}!"` evaluates the expression and converts the result to a string. Use `\{` to escape a literal brace. Multi-line strings use `\\` prefix per line (Zig-style). No single-quoted strings.
- **Structs/Tuples/Records:** unified as structs with optional labels (default numeric 0,1,…). Created with `(a=1, b=2)` for labeled fields or `(1, 2)` for positional. Spread via `...`: `(a=99, ...rest)` creates a new struct with `a` replaced. Positional fields from a spread are re-indexed after any preceding explicit fields: `(99, ...s)` where `s = (10, 20)` produces `(99, 10, 20)`. Trailing commas are allowed. `(1)` is just the parenthesized expression `1`. There is no distinct single-element tuple type.
- **Tags / Sum Types:** generated via `tag(hoge)` (sugar for `new_tag >> let(hoge)`). A tag constructor has type `T -> tag[T]`. There is no automatic wrapping — to create a unit-payload tag, explicitly apply the constructor: `() >> None`. Tags are nominal, generative, and lexical. Tag constructors are first-class values that can be compared for equality and matched in branching blocks.

### Name binding

- `let` is the only binding form.
- `let(name)` introduces a new lexical scope: it extends to the end of the current block.
- `let(x)` returns `x`, so the bound value flows through pipes: `value >> let(x) >> log` works.
- `let name = expr;` is sugar for `expr >> let(name)`. The `=` here is part of the let sugar, not a general assignment operator. (`=` also appears inside `()` for field labeling in struct construction and destructuring.)
- `let(a, b)` destructures a struct: if the struct has fields named `a` and `b`, it binds by name; otherwise, if the struct has positional fields, it binds positionally. If neither matches, it is an error. This is all-or-nothing: partial name matches are errors.
- `;` sequences expressions. It has the lowest precedence (lower than `>>`). Let-scopes extend to the end of the enclosing block, not to the next `;`.
- Shadowing is allowed; each `let` creates a fresh scope.
- `_name` binds but suppresses unused-binding warnings. `_` discards the value entirely. Shadowed bindings that are unused still produce warnings.

```
1 >> let(x); x + 2                              # basic binding; evaluates to 3
let x = 1; x + 2                                # sugar form; same result
1 >> let(x); x >> { in + 1 } >> let(y); x + y   # chained bindings; evaluates to 3
1 >> let(x); (2 >> let(y); y + 1) + x            # parentheses limit let scope; evaluates to 4
value >> let(x) >> log                            # let returns its value; pipes onward
(1, 2) >> let(a, b); a + b                       # positional destructuring; evaluates to 3
(x=1, y=2) >> let(x, y); x + y                  # named destructuring; evaluates to 3
```

### Generative Values

- `new_tag` is a built-in, generatively typed operator: each lexical reference to `new_tag` produces a unique tag constructor. A tag constructor has type `T -> tag[T]`.
- `tag(hoge)` is sugar for `new_tag >> let(hoge)` — it generates a fresh tag constructor and binds it to `hoge`.
- Per-call generation (inside lambda) would introduce dependent types; avoided by lexical generation.
- Pattern matching operates over tags safely.

---

## Expressions

- Comments: `#` to end of line.
- Operators: `+ - * /` (ints, floats), `+` for array and string concatenation, unary `-` for negation, comparison operators (`==`, `!=`, `<`, `>`, `<=`, `>=`). Comparisons require matching types — there is no implicit coercion between `int` and `float`. Comparisons work on primitives (same type), arrays, structs, tags, and tag constructors. Functions cannot be compared with operators — use a stdlib function instead (function equality is non-trivial). Unary `-` combined with postfix calls is ambiguous: `-a.f()` is a syntax error; write `(-a).f()` or `-(a.f())`.
- `and`, `or`, `not` are functions, not operators (e.g., `and(true, false)`). Evaluation order is unspecified. For conditional short-circuiting, use branching.
- Branching: `{ pattern -> expr, pattern -> expr, ... }` is the unified conditional and pattern matching form. A branching block is a function: it receives input via `in` (like any block) and matches it against the arms. Each arm is `pattern -> expr`. Patterns can be literal values (`true`, `false`, `0`), tag constructors (`Ok(x)`, `Err(msg)`), or bindings. Arms support `if` guards: `Ok(x) if x > 0 -> expr`. All arms must return the same type. Non-exhaustive matches are errors. `in` is available in arm bodies.
- Guard-only sugar: `{ if expr -> body, if expr2 -> body2, ... }` is sugar for `{ _ if expr -> body, _ if expr2 -> body2, ... }`. When an arm starts with `if`, only the guard expression is evaluated (no pattern matching). This enables if/else-if/else chains: `{ if in < 0 -> "negative", if in == 0 -> "zero", _ -> "positive" }`.
- Default arm sugar: the last arm in a branching block can be a bare expression without `->`, acting as a catch-all: `{ if in < 0 -> "neg", "non-neg" }` is sugar for `{ if in < 0 -> "neg", _ -> "non-neg" }`. The default arm must be the last arm.
- Ternary sugar: `{ a | b }` is sugar for `{ true -> a, false -> b }`. It is a shorthand for boolean branching that short-circuits — only the taken branch is evaluated. `in` is available in both branches. `|` has very low precedence inside the block (just above `;`), so `{ x + 1 | y * 2 }` parses as `{ (x + 1) | (y * 2) }`.
- Parentheses for grouping and struct construction only. `(expr)` is grouping; `(a=1, b=2)` is a struct.
- Pipes: `value >> func` pipes `value` into `func`. `value >> f(x)` is equivalent to `f(value, x)` — the piped value is prepended to the argument list.
- `;` sequences expressions (lowest precedence).
- `import` is resolved at compile/module-load time; the argument must be a literal word or string. `import` is a built-in special form, not a runtime function. External imports via `import(external_name) >> let(name)`.
- `use(name)` is sugar for `import(name) >> let(name)`.

### Operator Precedence (tightest to loosest)

1. `f(x)` / `f{}` / `f[]` / `f.x` — function call (with parens, block, or array) / field access / method call (left-to-right). `-f(x)` is a syntax error; write `-(f(x))` or `(-f)(x)`.
2. `-x` — unary negation (of simple operands only; see above)
3. `*` `/` — multiplication and division. `a / b * c` is a syntax error; `a * b / c` is valid. `/` requires a single operand on the right, so parentheses must be used for complex expressions (e.g., `a / (b * c)`).
4. `+ -` — addition, subtraction, concatenation
5. Comparisons — `==`, `!=`, `<`, `>`, `<=`, `>=`
6. `>>` — pipe (left-to-right)
7. `;` — sequencing

---

## Functions

- Defined as `{ … }` blocks; input accessed via `in`.
- `in` always refers to the nearest enclosing block's input. To access an outer block's input, rebind it with `let`.
- The top-level program is itself a `{ … }` block, so `in` at the top level is the program's input.
- Single-argument; multi-argument functions take a struct. `f(x, y)` is `f((x, y))` — the call parentheses are struct construction.
- Three call syntaxes: `f(args)` passes a struct, `f{ body }` passes a block, `f[elems]` passes an array. These are postfix and left-to-right: `f(x){ body }` calls `f` with a struct, then calls the result with a block.
- Blocks have two forms, both receiving input via `in`:
  - **Expression block:** `{ expr }` — evaluates the body expression. `{ >> f }` is sugar for `{ in >> f }`. `{ op x }` is sugar for `{ in op x }`.
  - **Branching block:** `{ pattern -> expr, ... }` — matches `in` against the arms. This is the unified form for conditionals and pattern matching.
- Both forms are functions. A branching block is not a separate construct — it is a block that pattern-matches its input.
- `{ f }` (expression block) is not sugar — it evaluates to a function that returns `f` as a value (`f` is evaluated when the block is called, not when created). Use `{ in >> f }` to apply `f` to the input.
- A block always has an input slot; if the block body doesn't reference `in`, the input type is unconstrained (generic).
- Examples:

```text
3 >> { in + 1 }                                  # expression block; result is 4
3 >> { + 1 }                                     # sugar for { in + 1 }; same result
{ in * 2 } >> let(f); 3 >> f                     # bind a lambda, then apply; result is 6
3 >> { in >> let(outer); 4 >> { outer + in } }   # nested blocks; result is 7
true >> { true -> "yes", false -> "no" }         # branching block; result is "yes"
true >> { "yes" | "no" }                         # ternary sugar; same result
x > 0 >> { "positive" | "non-positive" }         # ternary with comparison
```

### Method Calls

- `.name` on a value accesses a field (for structs) or calls a type-based method.
- Method dispatch is type-based: the method is resolved by the type of the receiver. For example, `arr.map{ * 2 }` calls the `map` method defined on arrays.
- Methods use the same call syntaxes: `arr.map{ * 2 }`, `arr.fold(0, f)`, `arr.get(0)`, `str.as_bytes[]`.
- There is no UFCS (uniform function call syntax); `>>` and methods are distinct mechanisms. `>>` pipes into a function, `.method` dispatches on the receiver's type.

```text
[1, 2, 3].map{ * 2 }                            # [2, 4, 6]
[1, 2, 3].filter{ > 1 }                         # [2, 3]
[1, 2, 3].fold(0, { in.acc + in.elem })          # 6
[1, 2, 3].get(0)                                 # 1
```

### Totality

- Recursion is not possible: there are no recursive bindings, and the Y-combinator is prevented by the occurs check (no recursive/infinite types). The language is total — every program terminates. User code cannot express unbounded recursion. Some built-in stdlib operations may internally iterate over finite collections. Totality refers to the core language surface.


## Destructuring

- Arrays and strings are destructured positionally via `let[…]`. Structs are destructured via `let(…)`.
- `let(a, b)` binds by name if the struct has fields `a` and `b`; otherwise binds positionally. All-or-nothing: partial name matches are errors.
- `let(a=x)` explicitly binds field `a` to variable `x` (always by name).
- A `let[…]` or `let(…)` binding is lexical; names exist only within the let-scope. Destructuring patterns are only valid inside `let`; a bare pattern outside `let` is a syntax error.
- If a pattern in `let[…]` or `let(…)` fails to match the value at runtime, it is an error and halts evaluation.
- `...name` captures remaining elements in arrays or remaining fields in structs. `_` discards an element. `_name` binds but suppresses unused-binding warnings.
- Tag matching uses branching blocks: `value >> { TagA(x) -> expr, TagB(y) -> expr }`. Each arm binds within its body. All arms must return the same type. Arms support `if` guards: `Ok(x) if x > 0 -> expr`.

```text
arr >> let[first, ...rest]; first + rest          # head and tail
arr >> let[start, ...mid, end]                    # first, middle, last
arr >> let[_, second, ...]                        # discard first, bind second, ignore rest
hoge >> let(a=hoge_a, ...rest); hoge_a            # bind one field, capture remaining fields as a struct
tagsum >> {                                       # pattern match on tags via branching block
  TagA(x) -> x + 1,
  TagB(y) -> y * 2
}
```
- All destructuring is done through `let` (with `[…]` for arrays/strings, `(…)` for structs/tuples) and branching blocks `{ pattern -> expr, ... }` for tags.

## Complete Example

```text
tag(Ok);
tag(Err);

# safe_div : (int, int) -> tag[int] | tag[string]
let safe_div = { in >> let(a, b);
  b == 0 >> {
    true -> Err("division by zero"),
    false -> a * 100 / b >> Ok
  }
};

(10, 3) >> safe_div >> {
  Ok(result) -> result,
  Err(_) -> 0
}
# evaluates to 333
```

---

## Standard Library Principles

- Pure, functional, and pipe-friendly.
- Collections: arrays with method-based API (`map`, `filter`, `fold`, `zip`, `get`, `slice`, `len`) and concatenation (`+`). Strings provide `byte_len`, `char_len`, and concatenation (`+`); `len` is for arrays only.
- Struct/tuple helpers: access, merge, construct, and destructure.
- Sum/tag combinators: pattern matching via branching.
- Math & logic primitives: arithmetic, comparisons, boolean functions (`and`, `or`, `not`).


---

## Modules and Imports

- Modules are represented as structs. Imports are resolved at module-load time; exported structs contain bound tag constructors. Importing the same module yields the same tag identities.
- Access modules with `use(name)` or `import(external_name) >> let(name)`.
- Unifies external modules, structs, and sum types under one system.

---

## Error Handling

- Division by zero, array out-of-bounds, and similar runtime errors halt execution.
- Non-exhaustive branch matches are compile-time errors.
- Since compilation and evaluation are the same process, halting and compile errors are equivalent.

---

## Resource Types

- Resource types originate from external sources (e.g., loggers, file handles, streams).
- Cannot be cloned: `(a=logger, b=logger)` is a compile error.
- A resource value passed to a function is temporarily borrowed. The borrow cannot escape the callee (cannot be returned or stored in a heap object). The original owner retains the resource. Attempts to escape a borrow are errors and halt evaluation.
- A struct containing a resource field is itself a resource type: it cannot be cloned or returned. Destructuring separates the fields; non-resource fields regain normal clonable/droppable semantics.
- Internally, ordinary expressions remain clonable and droppable; resource restrictions only apply to resource-typed values.

---

## Type System Notes

- Compilation and evaluation are the same process; there is no separate type-check phase.
- Tag generation is lexical, not per-call, avoiding dependent types.
- Lambdas producing fresh tags per call would require dependent types and are disallowed.
- The occurs check prevents recursive/infinite types, making the language total.
- Ordinary expressions, arrays, structs, sums, and functions remain fully statically typed.
- The language is fully inferred; there is no user-facing type annotation syntax.

---

## Compilation Target

- Target: WebAssembly.
  - Expressions map to WASM stack operations.
  - Primitives → WASM numeric types.
  - Arrays and structs → linear memory layout.
  - Tags → `(tag_id, payload)` representation.
  - Pattern matching → `switch` or `br_table`.
- No garbage collector required. Ordinary heap allocations (arrays, strings, structs) are freed at program end. Resource types handle external resources.

---

## Summary

A small, expressive, statically typed, pure functional DSL for:

- Spreadsheet-style pipelines
- Expression-only computations
- Nominal generative values via tags
- Controlled resource management via borrowing
- Unified compilation and evaluation (no separate type-check phase)

Influences include:

- Functional languages: F#, OCaml, Haskell, Elm
- Expression-oriented DSLs: Nix, Racket
- Array-centric languages: J, K, APL
- Spreadsheet formulas: Excel LAMBDA, SheetJS scripting

The result is a **pure, composable, pipeable, and deterministic environment** for technical computations.
