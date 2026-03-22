//! Type Checking for Zymbol-Lang
//!
//! Provides type inference and validation for expressions and statements.
//! Uses a multi-pass approach:
//! 1. First pass: collect function declarations
//! 2. Second pass: infer parameter types from usage
//! 3. Third pass: type check statements and expressions

use std::collections::HashMap;
use zymbol_ast::{Expr, Statement, Program, FunctionDecl, Block};
use zymbol_common::{BinaryOp, Literal, UnaryOp};
use zymbol_error::Diagnostic;

/// Represents a Zymbol type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZymbolType {
    /// Integer type
    Int,
    /// Float type
    Float,
    /// String type
    String,
    /// Character type
    Char,
    /// Boolean type
    Bool,
    /// Array of a specific element type
    Array(Box<ZymbolType>),
    /// Tuple of types
    Tuple(Vec<ZymbolType>),
    /// Named tuple with field names and types
    NamedTuple(Vec<(String, ZymbolType)>),
    /// Function type (params -> return)
    Function(Vec<ZymbolType>, Box<ZymbolType>),
    /// Unit type (no value)
    Unit,
    /// Error type (for error values)
    Error,
    /// Unknown type (for inference)
    Unknown,
    /// Any type (for polymorphic operations)
    Any,
}

impl ZymbolType {
    /// Get a human-readable name for this type
    pub fn name(&self) -> String {
        match self {
            ZymbolType::Int => "Int".to_string(),
            ZymbolType::Float => "Float".to_string(),
            ZymbolType::String => "String".to_string(),
            ZymbolType::Char => "Char".to_string(),
            ZymbolType::Bool => "Bool".to_string(),
            ZymbolType::Array(elem) => format!("[{}]", elem.name()),
            ZymbolType::Tuple(elems) => {
                let names: Vec<String> = elems.iter().map(|e| e.name()).collect();
                format!("({})", names.join(", "))
            }
            ZymbolType::NamedTuple(fields) => {
                let names: Vec<String> = fields.iter()
                    .map(|(n, t)| format!("{}: {}", n, t.name()))
                    .collect();
                format!("({})", names.join(", "))
            }
            ZymbolType::Function(params, ret) => {
                let param_names: Vec<String> = params.iter().map(|p| p.name()).collect();
                format!("({}) -> {}", param_names.join(", "), ret.name())
            }
            ZymbolType::Unit => "Unit".to_string(),
            ZymbolType::Error => "Error".to_string(),
            ZymbolType::Unknown => "?".to_string(),
            ZymbolType::Any => "Any".to_string(),
        }
    }

    /// Check if this type is numeric (Int or Float)
    pub fn is_numeric(&self) -> bool {
        matches!(self, ZymbolType::Int | ZymbolType::Float)
    }

    /// Check if two types are compatible for assignment
    pub fn is_compatible_with(&self, other: &ZymbolType) -> bool {
        match (self, other) {
            (ZymbolType::Any, _) | (_, ZymbolType::Any) => true,
            (ZymbolType::Unknown, _) | (_, ZymbolType::Unknown) => true,
            (ZymbolType::Int, ZymbolType::Float) | (ZymbolType::Float, ZymbolType::Int) => true,
            (a, b) => a == b,
        }
    }
}

/// Type constraint for inference
#[derive(Debug, Clone)]
pub enum TypeConstraint {
    /// Must be this exact type
    Exact(ZymbolType),
    /// Must be numeric (Int or Float)
    Numeric,
    /// Must be boolean
    Boolean,
    /// Must be compatible with another type
    CompatibleWith(ZymbolType),
    /// No constraint yet
    Unconstrained,
}

impl TypeConstraint {
    /// Unify two constraints into one
    fn unify(&self, other: &TypeConstraint) -> TypeConstraint {
        match (self, other) {
            (TypeConstraint::Exact(t), _) => TypeConstraint::Exact(t.clone()),
            (_, TypeConstraint::Exact(t)) => TypeConstraint::Exact(t.clone()),
            (TypeConstraint::Numeric, TypeConstraint::CompatibleWith(ZymbolType::Int)) |
            (TypeConstraint::CompatibleWith(ZymbolType::Int), TypeConstraint::Numeric) => {
                TypeConstraint::Exact(ZymbolType::Int)
            }
            (TypeConstraint::Numeric, TypeConstraint::CompatibleWith(ZymbolType::Float)) |
            (TypeConstraint::CompatibleWith(ZymbolType::Float), TypeConstraint::Numeric) => {
                TypeConstraint::Exact(ZymbolType::Float)
            }
            (TypeConstraint::Numeric, _) | (_, TypeConstraint::Numeric) => TypeConstraint::Numeric,
            (TypeConstraint::Boolean, _) | (_, TypeConstraint::Boolean) => TypeConstraint::Boolean,
            (TypeConstraint::CompatibleWith(t), _) => TypeConstraint::CompatibleWith(t.clone()),
            (_, TypeConstraint::CompatibleWith(t)) => TypeConstraint::CompatibleWith(t.clone()),
            (TypeConstraint::Unconstrained, TypeConstraint::Unconstrained) => TypeConstraint::Unconstrained,
        }
    }

    /// Convert constraint to a concrete type
    fn to_type(&self) -> ZymbolType {
        match self {
            TypeConstraint::Exact(t) => t.clone(),
            TypeConstraint::Numeric => ZymbolType::Int, // Default to Int if only numeric constraint
            TypeConstraint::Boolean => ZymbolType::Bool,
            TypeConstraint::CompatibleWith(t) => t.clone(),
            TypeConstraint::Unconstrained => ZymbolType::Any,
        }
    }
}

/// Type environment for tracking variable types
#[derive(Debug, Clone)]
pub struct TypeEnv {
    /// Variable types by scope level
    scopes: Vec<HashMap<String, ZymbolType>>,
    /// Function signatures
    functions: HashMap<String, (Vec<ZymbolType>, ZymbolType)>,
    /// Constants (immutable)
    constants: HashMap<String, ZymbolType>,
    /// Parameter constraints during inference (param_name -> constraints)
    param_constraints: HashMap<String, Vec<TypeConstraint>>,
}

impl TypeEnv {
    /// Create a new empty type environment
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            constants: HashMap::new(),
            param_constraints: HashMap::new(),
        }
    }

    /// Clear parameter constraints (used between inference passes)
    pub fn clear_param_constraints(&mut self) {
        self.param_constraints.clear();
    }

    /// Add a constraint for a parameter
    pub fn add_param_constraint(&mut self, name: &str, constraint: TypeConstraint) {
        self.param_constraints
            .entry(name.to_string())
            .or_default()
            .push(constraint);
    }

    /// Get the unified type for a parameter based on all constraints
    pub fn resolve_param_type(&self, name: &str) -> ZymbolType {
        if let Some(constraints) = self.param_constraints.get(name) {
            if constraints.is_empty() {
                return ZymbolType::Any;
            }
            let mut unified = constraints[0].clone();
            for constraint in &constraints[1..] {
                unified = unified.unify(constraint);
            }
            unified.to_type()
        } else {
            ZymbolType::Any
        }
    }

    /// Enter a new scope
    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Exit the current scope
    pub fn exit_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Define a variable in the current scope
    pub fn define_var(&mut self, name: &str, ty: ZymbolType) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    /// Define a constant
    pub fn define_const(&mut self, name: &str, ty: ZymbolType) {
        self.constants.insert(name.to_string(), ty);
    }

    /// Define a function
    pub fn define_function(&mut self, name: &str, params: Vec<ZymbolType>, return_type: ZymbolType) {
        self.functions.insert(name.to_string(), (params, return_type));
    }

    /// Look up a variable's type
    pub fn lookup_var(&self, name: &str) -> Option<&ZymbolType> {
        // Check constants first
        if let Some(ty) = self.constants.get(name) {
            return Some(ty);
        }
        // Check scopes from innermost to outermost
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }

    /// Look up a function's signature
    pub fn lookup_function(&self, name: &str) -> Option<&(Vec<ZymbolType>, ZymbolType)> {
        self.functions.get(name)
    }

    /// Check if a name is a constant
    pub fn is_constant(&self, name: &str) -> bool {
        self.constants.contains_key(name)
    }
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}

/// Type checker for Zymbol programs
#[derive(Debug)]
pub struct TypeChecker {
    /// Type environment
    env: TypeEnv,
    /// Collected errors (fatal)
    errors: Vec<Diagnostic>,
    /// Collected warnings (non-fatal)
    warnings: Vec<Diagnostic>,
}

impl TypeChecker {
    /// Create a new type checker
    pub fn new() -> Self {
        Self {
            env: TypeEnv::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Check a program and return all diagnostics (errors + warnings)
    pub fn check(&mut self, program: &Program) -> Vec<Diagnostic> {
        self.errors.clear();
        self.warnings.clear();

        // First pass: collect function declarations with placeholder types
        for stmt in &program.statements {
            if let Statement::FunctionDecl(func) = stmt {
                let param_types: Vec<ZymbolType> = func.parameters.iter()
                    .map(|_| ZymbolType::Any)
                    .collect();
                self.env.define_function(&func.name, param_types, ZymbolType::Any);
            }
        }

        // Second pass: infer function parameter and return types
        for stmt in &program.statements {
            if let Statement::FunctionDecl(func) = stmt {
                let (param_types, return_type) = self.infer_function_signature(func);
                self.env.define_function(&func.name, param_types, return_type);
            }
        }

        // Third pass: check statements with inferred types
        for stmt in &program.statements {
            self.check_statement(stmt);
        }

        // Return all diagnostics combined
        let mut all = std::mem::take(&mut self.errors);
        all.extend(std::mem::take(&mut self.warnings));
        all
    }

    /// Check a program and return only errors (fatal type errors)
    pub fn check_errors(&mut self, program: &Program) -> Vec<Diagnostic> {
        self.errors.clear();
        self.warnings.clear();

        // First pass: collect function declarations with placeholder types
        for stmt in &program.statements {
            if let Statement::FunctionDecl(func) = stmt {
                let param_types: Vec<ZymbolType> = func.parameters.iter()
                    .map(|_| ZymbolType::Any)
                    .collect();
                self.env.define_function(&func.name, param_types, ZymbolType::Any);
            }
        }

        // Second pass: infer function parameter and return types
        for stmt in &program.statements {
            if let Statement::FunctionDecl(func) = stmt {
                let (param_types, return_type) = self.infer_function_signature(func);
                self.env.define_function(&func.name, param_types, return_type);
            }
        }

        // Third pass: check statements with inferred types
        for stmt in &program.statements {
            self.check_statement(stmt);
        }

        std::mem::take(&mut self.errors)
    }

    /// Get only warnings (non-fatal type issues)
    pub fn get_warnings(&self) -> &[Diagnostic] {
        &self.warnings
    }

    /// Get only errors (fatal type issues)
    pub fn get_errors(&self) -> &[Diagnostic] {
        &self.errors
    }

    /// Check if there are any type errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Check a statement
    fn check_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Assignment(assign) => {
                let value_type = self.infer_expr(&assign.value);

                // Check if reassigning a constant - this is an ERROR
                if self.env.is_constant(&assign.name) {
                    self.errors.push(
                        Diagnostic::error(format!("cannot reassign constant '{}'", assign.name))
                            .with_span(assign.span)
                            .with_help("constants declared with ':=' cannot be modified")
                    );
                    return;
                }

                // Check for type consistency on reassignment - this is a WARNING
                if let Some(existing_type) = self.env.lookup_var(&assign.name).cloned() {
                    if !existing_type.is_compatible_with(&value_type) {
                        self.warnings.push(
                            Diagnostic::warning(format!(
                                "type mismatch: '{}' was {} but assigned {}",
                                assign.name, existing_type.name(), value_type.name()
                            ))
                            .with_span(assign.span)
                        );
                    }
                }

                self.env.define_var(&assign.name, value_type);
            }

            Statement::ConstDecl(const_decl) => {
                let value_type = self.infer_expr(&const_decl.value);
                self.env.define_const(&const_decl.name, value_type);
            }

            Statement::Output(output) => {
                for expr in &output.exprs {
                    self.infer_expr(expr);
                }
            }

            Statement::Input(input) => {
                // Input always produces a string
                self.env.define_var(&input.variable, ZymbolType::String);
            }

            Statement::If(if_stmt) => {
                let cond_type = self.infer_expr(&if_stmt.condition);
                if !matches!(cond_type, ZymbolType::Bool | ZymbolType::Any | ZymbolType::Unknown) {
                    self.warnings.push(
                        Diagnostic::warning(format!(
                            "if condition should be Bool, got {}",
                            cond_type.name()
                        ))
                        .with_span(if_stmt.condition.span())
                    );
                }

                // Check then block
                self.env.enter_scope();
                for stmt in &if_stmt.then_block.statements {
                    self.check_statement(stmt);
                }
                self.env.exit_scope();

                // Check else-if branches
                for branch in &if_stmt.else_if_branches {
                    let branch_cond_type = self.infer_expr(&branch.condition);
                    if !matches!(branch_cond_type, ZymbolType::Bool | ZymbolType::Any | ZymbolType::Unknown) {
                        self.warnings.push(
                            Diagnostic::warning(format!(
                                "else-if condition should be Bool, got {}",
                                branch_cond_type.name()
                            ))
                            .with_span(branch.condition.span())
                        );
                    }

                    self.env.enter_scope();
                    for stmt in &branch.block.statements {
                        self.check_statement(stmt);
                    }
                    self.env.exit_scope();
                }

                // Check else block
                if let Some(else_block) = &if_stmt.else_block {
                    self.env.enter_scope();
                    for stmt in &else_block.statements {
                        self.check_statement(stmt);
                    }
                    self.env.exit_scope();
                }
            }

            Statement::Loop(loop_stmt) => {
                // Check condition if present
                if let Some(condition) = &loop_stmt.condition {
                    let cond_type = self.infer_expr(condition);
                    if !matches!(cond_type, ZymbolType::Bool | ZymbolType::Any | ZymbolType::Unknown) {
                        self.warnings.push(
                            Diagnostic::warning(format!(
                                "loop condition should be Bool, got {}",
                                cond_type.name()
                            ))
                            .with_span(condition.span())
                        );
                    }
                }

                self.env.enter_scope();

                // Define iterator variable
                if let Some(iter_var) = &loop_stmt.iterator_var {
                    // Infer type from iterable
                    let iter_type = if let Some(iterable) = &loop_stmt.iterable {
                        match self.infer_expr(iterable) {
                            ZymbolType::Array(elem) => *elem,
                            ZymbolType::String => ZymbolType::Char,
                            _ => ZymbolType::Any,
                        }
                    } else {
                        ZymbolType::Int // Range loop
                    };
                    self.env.define_var(iter_var, iter_type);
                }

                for stmt in &loop_stmt.body.statements {
                    self.check_statement(stmt);
                }
                self.env.exit_scope();
            }

            Statement::FunctionDecl(func) => {
                self.env.enter_scope();

                // Get inferred parameter types from function signature
                let param_types = if let Some((params, _)) = self.env.lookup_function(&func.name).cloned() {
                    params
                } else {
                    vec![ZymbolType::Any; func.parameters.len()]
                };

                // Define parameters with their inferred types
                for (i, param) in func.parameters.iter().enumerate() {
                    let param_type = param_types.get(i).cloned().unwrap_or(ZymbolType::Any);
                    self.env.define_var(&param.name, param_type);
                }

                // Check body
                for stmt in &func.body.statements {
                    self.check_statement(stmt);
                }

                self.env.exit_scope();
            }

            Statement::Return(ret) => {
                if let Some(value) = &ret.value {
                    self.infer_expr(value);
                }
            }

            Statement::Try(try_stmt) => {
                // Check try block
                self.env.enter_scope();
                for stmt in &try_stmt.try_block.statements {
                    self.check_statement(stmt);
                }
                self.env.exit_scope();

                // Check catch blocks
                for catch in &try_stmt.catch_clauses {
                    self.env.enter_scope();
                    // _err is available in catch blocks
                    self.env.define_var("_err", ZymbolType::Error);
                    for stmt in &catch.block.statements {
                        self.check_statement(stmt);
                    }
                    self.env.exit_scope();
                }

                // Check finally block
                if let Some(finally) = &try_stmt.finally_clause {
                    self.env.enter_scope();
                    for stmt in &finally.block.statements {
                        self.check_statement(stmt);
                    }
                    self.env.exit_scope();
                }
            }

            Statement::Expr(expr_stmt) => {
                self.infer_expr(&expr_stmt.expr);
            }

            Statement::Match(match_stmt) => {
                let scrutinee_type = self.infer_expr(&match_stmt.scrutinee);

                for case in &match_stmt.cases {
                    // Validate pattern type against scrutinee
                    self.check_pattern_type(&case.pattern, &scrutinee_type);

                    if let Some(value) = &case.value {
                        self.infer_expr(value);
                    }
                    if let Some(block) = &case.block {
                        self.env.enter_scope();
                        for stmt in &block.statements {
                            self.check_statement(stmt);
                        }
                        self.env.exit_scope();
                    }
                }
            }

            // Other statements don't need type checking
            _ => {}
        }
    }

    /// Infer function signature from its body
    /// Returns (parameter_types, return_type)
    fn infer_function_signature(&mut self, func: &FunctionDecl) -> (Vec<ZymbolType>, ZymbolType) {
        // Clear any previous constraints
        self.env.clear_param_constraints();

        // Create a temporary scope for parameter analysis
        self.env.enter_scope();

        // Register parameters with Any type initially
        for param in &func.parameters {
            self.env.define_var(&param.name, ZymbolType::Any);
        }

        // Collect constraints from body usage
        self.collect_constraints_from_block(&func.body, &func.parameters.iter().map(|p| p.name.clone()).collect::<Vec<_>>());

        // Define local variables (non-parameter assignments) in scope so return type
        // inference can resolve them. Without this, `<~ local_var` produces a false
        // "undefined variable" error because local vars aren't in scope during pass 2.
        self.define_local_vars_from_block(&func.body);

        // Infer return type from return statements
        // Suppress false "undefined variable" errors that occur because local vars
        // defined in inner scopes may not be fully visible during signature inference.
        let errors_before = self.errors.len();
        let return_type = self.infer_return_type_from_block(&func.body);
        self.errors.truncate(errors_before);

        // Resolve parameter types from constraints
        let param_types: Vec<ZymbolType> = func.parameters.iter()
            .map(|p| self.env.resolve_param_type(&p.name))
            .collect();

        self.env.exit_scope();

        (param_types, return_type)
    }

    /// Collect type constraints from a block for parameter inference
    fn collect_constraints_from_block(&mut self, block: &Block, params: &[String]) {
        for stmt in &block.statements {
            self.collect_constraints_from_statement(stmt, params);
        }
    }

    /// Define all local variable assignments from a block in the current scope.
    /// Used during signature inference so `<~ local_var` doesn't produce false
    /// "undefined variable" errors when local vars aren't yet in scope.
    fn define_local_vars_from_block(&mut self, block: &Block) {
        for stmt in &block.statements {
            match stmt {
                Statement::Assignment(assign) => {
                    if self.env.lookup_var(&assign.name).is_none() {
                        self.env.define_var(&assign.name, ZymbolType::Any);
                    }
                }
                Statement::If(if_stmt) => {
                    self.define_local_vars_from_block(&if_stmt.then_block);
                    for branch in &if_stmt.else_if_branches {
                        self.define_local_vars_from_block(&branch.block);
                    }
                    if let Some(else_block) = &if_stmt.else_block {
                        self.define_local_vars_from_block(else_block);
                    }
                }
                Statement::Loop(loop_stmt) => {
                    self.define_local_vars_from_block(&loop_stmt.body);
                }
                _ => {}
            }
        }
    }

    /// Collect type constraints from a statement
    fn collect_constraints_from_statement(&mut self, stmt: &Statement, params: &[String]) {
        match stmt {
            Statement::Assignment(assign) => {
                self.collect_constraints_from_expr(&assign.value, params);
            }
            Statement::ConstDecl(const_decl) => {
                self.collect_constraints_from_expr(&const_decl.value, params);
            }
            Statement::Output(output) => {
                for expr in &output.exprs {
                    self.collect_constraints_from_expr(expr, params);
                }
            }
            Statement::Return(ret) => {
                if let Some(value) = &ret.value {
                    self.collect_constraints_from_expr(value, params);
                }
            }
            Statement::If(if_stmt) => {
                self.collect_constraints_from_expr(&if_stmt.condition, params);
                self.collect_constraints_from_block(&if_stmt.then_block, params);
                for branch in &if_stmt.else_if_branches {
                    self.collect_constraints_from_expr(&branch.condition, params);
                    self.collect_constraints_from_block(&branch.block, params);
                }
                if let Some(else_block) = &if_stmt.else_block {
                    self.collect_constraints_from_block(else_block, params);
                }
            }
            Statement::Loop(loop_stmt) => {
                if let Some(condition) = &loop_stmt.condition {
                    self.collect_constraints_from_expr(condition, params);
                }
                if let Some(iterable) = &loop_stmt.iterable {
                    self.collect_constraints_from_expr(iterable, params);
                }
                self.collect_constraints_from_block(&loop_stmt.body, params);
            }
            Statement::Expr(expr_stmt) => {
                self.collect_constraints_from_expr(&expr_stmt.expr, params);
            }
            Statement::Match(match_stmt) => {
                self.collect_constraints_from_expr(&match_stmt.scrutinee, params);
                for case in &match_stmt.cases {
                    if let Some(value) = &case.value {
                        self.collect_constraints_from_expr(value, params);
                    }
                }
            }
            _ => {}
        }
    }

    /// Collect type constraints from an expression
    fn collect_constraints_from_expr(&mut self, expr: &Expr, params: &[String]) {
        match expr {
            Expr::Binary(binary) => {
                // Check if either side is a parameter
                let left_param = self.get_param_name(&binary.left, params);
                let right_param = self.get_param_name(&binary.right, params);

                match binary.op {
                    // Arithmetic operations constrain parameters to be numeric
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div |
                    BinaryOp::Mod | BinaryOp::Pow => {
                        if let Some(param) = left_param {
                            // If adding with a string, the param might be for concat
                            let right_type = self.infer_expr_for_constraint(&binary.right, params);
                            if matches!(right_type, ZymbolType::String) && matches!(binary.op, BinaryOp::Add) {
                                self.env.add_param_constraint(&param, TypeConstraint::CompatibleWith(ZymbolType::String));
                            } else {
                                self.env.add_param_constraint(&param, TypeConstraint::Numeric);
                            }
                        }
                        if let Some(param) = right_param {
                            let left_type = self.infer_expr_for_constraint(&binary.left, params);
                            if matches!(left_type, ZymbolType::String) && matches!(binary.op, BinaryOp::Add) {
                                self.env.add_param_constraint(&param, TypeConstraint::CompatibleWith(ZymbolType::String));
                            } else {
                                self.env.add_param_constraint(&param, TypeConstraint::Numeric);
                            }
                        }
                    }
                    // Logical operations constrain parameters to be boolean
                    BinaryOp::And | BinaryOp::Or => {
                        if let Some(param) = left_param {
                            self.env.add_param_constraint(&param, TypeConstraint::Boolean);
                        }
                        if let Some(param) = right_param {
                            self.env.add_param_constraint(&param, TypeConstraint::Boolean);
                        }
                    }
                    // Comparison with known type constrains parameter
                    BinaryOp::Eq | BinaryOp::Neq | BinaryOp::Lt | BinaryOp::Le |
                    BinaryOp::Gt | BinaryOp::Ge => {
                        if let Some(param) = &left_param {
                            let right_type = self.infer_expr_for_constraint(&binary.right, params);
                            if !matches!(right_type, ZymbolType::Any | ZymbolType::Unknown) {
                                self.env.add_param_constraint(param, TypeConstraint::CompatibleWith(right_type));
                            }
                        }
                        if let Some(param) = &right_param {
                            let left_type = self.infer_expr_for_constraint(&binary.left, params);
                            if !matches!(left_type, ZymbolType::Any | ZymbolType::Unknown) {
                                self.env.add_param_constraint(param, TypeConstraint::CompatibleWith(left_type));
                            }
                        }
                    }
                    _ => {}
                }

                // Recursively collect from children
                self.collect_constraints_from_expr(&binary.left, params);
                self.collect_constraints_from_expr(&binary.right, params);
            }
            Expr::Unary(unary) => {
                let operand_param = self.get_param_name(&unary.operand, params);
                match unary.op {
                    UnaryOp::Neg | UnaryOp::Pos => {
                        if let Some(param) = operand_param {
                            self.env.add_param_constraint(&param, TypeConstraint::Numeric);
                        }
                    }
                    UnaryOp::Not => {
                        if let Some(param) = operand_param {
                            self.env.add_param_constraint(&param, TypeConstraint::Boolean);
                        }
                    }
                }
                self.collect_constraints_from_expr(&unary.operand, params);
            }
            Expr::FunctionCall(call) => {
                // Get the expected parameter types from the function
                if let Expr::Identifier(ident) = &*call.callable {
                    if let Some((expected_params, _)) = self.env.lookup_function(&ident.name).cloned() {
                        for (i, arg) in call.arguments.iter().enumerate() {
                            if let Some(expected_type) = expected_params.get(i) {
                                if let Some(param) = self.get_param_name(arg, params) {
                                    if !matches!(expected_type, ZymbolType::Any | ZymbolType::Unknown) {
                                        self.env.add_param_constraint(&param, TypeConstraint::CompatibleWith(expected_type.clone()));
                                    }
                                }
                            }
                        }
                    }
                }
                for arg in &call.arguments {
                    self.collect_constraints_from_expr(arg, params);
                }
            }
            Expr::Index(index) => {
                // If indexing with a param, it should be Int
                if let Some(param) = self.get_param_name(&index.index, params) {
                    self.env.add_param_constraint(&param, TypeConstraint::Exact(ZymbolType::Int));
                }
                self.collect_constraints_from_expr(&index.array, params);
                self.collect_constraints_from_expr(&index.index, params);
            }
            Expr::ArrayLiteral(arr) => {
                for elem in &arr.elements {
                    self.collect_constraints_from_expr(elem, params);
                }
            }
            Expr::CollectionAppend(op) => {
                self.collect_constraints_from_expr(&op.collection, params);
                self.collect_constraints_from_expr(&op.element, params);
            }
            Expr::CollectionContains(op) => {
                self.collect_constraints_from_expr(&op.collection, params);
                self.collect_constraints_from_expr(&op.element, params);
            }
            _ => {}
        }
    }

    /// Get parameter name if expression is a parameter identifier
    fn get_param_name(&self, expr: &Expr, params: &[String]) -> Option<String> {
        if let Expr::Identifier(ident) = expr {
            if params.contains(&ident.name) {
                return Some(ident.name.clone());
            }
        }
        None
    }

    /// Infer type of expression during constraint collection (simplified)
    fn infer_expr_for_constraint(&self, expr: &Expr, _params: &[String]) -> ZymbolType {
        match expr {
            Expr::Literal(lit) => match &lit.value {
                Literal::Int(_) => ZymbolType::Int,
                Literal::Float(_) => ZymbolType::Float,
                Literal::String(_) => ZymbolType::String,
                Literal::Char(_) => ZymbolType::Char,
                Literal::Bool(_) => ZymbolType::Bool,
            },
            Expr::Identifier(ident) => {
                self.env.lookup_var(&ident.name)
                    .cloned()
                    .unwrap_or(ZymbolType::Any)
            }
            _ => ZymbolType::Any,
        }
    }

    /// Infer return type from a block by examining return statements
    fn infer_return_type_from_block(&mut self, block: &Block) -> ZymbolType {
        let mut return_types: Vec<ZymbolType> = Vec::new();

        for stmt in &block.statements {
            self.collect_return_types(stmt, &mut return_types);
        }

        if return_types.is_empty() {
            ZymbolType::Unit
        } else if return_types.len() == 1 {
            return_types.pop().unwrap()
        } else {
            // Unify multiple return types
            self.unify_types(&return_types)
        }
    }

    /// Collect return types from a statement recursively
    fn collect_return_types(&mut self, stmt: &Statement, return_types: &mut Vec<ZymbolType>) {
        match stmt {
            Statement::Return(ret) => {
                if let Some(value) = &ret.value {
                    let ty = self.infer_expr(value);
                    return_types.push(ty);
                } else {
                    return_types.push(ZymbolType::Unit);
                }
            }
            Statement::If(if_stmt) => {
                for stmt in &if_stmt.then_block.statements {
                    self.collect_return_types(stmt, return_types);
                }
                for branch in &if_stmt.else_if_branches {
                    for stmt in &branch.block.statements {
                        self.collect_return_types(stmt, return_types);
                    }
                }
                if let Some(else_block) = &if_stmt.else_block {
                    for stmt in &else_block.statements {
                        self.collect_return_types(stmt, return_types);
                    }
                }
            }
            Statement::Loop(loop_stmt) => {
                for stmt in &loop_stmt.body.statements {
                    self.collect_return_types(stmt, return_types);
                }
            }
            Statement::Match(match_stmt) => {
                for case in &match_stmt.cases {
                    if let Some(value) = &case.value {
                        // Match case values are implicit returns
                        let ty = self.infer_expr(value);
                        return_types.push(ty);
                    }
                    if let Some(block) = &case.block {
                        for stmt in &block.statements {
                            self.collect_return_types(stmt, return_types);
                        }
                    }
                }
            }
            Statement::Try(try_stmt) => {
                for stmt in &try_stmt.try_block.statements {
                    self.collect_return_types(stmt, return_types);
                }
                for catch in &try_stmt.catch_clauses {
                    for stmt in &catch.block.statements {
                        self.collect_return_types(stmt, return_types);
                    }
                }
            }
            _ => {}
        }
    }

    /// Check pattern type against scrutinee type
    fn check_pattern_type(&mut self, pattern: &zymbol_ast::Pattern, scrutinee_type: &ZymbolType) {
        use zymbol_ast::Pattern;

        match pattern {
            Pattern::Literal(lit, span) => {
                let pattern_type = match lit {
                    Literal::Int(_) => ZymbolType::Int,
                    Literal::Float(_) => ZymbolType::Float,
                    Literal::String(_) => ZymbolType::String,
                    Literal::Char(_) => ZymbolType::Char,
                    Literal::Bool(_) => ZymbolType::Bool,
                };

                if !self.types_compatible(&pattern_type, scrutinee_type) {
                    self.errors.push(
                        Diagnostic::error(format!(
                            "pattern type {} does not match scrutinee type {}",
                            pattern_type.name(), scrutinee_type.name()
                        ))
                        .with_span(*span)
                    );
                }
            }

            Pattern::Range(start, end, span) => {
                let start_type = self.infer_expr(start);
                let end_type = self.infer_expr(end);

                // Range patterns should be numeric or char
                if !matches!(start_type, ZymbolType::Int | ZymbolType::Float | ZymbolType::Char | ZymbolType::Any | ZymbolType::Unknown) {
                    self.errors.push(
                        Diagnostic::error(format!(
                            "range pattern start must be Int, Float, or Char, got {}",
                            start_type.name()
                        ))
                        .with_span(start.span())
                    );
                }

                if !self.types_compatible(&start_type, scrutinee_type) {
                    self.errors.push(
                        Diagnostic::error(format!(
                            "range pattern type {} does not match scrutinee type {}",
                            start_type.name(), scrutinee_type.name()
                        ))
                        .with_span(*span)
                    );
                }

                if !self.types_compatible(&start_type, &end_type) {
                    self.warnings.push(
                        Diagnostic::warning(format!(
                            "range start type {} differs from end type {}",
                            start_type.name(), end_type.name()
                        ))
                        .with_span(*span)
                    );
                }
            }

            Pattern::List(patterns, span) => {
                // Scrutinee should be an array
                if let ZymbolType::Array(elem_type) = scrutinee_type {
                    for p in patterns {
                        self.check_pattern_type(p, elem_type);
                    }
                } else if !matches!(scrutinee_type, ZymbolType::Any | ZymbolType::Unknown) {
                    self.errors.push(
                        Diagnostic::error(format!(
                            "list pattern requires array scrutinee, got {}",
                            scrutinee_type.name()
                        ))
                        .with_span(*span)
                    );
                }
            }

            Pattern::Wildcard(_) => {
                // Wildcard matches any type
            }

            Pattern::Guard(inner_pattern, condition, _span) => {
                // Check inner pattern
                self.check_pattern_type(inner_pattern, scrutinee_type);

                // Guard condition must be Bool
                let cond_type = self.infer_expr(condition);
                if !matches!(cond_type, ZymbolType::Bool | ZymbolType::Any | ZymbolType::Unknown) {
                    self.errors.push(
                        Diagnostic::error(format!(
                            "pattern guard must be Bool, got {}",
                            cond_type.name()
                        ))
                        .with_span(condition.span())
                    );
                }
            }
        }
    }

    /// Check if two types are compatible (for function call validation)
    fn types_compatible(&self, actual: &ZymbolType, expected: &ZymbolType) -> bool {
        Self::types_compatible_static(actual, expected)
    }

    fn types_compatible_static(actual: &ZymbolType, expected: &ZymbolType) -> bool {
        match (actual, expected) {
            // Any/Unknown types are always compatible
            (ZymbolType::Any, _) | (_, ZymbolType::Any) => true,
            (ZymbolType::Unknown, _) | (_, ZymbolType::Unknown) => true,
            // Exact match
            (a, b) if a == b => true,
            // Int is compatible with Float
            (ZymbolType::Int, ZymbolType::Float) => true,
            // Arrays are compatible if element types are compatible
            (ZymbolType::Array(a), ZymbolType::Array(b)) => Self::types_compatible_static(a, b),
            // Tuples are compatible if all elements are compatible
            (ZymbolType::Tuple(a), ZymbolType::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| Self::types_compatible_static(x, y))
            }
            // Named tuples are compatible if fields match
            (ZymbolType::NamedTuple(a), ZymbolType::NamedTuple(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|((na, ta), (nb, tb))| {
                    na == nb && Self::types_compatible_static(ta, tb)
                })
            }
            // Functions are compatible if params and return are compatible
            (ZymbolType::Function(pa, ra), ZymbolType::Function(pb, rb)) => {
                pa.len() == pb.len() &&
                pa.iter().zip(pb.iter()).all(|(a, b)| Self::types_compatible_static(a, b)) &&
                Self::types_compatible_static(ra, rb)
            }
            _ => false,
        }
    }

    /// Unify multiple types into one
    fn unify_types(&self, types: &[ZymbolType]) -> ZymbolType {
        Self::unify_types_static(types)
    }

    fn unify_types_static(types: &[ZymbolType]) -> ZymbolType {
        if types.is_empty() {
            return ZymbolType::Unit;
        }

        let mut unified = types[0].clone();
        for ty in &types[1..] {
            unified = match (&unified, ty) {
                // Same types stay the same
                (a, b) if a == b => unified,
                // Int and Float unify to Float
                (ZymbolType::Int, ZymbolType::Float) | (ZymbolType::Float, ZymbolType::Int) => {
                    ZymbolType::Float
                }
                // Any absorbs other types
                (ZymbolType::Any, _) | (_, ZymbolType::Any) => ZymbolType::Any,
                // Unknown absorbs other types
                (ZymbolType::Unknown, other) | (other, ZymbolType::Unknown) => other.clone(),
                // Arrays unify element types
                (ZymbolType::Array(a), ZymbolType::Array(b)) => {
                    ZymbolType::Array(Box::new(Self::unify_types_static(&[*a.clone(), *b.clone()])))
                }
                // Different types become Any
                _ => ZymbolType::Any,
            };
        }
        unified
    }

    /// Infer the type of an expression
    fn infer_expr(&mut self, expr: &Expr) -> ZymbolType {
        match expr {
            Expr::Literal(lit) => match &lit.value {
                Literal::Int(_) => ZymbolType::Int,
                Literal::Float(_) => ZymbolType::Float,
                Literal::String(_) => ZymbolType::String,
                Literal::Char(_) => ZymbolType::Char,
                Literal::Bool(_) => ZymbolType::Bool,
            },

            Expr::Identifier(ident) => {
                if let Some(ty) = self.env.lookup_var(&ident.name) {
                    ty.clone()
                } else if self.env.lookup_function(&ident.name).is_some() {
                    // It's a function reference, return Function type
                    ZymbolType::Any
                } else {
                    // Variable not defined - emit error
                    self.errors.push(
                        Diagnostic::error(format!("undefined variable '{}'", ident.name))
                            .with_span(ident.span)
                            .with_help("variables must be defined before use")
                    );
                    ZymbolType::Unknown
                }
            }

            Expr::Binary(binary) => {
                let left_type = self.infer_expr(&binary.left);
                let right_type = self.infer_expr(&binary.right);

                match binary.op {
                    // Arithmetic operations
                    BinaryOp::Add => {
                        if matches!(left_type, ZymbolType::String) || matches!(right_type, ZymbolType::String) {
                            ZymbolType::String // String concatenation
                        } else if left_type.is_numeric() && right_type.is_numeric() {
                            if matches!(left_type, ZymbolType::Float) || matches!(right_type, ZymbolType::Float) {
                                ZymbolType::Float
                            } else {
                                ZymbolType::Int
                            }
                        } else {
                            ZymbolType::Any
                        }
                    }
                    BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod | BinaryOp::Pow => {
                        if !left_type.is_numeric() && !matches!(left_type, ZymbolType::Any | ZymbolType::Unknown) {
                            self.warnings.push(
                                Diagnostic::warning(format!(
                                    "arithmetic operation on non-numeric type: {}",
                                    left_type.name()
                                ))
                                .with_span(binary.left.span())
                            );
                        }
                        if matches!(left_type, ZymbolType::Float) || matches!(right_type, ZymbolType::Float) {
                            ZymbolType::Float
                        } else {
                            ZymbolType::Int
                        }
                    }

                    // Comparison operations
                    BinaryOp::Eq | BinaryOp::Neq | BinaryOp::Lt | BinaryOp::Le |
                    BinaryOp::Gt | BinaryOp::Ge => ZymbolType::Bool,

                    // Logical operations
                    BinaryOp::And | BinaryOp::Or => {
                        if !matches!(left_type, ZymbolType::Bool | ZymbolType::Any | ZymbolType::Unknown) {
                            self.warnings.push(
                                Diagnostic::warning(format!(
                                    "logical operation on non-boolean type: {}",
                                    left_type.name()
                                ))
                                .with_span(binary.left.span())
                            );
                        }
                        ZymbolType::Bool
                    }

                    // Range
                    BinaryOp::Range => ZymbolType::Array(Box::new(ZymbolType::Int)),

                    // Other
                    BinaryOp::Pipe | BinaryOp::Comma => ZymbolType::Any,
                }
            }

            Expr::Unary(unary) => {
                let operand_type = self.infer_expr(&unary.operand);
                match unary.op {
                    UnaryOp::Neg | UnaryOp::Pos => {
                        if !operand_type.is_numeric() && !matches!(operand_type, ZymbolType::Any | ZymbolType::Unknown) {
                            self.warnings.push(
                                Diagnostic::warning(format!(
                                    "unary {} on non-numeric type: {}",
                                    if matches!(unary.op, UnaryOp::Neg) { "-" } else { "+" },
                                    operand_type.name()
                                ))
                                .with_span(unary.operand.span())
                            );
                        }
                        operand_type
                    }
                    UnaryOp::Not => {
                        if !matches!(operand_type, ZymbolType::Bool | ZymbolType::Any | ZymbolType::Unknown) {
                            self.warnings.push(
                                Diagnostic::warning(format!(
                                    "logical not on non-boolean type: {}",
                                    operand_type.name()
                                ))
                                .with_span(unary.operand.span())
                            );
                        }
                        ZymbolType::Bool
                    }
                }
            }

            Expr::ArrayLiteral(arr) => {
                if arr.elements.is_empty() {
                    ZymbolType::Array(Box::new(ZymbolType::Any))
                } else {
                    let first_type = self.infer_expr(&arr.elements[0]);

                    // Validate all elements have compatible types
                    for (i, elem) in arr.elements.iter().skip(1).enumerate() {
                        let elem_type = self.infer_expr(elem);
                        if !self.types_compatible(&elem_type, &first_type) {
                            self.errors.push(
                                Diagnostic::error(format!(
                                    "array element {} has type {}, but expected {} (same as first element)",
                                    i + 2, elem_type.name(), first_type.name()
                                ))
                                .with_span(elem.span())
                                .with_help("all array elements must have the same type")
                            );
                        }
                    }

                    ZymbolType::Array(Box::new(first_type))
                }
            }

            Expr::Tuple(tuple) => {
                let types: Vec<ZymbolType> = tuple.elements.iter()
                    .map(|e| self.infer_expr(e))
                    .collect();
                ZymbolType::Tuple(types)
            }

            Expr::NamedTuple(named) => {
                let fields: Vec<(String, ZymbolType)> = named.fields.iter()
                    .map(|(name, expr)| (name.clone(), self.infer_expr(expr)))
                    .collect();
                ZymbolType::NamedTuple(fields)
            }

            Expr::Index(index) => {
                let array_type = self.infer_expr(&index.array);
                let index_type = self.infer_expr(&index.index);

                // Validate index is Int
                if !matches!(index_type, ZymbolType::Int | ZymbolType::Any | ZymbolType::Unknown) {
                    self.errors.push(
                        Diagnostic::error(format!(
                            "array index must be Int, got {}",
                            index_type.name()
                        ))
                        .with_span(index.index.span())
                    );
                }

                match array_type {
                    ZymbolType::Array(elem) => *elem,
                    ZymbolType::String => ZymbolType::Char,
                    ZymbolType::Tuple(types) => {
                        // Try to get index as literal for static validation
                        if let Expr::Literal(lit) = &*index.index {
                            if let Literal::Int(i) = &lit.value {
                                let idx = *i as usize;
                                if idx >= types.len() {
                                    self.errors.push(
                                        Diagnostic::error(format!(
                                            "tuple index {} is out of bounds (tuple has {} elements)",
                                            idx, types.len()
                                        ))
                                        .with_span(index.index.span())
                                    );
                                    return ZymbolType::Any;
                                }
                                if let Some(ty) = types.get(idx) {
                                    return ty.clone();
                                }
                            }
                        }
                        ZymbolType::Any
                    }
                    ZymbolType::NamedTuple(_) => {
                        // Named tuples can be indexed too
                        ZymbolType::Any
                    }
                    _ => ZymbolType::Any,
                }
            }

            Expr::MemberAccess(member) => {
                let obj_type = self.infer_expr(&member.object);
                match obj_type {
                    ZymbolType::NamedTuple(fields) => {
                        fields.iter()
                            .find(|(name, _)| name == &member.field)
                            .map(|(_, ty)| ty.clone())
                            .unwrap_or(ZymbolType::Unknown)
                    }
                    _ => ZymbolType::Any,
                }
            }

            Expr::FunctionCall(call) => {
                // Infer argument types
                let arg_types: Vec<ZymbolType> = call.arguments.iter()
                    .map(|arg| self.infer_expr(arg))
                    .collect();

                // Try to get function signature
                if let Expr::Identifier(ident) = &*call.callable {
                    if let Some((param_types, ret_type)) = self.env.lookup_function(&ident.name).cloned() {
                        // Validate argument count
                        if arg_types.len() != param_types.len() {
                            self.errors.push(
                                Diagnostic::error(format!(
                                    "function '{}' expects {} argument(s), but {} were provided",
                                    ident.name, param_types.len(), arg_types.len()
                                ))
                                .with_span(call.span)
                                .with_help(format!(
                                    "expected signature: {}({})",
                                    ident.name,
                                    param_types.iter().map(|t| t.name()).collect::<Vec<_>>().join(", ")
                                ))
                            );
                        } else {
                            // Validate argument types
                            for (i, (arg_type, param_type)) in arg_types.iter().zip(param_types.iter()).enumerate() {
                                if !self.types_compatible(arg_type, param_type) {
                                    self.errors.push(
                                        Diagnostic::error(format!(
                                            "argument {} has type {}, but function '{}' expects {}",
                                            i + 1, arg_type.name(), ident.name, param_type.name()
                                        ))
                                        .with_span(call.arguments[i].span())
                                    );
                                }
                            }
                        }
                        return ret_type;
                    }
                }
                ZymbolType::Any
            }

            Expr::Lambda(lambda) => {
                // Create function type from lambda structure
                // Without context, parameters are Any
                let param_types: Vec<ZymbolType> = lambda.params.iter()
                    .map(|_| ZymbolType::Any)
                    .collect();

                // Enter a new scope and define lambda parameters so that
                // identifier lookups inside the body don't produce false
                // "undefined variable" errors.
                self.env.enter_scope();
                for param in &lambda.params {
                    self.env.define_var(param, ZymbolType::Any);
                }

                // Infer return type from body
                let return_type = match &lambda.body {
                    zymbol_ast::LambdaBody::Expr(expr) => self.infer_expr(expr),
                    zymbol_ast::LambdaBody::Block(block) => {
                        self.infer_return_type_from_block(block)
                    }
                };

                self.env.exit_scope();

                ZymbolType::Function(param_types, Box::new(return_type))
            }

            Expr::Range(_) => ZymbolType::Array(Box::new(ZymbolType::Int)),

            // Collection operations
            Expr::CollectionLength(_) => ZymbolType::Int,
            Expr::CollectionAppend(op) => {
                let collection_type = self.infer_expr(&op.collection);
                let element_type = self.infer_expr(&op.element);

                // Validate element type matches array element type
                if let ZymbolType::Array(elem_type) = &collection_type {
                    if !self.types_compatible(&element_type, elem_type) {
                        self.errors.push(
                            Diagnostic::error(format!(
                                "cannot append {} to {}: type mismatch",
                                element_type.name(), collection_type.name()
                            ))
                            .with_span(op.element.span())
                            .with_help(format!("expected element of type {}", elem_type.name()))
                        );
                    }
                }

                collection_type
            }
            Expr::CollectionRemove(op) => self.infer_expr(&op.collection),
            Expr::CollectionContains(_) => ZymbolType::Bool,
            Expr::CollectionUpdate(op) => self.infer_expr(&op.target),
            Expr::CollectionSlice(op) => self.infer_expr(&op.collection),
            Expr::CollectionMap(op) => {
                let collection_type = self.infer_expr(&op.collection);
                let lambda_type = self.infer_expr(&op.lambda);

                // Validate lambda
                if let ZymbolType::Function(params, ret) = &lambda_type {
                    if params.len() != 1 {
                        self.errors.push(
                            Diagnostic::error(format!(
                                "map lambda must have exactly 1 parameter, got {}",
                                params.len()
                            ))
                            .with_span(op.lambda.span())
                        );
                    }

                    // Result type is array of lambda return type
                    ZymbolType::Array(ret.clone())
                } else if !matches!(lambda_type, ZymbolType::Any) {
                    self.errors.push(
                        Diagnostic::error(format!(
                            "map operation requires a lambda, got {}",
                            lambda_type.name()
                        ))
                        .with_span(op.lambda.span())
                    );
                    collection_type
                } else {
                    collection_type
                }
            }

            Expr::CollectionFilter(op) => {
                let collection_type = self.infer_expr(&op.collection);
                let lambda_type = self.infer_expr(&op.lambda);

                // Validate lambda
                if let ZymbolType::Function(params, ret) = &lambda_type {
                    if params.len() != 1 {
                        self.errors.push(
                            Diagnostic::error(format!(
                                "filter lambda must have exactly 1 parameter, got {}",
                                params.len()
                            ))
                            .with_span(op.lambda.span())
                        );
                    }

                    // Filter lambda must return Bool
                    if !matches!(**ret, ZymbolType::Bool | ZymbolType::Any | ZymbolType::Unknown) {
                        self.warnings.push(
                            Diagnostic::warning(format!(
                                "filter lambda should return Bool, got {}",
                                ret.name()
                            ))
                            .with_span(op.lambda.span())
                        );
                    }
                } else if !matches!(lambda_type, ZymbolType::Any) {
                    self.errors.push(
                        Diagnostic::error(format!(
                            "filter operation requires a lambda, got {}",
                            lambda_type.name()
                        ))
                        .with_span(op.lambda.span())
                    );
                }

                // Result type is same as collection type
                collection_type
            }

            Expr::CollectionReduce(op) => {
                let _collection_type = self.infer_expr(&op.collection);
                let initial_type = self.infer_expr(&op.initial);
                let lambda_type = self.infer_expr(&op.lambda);

                // Validate lambda
                if let ZymbolType::Function(params, ret) = &lambda_type {
                    if params.len() != 2 {
                        self.errors.push(
                            Diagnostic::error(format!(
                                "reduce lambda must have exactly 2 parameters (acc, elem), got {}",
                                params.len()
                            ))
                            .with_span(op.lambda.span())
                        );
                    }

                    // Result type is lambda return type (should match initial)
                    if !self.types_compatible(&initial_type, ret) {
                        self.warnings.push(
                            Diagnostic::warning(format!(
                                "reduce initial value ({}) may be incompatible with lambda return ({})",
                                initial_type.name(), ret.name()
                            ))
                            .with_span(op.initial.span())
                        );
                    }

                    *ret.clone()
                } else if !matches!(lambda_type, ZymbolType::Any) {
                    self.errors.push(
                        Diagnostic::error(format!(
                            "reduce operation requires a lambda, got {}",
                            lambda_type.name()
                        ))
                        .with_span(op.lambda.span())
                    );
                    initial_type
                } else {
                    initial_type
                }
            }

            // String operations
            Expr::StringFindPositions(_) => ZymbolType::Array(Box::new(ZymbolType::Int)),
            Expr::StringInsert(_) => ZymbolType::String,
            Expr::StringRemove(_) => ZymbolType::String,
            Expr::StringReplace(_) => ZymbolType::String,

            // Data operations
            Expr::NumericEval(_) => ZymbolType::Float,
            Expr::TypeMetadata(_) => ZymbolType::Tuple(vec![ZymbolType::String, ZymbolType::Int, ZymbolType::Any]),
            Expr::Format(_) => ZymbolType::String,
            Expr::BaseConversion(_) => ZymbolType::String,
            Expr::Round(_) | Expr::Trunc(_) => ZymbolType::Float,

            // Error handling
            Expr::ErrorCheck(_) => ZymbolType::Bool,
            Expr::ErrorPropagate(op) => self.infer_expr(&op.expr),

            // Execution
            Expr::Execute(_) | Expr::BashExec(_) => ZymbolType::String,

            // Match expression
            Expr::Match(match_expr) => {
                let scrutinee_type = self.infer_expr(&match_expr.scrutinee);

                // Collect types from all case values
                let mut case_types: Vec<ZymbolType> = Vec::new();

                for case in &match_expr.cases {
                    // Validate pattern type against scrutinee
                    self.check_pattern_type(&case.pattern, &scrutinee_type);

                    // Infer value type
                    if let Some(value) = &case.value {
                        case_types.push(self.infer_expr(value));
                    }

                    // Check block for return statements
                    if let Some(block) = &case.block {
                        self.env.enter_scope();
                        for stmt in &block.statements {
                            self.check_statement(stmt);
                        }
                        let block_type = self.infer_return_type_from_block(block);
                        if !matches!(block_type, ZymbolType::Unit) {
                            case_types.push(block_type);
                        }
                        self.env.exit_scope();
                    }
                }

                // Unify all case types
                if case_types.is_empty() {
                    ZymbolType::Unit
                } else {
                    self.unify_types(&case_types)
                }
            }
            Expr::Pipe(pipe) => {
                // Type of the left side (value being piped)
                let left_type = self.infer_expr(&pipe.left);

                // Type check the callable
                let callable_type = self.infer_expr(&pipe.callable);

                // Determine return type based on callable
                match &callable_type {
                    ZymbolType::Function(params, ret) => {
                        // Count placeholders in arguments
                        let placeholder_count = pipe.arguments.iter()
                            .filter(|a| matches!(a, zymbol_ast::PipeArg::Placeholder))
                            .count();

                        // Validate placeholder count
                        if placeholder_count == 0 {
                            self.warnings.push(
                                Diagnostic::warning("pipe expression has no placeholder '_'")
                                    .with_span(pipe.span)
                                    .with_help("use '_' to indicate where the piped value should go")
                            );
                        } else if placeholder_count > 1 {
                            self.warnings.push(
                                Diagnostic::warning(format!(
                                    "pipe expression has {} placeholders, only first will be used",
                                    placeholder_count
                                ))
                                .with_span(pipe.span)
                            );
                        }

                        // Validate argument types
                        for (arg_index, arg) in pipe.arguments.iter().enumerate() {
                            match arg {
                                zymbol_ast::PipeArg::Placeholder => {
                                    // Placeholder gets the left type
                                    if let Some(param_type) = params.get(arg_index) {
                                        if !self.types_compatible(&left_type, param_type) {
                                            self.errors.push(
                                                Diagnostic::error(format!(
                                                    "piped value type {} incompatible with function parameter {}",
                                                    left_type.name(), param_type.name()
                                                ))
                                                .with_span(pipe.left.span())
                                            );
                                        }
                                    }
                                }
                                zymbol_ast::PipeArg::Expr(e) => {
                                    let arg_type = self.infer_expr(e);
                                    if let Some(param_type) = params.get(arg_index) {
                                        if !self.types_compatible(&arg_type, param_type) {
                                            self.errors.push(
                                                Diagnostic::error(format!(
                                                    "argument type {} incompatible with function parameter {}",
                                                    arg_type.name(), param_type.name()
                                                ))
                                                .with_span(e.span())
                                            );
                                        }
                                    }
                                }
                            }
                        }

                        *ret.clone()
                    }
                    ZymbolType::Any | ZymbolType::Unknown => {
                        // Can't infer, return Any
                        ZymbolType::Any
                    }
                    _ => {
                        self.errors.push(
                            Diagnostic::error(format!(
                                "pipe callable must be a function, got {}",
                                callable_type.name()
                            ))
                            .with_span(pipe.callable.span())
                        );
                        ZymbolType::Any
                    }
                }
            }
        }
    }

    /// Get all collected diagnostics (errors + warnings)
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut all = self.errors.clone();
        all.extend(self.warnings.clone());
        all
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_names() {
        assert_eq!(ZymbolType::Int.name(), "Int");
        assert_eq!(ZymbolType::Array(Box::new(ZymbolType::Int)).name(), "[Int]");
        assert_eq!(ZymbolType::Tuple(vec![ZymbolType::Int, ZymbolType::String]).name(), "(Int, String)");
    }

    #[test]
    fn test_is_numeric() {
        assert!(ZymbolType::Int.is_numeric());
        assert!(ZymbolType::Float.is_numeric());
        assert!(!ZymbolType::String.is_numeric());
        assert!(!ZymbolType::Bool.is_numeric());
    }

    #[test]
    fn test_type_compatibility() {
        assert!(ZymbolType::Int.is_compatible_with(&ZymbolType::Int));
        assert!(ZymbolType::Int.is_compatible_with(&ZymbolType::Float));
        assert!(ZymbolType::Any.is_compatible_with(&ZymbolType::String));
        assert!(!ZymbolType::Int.is_compatible_with(&ZymbolType::String));
    }

    #[test]
    fn test_type_env() {
        let mut env = TypeEnv::new();
        env.define_var("x", ZymbolType::Int);
        assert_eq!(env.lookup_var("x"), Some(&ZymbolType::Int));
        assert_eq!(env.lookup_var("y"), None);

        env.enter_scope();
        env.define_var("y", ZymbolType::String);
        assert_eq!(env.lookup_var("y"), Some(&ZymbolType::String));
        assert_eq!(env.lookup_var("x"), Some(&ZymbolType::Int)); // Still visible

        env.exit_scope();
        assert_eq!(env.lookup_var("y"), None); // Out of scope
    }

    #[test]
    fn test_constant_detection() {
        let mut env = TypeEnv::new();
        env.define_const("PI", ZymbolType::Float);
        assert!(env.is_constant("PI"));
        assert!(!env.is_constant("x"));
    }

    #[test]
    fn test_type_checker_errors_and_warnings_separated() {
        let checker = TypeChecker::new();
        assert!(!checker.has_errors());
        assert!(checker.get_errors().is_empty());
        assert!(checker.get_warnings().is_empty());
    }

    #[test]
    fn test_function_signature() {
        let mut env = TypeEnv::new();
        env.define_function("add", vec![ZymbolType::Int, ZymbolType::Int], ZymbolType::Int);

        let sig = env.lookup_function("add");
        assert!(sig.is_some());
        let (params, ret) = sig.unwrap();
        assert_eq!(params.len(), 2);
        assert_eq!(*ret, ZymbolType::Int);
    }
}
