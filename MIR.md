# MIR — Mid-level Intermediate Representation

The MIR is a desugared form of the AST with fewer, more uniform node types.
It sits between the AST (which mirrors surface syntax) and evaluation/codegen.

**Pipeline:** Source → Lexer → Parser → AST → **Type Check + Lower** → MIR → Eval

## Why MIR exists

The AST has many syntactic conveniences that are really combinations of simpler
operations:

- `+` is `.add()`, `==` is `.eq()`, `-x` is `x.negate()`
- `a >> f(x)` is `f(a, x)`
- `"hello {name}"` is `"hello ".add(name.to_string())`
- `1..3` is `(start=1, end=3)`
- `let(a, b)` on a struct is a chain of field accesses and simple binds
- `let[x, ...rest]` on an array is a chain of `.get()` / `.slice()` calls
- `(expr)` grouping is semantically transparent

The MIR eliminates all of these, leaving a small core that the evaluator (and
eventually a compiler) can target without duplicating desugaring logic.

## Current state

We have a partial MIR (`src/mir.rs`) that handles the easy desugarings:

- **Done:** BinOp → MethodCall, Compare → MethodCall, UnaryMinus → MethodCall
- **Done:** StringInterp → .add()/.to_string() chains
- **Done:** Range → struct literal `(start=a, end=b)`
- **Done:** Group → stripped
- **Done:** Pipe into calls/methods → inlined (`a >> f(x)` → `f(a, x)`)

But several desugarings are **blocked on the type system** and are passed
through as-is (the MIR still contains `Let`, `LetArray`, `Pipe`, `Apply`):

- **Let patterns** — `let(a, b)` must decide at lowering time whether to
  access by field name or by position. This depends on the type of the
  scrutinee, which is unknown without type information.
- **LetArray** — `let[x, ...rest]` desugars to `.get()`/`.slice()` calls, but
  the index arithmetic for `...rest` in the middle (`let[a, ...mid, b]`)
  needs to know the array length, which requires type-level length info or
  runtime inspection.
- **Pipe into let/apply** — `a >> let(x); body` needs to thread the pipe
  value as the `in` binding for the let, which interacts with scoping in ways
  that can't be expressed as simple Call without losing the `in` semantics.
- **Method dispatch** — `.method()` currently relies on runtime method set
  lookup. With types, it resolves to a direct function call.

## Target MIR (post type system)

Once the type system is in place, the MIR should have **no** sugar left.
Every node maps directly to a runtime/codegen operation.

### Design principles

- **Literal is a single kind.** The MIR doesn't have types, it has *kinds*
  (structural categories). All literal values — int, float, bool, string,
  char, byte, unit — share one kind: `Literal`. The distinction between them
  is a type-level concern resolved during type checking; the MIR just carries
  the value. A literal constructor (e.g. `Int(42)`) is inferred alongside
  types.

- **No separate Array or Struct kinds.** Arrays and structs are values, not
  declarations. An array literal `[1, 2, 3]` and a struct literal `(a=1, b=2)`
  are both value constructors — syntactic forms that produce values. In the
  MIR they appear as `ArrayLiteral` and `StructLiteral` (constructing a value
  from sub-expressions), not as distinct "collection" kinds. A function
  returning an array just returns a value; the array-ness is a type property,
  not an MIR node property.

- **Lambda vs Match are distinct.** `{ body }` (expression lambda) and
  `{ pattern -> expr, ... }` (pattern match lambda) are structurally
  different: one has a body expression, the other has arms. They deserve
  separate kind names: `Lambda` and `Match`.

- **Import means host import.** By the time MIR lowering is done, nana-to-nana
  module imports have been inlined. The only remaining `import` is for host-
  provided (FFI) functions. The kind is named `HostImport`.

- **NewTag is just an ID.** `NewTag(id)` — the tag's display name (e.g.
  `"Foo"` from `tag(Foo)`) is a presentation concern, not a semantic one.
  Display names live in a separate lookup table (tag ID → name), populated
  during parsing. The MIR only needs the ID for construction, pattern
  matching, and equality.

```
MirKind:
    // Values
    Literal(LiteralValue)      // int, float, bool, string, char, byte, unit
    ArrayLiteral(Vec<Mir>)     // [a, b, c]
    StructLiteral(Vec<Field>)  // (a=x, b=y) or (x, y)

    // Reference
    Ident(String)

    // Lambda forms
    Lambda(Mir)                          // { body }
    Match(Vec<MatchArm>)                 // { pattern -> expr, ... }

    // Access + calls
    FieldAccess(Mir, String)
    Call(Mir, Mir)

    // Binding — single name only
    Bind { name: String, value: Mir, body: Mir }

    // Tag constructor
    NewTag(u64)

    // Host-provided function import
    HostImport(String)
```

### What's gone

| AST / current MIR node | Desugars to | Requires types? |
|---|---|---|
| `BinOp(+, a, b)` | `Call(resolved_add_fn, (a, b))` | Yes — need receiver type to pick the right `add` |
| `Compare(==, a, b)` | `Call(resolved_eq_fn, (a, b))` | Yes |
| `UnaryMinus(a)` | `Call(resolved_negate_fn, a)` | Yes |
| `MethodCall(recv, name, arg)` | `Call(resolved_fn, (recv, arg))` | Yes — method → concrete function |
| `Pipe(a, f)` | `Call(f, a)` or `Bind` chain | Partially — pipe-into-let needs type info for destructuring |
| `Let { pattern, body }` | Chain of `Bind` + `FieldAccess` | Yes — name-vs-positional heuristic |
| `LetArray { patterns, body }` | Chain of `Bind` + `Call(.get)` + `Call(.slice)` | Yes — rest pattern index math |
| `Apply { expr, body }` | Gone entirely — methods resolved statically | Yes |
| `StringInterp` | Chain of `Call(add, Call(to_string, x))` | Already done (partially — still uses MethodCall) |
| `Range(a, b)` | `StructLiteral([(start, a), (end, b)])` | Already done |
| `Group(x)` | `x` | Already done |

### Key insight: method resolution replaces Apply

In the current system, `apply(method_set)` activates methods in lexical scope,
and `.method()` does a runtime lookup. With types:

1. The type checker knows every value's type.
2. Method sets are associated with types statically (via `apply` scoping, but
   resolved at check time).
3. Every `.method(arg)` becomes `Call(concrete_function, (receiver, arg))`.
4. `Apply` disappears — it only exists to inform the type checker which methods
   are in scope. After type checking, all methods are resolved.
5. `MethodCall` disappears — it's replaced by a direct `Call`.

This means operators like `+` go from:

```
AST:       BinOp(Add, a, b)
Current:   MethodCall { receiver: a, method: "add", arg: b }  (runtime lookup)
Target:    Call(int_add, StructLiteral([a, b]))                (direct call)
```

### Let pattern desugaring

With types, the lowering pass knows the struct's field layout:

```nana
(x=1, y=2) >> let(x, y); x + y
```

Type checker sees `(x=1, y=2)` has type `(x: Int, y: Int)`. The pattern
`let(x, y)` matches by name (fields `x` and `y` exist). Lowering:

```
Bind { name: "\0tmp", value: <the struct>,
  Bind { name: "x", value: FieldAccess(Ident("\0tmp"), "x"),
    Bind { name: "y", value: FieldAccess(Ident("\0tmp"), "y"),
      <body> } } }
```

For positional patterns on a positional struct `(1, 2) >> let(a, b)`:

```
Bind { name: "\0tmp", value: <the struct>,
  Bind { name: "a", value: FieldAccess(Ident("\0tmp"), "0"),
    Bind { name: "b", value: FieldAccess(Ident("\0tmp"), "1"),
      <body> } } }
```

The type system resolves the ambiguity; the MIR lowering is mechanical.

### LetArray desugaring

```nana
[10, 20, 30] >> let[a, ...rest]
```

Type checker knows the value is `Array(Int)`. Lowering:

```
Bind { name: "\0arr", value: <the array>,
  Bind { name: "a", value: Call(array_get, (\0arr, 0)),
    Bind { name: "rest", value: Call(array_slice, (\0arr, (start=1, end=Call(array_len, \0arr)))),
      <body> } } }
```

Middle rest patterns like `let[a, ...mid, b]`:

```
Bind { name: "\0arr", ...,
  Bind { name: "a", value: Call(array_get, (\0arr, 0)),
    Bind { name: "\0len", value: Call(array_len, \0arr),
      Bind { name: "b", value: Call(array_get, (\0arr, Call(int_subtract, (\0len, 1)))),
        Bind { name: "mid", value: Call(array_slice, (\0arr, (start=1, end=Call(int_subtract, (\0len, 1))))),
          <body> } } } } }
```

### Pipe desugaring

Most pipes are already handled. The remaining case is `a >> let(pat); body`:

```
a >> let(x, y); body
```

With types, this becomes:

```
Bind { name: "\0tmp", value: a,
  Bind { name: "x", value: FieldAccess(Ident("\0tmp"), "x"),   # or "0"
    Bind { name: "y", value: FieldAccess(Ident("\0tmp"), "y"), # or "1"
      <body with \0 bound to \0tmp for `in` access> } } }
```

The `\0` binding (the `in` override for pipe-into-let) becomes just another
`Bind` — no special `Pipe` node needed.

## How to get there

### Phase 1: Type inference (prerequisite)

Implement bidirectional type inference. At minimum:

- Infer types for all expressions (literals, idents, calls, field access, etc.)
- Track struct field layouts (named vs positional, field types)
- Resolve method calls: given a receiver type + method name + active method
  sets in scope, produce the concrete function

### Phase 2: Typed lowering

Replace the current `lower(expr: &Expr) -> Mir` with a two-step process:

1. **Type check:** `check(expr: &Expr, env: &TypeEnv) -> TypedExpr`
   — annotates every node with its type
2. **Lower:** `lower(expr: &TypedExpr) -> Mir`
   — uses type annotations to desugar everything fully

Or combine them: lower during type checking, emitting MIR as types are resolved.

### Phase 3: Remove runtime sugar handling

Once the MIR is fully desugared:

- Remove `bind_pattern`, `bind_array_pattern` from eval.rs
- Remove `eval_pipe`, `eval_pipe_collecting` from eval.rs
- Remove method set lookup (`find_method_in_method_sets`) from eval.rs
- Remove the `Apply` handling from eval.rs
- The evaluator becomes a straightforward tree-walker over the small MIR

### Phase 4: Implicit std

Once methods resolve statically, `use(std)` and its prelude of method sets
become a type-system concern (which method sets are in scope) rather than a
runtime concern (binding method sets into the environment). Programs get
`std` methods automatically because the type checker knows about them.
