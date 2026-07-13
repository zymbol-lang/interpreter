//! Function and lambda evaluation for Zymbol-Lang
//!
//! Handles runtime evaluation of:
//! - Lambda expressions: x -> expr or (x, y) -> { block }
//! - Lambda calls: Closure execution with captured environment
//! - Function calls: Traditional functions, module functions
//! - Parameter types: Normal, Mutable (~), Output (<~)

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use zymbol_ast::{Expr, ParameterKind};
use zymbol_span::Span;
use crate::{ControlFlow, FunctionDef, FunctionValue, Interpreter, Result, RuntimeError, Value};
use std::io::Write;

impl<W: Write> Interpreter<W> {
    /// Evaluate lambda expression: creates a closure.
    /// Only captures variables that are actually referenced in the lambda body.
    pub(crate) fn eval_lambda(&mut self, lambda: &zymbol_ast::LambdaExpr) -> Result<Value> {
        let mut refs = HashSet::new();
        let excluded: HashSet<&str> = lambda.params.iter().map(|s| s.as_str()).collect();
        collect_refs_in_body(&lambda.body, &excluded, &mut refs);

        let captures = self.capture_only(&refs);

        Ok(Value::Function(FunctionValue {
            params: lambda.params.clone(),
            body: lambda.body.clone(),
            captures: Rc::new(captures),
            is_named_fn: false,
            module_aliases: std::collections::HashMap::new(),
        }))
    }

    /// Capture only the variables in `names` from the current scope stack.
    fn capture_only(&self, names: &HashSet<String>) -> HashMap<String, Value> {
        if names.is_empty() {
            return HashMap::new();
        }
        let mut captures = HashMap::with_capacity(names.len());
        // Walk from inner to outer scope so inner bindings shadow outer ones
        for scope in self.scope_stack.iter().rev() {
            for name in names {
                if !captures.contains_key(name) {
                    if let Some(val) = scope.get(name) {
                        captures.insert(name.clone(), val.clone());
                    }
                }
            }
        }
        captures
    }

    /// Call a lambda function with given arguments
    pub(crate) fn eval_lambda_call(
        &mut self,
        func: FunctionValue,
        mut arg_values: Vec<Value>,
        span: &Span,
    ) -> Result<Value> {
        // Validate argument count
        if arg_values.len() != func.params.len() {
            return Err(RuntimeError::Generic {
                message: format!(
                    "lambda expects {} arguments, got {}",
                    func.params.len(),
                    arg_values.len()
                ),
                span: *span,
            });
        }

        // HOF fast path: expression lambda with no captures.
        // Expression bodies cannot contain assignment statements, so they cannot
        // write to any outer scope. A simple push_scope/pop_scope is sufficient —
        // no need to hide the caller's scope with the expensive take_call_state cycle.
        if func.captures.is_empty() {
            if let zymbol_ast::LambdaBody::Expr(ref expr) = func.body {
                self.push_scope();
                for (i, param) in func.params.iter().enumerate() {
                    let value = std::mem::replace(&mut arg_values[i], Value::Unit);
                    self.set_variable_new(param, value);
                }
                // Pop the scope on the error path too, or the lambda's params
                // leak into the caller's scope view (L16 family).
                let result = self.eval_expr(expr);
                self.pop_scope();
                return result;
            }
        }

        // B2: zero-copy save + fresh isolated scope (see take_call_state)
        let saved = self.take_call_state();

        // G7 fix: named functions carry module_aliases captured at definition time.
        // take_call_state() clears import_aliases; restore from the function's own
        // snapshot so that alias::fn calls work regardless of call depth.
        if func.is_named_fn && !func.module_aliases.is_empty() {
            self.import_aliases = func.module_aliases.clone();
        }

        // Restore closure captures into the fresh scope (before binding params,
        // so params shadow captures with the same name).
        // Borrow the Rc — clone individual values (O(1) when captures is empty).
        for (name, value) in func.captures.as_ref() {
            self.set_variable(name, value.clone());
        }

        // QW8: move values out of arg_values instead of cloning
        for (i, param) in func.params.iter().enumerate() {
            let value = std::mem::replace(&mut arg_values[i], Value::Unit);
            self.set_variable(param, value);
        }

        // QW1: execute_block_no_scope avoids the extra push_scope/pop_scope that
        // execute_block would add — take_call_state already created scope[0].
        // L16 fix: compute the result WITHOUT early `?` returns — the caller's
        // scope_stack was swapped out by take_call_state, so every exit path
        // (including errors) must run restore_call_state or the caller's
        // variables vanish after a caught error.
        let is_named = func.is_named_fn;
        let result: Result<Value> = match &func.body {
            zymbol_ast::LambdaBody::Expr(expr) => self.eval_expr(expr),
            zymbol_ast::LambdaBody::Block(block) => match self.execute_block_no_scope(block) {
                Err(e) => Err(e),
                Ok(()) => match std::mem::replace(&mut self.control_flow, ControlFlow::None) {
                    ControlFlow::Return(val) => {
                        self.has_control_flow = false;
                        Ok(val.unwrap_or(Value::Unit))
                    }
                    _ => {
                        if is_named {
                            Ok(Value::Unit)
                        } else {
                            Err(RuntimeError::Generic {
                                message: "block lambda must use <~ to return value".to_string(),
                                span: *span,
                            })
                        }
                    }
                },
            },
        };

        self.restore_call_state(saved);
        result
    }

    /// Evaluate a function call
    pub(crate) fn eval_function_call(&mut self, call: &zymbol_ast::FunctionCallExpr) -> Result<Value> {
        // Determine what we're calling based on the callable expression
        match call.callable.unwrap_group() {
            // Simple identifier: could be lambda variable or traditional function
            Expr::Identifier(ident) => {
                // Check if it's a lambda stored in a variable
                if let Some(Value::Function(func)) = self.get_variable(&ident.name).cloned() {
                    let mut arg_values = Vec::with_capacity(call.arguments.len());
                    for arg in &call.arguments {
                        arg_values.push(self.eval_expr(arg)?);
                    }
                    return self.eval_lambda_call(func, arg_values, &call.span);
                }

                // Not a lambda variable - look up as traditional function
                let func_def = self.functions.get(&ident.name).cloned().ok_or_else(|| {
                    RuntimeError::Generic {
                        message: format!("undefined function: '{}'", ident.name),
                        span: call.span,
                    }
                })?;

                self.eval_traditional_function_call(func_def, &call.arguments, &call.span, None, Some(&ident.name))
            }

            // Member access: could be module::function or object.method (only module supported)
            Expr::MemberAccess(member) => {
                // Check if it's a module function call: module.function
                if let Expr::Identifier(module_ident) = member.object.unwrap_group() {
                    let module_alias = &module_ident.name;
                    let func_name = &member.field;

                    let module_path = self.import_aliases.get(module_alias).ok_or_else(|| {
                        RuntimeError::Generic {
                            message: format!("undefined module alias: '{}'", module_alias),
                            span: call.span,
                        }
                    })?;

                    let module = self.loaded_modules.get(module_path).ok_or_else(|| {
                        RuntimeError::Generic {
                            message: format!("module '{}' not loaded", module_alias),
                            span: call.span,
                        }
                    })?;

                    let func_def = module.functions.get(func_name).cloned().ok_or_else(|| {
                        RuntimeError::FunctionNotExported {
                            module: module_alias.clone(),
                            function: func_name.clone(),
                        }
                    })?;

                    return self.eval_traditional_function_call(func_def, &call.arguments, &call.span, Some((module_alias.clone(), module_path.clone())), None);
                }

                // Not a module function - error
                Err(RuntimeError::Generic {
                    message: "member function calls not supported".to_string(),
                    span: call.span,
                })
            }

            // Any other expression: evaluate it and expect a Value::Function
            _ => {
                let callable_value = self.eval_expr(&call.callable)?;

                match callable_value {
                    Value::Function(func) => {
                        let mut arg_values = Vec::with_capacity(call.arguments.len());
                        for arg in &call.arguments {
                            arg_values.push(self.eval_expr(arg)?);
                        }
                        self.eval_lambda_call(func, arg_values, &call.span)
                    }
                    _ => {
                        Err(RuntimeError::Generic {
                            message: "expression is not callable".to_string(),
                            span: call.span,
                        })
                    }
                }
            }
        }
    }

    /// Convert a named FunctionDef into a first-class FunctionValue.
    /// Selectively captures only the variables referenced in the function body
    /// from the current scope, so the result behaves like a closure.
    pub(crate) fn func_def_to_value(&self, func_def: &Rc<FunctionDef>) -> FunctionValue {
        let FunctionDef::Zymbol { parameters, body, .. } = func_def.as_ref() else {
            // Native functions cannot be used as first-class values in v0.0.6.
            return FunctionValue {
                params: vec![],
                body: zymbol_ast::LambdaBody::Block(
                    zymbol_ast::Block::new(vec![], zymbol_span::Span::new(
                        zymbol_span::Position::start(),
                        zymbol_span::Position::start(),
                        zymbol_span::FileId(0),
                    ))
                ),
                captures: Rc::new(std::collections::HashMap::new()),
                is_named_fn: false,
                module_aliases: std::collections::HashMap::new(),
            };
        };
        let mut refs = HashSet::new();
        let mut locals: HashSet<String> = parameters.iter().map(|p| p.name.clone()).collect();
        collect_refs_in_stmts(&body.statements, &mut locals, &mut refs);
        let captures = self.capture_only(&refs);
        FunctionValue {
            params: parameters.iter().map(|p| p.name.clone()).collect(),
            body: zymbol_ast::LambdaBody::Block(body.clone()),
            captures: Rc::new(captures),
            is_named_fn: true,
            module_aliases: self.import_aliases.clone(),
        }
    }

    /// Helper to evaluate traditional (non-lambda) function calls
    pub(crate) fn eval_traditional_function_call(
        &mut self,
        func_def: Rc<FunctionDef>,
        arguments: &[zymbol_ast::Expr],
        span: &Span,
        module_info: Option<(String, std::path::PathBuf)>,
        func_name: Option<&str>,
    ) -> Result<Value> {
        // Native dispatch: evaluate args and invoke the function pointer directly.
        // No scope setup needed — native functions are pure Rust, isolated from interpreter state.
        if let FunctionDef::Native { arity, func, .. } = func_def.as_ref() {
            let expected = *arity;
            let got = arguments.len() as i8;
            if expected >= 0 && got != expected {
                return Err(RuntimeError::Generic {
                    message: format!(
                        "function expects {} argument(s), got {}",
                        expected, got
                    ),
                    span: *span,
                });
            }
            let mut arg_values = Vec::with_capacity(arguments.len());
            for arg in arguments {
                arg_values.push(self.eval_expr(arg)?);
            }
            return func(arg_values, *span);
        }

        // Zymbol function: destructure the enum — unreachable arm is dead code after the guard above.
        let FunctionDef::Zymbol { parameters, body, origin_module_path, auto_free } = func_def.as_ref() else {
            unreachable!()
        };

        // Check parameter count
        if arguments.len() != parameters.len() {
            return Err(RuntimeError::Generic {
                message: format!(
                    "function expects {} arguments, got {}",
                    parameters.len(),
                    arguments.len()
                ),
                span: *span,
            });
        }

        // QW9: reuse pooled Vec to avoid per-call heap allocation
        let mut arg_values = self.arg_vec_pool.pop().unwrap_or_else(|| Vec::with_capacity(4));
        arg_values.clear();
        if arg_values.capacity() < arguments.len() {
            arg_values.reserve(arguments.len() - arg_values.capacity());
        }
        for arg in arguments {
            arg_values.push(self.eval_expr(arg)?);
        }

        // B2: zero-copy save + fresh isolated scope (see take_call_state)
        let saved = self.take_call_state();

        // B4: pre-alloc scope capacity to avoid rehashing on parameter binding
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.reserve(parameters.len());
        }

        // Determine the module whose state this frame executes against (MM-2):
        // - alias:: calls carry module_info (re-export adapters redirect via
        //   origin_module_path — BUG-001);
        // - bare-name intra-module calls (private helpers, exported siblings)
        //   carry only origin_module_path.
        // Main-script functions have origin_module_path = Some(main.zy), which is
        // NOT in loaded_modules, so they resolve to None (script path below).
        let module_ctx_path: Option<std::path::PathBuf> = if let Some((_, module_path)) = &module_info {
            Some(origin_module_path.as_ref().unwrap_or(module_path).clone())
        } else {
            origin_module_path
                .as_ref()
                .filter(|p| self.loaded_modules.contains_key(p.as_path()))
                .cloned()
        };

        // Snapshot of the module values injected into this frame. The write-back
        // below diffs against it, so keys this frame never modified cannot
        // clobber changes written back by nested calls (MM-2).
        let mut injected_module_vars: HashMap<String, Value> = HashMap::new();

        // If this is a module function call, restore the module's execution context.
        // BUG-01: alias:: calls also swap self.functions with the module's full
        // function table so intra-module calls resolve correctly inside the body.
        // G17 fix: script-level functions inherit the caller's import_aliases so
        // module calls (ollama::fn, ui::fn, etc.) resolve correctly.
        let saved_functions = if let Some(ctx_path) = &module_ctx_path {
            if let Some(module) = self.loaded_modules.get(ctx_path).cloned() {
                // MM-2: on a same-module nested call the caller's frame holds
                // fresher values than the store (its own write-back has not run
                // yet) — inject the caller's live copies instead of stale ones.
                let same_module_caller =
                    saved.current_module_path.as_deref() == Some(ctx_path.as_path());
                for (name, value) in &module.all_variables {
                    let live = if same_module_caller {
                        lookup_in_scopes(&saved.scope_stack, name)
                            .cloned()
                            .unwrap_or_else(|| value.clone())
                    } else {
                        value.clone()
                    };
                    injected_module_vars.insert(name.clone(), live.clone());
                    self.set_variable(name, live);
                }
                // MM-4 runtime guard: module constants stay immutable inside
                // module function bodies even when static analysis was skipped.
                for const_name in &module.const_names {
                    self.mark_const(const_name.clone());
                }
                self.import_aliases = module.import_aliases.clone();
                self.current_module_path = Some(ctx_path.clone());
                if module_info.is_some() {
                    // Swap in the module's complete function table; save caller's table
                    Some(std::mem::replace(&mut self.functions, module.all_functions.clone()))
                } else {
                    // Intra-module call: table already swapped by the outer alias:: call
                    None
                }
            } else {
                None
            }
        } else {
            // Script-level function call (no module context).
            // Inherit caller's import aliases so module calls (alias::fn()) resolve correctly.
            self.import_aliases = saved.import_aliases.clone();
            // Forward constants (:=) visible in the caller's frame into the fresh
            // isolated scope, and re-mark them (MM-9) so the next frame in the
            // chain forwards them again. Root-scope constants additionally resolve
            // through global_consts at any depth; this loop covers block-local
            // constants declared before the call site.
            for (scope, const_set) in saved.scope_stack.iter().zip(saved.const_vars_stack.iter()) {
                for const_name in const_set {
                    if let Some(value) = scope.get(const_name) {
                        self.set_variable(const_name, value.clone());
                        self.mark_const(const_name.clone());
                    }
                }
            }
            None
        };

        // QW8: move values out of arg_values instead of cloning
        // set_variable_new: skip scope-stack scan for Normal params (fresh isolated scope)
        for (i, param) in parameters.iter().enumerate() {
            let arg_value = std::mem::replace(&mut arg_values[i], Value::Unit);
            // A parameter may shadow a forwarded constant of the same name —
            // it must stay assignable inside the body (MM-9).
            self.unmark_const(&param.name);
            match param.kind {
                ParameterKind::Normal => {
                    self.set_variable_new(&param.name, arg_value);
                }
                ParameterKind::Mutable => {
                    self.mark_mutable(param.name.clone());
                    self.set_variable_new(&param.name, arg_value);
                }
                ParameterKind::Output => {
                    self.mark_mutable(param.name.clone());
                    self.set_variable_new(&param.name, arg_value);
                }
            }
        }

        // QW9: return the drained Vec to the pool (QW8 left Value::Unit sentinels)
        arg_values.clear();
        if self.arg_vec_pool.len() < 32 { self.arg_vec_pool.push(arg_values); }

        // QW17: set current_function so Statement::Return can detect tail calls
        let prev_fn = self.current_function.take();
        self.current_function = func_name.map(|s| s.to_string());

        // QW13 fix: track output param names so MoveOrClone skips take_variable for them
        let prev_output_params = std::mem::take(&mut self.current_output_params);
        for param in parameters {
            if matches!(param.kind, ParameterKind::Output) {
                self.current_output_params.insert(param.name.clone());
            }
        }

        // QW1: execute_block_no_scope — take_call_state already owns scope[0] (params).
        // QW17: TCO loop — if tco_pending is set after execution, rebind params and restart.
        let return_value = 'tco: loop {
            if let Err(e) = self.execute_body_scheduled(body, auto_free) {
                // L16 fix: the caller's scope_stack was swapped out by
                // take_call_state — restore the full caller state before
                // propagating, or every outer variable vanishes after the
                // error is caught by an enclosing `!?`.
                self.current_function = prev_fn;
                self.current_output_params = prev_output_params;
                self.restore_call_state(saved);
                if let Some(caller_functions) = saved_functions {
                    self.functions = caller_functions;
                }
                return Err(e);
            }

            if self.tco_pending {
                self.tco_pending = false;
                // Rebind parameters to tco_args
                // Move tco_args into params (no clone — QW8 semantics for TCO)
                let tco_args = std::mem::take(&mut self.tco_args);
                for (param, val) in parameters.iter().zip(tco_args) {
                    self.set_variable(&param.name, val);
                }
                // Clear the Return control flow set by the TCO trigger
                self.clear_control_flow();
                continue 'tco;
            }

            // Extract return value
            break 'tco match std::mem::replace(&mut self.control_flow, ControlFlow::None) {
                ControlFlow::Return(val) => {
                    self.has_control_flow = false;
                    val.unwrap_or(Value::Unit)
                }
                _ => Value::Unit,
            };
        };

        self.current_function = prev_fn;
        self.current_output_params = prev_output_params;

        // MODULE STATE WRITE-BACK (MM-2): persist module-level mutations back to the
        // LoadedModule store. Runs for every module frame — alias:: calls and
        // bare-name intra-module calls alike. Only keys whose value actually
        // changed relative to the injected snapshot are written, so an outer
        // frame that never touched a key cannot clobber a nested call's
        // write-back with its stale copy. Parameters are excluded: a parameter
        // named like a module variable shadows it and must not corrupt state.
        // This implements private mutable module state: variables declared with
        // `=` at module level persist across calls but are never directly
        // accessible from outside the module.
        let mut module_state_updates: Vec<(String, Value)> = Vec::new();
        if let Some(ctx_path) = &module_ctx_path {
            for (key, injected_val) in &injected_module_vars {
                if parameters.iter().any(|p| p.name == *key) {
                    continue;
                }
                // Missing key: moved out by a `<~ key` return — reads don't mutate.
                if let Some(current) = self.get_variable(key) {
                    if current != injected_val {
                        module_state_updates.push((key.clone(), current.clone()));
                    }
                }
            }
            if !module_state_updates.is_empty() {
                if let Some(module) = self.loaded_modules.get_mut(ctx_path) {
                    for (key, val) in &module_state_updates {
                        module.all_variables.insert(key.clone(), val.clone());
                    }
                }
            }
        }

        // QW2: lazy output-param collection — only allocate if function has output params.
        // Eliminates HashMap::new() on every call (most functions have no output params).
        let has_output_params = parameters.iter().any(|p| matches!(p.kind, ParameterKind::Output));
        if has_output_params {
            let mut updates = Vec::new();
            for (i, param) in parameters.iter().enumerate() {
                if matches!(param.kind, ParameterKind::Output) {
                    if let Expr::Identifier(ident) = arguments[i].unwrap_group() {
                        let value = self.get_variable(&param.name).cloned().unwrap_or(Value::Unit);
                        updates.push((ident.name.clone(), value));
                    }
                }
            }
            self.restore_call_state(saved);
            for (name, value) in updates {
                self.set_variable(&name, value);
            }
        } else {
            self.restore_call_state(saved);
        }

        // MM-2: a same-module caller still holds the pre-call copies of the keys
        // this frame just wrote back — refresh them so subsequent reads in the
        // caller observe the new module state.
        if !module_state_updates.is_empty()
            && self.current_module_path.as_deref() == module_ctx_path.as_deref()
        {
            for (key, val) in module_state_updates {
                self.set_variable(&key, val);
            }
        }

        // BUG-01: restore caller's function table after module function execution
        if let Some(caller_functions) = saved_functions {
            self.functions = caller_functions;
        }

        Ok(return_value)
    }
}

/// Look up a name in a saved scope stack, innermost scope first.
fn lookup_in_scopes<'a>(scopes: &'a [HashMap<String, Value>], name: &str) -> Option<&'a Value> {
    scopes.iter().rev().find_map(|scope| scope.get(name))
}

// ── Free-variable collection for efficient closure capture ────────────────────
// Walks the lambda body AST and collects all identifier names that are not
// lambda parameters (locals). Only these names need to be captured from scope.

fn collect_refs_in_body(
    body: &zymbol_ast::LambdaBody,
    locals: &HashSet<&str>,
    refs: &mut HashSet<String>,
) {
    match body {
        zymbol_ast::LambdaBody::Expr(e) => collect_refs_in_expr(e, locals, refs),
        zymbol_ast::LambdaBody::Block(block) => {
            let mut block_locals = locals.iter().map(|s| s.to_string()).collect::<HashSet<_>>();
            collect_refs_in_stmts(&block.statements, &mut block_locals, refs);
        }
    }
}

fn collect_refs_in_expr(
    expr: &Expr,
    locals: &HashSet<&str>,
    refs: &mut HashSet<String>,
) {
    match expr {
        // Grouping parens are transparent
        Expr::Group(group) => collect_refs_in_expr(&group.expr, locals, refs),
        Expr::Identifier(id) => {
            if !locals.contains(id.name.as_str()) {
                refs.insert(id.name.clone());
            }
        }
        Expr::Binary(b) => {
            collect_refs_in_expr(&b.left, locals, refs);
            collect_refs_in_expr(&b.right, locals, refs);
        }
        Expr::Unary(u) => collect_refs_in_expr(&u.operand, locals, refs),
        Expr::FunctionCall(call) => {
            collect_refs_in_expr(&call.callable, locals, refs);
            for arg in &call.arguments { collect_refs_in_expr(arg, locals, refs); }
        }
        Expr::ArrayLiteral(arr) => {
            for e in &arr.elements { collect_refs_in_expr(e, locals, refs); }
        }
        Expr::Tuple(t) => {
            for e in &t.elements { collect_refs_in_expr(e, locals, refs); }
        }
        Expr::NamedTuple(nt) => {
            for (_, v) in &nt.fields { collect_refs_in_expr(v, locals, refs); }
        }
        Expr::MemberAccess(m) => collect_refs_in_expr(&m.object, locals, refs),
        Expr::Index(idx) => {
            collect_refs_in_expr(&idx.array, locals, refs);
            collect_refs_in_expr(&idx.index, locals, refs);
        }
        Expr::Range(r) => {
            collect_refs_in_expr(&r.start, locals, refs);
            collect_refs_in_expr(&r.end, locals, refs);
            if let Some(s) = &r.step { collect_refs_in_expr(s, locals, refs); }
        }
        Expr::Match(m) => {
            collect_refs_in_expr(&m.scrutinee, locals, refs);
            for case in &m.cases {
                if let Some(v) = &case.value { collect_refs_in_expr(v, locals, refs); }
                if let Some(block) = &case.block {
                    let mut bl = locals.iter().map(|s| s.to_string()).collect::<HashSet<_>>();
                    collect_refs_in_stmts(&block.statements, &mut bl, refs);
                }
            }
        }
        Expr::Lambda(lam) => {
            // Nested lambda: its params shadow the current locals
            let mut inner_locals = locals.clone();
            let owned: Vec<String> = lam.params.clone();
            for p in &owned { inner_locals.insert(p.as_str()); }
            collect_refs_in_body(&lam.body, &inner_locals, refs);
        }
        Expr::CollectionLength(op) => collect_refs_in_expr(&op.collection, locals, refs),
        Expr::CollectionAppend(op) => {
            collect_refs_in_expr(&op.collection, locals, refs);
            collect_refs_in_expr(&op.element, locals, refs);
        }
        Expr::CollectionInsert(op) => {
            collect_refs_in_expr(&op.collection, locals, refs);
            collect_refs_in_expr(&op.index, locals, refs);
            collect_refs_in_expr(&op.element, locals, refs);
        }
        Expr::CollectionRemoveValue(op) => {
            collect_refs_in_expr(&op.collection, locals, refs);
            collect_refs_in_expr(&op.value, locals, refs);
        }
        Expr::CollectionRemoveAll(op) => {
            collect_refs_in_expr(&op.collection, locals, refs);
            collect_refs_in_expr(&op.value, locals, refs);
        }
        Expr::CollectionRemoveAt(op) => {
            collect_refs_in_expr(&op.collection, locals, refs);
            collect_refs_in_expr(&op.index, locals, refs);
        }
        Expr::CollectionRemoveRange(op) => {
            collect_refs_in_expr(&op.collection, locals, refs);
            if let Some(s) = &op.start { collect_refs_in_expr(s, locals, refs); }
            if let Some(e) = &op.end { collect_refs_in_expr(e, locals, refs); }
        }
        Expr::CollectionContains(op) => {
            collect_refs_in_expr(&op.collection, locals, refs);
            collect_refs_in_expr(&op.element, locals, refs);
        }
        Expr::CollectionUpdate(op) => {
            collect_refs_in_expr(&op.target, locals, refs);
            collect_refs_in_expr(&op.value, locals, refs);
        }
        Expr::CollectionSlice(op) => {
            collect_refs_in_expr(&op.collection, locals, refs);
            if let Some(s) = &op.start { collect_refs_in_expr(s, locals, refs); }
            if let Some(e) = &op.end { collect_refs_in_expr(e, locals, refs); }
        }
        Expr::CollectionMap(op) => {
            collect_refs_in_expr(&op.collection, locals, refs);
            collect_refs_in_expr(&op.lambda, locals, refs);
        }
        Expr::CollectionFilter(op) => {
            collect_refs_in_expr(&op.collection, locals, refs);
            collect_refs_in_expr(&op.lambda, locals, refs);
        }
        Expr::CollectionReduce(op) => {
            collect_refs_in_expr(&op.collection, locals, refs);
            collect_refs_in_expr(&op.initial, locals, refs);
            collect_refs_in_expr(&op.lambda, locals, refs);
        }
        Expr::CollectionSortAsc(op) | Expr::CollectionSortDesc(op) | Expr::CollectionSortCustom(op) => {
            collect_refs_in_expr(&op.collection, locals, refs);
            if let Some(ref cmp) = op.comparator { collect_refs_in_expr(cmp, locals, refs); }
        }
        Expr::NumericEval(op)    => collect_refs_in_expr(&op.expr, locals, refs),
        Expr::TypeMetadata(op)   => collect_refs_in_expr(&op.expr, locals, refs),
        Expr::Format(op)         => collect_refs_in_expr(&op.expr, locals, refs),
        Expr::BaseConversion(op) => collect_refs_in_expr(&op.expr, locals, refs),
        Expr::Round(op)          => collect_refs_in_expr(&op.expr, locals, refs),
        Expr::Trunc(op)          => collect_refs_in_expr(&op.expr, locals, refs),
        Expr::ErrorCheck(op)     => collect_refs_in_expr(&op.expr, locals, refs),
        Expr::ErrorPropagate(op) => collect_refs_in_expr(&op.expr, locals, refs),
        Expr::Pipe(pipe) => {
            collect_refs_in_expr(&pipe.left, locals, refs);
            collect_refs_in_expr(&pipe.callable, locals, refs);
            for arg in &pipe.arguments {
                if let zymbol_ast::PipeArg::Expr(e) = arg { collect_refs_in_expr(e, locals, refs); }
            }
        }
        Expr::CollectionFindAll(op) => {
            collect_refs_in_expr(&op.collection, locals, refs);
            collect_refs_in_expr(&op.value, locals, refs);
        }
        Expr::StringRepeat(op) => {
            collect_refs_in_expr(&op.string, locals, refs);
            collect_refs_in_expr(&op.count, locals, refs);
        }
        Expr::StringReplace(op) => {
            collect_refs_in_expr(&op.string, locals, refs);
            collect_refs_in_expr(&op.pattern, locals, refs);
            collect_refs_in_expr(&op.replacement, locals, refs);
            if let Some(c) = &op.count { collect_refs_in_expr(c, locals, refs); }
        }
        Expr::StringSplit(op) => {
            collect_refs_in_expr(&op.string, locals, refs);
            collect_refs_in_expr(&op.delimiter, locals, refs);
        }
        Expr::ConcatBuild(op) => {
            collect_refs_in_expr(&op.base, locals, refs);
            for item in &op.items { collect_refs_in_expr(item, locals, refs); }
        }
        Expr::NumericCast(op) => collect_refs_in_expr(&op.expr, locals, refs),
        Expr::DeepIndex(di) => {
            collect_refs_in_expr(&di.array, locals, refs);
            for step in &di.path.steps {
                collect_refs_in_expr(&step.index, locals, refs);
                if let Some(end) = &step.range_end { collect_refs_in_expr(end, locals, refs); }
            }
        }
        Expr::FlatExtract(fe) => {
            collect_refs_in_expr(&fe.array, locals, refs);
            for path in &fe.paths {
                for step in &path.steps {
                    collect_refs_in_expr(&step.index, locals, refs);
                    if let Some(end) = &step.range_end { collect_refs_in_expr(end, locals, refs); }
                }
            }
        }
        Expr::StructuredExtract(se) => {
            collect_refs_in_expr(&se.array, locals, refs);
            for group in &se.groups {
                for path in &group.paths {
                    for step in &path.steps {
                        collect_refs_in_expr(&step.index, locals, refs);
                        if let Some(end) = &step.range_end { collect_refs_in_expr(end, locals, refs); }
                    }
                }
            }
        }
        // Literals and shell exprs have no capturable sub-expressions
        Expr::Literal(_) | Expr::Execute(_) | Expr::BashExec(_) | Expr::TerminalSize(_) => {}
    }
}

fn collect_refs_in_stmts(
    stmts: &[zymbol_ast::Statement],
    locals: &mut HashSet<String>,
    refs: &mut HashSet<String>,
) {
    for stmt in stmts {
        match stmt {
            zymbol_ast::Statement::Assignment(a) => {
                collect_refs_in_expr(&a.value, &locals.iter().map(|s| s.as_str()).collect(), refs);
                locals.insert(a.name.clone());
            }
            zymbol_ast::Statement::ConstDecl(c) => {
                collect_refs_in_expr(&c.value, &locals.iter().map(|s| s.as_str()).collect(), refs);
                locals.insert(c.name.clone());
            }
            zymbol_ast::Statement::Expr(es) => {
                collect_refs_in_expr(&es.expr, &locals.iter().map(|s| s.as_str()).collect(), refs);
            }
            zymbol_ast::Statement::Output(o) => {
                for item in &o.exprs {
                    collect_refs_in_expr(item, &locals.iter().map(|s| s.as_str()).collect(), refs);
                }
            }
            zymbol_ast::Statement::If(if_stmt) => {
                collect_refs_in_expr(&if_stmt.condition, &locals.iter().map(|s| s.as_str()).collect(), refs);
                let mut bl = locals.clone();
                collect_refs_in_stmts(&if_stmt.then_block.statements, &mut bl, refs);
            }
            zymbol_ast::Statement::Return(r) => {
                if let Some(v) = &r.value {
                    collect_refs_in_expr(v, &locals.iter().map(|s| s.as_str()).collect(), refs);
                }
            }
            zymbol_ast::Statement::Loop(l) => {
                let mut bl = locals.clone();
                collect_refs_in_stmts(&l.body.statements, &mut bl, refs);
            }
            zymbol_ast::Statement::DestructureAssign(d) => {
                collect_refs_in_expr(&d.value, &locals.iter().map(|s| s.as_str()).collect(), refs);
            }
            _ => {}  // Break, Continue, Newline, etc. have no expressions
        }
    }
}
