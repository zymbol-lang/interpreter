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

        // B2: zero-copy save + fresh isolated scope (see take_call_state)
        let saved = self.take_call_state();

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
        let result = match &func.body {
            zymbol_ast::LambdaBody::Expr(expr) => {
                self.eval_expr(expr)?
            }
            zymbol_ast::LambdaBody::Block(block) => {
                self.execute_block_no_scope(block)?;
                match std::mem::replace(&mut self.control_flow, ControlFlow::None) {
                    ControlFlow::Return(val) => {
                        self.has_control_flow = false;
                        val.unwrap_or(Value::Unit)
                    }
                    _ => {
                        return Err(RuntimeError::Generic {
                            message: "block lambda must use <~ to return value".to_string(),
                            span: *span,
                        });
                    }
                }
            }
        };

        self.restore_call_state(saved);
        Ok(result)
    }

    /// Evaluate a function call
    pub(crate) fn eval_function_call(&mut self, call: &zymbol_ast::FunctionCallExpr) -> Result<Value> {
        // Determine what we're calling based on the callable expression
        match call.callable.as_ref() {
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
                if let Expr::Identifier(module_ident) = member.object.as_ref() {
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

    /// Helper to evaluate traditional (non-lambda) function calls
    pub(crate) fn eval_traditional_function_call(
        &mut self,
        func_def: Rc<FunctionDef>,
        arguments: &[zymbol_ast::Expr],
        span: &Span,
        module_info: Option<(String, std::path::PathBuf)>,
        func_name: Option<&str>,
    ) -> Result<Value> {
        // Check parameter count
        if arguments.len() != func_def.parameters.len() {
            return Err(RuntimeError::Generic {
                message: format!(
                    "function expects {} arguments, got {}",
                    func_def.parameters.len(),
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
            scope.reserve(func_def.parameters.len());
        }

        // If this is a module function call, restore module's execution context
        if let Some((_, module_path)) = &module_info {
            if let Some(module) = self.loaded_modules.get(module_path).cloned() {
                for (name, value) in &module.all_variables {
                    self.set_variable(name, value.clone());
                }
                self.import_aliases = module.import_aliases.clone();
            }
        }

        // QW8: move values out of arg_values instead of cloning
        // set_variable_new: skip scope-stack scan for Normal params (fresh isolated scope)
        for (i, param) in func_def.parameters.iter().enumerate() {
            let arg_value = std::mem::replace(&mut arg_values[i], Value::Unit);
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
        for param in &func_def.parameters {
            if matches!(param.kind, ParameterKind::Output) {
                self.current_output_params.insert(param.name.clone());
            }
        }

        // QW1: execute_block_no_scope — take_call_state already owns scope[0] (params).
        // QW17: TCO loop — if tco_pending is set after execution, rebind params and restart.
        let return_value = 'tco: loop {
            self.execute_block_no_scope(&func_def.body)?;

            if self.tco_pending {
                self.tco_pending = false;
                // Rebind parameters to tco_args
                // Move tco_args into params (no clone — QW8 semantics for TCO)
                let tco_args = std::mem::take(&mut self.tco_args);
                for (param, val) in func_def.parameters.iter().zip(tco_args.into_iter()) {
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

        // QW2: lazy output-param collection — only allocate if function has output params.
        // Eliminates HashMap::new() on every call (most functions have no output params).
        let has_output_params = func_def.parameters.iter().any(|p| matches!(p.kind, ParameterKind::Output));
        if has_output_params {
            let mut updates = Vec::new();
            for (i, param) in func_def.parameters.iter().enumerate() {
                if matches!(param.kind, ParameterKind::Output) {
                    if let Expr::Identifier(ident) = &arguments[i] {
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

        Ok(return_value)
    }
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
        Expr::CollectionRemove(op) => {
            collect_refs_in_expr(&op.collection, locals, refs);
            collect_refs_in_expr(&op.index, locals, refs);
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
        Expr::StringFindPositions(op) => {
            collect_refs_in_expr(&op.string, locals, refs);
            collect_refs_in_expr(&op.pattern, locals, refs);
        }
        Expr::StringInsert(op) => {
            collect_refs_in_expr(&op.string, locals, refs);
            collect_refs_in_expr(&op.position, locals, refs);
            collect_refs_in_expr(&op.text, locals, refs);
        }
        Expr::StringRemove(op) => {
            collect_refs_in_expr(&op.string, locals, refs);
            collect_refs_in_expr(&op.position, locals, refs);
            collect_refs_in_expr(&op.count, locals, refs);
        }
        Expr::StringReplace(op) => {
            collect_refs_in_expr(&op.string, locals, refs);
            collect_refs_in_expr(&op.pattern, locals, refs);
            collect_refs_in_expr(&op.replacement, locals, refs);
            if let Some(c) = &op.count { collect_refs_in_expr(c, locals, refs); }
        }
        // Literals and shell exprs have no capturable sub-expressions
        Expr::Literal(_) | Expr::Execute(_) | Expr::BashExec(_) => {}
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
            _ => {}  // Break, Continue, Newline, etc. have no expressions
        }
    }
}
