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
- **Arrays:** single-type, created with `[1, 2, 3]`. Indexing via `arr[0]`, slicing via `arr[1..3]`, concatenation (`+`), destructuring via `let[...]` with `...` for rest capture. An empty array `[]` has an undecided element type, refined on first use.
- **Ranges:** `1..3` is sugar for `(start=1, end=3)`.
- **Strings:** UTF-8 byte sequences. Not directly indexable; use `as_bytes` to convert to a byte array, then index. Concatenation via `+`. Literals use `"…"` with escape sequences. Multi-line strings use `\\` prefix per line (Zig-style). No single-quoted strings.
- **Structs/Tuples/Records:** unified as structs with optional labels (default numeric 0,1,…). Created with `(a=1, b=2)` for labeled fields or `(1, 2)` for positional. Spread via `...`: `(a=99, ...rest)` creates a new struct with `a` replaced. Trailing commas are allowed. `(1)` is just `1` — a single value is a one-element sequence.
- **Tags / Sum Types:** generated via `tag(hoge)` (sugar for `new_tag >> let(hoge)`). Tags have type `T -> tag[T]`; a no-payload tag wraps `()`. Tags are nominal, generative, and lexical.

### Name binding

- `let` is the only binding form. There is no `=` assignment syntax. (`=` appears only inside `()` for field labeling in struct construction and destructuring.)
- `let(name)` introduces a new lexical scope: it acts like an implicit `(`, extending as far right as possible (to the end of the enclosing block or parenthesized expression).
- `let(x)` returns `x`, so the bound value flows through pipes: `value >> let(x) >> log` works.
- `;` sequences let-scopes. It has the lowest precedence (lower than `>>`).
- Shadowing is allowed; each `let` creates a fresh scope.
- `_name` binds but suppresses unused-binding warnings. `_` discards the value entirely.

```
1 >> let(x); x + 2                              # basic binding; evaluates to 3
1 >> let(x); x >> { in + 1 } >> let(y); x + y   # chained bindings; evaluates to 3
1 >> let(x); (2 >> let(y); y + 1) + x            # parentheses limit let scope; evaluates to 4
value >> let(x) >> log                            # let returns its value; pipes onward
```

### Generative Values

- `new_tag` is a built-in, generatively typed operator: each lexical reference to `new_tag` produces a unique tag constructor. A tag constructor has type `T -> tag[T]`.
- `tag(hoge)` is sugar for `new_tag >> let(hoge)` — it generates a fresh tag constructor and binds it to `hoge`.
- Per-call generation (inside lambda) would introduce dependent types; avoided by lexical generation.
- Pattern matching and `case` operate over tags safely.

---

## Expressions

- Comments: `#` to end of line.
- Operators: `+ - * /` (ints, floats), `+` for array and string concatenation, unary `-` for negation, comparison operators (`==`, `!=`, `<`, `>`, `<=`, `>=`). Comparisons work on primitives, arrays, structs, and tags. Functions cannot be compared with operators — use a stdlib function instead (function equality is non-trivial).
- `and`, `or`, `not` are functions, not operators (e.g., `and(true, false)`).
- Conditionals: `bool >> if expr else expr`, or `if bool then expr else expr` when piping is less clear. Parentheses around expressions are optional. `else` is optional only when the true branch returns `()`. Both branches must return the same type. Sugar for `case` on booleans.
- Parentheses for grouping.
- Pipes: `value >> func` pipes `value` into `func`. `value >> f(x)` is equivalent to `f(value, x)` — the piped value is prepended to the argument list.
- `;` sequences expressions (lowest precedence).
- External imports via `import(external_name) >> let(name)`.
- `use(name)` is sugar for `import(name) >> let(name)`.

### Operator Precedence (tightest to loosest)

1. `f(x)` / `f.x` — function call / field access (left-to-right)
2. `-x` — unary negation
3. `*` `/` — multiplication and division. `/` only accepts a single operand on the right; use parentheses for complex right-hand expressions (e.g., `a / (b * c)`). `a / b * c` is a syntax error; `a * b / c` is valid.
4. `+ -` — addition, subtraction, concatenation
5. Comparisons — `==`, `!=`, `<`, `>`, `<=`, `>=`
6. `>>` — pipe (left-to-right)
7. `if`/`else`, `case` — conditionals and pattern matching. Mixing `>>` after `if`/`else` requires parentheses: `a >> if c else d >> e` is a syntax error; write `a >> (if c else d) >> e` or `a >> if c else (d >> e)`.
8. `;` — sequencing

---

## Functions

- Defined as `{ … }` blocks; input accessed via `in`.
- `in` always refers to the nearest enclosing block's input. To access an outer block's input, rebind it with `let`.
- The top-level program is itself a `{ … }` block, so `in` at the top level is the program's input.
- Single-argument; multi-argument functions take a struct. `f(x, y)` is `f((x, y))` — the call parentheses are struct construction.
- `{ >> f }` is sugar for `{ in >> f }`. More generally, `{ op x }` where `op` is a binary operator is sugar for `{ in op x }`.
- This sugar applies only to binary operators at the start of a block. `{ f }` is not sugar — it evaluates to `f` as a value. Use `{ in >> f }` to apply `f` to the input.
- A block always has an input slot; if the block body doesn't reference `in`, the input type is unconstrained (generic).
- Examples:

```text
3 >> { in + 1 }                                  # explicit form; result is 4
3 >> { + 1 }                                     # sugar for { in + 1 }; same result
{ in * 2 } >> let(f); 3 >> f                     # bind a lambda, then apply; result is 6
3 >> { in >> let(outer); 4 >> { outer + in } }   # nested blocks; result is 7
```

- Recursion is not possible: there are no recursive bindings, and the Y-combinator is prevented by the occurs check (no recursive/infinite types). The language is total — every program terminates. Recursive operations like `map`, `filter`, and `fold` are provided as built-in stdlib functions.


## Destructuring

- Arrays and strings are destructured positionally via `let[…]`. Structs are destructured by named labels via `let(…)`.
- Destructuring patterns are only valid inside `let`; a bare pattern outside `let` is a syntax error.
- `...name` captures remaining elements in arrays or remaining fields in structs. `_` discards an element. `_name` binds but suppresses unused-binding warnings.
- `case` is a keyword; each arm `Tag(x) -> expr` binds `x` only within that arm's body. All arms must return the same type. Arms support `if` guards: `Tag(x) if x > 0 -> expr`.

```text
arr >> let[first, ...rest]; first + rest          # head and tail
arr >> let[start, ...mid, end]                    # first, middle, last
arr >> let[_, second, ...]                        # discard first, bind second, ignore rest
hoge >> let(a=hoge_a, ...rest); hoge_a            # bind one field, capture remaining fields as a struct
tagsum >> case(                                   # pattern match on tags (multi-arm, comma-separated)
  TagA(x) -> x + 1,
  TagB(y) -> y * 2
)
```
- All destructuring is done through `let` (with `[…]` for arrays/strings, `(…)` for structs/tuples) and `case` for tags.

## Complete Example

```text
tag(Ok);
tag(Err);

# safe_div : (int, int) -> tag[int] | tag[string]
{ in >> let(a, b);
  if b == 0 then Err("division by zero")
    else (a * 100 / b >> Ok)
} >> let(safe_div);

(10, 3) >> safe_div >> case(
  Ok(result) -> result,
  Err(_) -> 0
)
# evaluates to 333
```

---

## Standard Library Principles

- Pure, functional, and pipe-friendly.
- Collections: arrays and strings with map, filter, fold, zip, and concatenation.
- Struct/tuple helpers: access, merge, construct, and destructure.
- Sum/tag combinators: `map_tag`, `bind_tag`, pattern matching helpers.
- Math & logic primitives: arithmetic, comparisons, boolean operations.


---

## Modules and Imports

- Modules are represented as structs.
- Access modules with `use(name)` or `import(external_name) >> let(name)`.
- Unifies external modules, structs, and sum types under one system.

---

## Error Handling

- Division by zero, array out-of-bounds, and similar runtime errors halt execution.
- Non-exhaustive `case` matches are compile-time errors.
- Since compilation and evaluation are the same process, halting and compile errors are equivalent.

---

## Resource Types

- Resource types originate from external sources (e.g., loggers, file handles, streams).
- Cannot be cloned: `(a=logger, b=logger)` is a compile error.
- Can be passed to functions as implicit borrows: the callee uses the resource, but does not consume it. The resource is implicitly returned when the function exits.
- A function cannot return a borrowed resource in its return value — the borrow does not escape.
- The original owner retains the resource after a call.
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
