//! Function and Method Inlining Pass for Perry HIR
//!
//! This module inlines small functions and methods at their call sites to eliminate
//! call overhead and enable further optimizations.

use perry_hir::{Expr, Function, Module, Stmt};
use perry_types::{FuncId, LocalId, Type};
use std::collections::HashMap;

/// Maximum number of statements for a function to be considered for inlining
const MAX_INLINE_STMTS: usize = 10;

/// Information about a method that can be inlined
#[derive(Clone)]
struct MethodCandidate {
    func: Function,
    /// The index of the `this` parameter (if present)
    this_param_id: Option<LocalId>,
}

/// Inline small functions and methods in the module
pub fn inline_functions(module: &mut Module) {
    // Phase 1: Identify inlinable functions
    let func_candidates: HashMap<FuncId, Function> = module.functions.iter()
        .filter(|f| is_inlinable(f))
        .map(|f| (f.id, f.clone()))
        .collect();

    // Phase 2: Identify inlinable methods (class_name, method_name) -> MethodCandidate
    let mut method_candidates: HashMap<(String, String), MethodCandidate> = HashMap::new();
    for class in &module.classes {
        // Don't inline methods from classes with native parents (e.g., EventEmitter)
        // because the `this` reference needs special handling in those contexts
        if class.native_extends.is_some() {
            continue;
        }

        for method in &class.methods {
            if is_inlinable(method) {
                // Note: Methods don't have 'this' as a parameter in the HIR.
                // They access 'this' via Expr::This. So this_param_id is None.
                method_candidates.insert(
                    (class.name.clone(), method.name.clone()),
                    MethodCandidate {
                        func: method.clone(),
                        this_param_id: None,
                    },
                );
            }
        }
    }

    // Phase 3: Build class name lookup for types
    let class_names: HashMap<String, String> = module.classes.iter()
        .map(|c| (c.name.clone(), c.name.clone()))
        .collect();

    // Phase 4: Inline calls in init statements
    let mut next_local_id = find_max_local_id(&module.init) + 1;
    let mut local_types: HashMap<LocalId, String> = HashMap::new();
    inline_calls_in_stmts(&mut module.init, &func_candidates, &method_candidates, &class_names, &mut local_types, &mut next_local_id);

    // Phase 5: Inline calls in function bodies
    for func in &mut module.functions {
        if func_candidates.contains_key(&func.id) {
            continue;
        }
        let mut local_id = find_max_local_id(&func.body) + 1;
        let mut local_types: HashMap<LocalId, String> = HashMap::new();
        // Add function parameters to local_types
        for param in &func.params {
            if let Type::Named(class_name) = &param.ty {
                local_types.insert(param.id, class_name.clone());
            }
        }
        inline_calls_in_stmts(&mut func.body, &func_candidates, &method_candidates, &class_names, &mut local_types, &mut local_id);
    }

    // Phase 6: Inline calls in class method bodies
    for class in &mut module.classes {
        for method in &mut class.methods {
            // Skip if this method is itself a candidate (avoid recursion)
            if method_candidates.contains_key(&(class.name.clone(), method.name.clone())) {
                continue;
            }
            let mut local_id = find_max_local_id(&method.body) + 1;
            let mut local_types: HashMap<LocalId, String> = HashMap::new();
            for param in &method.params {
                if let Type::Named(class_name) = &param.ty {
                    local_types.insert(param.id, class_name.clone());
                }
            }
            inline_calls_in_stmts(&mut method.body, &func_candidates, &method_candidates, &class_names, &mut local_types, &mut local_id);
        }
    }
}

/// Check if a function is suitable for inlining
fn is_inlinable(func: &Function) -> bool {
    // Don't inline async functions
    if func.is_async {
        return false;
    }

    // Don't inline functions with captures (closures)
    if !func.captures.is_empty() {
        return false;
    }

    // Don't inline functions that are too large
    if func.body.len() > MAX_INLINE_STMTS {
        return false;
    }

    // Check for simple patterns
    if !has_simple_control_flow(&func.body) {
        return false;
    }

    // Don't inline functions that return closures capturing parameters
    // When inlined, the parameter IDs won't exist in the outer context
    let param_ids: std::collections::HashSet<LocalId> = func.params.iter().map(|p| p.id).collect();
    if body_contains_closure_capturing(&func.body, &param_ids) {
        return false;
    }

    true
}

/// Check if statements contain a closure that captures any of the given local IDs
fn body_contains_closure_capturing(stmts: &[Stmt], captured_ids: &std::collections::HashSet<LocalId>) -> bool {
    fn check_expr(expr: &Expr, captured_ids: &std::collections::HashSet<LocalId>) -> bool {
        match expr {
            Expr::Closure { captures, body, .. } => {
                // Check if any capture is in the set of IDs we're looking for
                for capture_id in captures {
                    if captured_ids.contains(capture_id) {
                        return true;
                    }
                }
                // Also check the closure body for nested closures
                body_contains_closure_capturing(body, captured_ids)
            }
            Expr::Binary { left, right, .. } | Expr::Logical { left, right, .. } |
            Expr::Compare { left, right, .. } => {
                check_expr(left, captured_ids) || check_expr(right, captured_ids)
            }
            Expr::Unary { operand, .. } => check_expr(operand, captured_ids),
            Expr::Conditional { condition, then_expr, else_expr } => {
                check_expr(condition, captured_ids) ||
                check_expr(then_expr, captured_ids) ||
                check_expr(else_expr, captured_ids)
            }
            Expr::Call { callee, args, .. } => {
                check_expr(callee, captured_ids) ||
                args.iter().any(|a| check_expr(a, captured_ids))
            }
            Expr::Array(elements) => elements.iter().any(|e| check_expr(e, captured_ids)),
            Expr::IndexGet { object, index } => {
                check_expr(object, captured_ids) || check_expr(index, captured_ids)
            }
            Expr::IndexSet { object, index, value } => {
                check_expr(object, captured_ids) ||
                check_expr(index, captured_ids) ||
                check_expr(value, captured_ids)
            }
            Expr::PropertyGet { object, .. } => check_expr(object, captured_ids),
            Expr::PropertySet { object, value, .. } => {
                check_expr(object, captured_ids) || check_expr(value, captured_ids)
            }
            Expr::LocalSet(_, value) => check_expr(value, captured_ids),
            _ => false,
        }
    }

    fn check_stmt(stmt: &Stmt, captured_ids: &std::collections::HashSet<LocalId>) -> bool {
        match stmt {
            Stmt::Let { init: Some(expr), .. } => check_expr(expr, captured_ids),
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                check_expr(expr, captured_ids)
            }
            Stmt::If { condition, then_branch, else_branch } => {
                check_expr(condition, captured_ids) ||
                then_branch.iter().any(|s| check_stmt(s, captured_ids)) ||
                else_branch.as_ref().map_or(false, |b| b.iter().any(|s| check_stmt(s, captured_ids)))
            }
            Stmt::While { condition, body } => {
                check_expr(condition, captured_ids) ||
                body.iter().any(|s| check_stmt(s, captured_ids))
            }
            Stmt::For { init, condition, update, body } => {
                init.as_ref().map_or(false, |i| check_stmt(i, captured_ids)) ||
                condition.as_ref().map_or(false, |c| check_expr(c, captured_ids)) ||
                update.as_ref().map_or(false, |u| check_expr(u, captured_ids)) ||
                body.iter().any(|s| check_stmt(s, captured_ids))
            }
            _ => false,
        }
    }

    stmts.iter().any(|s| check_stmt(s, captured_ids))
}

/// Check if statements have simple control flow suitable for inlining
fn has_simple_control_flow(stmts: &[Stmt]) -> bool {
    for stmt in stmts {
        match stmt {
            Stmt::Let { .. } | Stmt::Expr(_) | Stmt::Return(_) => {}
            Stmt::If { then_branch, else_branch, .. } => {
                if !has_simple_control_flow(then_branch) {
                    return false;
                }
                if let Some(else_b) = else_branch {
                    if !has_simple_control_flow(else_b) {
                        return false;
                    }
                }
            }
            Stmt::While { .. } | Stmt::For { .. } | Stmt::Try { .. } |
            Stmt::Switch { .. } | Stmt::Break | Stmt::Continue | Stmt::Throw(_) => {
                return false;
            }
        }
    }
    true
}

/// Find the maximum local ID used in statements
fn find_max_local_id(stmts: &[Stmt]) -> LocalId {
    let mut max_id: LocalId = 0;

    fn check_expr(expr: &Expr, max_id: &mut LocalId) {
        match expr {
            Expr::LocalGet(id) | Expr::LocalSet(id, _) => {
                *max_id = (*max_id).max(*id);
            }
            Expr::Update { id, .. } => {
                *max_id = (*max_id).max(*id);
            }
            Expr::Binary { left, right, .. } | Expr::Logical { left, right, .. } |
            Expr::Compare { left, right, .. } => {
                check_expr(left, max_id);
                check_expr(right, max_id);
            }
            Expr::Unary { operand, .. } => {
                check_expr(operand, max_id);
            }
            Expr::Conditional { condition, then_expr, else_expr } => {
                check_expr(condition, max_id);
                check_expr(then_expr, max_id);
                check_expr(else_expr, max_id);
            }
            Expr::Call { callee, args, .. } => {
                check_expr(callee, max_id);
                for arg in args {
                    check_expr(arg, max_id);
                }
            }
            Expr::Array(elements) => {
                for elem in elements {
                    check_expr(elem, max_id);
                }
            }
            Expr::IndexGet { object, index } | Expr::IndexSet { object, index, .. } => {
                check_expr(object, max_id);
                check_expr(index, max_id);
            }
            Expr::PropertyGet { object, .. } | Expr::PropertySet { object, .. } => {
                check_expr(object, max_id);
            }
            _ => {}
        }
    }

    fn check_stmt(stmt: &Stmt, max_id: &mut LocalId) {
        match stmt {
            Stmt::Let { id, init, .. } => {
                *max_id = (*max_id).max(*id);
                if let Some(expr) = init {
                    check_expr(expr, max_id);
                }
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                check_expr(expr, max_id);
            }
            Stmt::Return(None) => {}
            Stmt::If { condition, then_branch, else_branch } => {
                check_expr(condition, max_id);
                for s in then_branch {
                    check_stmt(s, max_id);
                }
                if let Some(else_b) = else_branch {
                    for s in else_b {
                        check_stmt(s, max_id);
                    }
                }
            }
            Stmt::While { condition, body } => {
                check_expr(condition, max_id);
                for s in body {
                    check_stmt(s, max_id);
                }
            }
            Stmt::For { init, condition, update, body } => {
                if let Some(i) = init {
                    check_stmt(i, max_id);
                }
                if let Some(c) = condition {
                    check_expr(c, max_id);
                }
                if let Some(u) = update {
                    check_expr(u, max_id);
                }
                for s in body {
                    check_stmt(s, max_id);
                }
            }
            Stmt::Try { body, catch, finally } => {
                for s in body {
                    check_stmt(s, max_id);
                }
                if let Some(c) = catch {
                    if let Some((id, _)) = &c.param {
                        *max_id = (*max_id).max(*id);
                    }
                    for s in &c.body {
                        check_stmt(s, max_id);
                    }
                }
                if let Some(f) = finally {
                    for s in f {
                        check_stmt(s, max_id);
                    }
                }
            }
            Stmt::Switch { discriminant, cases } => {
                check_expr(discriminant, max_id);
                for case in cases {
                    if let Some(test) = &case.test {
                        check_expr(test, max_id);
                    }
                    for s in &case.body {
                        check_stmt(s, max_id);
                    }
                }
            }
            Stmt::Break | Stmt::Continue => {}
        }
    }

    for stmt in stmts {
        check_stmt(stmt, &mut max_id);
    }

    max_id
}

/// Inline function and method calls in a list of statements
fn inline_calls_in_stmts(
    stmts: &mut Vec<Stmt>,
    func_candidates: &HashMap<FuncId, Function>,
    method_candidates: &HashMap<(String, String), MethodCandidate>,
    class_names: &HashMap<String, String>,
    local_types: &mut HashMap<LocalId, String>,
    next_local_id: &mut LocalId,
) {
    let mut i = 0;
    while i < stmts.len() {
        // Track local variable types from Let statements
        if let Stmt::Let { id, ty, init, .. } = &stmts[i] {
            if let Type::Named(class_name) = ty {
                local_types.insert(*id, class_name.clone());
            }
            // Also check if init is a New expression
            if let Some(Expr::New { class_name, .. }) = init {
                local_types.insert(*id, class_name.clone());
            }
        }

        let mut new_stmts: Option<Vec<Stmt>> = None;

        match &mut stmts[i] {
            Stmt::Expr(expr) => {
                if let Some((inlined_stmts, _result_expr)) = try_inline_call(expr, func_candidates, method_candidates, local_types, next_local_id) {
                    new_stmts = Some(inlined_stmts);
                } else {
                    inline_calls_in_expr(expr, func_candidates, method_candidates, local_types, next_local_id);
                }
            }
            Stmt::Let { init: Some(expr), .. } => {
                inline_calls_in_expr(expr, func_candidates, method_candidates, local_types, next_local_id);
            }
            Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                inline_calls_in_expr(expr, func_candidates, method_candidates, local_types, next_local_id);
            }
            Stmt::If { condition, then_branch, else_branch } => {
                inline_calls_in_expr(condition, func_candidates, method_candidates, local_types, next_local_id);
                inline_calls_in_stmts(then_branch, func_candidates, method_candidates, class_names, local_types, next_local_id);
                if let Some(else_b) = else_branch {
                    inline_calls_in_stmts(else_b, func_candidates, method_candidates, class_names, local_types, next_local_id);
                }
            }
            Stmt::While { condition, body } => {
                inline_calls_in_expr(condition, func_candidates, method_candidates, local_types, next_local_id);
                inline_calls_in_stmts(body, func_candidates, method_candidates, class_names, local_types, next_local_id);
            }
            Stmt::For { init, condition, update, body } => {
                if let Some(init_stmt) = init {
                    let mut init_stmts = vec![*init_stmt.clone()];
                    inline_calls_in_stmts(&mut init_stmts, func_candidates, method_candidates, class_names, local_types, next_local_id);
                    if init_stmts.len() == 1 {
                        **init_stmt = init_stmts.remove(0);
                    }
                }
                if let Some(cond) = condition {
                    inline_calls_in_expr(cond, func_candidates, method_candidates, local_types, next_local_id);
                }
                if let Some(upd) = update {
                    inline_calls_in_expr(upd, func_candidates, method_candidates, local_types, next_local_id);
                }
                inline_calls_in_stmts(body, func_candidates, method_candidates, class_names, local_types, next_local_id);
            }
            _ => {}
        }

        if let Some(mut inlined) = new_stmts {
            stmts.remove(i);
            let inlined_len = inlined.len();
            for (j, stmt) in inlined.drain(..).enumerate() {
                stmts.insert(i + j, stmt);
            }
            i += inlined_len.max(1);
        } else {
            i += 1;
        }
    }
}

/// Inline function and method calls in an expression
fn inline_calls_in_expr(
    expr: &mut Expr,
    func_candidates: &HashMap<FuncId, Function>,
    method_candidates: &HashMap<(String, String), MethodCandidate>,
    local_types: &HashMap<LocalId, String>,
    next_local_id: &mut LocalId,
) {
    // First try to inline this expression if it's a call
    if let Some((_stmts, mut result)) = try_inline_simple_call(expr, func_candidates, method_candidates, local_types, next_local_id) {
        inline_calls_in_expr(&mut result, func_candidates, method_candidates, local_types, next_local_id);
        *expr = result;
        return;
    }

    // Otherwise recurse into sub-expressions
    match expr {
        Expr::Binary { left, right, .. } | Expr::Logical { left, right, .. } |
        Expr::Compare { left, right, .. } => {
            inline_calls_in_expr(left, func_candidates, method_candidates, local_types, next_local_id);
            inline_calls_in_expr(right, func_candidates, method_candidates, local_types, next_local_id);
        }
        Expr::Unary { operand, .. } => {
            inline_calls_in_expr(operand, func_candidates, method_candidates, local_types, next_local_id);
        }
        Expr::Conditional { condition, then_expr, else_expr } => {
            inline_calls_in_expr(condition, func_candidates, method_candidates, local_types, next_local_id);
            inline_calls_in_expr(then_expr, func_candidates, method_candidates, local_types, next_local_id);
            inline_calls_in_expr(else_expr, func_candidates, method_candidates, local_types, next_local_id);
        }
        Expr::Call { callee, args, .. } => {
            inline_calls_in_expr(callee, func_candidates, method_candidates, local_types, next_local_id);
            for arg in args {
                inline_calls_in_expr(arg, func_candidates, method_candidates, local_types, next_local_id);
            }
        }
        Expr::Array(elements) => {
            for elem in elements {
                inline_calls_in_expr(elem, func_candidates, method_candidates, local_types, next_local_id);
            }
        }
        Expr::IndexGet { object, index } => {
            inline_calls_in_expr(object, func_candidates, method_candidates, local_types, next_local_id);
            inline_calls_in_expr(index, func_candidates, method_candidates, local_types, next_local_id);
        }
        Expr::IndexSet { object, index, value } => {
            inline_calls_in_expr(object, func_candidates, method_candidates, local_types, next_local_id);
            inline_calls_in_expr(index, func_candidates, method_candidates, local_types, next_local_id);
            inline_calls_in_expr(value, func_candidates, method_candidates, local_types, next_local_id);
        }
        Expr::PropertyGet { object, .. } => {
            inline_calls_in_expr(object, func_candidates, method_candidates, local_types, next_local_id);
        }
        Expr::PropertySet { object, value, .. } => {
            inline_calls_in_expr(object, func_candidates, method_candidates, local_types, next_local_id);
            inline_calls_in_expr(value, func_candidates, method_candidates, local_types, next_local_id);
        }
        Expr::LocalSet(_, value) => {
            inline_calls_in_expr(value, func_candidates, method_candidates, local_types, next_local_id);
        }
        _ => {}
    }
}

/// Try to inline a simple function or method call (single return expression)
fn try_inline_simple_call(
    expr: &Expr,
    func_candidates: &HashMap<FuncId, Function>,
    method_candidates: &HashMap<(String, String), MethodCandidate>,
    local_types: &HashMap<LocalId, String>,
    next_local_id: &mut LocalId,
) -> Option<(Vec<Stmt>, Expr)> {
    if let Expr::Call { callee, args, .. } = expr {
        // Check for regular function call
        if let Expr::FuncRef(func_id) = callee.as_ref() {
            if let Some(func) = func_candidates.get(func_id) {
                if func.body.len() == 1 {
                    if let Stmt::Return(Some(return_expr)) = &func.body[0] {
                        let mut param_map: HashMap<LocalId, Expr> = HashMap::new();
                        for (param, arg) in func.params.iter().zip(args.iter()) {
                            param_map.insert(param.id, arg.clone());
                        }
                        let mut result = return_expr.clone();
                        substitute_locals(&mut result, &param_map, next_local_id);
                        return Some((vec![], result));
                    }
                }
            }
        }

        // Check for method call: callee is PropertyGet { object: LocalGet(id), property: method_name }
        if let Expr::PropertyGet { object, property: method_name } = callee.as_ref() {
            if let Expr::LocalGet(obj_id) = object.as_ref() {
                // Look up the class type of this local variable
                if let Some(class_name) = local_types.get(obj_id) {
                    // Look up the method candidate
                    if let Some(method_candidate) = method_candidates.get(&(class_name.clone(), method_name.clone())) {
                        // Check for single return statement
                        if method_candidate.func.body.len() == 1 {
                            if let Stmt::Return(Some(return_expr)) = &method_candidate.func.body[0] {
                                let mut param_map: HashMap<LocalId, Expr> = HashMap::new();

                                // Map 'this' parameter to the receiver object
                                if let Some(this_id) = method_candidate.this_param_id {
                                    param_map.insert(this_id, Expr::LocalGet(*obj_id));
                                }

                                // Map parameters to arguments
                                // Note: Method params don't include 'this' - they use Expr::This instead
                                for (param, arg) in method_candidate.func.params.iter().zip(args.iter()) {
                                    param_map.insert(param.id, arg.clone());
                                }

                                let mut result = return_expr.clone();
                                substitute_locals(&mut result, &param_map, next_local_id);

                                // Also substitute Expr::This with the receiver
                                substitute_this(&mut result, *obj_id);

                                return Some((vec![], result));
                            }
                        }

                        // Handle void methods (no return or empty return)
                        if method_candidate.func.body.len() <= 2 {
                            let mut is_void_method = true;
                            let mut inlined_stmts = Vec::new();

                            for stmt in &method_candidate.func.body {
                                match stmt {
                                    Stmt::Return(None) => {}
                                    Stmt::Expr(e) => {
                                        let mut param_map: HashMap<LocalId, Expr> = HashMap::new();
                                        if let Some(this_id) = method_candidate.this_param_id {
                                            param_map.insert(this_id, Expr::LocalGet(*obj_id));
                                        }
                                        // Note: Method params don't include 'this' - they use Expr::This instead
                                        for (param, arg) in method_candidate.func.params.iter().zip(args.iter()) {
                                            param_map.insert(param.id, arg.clone());
                                        }
                                        let mut expr = e.clone();
                                        substitute_locals(&mut expr, &param_map, next_local_id);
                                        substitute_this(&mut expr, *obj_id);
                                        inlined_stmts.push(Stmt::Expr(expr));
                                    }
                                    _ => {
                                        is_void_method = false;
                                        break;
                                    }
                                }
                            }

                            if is_void_method && !inlined_stmts.is_empty() {
                                return Some((inlined_stmts, Expr::Undefined));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Try to inline a call that may have multiple statements
fn try_inline_call(
    expr: &Expr,
    func_candidates: &HashMap<FuncId, Function>,
    method_candidates: &HashMap<(String, String), MethodCandidate>,
    local_types: &HashMap<LocalId, String>,
    next_local_id: &mut LocalId,
) -> Option<(Vec<Stmt>, Option<Expr>)> {
    if let Expr::Call { callee, args, .. } = expr {
        // Handle regular function calls
        if let Expr::FuncRef(func_id) = callee.as_ref() {
            if let Some(func) = func_candidates.get(func_id) {
                let mut setup_stmts: Vec<Stmt> = Vec::new();
                let mut param_map: HashMap<LocalId, Expr> = HashMap::new();

                for (param, arg) in func.params.iter().zip(args.iter()) {
                    if is_trivial_expr(arg) {
                        param_map.insert(param.id, arg.clone());
                    } else {
                        let local_id = *next_local_id;
                        *next_local_id += 1;

                        setup_stmts.push(Stmt::Let {
                            id: local_id,
                            name: param.name.clone(),
                            ty: param.ty.clone(),
                            mutable: false,
                            init: Some(arg.clone()),
                        });

                        param_map.insert(param.id, Expr::LocalGet(local_id));
                    }
                }

                let mut inlined_body = func.body.clone();
                substitute_locals_in_stmts(&mut inlined_body, &param_map, next_local_id);

                setup_stmts.extend(inlined_body);

                return Some((setup_stmts, None));
            }
        }

        // Handle method calls
        if let Expr::PropertyGet { object, property: method_name } = callee.as_ref() {
            if let Expr::LocalGet(obj_id) = object.as_ref() {
                if let Some(class_name) = local_types.get(obj_id) {
                    if let Some(method_candidate) = method_candidates.get(&(class_name.clone(), method_name.clone())) {
                        let mut setup_stmts: Vec<Stmt> = Vec::new();
                        let mut param_map: HashMap<LocalId, Expr> = HashMap::new();

                        // Map 'this' parameter to the receiver object (if present as a param)
                        if let Some(this_id) = method_candidate.this_param_id {
                            param_map.insert(this_id, Expr::LocalGet(*obj_id));
                        }

                        // Map parameters to arguments
                        // Note: Method params don't include 'this' - they use Expr::This instead
                        for (param, arg) in method_candidate.func.params.iter().zip(args.iter()) {
                            if is_trivial_expr(arg) {
                                param_map.insert(param.id, arg.clone());
                            } else {
                                let local_id = *next_local_id;
                                *next_local_id += 1;

                                setup_stmts.push(Stmt::Let {
                                    id: local_id,
                                    name: param.name.clone(),
                                    ty: param.ty.clone(),
                                    mutable: false,
                                    init: Some(arg.clone()),
                                });

                                param_map.insert(param.id, Expr::LocalGet(local_id));
                            }
                        }

                        // Clone and substitute the method body
                        let mut inlined_body = method_candidate.func.body.clone();
                        substitute_locals_in_stmts(&mut inlined_body, &param_map, next_local_id);
                        substitute_this_in_stmts(&mut inlined_body, *obj_id);

                        setup_stmts.extend(inlined_body);

                        return Some((setup_stmts, None));
                    }
                }
            }
        }
    }
    None
}

/// Check if an expression is trivial (safe to duplicate)
fn is_trivial_expr(expr: &Expr) -> bool {
    matches!(expr,
        Expr::Integer(_) | Expr::Number(_) | Expr::Bool(_) |
        Expr::String(_) | Expr::Null | Expr::Undefined |
        Expr::LocalGet(_) | Expr::GlobalGet(_)
    )
}

/// Substitute local variable references in an expression
fn substitute_locals(expr: &mut Expr, param_map: &HashMap<LocalId, Expr>, next_local_id: &mut LocalId) {
    match expr {
        Expr::LocalGet(id) => {
            if let Some(replacement) = param_map.get(id) {
                *expr = replacement.clone();
            }
        }
        Expr::LocalSet(id, value) => {
            substitute_locals(value, param_map, next_local_id);
            if let Some(replacement) = param_map.get(id) {
                if let Expr::LocalGet(new_id) = replacement {
                    *id = *new_id;
                }
            }
        }
        Expr::Update { id, .. } => {
            if let Some(Expr::LocalGet(new_id)) = param_map.get(id) {
                *id = *new_id;
            }
        }
        Expr::Binary { left, right, .. } | Expr::Logical { left, right, .. } |
        Expr::Compare { left, right, .. } => {
            substitute_locals(left, param_map, next_local_id);
            substitute_locals(right, param_map, next_local_id);
        }
        Expr::Unary { operand, .. } => {
            substitute_locals(operand, param_map, next_local_id);
        }
        Expr::Conditional { condition, then_expr, else_expr } => {
            substitute_locals(condition, param_map, next_local_id);
            substitute_locals(then_expr, param_map, next_local_id);
            substitute_locals(else_expr, param_map, next_local_id);
        }
        Expr::Call { callee, args, .. } => {
            substitute_locals(callee, param_map, next_local_id);
            for arg in args {
                substitute_locals(arg, param_map, next_local_id);
            }
        }
        Expr::Array(elements) => {
            for elem in elements {
                substitute_locals(elem, param_map, next_local_id);
            }
        }
        Expr::IndexGet { object, index } => {
            substitute_locals(object, param_map, next_local_id);
            substitute_locals(index, param_map, next_local_id);
        }
        Expr::IndexSet { object, index, value } => {
            substitute_locals(object, param_map, next_local_id);
            substitute_locals(index, param_map, next_local_id);
            substitute_locals(value, param_map, next_local_id);
        }
        Expr::PropertyGet { object, .. } => {
            substitute_locals(object, param_map, next_local_id);
        }
        Expr::PropertySet { object, value, .. } => {
            substitute_locals(object, param_map, next_local_id);
            substitute_locals(value, param_map, next_local_id);
        }
        Expr::TypeOf(inner) => {
            substitute_locals(inner, param_map, next_local_id);
        }
        _ => {}
    }
}

/// Substitute Expr::This with a LocalGet reference
fn substitute_this(expr: &mut Expr, obj_id: LocalId) {
    match expr {
        Expr::This => {
            *expr = Expr::LocalGet(obj_id);
        }
        Expr::PropertyGet { object, .. } => {
            substitute_this(object, obj_id);
        }
        Expr::PropertySet { object, value, .. } => {
            substitute_this(object, obj_id);
            substitute_this(value, obj_id);
        }
        Expr::Binary { left, right, .. } | Expr::Logical { left, right, .. } |
        Expr::Compare { left, right, .. } => {
            substitute_this(left, obj_id);
            substitute_this(right, obj_id);
        }
        Expr::Unary { operand, .. } => {
            substitute_this(operand, obj_id);
        }
        Expr::Conditional { condition, then_expr, else_expr } => {
            substitute_this(condition, obj_id);
            substitute_this(then_expr, obj_id);
            substitute_this(else_expr, obj_id);
        }
        Expr::Call { callee, args, .. } => {
            substitute_this(callee, obj_id);
            for arg in args {
                substitute_this(arg, obj_id);
            }
        }
        Expr::Array(elements) => {
            for elem in elements {
                substitute_this(elem, obj_id);
            }
        }
        Expr::IndexGet { object, index } => {
            substitute_this(object, obj_id);
            substitute_this(index, obj_id);
        }
        Expr::IndexSet { object, index, value } => {
            substitute_this(object, obj_id);
            substitute_this(index, obj_id);
            substitute_this(value, obj_id);
        }
        Expr::LocalSet(_, value) => {
            substitute_this(value, obj_id);
        }
        Expr::TypeOf(inner) => {
            substitute_this(inner, obj_id);
        }
        _ => {}
    }
}

/// Substitute Expr::This with a LocalGet reference in statements
fn substitute_this_in_stmts(stmts: &mut Vec<Stmt>, obj_id: LocalId) {
    for stmt in stmts.iter_mut() {
        match stmt {
            Stmt::Let { init: Some(expr), .. } => {
                substitute_this(expr, obj_id);
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                substitute_this(expr, obj_id);
            }
            Stmt::If { condition, then_branch, else_branch } => {
                substitute_this(condition, obj_id);
                substitute_this_in_stmts(then_branch, obj_id);
                if let Some(else_b) = else_branch {
                    substitute_this_in_stmts(else_b, obj_id);
                }
            }
            Stmt::While { condition, body } => {
                substitute_this(condition, obj_id);
                substitute_this_in_stmts(body, obj_id);
            }
            Stmt::For { init, condition, update, body } => {
                if let Some(init_stmt) = init {
                    let mut init_vec = vec![*init_stmt.clone()];
                    substitute_this_in_stmts(&mut init_vec, obj_id);
                    if init_vec.len() == 1 {
                        **init_stmt = init_vec.remove(0);
                    }
                }
                if let Some(cond) = condition {
                    substitute_this(cond, obj_id);
                }
                if let Some(upd) = update {
                    substitute_this(upd, obj_id);
                }
                substitute_this_in_stmts(body, obj_id);
            }
            _ => {}
        }
    }
}

/// Substitute local variable references in statements
fn substitute_locals_in_stmts(stmts: &mut Vec<Stmt>, param_map: &HashMap<LocalId, Expr>, next_local_id: &mut LocalId) {
    for stmt in stmts.iter_mut() {
        match stmt {
            Stmt::Let { init: Some(expr), .. } => {
                substitute_locals(expr, param_map, next_local_id);
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                substitute_locals(expr, param_map, next_local_id);
            }
            Stmt::If { condition, then_branch, else_branch } => {
                substitute_locals(condition, param_map, next_local_id);
                substitute_locals_in_stmts(then_branch, param_map, next_local_id);
                if let Some(else_b) = else_branch {
                    substitute_locals_in_stmts(else_b, param_map, next_local_id);
                }
            }
            Stmt::While { condition, body } => {
                substitute_locals(condition, param_map, next_local_id);
                substitute_locals_in_stmts(body, param_map, next_local_id);
            }
            Stmt::For { init, condition, update, body } => {
                if let Some(init_stmt) = init {
                    let mut init_vec = vec![*init_stmt.clone()];
                    substitute_locals_in_stmts(&mut init_vec, param_map, next_local_id);
                    if init_vec.len() == 1 {
                        **init_stmt = init_vec.remove(0);
                    }
                }
                if let Some(cond) = condition {
                    substitute_locals(cond, param_map, next_local_id);
                }
                if let Some(upd) = update {
                    substitute_locals(upd, param_map, next_local_id);
                }
                substitute_locals_in_stmts(body, param_map, next_local_id);
            }
            _ => {}
        }
    }
}
