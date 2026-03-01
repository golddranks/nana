use std::sync::atomic::{AtomicU64, Ordering};

use crate::ast::{CmpOp, Pattern, PatField, ArrayPat};
use crate::mir::*;
use crate::value::*;

static CLOSURE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_closure_id() -> u64 {
    CLOSURE_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub fn eval(expr: &Mir, env: &Env, input: &Value) -> Result<Value, String> {
    match expr.as_ref() {
        // ── Literals ──
        MirKind::Int(n) => Ok(Value::I64(*n)),
        MirKind::Float(f) => Ok(Value::F64(*f)),
        MirKind::Bool(b) => Ok(Value::Bool(*b)),
        MirKind::Str(s) => Ok(Value::Str(s.clone())),
        MirKind::Char(c) => Ok(Value::Char(*c)),
        MirKind::Byte(b) => Ok(Value::U8(*b)),
        MirKind::Unit => Ok(Value::Unit),

        // ── Variable reference ──
        MirKind::Ident(name) if name == "in" => Ok(input.clone()),
        MirKind::Ident(name) => env
            .get(name)
            .cloned()
            .ok_or_else(|| format!("undefined variable: {}", name)),

        // ── Block (lambda) ──
        MirKind::Block(body) => Ok(Value::Closure {
            id: next_closure_id(),
            body: body.clone(),
            env: env.clone(),
        }),

        // ── Branching block (pattern matching lambda) ──
        MirKind::BranchBlock(arms) => Ok(Value::BranchClosure {
            id: next_closure_id(),
            arms: arms.clone(),
            env: env.clone(),
        }),

        // ── Array ──
        MirKind::Array(elems) => {
            let values: Result<Vec<Value>, String> =
                elems.iter().map(|e| eval(e, env, input)).collect();
            Ok(Value::Array(values?))
        }

        // ── Struct ──
        MirKind::Struct(fields) => {
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
        MirKind::FieldAccess(expr, field) => {
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
        MirKind::MethodCall { receiver, method, arg } => {
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
                    // Auto-coerce int to byte when calling byte methods
                    let arg_val = coerce_literal_if_needed(&recv, arg_val);
                    let combined = prepend_arg(&recv, arg_val);
                    return apply(&func, combined);
                }
            }
            // Fallback: built-in comparison for types without method sets
            let arg_val = eval(arg, env, input)?;
            if let Some(cmp_op) = method_to_cmp_op(method) {
                return eval_compare(cmp_op, &recv, &arg_val);
            }
            Err(format!("no method '{}' on {}", method, recv))
        }

        // ── Function call ──
        MirKind::Call(func_expr, arg_expr) => {
            let func = eval(func_expr, env, input)?;
            let arg = eval(arg_expr, env, input)?;
            apply(&func, arg)
        }

        // ── Bind ──
        MirKind::Bind { name, value, body } => {
            let val = eval(value, env, input)?;
            let new_env = env.bind(name.clone(), val);
            eval(body, &new_env, input)
        }

        // ── Let binding (pattern destructuring) ──
        MirKind::Let { pattern, body } => {
            let new_env = bind_pattern(pattern, input, env)?;
            let new_env = new_env.bind("\0".to_string(), input.clone());
            eval(body, &new_env, input)
        }

        // ── Let array destructuring ──
        MirKind::LetArray { patterns, body } => {
            let new_env = bind_array_pattern(patterns, input, env)?;
            let new_env = new_env.bind("\0".to_string(), input.clone());
            eval(body, &new_env, input)
        }

        // ── Pipe (for let/letarray/apply targets) ──
        MirKind::Pipe(lhs, rhs) => {
            let lhs_val = eval(lhs, env, input)?;
            eval_pipe(&lhs_val, rhs, env, input)
        }

        // ── NewTag ──
        MirKind::NewTag(id, name) => Ok(Value::TagConstructor {
            id: *id,
            name: name
                .clone()
                .unwrap_or_else(|| format!("tag_{}", id)),
        }),

        // ── Import ──
        MirKind::Import(name) => env
            .get_module(name)
            .cloned()
            .ok_or_else(|| format!("module not provided: {}", name)),

        // ── Apply (method set scope) ──
        MirKind::Apply { expr, body } => {
            let ms = eval(expr, env, input)?;
            match &ms {
                Value::MethodSet { .. } => {
                    let new_env = env.bind(format!("\0ms"), ms);
                    eval(body, &new_env, input)
                }
                _ => Err("apply: expected a method set value".to_string()),
            }
        }
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

/// If a module value has a "prelude" field that is a struct of method sets,
/// bind all those method sets into the environment (auto-apply).
fn apply_prelude(module: &Value, env: &Env) -> Env {
    let mut result = env.clone();
    if let Value::Struct(fields) = module {
        for (label, val) in fields {
            if label == "prelude" {
                if let Value::Struct(prelude_fields) = val {
                    for (_, ms_val) in prelude_fields {
                        if matches!(ms_val, Value::MethodSet { .. }) {
                            result = result.bind(format!("\0ms"), ms_val.clone());
                        }
                    }
                }
            }
        }
    }
    result
}

/// Evaluate pipe for let/letarray/apply targets that need special input handling.
fn eval_pipe(lhs_val: &Value, rhs: &Mir, env: &Env, input: &Value) -> Result<Value, String> {
    match rhs.as_ref() {
        MirKind::Let { pattern, body } => {
            let new_env = bind_pattern(pattern, lhs_val, env)?;
            let new_env = apply_prelude(lhs_val, &new_env);
            let new_env = new_env.bind("\0".to_string(), lhs_val.clone());
            eval(body, &new_env, input)
        }
        MirKind::LetArray { patterns, body } => {
            let new_env = bind_array_pattern(patterns, lhs_val, env)?;
            let new_env = new_env.bind("\0".to_string(), lhs_val.clone());
            eval(body, &new_env, input)
        }
        MirKind::Apply { expr: ms_expr, body } => {
            let ms = eval(ms_expr, env, input)?;
            match &ms {
                Value::MethodSet { .. } => {
                    let new_env = env.bind(format!("\0ms"), ms);
                    eval(body, &new_env, input)
                }
                _ => Err("apply: expected a method set value".to_string()),
            }
        }
        // Fallback — shouldn't normally be reached since lower() only
        // creates Pipe for let/letarray/apply targets
        _ => {
            let rhs_val = eval(rhs, env, input)?;
            apply(&rhs_val, lhs_val.clone())
        }
    }
}

/// Evaluate branching: match scrutinee against branch arms.
fn eval_branch(scrutinee: &Value, arms: &[MirBranchArm], env: &Env, input: &Value) -> Result<Value, String> {
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
fn match_branch_pattern(pattern: &MirBranchPattern, value: &Value, env: &Env) -> Result<Option<Env>, String> {
    match pattern {
        MirBranchPattern::Discard => Ok(Some(env.clone())),
        MirBranchPattern::Binding(name) => {
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
        MirBranchPattern::Tag(tag_name, binding) => {
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
                            crate::ast::BranchBinding::Name(name) => {
                                arm_env = arm_env.bind(name.clone(), payload.as_ref().clone());
                            }
                            crate::ast::BranchBinding::Discard => {}
                        }
                    }
                    return Ok(Some(arm_env));
                }
            }
            Ok(None)
        }
        MirBranchPattern::Literal(lit_expr) => {
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

            let unlabeled_fields: Vec<&PatField> = fields.iter()
                .filter(|f| !f.is_rest && f.label.is_none() && f.binding != "_")
                .collect();

            let has_explicit_labels = fields.iter().any(|f| f.label.is_some());

            let bind_by_name = if has_explicit_labels {
                true
            } else if unlabeled_fields.is_empty() {
                struct_fields.iter().any(|(l, _)| l.parse::<u64>().is_err())
            } else {
                let all_match = unlabeled_fields.iter().all(|pf| {
                    struct_fields.iter().any(|(l, _)| l == &pf.binding)
                });
                if all_match {
                    true
                } else {
                    let has_positional = struct_fields.iter().any(|(l, _)| l.parse::<u64>().is_ok());
                    if has_positional {
                        let any_match = unlabeled_fields.iter().any(|pf| {
                            struct_fields.iter().any(|(l, _)| l == &pf.binding && l.parse::<u64>().is_err())
                        });
                        if any_match {
                            return Err("partial name match in destructuring: either all names must match struct fields or none".to_string());
                        }
                        false
                    } else {
                        // !all_match guarantees at least one field doesn't match
                        let missing = unlabeled_fields.iter()
                            .find(|pf| !struct_fields.iter().any(|(l, _)| l == &pf.binding))
                            .expect("!all_match guarantees a missing field");
                        return Err(format!("field '{}' not found in struct", missing.binding));
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
                        if let Some((i, _)) = struct_fields.iter().enumerate().find(|(i, _)| !used_indices.contains(i)) {
                            used_indices.push(i);
                        }
                    } else {
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

/// Extract the receiver (field "0") as an int, and the remaining arg.
fn extract_receiver_i64(arg: &Value, name: &str) -> Result<(i64, Value), String> {
    match arg {
        Value::I64(n) => Ok((*n, Value::Unit)),
        Value::Struct(fields) => {
            let recv = fields.iter().find(|(l, _)| l == "0")
                .map(|(_, v)| v)
                .ok_or_else(|| format!("{}: expected int as first argument", name))?;
            let n = match recv {
                Value::I64(n) => *n,
                _ => return Err(format!("{}: expected int as first argument", name)),
            };
            let rest = extract_rest_arg(fields);
            Ok((n, rest))
        }
        _ => Err(format!("{}: expected int as first argument", name)),
    }
}
fn extract_receiver_i32(arg: &Value, name: &str) -> Result<(i32, Value), String> {
    match arg {
        Value::I32(n) => Ok((*n, Value::Unit)),
        Value::Struct(fields) if fields.len() >= 1 => {
            match &fields[0].1 {
                Value::I32(n) => {
                    let rest = if fields.len() == 2 {
                        fields[1].1.clone()
                    } else {
                        Value::Struct(fields[1..].to_vec())
                    };
                    Ok((*n, rest))
                }
                _ => Err(format!("{}: expected i32 receiver", name)),
            }
        }
        _ => Err(format!("{}: expected i32 receiver", name)),
    }
}

fn extract_receiver_f32(arg: &Value, name: &str) -> Result<(f32, Value), String> {
    match arg {
        Value::F32(f) => Ok((*f, Value::Unit)),
        Value::Struct(fields) if fields.len() >= 1 => {
            match &fields[0].1 {
                Value::F32(f) => {
                    let rest = if fields.len() == 2 {
                        fields[1].1.clone()
                    } else {
                        Value::Struct(fields[1..].to_vec())
                    };
                    Ok((*f, rest))
                }
                _ => Err(format!("{}: expected f32 receiver", name)),
            }
        }
        _ => Err(format!("{}: expected f32 receiver", name)),
    }
}

fn extract_receiver_i128(arg: &Value, name: &str) -> Result<(i128, Value), String> {
    match arg {
        Value::I128(n) => Ok((*n, Value::Unit)),
        Value::Struct(fields) if !fields.is_empty() => {
            match &fields[0].1 {
                Value::I128(n) => {
                    let rest = if fields.len() == 2 {
                        fields[1].1.clone()
                    } else {
                        Value::Struct(fields[1..].to_vec())
                    };
                    Ok((*n, rest))
                }
                _ => Err(format!("{}: expected i128 receiver", name)),
            }
        }
        _ => Err(format!("{}: expected i128 receiver", name)),
    }
}

fn extract_receiver_u128(arg: &Value, name: &str) -> Result<(u128, Value), String> {
    match arg {
        Value::U128(n) => Ok((*n, Value::Unit)),
        Value::Struct(fields) if !fields.is_empty() => {
            match &fields[0].1 {
                Value::U128(n) => {
                    let rest = if fields.len() == 2 {
                        fields[1].1.clone()
                    } else {
                        Value::Struct(fields[1..].to_vec())
                    };
                    Ok((*n, rest))
                }
                _ => Err(format!("{}: expected u128 receiver", name)),
            }
        }
        _ => Err(format!("{}: expected u128 receiver", name)),
    }
}

fn extract_receiver_i8(arg: &Value, name: &str) -> Result<(i8, Value), String> {
    match arg {
        Value::I8(n) => Ok((*n, Value::Unit)),
        Value::Struct(fields) if !fields.is_empty() => {
            match &fields[0].1 {
                Value::I8(n) => {
                    let rest = if fields.len() == 2 {
                        fields[1].1.clone()
                    } else {
                        Value::Struct(fields[1..].to_vec())
                    };
                    Ok((*n, rest))
                }
                _ => Err(format!("{}: expected i8 receiver", name)),
            }
        }
        _ => Err(format!("{}: expected i8 receiver", name)),
    }
}

fn extract_receiver_u8(arg: &Value, name: &str) -> Result<(u8, Value), String> {
    match arg {
        Value::U8(n) => Ok((*n, Value::Unit)),
        Value::Struct(fields) if !fields.is_empty() => {
            match &fields[0].1 {
                Value::U8(n) => {
                    let rest = if fields.len() == 2 {
                        fields[1].1.clone()
                    } else {
                        Value::Struct(fields[1..].to_vec())
                    };
                    Ok((*n, rest))
                }
                _ => Err(format!("{}: expected u8 receiver", name)),
            }
        }
        _ => Err(format!("{}: expected u8 receiver", name)),
    }
}

fn extract_receiver_i16(arg: &Value, name: &str) -> Result<(i16, Value), String> {
    match arg {
        Value::I16(n) => Ok((*n, Value::Unit)),
        Value::Struct(fields) if !fields.is_empty() => {
            match &fields[0].1 {
                Value::I16(n) => {
                    let rest = if fields.len() == 2 {
                        fields[1].1.clone()
                    } else {
                        Value::Struct(fields[1..].to_vec())
                    };
                    Ok((*n, rest))
                }
                _ => Err(format!("{}: expected i16 receiver", name)),
            }
        }
        _ => Err(format!("{}: expected i16 receiver", name)),
    }
}

fn extract_receiver_u16(arg: &Value, name: &str) -> Result<(u16, Value), String> {
    match arg {
        Value::U16(n) => Ok((*n, Value::Unit)),
        Value::Struct(fields) if !fields.is_empty() => {
            match &fields[0].1 {
                Value::U16(n) => {
                    let rest = if fields.len() == 2 {
                        fields[1].1.clone()
                    } else {
                        Value::Struct(fields[1..].to_vec())
                    };
                    Ok((*n, rest))
                }
                _ => Err(format!("{}: expected u16 receiver", name)),
            }
        }
        _ => Err(format!("{}: expected u16 receiver", name)),
    }
}

fn extract_receiver_u32(arg: &Value, name: &str) -> Result<(u32, Value), String> {
    match arg {
        Value::U32(n) => Ok((*n, Value::Unit)),
        Value::Struct(fields) if !fields.is_empty() => {
            match &fields[0].1 {
                Value::U32(n) => {
                    let rest = if fields.len() == 2 {
                        fields[1].1.clone()
                    } else {
                        Value::Struct(fields[1..].to_vec())
                    };
                    Ok((*n, rest))
                }
                _ => Err(format!("{}: expected u32 receiver", name)),
            }
        }
        _ => Err(format!("{}: expected u32 receiver", name)),
    }
}

fn extract_receiver_u64(arg: &Value, name: &str) -> Result<(u64, Value), String> {
    match arg {
        Value::U64(n) => Ok((*n, Value::Unit)),
        Value::Struct(fields) if !fields.is_empty() => {
            match &fields[0].1 {
                Value::U64(n) => {
                    let rest = if fields.len() == 2 {
                        fields[1].1.clone()
                    } else {
                        Value::Struct(fields[1..].to_vec())
                    };
                    Ok((*n, rest))
                }
                _ => Err(format!("{}: expected u64 receiver", name)),
            }
        }
        _ => Err(format!("{}: expected u64 receiver", name)),
    }
}

fn extract_receiver_f64(arg: &Value, name: &str) -> Result<(f64, Value), String> {
    match arg {
        Value::F64(n) => Ok((*n, Value::Unit)),
        Value::Struct(fields) if !fields.is_empty() => {
            match &fields[0].1 {
                Value::F64(n) => {
                    let rest = if fields.len() == 2 {
                        fields[1].1.clone()
                    } else {
                        Value::Struct(fields[1..].to_vec())
                    };
                    Ok((*n, rest))
                }
                _ => Err(format!("{}: expected f64 receiver", name)),
            }
        }
        _ => Err(format!("{}: expected f64 receiver", name)),
    }
}
/// Extract the receiver (field "0") as a bool, and the remaining arg.
fn extract_receiver_bool(arg: &Value, name: &str) -> Result<(bool, Value), String> {
    match arg {
        Value::Bool(b) => Ok((*b, Value::Unit)),
        Value::Struct(fields) => {
            let recv = fields.iter().find(|(l, _)| l == "0")
                .map(|(_, v)| v)
                .ok_or_else(|| format!("{}: expected bool as first argument", name))?;
            let b = match recv {
                Value::Bool(b) => *b,
                _ => return Err(format!("{}: expected bool as first argument", name)),
            };
            let rest = extract_rest_arg(fields);
            Ok((b, rest))
        }
        _ => Err(format!("{}: expected bool as first argument", name)),
    }
}

/// Extract the receiver (field "0") as a char, and the remaining arg.
fn extract_receiver_char(arg: &Value, name: &str) -> Result<(char, Value), String> {
    match arg {
        Value::Char(c) => Ok((*c, Value::Unit)),
        Value::Struct(fields) => {
            let recv = fields.iter().find(|(l, _)| l == "0")
                .map(|(_, v)| v)
                .ok_or_else(|| format!("{}: expected char as first argument", name))?;
            let c = match recv {
                Value::Char(c) => *c,
                _ => return Err(format!("{}: expected char as first argument", name)),
            };
            let rest = extract_rest_arg(fields);
            Ok((c, rest))
        }
        _ => Err(format!("{}: expected char as first argument", name)),
    }
}
/// Extract the receiver (field "0") as unit, and the remaining arg.
fn extract_receiver_unit(arg: &Value, name: &str) -> Result<Value, String> {
    match arg {
        Value::Unit => Ok(Value::Unit),
        Value::Struct(fields) => {
            let recv = fields.iter().find(|(l, _)| l == "0")
                .map(|(_, v)| v)
                .ok_or_else(|| format!("{}: expected unit as first argument", name))?;
            match recv {
                Value::Unit => {}
                _ => return Err(format!("{}: expected unit as first argument", name)),
            }
            let rest = extract_rest_arg(fields);
            Ok(rest)
        }
        _ => Err(format!("{}: expected unit as first argument", name)),
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
                (Value::I64(s), Value::I64(e)) => Ok((s, e)),
                _ => Err(format!("{}: start and end must be integers", name)),
            }
        }
        _ => Err(format!("{}: expected a range argument", name)),
    }
}

/// Auto-coerce numeric literals to the required type when calling methods.
/// This implements the spec's "numeric literals convert automatically to the required type".
fn coerce_literal_if_needed(recv: &Value, arg: Value) -> Value {
    match recv {
        Value::U8(_) => match &arg {
            Value::I64(n) if *n >= 0 && *n <= 255 => Value::U8(*n as u8),
            _ => arg,
        },
        Value::I32(_) => match &arg {
            Value::I64(n) if *n >= i32::MIN as i64 && *n <= i32::MAX as i64 => Value::I32(*n as i32),
            _ => arg,
        },
        Value::F32(_) => match &arg {
            Value::F64(f) => Value::F32(*f as f32),
            _ => arg,
        },
        Value::I128(_) => match &arg {
            Value::I64(n) => Value::I128(*n as i128),
            _ => arg,
        },
        Value::U128(_) => match &arg {
            Value::I64(n) if *n >= 0 => Value::U128(*n as u128),
            _ => arg,
        },
        Value::I8(_) => match &arg {
            Value::I64(n) if *n >= i8::MIN as i64 && *n <= i8::MAX as i64 => Value::I8(*n as i8),
            _ => arg,
        },
        Value::I16(_) => match &arg {
            Value::I64(n) if *n >= i16::MIN as i64 && *n <= i16::MAX as i64 => Value::I16(*n as i16),
            _ => arg,
        },
        Value::U16(_) => match &arg {
            Value::I64(n) if *n >= 0 && *n <= u16::MAX as i64 => Value::U16(*n as u16),
            _ => arg,
        },
        Value::U32(_) => match &arg {
            Value::I64(n) if *n >= 0 && *n <= u32::MAX as i64 => Value::U32(*n as u32),
            _ => arg,
        },
        Value::U64(_) => match &arg {
            Value::I64(n) if *n >= 0 => Value::U64(*n as u64),
            _ => arg,
        },
        Value::F64(_) => match &arg {
            Value::F64(f) => Value::F64(*f),
            _ => arg,
        },
        _ => arg,
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

fn method_to_cmp_op(method: &str) -> Option<CmpOp> {
    match method {
        "eq" => Some(CmpOp::Eq),
        "not_eq" => Some(CmpOp::NotEq),
        "lt" => Some(CmpOp::Lt),
        "gt" => Some(CmpOp::Gt),
        "lt_eq" => Some(CmpOp::LtEq),
        "gt_eq" => Some(CmpOp::GtEq),
        _ => None,
    }
}

fn is_function(v: &Value) -> bool {
    matches!(v, Value::Closure { .. } | Value::BranchClosure { .. } | Value::BuiltinFn(_))
}

fn eval_compare(op: CmpOp, lhs: &Value, rhs: &Value) -> Result<Value, String> {
    match (lhs, rhs) {
        (Value::I64(a), Value::I64(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::F64(a), Value::F64(b)) => Ok(Value::Bool(compare_partial(op, a, b)?)),
        (Value::I32(a), Value::I32(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::F32(a), Value::F32(b)) => Ok(Value::Bool(compare_partial(op, a, b)?)),
        (Value::I128(a), Value::I128(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::U128(a), Value::U128(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::I8(a), Value::I8(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::U8(a), Value::U8(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::I16(a), Value::I16(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::U16(a), Value::U16(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::U32(a), Value::U32(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::U64(a), Value::U64(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::Str(a), Value::Str(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
        (Value::Char(a), Value::Char(b)) => Ok(Value::Bool(compare_ord(op, a, b))),
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
        "print" => {
            println!("{}", arg.print_string());
            Ok(Value::Unit)
        }
        // Type hints — identity functions that assert the type.
        // At runtime, IntLiteral values arrive as Value::Int, so we coerce.
        "byte" => match arg {
            Value::U8(b) => Ok(Value::U8(b)),
            Value::I64(n) => {
                if n < 0 || n > 255 {
                    Err(format!("byte: value {} out of range (0..255)", n))
                } else {
                    Ok(Value::U8(n as u8))
                }
            }
            _ => Err(format!("byte: expected byte, got {}", arg)),
        },
        "int" | "i64" => match arg {
            Value::I64(n) => Ok(Value::I64(n)),
            _ => Err(format!("int: expected int, got {}", arg)),
        },
        "float" => match arg {
            Value::F64(f) => Ok(Value::F64(f)),
            _ => Err(format!("float: expected float, got {}", arg)),
        },
        "char" => match arg {
            Value::Char(c) => Ok(Value::Char(c)),
            Value::I64(n) => {
                if n < 0 {
                    return Err(format!("char: negative value {}", n));
                }
                let n = n as u32;
                char::from_u32(n)
                    .map(Value::Char)
                    .ok_or_else(|| format!("char: value {} is not a valid Unicode scalar value", n))
            }
            _ => Err(format!("char: expected char, got {}", arg)),
        },
        "i32" => match arg {
            Value::I32(n) => Ok(Value::I32(n)),
            Value::I64(n) => {
                if n < i32::MIN as i64 || n > i32::MAX as i64 {
                    Err(format!("i32: value {} out of range", n))
                } else {
                    Ok(Value::I32(n as i32))
                }
            }
            _ => Err(format!("i32: expected i32, got {}", arg)),
        },
        "f32" => match arg {
            Value::F32(f) => Ok(Value::F32(f)),
            Value::F64(f) => Ok(Value::F32(f as f32)),
            _ => Err(format!("f32: expected f32, got {}", arg)),
        },
        "i128" => match arg {
            Value::I128(n) => Ok(Value::I128(n)),
            Value::I64(n) => Ok(Value::I128(n as i128)),
            _ => Err(format!("i128: expected i128, got {}", arg)),
        },
        "u128" => match arg {
            Value::U128(n) => Ok(Value::U128(n)),
            Value::I64(n) => {
                if n < 0 {
                    Err(format!("u128: value {} out of range", n))
                } else {
                    Ok(Value::U128(n as u128))
                }
            }
            _ => Err(format!("u128: expected u128, got {}", arg)),
        },
        "i8" => match arg {
            Value::I8(n) => Ok(Value::I8(n)),
            Value::I64(n) => {
                if n < i8::MIN as i64 || n > i8::MAX as i64 {
                    Err(format!("i8: value {} out of range", n))
                } else {
                    Ok(Value::I8(n as i8))
                }
            }
            _ => Err(format!("i8: expected i8, got {}", arg)),
        },
        "u8" => match arg {
            Value::U8(n) => Ok(Value::U8(n)),
            Value::I64(n) => {
                if n < 0 || n > u8::MAX as i64 {
                    Err(format!("u8: value {} out of range", n))
                } else {
                    Ok(Value::U8(n as u8))
                }
            }
            _ => Err(format!("u8: expected u8, got {}", arg)),
        },
        "i16" => match arg {
            Value::I16(n) => Ok(Value::I16(n)),
            Value::I64(n) => {
                if n < i16::MIN as i64 || n > i16::MAX as i64 {
                    Err(format!("i16: value {} out of range", n))
                } else {
                    Ok(Value::I16(n as i16))
                }
            }
            _ => Err(format!("i16: expected i16, got {}", arg)),
        },
        "u16" => match arg {
            Value::U16(n) => Ok(Value::U16(n)),
            Value::I64(n) => {
                if n < 0 || n > u16::MAX as i64 {
                    Err(format!("u16: value {} out of range", n))
                } else {
                    Ok(Value::U16(n as u16))
                }
            }
            _ => Err(format!("u16: expected u16, got {}", arg)),
        },
        "u32" => match arg {
            Value::U32(n) => Ok(Value::U32(n)),
            Value::I64(n) => {
                if n < 0 || n > u32::MAX as i64 {
                    Err(format!("u32: value {} out of range", n))
                } else {
                    Ok(Value::U32(n as u32))
                }
            }
            _ => Err(format!("u32: expected u32, got {}", arg)),
        },
        "u64" => match arg {
            Value::U64(n) => Ok(Value::U64(n)),
            Value::I64(n) => {
                if n < 0 {
                    Err(format!("u64: value {} out of range", n))
                } else {
                    Ok(Value::U64(n as u64))
                }
            }
            _ => Err(format!("u64: expected u64, got {}", arg)),
        },
        "f64" => match arg {
            Value::F64(f) => Ok(Value::F64(f)),
            _ => Err(format!("f64: expected f64, got {}", arg)),
        },
        // ── Conversion methods (receiver is first arg) ──
        "int_to_char" => match arg {
            Value::I64(n) => {
                if n < 0 {
                    return Err(format!("to_char: negative value {}", n));
                }
                let n = n as u32;
                char::from_u32(n)
                    .map(Value::Char)
                    .ok_or_else(|| format!("to_char: value {} is not a valid Unicode scalar value", n))
            }
            _ => Err(format!("to_char: expected int, got {}", arg)),
        },
        "float_ceil" => match arg {
            Value::F64(f) => Ok(Value::I64(f.ceil() as i64)),
            _ => Err(format!("ceil: expected float, got {}", arg)),
        },
        "float_floor" => match arg {
            Value::F64(f) => Ok(Value::I64(f.floor() as i64)),
            _ => Err(format!("floor: expected float, got {}", arg)),
        },
        "float_round" => match arg {
            Value::F64(f) => Ok(Value::I64(f.round() as i64)),
            _ => Err(format!("round: expected float, got {}", arg)),
        },
        "float_trunc" => match arg {
            Value::F64(f) => Ok(Value::I64(f.trunc() as i64)),
            _ => Err(format!("trunc: expected float, got {}", arg)),
        },
        "float_to_i64" => match arg {
            Value::F64(f) => Ok(Value::I64(f as i64)),
            _ => Err(format!("to_int: expected float, got {}", arg)),
        },
        "char_to_i64" => match arg {
            Value::Char(c) => Ok(Value::I64(c as u32 as i64)),
            _ => Err(format!("to_int: expected char, got {}", arg)),
        },
        "byte_to_i64" => match arg {
            Value::U8(b) => Ok(Value::I64(b as i64)),
            _ => Err(format!("to_int: expected byte, got {}", arg)),
        },
        "int_to_i32" => match arg {
            Value::I64(n) => {
                if n < i32::MIN as i64 || n > i32::MAX as i64 {
                    Err(format!("to_i32: value {} out of range", n))
                } else {
                    Ok(Value::I32(n as i32))
                }
            }
            _ => Err(format!("to_i32: expected int, got {}", arg)),
        },
        "int_to_f32" => match arg {
            Value::I64(n) => Ok(Value::F32(n as f32)),
            _ => Err(format!("to_f32: expected int, got {}", arg)),
        },
        "float_to_f32" => match arg {
            Value::F64(f) => Ok(Value::F32(f as f32)),
            _ => Err(format!("to_f32: expected float, got {}", arg)),
        },
        "u8_to_i32" => match arg {
            Value::U8(b) => Ok(Value::I32(b as i32)),
            _ => Err(format!("to_i32: expected u8, got {}", arg)),
        },
        "int_to_i128" => match arg {
            Value::I64(n) => Ok(Value::I128(n as i128)),
            _ => Err(format!("to_i128: expected int, got {}", arg)),
        },
        "int_to_u128" => match arg {
            Value::I64(n) => {
                if n < 0 {
                    Err(format!("to_u128: value {} out of range", n))
                } else {
                    Ok(Value::U128(n as u128))
                }
            }
            _ => Err(format!("to_u128: expected int, got {}", arg)),
        },
        "u8_to_i128" => match arg {
            Value::U8(b) => Ok(Value::I128(b as i128)),
            _ => Err(format!("to_i128: expected u8, got {}", arg)),
        },
        "u8_to_u128" => match arg {
            Value::U8(b) => Ok(Value::U128(b as u128)),
            _ => Err(format!("to_u128: expected u8, got {}", arg)),
        },
        "i32_to_i128" => match arg {
            Value::I32(n) => Ok(Value::I128(n as i128)),
            _ => Err(format!("to_i128: expected i32, got {}", arg)),
        },
        "i32_to_u128" => match arg {
            Value::I32(n) => {
                if n < 0 {
                    Err(format!("to_u128: value {} out of range", n))
                } else {
                    Ok(Value::U128(n as u128))
                }
            }
            _ => Err(format!("to_u128: expected i32, got {}", arg)),
        },
        "int_to_i8" => match arg {
            Value::I64(n) => {
                if n < i8::MIN as i64 || n > i8::MAX as i64 { Err(format!("to_i8: value {} out of range", n)) }
                else { Ok(Value::I8(n as i8)) }
            }
            _ => Err(format!("to_i8: expected int, got {}", arg)),
        },
        "int_to_u8" => match arg {
            Value::I64(n) => {
                if n < 0 || n > u8::MAX as i64 { Err(format!("to_u8: value {} out of range", n)) }
                else { Ok(Value::U8(n as u8)) }
            }
            _ => Err(format!("to_u8: expected int, got {}", arg)),
        },
        "int_to_i16" => match arg {
            Value::I64(n) => {
                if n < i16::MIN as i64 || n > i16::MAX as i64 { Err(format!("to_i16: value {} out of range", n)) }
                else { Ok(Value::I16(n as i16)) }
            }
            _ => Err(format!("to_i16: expected int, got {}", arg)),
        },
        "int_to_u16" => match arg {
            Value::I64(n) => {
                if n < 0 || n > u16::MAX as i64 { Err(format!("to_u16: value {} out of range", n)) }
                else { Ok(Value::U16(n as u16)) }
            }
            _ => Err(format!("to_u16: expected int, got {}", arg)),
        },
        "int_to_u32" => match arg {
            Value::I64(n) => {
                if n < 0 || n > u32::MAX as i64 { Err(format!("to_u32: value {} out of range", n)) }
                else { Ok(Value::U32(n as u32)) }
            }
            _ => Err(format!("to_u32: expected int, got {}", arg)),
        },
        "int_to_u64" => match arg {
            Value::I64(n) => {
                if n < 0 { Err(format!("to_u64: value {} out of range", n)) }
                else { Ok(Value::U64(n as u64)) }
            }
            _ => Err(format!("to_u64: expected int, got {}", arg)),
        },
        "int_to_f64" => match arg {
            Value::I64(n) => Ok(Value::F64(n as f64)),
            _ => Err(format!("to_f64: expected int, got {}", arg)),
        },
        "float_to_f64" => match arg {
            Value::F64(f) => Ok(Value::F64(f)),
            _ => Err(format!("to_f64: expected float, got {}", arg)),
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
                Value::I64(i) => i,
                _ => return Err("array_get: expected integer index".to_string()),
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
            Ok(Value::I64(elems.len() as i64))
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
                    _ => return Err("array_filter: predicate must return bool".to_string()),
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
            Ok(Value::I64(s.len() as i64))
        }
        "string_char_len" => {
            let (s, _) = extract_receiver_str(&arg, "string_char_len")?;
            Ok(Value::I64(s.chars().count() as i64))
        }
        "string_byte_get" => {
            let (s, rest) = extract_receiver_str(&arg, "string_byte_get")?;
            let idx = match rest {
                Value::I64(i) => i,
                _ => return Err("byte_get: expected integer index".to_string()),
            };
            if idx < 0 {
                return Err(format!("byte_get: negative index: {}", idx));
            }
            let idx = idx as usize;
            s.as_bytes().get(idx).copied().map(Value::U8)
                .ok_or_else(|| format!("byte_get: index {} out of bounds (byte_len {})", idx, s.len()))
        }
        "string_char_get" => {
            let (s, rest) = extract_receiver_str(&arg, "string_char_get")?;
            let idx = match rest {
                Value::I64(i) => i,
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
            Ok(Value::Array(s.bytes().map(Value::U8).collect()))
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
            match rest {
                Value::Str(n) => Ok(Value::Bool(s.contains(n.as_str()))),
                _ => Err("string_contains: expected string".to_string()),
            }
        }
        "string_contains_char" => {
            let (s, rest) = extract_receiver_str(&arg, "string_contains_char")?;
            match rest {
                Value::Char(c) => Ok(Value::Bool(s.contains(c))),
                _ => Err("string_contains_char: expected char".to_string()),
            }
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
                return Err(format!("slice: indices {}..{} out of bounds (byte_len {})", start, end, s.len()));
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
        // ── Int operator builtins ──
        "int_add" => {
            let (a, rest) = extract_receiver_i64(&arg, "int_add")?;
            match rest {
                Value::I64(b) => a.checked_add(b).map(Value::I64)
                    .ok_or_else(|| "integer overflow in addition".to_string()),
                _ => Err("add: expected int argument".to_string()),
            }
        }
        "int_subtract" => {
            let (a, rest) = extract_receiver_i64(&arg, "int_subtract")?;
            match rest {
                Value::I64(b) => a.checked_sub(b).map(Value::I64)
                    .ok_or_else(|| "integer overflow in subtraction".to_string()),
                _ => Err("subtract: expected int argument".to_string()),
            }
        }
        "int_times" => {
            let (a, rest) = extract_receiver_i64(&arg, "int_times")?;
            match rest {
                Value::I64(b) => a.checked_mul(b).map(Value::I64)
                    .ok_or_else(|| "integer overflow in multiplication".to_string()),
                _ => Err("times: expected int argument".to_string()),
            }
        }
        "int_divided_by" => {
            let (a, rest) = extract_receiver_i64(&arg, "int_divided_by")?;
            match rest {
                Value::I64(0) => Err("division by zero".to_string()),
                Value::I64(b) => a.checked_div(b).map(Value::I64)
                    .ok_or_else(|| "integer overflow in division".to_string()),
                _ => Err("divided_by: expected int argument".to_string()),
            }
        }
        "int_negate" => {
            let (a, _) = extract_receiver_i64(&arg, "int_negate")?;
            a.checked_neg().map(Value::I64)
                .ok_or_else(|| "integer overflow in negation".to_string())
        }
        "int_eq" => {
            let (a, rest) = extract_receiver_i64(&arg, "int_eq")?;
            match rest { Value::I64(b) => Ok(Value::Bool(a == b)), _ => Err("eq: expected int".to_string()) }
        }
        "int_not_eq" => {
            let (a, rest) = extract_receiver_i64(&arg, "int_not_eq")?;
            match rest { Value::I64(b) => Ok(Value::Bool(a != b)), _ => Err("not_eq: expected int".to_string()) }
        }
        "int_lt" => {
            let (a, rest) = extract_receiver_i64(&arg, "int_lt")?;
            match rest { Value::I64(b) => Ok(Value::Bool(a < b)), _ => Err("lt: expected int".to_string()) }
        }
        "int_gt" => {
            let (a, rest) = extract_receiver_i64(&arg, "int_gt")?;
            match rest { Value::I64(b) => Ok(Value::Bool(a > b)), _ => Err("gt: expected int".to_string()) }
        }
        "int_lt_eq" => {
            let (a, rest) = extract_receiver_i64(&arg, "int_lt_eq")?;
            match rest { Value::I64(b) => Ok(Value::Bool(a <= b)), _ => Err("lt_eq: expected int".to_string()) }
        }
        "int_gt_eq" => {
            let (a, rest) = extract_receiver_i64(&arg, "int_gt_eq")?;
            match rest { Value::I64(b) => Ok(Value::Bool(a >= b)), _ => Err("gt_eq: expected int".to_string()) }
        }
        "int_to_string" => {
            let (a, _) = extract_receiver_i64(&arg, "int_to_string")?;
            Ok(Value::Str(a.to_string()))
        }

        // ── I32 operator builtins ──
        "i32_add" => {
            let (a, rest) = extract_receiver_i32(&arg, "i32_add")?;
            match rest {
                Value::I32(b) => a.checked_add(b).map(Value::I32)
                    .ok_or_else(|| "integer overflow in i32 addition".to_string()),
                _ => Err("add: expected i32 argument".to_string()),
            }
        }
        "i32_subtract" => {
            let (a, rest) = extract_receiver_i32(&arg, "i32_subtract")?;
            match rest {
                Value::I32(b) => a.checked_sub(b).map(Value::I32)
                    .ok_or_else(|| "integer overflow in i32 subtraction".to_string()),
                _ => Err("subtract: expected i32 argument".to_string()),
            }
        }
        "i32_times" => {
            let (a, rest) = extract_receiver_i32(&arg, "i32_times")?;
            match rest {
                Value::I32(b) => a.checked_mul(b).map(Value::I32)
                    .ok_or_else(|| "integer overflow in i32 multiplication".to_string()),
                _ => Err("times: expected i32 argument".to_string()),
            }
        }
        "i32_divided_by" => {
            let (a, rest) = extract_receiver_i32(&arg, "i32_divided_by")?;
            match rest {
                Value::I32(0) => Err("division by zero".to_string()),
                Value::I32(b) => a.checked_div(b).map(Value::I32)
                    .ok_or_else(|| "integer overflow in i32 division".to_string()),
                _ => Err("divided_by: expected i32 argument".to_string()),
            }
        }
        "i32_negate" => {
            let (a, _) = extract_receiver_i32(&arg, "i32_negate")?;
            a.checked_neg().map(Value::I32)
                .ok_or_else(|| "integer overflow in i32 negation".to_string())
        }
        "i32_eq" | "i32_not_eq" | "i32_lt" | "i32_gt" | "i32_lt_eq" | "i32_gt_eq" => {
            let (a, rest) = extract_receiver_i32(&arg, name)?;
            match rest {
                Value::I32(b) => {
                    let result = match name {
                        "i32_eq" => a == b,
                        "i32_not_eq" => a != b,
                        "i32_lt" => a < b,
                        "i32_gt" => a > b,
                        "i32_lt_eq" => a <= b,
                        "i32_gt_eq" => a >= b,
                        _ => unreachable!(),
                    };
                    Ok(Value::Bool(result))
                }
                _ => Err(format!("{}: expected i32 argument", name)),
            }
        }
        "i32_to_string" => {
            let (a, _) = extract_receiver_i32(&arg, "i32_to_string")?;
            Ok(Value::Str(format!("{}i32", a)))
        }
        "i32_to_i64" => {
            let (a, _) = extract_receiver_i32(&arg, "i32_to_i64")?;
            Ok(Value::I64(a as i64))
        }
        "i32_to_f64" => {
            let (a, _) = extract_receiver_i32(&arg, "i32_to_f64")?;
            Ok(Value::F64(a as f64))
        }
        "i32_to_f32" => {
            let (a, _) = extract_receiver_i32(&arg, "i32_to_f32")?;
            Ok(Value::F32(a as f32))
        }
        "i32_to_u8" => {
            let (a, _) = extract_receiver_i32(&arg, "i32_to_u8")?;
            if a < 0 || a > 255 {
                Err(format!("to_byte: value {} out of range (0..255)", a))
            } else {
                Ok(Value::U8(a as u8))
            }
        }

        // ── F32 operator builtins ──
        "f32_add" => {
            let (a, rest) = extract_receiver_f32(&arg, "f32_add")?;
            match rest {
                Value::F32(b) => Ok(Value::F32(a + b)),
                _ => Err("add: expected f32 argument".to_string()),
            }
        }
        "f32_subtract" => {
            let (a, rest) = extract_receiver_f32(&arg, "f32_subtract")?;
            match rest {
                Value::F32(b) => Ok(Value::F32(a - b)),
                _ => Err("subtract: expected f32 argument".to_string()),
            }
        }
        "f32_times" => {
            let (a, rest) = extract_receiver_f32(&arg, "f32_times")?;
            match rest {
                Value::F32(b) => Ok(Value::F32(a * b)),
                _ => Err("times: expected f32 argument".to_string()),
            }
        }
        "f32_divided_by" => {
            let (a, rest) = extract_receiver_f32(&arg, "f32_divided_by")?;
            match rest {
                Value::F32(b) if b == 0.0 => Err("division by zero".to_string()),
                Value::F32(b) => Ok(Value::F32(a / b)),
                _ => Err("divided_by: expected f32 argument".to_string()),
            }
        }
        "f32_negate" => {
            let (a, _) = extract_receiver_f32(&arg, "f32_negate")?;
            Ok(Value::F32(-a))
        }
        "f32_eq" | "f32_not_eq" | "f32_lt" | "f32_gt" | "f32_lt_eq" | "f32_gt_eq" => {
            let (a, rest) = extract_receiver_f32(&arg, name)?;
            match rest {
                Value::F32(b) => {
                    let result = match name {
                        "f32_eq" => a == b,
                        "f32_not_eq" => a != b,
                        "f32_lt" => a < b,
                        "f32_gt" => a > b,
                        "f32_lt_eq" => a <= b,
                        "f32_gt_eq" => a >= b,
                        _ => unreachable!(),
                    };
                    Ok(Value::Bool(result))
                }
                _ => Err(format!("{}: expected f32 argument", name)),
            }
        }
        "f32_to_string" => {
            let (a, _) = extract_receiver_f32(&arg, "f32_to_string")?;
            if a.fract() == 0.0 {
                Ok(Value::Str(format!("{}.0f32", a)))
            } else {
                Ok(Value::Str(format!("{}f32", a)))
            }
        }
        "f32_to_f64" => {
            let (a, _) = extract_receiver_f32(&arg, "f32_to_f64")?;
            Ok(Value::F64(a as f64))
        }
        "f32_to_i64" => {
            let (a, _) = extract_receiver_f32(&arg, "f32_to_i64")?;
            Ok(Value::I64(a as i64))
        }
        "f32_to_i32" => {
            let (a, _) = extract_receiver_f32(&arg, "f32_to_i32")?;
            Ok(Value::I32(a as i32))
        }
        "f32_ceil" => {
            let (a, _) = extract_receiver_f32(&arg, "f32_ceil")?;
            Ok(Value::I32(a.ceil() as i32))
        }
        "f32_floor" => {
            let (a, _) = extract_receiver_f32(&arg, "f32_floor")?;
            Ok(Value::I32(a.floor() as i32))
        }
        "f32_round" => {
            let (a, _) = extract_receiver_f32(&arg, "f32_round")?;
            Ok(Value::I32(a.round() as i32))
        }
        "f32_trunc" => {
            let (a, _) = extract_receiver_f32(&arg, "f32_trunc")?;
            Ok(Value::I32(a.trunc() as i32))
        }

        // ── I128 operator builtins ──
        "i128_add" => {
            let (a, rest) = extract_receiver_i128(&arg, "i128_add")?;
            match rest {
                Value::I128(b) => a.checked_add(b).map(Value::I128)
                    .ok_or_else(|| "integer overflow in i128 addition".to_string()),
                _ => Err("add: expected i128 argument".to_string()),
            }
        }
        "i128_subtract" => {
            let (a, rest) = extract_receiver_i128(&arg, "i128_subtract")?;
            match rest {
                Value::I128(b) => a.checked_sub(b).map(Value::I128)
                    .ok_or_else(|| "integer overflow in i128 subtraction".to_string()),
                _ => Err("subtract: expected i128 argument".to_string()),
            }
        }
        "i128_times" => {
            let (a, rest) = extract_receiver_i128(&arg, "i128_times")?;
            match rest {
                Value::I128(b) => a.checked_mul(b).map(Value::I128)
                    .ok_or_else(|| "integer overflow in i128 multiplication".to_string()),
                _ => Err("times: expected i128 argument".to_string()),
            }
        }
        "i128_divided_by" => {
            let (a, rest) = extract_receiver_i128(&arg, "i128_divided_by")?;
            match rest {
                Value::I128(0) => Err("division by zero".to_string()),
                Value::I128(b) => a.checked_div(b).map(Value::I128)
                    .ok_or_else(|| "integer overflow in i128 division".to_string()),
                _ => Err("divided_by: expected i128 argument".to_string()),
            }
        }
        "i128_negate" => {
            let (a, _) = extract_receiver_i128(&arg, "i128_negate")?;
            a.checked_neg().map(Value::I128)
                .ok_or_else(|| "integer overflow in i128 negation".to_string())
        }
        "i128_eq" | "i128_not_eq" | "i128_lt" | "i128_gt" | "i128_lt_eq" | "i128_gt_eq" => {
            let (a, rest) = extract_receiver_i128(&arg, name)?;
            match rest {
                Value::I128(b) => {
                    let result = match name {
                        "i128_eq" => a == b,
                        "i128_not_eq" => a != b,
                        "i128_lt" => a < b,
                        "i128_gt" => a > b,
                        "i128_lt_eq" => a <= b,
                        "i128_gt_eq" => a >= b,
                        _ => unreachable!(),
                    };
                    Ok(Value::Bool(result))
                }
                _ => Err(format!("{}: expected i128 argument", name)),
            }
        }
        "i128_to_string" => {
            let (a, _) = extract_receiver_i128(&arg, "i128_to_string")?;
            Ok(Value::Str(format!("{}i128", a)))
        }
        "i128_to_i64" => {
            let (a, _) = extract_receiver_i128(&arg, "i128_to_i64")?;
            if a < i64::MIN as i128 || a > i64::MAX as i128 {
                Err(format!("to_int: value {} out of range", a))
            } else {
                Ok(Value::I64(a as i64))
            }
        }
        "i128_to_i32" => {
            let (a, _) = extract_receiver_i128(&arg, "i128_to_i32")?;
            if a < i32::MIN as i128 || a > i32::MAX as i128 {
                Err(format!("to_i32: value {} out of range", a))
            } else {
                Ok(Value::I32(a as i32))
            }
        }
        "i128_to_u128" => {
            let (a, _) = extract_receiver_i128(&arg, "i128_to_u128")?;
            if a < 0 {
                Err(format!("to_u128: value {} out of range", a))
            } else {
                Ok(Value::U128(a as u128))
            }
        }

        // ── U128 operator builtins ──
        "u128_add" => {
            let (a, rest) = extract_receiver_u128(&arg, "u128_add")?;
            match rest {
                Value::U128(b) => a.checked_add(b).map(Value::U128)
                    .ok_or_else(|| "integer overflow in u128 addition".to_string()),
                _ => Err("add: expected u128 argument".to_string()),
            }
        }
        "u128_subtract" => {
            let (a, rest) = extract_receiver_u128(&arg, "u128_subtract")?;
            match rest {
                Value::U128(b) => a.checked_sub(b).map(Value::U128)
                    .ok_or_else(|| "integer underflow in u128 subtraction".to_string()),
                _ => Err("subtract: expected u128 argument".to_string()),
            }
        }
        "u128_times" => {
            let (a, rest) = extract_receiver_u128(&arg, "u128_times")?;
            match rest {
                Value::U128(b) => a.checked_mul(b).map(Value::U128)
                    .ok_or_else(|| "integer overflow in u128 multiplication".to_string()),
                _ => Err("times: expected u128 argument".to_string()),
            }
        }
        "u128_divided_by" => {
            let (a, rest) = extract_receiver_u128(&arg, "u128_divided_by")?;
            match rest {
                Value::U128(0) => Err("division by zero".to_string()),
                Value::U128(b) => Ok(Value::U128(a / b)),
                _ => Err("divided_by: expected u128 argument".to_string()),
            }
        }
        "u128_eq" | "u128_not_eq" | "u128_lt" | "u128_gt" | "u128_lt_eq" | "u128_gt_eq" => {
            let (a, rest) = extract_receiver_u128(&arg, name)?;
            match rest {
                Value::U128(b) => {
                    let result = match name {
                        "u128_eq" => a == b,
                        "u128_not_eq" => a != b,
                        "u128_lt" => a < b,
                        "u128_gt" => a > b,
                        "u128_lt_eq" => a <= b,
                        "u128_gt_eq" => a >= b,
                        _ => unreachable!(),
                    };
                    Ok(Value::Bool(result))
                }
                _ => Err(format!("{}: expected u128 argument", name)),
            }
        }
        "u128_to_string" => {
            let (a, _) = extract_receiver_u128(&arg, "u128_to_string")?;
            Ok(Value::Str(format!("{}u128", a)))
        }
        "u128_to_i64" => {
            let (a, _) = extract_receiver_u128(&arg, "u128_to_i64")?;
            if a > i64::MAX as u128 {
                Err(format!("to_int: value {} out of range", a))
            } else {
                Ok(Value::I64(a as i64))
            }
        }
        "u128_to_i32" => {
            let (a, _) = extract_receiver_u128(&arg, "u128_to_i32")?;
            if a > i32::MAX as u128 {
                Err(format!("to_i32: value {} out of range", a))
            } else {
                Ok(Value::I32(a as i32))
            }
        }
        "u128_to_i128" => {
            let (a, _) = extract_receiver_u128(&arg, "u128_to_i128")?;
            if a > i128::MAX as u128 {
                Err(format!("to_i128: value {} out of range", a))
            } else {
                Ok(Value::I128(a as i128))
            }
        }

        // ── I8 operator builtins ──
        "i8_add" => {
            let (a, rest) = extract_receiver_i8(&arg, "i8_add")?;
            match rest {
                Value::I8(b) => a.checked_add(b).map(Value::I8)
                    .ok_or_else(|| "integer overflow in i8 addition".to_string()),
                _ => Err("add: expected i8 argument".to_string()),
            }
        }
        "i8_subtract" => {
            let (a, rest) = extract_receiver_i8(&arg, "i8_subtract")?;
            match rest {
                Value::I8(b) => a.checked_sub(b).map(Value::I8)
                    .ok_or_else(|| "integer overflow in i8 subtraction".to_string()),
                _ => Err("subtract: expected i8 argument".to_string()),
            }
        }
        "i8_times" => {
            let (a, rest) = extract_receiver_i8(&arg, "i8_times")?;
            match rest {
                Value::I8(b) => a.checked_mul(b).map(Value::I8)
                    .ok_or_else(|| "integer overflow in i8 multiplication".to_string()),
                _ => Err("times: expected i8 argument".to_string()),
            }
        }
        "i8_divided_by" => {
            let (a, rest) = extract_receiver_i8(&arg, "i8_divided_by")?;
            match rest {
                Value::I8(0) => Err("division by zero".to_string()),
                Value::I8(b) => a.checked_div(b).map(Value::I8)
                    .ok_or_else(|| "integer overflow in i8 division".to_string()),
                _ => Err("divided_by: expected i8 argument".to_string()),
            }
        }
        "i8_negate" => {
            let (a, _) = extract_receiver_i8(&arg, "i8_negate")?;
            a.checked_neg().map(Value::I8)
                .ok_or_else(|| "integer overflow in i8 negation".to_string())
        }
        "i8_eq" | "i8_not_eq" | "i8_lt" | "i8_gt" | "i8_lt_eq" | "i8_gt_eq" => {
            let (a, rest) = extract_receiver_i8(&arg, name)?;
            match rest {
                Value::I8(b) => {
                    let result = match name {
                        "i8_eq" => a == b,
                        "i8_not_eq" => a != b,
                        "i8_lt" => a < b,
                        "i8_gt" => a > b,
                        "i8_lt_eq" => a <= b,
                        "i8_gt_eq" => a >= b,
                        _ => unreachable!(),
                    };
                    Ok(Value::Bool(result))
                }
                _ => Err(format!("{}: expected i8 argument", name)),
            }
        }
        "i8_to_string" => {
            let (a, _) = extract_receiver_i8(&arg, "i8_to_string")?;
            Ok(Value::Str(format!("{}i8", a)))
        }
        "i8_to_i64" => {
            let (a, _) = extract_receiver_i8(&arg, "i8_to_i64")?;
            Ok(Value::I64(a as i64))
        }

        // ── U8 operator builtins ──
        "u8_add" => {
            let (a, rest) = extract_receiver_u8(&arg, "u8_add")?;
            match rest {
                Value::U8(b) => a.checked_add(b).map(Value::U8)
                    .ok_or_else(|| "integer overflow in u8 addition".to_string()),
                _ => Err("add: expected u8 argument".to_string()),
            }
        }
        "u8_subtract" => {
            let (a, rest) = extract_receiver_u8(&arg, "u8_subtract")?;
            match rest {
                Value::U8(b) => a.checked_sub(b).map(Value::U8)
                    .ok_or_else(|| "integer underflow in u8 subtraction".to_string()),
                _ => Err("subtract: expected u8 argument".to_string()),
            }
        }
        "u8_times" => {
            let (a, rest) = extract_receiver_u8(&arg, "u8_times")?;
            match rest {
                Value::U8(b) => a.checked_mul(b).map(Value::U8)
                    .ok_or_else(|| "integer overflow in u8 multiplication".to_string()),
                _ => Err("times: expected u8 argument".to_string()),
            }
        }
        "u8_divided_by" => {
            let (a, rest) = extract_receiver_u8(&arg, "u8_divided_by")?;
            match rest {
                Value::U8(0) => Err("division by zero".to_string()),
                Value::U8(b) => Ok(Value::U8(a / b)),
                _ => Err("divided_by: expected u8 argument".to_string()),
            }
        }
        "u8_eq" | "u8_not_eq" | "u8_lt" | "u8_gt" | "u8_lt_eq" | "u8_gt_eq" => {
            let (a, rest) = extract_receiver_u8(&arg, name)?;
            match rest {
                Value::U8(b) => {
                    let result = match name {
                        "u8_eq" => a == b,
                        "u8_not_eq" => a != b,
                        "u8_lt" => a < b,
                        "u8_gt" => a > b,
                        "u8_lt_eq" => a <= b,
                        "u8_gt_eq" => a >= b,
                        _ => unreachable!(),
                    };
                    Ok(Value::Bool(result))
                }
                _ => Err(format!("{}: expected u8 argument", name)),
            }
        }
        "u8_to_string" => {
            let (a, _) = extract_receiver_u8(&arg, "u8_to_string")?;
            Ok(Value::Str(format!("{}u8", a)))
        }
        "u8_to_i64" => {
            let (a, _) = extract_receiver_u8(&arg, "u8_to_i64")?;
            Ok(Value::I64(a as i64))
        }

        // ── I16 operator builtins ──
        "i16_add" => {
            let (a, rest) = extract_receiver_i16(&arg, "i16_add")?;
            match rest {
                Value::I16(b) => a.checked_add(b).map(Value::I16)
                    .ok_or_else(|| "integer overflow in i16 addition".to_string()),
                _ => Err("add: expected i16 argument".to_string()),
            }
        }
        "i16_subtract" => {
            let (a, rest) = extract_receiver_i16(&arg, "i16_subtract")?;
            match rest {
                Value::I16(b) => a.checked_sub(b).map(Value::I16)
                    .ok_or_else(|| "integer overflow in i16 subtraction".to_string()),
                _ => Err("subtract: expected i16 argument".to_string()),
            }
        }
        "i16_times" => {
            let (a, rest) = extract_receiver_i16(&arg, "i16_times")?;
            match rest {
                Value::I16(b) => a.checked_mul(b).map(Value::I16)
                    .ok_or_else(|| "integer overflow in i16 multiplication".to_string()),
                _ => Err("times: expected i16 argument".to_string()),
            }
        }
        "i16_divided_by" => {
            let (a, rest) = extract_receiver_i16(&arg, "i16_divided_by")?;
            match rest {
                Value::I16(0) => Err("division by zero".to_string()),
                Value::I16(b) => a.checked_div(b).map(Value::I16)
                    .ok_or_else(|| "integer overflow in i16 division".to_string()),
                _ => Err("divided_by: expected i16 argument".to_string()),
            }
        }
        "i16_negate" => {
            let (a, _) = extract_receiver_i16(&arg, "i16_negate")?;
            a.checked_neg().map(Value::I16)
                .ok_or_else(|| "integer overflow in i16 negation".to_string())
        }
        "i16_eq" | "i16_not_eq" | "i16_lt" | "i16_gt" | "i16_lt_eq" | "i16_gt_eq" => {
            let (a, rest) = extract_receiver_i16(&arg, name)?;
            match rest {
                Value::I16(b) => {
                    let result = match name {
                        "i16_eq" => a == b,
                        "i16_not_eq" => a != b,
                        "i16_lt" => a < b,
                        "i16_gt" => a > b,
                        "i16_lt_eq" => a <= b,
                        "i16_gt_eq" => a >= b,
                        _ => unreachable!(),
                    };
                    Ok(Value::Bool(result))
                }
                _ => Err(format!("{}: expected i16 argument", name)),
            }
        }
        "i16_to_string" => {
            let (a, _) = extract_receiver_i16(&arg, "i16_to_string")?;
            Ok(Value::Str(format!("{}i16", a)))
        }
        "i16_to_i64" => {
            let (a, _) = extract_receiver_i16(&arg, "i16_to_i64")?;
            Ok(Value::I64(a as i64))
        }

        // ── U16 operator builtins ──
        "u16_add" => {
            let (a, rest) = extract_receiver_u16(&arg, "u16_add")?;
            match rest {
                Value::U16(b) => a.checked_add(b).map(Value::U16)
                    .ok_or_else(|| "integer overflow in u16 addition".to_string()),
                _ => Err("add: expected u16 argument".to_string()),
            }
        }
        "u16_subtract" => {
            let (a, rest) = extract_receiver_u16(&arg, "u16_subtract")?;
            match rest {
                Value::U16(b) => a.checked_sub(b).map(Value::U16)
                    .ok_or_else(|| "integer underflow in u16 subtraction".to_string()),
                _ => Err("subtract: expected u16 argument".to_string()),
            }
        }
        "u16_times" => {
            let (a, rest) = extract_receiver_u16(&arg, "u16_times")?;
            match rest {
                Value::U16(b) => a.checked_mul(b).map(Value::U16)
                    .ok_or_else(|| "integer overflow in u16 multiplication".to_string()),
                _ => Err("times: expected u16 argument".to_string()),
            }
        }
        "u16_divided_by" => {
            let (a, rest) = extract_receiver_u16(&arg, "u16_divided_by")?;
            match rest {
                Value::U16(0) => Err("division by zero".to_string()),
                Value::U16(b) => Ok(Value::U16(a / b)),
                _ => Err("divided_by: expected u16 argument".to_string()),
            }
        }
        "u16_eq" | "u16_not_eq" | "u16_lt" | "u16_gt" | "u16_lt_eq" | "u16_gt_eq" => {
            let (a, rest) = extract_receiver_u16(&arg, name)?;
            match rest {
                Value::U16(b) => {
                    let result = match name {
                        "u16_eq" => a == b,
                        "u16_not_eq" => a != b,
                        "u16_lt" => a < b,
                        "u16_gt" => a > b,
                        "u16_lt_eq" => a <= b,
                        "u16_gt_eq" => a >= b,
                        _ => unreachable!(),
                    };
                    Ok(Value::Bool(result))
                }
                _ => Err(format!("{}: expected u16 argument", name)),
            }
        }
        "u16_to_string" => {
            let (a, _) = extract_receiver_u16(&arg, "u16_to_string")?;
            Ok(Value::Str(format!("{}u16", a)))
        }
        "u16_to_i64" => {
            let (a, _) = extract_receiver_u16(&arg, "u16_to_i64")?;
            Ok(Value::I64(a as i64))
        }

        // ── U32 operator builtins ──
        "u32_add" => {
            let (a, rest) = extract_receiver_u32(&arg, "u32_add")?;
            match rest {
                Value::U32(b) => a.checked_add(b).map(Value::U32)
                    .ok_or_else(|| "integer overflow in u32 addition".to_string()),
                _ => Err("add: expected u32 argument".to_string()),
            }
        }
        "u32_subtract" => {
            let (a, rest) = extract_receiver_u32(&arg, "u32_subtract")?;
            match rest {
                Value::U32(b) => a.checked_sub(b).map(Value::U32)
                    .ok_or_else(|| "integer underflow in u32 subtraction".to_string()),
                _ => Err("subtract: expected u32 argument".to_string()),
            }
        }
        "u32_times" => {
            let (a, rest) = extract_receiver_u32(&arg, "u32_times")?;
            match rest {
                Value::U32(b) => a.checked_mul(b).map(Value::U32)
                    .ok_or_else(|| "integer overflow in u32 multiplication".to_string()),
                _ => Err("times: expected u32 argument".to_string()),
            }
        }
        "u32_divided_by" => {
            let (a, rest) = extract_receiver_u32(&arg, "u32_divided_by")?;
            match rest {
                Value::U32(0) => Err("division by zero".to_string()),
                Value::U32(b) => Ok(Value::U32(a / b)),
                _ => Err("divided_by: expected u32 argument".to_string()),
            }
        }
        "u32_eq" | "u32_not_eq" | "u32_lt" | "u32_gt" | "u32_lt_eq" | "u32_gt_eq" => {
            let (a, rest) = extract_receiver_u32(&arg, name)?;
            match rest {
                Value::U32(b) => {
                    let result = match name {
                        "u32_eq" => a == b,
                        "u32_not_eq" => a != b,
                        "u32_lt" => a < b,
                        "u32_gt" => a > b,
                        "u32_lt_eq" => a <= b,
                        "u32_gt_eq" => a >= b,
                        _ => unreachable!(),
                    };
                    Ok(Value::Bool(result))
                }
                _ => Err(format!("{}: expected u32 argument", name)),
            }
        }
        "u32_to_string" => {
            let (a, _) = extract_receiver_u32(&arg, "u32_to_string")?;
            Ok(Value::Str(format!("{}u32", a)))
        }
        "u32_to_i64" => {
            let (a, _) = extract_receiver_u32(&arg, "u32_to_i64")?;
            Ok(Value::I64(a as i64))
        }

        // ── U64 operator builtins ──
        "u64_add" => {
            let (a, rest) = extract_receiver_u64(&arg, "u64_add")?;
            match rest {
                Value::U64(b) => a.checked_add(b).map(Value::U64)
                    .ok_or_else(|| "integer overflow in u64 addition".to_string()),
                _ => Err("add: expected u64 argument".to_string()),
            }
        }
        "u64_subtract" => {
            let (a, rest) = extract_receiver_u64(&arg, "u64_subtract")?;
            match rest {
                Value::U64(b) => a.checked_sub(b).map(Value::U64)
                    .ok_or_else(|| "integer underflow in u64 subtraction".to_string()),
                _ => Err("subtract: expected u64 argument".to_string()),
            }
        }
        "u64_times" => {
            let (a, rest) = extract_receiver_u64(&arg, "u64_times")?;
            match rest {
                Value::U64(b) => a.checked_mul(b).map(Value::U64)
                    .ok_or_else(|| "integer overflow in u64 multiplication".to_string()),
                _ => Err("times: expected u64 argument".to_string()),
            }
        }
        "u64_divided_by" => {
            let (a, rest) = extract_receiver_u64(&arg, "u64_divided_by")?;
            match rest {
                Value::U64(0) => Err("division by zero".to_string()),
                Value::U64(b) => Ok(Value::U64(a / b)),
                _ => Err("divided_by: expected u64 argument".to_string()),
            }
        }
        "u64_eq" | "u64_not_eq" | "u64_lt" | "u64_gt" | "u64_lt_eq" | "u64_gt_eq" => {
            let (a, rest) = extract_receiver_u64(&arg, name)?;
            match rest {
                Value::U64(b) => {
                    let result = match name {
                        "u64_eq" => a == b,
                        "u64_not_eq" => a != b,
                        "u64_lt" => a < b,
                        "u64_gt" => a > b,
                        "u64_lt_eq" => a <= b,
                        "u64_gt_eq" => a >= b,
                        _ => unreachable!(),
                    };
                    Ok(Value::Bool(result))
                }
                _ => Err(format!("{}: expected u64 argument", name)),
            }
        }
        "u64_to_string" => {
            let (a, _) = extract_receiver_u64(&arg, "u64_to_string")?;
            Ok(Value::Str(format!("{}u64", a)))
        }
        "u64_to_i64" => {
            let (a, _) = extract_receiver_u64(&arg, "u64_to_i64")?;
            Ok(Value::I64(a as i64))
        }

        // ── F64 operator builtins ──
        "f64_add" => {
            let (a, rest) = extract_receiver_f64(&arg, "f64_add")?;
            match rest {
                Value::F64(b) => Ok(Value::F64(a + b)),
                _ => Err("add: expected f64 argument".to_string()),
            }
        }
        "f64_subtract" => {
            let (a, rest) = extract_receiver_f64(&arg, "f64_subtract")?;
            match rest {
                Value::F64(b) => Ok(Value::F64(a - b)),
                _ => Err("subtract: expected f64 argument".to_string()),
            }
        }
        "f64_times" => {
            let (a, rest) = extract_receiver_f64(&arg, "f64_times")?;
            match rest {
                Value::F64(b) => Ok(Value::F64(a * b)),
                _ => Err("times: expected f64 argument".to_string()),
            }
        }
        "f64_divided_by" => {
            let (a, rest) = extract_receiver_f64(&arg, "f64_divided_by")?;
            match rest {
                Value::F64(b) if b == 0.0 => Err("division by zero".to_string()),
                Value::F64(b) => Ok(Value::F64(a / b)),
                _ => Err("divided_by: expected f64 argument".to_string()),
            }
        }
        "f64_negate" => {
            let (a, _) = extract_receiver_f64(&arg, "f64_negate")?;
            Ok(Value::F64(-a))
        }
        "f64_eq" | "f64_not_eq" | "f64_lt" | "f64_gt" | "f64_lt_eq" | "f64_gt_eq" => {
            let (a, rest) = extract_receiver_f64(&arg, name)?;
            match rest {
                Value::F64(b) => {
                    let result = match name {
                        "f64_eq" => a == b,
                        "f64_not_eq" => a != b,
                        "f64_lt" => a < b,
                        "f64_gt" => a > b,
                        "f64_lt_eq" => a <= b,
                        "f64_gt_eq" => a >= b,
                        _ => unreachable!(),
                    };
                    Ok(Value::Bool(result))
                }
                _ => Err(format!("{}: expected f64 argument", name)),
            }
        }
        "f64_to_string" => {
            let (a, _) = extract_receiver_f64(&arg, "f64_to_string")?;
            if a.fract() == 0.0 {
                Ok(Value::Str(format!("{:.1}f64", a)))
            } else {
                Ok(Value::Str(format!("{}f64", a)))
            }
        }
        "f64_to_f64" => {
            let (a, _) = extract_receiver_f64(&arg, "f64_to_f64")?;
            Ok(Value::F64(a))
        }
        "f64_to_i64" => {
            let (a, _) = extract_receiver_f64(&arg, "f64_to_i64")?;
            Ok(Value::I64(a as i64))
        }
        "f64_ceil" => {
            let (a, _) = extract_receiver_f64(&arg, "f64_ceil")?;
            Ok(Value::I64(a.ceil() as i64))
        }
        "f64_floor" => {
            let (a, _) = extract_receiver_f64(&arg, "f64_floor")?;
            Ok(Value::I64(a.floor() as i64))
        }
        "f64_round" => {
            let (a, _) = extract_receiver_f64(&arg, "f64_round")?;
            Ok(Value::I64(a.round() as i64))
        }
        "f64_trunc" => {
            let (a, _) = extract_receiver_f64(&arg, "f64_trunc")?;
            Ok(Value::I64(a.trunc() as i64))
        }

        // ── Float operator builtins ──
        "float_add" => {
            let (a, rest) = extract_receiver_f64(&arg, "float_add")?;
            match rest {
                Value::F64(b) => Ok(Value::F64(a + b)),
                _ => Err("add: expected float argument".to_string()),
            }
        }
        "float_subtract" => {
            let (a, rest) = extract_receiver_f64(&arg, "float_subtract")?;
            match rest {
                Value::F64(b) => Ok(Value::F64(a - b)),
                _ => Err("subtract: expected float argument".to_string()),
            }
        }
        "float_times" => {
            let (a, rest) = extract_receiver_f64(&arg, "float_times")?;
            match rest {
                Value::F64(b) => Ok(Value::F64(a * b)),
                _ => Err("times: expected float argument".to_string()),
            }
        }
        "float_divided_by" => {
            let (a, rest) = extract_receiver_f64(&arg, "float_divided_by")?;
            match rest {
                Value::F64(b) if b == 0.0 => Err("division by zero".to_string()),
                Value::F64(b) => Ok(Value::F64(a / b)),
                _ => Err("divided_by: expected float argument".to_string()),
            }
        }
        "float_negate" => {
            let (a, _) = extract_receiver_f64(&arg, "float_negate")?;
            Ok(Value::F64(-a))
        }
        "float_eq" => {
            let (a, rest) = extract_receiver_f64(&arg, "float_eq")?;
            match rest { Value::F64(b) => Ok(Value::Bool(a == b)), _ => Err("eq: expected float".to_string()) }
        }
        "float_not_eq" => {
            let (a, rest) = extract_receiver_f64(&arg, "float_not_eq")?;
            match rest { Value::F64(b) => Ok(Value::Bool(a != b)), _ => Err("not_eq: expected float".to_string()) }
        }
        "float_lt" => {
            let (a, rest) = extract_receiver_f64(&arg, "float_lt")?;
            match rest {
                Value::F64(b) => a.partial_cmp(&b).map(|o| Value::Bool(o.is_lt())).ok_or_else(|| "NaN comparison".to_string()),
                _ => Err("lt: expected float".to_string()),
            }
        }
        "float_gt" => {
            let (a, rest) = extract_receiver_f64(&arg, "float_gt")?;
            match rest {
                Value::F64(b) => a.partial_cmp(&b).map(|o| Value::Bool(o.is_gt())).ok_or_else(|| "NaN comparison".to_string()),
                _ => Err("gt: expected float".to_string()),
            }
        }
        "float_lt_eq" => {
            let (a, rest) = extract_receiver_f64(&arg, "float_lt_eq")?;
            match rest {
                Value::F64(b) => a.partial_cmp(&b).map(|o| Value::Bool(o.is_le())).ok_or_else(|| "NaN comparison".to_string()),
                _ => Err("lt_eq: expected float".to_string()),
            }
        }
        "float_gt_eq" => {
            let (a, rest) = extract_receiver_f64(&arg, "float_gt_eq")?;
            match rest {
                Value::F64(b) => a.partial_cmp(&b).map(|o| Value::Bool(o.is_ge())).ok_or_else(|| "NaN comparison".to_string()),
                _ => Err("gt_eq: expected float".to_string()),
            }
        }
        "float_to_string" => {
            let (a, _) = extract_receiver_f64(&arg, "float_to_string")?;
            // Match Display impl: whole-number floats get ".0" suffix
            let s = if a.fract() == 0.0 {
                format!("{}.0", a)
            } else {
                format!("{}", a)
            };
            Ok(Value::Str(s))
        }

        // ── String operator builtins ──
        "string_add" => {
            let (a, rest) = extract_receiver_str(&arg, "string_add")?;
            match rest {
                Value::Str(b) => Ok(Value::Str(format!("{}{}", a, b))),
                _ => Err("add: expected string argument".to_string()),
            }
        }
        "string_eq" => {
            let (a, rest) = extract_receiver_str(&arg, "string_eq")?;
            match rest { Value::Str(b) => Ok(Value::Bool(a == b)), _ => Err("eq: expected string".to_string()) }
        }
        "string_not_eq" => {
            let (a, rest) = extract_receiver_str(&arg, "string_not_eq")?;
            match rest { Value::Str(b) => Ok(Value::Bool(a != b)), _ => Err("not_eq: expected string".to_string()) }
        }
        "string_lt" => {
            let (a, rest) = extract_receiver_str(&arg, "string_lt")?;
            match rest { Value::Str(b) => Ok(Value::Bool(a < b)), _ => Err("lt: expected string".to_string()) }
        }
        "string_gt" => {
            let (a, rest) = extract_receiver_str(&arg, "string_gt")?;
            match rest { Value::Str(b) => Ok(Value::Bool(a > b)), _ => Err("gt: expected string".to_string()) }
        }
        "string_lt_eq" => {
            let (a, rest) = extract_receiver_str(&arg, "string_lt_eq")?;
            match rest { Value::Str(b) => Ok(Value::Bool(a <= b)), _ => Err("lt_eq: expected string".to_string()) }
        }
        "string_gt_eq" => {
            let (a, rest) = extract_receiver_str(&arg, "string_gt_eq")?;
            match rest { Value::Str(b) => Ok(Value::Bool(a >= b)), _ => Err("gt_eq: expected string".to_string()) }
        }
        "string_to_string" => {
            let (a, _) = extract_receiver_str(&arg, "string_to_string")?;
            Ok(Value::Str(a))
        }

        // ── Array operator builtins ──
        "array_add" => {
            let (a, rest) = extract_receiver_array(&arg, "array_add")?;
            match rest {
                Value::Array(b) => {
                    let mut result = a;
                    result.extend(b);
                    Ok(Value::Array(result))
                }
                _ => Err("add: expected array argument".to_string()),
            }
        }
        "array_eq" => {
            let (a, rest) = extract_receiver_array(&arg, "array_eq")?;
            match rest {
                Value::Array(b) => {
                    if a.len() != b.len() { return Ok(Value::Bool(false)); }
                    for (x, y) in a.iter().zip(b.iter()) {
                        match eval_compare(CmpOp::Eq, x, y)? {
                            Value::Bool(false) => return Ok(Value::Bool(false)),
                            _ => {}
                        }
                    }
                    Ok(Value::Bool(true))
                }
                _ => Err("eq: expected array".to_string()),
            }
        }
        "array_not_eq" => {
            let (a, rest) = extract_receiver_array(&arg, "array_not_eq")?;
            match rest {
                Value::Array(b) => {
                    if a.len() != b.len() { return Ok(Value::Bool(true)); }
                    for (x, y) in a.iter().zip(b.iter()) {
                        match eval_compare(CmpOp::Eq, x, y)? {
                            Value::Bool(false) => return Ok(Value::Bool(true)),
                            _ => {}
                        }
                    }
                    Ok(Value::Bool(false))
                }
                _ => Err("not_eq: expected array".to_string()),
            }
        }

        // ── Bool operator builtins ──
        "bool_eq" => {
            let (a, rest) = extract_receiver_bool(&arg, "bool_eq")?;
            match rest { Value::Bool(b) => Ok(Value::Bool(a == b)), _ => Err("eq: expected bool".to_string()) }
        }
        "bool_not_eq" => {
            let (a, rest) = extract_receiver_bool(&arg, "bool_not_eq")?;
            match rest { Value::Bool(b) => Ok(Value::Bool(a != b)), _ => Err("not_eq: expected bool".to_string()) }
        }
        "bool_to_string" => {
            let (a, _) = extract_receiver_bool(&arg, "bool_to_string")?;
            Ok(Value::Str(a.to_string()))
        }

        // ── Char operator builtins ──
        "char_eq" => {
            let (a, rest) = extract_receiver_char(&arg, "char_eq")?;
            match rest { Value::Char(b) => Ok(Value::Bool(a == b)), _ => Err("eq: expected char".to_string()) }
        }
        "char_not_eq" => {
            let (a, rest) = extract_receiver_char(&arg, "char_not_eq")?;
            match rest { Value::Char(b) => Ok(Value::Bool(a != b)), _ => Err("not_eq: expected char".to_string()) }
        }
        "char_lt" => {
            let (a, rest) = extract_receiver_char(&arg, "char_lt")?;
            match rest { Value::Char(b) => Ok(Value::Bool(a < b)), _ => Err("lt: expected char".to_string()) }
        }
        "char_gt" => {
            let (a, rest) = extract_receiver_char(&arg, "char_gt")?;
            match rest { Value::Char(b) => Ok(Value::Bool(a > b)), _ => Err("gt: expected char".to_string()) }
        }
        "char_lt_eq" => {
            let (a, rest) = extract_receiver_char(&arg, "char_lt_eq")?;
            match rest { Value::Char(b) => Ok(Value::Bool(a <= b)), _ => Err("lt_eq: expected char".to_string()) }
        }
        "char_gt_eq" => {
            let (a, rest) = extract_receiver_char(&arg, "char_gt_eq")?;
            match rest { Value::Char(b) => Ok(Value::Bool(a >= b)), _ => Err("gt_eq: expected char".to_string()) }
        }
        "char_to_string" => {
            let (a, _) = extract_receiver_char(&arg, "char_to_string")?;
            Ok(Value::Str(a.to_string()))
        }

        // ── Byte operator builtins ──
        "byte_eq" => {
            let (a, rest) = extract_receiver_u8(&arg, "byte_eq")?;
            match rest { Value::U8(b) => Ok(Value::Bool(a == b)), _ => Err("eq: expected byte".to_string()) }
        }
        "byte_not_eq" => {
            let (a, rest) = extract_receiver_u8(&arg, "byte_not_eq")?;
            match rest { Value::U8(b) => Ok(Value::Bool(a != b)), _ => Err("not_eq: expected byte".to_string()) }
        }
        "byte_lt" => {
            let (a, rest) = extract_receiver_u8(&arg, "byte_lt")?;
            match rest { Value::U8(b) => Ok(Value::Bool(a < b)), _ => Err("lt: expected byte".to_string()) }
        }
        "byte_gt" => {
            let (a, rest) = extract_receiver_u8(&arg, "byte_gt")?;
            match rest { Value::U8(b) => Ok(Value::Bool(a > b)), _ => Err("gt: expected byte".to_string()) }
        }
        "byte_lt_eq" => {
            let (a, rest) = extract_receiver_u8(&arg, "byte_lt_eq")?;
            match rest { Value::U8(b) => Ok(Value::Bool(a <= b)), _ => Err("lt_eq: expected byte".to_string()) }
        }
        "byte_gt_eq" => {
            let (a, rest) = extract_receiver_u8(&arg, "byte_gt_eq")?;
            match rest { Value::U8(b) => Ok(Value::Bool(a >= b)), _ => Err("gt_eq: expected byte".to_string()) }
        }
        "byte_to_string" => {
            let (a, _) = extract_receiver_u8(&arg, "byte_to_string")?;
            Ok(Value::Str(format!("0x{:02x}", a)))
        }

        // ── Unit operator builtins ──
        "unit_eq" => {
            let rest = extract_receiver_unit(&arg, "unit_eq")?;
            Ok(Value::Bool(rest == Value::Unit))
        }
        "unit_not_eq" => {
            let rest = extract_receiver_unit(&arg, "unit_not_eq")?;
            Ok(Value::Bool(rest != Value::Unit))
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
pub fn eval_toplevel(expr: &Mir, env: &Env, input: &Value) -> Result<(Value, Env), String> {
    eval_collecting(expr, env, input)
}

fn eval_collecting(expr: &Mir, env: &Env, input: &Value) -> Result<(Value, Env), String> {
    match expr.as_ref() {
        MirKind::Bind { name, value, body } => {
            let val = eval(value, env, input)?;
            let new_env = env.bind(name.clone(), val);
            eval_collecting(body, &new_env, input)
        }
        MirKind::Pipe(lhs, rhs) if has_toplevel_let(rhs) => {
            let lhs_val = eval(lhs, env, input)?;
            eval_pipe_collecting(&lhs_val, rhs, env, input)
        }
        MirKind::Let { pattern, body } => {
            let new_env = bind_pattern(pattern, input, env)?;
            let new_env = new_env.bind("\0".to_string(), input.clone());
            eval_collecting(body, &new_env, input)
        }
        MirKind::LetArray { patterns, body } => {
            let new_env = bind_array_pattern(patterns, input, env)?;
            let new_env = new_env.bind("\0".to_string(), input.clone());
            eval_collecting(body, &new_env, input)
        }
        MirKind::Apply { expr: ms_expr, body } => {
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

fn eval_pipe_collecting(lhs_val: &Value, rhs: &Mir, env: &Env, input: &Value) -> Result<(Value, Env), String> {
    match rhs.as_ref() {
        MirKind::Let { pattern, body } => {
            let new_env = bind_pattern(pattern, lhs_val, env)?;
            let new_env = apply_prelude(lhs_val, &new_env);
            let new_env = new_env.bind("\0".to_string(), lhs_val.clone());
            eval_collecting(body, &new_env, input)
        }
        MirKind::LetArray { patterns, body } => {
            let new_env = bind_array_pattern(patterns, lhs_val, env)?;
            let new_env = new_env.bind("\0".to_string(), lhs_val.clone());
            eval_collecting(body, &new_env, input)
        }
        MirKind::Apply { expr: ms_expr, body } => {
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

fn has_toplevel_let(expr: &Mir) -> bool {
    matches!(expr.as_ref(), MirKind::Let { .. } | MirKind::LetArray { .. } | MirKind::Apply { .. })
}

/// Build the core module as a Value::Struct.
pub fn build_core_module() -> Value {
    let mut fields = Vec::new();

    // Type constructors for primitive types (using reserved TagIds)
    let type_constructors = [
        ("I64", TAG_ID_I64),
        ("F64", TAG_ID_F64),
        ("Bool", TAG_ID_BOOL),
        ("String", TAG_ID_STRING),
        ("Char", TAG_ID_CHAR),
        ("U8", TAG_ID_U8),
        ("Array", TAG_ID_ARRAY),
        ("Unit", TAG_ID_UNIT),
        ("I32", TAG_ID_I32),
        ("F32", TAG_ID_F32),
        ("I128", TAG_ID_I128),
        ("U128", TAG_ID_U128),
        ("I8", TAG_ID_I8),
        ("I16", TAG_ID_I16),
        ("U16", TAG_ID_U16),
        ("U32", TAG_ID_U32),
        ("U64", TAG_ID_U64),
        ("F64", TAG_ID_F64),
    ];
    for (name, id) in &type_constructors {
        fields.push((name.to_string(), Value::TagConstructor {
            id: *id,
            name: name.to_string(),
        }));
    }

    // All builtin functions
    let builtins = [
        "not", "and", "or", "print",
        "byte", "int", "i64", "float", "char", "i32", "f32", "i128", "u128",
        "i8", "u8", "i16", "u16", "u32", "u64", "f64",
        "ref_eq", "val_eq", "method_set",
        // Array method builtins (receiver as first arg)
        "array_get", "array_slice", "array_len", "array_map", "array_filter",
        "array_fold", "array_zip",
        // Array operator builtins
        "array_add", "array_eq", "array_not_eq",
        // String method builtins (receiver as first arg)
        "string_byte_len", "string_char_len", "string_byte_get", "string_char_get",
        "string_as_bytes", "string_chars", "string_split", "string_trim",
        "string_contains", "string_contains_char", "string_slice", "string_starts_with", "string_ends_with",
        "string_replace",
        // String operator builtins
        "string_add", "string_eq", "string_not_eq", "string_lt", "string_gt",
        "string_lt_eq", "string_gt_eq", "string_to_string",
        // Int operator builtins
        "int_add", "int_subtract", "int_times", "int_divided_by", "int_negate",
        "int_eq", "int_not_eq", "int_lt", "int_gt", "int_lt_eq", "int_gt_eq",
        "int_to_string", "int_to_f64", "int_to_u8", "int_to_char",
        // I32 operator builtins
        "i32_add", "i32_subtract", "i32_times", "i32_divided_by", "i32_negate",
        "i32_eq", "i32_not_eq", "i32_lt", "i32_gt", "i32_lt_eq", "i32_gt_eq",
        "i32_to_string", "i32_to_i64", "i32_to_f64", "i32_to_f32", "i32_to_u8",
        // I128 operator builtins
        "i128_add", "i128_subtract", "i128_times", "i128_divided_by", "i128_negate",
        "i128_eq", "i128_not_eq", "i128_lt", "i128_gt", "i128_lt_eq", "i128_gt_eq",
        "i128_to_string", "i128_to_i64", "i128_to_i32", "i128_to_u128",
        // U128 operator builtins
        "u128_add", "u128_subtract", "u128_times", "u128_divided_by",
        "u128_eq", "u128_not_eq", "u128_lt", "u128_gt", "u128_lt_eq", "u128_gt_eq",
        "u128_to_string", "u128_to_i64", "u128_to_i32", "u128_to_i128",
        // F32 operator builtins
        "f32_add", "f32_subtract", "f32_times", "f32_divided_by", "f32_negate",
        "f32_eq", "f32_not_eq", "f32_lt", "f32_gt", "f32_lt_eq", "f32_gt_eq",
        "f32_to_string", "f32_to_f64", "f32_to_i64", "f32_to_i32",
        "f32_ceil", "f32_floor", "f32_round", "f32_trunc",
        // Float operator builtins
        "float_add", "float_subtract", "float_times", "float_divided_by", "float_negate",
        "float_eq", "float_not_eq", "float_lt", "float_gt", "float_lt_eq", "float_gt_eq",
        "float_to_string", "float_to_i64", "float_ceil", "float_floor", "float_round", "float_trunc",
        // Bool operator builtins
        "bool_eq", "bool_not_eq", "bool_to_string",
        // Char operator builtins
        "char_eq", "char_not_eq", "char_lt", "char_gt", "char_lt_eq", "char_gt_eq",
        "char_to_string", "char_to_i64",
        // Byte operator builtins
        "byte_eq", "byte_not_eq", "byte_lt", "byte_gt", "byte_lt_eq", "byte_gt_eq",
        "byte_to_string", "byte_to_i64",
        // Unit operator builtins
        "unit_eq", "unit_not_eq",
        // Cross-type conversions
        "int_to_i32", "float_to_f32", "u8_to_i32",
        "int_to_i128", "int_to_u128", "u8_to_i128", "u8_to_u128",
        "i32_to_i128", "i32_to_u128",
        // Cross-type conversions for new types
        "int_to_i8", "int_to_i16", "int_to_u16",
        "int_to_u32", "int_to_u64", "int_to_f32", "float_to_f64",
        // I8 operator builtins
        "i8_add", "i8_subtract", "i8_times", "i8_divided_by", "i8_negate",
        "i8_eq", "i8_not_eq", "i8_lt", "i8_gt", "i8_lt_eq", "i8_gt_eq",
        "i8_to_string", "i8_to_i64",
        // U8 operator builtins
        "u8_add", "u8_subtract", "u8_times", "u8_divided_by",
        "u8_eq", "u8_not_eq", "u8_lt", "u8_gt", "u8_lt_eq", "u8_gt_eq",
        "u8_to_string", "u8_to_i64",
        // I16 operator builtins
        "i16_add", "i16_subtract", "i16_times", "i16_divided_by", "i16_negate",
        "i16_eq", "i16_not_eq", "i16_lt", "i16_gt", "i16_lt_eq", "i16_gt_eq",
        "i16_to_string", "i16_to_i64",
        // U16 operator builtins
        "u16_add", "u16_subtract", "u16_times", "u16_divided_by",
        "u16_eq", "u16_not_eq", "u16_lt", "u16_gt", "u16_lt_eq", "u16_gt_eq",
        "u16_to_string", "u16_to_i64",
        // U32 operator builtins
        "u32_add", "u32_subtract", "u32_times", "u32_divided_by",
        "u32_eq", "u32_not_eq", "u32_lt", "u32_gt", "u32_lt_eq", "u32_gt_eq",
        "u32_to_string", "u32_to_i64",
        // U64 operator builtins
        "u64_add", "u64_subtract", "u64_times", "u64_divided_by",
        "u64_eq", "u64_not_eq", "u64_lt", "u64_gt", "u64_lt_eq", "u64_gt_eq",
        "u64_to_string", "u64_to_i64",
        // F64 operator builtins
        "f64_add", "f64_subtract", "f64_times", "f64_divided_by", "f64_negate",
        "f64_eq", "f64_not_eq", "f64_lt", "f64_gt", "f64_lt_eq", "f64_gt_eq",
        "f64_to_string", "f64_to_f64", "f64_to_i64",
        "f64_ceil", "f64_floor", "f64_round", "f64_trunc",
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
