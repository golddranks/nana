# Implementation Status

Cross-reference of DESIGN.md spec against the current interpreter implementation.

---

## Value Types

| Spec Feature | Status | Notes |
|---|---|---|
| `int` (64-bit) | Implemented | Literals, arithmetic, overflow checks |
| `float` (64-bit) | Implemented | Literals, arithmetic, div-by-zero check |
| `bool` (`true`/`false`) | Implemented | |
| `byte` (literal `b'a'`, `0xF4`) | Implemented | `b'a'` is explicit byte literal; `0xF4` is an int literal that coerces to byte via `IntLiteral` unification |
| `byte` (plain `4` auto-converts) | Implemented | Type checker validates int literal → `Byte` coercion via constrained Infer; runtime auto-coerces int 0..255 to byte in method dispatch |
| Hex integer literals (`0xFF`) | Implemented | Parsed by lexer; underscore separators supported |
| Binary integer literals (`0b1010`) | Implemented | Parsed by lexer; underscore separators supported |
| `char` (Unicode scalar) | Implemented | |
| `string` (UTF-8) | Implemented | Literals, concatenation, escape sequences |
| String interpolation `"Hello, {name}!"` | Implemented | Lexer produces `InterpStr` tokens; parser/eval handle nested expressions |
| Multi-line strings (`\\` prefix, Zig-style) | Implemented | Lexer joins `\\`-prefixed lines, strips leading whitespace |
| `as_bytes` (string to byte array) | Implemented | String method |
| `chars()` (string to codepoints) | Implemented | String method |
| `i32` (32-bit signed integer) | Implemented | `I32` type with full arithmetic, comparison, and conversion methods; `i32()` type hint constructor |
| `f32` (32-bit float) | Implemented | `F32` type with full arithmetic, comparison, rounding, and conversion methods; `f32()` type hint constructor |
| Literal-typed numerics / auto-coercion | Implemented | Int literals create constrained Infer variables that coerce to `Int`, `Byte`, or `I32`; float literals coerce to `Float` or `F32` |
| Unit `()` | Implemented | |
| Arrays `[1,2,3]` | Implemented | Creation, concatenation, destructuring |
| `arr.get(idx)` element access | Implemented | Method on arrays; bounds-checked |
| `arr.slice(range)` slicing | Implemented | Method on arrays; takes a range struct |
| Empty array `[]` type refinement | Implemented | `[]` is `Array(Infer(_))`, which unifies with any `Array(T)` via union-find. Unconstrained element types are defaulted to `Unit` after inference (`default_infer_in_arrays`), so standalone `[]` and `[].map{...}` are valid. |
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
| Function comparison rejection | Implemented | `==`/`!=` on functions gives specific error: "use ref_eq()" |
| `and`, `or`, `not` as functions | Implemented | Builtins, not operators |
| Branching blocks `{ pattern -> expr, ... }` | Implemented | Unified conditional / pattern matching form |
| `if` guards in branch arms | Implemented | `pattern if guard -> expr` |
| All arms must return same type | Implemented | Type checker unifies branch arm types (`types::unify`) |
| Non-exhaustive branch is error | Implemented | Runtime error (compile = eval in this language) |
| Ternary sugar `{ a \| b }` | Implemented | Desugars to `{ true -> a, false -> b }`; short-circuits |
| Parentheses for grouping | Implemented | |
| Pipe `>>` | Implemented | |
| Pipe prepend `value >> f(x)` = `f(value, x)` | Implemented | |
| `;` sequencing | Implemented | |
| Semicolons rejected in array/struct/call-arg | Implemented | `[1; 2]`, `(1, 2; 3)`, `f(1; 2)` are errors |
| `import(name)` | Implemented | Identifier-only syntax; resolved via host-provided modules |
| `use(name)` | Implemented | Desugars to `import(name) >> let(name)` |

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
| `str.byte_len()` | Implemented | Returns byte length |
| `str.char_len()` | Implemented | Returns Unicode codepoint count |
| `str.byte_get(idx)` | Implemented | Returns `Byte` at byte offset |
| `str.char_get(idx)` | Implemented | Returns `Char` at codepoint offset |

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
| All arms must return same type | Implemented | Type checker unifies branch arm types (`types::unify`) |
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
| `len` | Implemented | Builtin function and array method |
| `not` | Implemented | `not(bool)` |
| `and` | Implemented | `and(bool, bool)` |
| `or` | Implemented | `or(bool, bool)` |
| `print` | Implemented | Side-effecting, returns `()` |
| `get` | Implemented | Array method only (not a builtin function) |
| `slice` | Implemented | Array method only (not a builtin function) |
| `byte_len` | Implemented | String method; returns byte length |
| `char_len` | Implemented | String method; returns codepoint count |
| `as_bytes` | Implemented | String method; returns byte array |
| `chars` | Implemented | String method; returns array of chars |
| `split` | Implemented | String method; splits by delimiter |
| `trim` | Implemented | String method; strips leading/trailing whitespace |
| `contains` | Implemented | String method; checks for substring or char |
| `starts_with` | Implemented | String method; checks prefix |
| `ends_with` | Implemented | String method; checks suffix |
| `replace` | Implemented | String method; takes `(pattern, replacement)` struct |
| `str.slice(range)` | Implemented | String method; byte-based slicing |
| `byte(n)` | Implemented | Builtin; converts int (0..255) to byte |
| `int(x)` | Implemented | Builtin; converts float/byte/char/bool to int |
| `float(x)` | Implemented | Builtin; converts int to float |
| `char(n)` | Implemented | Builtin; converts int/byte to char (Unicode scalar validation) |
| `i32(x)` | Implemented | Type hint; I32 → I32 identity, coerces int literals at runtime |
| `f32(x)` | Implemented | Type hint; F32 → F32 identity, coerces float literals at runtime |
| `ref_eq(a, b)` | Implemented | Builtin; structural equality for any values including functions |

---

## Modules and Imports

| Spec Feature | Status | Notes |
|---|---|---|
| `import(name)` syntax | Implemented | Identifier-only; string syntax is a parse error |
| `use(name)` sugar | Implemented | Desugars to `import(name) >> let(name)` |
| Static import analysis | Implemented | `nana::imports()` extracts module names from AST |
| Host-provided modules | Implemented | `run_with_modules()` accepts `&[(&str, Value)]`; `Env` stores modules as `Rc<HashMap>` |
| Modules as structs | Implemented | Modules are regular `Value`s (typically structs with fields) |
| Same module yields same tag IDs | Implemented | Host provides same value; `import(x)` twice returns the same value |

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
| Static typing | Implemented | `types.rs`: forward checker with unification validates all expressions — literals, bindings, calls, method dispatch (2-stage: struct field, method set; method-not-found is a type error), branch arm consistency, struct/array construction, destructuring (struct + array patterns), tagged types, precise builtin param+return types, auto-apply prelude on `use(std)`, union/sum types for tagged branches, method param type checking with error propagation, numeric literal coercion via constrained Infer; `method_set` tracked as `Ty::MethodSetConstructor` (works through field access and aliasing); `ref_eq`/`val_eq` enforce same-type arguments via generics |
| Type inference | Implemented | Forward propagation + unification + bidirectional inference for block/branch `in` parameters (inline and stored); `Ty::Infer(u64)` inference variables for deferred type resolution (standalone blocks, empty arrays, pattern fallbacks) — each use site gets a unique ID via `TyEnv::fresh_infer()`; constrained Infer variables for numeric literals (`InferConstraint::IntLiteral` coerces to `Int`/`Byte`/`I32`, `InferConstraint::FloatLiteral` coerces to `Float`/`F32`); generic type variables (`Ty::Generic(id)`) for parametric methods — type information flows through `map`, `filter`, `fold`, `zip`, `get`, `slice`, `ref_eq`, `val_eq`, etc. via `unify_with_generics` + `substitute_generics`; tagged value method dispatch; built-in comparison fallback for types without method sets; rest pattern struct type propagation; tag payload resolution in branch patterns; literal defaulting in bindings; no annotation syntax needed (fully inferred) |
| Generic array methods | Implemented | Array method signatures use `Generic(0)`/`Generic(1)` type variables: `get: Array(T)→T`, `map: Array(T)×(T→U)→Array(U)`, `filter: Array(T)×(T→Bool)→Array(T)`, `fold: Array(T)×(U, (acc:U,elem:T)→U)→U`, `zip: Array(T)×Array(U)→Array((T,U))`, etc. |
| Occurs check | Implemented | Totality maintained by disallowing recursion syntactically; without recursive bindings or Y-combinator, infinite types cannot be constructed, so the occurs check is trivially satisfied |
| Compilation = evaluation | Implemented | Single pass |

---

## Type Inference Completeness

Programs whose result type contains unresolved inference variables (`Ty::Infer`) are rejected at the top level (`contains_infer` check in `lib.rs`). The items below track where the inference system fails to resolve types that *should* be resolvable according to the spec.

### Infrastructure

| Item | Status | Notes |
|---|---|---|
| `Ty::contains_infer()` recursive check | Implemented | Traverses Array, Fn, Struct, Tagged, Union |
| Top-level rejection of unresolved Infer | Implemented | `lib.rs` rejects programs whose result type contains Infer |
| BranchBlock shares `input_ty` with arm checking | Implemented | Was disconnected; now matches Block behavior |
| `block_bodies` re-check mechanism | Implemented | `let f = { body }` → `f(x)` re-checks body with known arg type. Extended for struct fields (`s.f(x)`) and method sets. |
| Union-find inference (`infer_subst`) | Implemented | `TyEnv::infer_subst: HashMap<u64, Ty>` records `Infer(id) → resolved type` during unification. `resolve()` chases chains. Constrained Infer variables (`InferConstraint::IntLiteral`, `FloatLiteral`) restrict which concrete types a numeric literal can resolve to. All `unify`/`unify_with_generics` calls now record bindings. |
| Branch exhaustiveness check | Implemented | `check_branch_block_with_input` validates non-exhaustive branches and undefined tags when input type is known |

### Actionable Inference Bugs

These are cases where programs that should type-check according to the spec fail with unresolved inference. Each item is a concrete fix.

| # | Bug | Tests | Root Cause | Fix |
|---|---|---|---|---|
| I1 | Named struct destructuring: `(x=1, y=2) >> let(x, y)` | 6 tests (`dual_let_by_name`, `bug47_*`, `probe17_let_destructure_then_use_both`, `probe18c_destructure_named`) | `check_let` always looks up fields positionally when pattern fields have no explicit label. For `let(x, y)` on `(x=1, y=2)`, it looks for fields "0" and "1" which don't exist, producing Infer. | **Fixed.** Added `bind_by_name` logic mirroring `eval.rs` to `check_let`. |
| I2 | String destructuring via `let[...]` | 10 tests (`string_destructure_*`, `let_array_assign_sugar_*`) | `check_let_array` only handles `Ty::Array(elem)`. When input is `Ty::String`, it falls through to `fresh_infer()`. | **Fixed.** Added `Ty::String` arm to `check_let_array`. |
| I3 | Struct field function calls: `obj.field(args)` | 15 tests | Struct fields containing blocks have type `Fn(Infer -> Infer)`. When called via `obj.field(args)`, the block body needs re-checking. | **Fixed.** Union-find + struct field `block_bodies` tracking + inline struct literal + FieldAccess in check_call + block body propagation through destructuring and function returns. All 15 tests pass. |
| I4 | Unit destructuring with rest: `() >> let(...rest)` | 3 tests (`bug55_unit_destructure_rest`, `bug55_unit_destructure_only_rest`, `probe24_empty_struct_let_destructure_rest`) | `check_let` rest pattern on a non-`Struct` input (Unit is not `Ty::Struct(vec![])`) returns `fresh_infer()`. | **Fixed.** Treat `Ty::Unit` as empty struct in rest-pattern branch. |
| I5 | Empty array operations: `[].map{...}`, `[].get(0)`, `[] + []`, `[] >> let[...]` | 9 tests | Empty array `[]` has element type `Infer`. | **Fixed.** Unconstrained `Infer` inside arrays is defaulted to `Unit` after inference completes (`default_infer_in_arrays`). Empty arrays are valid expressions — D2 revised. All 9 tests pass. |
| I6 | Tag branch matching failures | 5 tests (`minor_non_exhaustive_branch`, `edge8_match_untagged_with_tag_pattern`, `probe17_shadowed_tag_old_value_no_match`, `probe17b_undefined_tag_pattern_with_parens`, `probe17b_tag_identity_mismatch`) | Various: undefined tags in branch patterns, non-exhaustive branches. | **Fixed.** Per D5, non-exhaustive branches and undefined tags are now type errors. Added exhaustiveness check in `check_branch_block_with_input`. |
| I7 | Higher-order functions returning closures | 5 tests (`call_with_block`, `probe22b_higher_order_compose`, `probe22b_tagged_option_map`, `probe24_array_of_functions_get`, `probe24_array_of_functions_apply_via_let`) | Closures returned from other closures or retrieved from arrays have type `Fn(Infer -> Infer)`. | **Fixed.** Block body propagation through identity functions (`resolve_to_block_mir`), compose-like tail expressions (`find_tail_expr`), let destructuring (`propagate_block_bodies_to_fields`), tag pattern fix in `bind_branch_pattern`, and array element block body propagation via `"\0arr:name"` keys in `block_bodies`. All 5 tests pass. |
| I8 | Method sets: `method_set(Tag, struct_of_fns)` / `apply(ms)` | 6 tests | The closures inside the method struct have `Fn(Infer -> Infer)` types. | **Fixed.** Block bodies from method set registration are stored under `"\0ms{id}.{method}"` keys and re-checked when the method is called. All 6 tests pass. |
| I9 | Standalone closure as program result: `{ in + 1 }` | 1 test (`probe19c_closure_display`) | A block that is never called has type `Fn(Infer -> Infer)`. The `contains_infer` check rejects it. | **Fixed.** Per D1, correctly rejected. Test updated to `assert_error`. |

### Design Decisions (Resolved)

| # | Decision | Notes |
|---|---|---|
| D1 | Standalone closures as program results: **reject**. A program must produce a fully-typed value. Current behavior is correct. | |
| D2 | Empty arrays: **accept**. Unconstrained `Infer` inside `Array` is defaulted to `Unit` after inference. `[]` is a valid empty array. (Revised from earlier "reject" — defaulting is safe since no elements exist to observe the type.) | |
| D3 | Closures in data structures: **union-find inference + block body propagation**. Union-find links inference variables across uses. `block_bodies` HashMap stores MIR for deferred re-checking at call sites. Array element blocks propagated via `"\0arr:name"` keys. | Covers I3, I7, I8 |
| D4 | Higher-order function inference: **same as D3** — union-find inference + block body re-checking. All 5 I7 tests pass. | Covers I7 |
| D5 | Non-exhaustive branches: **type error**. Non-exhaustive `let` destructuring is also a type error. Note: inside branch arms, partial patterns are fine (the branch itself handles exhaustiveness). An `if let`-style construct may be needed in the future for partial `let` patterns outside branches. | Covers I6 |

---

## Compilation Target

| Spec Feature | Status | Notes |
|---|---|---|
| WebAssembly output | Not implemented | Tree-walking interpreter only |

---

## Summary of Unimplemented Features

**Large / Architectural:**
1. Resource types (clone restriction, borrow semantics)
2. WebAssembly compilation backend
