use crate::ast::*;
use crate::value::*;

pub fn eval(expr: &Expr, env: &Env, input: &Value) -> Result<Value, String> {
    match expr.as_ref() {
        // ── Literals ──
        ExprKind::Int(n) => Ok(Value::Int(*n)),
        ExprKind::Float(f) => Ok(Value::Float(*f)),
        ExprKind::Bool(b) => Ok(Value::Bool(*b)),
        ExprKind::Str(s) => Ok(Value::Str(s.clone())),
        ExprKind::Char(c) => Ok(Value::Char(*c)),
        ExprKind::Byte(b) => Ok(Value::Byte(*b)),
        ExprKind::Unit => Ok(Value::Unit),

        // ── Variable reference ──
        ExprKind::Ident(name) if name == "in" => Ok(input.clone()),
        ExprKind::Ident(name) => env
            .get(name)
            .cloned()
            .ok_or_else(|| format!("undefined variable: {}", name)),

        // ── Block (lambda) ──
        ExprKind::Block(body) => Ok(Value::Closure {
            body: body.clone(),
            env: env.clone(),
        }),

        // ── Branching block (pattern matching lambda) ──
        ExprKind::BranchBlock(arms) => Ok(Value::BranchClosure {
            arms: arms.clone(),
            env: env.clone(),
        }),

        // ── Array ──
        ExprKind::Array(elems) => {
            let values: Result<Vec<Value>, String> =
                elems.iter().map(|e| eval(e, env, input)).collect();
            Ok(Value::Array(values?))
        }

        // ── Struct ──
        ExprKind::Struct(fields) => {
            let explicit_labels: Vec<String> = fields
                .iter()
                .filter(|f| !f.is_spread && f.label.is_some())
                .map(|f| f.label.clone().unwrap())
                .collect();

            // Reject duplicate named labels
            {
                let mut seen = Vec::new();
                for label in &explicit_labels {
                    if seen.contains(label) {
                        return Err(format!("duplicate field label '{}' in struct", label));
                    }
                    seen.push(label.clone());
                }
            }

            let mut result = Vec::new();
            let mut pos_index = 0u64;
            for field in fields {
                if field.is_spread {
                    let val = eval(&field.value, env, input)?;
                    match val {
                        Value::Struct(spread_fields) => {
                            for (label, v) in spread_fields {
                                if label.parse::<u64>().is_ok() {
                                    let new_label = pos_index.to_string();
                                    pos_index += 1;
                                    result.push((new_label, v));
                                } else if !explicit_labels.contains(&label) {
                                    if result.iter().any(|(l, _)| l == &label) {
                                        return Err(format!("duplicate field label '{}' in struct (from spread)", label));
                                    }
                                    result.push((label, v));
                                }
                            }
                        }
                        // () is the zero-field struct — spreading it is a no-op
                        Value::Unit => {}
                        _ => return Err("spread on non-struct value".to_string()),
                    }
                } else {
                    let val = eval(&field.value, env, input)?;
                    let label = field
                        .label
                        .clone()
                        .unwrap_or_else(|| {
                            let l = pos_index.to_string();
                            pos_index += 1;
                            l
                        });
                    result.push((label, val));
                }
            }
            Ok(Value::Struct(result))
        }

        // ── Field access ──
        ExprKind::FieldAccess(expr, field) => {
            let val = eval(expr, env, input)?;
            match val {
                Value::Struct(fields) => {
                    for (label, v) in &fields {
                        if label == field {
                            return Ok(v.clone());
                        }
                    }
                    Err(format!("field '{}' not found in struct", field))
                }
                _ => Err(format!("field access on non-struct value: .{}", field)),
            }
        }

        // ── Method call ──
        ExprKind::MethodCall { receiver, method, arg } => {
            let recv = eval(receiver, env, input)?;
            // If the receiver is a struct with a field matching the method name,
            // treat this as field access + function call rather than a method call.
            if let Value::Struct(ref fields) = recv {
                if let Some((_, field_val)) = fields.iter().find(|(l, _)| l == method) {
                    let func = field_val.clone();
                    let arg_val = eval(arg, env, input)?;
                    return apply(&func, arg_val);
                }
            }
            let arg_val = eval(arg, env, input)?;
            eval_method(&recv, method, arg_val)
        }

        // ── Function call ──
        ExprKind::Call(func_expr, arg_expr) => {
            let func = eval(func_expr, env, input)?;
            let arg = eval(arg_expr, env, input)?;
            apply(&func, arg)
        }

        // ── Unary minus ──
        ExprKind::UnaryMinus(expr) => {
            let val = eval(expr, env, input)?;
            match val {
                Value::Int(n) => n
                    .checked_neg()
                    .map(Value::Int)
                    .ok_or_else(|| "integer overflow in negation".to_string()),
                Value::Float(f) => Ok(Value::Float(-f)),
                _ => Err("unary minus on non-numeric value".to_string()),
            }
        }

        // ── Binary operators ──
        ExprKind::BinOp(op, lhs, rhs) => {
            let l = eval(lhs, env, input)?;
            let r = eval(rhs, env, input)?;
            eval_binop(*op, &l, &r)
        }

        // ── Comparisons ──
        ExprKind::Compare(op, lhs, rhs) => {
            let l = eval(lhs, env, input)?;
            let r = eval(rhs, env, input)?;
            eval_compare(*op, &l, &r)
        }

        // ── Pipe ──
        ExprKind::Pipe(lhs, rhs) => {
            let lhs_val = eval(lhs, env, input)?;
            eval_pipe(&lhs_val, rhs, env, input)
        }

        // ── Let binding ──
        ExprKind::Let { pattern, body } => {
            let new_env = bind_pattern(pattern, input, env)?;
            let new_env = new_env.bind("\0".to_string(), input.clone());
            eval(body, &new_env, input)
        }

        // ── Let array destructuring ──
        ExprKind::LetArray { patterns, body } => {
            let new_env = bind_array_pattern(patterns, input, env)?;
            let new_env = new_env.bind("\0".to_string(), input.clone());
            eval(body, &new_env, input)
        }

        // ── NewTag ──
        ExprKind::NewTag(id, name) => Ok(Value::TagConstructor {
            id: *id,
            name: name
                .clone()
                .unwrap_or_else(|| format!("tag_{}", id)),
        }),

        // ── Range ──
        ExprKind::Range(start, end) => {
            let s = eval(start, env, input)?;
            let e = eval(end, env, input)?;
            Ok(Value::Struct(vec![
                ("start".to_string(), s),
                ("end".to_string(), e),
            ]))
        }

        // ── Import ──
        ExprKind::Import(name) => Err(format!("import not available: {}", name)),

        // ── Group (transparent) ──
        ExprKind::Group(inner) => eval(inner, env, input),
    }
}

/// Apply a function value to an argument.
pub fn apply(func: &Value, arg: Value) -> Result<Value, String> {
    match func {
        Value::Closure { body, env } => {
            eval(body, env, &arg)
        }
        Value::BranchClosure { arms, env } => {
            eval_branch(&arg, arms, env, &arg)
        }
        Value::TagConstructor { id, name } => Ok(Value::Tagged {
            id: *id,
            name: name.clone(),
            payload: Box::new(arg),
        }),
        Value::BuiltinFn(name) => eval_builtin(name, arg),
        _ => Err(format!("cannot call non-function value: {}", func)),
    }
}

/// Evaluate pipe: lhs >> rhs.
fn eval_pipe(lhs_val: &Value, rhs: &Expr, env: &Env, input: &Value) -> Result<Value, String> {
    match rhs.as_ref() {
        // value >> f(args) → f(value, args)
        ExprKind::Call(func_expr, arg_expr) => {
            let func = eval(func_expr, env, input)?;
            let extra_arg = eval(arg_expr, env, input)?;
            let combined = match extra_arg {
                Value::Struct(mut fields) => {
                    let mut new_fields = vec![("0".to_string(), lhs_val.clone())];
                    for (label, val) in fields.drain(..) {
                        if let Ok(n) = label.parse::<u64>() {
                            new_fields.push(((n + 1).to_string(), val));
                        } else {
                            new_fields.push((label, val));
                        }
                    }
                    Value::Struct(new_fields)
                }
                Value::Unit => lhs_val.clone(),
                single => Value::Struct(vec![
                    ("0".to_string(), lhs_val.clone()),
                    ("1".to_string(), single),
                ]),
            };
            apply(&func, combined)
        }

        // value >> let { ... }
        ExprKind::Let { pattern, body } => {
            let new_env = bind_pattern(pattern, lhs_val, env)?;
            // Bind the hidden passthrough variable so multi-field patterns
            // can pass through the original piped value.
            let new_env = new_env.bind("\0".to_string(), lhs_val.clone());
            eval(body, &new_env, input)
        }

        ExprKind::LetArray { patterns, body } => {
            let new_env = bind_array_pattern(patterns, lhs_val, env)?;
            let new_env = new_env.bind("\0".to_string(), lhs_val.clone());
            eval(body, &new_env, input)
        }

        // value >> receiver.method(args) → prepend piped value to method args
        ExprKind::MethodCall { receiver, method, arg } => {
            let recv = eval(receiver, env, input)?;
            let extra_arg = eval(arg, env, input)?;
            let combined = match extra_arg {
                Value::Struct(mut fields) => {
                    let mut new_fields = vec![("0".to_string(), lhs_val.clone())];
                    for (label, val) in fields.drain(..) {
                        if let Ok(n) = label.parse::<u64>() {
                            new_fields.push(((n + 1).to_string(), val));
                        } else {
                            new_fields.push((label, val));
                        }
                    }
                    Value::Struct(new_fields)
                }
                Value::Unit => lhs_val.clone(),
                single => Value::Struct(vec![
                    ("0".to_string(), lhs_val.clone()),
                    ("1".to_string(), single),
                ]),
            };
            // Struct field takes priority over method dispatch (BUG-45)
            if let Value::Struct(ref fields) = recv {
                if let Some((_, field_val)) = fields.iter().find(|(l, _)| l == method) {
                    let func = field_val.clone();
                    return apply(&func, combined);
                }
            }
            eval_method(&recv, method, combined)
        }

        // value >> expr — eval rhs to a function, apply to value
        _ => {
            let rhs_val = eval(rhs, env, input)?;
            apply(&rhs_val, lhs_val.clone())
        }
    }
}

/// Evaluate branching: match scrutinee against branch arms.
fn eval_branch(scrutinee: &Value, arms: &[BranchArm], env: &Env, input: &Value) -> Result<Value, String> {
    for arm in arms {
        if let Some(arm_env) = match_branch_pattern(&arm.pattern, scrutinee, env)? {
            // Check guard
            if let Some(guard) = &arm.guard {
                let guard_val = eval(guard, &arm_env, input)?;
                match guard_val {
                    Value::Bool(true) => {}
                    Value::Bool(false) => continue,
                    _ => return Err("branch guard must be boolean".to_string()),
                }
            }
            return eval(&arm.body, &arm_env, input);
        }
    }
    Err(format!("non-exhaustive match: no arm matched value '{}'", scrutinee))
}

/// Try to match a branch pattern against a value.
/// Returns Some(env) if it matches, None if it doesn't.
fn match_branch_pattern(pattern: &BranchPattern, value: &Value, env: &Env) -> Result<Option<Env>, String> {
    match pattern {
        BranchPattern::Discard => Ok(Some(env.clone())),
        BranchPattern::Binding(name) => {
            // Check if the name refers to a tag constructor in the environment
            if let Some(tag_ctor) = env.get(name) {
                if let Value::TagConstructor { id: ctor_id, .. } = tag_ctor {
                    // It's a tag — match against tagged values with unit payload
                    if let Value::Tagged { id, payload, .. } = value {
                        if id == ctor_id && matches!(**payload, Value::Unit) {
                            return Ok(Some(env.clone()));
                        }
                    }
                    return Ok(None);
                }
            }
            // Not a tag — catch-all binding
            Ok(Some(env.bind(name.clone(), value.clone())))
        }
        BranchPattern::Tag(tag_name, binding) => {
            // Look up the tag constructor
            let tag_ctor = env.get(tag_name)
                .ok_or_else(|| format!("undefined tag in branch pattern: {}", tag_name))?;
            let ctor_id = match tag_ctor {
                Value::TagConstructor { id, .. } => *id,
                _ => return Err(format!("'{}' is not a tag constructor", tag_name)),
            };
            // Match against tagged value
            if let Value::Tagged { id, payload, .. } = value {
                if *id == ctor_id {
                    let mut arm_env = env.clone();
                    if let Some(b) = binding {
                        match b {
                            BranchBinding::Name(name) => {
                                arm_env = arm_env.bind(name.clone(), payload.as_ref().clone());
                            }
                            BranchBinding::Discard => {}
                        }
                    }
                    return Ok(Some(arm_env));
                }
            }
            Ok(None)
        }
        BranchPattern::Literal(lit_expr) => {
            // Evaluate the literal pattern (it's always a simple literal)
            let lit_val = eval(lit_expr, env, &Value::Unit)?;
            // Compare with the scrutinee; type mismatches mean no match
            match eval_compare(CmpOp::Eq, value, &lit_val) {
                Ok(Value::Bool(true)) => Ok(Some(env.clone())),
                Ok(_) | Err(_) => Ok(None),
            }
        }
    }
}

/// Bind a pattern to a value, extending the environment.
fn bind_pattern(pattern: &Pattern, value: &Value, env: &Env) -> Result<Env, String> {
    match pattern {
        Pattern::Name(name) => Ok(env.bind(name.clone(), value.clone())),
        Pattern::Discard => Ok(env.clone()),
        Pattern::Fields(fields) => {
            // () is the zero-field struct per spec — treat Unit as Struct([])
            let empty_fields = vec![];
            let struct_fields = match value {
                Value::Struct(f) => f,
                Value::Unit => &empty_fields,
                _ => return Err("cannot destructure non-struct value with let(...)".to_string()),
            };

            // Determine if we should bind by name or positionally.
            // Check if all pattern field names (non-rest, non-labeled) match named fields in the struct.
            let unlabeled_fields: Vec<&PatField> = fields.iter()
                .filter(|f| !f.is_rest && f.label.is_none() && f.binding != "_")
                .collect();

            let has_explicit_labels = fields.iter().any(|f| f.label.is_some());

            // If there are explicit labels (let(a=x)), use named binding always.
            // If all pattern names match struct field names, bind by name.
            // Otherwise, bind positionally.
            let bind_by_name = if has_explicit_labels {
                true
            } else if unlabeled_fields.is_empty() {
                // Only rest/discard patterns — use named if struct has named fields
                struct_fields.iter().any(|(l, _)| l.parse::<u64>().is_err())
            } else {
                // Check if ALL unlabeled field names exist as named fields in the struct
                let all_match = unlabeled_fields.iter().all(|pf| {
                    struct_fields.iter().any(|(l, _)| l == &pf.binding)
                });
                if all_match {
                    true
                } else {
                    // Check if it's a positional struct
                    let has_positional = struct_fields.iter().any(|(l, _)| l.parse::<u64>().is_ok());
                    if has_positional {
                        // Check that NONE of the names match (all-or-nothing)
                        let any_match = unlabeled_fields.iter().any(|pf| {
                            struct_fields.iter().any(|(l, _)| l == &pf.binding && l.parse::<u64>().is_err())
                        });
                        if any_match {
                            return Err("partial name match in destructuring: either all names must match struct fields or none".to_string());
                        }
                        false
                    } else {
                        let missing = unlabeled_fields.iter()
                            .find(|pf| !struct_fields.iter().any(|(l, _)| l == &pf.binding))
                            .map(|pf| pf.binding.as_str())
                            .unwrap_or(&unlabeled_fields[0].binding);
                        return Err(format!(
                            "field '{}' not found in struct",
                            missing
                        ));
                    }
                }
            };

            let mut new_env = env.clone();
            let mut used_indices = Vec::new();

            for pat_field in fields {
                if pat_field.is_rest {
                    let mut rest = Vec::new();
                    let mut rest_pos = 0u64;
                    for (i, (l, v)) in struct_fields.iter().enumerate() {
                        if used_indices.contains(&i) {
                            continue;
                        }
                        if l.parse::<u64>().is_ok() {
                            rest.push((rest_pos.to_string(), v.clone()));
                            rest_pos += 1;
                        } else {
                            rest.push((l.clone(), v.clone()));
                        }
                    }
                    if pat_field.binding != "_" {
                        new_env = new_env.bind(pat_field.binding.clone(), Value::Struct(rest));
                    }
                } else if let Some(label) = &pat_field.label {
                    // Explicit named field: let(label=binding)
                    let found = struct_fields
                        .iter()
                        .enumerate()
                        .find(|(_, (l, _))| l == label);
                    match found {
                        Some((i, (_, v))) => {
                            used_indices.push(i);
                            if pat_field.binding != "_" {
                                new_env = new_env.bind(pat_field.binding.clone(), v.clone());
                            }
                        }
                        None => return Err(format!("field '{}' not found in struct", label)),
                    }
                } else if bind_by_name {
                    if pat_field.binding == "_" {
                        // Discard — consume the next unused field positionally
                        if let Some((i, _)) = struct_fields.iter().enumerate().find(|(i, _)| !used_indices.contains(i)) {
                            used_indices.push(i);
                        }
                    } else {
                        // Bind by name: look up field by pattern binding name
                        let found = struct_fields
                            .iter()
                            .enumerate()
                            .find(|(_, (l, _))| l == &pat_field.binding);
                        match found {
                            Some((i, (_, v))) => {
                                used_indices.push(i);
                                new_env = new_env.bind(pat_field.binding.clone(), v.clone());
                            }
                            None => {
                                return Err(format!("field '{}' not found in struct", pat_field.binding));
                            }
                        }
                    }
                } else {
                    // Bind positionally
                    let idx = used_indices.len();
                    let pos_label = idx.to_string();
                    let found = struct_fields
                        .iter()
                        .enumerate()
                        .find(|(i, (l, _))| !used_indices.contains(i) && l == &pos_label);
                    match found {
                        Some((i, (_, v))) => {
                            used_indices.push(i);
                            if pat_field.binding != "_" {
                                new_env = new_env.bind(pat_field.binding.clone(), v.clone());
                            }
                        }
                        None => {
                            return Err(format!(
                                "positional field '{}' not found in struct",
                                pos_label
                            ));
                        }
                    }
                }
            }
            // Check for unconsumed fields
            let has_rest = fields.iter().any(|f| f.is_rest);
            if !has_rest && used_indices.len() < struct_fields.len() {
                return Err(format!(
                    "too many fields in struct: pattern expects {} but got {}",
                    fields.len(),
                    struct_fields.len()
                ));
            }
            Ok(new_env)
        }
    }
}

/// Bind array destructuring pattern.
fn bind_array_pattern(
    patterns: &[ArrayPat],
    value: &Value,
    env: &Env,
) -> Result<Env, String> {
    let elems = match value {
        Value::Array(e) => e,
        Value::Str(_) => {
            return Err("string destructuring not yet implemented".to_string());
        }
        _ => return Err("cannot destructure non-array value with let[...]".to_string()),
    };

    let mut new_env = env.clone();
    let mut pos = 0;

    for (i, pat) in patterns.iter().enumerate() {
        match pat {
            ArrayPat::Name(name) => {
                if pos >= elems.len() {
                    return Err("not enough elements in array for destructuring".to_string());
                }
                new_env = new_env.bind(name.clone(), elems[pos].clone());
                pos += 1;
            }
            ArrayPat::Discard => {
                if pos >= elems.len() {
                    return Err("not enough elements in array for destructuring".to_string());
                }
                pos += 1;
            }
            ArrayPat::Rest(name) => {
                let remaining_pats = patterns.len() - i - 1;
                let rest_end = elems.len().saturating_sub(remaining_pats);
                if pos > rest_end {
                    return Err("not enough elements in array for destructuring".to_string());
                }
                let rest = elems[pos..rest_end].to_vec();
                if let Some(name) = name {
                    new_env = new_env.bind(name.clone(), Value::Array(rest));
                }
                pos = rest_end;
            }
        }
    }
    let has_rest = patterns.iter().any(|p| matches!(p, ArrayPat::Rest(_)));
    if !has_rest && pos < elems.len() {
        return Err(format!(
            "too many elements in array: pattern expects {} but got {}",
            patterns.len(),
            elems.len()
        ));
    }
    Ok(new_env)
}

// ── Method dispatch ─────────────────────────────────────────────

fn eval_method(receiver: &Value, method: &str, arg: Value) -> Result<Value, String> {
    match receiver {
        Value::Array(elems) => eval_array_method(elems, method, arg),
        Value::Str(s) => eval_string_method(s, method, arg),
        _ => Err(format!("no method '{}' on value: {}", method, receiver)),
    }
}

fn eval_array_method(elems: &[Value], method: &str, arg: Value) -> Result<Value, String> {
    match method {
        "get" => {
            let idx = match arg {
                Value::Int(i) => i,
                _ => return Err("get: expected integer index".to_string()),
            };
            if idx < 0 {
                return Err(format!("negative array index: {}", idx));
            }
            let idx = idx as usize;
            elems
                .get(idx)
                .cloned()
                .ok_or_else(|| format!("array index {} out of bounds (len {})", idx, elems.len()))
        }
        "slice" => {
            // arg should be a range struct (start=n, end=m)
            let (start, end) = match arg {
                Value::Struct(fields) => {
                    let s = fields.iter().find(|(l, _)| l == "start")
                        .map(|(_, v)| v.clone())
                        .ok_or("slice: expected range with 'start' field")?;
                    let e = fields.iter().find(|(l, _)| l == "end")
                        .map(|(_, v)| v.clone())
                        .ok_or("slice: expected range with 'end' field")?;
                    match (s, e) {
                        (Value::Int(s), Value::Int(e)) => (s, e),
                        _ => return Err("slice: start and end must be integers".to_string()),
                    }
                }
                _ => return Err("slice: expected a range argument".to_string()),
            };
            if start < 0 || end < 0 {
                return Err("slice: negative index".to_string());
            }
            let start = start as usize;
            let end = end as usize;
            if start > elems.len() || end > elems.len() || start > end {
                return Err(format!("slice: indices {}..{} out of bounds (len {})", start, end, elems.len()));
            }
            Ok(Value::Array(elems[start..end].to_vec()))
        }
        "len" => Ok(Value::Int(elems.len() as i64)),
        "map" => {
            let func = arg;
            let result: Result<Vec<Value>, String> =
                elems.iter().map(|v| apply(&func, v.clone())).collect();
            Ok(Value::Array(result?))
        }
        "filter" => {
            let func = arg;
            let mut result = Vec::new();
            for v in elems {
                let keep = apply(&func, v.clone())?;
                match keep {
                    Value::Bool(true) => result.push(v.clone()),
                    Value::Bool(false) => {}
                    _ => return Err("filter: predicate must return bool".to_string()),
                }
            }
            Ok(Value::Array(result))
        }
        "fold" => {
            // arg should be (init, func)
            match arg {
                Value::Struct(fields) if fields.len() == 2 => {
                    let mut acc = fields[0].1.clone();
                    let func = &fields[1].1;
                    for v in elems {
                        let pair = Value::Struct(vec![
                            ("acc".to_string(), acc),
                            ("elem".to_string(), v.clone()),
                        ]);
                        acc = apply(func, pair)?;
                    }
                    Ok(acc)
                }
                _ => Err("fold: expected (init, function)".to_string()),
            }
        }
        "zip" => {
            // arg should be another array
            match arg {
                Value::Array(other) => {
                    let result: Vec<Value> = elems
                        .iter()
                        .zip(other)
                        .map(|(a, b)| {
                            Value::Struct(vec![
                                ("0".to_string(), a.clone()),
                                ("1".to_string(), b),
                            ])
                        })
                        .collect();
                    Ok(Value::Array(result))
                }
                _ => Err("zip: expected an array argument".to_string()),
            }
        }
        _ => Err(format!("no method '{}' on array", method)),
    }
}

fn eval_string_method(s: &str, method: &str, _arg: Value) -> Result<Value, String> {
    match method {
        "len" => Ok(Value::Int(s.len() as i64)),
        _ => Err(format!("no method '{}' on string", method)),
    }
}

// ── Binary operators ─────────────────────────────────────────────

fn eval_binop(op: BinOp, lhs: &Value, rhs: &Value) -> Result<Value, String> {
    match (op, lhs, rhs) {
        // Int arithmetic
        (BinOp::Add, Value::Int(a), Value::Int(b)) => a
            .checked_add(*b)
            .map(Value::Int)
            .ok_or_else(|| "integer overflow in addition".to_string()),
        (BinOp::Sub, Value::Int(a), Value::Int(b)) => a
            .checked_sub(*b)
            .map(Value::Int)
            .ok_or_else(|| "integer overflow in subtraction".to_string()),
        (BinOp::Mul, Value::Int(a), Value::Int(b)) => a
            .checked_mul(*b)
            .map(Value::Int)
            .ok_or_else(|| "integer overflow in multiplication".to_string()),
        (BinOp::Div, Value::Int(_), Value::Int(0)) => Err("division by zero".to_string()),
        (BinOp::Div, Value::Int(a), Value::Int(b)) => a
            .checked_div(*b)
            .map(Value::Int)
            .ok_or_else(|| "integer overflow in division".to_string()),

        // Float arithmetic
        (BinOp::Add, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        (BinOp::Sub, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
        (BinOp::Mul, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
        (BinOp::Div, Value::Float(_), Value::Float(b)) if *b == 0.0 => {
            Err("division by zero".to_string())
        }
        (BinOp::Div, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),

        // Mixed int/float
        (BinOp::Add, Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
        (BinOp::Add, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + *b as f64)),
        (BinOp::Sub, Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 - b)),
        (BinOp::Sub, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - *b as f64)),
        (BinOp::Mul, Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 * b)),
        (BinOp::Mul, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * *b as f64)),
        (BinOp::Div, Value::Int(_), Value::Float(b)) if *b == 0.0 => {
            Err("division by zero".to_string())
        }
        (BinOp::Div, Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 / b)),
        (BinOp::Div, Value::Float(_), Value::Int(0)) => Err("division by zero".to_string()),
        (BinOp::Div, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a / *b as f64)),

        // String concatenation
        (BinOp::Add, Value::Str(a), Value::Str(b)) => Ok(Value::Str(format!("{}{}", a, b))),

        // Array concatenation
        (BinOp::Add, Value::Array(a), Value::Array(b)) => {
            let mut result = a.clone();
            result.extend(b.iter().cloned());
            Ok(Value::Array(result))
        }

        _ => Err(format!(
            "invalid operands for {:?}: {} and {}",
            op, lhs, rhs
        )),
    }
}

fn eval_compare(op: CmpOp, lhs: &Value, rhs: &Value) -> Result<Value, String> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(compare_partial(op, a, b)?)),
        (Value::Int(a), Value::Float(b)) => {
            Ok(Value::Bool(compare_partial(op, &(*a as f64), b)?))
        }
        (Value::Float(a), Value::Int(b)) => {
            Ok(Value::Bool(compare_partial(op, a, &(*b as f64))?))
        }
        (Value::Str(a), Value::Str(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::Char(a), Value::Char(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::Byte(a), Value::Byte(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::Unit, Value::Unit) => match op {
            CmpOp::Eq => Ok(Value::Bool(true)),
            CmpOp::NotEq => Ok(Value::Bool(false)),
            _ => Err("cannot order unit values".to_string()),
        },
        // Tagged values
        (
            Value::Tagged {
                id: id1,
                payload: p1,
                ..
            },
            Value::Tagged {
                id: id2,
                payload: p2,
                ..
            },
        ) => match op {
            CmpOp::Eq => {
                if id1 != id2 {
                    Ok(Value::Bool(false))
                } else {
                    eval_compare(CmpOp::Eq, p1, p2)
                }
            }
            CmpOp::NotEq => {
                if id1 != id2 {
                    Ok(Value::Bool(true))
                } else {
                    eval_compare(CmpOp::NotEq, p1, p2)
                }
            }
            _ => Err("cannot order tagged values".to_string()),
        },
        // Array comparison
        (Value::Array(a), Value::Array(b)) => match op {
            CmpOp::Eq => {
                if a.len() != b.len() {
                    return Ok(Value::Bool(false));
                }
                for (x, y) in a.iter().zip(b.iter()) {
                    match eval_compare(CmpOp::Eq, x, y)? {
                        Value::Bool(false) => return Ok(Value::Bool(false)),
                        _ => {}
                    }
                }
                Ok(Value::Bool(true))
            }
            CmpOp::NotEq => {
                if a.len() != b.len() {
                    return Ok(Value::Bool(true));
                }
                for (x, y) in a.iter().zip(b.iter()) {
                    match eval_compare(CmpOp::Eq, x, y)? {
                        Value::Bool(false) => return Ok(Value::Bool(true)),
                        _ => {}
                    }
                }
                Ok(Value::Bool(false))
            }
            _ => Err("cannot order arrays".to_string()),
        },
        // Unit == empty struct (since () is the zero-field struct)
        (Value::Unit, Value::Struct(b)) if b.is_empty() => match op {
            CmpOp::Eq => Ok(Value::Bool(true)),
            CmpOp::NotEq => Ok(Value::Bool(false)),
            _ => Err("cannot order unit values".to_string()),
        },
        (Value::Struct(a), Value::Unit) if a.is_empty() => match op {
            CmpOp::Eq => Ok(Value::Bool(true)),
            CmpOp::NotEq => Ok(Value::Bool(false)),
            _ => Err("cannot order unit values".to_string()),
        },
        // Struct comparison
        (Value::Struct(a), Value::Struct(b)) => {
            let all_named = |fields: &[(String, Value)]| {
                fields.iter().all(|(l, _)| l.parse::<u64>().is_err())
            };
            let eq = struct_eq(a, b, all_named(a) && all_named(b))?;
            match op {
                CmpOp::Eq => Ok(Value::Bool(eq)),
                CmpOp::NotEq => Ok(Value::Bool(!eq)),
                _ => Err("cannot order structs".to_string()),
            }
        }
        _ => Err(format!(
            "cannot compare values: {} and {}",
            lhs, rhs
        )),
    }
}

fn compare_ord<T: Ord>(op: CmpOp, a: &T, b: &T) -> bool {
    match op {
        CmpOp::Eq => a == b,
        CmpOp::NotEq => a != b,
        CmpOp::Lt => a < b,
        CmpOp::Gt => a > b,
        CmpOp::LtEq => a <= b,
        CmpOp::GtEq => a >= b,
    }
}

fn struct_eq(a: &[(String, Value)], b: &[(String, Value)], by_name: bool) -> Result<bool, String> {
    if a.len() != b.len() {
        return Ok(false);
    }
    if by_name {
        for (la, va) in a {
            let found = b.iter().find(|(lb, _)| lb == la);
            match found {
                Some((_, vb)) => {
                    match eval_compare(CmpOp::Eq, va, vb)? {
                        Value::Bool(false) => return Ok(false),
                        _ => {}
                    }
                }
                None => return Ok(false),
            }
        }
        Ok(true)
    } else {
        for ((la, va), (lb, vb)) in a.iter().zip(b.iter()) {
            if la != lb {
                return Ok(false);
            }
            match eval_compare(CmpOp::Eq, va, vb)? {
                Value::Bool(false) => return Ok(false),
                _ => {}
            }
        }
        Ok(true)
    }
}

fn compare_partial<T: PartialOrd>(op: CmpOp, a: &T, b: &T) -> Result<bool, String> {
    match op {
        CmpOp::Eq => Ok(a == b),
        CmpOp::NotEq => Ok(a != b),
        CmpOp::Lt => a.partial_cmp(b).map(|o| o.is_lt()).ok_or_else(|| "NaN comparison".to_string()),
        CmpOp::Gt => a.partial_cmp(b).map(|o| o.is_gt()).ok_or_else(|| "NaN comparison".to_string()),
        CmpOp::LtEq => a.partial_cmp(b).map(|o| o.is_le()).ok_or_else(|| "NaN comparison".to_string()),
        CmpOp::GtEq => a.partial_cmp(b).map(|o| o.is_ge()).ok_or_else(|| "NaN comparison".to_string()),
    }
}

// ── Builtin functions ────────────────────────────────────────────

fn eval_builtin(name: &str, arg: Value) -> Result<Value, String> {
    match name {
        "not" => match arg {
            Value::Bool(b) => Ok(Value::Bool(!b)),
            _ => Err("not: expected bool".to_string()),
        },
        "and" => match arg {
            Value::Struct(fields) if fields.len() == 2 => {
                match (&fields[0].1, &fields[1].1) {
                    (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a && *b)),
                    _ => Err("and: expected two bools".to_string()),
                }
            }
            _ => Err("and: expected (bool, bool)".to_string()),
        },
        "or" => match arg {
            Value::Struct(fields) if fields.len() == 2 => {
                match (&fields[0].1, &fields[1].1) {
                    (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a || *b)),
                    _ => Err("or: expected two bools".to_string()),
                }
            }
            _ => Err("or: expected (bool, bool)".to_string()),
        },
        "len" => match arg {
            Value::Array(a) => Ok(Value::Int(a.len() as i64)),
            Value::Str(s) => Ok(Value::Int(s.len() as i64)),
            _ => Err("len: expected array or string".to_string()),
        },
        "print" => {
            println!("{}", arg);
            Ok(Value::Unit)
        }
        "map" => match arg {
            Value::Struct(fields) if fields.len() == 2 => {
                let arr = match &fields[0].1 {
                    Value::Array(a) => a.clone(),
                    _ => return Err("map: first argument must be an array".to_string()),
                };
                let func = &fields[1].1;
                let result: Result<Vec<Value>, String> =
                    arr.into_iter().map(|v| apply(func, v)).collect();
                Ok(Value::Array(result?))
            }
            _ => Err("map: expected (array, function)".to_string()),
        },
        "filter" => match arg {
            Value::Struct(fields) if fields.len() == 2 => {
                let arr = match &fields[0].1 {
                    Value::Array(a) => a.clone(),
                    _ => return Err("filter: first argument must be an array".to_string()),
                };
                let func = &fields[1].1;
                let mut result = Vec::new();
                for v in arr {
                    let keep = apply(func, v.clone())?;
                    match keep {
                        Value::Bool(true) => result.push(v),
                        Value::Bool(false) => {}
                        _ => return Err("filter: predicate must return bool".to_string()),
                    }
                }
                Ok(Value::Array(result))
            }
            _ => Err("filter: expected (array, function)".to_string()),
        },
        "fold" => match arg {
            Value::Struct(fields) if fields.len() == 3 => {
                let arr = match &fields[0].1 {
                    Value::Array(a) => a.clone(),
                    _ => return Err("fold: first argument must be an array".to_string()),
                };
                let mut acc = fields[1].1.clone();
                let func = &fields[2].1;
                for v in arr {
                    let pair = Value::Struct(vec![
                        ("acc".to_string(), acc),
                        ("elem".to_string(), v),
                    ]);
                    acc = apply(func, pair)?;
                }
                Ok(acc)
            }
            _ => Err("fold: expected (array, init, function)".to_string()),
        },
        "zip" => match arg {
            Value::Struct(fields) if fields.len() == 2 => {
                let arr1 = match &fields[0].1 {
                    Value::Array(a) => a.clone(),
                    _ => return Err("zip: arguments must be arrays".to_string()),
                };
                let arr2 = match &fields[1].1 {
                    Value::Array(a) => a.clone(),
                    _ => return Err("zip: arguments must be arrays".to_string()),
                };
                let result: Vec<Value> = arr1
                    .into_iter()
                    .zip(arr2)
                    .map(|(a, b)| {
                        Value::Struct(vec![
                            ("0".to_string(), a),
                            ("1".to_string(), b),
                        ])
                    })
                    .collect();
                Ok(Value::Array(result))
            }
            _ => Err("zip: expected (array, array)".to_string()),
        },
        _ => Err(format!("unknown builtin function: {}", name)),
    }
}

/// Evaluate an expression and return both the result value and the
/// environment after all top-level bindings have been applied.
pub fn eval_toplevel(expr: &Expr, env: &Env, input: &Value) -> Result<(Value, Env), String> {
    eval_collecting(expr, env, input)
}

fn eval_collecting(expr: &Expr, env: &Env, input: &Value) -> Result<(Value, Env), String> {
    match expr.as_ref() {
        ExprKind::Pipe(lhs, rhs) if has_toplevel_let(rhs) => {
            let lhs_val = eval(lhs, env, input)?;
            eval_pipe_collecting(&lhs_val, rhs, env, input)
        }
        ExprKind::Let { pattern, body } => {
            let new_env = bind_pattern(pattern, input, env)?;
            let new_env = new_env.bind("\0".to_string(), input.clone());
            eval_collecting(body, &new_env, input)
        }
        ExprKind::LetArray { patterns, body } => {
            let new_env = bind_array_pattern(patterns, input, env)?;
            let new_env = new_env.bind("\0".to_string(), input.clone());
            eval_collecting(body, &new_env, input)
        }
        _ => {
            let val = eval(expr, env, input)?;
            Ok((val, env.clone()))
        }
    }
}

fn eval_pipe_collecting(lhs_val: &Value, rhs: &Expr, env: &Env, input: &Value) -> Result<(Value, Env), String> {
    match rhs.as_ref() {
        ExprKind::Let { pattern, body } => {
            let new_env = bind_pattern(pattern, lhs_val, env)?;
            let new_env = new_env.bind("\0".to_string(), lhs_val.clone());
            eval_collecting(body, &new_env, input)
        }
        ExprKind::LetArray { patterns, body } => {
            let new_env = bind_array_pattern(patterns, lhs_val, env)?;
            let new_env = new_env.bind("\0".to_string(), lhs_val.clone());
            eval_collecting(body, &new_env, input)
        }
        _ => {
            let val = eval_pipe(lhs_val, rhs, env, input)?;
            Ok((val, env.clone()))
        }
    }
}

fn has_toplevel_let(expr: &Expr) -> bool {
    match expr.as_ref() {
        ExprKind::Let { .. } | ExprKind::LetArray { .. } => true,
        _ => false,
    }
}

/// Create the default environment with builtins.
pub fn default_env() -> Env {
    let mut env = Env::new();
    for name in &["not", "and", "or", "len", "print", "map", "filter", "fold", "zip"] {
        env = env.bind(name.to_string(), Value::BuiltinFn(name.to_string()));
    }
    env
}
