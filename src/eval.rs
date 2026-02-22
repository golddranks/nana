use std::sync::atomic::{AtomicU64, Ordering};

use crate::ast::*;
use crate::value::*;

static CLOSURE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_closure_id() -> u64 {
    CLOSURE_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub fn eval(expr: &Expr, env: &Env, input: &Value) -> Result<Value, String> {
    match expr.as_ref() {
        // ── Literals ──
        ExprKind::Int(n) => Ok(Value::Int(*n)),
        ExprKind::Float(f) => Ok(Value::Float(*f)),
        ExprKind::Bool(b) => Ok(Value::Bool(*b)),
        ExprKind::Str(s) => Ok(Value::Str(s.clone())),
        ExprKind::StringInterp(parts) => {
            let mut result = String::new();
            for part in parts {
                match part {
                    StringInterpPart::Literal(s) => result.push_str(s),
                    StringInterpPart::Expr(expr) => {
                        let val = eval(expr, env, input)?;
                        result.push_str(&val.print_string());
                    }
                }
            }
            Ok(Value::Str(result))
        }
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
            id: next_closure_id(),
            body: body.clone(),
            env: env.clone(),
        }),

        // ── Branching block (pattern matching lambda) ──
        ExprKind::BranchBlock(arms) => Ok(Value::BranchClosure {
            id: next_closure_id(),
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
            // Check method sets for tagged values and primitive types
            let type_id = match &recv {
                Value::Tagged { id, .. } => Some(*id),
                other => builtin_tag_id(other),
            };
            if let Some(tid) = type_id {
                if let Some(func) = env.find_method_in_method_sets(tid, method) {
                    let func = func.clone();
                    let arg_val = eval(arg, env, input)?;
                    let combined = prepend_arg(&recv, arg_val);
                    return apply(&func, combined);
                }
            }
            Err(format!("no method '{}' on {}", method, recv))
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
        ExprKind::Import(name) => env
            .get_module(name)
            .cloned()
            .ok_or_else(|| format!("module not provided: {}", name)),

        // ── Apply (method set scope) ──
        ExprKind::Apply { expr, body } => {
            let ms = eval(expr, env, input)?;
            match &ms {
                Value::MethodSet { .. } => {
                    let new_env = env.bind(format!("\0ms"), ms);
                    eval(body, &new_env, input)
                }
                _ => Err("apply: expected a method set value".to_string()),
            }
        }

        // ── Group (transparent) ──
        ExprKind::Group(inner) => eval(inner, env, input),
    }
}

/// Apply a function value to an argument.
pub fn apply(func: &Value, arg: Value) -> Result<Value, String> {
    match func {
        Value::Closure { body, env, .. } => {
            eval(body, env, &arg)
        }
        Value::BranchClosure { arms, env, .. } => {
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

        ExprKind::Apply { expr: ms_expr, body } => {
            let ms = eval(ms_expr, env, input)?;
            match &ms {
                Value::MethodSet { .. } => {
                    let new_env = env.bind(format!("\0ms"), ms);
                    eval(body, &new_env, input)
                }
                _ => Err("apply: expected a method set value".to_string()),
            }
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
            // Check method sets for tagged values and primitive types
            let type_id = match &recv {
                Value::Tagged { id, .. } => Some(*id),
                other => builtin_tag_id(other),
            };
            if let Some(tid) = type_id {
                if let Some(func) = env.find_method_in_method_sets(tid, method) {
                    let func = func.clone();
                    let recv_combined = prepend_arg(&recv, combined);
                    return apply(&func, recv_combined);
                }
            }
            Err(format!("no method '{}' on {}", method, recv))
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
                    // or against the tag constructor itself
                    if let Value::Tagged { id, payload, .. } = value {
                        if id == ctor_id && matches!(**payload, Value::Unit) {
                            return Ok(Some(env.clone()));
                        }
                    }
                    if let Value::TagConstructor { id, .. } = value {
                        if id == ctor_id {
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
    // For strings, convert to an array of single-character strings for destructuring
    let string_parts: Vec<Value>;
    let elems = match value {
        Value::Array(e) => e,
        Value::Str(s) => {
            string_parts = s.chars().map(|c| Value::Str(c.to_string())).collect();
            &string_parts
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
                    let rest_val = if matches!(value, Value::Str(_)) {
                        let s: String = rest.iter().map(|v| match v {
                            Value::Str(s) => s.as_str(),
                            _ => "",
                        }).collect();
                        Value::Str(s)
                    } else {
                        Value::Array(rest)
                    };
                    new_env = new_env.bind(name.clone(), rest_val);
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

/// Extract the receiver (field "0") as an array, and the remaining arg.
/// When called via method set dispatch, the receiver is prepended as field "0".
fn extract_receiver_array(arg: &Value, name: &str) -> Result<(Vec<Value>, Value), String> {
    match arg {
        Value::Array(elems) => Ok((elems.clone(), Value::Unit)),
        Value::Struct(fields) => {
            let recv = fields.iter().find(|(l, _)| l == "0")
                .map(|(_, v)| v)
                .ok_or_else(|| format!("{}: expected array as first argument", name))?;
            let elems = match recv {
                Value::Array(e) => e.clone(),
                _ => return Err(format!("{}: expected array as first argument", name)),
            };
            // Remaining arg: if there's a field "1" and only 2 positional fields, return it directly
            let rest = extract_rest_arg(fields);
            Ok((elems, rest))
        }
        _ => Err(format!("{}: expected array as first argument", name)),
    }
}

/// Extract the receiver (field "0") as a string, and the remaining arg.
fn extract_receiver_str(arg: &Value, name: &str) -> Result<(String, Value), String> {
    match arg {
        Value::Str(s) => Ok((s.clone(), Value::Unit)),
        Value::Struct(fields) => {
            let recv = fields.iter().find(|(l, _)| l == "0")
                .map(|(_, v)| v)
                .ok_or_else(|| format!("{}: expected string as first argument", name))?;
            let s = match recv {
                Value::Str(s) => s.clone(),
                _ => return Err(format!("{}: expected string as first argument", name)),
            };
            let rest = extract_rest_arg(fields);
            Ok((s, rest))
        }
        _ => Err(format!("{}: expected string as first argument", name)),
    }
}

/// Extract the remaining argument after the receiver (field "0") has been consumed.
/// If there's only field "0", return Unit. If there's exactly one remaining field
/// (positional "1"), return it directly. Otherwise, return a struct of the remaining
/// fields with positional fields re-numbered from "0".
fn extract_rest_arg(fields: &[(String, Value)]) -> Value {
    let rest: Vec<&(String, Value)> = fields.iter()
        .filter(|(l, _)| l != "0")
        .collect();
    if rest.is_empty() {
        return Value::Unit;
    }
    // If there's exactly one positional field "1" and no named fields, return it directly
    if rest.len() == 1 && rest[0].0 == "1" {
        return rest[0].1.clone();
    }
    // Otherwise return a struct with remaining fields, re-numbering positional ones
    let mut rest_fields = Vec::new();
    let mut idx = 0u64;
    for (label, val) in fields {
        if label == "0" {
            continue;
        }
        if label.parse::<u64>().is_ok() {
            rest_fields.push((idx.to_string(), val.clone()));
            idx += 1;
        } else {
            rest_fields.push((label.clone(), val.clone()));
        }
    }
    Value::Struct(rest_fields)
}

/// Extract a range (start, end) from a struct value.
fn extract_range(arg: &Value, name: &str) -> Result<(i64, i64), String> {
    match arg {
        Value::Struct(fields) => {
            let s = fields.iter().find(|(l, _)| l == "start")
                .map(|(_, v)| v.clone())
                .ok_or_else(|| format!("{}: expected range with 'start' field", name))?;
            let e = fields.iter().find(|(l, _)| l == "end")
                .map(|(_, v)| v.clone())
                .ok_or_else(|| format!("{}: expected range with 'end' field", name))?;
            match (s, e) {
                (Value::Int(s), Value::Int(e)) => Ok((s, e)),
                _ => Err(format!("{}: start and end must be integers", name)),
            }
        }
        _ => Err(format!("{}: expected a range argument", name)),
    }
}

/// Prepend a receiver to an argument for method set dispatch.
fn prepend_arg(receiver: &Value, arg: Value) -> Value {
    match arg {
        Value::Unit => receiver.clone(),
        Value::Struct(mut fields) => {
            let mut new_fields = vec![("0".to_string(), receiver.clone())];
            for (label, val) in fields.drain(..) {
                if let Ok(n) = label.parse::<u64>() {
                    new_fields.push(((n + 1).to_string(), val));
                } else {
                    new_fields.push((label, val));
                }
            }
            Value::Struct(new_fields)
        }
        single => Value::Struct(vec![
            ("0".to_string(), receiver.clone()),
            ("1".to_string(), single),
        ]),
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

fn is_function(v: &Value) -> bool {
    matches!(v, Value::Closure { .. } | Value::BranchClosure { .. } | Value::BuiltinFn(_))
}

fn eval_compare(op: CmpOp, lhs: &Value, rhs: &Value) -> Result<Value, String> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Bool(compare_partial(op, a, b)?)),
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
        // Tag constructor comparison (by identity)
        (
            Value::TagConstructor { id: id1, .. },
            Value::TagConstructor { id: id2, .. },
        ) => match op {
            CmpOp::Eq => Ok(Value::Bool(id1 == id2)),
            CmpOp::NotEq => Ok(Value::Bool(id1 != id2)),
            _ => Err("cannot order tag constructors".to_string()),
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
        _ => {
            if is_function(lhs) || is_function(rhs) {
                Err("cannot compare functions with ==; use ref_eq()".to_string())
            } else {
                Err(format!("cannot compare values: {} and {}", lhs, rhs))
            }
        }
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
            _ => Err("len: expected array".to_string()),
        },
        "print" => {
            println!("{}", arg.print_string());
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
        "byte" => match arg {
            Value::Int(n) => {
                if n < 0 || n > 255 {
                    Err(format!("byte: value {} out of range (0..255)", n))
                } else {
                    Ok(Value::Byte(n as u8))
                }
            }
            _ => Err("byte: expected int".to_string()),
        },
        "int" => match arg {
            Value::Int(n) => Ok(Value::Int(n)),
            Value::Float(f) => Ok(Value::Int(f as i64)),
            Value::Byte(b) => Ok(Value::Int(b as i64)),
            Value::Char(c) => Ok(Value::Int(c as u32 as i64)),
            Value::Bool(b) => Ok(Value::Int(if b { 1 } else { 0 })),
            _ => Err(format!("int: cannot convert {} to int", arg)),
        },
        "float" => match arg {
            Value::Float(f) => Ok(Value::Float(f)),
            Value::Int(n) => Ok(Value::Float(n as f64)),
            _ => Err(format!("float: cannot convert {} to float", arg)),
        },
        "char" => match arg {
            Value::Int(n) => {
                if n < 0 {
                    return Err(format!("char: negative value {}", n));
                }
                let n = n as u32;
                char::from_u32(n)
                    .map(Value::Char)
                    .ok_or_else(|| format!("char: value {} is not a valid Unicode scalar value", n))
            }
            Value::Byte(b) => Ok(Value::Char(b as char)),
            _ => Err(format!("char: cannot convert {} to char", arg)),
        },
        "ref_eq" => match arg {
            Value::Struct(fields) if fields.len() == 2 => {
                Ok(Value::Bool(fields[0].1 == fields[1].1))
            }
            _ => Err("ref_eq: expected (value, value)".to_string()),
        },
        "val_eq" => match arg {
            Value::Struct(fields) if fields.len() == 2 => {
                Ok(Value::Bool(fields[0].1.val_eq(&fields[1].1)))
            }
            _ => Err("val_eq: expected (value, value)".to_string()),
        },
        // ── Array method builtins (receiver is first arg) ──
        "array_get" => {
            let (elems, rest) = extract_receiver_array(&arg, "array_get")?;
            let idx = match rest {
                Value::Int(i) => i,
                _ => return Err("get: expected integer index".to_string()),
            };
            if idx < 0 {
                return Err(format!("negative array index: {}", idx));
            }
            let idx = idx as usize;
            elems.get(idx).cloned()
                .ok_or_else(|| format!("array index {} out of bounds (len {})", idx, elems.len()))
        }
        "array_slice" => {
            let (elems, rest) = extract_receiver_array(&arg, "array_slice")?;
            let (start, end) = extract_range(&rest, "slice")?;
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
        "array_len" => {
            let (elems, _) = extract_receiver_array(&arg, "array_len")?;
            Ok(Value::Int(elems.len() as i64))
        }
        "array_map" => {
            let (elems, func) = extract_receiver_array(&arg, "array_map")?;
            let result: Result<Vec<Value>, String> =
                elems.iter().map(|v| apply(&func, v.clone())).collect();
            Ok(Value::Array(result?))
        }
        "array_filter" => {
            let (elems, func) = extract_receiver_array(&arg, "array_filter")?;
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
        "array_fold" => {
            let (elems, rest) = extract_receiver_array(&arg, "array_fold")?;
            match rest {
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
        "array_zip" => {
            let (elems, rest) = extract_receiver_array(&arg, "array_zip")?;
            match rest {
                Value::Array(other) => {
                    let result: Vec<Value> = elems.iter()
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
        // ── String method builtins (receiver is first arg) ──
        "string_byte_len" => {
            let (s, _) = extract_receiver_str(&arg, "string_byte_len")?;
            Ok(Value::Int(s.len() as i64))
        }
        "string_char_len" => {
            let (s, _) = extract_receiver_str(&arg, "string_char_len")?;
            Ok(Value::Int(s.chars().count() as i64))
        }
        "string_byte_get" => {
            let (s, rest) = extract_receiver_str(&arg, "string_byte_get")?;
            let idx = match rest {
                Value::Int(i) => i,
                _ => return Err("byte_get: expected integer index".to_string()),
            };
            if idx < 0 {
                return Err(format!("byte_get: negative index: {}", idx));
            }
            let idx = idx as usize;
            s.as_bytes().get(idx).copied().map(Value::Byte)
                .ok_or_else(|| format!("byte_get: index {} out of bounds (byte_len {})", idx, s.len()))
        }
        "string_char_get" => {
            let (s, rest) = extract_receiver_str(&arg, "string_char_get")?;
            let idx = match rest {
                Value::Int(i) => i,
                _ => return Err("char_get: expected integer index".to_string()),
            };
            if idx < 0 {
                return Err(format!("char_get: negative index: {}", idx));
            }
            let idx = idx as usize;
            s.chars().nth(idx).map(Value::Char)
                .ok_or_else(|| format!("char_get: index {} out of bounds (char_len {})", idx, s.chars().count()))
        }
        "string_as_bytes" => {
            let (s, _) = extract_receiver_str(&arg, "string_as_bytes")?;
            Ok(Value::Array(s.bytes().map(Value::Byte).collect()))
        }
        "string_chars" => {
            let (s, _) = extract_receiver_str(&arg, "string_chars")?;
            Ok(Value::Array(s.chars().map(Value::Char).collect()))
        }
        "string_split" => {
            let (s, rest) = extract_receiver_str(&arg, "string_split")?;
            let delimiter = match rest {
                Value::Str(d) => d,
                _ => return Err("split: expected string delimiter".to_string()),
            };
            let parts: Vec<Value> = s.split(&delimiter).map(|p| Value::Str(p.to_string())).collect();
            Ok(Value::Array(parts))
        }
        "string_trim" => {
            let (s, _) = extract_receiver_str(&arg, "string_trim")?;
            Ok(Value::Str(s.trim().to_string()))
        }
        "string_contains" => {
            let (s, rest) = extract_receiver_str(&arg, "string_contains")?;
            let needle = match rest {
                Value::Str(n) => n,
                Value::Char(c) => c.to_string(),
                _ => return Err("contains: expected string or char".to_string()),
            };
            Ok(Value::Bool(s.contains(&needle)))
        }
        "string_slice" => {
            let (s, rest) = extract_receiver_str(&arg, "string_slice")?;
            let (start, end) = extract_range(&rest, "slice")?;
            if start < 0 || end < 0 {
                return Err("slice: negative index".to_string());
            }
            let start = start as usize;
            let end = end as usize;
            if start > s.len() || end > s.len() || start > end {
                return Err(format!("slice: indices {}..{} out of bounds (len {})", start, end, s.len()));
            }
            if !s.is_char_boundary(start) || !s.is_char_boundary(end) {
                return Err("slice: index is not on a UTF-8 character boundary".to_string());
            }
            Ok(Value::Str(s[start..end].to_string()))
        }
        "string_starts_with" => {
            let (s, rest) = extract_receiver_str(&arg, "string_starts_with")?;
            let prefix = match rest {
                Value::Str(p) => p,
                _ => return Err("starts_with: expected string".to_string()),
            };
            Ok(Value::Bool(s.starts_with(&prefix)))
        }
        "string_ends_with" => {
            let (s, rest) = extract_receiver_str(&arg, "string_ends_with")?;
            let suffix = match rest {
                Value::Str(p) => p,
                _ => return Err("ends_with: expected string".to_string()),
            };
            Ok(Value::Bool(s.ends_with(&suffix)))
        }
        "string_replace" => {
            let (s, rest) = extract_receiver_str(&arg, "string_replace")?;
            match rest {
                Value::Struct(fields) if fields.len() == 2 => {
                    let pattern = match &fields[0].1 {
                        Value::Str(p) => p.clone(),
                        _ => return Err("replace: first argument must be a string".to_string()),
                    };
                    let replacement = match &fields[1].1 {
                        Value::Str(r) => r.clone(),
                        _ => return Err("replace: second argument must be a string".to_string()),
                    };
                    Ok(Value::Str(s.replace(&pattern, &replacement)))
                }
                _ => Err("replace: expected (pattern, replacement)".to_string()),
            }
        }
        "method_set" => match arg {
            Value::Struct(fields) if fields.len() == 2 => {
                let tag_id = match &fields[0].1 {
                    Value::TagConstructor { id, .. } => *id,
                    _ => return Err("method_set: first argument must be a type constructor".to_string()),
                };
                let methods = match &fields[1].1 {
                    Value::Struct(method_fields) => {
                        for (label, _) in method_fields {
                            if label.parse::<u64>().is_ok() {
                                return Err("method_set: second argument must be a struct with named fields".to_string());
                            }
                        }
                        method_fields.clone()
                    }
                    _ => return Err("method_set: second argument must be a struct of functions".to_string()),
                };
                Ok(Value::MethodSet {
                    id: next_closure_id(),
                    tag_id,
                    methods,
                })
            }
            _ => Err("method_set: expected (constructor, struct_of_functions)".to_string()),
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
        ExprKind::Apply { expr: ms_expr, body } => {
            let ms = eval(ms_expr, env, input)?;
            match &ms {
                Value::MethodSet { .. } => {
                    let new_env = env.bind(format!("\0ms"), ms);
                    eval_collecting(body, &new_env, input)
                }
                _ => Err("apply: expected a method set value".to_string()),
            }
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
        ExprKind::Apply { expr: ms_expr, body } => {
            let ms = eval(ms_expr, env, input)?;
            match &ms {
                Value::MethodSet { .. } => {
                    let new_env = env.bind(format!("\0ms"), ms);
                    eval_collecting(body, &new_env, input)
                }
                _ => Err("apply: expected a method set value".to_string()),
            }
        }
        _ => {
            let val = eval_pipe(lhs_val, rhs, env, input)?;
            Ok((val, env.clone()))
        }
    }
}

fn has_toplevel_let(expr: &Expr) -> bool {
    match expr.as_ref() {
        ExprKind::Let { .. } | ExprKind::LetArray { .. } | ExprKind::Apply { .. } => true,
        _ => false,
    }
}

/// Build the core module as a Value::Struct.
pub fn build_core_module() -> Value {
    let mut fields = Vec::new();

    // Type constructors for primitive types (using reserved TagIds)
    let type_constructors = [
        ("Int", TAG_ID_INT),
        ("Float", TAG_ID_FLOAT),
        ("Bool", TAG_ID_BOOL),
        ("String", TAG_ID_STRING),
        ("Char", TAG_ID_CHAR),
        ("Byte", TAG_ID_BYTE),
        ("Array", TAG_ID_ARRAY),
        ("Unit", TAG_ID_UNIT),
    ];
    for (name, id) in &type_constructors {
        fields.push((name.to_string(), Value::TagConstructor {
            id: *id,
            name: name.to_string(),
        }));
    }

    // All builtin functions
    let builtins = [
        "not", "and", "or", "len", "print", "map", "filter", "fold", "zip",
        "byte", "int", "float", "char", "ref_eq", "val_eq", "method_set",
        // Array method builtins (receiver as first arg)
        "array_get", "array_slice", "array_len", "array_map", "array_filter",
        "array_fold", "array_zip",
        // String method builtins (receiver as first arg)
        "string_byte_len", "string_char_len", "string_byte_get", "string_char_get",
        "string_as_bytes", "string_chars", "string_split", "string_trim",
        "string_contains", "string_slice", "string_starts_with", "string_ends_with",
        "string_replace",
    ];
    for name in &builtins {
        fields.push((name.to_string(), Value::BuiltinFn(name.to_string())));
    }

    Value::Struct(fields)
}

/// Create the default environment (no builtins pre-bound).
/// Programs should `use(std)` to access builtins.
pub fn default_env() -> Env {
    Env::new()
}

/// Create the default environment with provided modules (no builtins pre-bound).
/// Programs should `use(std)` to access builtins.
pub fn default_env_with_modules(modules: std::collections::HashMap<String, Value>) -> Env {
    Env::with_modules(modules)
}
