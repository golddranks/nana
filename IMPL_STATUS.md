# Implementation Status

Cross-reference of DESIGN.md spec against the current interpreter implementation.

---

## Value Types

| Spec Feature | Status | Notes |
|---|---|---|
| `int` (64-bit) | Implemented | Literals, arithmetic, overflow checks |
| `float` (64-bit) | Implemented | Literals, arithmetic, div-by-zero check |
| `bool` (`true`/`false`) | Implemented | |
| `byte` (literal `b'a'`, `0xF4`) | Partial | `b'a'` works; hex `0xF4` lexes as `int`, not `byte` |
| `byte` (plain `4` auto-converts) | Not implemented | No automatic numeric literal coercion to byte |
| Hex integer literals (`0xFF`) | Implemented | Parsed by lexer; underscore separators supported |
| Binary integer literals (`0b1010`) | Implemented | Parsed by lexer; underscore separators supported |
| `char` (Unicode scalar) | Implemented | |
| `string` (UTF-8) | Implemented | Literals, concatenation, escape sequences |
| String interpolation `"Hello, {name}!"` | Implemented | Lexer produces `InterpStr` tokens; parser/eval handle nested expressions |
| Multi-line strings (`\\` prefix, Zig-style) | Implemented | Lexer joins `\\`-prefixed lines, strips leading whitespace |
| `as_bytes` (string to byte array) | Implemented | String method |
| `chars()` (string to codepoints) | Implemented | String method |
| Other bit-size constructors | Not implemented | Spec mentions "other bit sizes via constructors" |
| Literal-typed numerics / auto-coercion | Not implemented | int/float are fixed; no automatic widening to required type |
| Unit `()` | Implemented | |
| Arrays `[1,2,3]` | Implemented | Creation, concatenation, destructuring |
| `arr.get(idx)` element access | Implemented | Method on arrays; bounds-checked |
| `arr.slice(range)` slicing | Implemented | Method on arrays; takes a range struct |
| Empty array `[]` type refinement | Partial | `[]` works, but no type system to refine |
| Ranges `1..3` | Implemented | Sugar for `(start=1, end=3)` struct |
| Structs / Tuples / Records | Implemented | Labeled, positional, spread, trailing commas |
| `(1)` is grouping, not tuple | Implemented | |
| Parentheses limit `let` scope | Implemented | `(let x = 1; x); x` correctly errors |
| Tags / Sum Types | Implemented | `tag()`, `new_tag`, generative, lexical |
| No-payload tag wraps `()` | Implemented | `A()` produces `A` with unit payload |
| Spread on `()` (unit as empty struct) | Implemented | `(...unit_val)` is a no-op |

---

## Name Binding

| Spec Feature | Status | Notes |
|---|---|---|
| `let(name)` binding | Implemented | |
| `let name = expr;` sugar | Implemented | Desugars to `expr >> let(name)` |
| `let(x)` returns `x` (passthrough) | Implemented | |
| `;` sequences let-scopes | Implemented | |
| Shadowing | Implemented | |
| `_` discards | Implemented | |
| `_name` binds, suppresses warnings | Implemented | `Env::unused_warnings()` checks for unused bindings; `_` prefix suppresses |

---

## Generative Values

| Spec Feature | Status | Notes |
|---|---|---|
| `new_tag` (unique per lexical reference) | Implemented | Global `AtomicU64` counter, assigned at parse time |
| `tag(name)` sugar | Implemented | Desugars to `new_tag >> let(name)` |
| Per-call generation prevented | Implemented | Tag IDs assigned at parse time, not runtime |

---

## Expressions

| Spec Feature | Status | Notes |
|---|---|---|
| Comments `#` | Implemented | |
| Arithmetic `+ - * /` | Implemented | |
| String concatenation `+` | Implemented | |
| Array concatenation `+` | Implemented | |
| Unary `-` | Implemented | |
| Comparisons `== != < > <= >=` | Implemented | Primitives, arrays, structs, tags |
| Function comparison rejection | Not implemented | Generic type mismatch error, not a specific message |
| `and`, `or`, `not` as functions | Implemented | Builtins, not operators |
| Branching blocks `{ pattern -> expr, ... }` | Implemented | Unified conditional / pattern matching form |
| `if` guards in branch arms | Implemented | `pattern if guard -> expr` |
| All arms must return same type | Not enforced | No type system |
| Non-exhaustive branch is error | Implemented | Runtime error (compile = eval in this language) |
| Ternary sugar `{ a \| b }` | Implemented | Desugars to `{ true -> a, false -> b }`; short-circuits |
| Parentheses for grouping | Implemented | |
| Pipe `>>` | Implemented | |
| Pipe prepend `value >> f(x)` = `f(value, x)` | Implemented | |
| `;` sequencing | Implemented | |
| Semicolons rejected in array/struct/call-arg | Implemented | `[1; 2]`, `(1, 2; 3)`, `f(1; 2)` are errors |
| `import(name)` | Parsed only | Always errors at runtime: "import not available" |
| `use(name)` | Parsed only | Desugars to import, which errors |

---

## Operator Precedence

| Spec Feature | Status | Notes |
|---|---|---|
| `f(x)` / `f.x` tightest | Implemented | bp::POSTFIX = 19 |
| `-x` unary | Implemented | bp::UNARY = 17 |
| `* /` | Implemented | bp::MUL = 14/15 |
| `a / b * c` syntax error | Implemented | Also rejects `a / b / c` |
| `..` range | Implemented | bp::RANGE = 12/13 |
| `+ -` | Implemented | bp::ADD = 10/11 |
| Comparisons (non-associative) | Implemented | bp::CMP = 8/9; chained comparisons rejected |
| `>>` pipe | Implemented | bp::PIPE = 6/7 |
| `;` lowest | Implemented | bp::SEMI = 2 |

---

## Functions

| Spec Feature | Status | Notes |
|---|---|---|
| `{ ... }` blocks as lambdas | Implemented | |
| `in` refers to nearest block input | Implemented | |
| Top-level `in` is program input | Implemented | Defaults to `Value::Unit` |
| Single-argument; multi-arg via struct | Implemented | `f(x, y)` is `f((x, y))` |
| Three call syntaxes: `f(args)`, `f{ body }`, `f[elems]` | Implemented | |
| `{ >> f }` sugar for `{ in >> f }` | Implemented | |
| `{ op x }` sugar for `{ in op x }` | Implemented | |
| `{ f }` is NOT sugar (lazy eval) | Implemented | Evaluates `f` at call time |
| `{}` evaluates to unit (when called) | Implemented | `Block(Unit)` |
| Recursion impossible | Implemented | No recursive bindings, no Y-combinator |
| Totality | Implemented | No user-expressible unbounded recursion; stdlib iterates finite collections |

---

## Method Calls

| Spec Feature | Status | Notes |
|---|---|---|
| `.name` field access (structs) | Implemented | |
| `.name(arg)` method calls (type-based dispatch) | Implemented | |
| `arr.get(idx)` | Implemented | Returns element or error |
| `arr.slice(range)` | Implemented | Takes range struct `start..end` |
| `arr.len()` | Implemented | |
| `arr.map{ f }` | Implemented | |
| `arr.filter{ f }` | Implemented | |
| `arr.fold(init, f)` | Implemented | Passes `(acc=, elem=)` struct to f |
| `arr.zip(other)` | Implemented | |
| `str.len()` | Implemented | Byte length |
| UFCS (uniform function call syntax) | Not implemented | Spec says `>>` and `.method` are distinct |

---

## Destructuring

| Spec Feature | Status | Notes |
|---|---|---|
| `let[...]` array destructuring | Implemented | Positional, `...rest`, `_` |
| `let(...)` struct destructuring | Implemented | Named labels, `...rest`, `_` |
| `let(a=x)` named field binding | Implemented | Binds field `a` to variable `x` |
| String destructuring via `let[...]` | Implemented | Strings treated as arrays of single-char strings; supports `...rest` |
| Pattern failure halts evaluation | Implemented | |
| `...name` rest capture (arrays) | Implemented | |
| `...name` rest capture (structs) | Implemented | Re-indexes positional fields |
| `_` discard | Implemented | |
| `_name` suppresses warnings | Implemented | `_` prefix suppresses unused-binding warnings |
| Branching block pattern matching on tags | Implemented | `value >> { Tag(x) -> expr, ... }` |
| `if` guards in branch arms | Implemented | `Ok(x) if x > 0 -> expr` |
| All arms must return same type | Not enforced | No type system |
| Non-exhaustive branch is error | Implemented | Runtime error (compile = eval in this language) |
| Strict destructuring (all fields consumed) | Implemented | |

---

## Standard Library

| Spec Feature | Status | Notes |
|---|---|---|
| `map` | Implemented | Builtin function and array method |
| `filter` | Implemented | Builtin function and array method |
| `fold` | Implemented | Builtin function and array method |
| `zip` | Implemented | Builtin function and array method |
| `len` | Implemented | Builtin function and array/string method |
| `not` | Implemented | `not(bool)` |
| `and` | Implemented | `and(bool, bool)` |
| `or` | Implemented | `or(bool, bool)` |
| `print` | Implemented | Side-effecting, returns `()` |
| `get` | Implemented | Array method only (not a builtin function) |
| `slice` | Implemented | Array method only (not a builtin function) |
| `as_bytes` | Implemented | String method; returns byte array |
| `chars` | Implemented | String method; returns array of chars |
| `split` | Implemented | String method; splits by delimiter |
| `trim` | Implemented | String method; strips leading/trailing whitespace |
| `contains` | Implemented | String method; checks for substring or char |
| `starts_with` | Implemented | String method; checks prefix |
| `ends_with` | Implemented | String method; checks suffix |
| `replace` | Implemented | String method; takes `(pattern, replacement)` struct |
| `str.slice(range)` | Implemented | String method; byte-based slicing |
| `str.get(idx)` | Implemented | String method; returns single-char string |
| Struct/tuple helpers (access, merge) | Not implemented | Only field access via `.` syntax |
| Sum/tag combinators (`map_tag`, `bind_tag`) | Not implemented | |

---

## Modules and Imports

| Spec Feature | Status | Notes |
|---|---|---|
| `import(name)` syntax | Parsed only | Always errors: "import not available" |
| `use(name)` sugar | Parsed only | Desugars correctly, but import always fails |
| Module loading / resolution | Not implemented | No file system integration |
| Same module yields same tag IDs | Not implemented | No module system |

---

## Error Handling

| Spec Feature | Status | Notes |
|---|---|---|
| Division by zero halts | Implemented | Int and float |
| Array out-of-bounds halts | Implemented | |
| Non-exhaustive branch match | Implemented | Runtime error |
| Compile = eval (errors are equivalent) | Implemented | Single-pass interpretation |

---

## Resource Types

| Spec Feature | Status | Notes |
|---|---|---|
| Resource type tracking | Not implemented | No resource type distinction in `Value` |
| Clone restriction | Not implemented | All values are freely clonable |
| Borrow semantics | Not implemented | |
| Struct-containing-resource propagation | Not implemented | |

---

## Type System

| Spec Feature | Status | Notes |
|---|---|---|
| Static typing | Not implemented | Dynamically typed; type errors at runtime |
| Type inference | Not implemented | No types to infer |
| Occurs check | Not implemented | Totality maintained by disallowing recursion syntactically |
| Compilation = evaluation | Implemented | Single pass |

---

## Compilation Target

| Spec Feature | Status | Notes |
|---|---|---|
| WebAssembly output | Not implemented | Tree-walking interpreter only |

---

## Summary of Unimplemented Features

**Large / Architectural:**
1. Static type system (type inference, occurs check, type errors at compile time)
2. Resource types (clone restriction, borrow semantics)
3. Module/import system (file resolution, module loading)
4. WebAssembly compilation backend

**Medium:**
5. Numeric literal coercion (byte from plain `4`, auto-widening)

**Small / Stdlib:**
6. Struct/tuple helpers (`map_tag`, `bind_tag`, merge, etc.)
7. Other bit-size numeric constructors
8. Function comparison specific error message
