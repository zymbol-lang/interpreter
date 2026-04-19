//! AST visitor for Zymbol-Lang formatter
//!
//! Walks the AST and emits formatted code.

use zymbol_ast::{
    ArrayLiteralExpr, Assignment, BasePrefix, BinaryExpr, Block, Break, CatchClause,
    CollectionAppendExpr, CollectionContainsExpr, CollectionFilterExpr, CollectionLengthExpr,
    CollectionFindAllExpr, CollectionInsertExpr, CollectionMapExpr, CollectionReduceExpr, CollectionSortExpr,
    CollectionRemoveAllExpr, CollectionRemoveAtExpr, CollectionRemoveRangeExpr,
    CollectionRemoveValueExpr, CollectionSliceExpr,
    CollectionUpdateExpr, ConstDecl, Continue, DestructureAssign, DestructureItem, DestructurePattern,
    ErrorCheckExpr, ErrorPropagateExpr, ErrorType,
    ExportBlock, ExportItem, Expr, ExprStatement, FinallyClause, FormatExpr, FormatKind, PrecisionOp,
    FunctionCallExpr, FunctionDecl, IdentifierExpr, IfStmt, ImportStmt, IndexExpr, Input,
    InputPrompt, ItemType, LambdaBody, LambdaExpr, LifetimeEnd, LiteralExpr, Loop, MatchCase,
    MatchExpr, MemberAccessExpr, ModuleDecl, NamedTupleExpr, NumericEvalExpr, Output,
    Parameter, ParameterKind, Pattern, Program, RangeExpr, ReturnStmt, RoundExpr, Statement,
    StringReplaceExpr, TruncExpr,
    TryStmt, TupleExpr, TypeMetadataExpr, UnaryExpr,
    ExecuteExpr, BashExecExpr, CliArgsCaptureStmt,
};
use zymbol_ast::PipeExpr;
use zymbol_common::{BinaryOp, Literal, UnaryOp};
use zymbol_lexer::StringPart;

use crate::output::OutputBuilder;

/// AST visitor that formats Zymbol code
pub struct FormatVisitor<'a> {
    output: &'a mut OutputBuilder,
}

impl<'a> FormatVisitor<'a> {
    /// Create a new format visitor
    pub fn new(output: &'a mut OutputBuilder) -> Self {
        Self { output }
    }

    /// Format an entire program
    pub fn format_program(&mut self, program: &Program) {
        // Format module declaration if present
        if let Some(ref module_decl) = program.module_decl {
            self.format_module_decl(module_decl);
            self.output.newline();
        }

        // Format imports
        for import in &program.imports {
            self.format_import(import);
            self.output.newline();
        }

        // Add blank line after imports if there are any
        if !program.imports.is_empty() && !program.statements.is_empty() {
            self.output.newline();
        }

        // Format statements
        let mut prev_was_function = false;
        for (i, stmt) in program.statements.iter().enumerate() {
            // Add blank line before function declarations (except first statement)
            let is_function = matches!(stmt, Statement::FunctionDecl(_));
            let is_newline = matches!(stmt, Statement::Newline(_));

            if i > 0 && (is_function || prev_was_function) && !is_newline {
                self.output.newline();
            }

            // Newline statements (¶) should be on the same line as previous statement
            if is_newline && i > 0 {
                // Remove the previous newline and add space + ¶
                self.output.backspace_newline();
                self.output.space();
            }

            self.format_statement(stmt);
            self.output.newline();

            prev_was_function = is_function;
        }
    }

    /// Format a module declaration
    fn format_module_decl(&mut self, decl: &ModuleDecl) {
        self.output.write("# ");
        self.output.write(&decl.name);

        if let Some(ref export_block) = decl.export_block {
            self.output.newline();
            self.output.newline();
            self.format_export_block(export_block);
        }
    }

    /// Format an export block
    fn format_export_block(&mut self, block: &ExportBlock) {
        self.output.write("#>");
        self.output.open_brace();
        self.output.newline();
        self.output.indent();

        for (i, item) in block.items.iter().enumerate() {
            self.format_export_item(item);
            if i < block.items.len() - 1 {
                self.output.write(",");
            }
            self.output.newline();
        }

        self.output.dedent();
        self.output.close_brace();
    }

    /// Format an export item
    fn format_export_item(&mut self, item: &ExportItem) {
        match item {
            ExportItem::Own { name, rename, .. } => {
                self.output.write(name);
                if let Some(alias) = rename {
                    self.output.write(" <= ");
                    self.output.write(alias);
                }
            }
            ExportItem::ReExport {
                module_alias,
                item_name,
                item_type,
                rename,
                ..
            } => {
                self.output.write(module_alias);
                match item_type {
                    ItemType::Function => self.output.write("::"),
                    ItemType::Constant => self.output.write("."),
                }
                self.output.write(item_name);
                if let Some(alias) = rename {
                    self.output.write(" <= ");
                    self.output.write(alias);
                }
            }
        }
    }

    /// Format an import statement
    fn format_import(&mut self, import: &ImportStmt) {
        self.output.write("<# ");

        // Format the path
        let path = &import.path;
        if path.is_relative {
            for _ in 0..path.parent_levels {
                self.output.write("../");
            }
            if path.parent_levels == 0 {
                self.output.write("./");
            }
        }
        self.output.write(&path.components.join("/"));

        self.output.write(" <= ");
        self.output.write(&import.alias);
    }

    /// Format a statement
    pub fn format_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Output(output) => self.format_output(output),
            Statement::Assignment(assign) => self.format_assignment(assign),
            Statement::ConstDecl(decl) => self.format_const_decl(decl),
            Statement::LifetimeEnd(end) => self.format_lifetime_end(end),
            Statement::Input(input) => self.format_input(input),
            Statement::If(if_stmt) => self.format_if(if_stmt),
            Statement::Loop(loop_stmt) => self.format_loop(loop_stmt),
            Statement::Break(brk) => self.format_break(brk),
            Statement::Continue(cont) => self.format_continue(cont),
            Statement::Try(try_stmt) => self.format_try(try_stmt),
            Statement::Newline(_) => self.output.write("¶"),
            Statement::FunctionDecl(decl) => self.format_function_decl(decl),
            Statement::Return(ret) => self.format_return(ret),
            Statement::Match(match_expr) => {
                self.format_match(match_expr);
            }
            Statement::Expr(expr_stmt) => self.format_expr_statement(expr_stmt),
            Statement::DestructureAssign(d) => self.format_destructure_assign(d),
            Statement::CliArgsCapture(capture) => self.format_cli_args_capture(capture),
            Statement::SetNumeralMode { base, .. } => {
                // Reconstruct #<digit0><digit9># from the block base codepoint
                let d0 = char::from_u32(*base).unwrap_or('0');
                let d9 = char::from_u32(base + 9).unwrap_or('9');
                self.output.write(&format!("#{}{}\u{23}", d0, d9));
            }
        }
    }

    /// Format an output statement
    fn format_output(&mut self, output: &Output) {
        self.output.write(">>");
        for expr in &output.exprs {
            self.output.space();
            self.format_expr(expr);
        }
    }

    /// Format an assignment statement
    fn format_assignment(&mut self, assign: &Assignment) {
        self.output.write(&assign.name);
        self.output.write(" = ");
        self.format_expr(&assign.value);
    }

    /// Format a destructure assignment statement
    fn format_destructure_assign(&mut self, d: &DestructureAssign) {
        match &d.pattern {
            DestructurePattern::Array(items) => {
                self.output.write("[");
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.output.write(", ");
                    }
                    match item {
                        DestructureItem::Bind(name) => self.output.write(name),
                        DestructureItem::Rest(name) => {
                            self.output.write("*");
                            self.output.write(name);
                        }
                        DestructureItem::Ignore => self.output.write("_"),
                    }
                }
                self.output.write("]");
            }
            DestructurePattern::Positional(items) => {
                self.output.write("(");
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.output.write(", ");
                    }
                    match item {
                        DestructureItem::Bind(name) => self.output.write(name),
                        DestructureItem::Rest(name) => {
                            self.output.write("*");
                            self.output.write(name);
                        }
                        DestructureItem::Ignore => self.output.write("_"),
                    }
                }
                self.output.write(")");
            }
            DestructurePattern::NamedTuple(fields) => {
                self.output.write("(");
                for (i, (field, var)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.output.write(", ");
                    }
                    self.output.write(field);
                    self.output.write(": ");
                    self.output.write(var);
                }
                self.output.write(")");
            }
        }
        self.output.write(" = ");
        self.format_expr(&d.value);
    }

    /// Format a constant declaration
    fn format_const_decl(&mut self, decl: &ConstDecl) {
        self.output.write(&decl.name);
        self.output.write(" := ");
        self.format_expr(&decl.value);
    }

    /// Format a lifetime end statement
    fn format_lifetime_end(&mut self, end: &LifetimeEnd) {
        self.output.write("\\");
        self.output.write(&end.variable_name);
    }

    /// Format an input statement
    fn format_input(&mut self, input: &Input) {
        self.output.write("<<");
        if let Some(ref prompt) = input.prompt {
            self.output.space();
            match prompt {
                InputPrompt::Simple(s) => {
                    self.output.write("\"");
                    self.output.write(&escape_string(s));
                    self.output.write("\"");
                }
                InputPrompt::Interpolated(parts) => {
                    self.format_interpolated_string(parts);
                }
            }
        }
        self.output.space();
        self.output.write(&input.variable);
    }

    /// Format an if statement
    fn format_if(&mut self, if_stmt: &IfStmt) {
        self.output.write("?");
        self.output.space();
        self.format_expr(&if_stmt.condition);
        self.format_block(&if_stmt.then_block);

        // Format else-if branches
        for branch in &if_stmt.else_if_branches {
            self.output.newline();
            self.output.write("_?");
            self.output.space();
            self.format_expr(&branch.condition);
            self.format_block(&branch.block);
        }

        // Format else block (no space between _ and {)
        if let Some(ref else_block) = if_stmt.else_block {
            self.output.newline();
            self.output.write("_");
            self.format_block_no_leading_space(else_block);
        }
    }

    /// Format a loop statement
    fn format_loop(&mut self, loop_stmt: &Loop) {
        self.output.write("@");

        // Handle labeled loop
        if let Some(ref label) = loop_stmt.label {
            self.output.space();
            self.output.write("@");
            self.output.write(label);
        }

        // Handle for-each loop
        if let Some(ref iter_var) = loop_stmt.iterator_var {
            self.output.space();
            self.output.write(iter_var);
            self.output.write(":");
            if let Some(ref iterable) = loop_stmt.iterable {
                self.format_expr(iterable);
            }
        } else if let Some(ref condition) = loop_stmt.condition {
            // While loop
            self.output.space();
            self.format_expr(condition);
        }
        // Infinite loop has no condition

        self.format_block(&loop_stmt.body);
    }

    /// Format a break statement
    fn format_break(&mut self, brk: &Break) {
        self.output.write("@!");
        if let Some(ref label) = brk.label {
            self.output.space();
            self.output.write(label);
        }
    }

    /// Format a continue statement
    fn format_continue(&mut self, cont: &Continue) {
        self.output.write("@>");
        if let Some(ref label) = cont.label {
            self.output.space();
            self.output.write(label);
        }
    }

    /// Format a try statement
    fn format_try(&mut self, try_stmt: &TryStmt) {
        self.output.write("!?");
        self.format_block(&try_stmt.try_block);

        for catch_clause in &try_stmt.catch_clauses {
            self.output.newline();
            self.format_catch_clause(catch_clause);
        }

        if let Some(ref finally_clause) = try_stmt.finally_clause {
            self.output.newline();
            self.format_finally_clause(finally_clause);
        }
    }

    /// Format a catch clause
    fn format_catch_clause(&mut self, clause: &CatchClause) {
        self.output.write(":!");
        if let Some(ref error_type) = clause.error_type {
            self.output.space();
            self.format_error_type(error_type);
        }
        self.format_block(&clause.block);
    }

    /// Format an error type
    fn format_error_type(&mut self, error_type: &ErrorType) {
        self.output.write("##");
        self.output.write(&error_type.name);
    }

    /// Format a finally clause
    fn format_finally_clause(&mut self, clause: &FinallyClause) {
        self.output.write(":>");
        self.format_block(&clause.block);
    }

    /// Format a function declaration
    fn format_function_decl(&mut self, decl: &FunctionDecl) {
        self.output.write(&decl.name);
        self.output.write("(");

        for (i, param) in decl.parameters.iter().enumerate() {
            self.format_parameter(param);
            if i < decl.parameters.len() - 1 {
                self.output.write(", ");
            }
        }

        self.output.write(")");
        self.format_block(&decl.body);
    }

    /// Format a function parameter
    fn format_parameter(&mut self, param: &Parameter) {
        match param.kind {
            ParameterKind::Normal => {}
            ParameterKind::Mutable => self.output.write("~"),
            ParameterKind::Output => self.output.write("<~"),
        }
        self.output.write(&param.name);
    }

    /// Format a return statement
    fn format_return(&mut self, ret: &ReturnStmt) {
        self.output.write("<~");
        if let Some(ref value) = ret.value {
            self.output.space();
            self.format_expr(value);
        }
    }

    /// Format an expression statement
    fn format_expr_statement(&mut self, stmt: &ExprStatement) {
        self.format_expr(&stmt.expr);
    }

    /// Format CLI args capture statement
    fn format_cli_args_capture(&mut self, capture: &CliArgsCaptureStmt) {
        self.output.write("><");
        self.output.write(&capture.variable_name);
    }

    /// Format a block
    fn format_block(&mut self, block: &Block) {
        self.format_block_internal(block, true)
    }

    /// Format a block without leading space (for else blocks)
    fn format_block_no_leading_space(&mut self, block: &Block) {
        self.format_block_internal(block, false)
    }

    /// Internal block formatting with configurable leading space
    fn format_block_internal(&mut self, block: &Block, leading_space: bool) {
        let config = self.output.config().clone();
        let single_stmt = block.statements.len() == 1;
        let is_simple = single_stmt && self.is_simple_statement(&block.statements[0]);

        if config.inline_single_statement && is_simple {
            // Inline single statement
            if leading_space {
                self.output.write(" { ");
            } else {
                self.output.write("{ ");
            }
            self.format_statement(&block.statements[0]);
            self.output.write(" }");
        } else {
            // Multi-line block
            if leading_space {
                self.output.open_brace();
            } else {
                self.output.write("{");
            }
            self.output.newline();
            self.output.indent();

            for stmt in &block.statements {
                self.format_statement(stmt);
                self.output.newline();
            }

            self.output.dedent();
            self.output.close_brace();
        }
    }

    /// Check if a statement is simple enough to be inlined
    fn is_simple_statement(&self, stmt: &Statement) -> bool {
        matches!(stmt,
            Statement::Output(_)
            | Statement::Assignment(_)
            | Statement::ConstDecl(_)
            | Statement::DestructureAssign(_)
            | Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Return(_)
            | Statement::Newline(_)
            | Statement::Expr(_)
        )
    }

    /// Format an expression
    pub fn format_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(lit) => self.format_literal(lit),
            Expr::Identifier(ident) => self.format_identifier(ident),
            Expr::Binary(binary) => self.format_binary(binary),
            Expr::Unary(unary) => self.format_unary(unary),
            Expr::Range(range) => self.format_range(range),
            Expr::ArrayLiteral(arr) => self.format_array_literal(arr),
            Expr::Tuple(tuple) => self.format_tuple(tuple),
            Expr::NamedTuple(named_tuple) => self.format_named_tuple(named_tuple),
            Expr::MemberAccess(member) => self.format_member_access(member),
            Expr::Index(index) => self.format_index(index),
            Expr::FunctionCall(call) => self.format_function_call(call),
            Expr::Match(match_expr) => self.format_match(match_expr),
            Expr::CollectionLength(op) => self.format_collection_length(op),
            Expr::CollectionAppend(op) => self.format_collection_append(op),
            Expr::CollectionInsert(op) => self.format_collection_insert(op),
            Expr::CollectionRemoveValue(op) => self.format_collection_remove_value(op),
            Expr::CollectionRemoveAll(op) => self.format_collection_remove_all(op),
            Expr::CollectionRemoveAt(op) => self.format_collection_remove_at(op),
            Expr::CollectionRemoveRange(op) => self.format_collection_remove_range(op),
            Expr::CollectionContains(op) => self.format_collection_contains(op),
            Expr::CollectionFindAll(op) => self.format_collection_find_all(op),
            Expr::CollectionUpdate(op) => self.format_collection_update(op),
            Expr::CollectionSlice(op) => self.format_collection_slice(op),
            Expr::StringReplace(op) => self.format_string_replace(op),
            Expr::StringSplit(op) => {
                self.format_expr(&op.string);
                self.output.write("$/ ");
                self.format_expr(&op.delimiter);
            }
            Expr::ConcatBuild(op) => {
                self.format_expr(&op.base);
                self.output.write("$++");
                for item in &op.items {
                    self.output.write(" ");
                    self.format_expr(item);
                }
            }
            Expr::NumericCast(op) => {
                let prefix = match op.kind {
                    zymbol_ast::CastKind::ToFloat    => "##.",
                    zymbol_ast::CastKind::ToIntRound => "###",
                    zymbol_ast::CastKind::ToIntTrunc => "##!",
                };
                self.output.write(prefix);
                self.format_expr(&op.expr);
            }
            Expr::NumericEval(op) => self.format_numeric_eval(op),
            Expr::TypeMetadata(op) => self.format_type_metadata(op),
            Expr::Format(op) => self.format_format_expr(op),
            Expr::BaseConversion(op) => self.format_base_conversion(op),
            Expr::Lambda(lambda) => self.format_lambda(lambda),
            Expr::CollectionMap(op) => self.format_collection_map(op),
            Expr::CollectionFilter(op) => self.format_collection_filter(op),
            Expr::CollectionReduce(op) => self.format_collection_reduce(op),
            Expr::CollectionSortAsc(op) => self.format_collection_sort(op),
            Expr::CollectionSortDesc(op) => self.format_collection_sort(op),
            Expr::CollectionSortCustom(op) => self.format_collection_sort(op),
            Expr::Pipe(pipe) => self.format_pipe(pipe),
            Expr::Execute(exec) => self.format_execute(exec),
            Expr::BashExec(bash) => self.format_bash_exec(bash),
            Expr::Round(round) => self.format_round(round),
            Expr::Trunc(trunc) => self.format_trunc(trunc),
            Expr::ErrorCheck(check) => self.format_error_check(check),
            Expr::ErrorPropagate(prop) => self.format_error_propagate(prop),
            // Multi-dimensional indexing — format as source text (not yet pretty-printed)
            Expr::DeepIndex(_) | Expr::FlatExtract(_) | Expr::StructuredExtract(_) => {}
        }
    }

    /// Format a literal expression
    fn format_literal(&mut self, lit: &LiteralExpr) {
        match &lit.value {
            Literal::Int(n) => self.output.write(&n.to_string()),
            Literal::Float(f) => self.output.write(&format_float(*f)),
            Literal::String(s) | Literal::InterpolatedString(s) => {
                self.output.write("\"");
                self.output.write(&escape_string(s));
                self.output.write("\"");
            }
            Literal::Char(c) => {
                self.output.write("'");
                self.output.write(&escape_char(*c));
                self.output.write("'");
            }
            Literal::Bool(b) => {
                if *b {
                    self.output.write("#1");
                } else {
                    self.output.write("#0");
                }
            }
        }
    }

    /// Format an identifier expression
    fn format_identifier(&mut self, ident: &IdentifierExpr) {
        self.output.write(&ident.name);
    }

    /// Format a binary expression
    fn format_binary(&mut self, binary: &BinaryExpr) {
        // Estimate total length to decide if we need line breaking
        let total_len = self.estimate_binary_length(binary);
        let should_break = self.output.would_exceed_line_length(total_len)
            && !matches!(binary.op, BinaryOp::Range)
            && self.is_breakable_binary(binary);

        // Check if we need parentheses for left operand
        let needs_left_parens = self.needs_parens_for_child(&binary.left, binary.op, true);
        if needs_left_parens {
            self.output.write("(");
        }
        self.format_expr(&binary.left);
        if needs_left_parens {
            self.output.write(")");
        }

        // Format operator with appropriate spacing
        match binary.op {
            BinaryOp::Range => {
                // No spaces around ..
                self.output.write("..");
            }
            BinaryOp::Concat => {
                // Juxtaposition: no explicit operator, just a space separator
                self.output.write(" ");
            }
            _ => {
                if should_break {
                    // Break line after operator
                    self.output.write(" ");
                    self.output.write(&binary.op.to_string());
                    self.output.newline();
                    self.output.indent();
                } else {
                    // Spaces around other operators
                    self.output.write(" ");
                    self.output.write(&binary.op.to_string());
                    self.output.write(" ");
                }
            }
        }

        // Check if we need parentheses for right operand
        let needs_right_parens = self.needs_parens_for_child(&binary.right, binary.op, false);
        if needs_right_parens {
            self.output.write("(");
        }
        self.format_expr(&binary.right);
        if needs_right_parens {
            self.output.write(")");
        }

        if should_break {
            self.output.dedent();
        }
    }

    /// Estimate the length of a binary expression
    fn estimate_binary_length(&self, binary: &BinaryExpr) -> usize {
        let left_len = self.estimate_expr_length(&binary.left);
        let right_len = self.estimate_expr_length(&binary.right);
        let op_len = binary.op.to_string().len() + 2; // spaces around
        left_len + op_len + right_len
    }

    /// Check if a binary expression is worth breaking
    fn is_breakable_binary(&self, binary: &BinaryExpr) -> bool {
        // Only break logical expressions or arithmetic with multiple terms
        matches!(binary.op, BinaryOp::And | BinaryOp::Or)
            || matches!(&*binary.left, Expr::Binary(_))
            || matches!(&*binary.right, Expr::Binary(_))
    }

    /// Check if a child expression needs parentheses
    fn needs_parens_for_child(&self, child: &Expr, parent_op: BinaryOp, is_left: bool) -> bool {
        if let Expr::Binary(child_binary) = child {
            let child_prec = self.operator_precedence(child_binary.op);
            let parent_prec = self.operator_precedence(parent_op);

            if child_prec < parent_prec {
                return true;
            }

            // Handle right associativity for power operator
            if child_prec == parent_prec && !is_left && parent_op == BinaryOp::Pow {
                return false;
            }

            // Same precedence on right side needs parens for left-associative ops
            if child_prec == parent_prec && !is_left {
                return true;
            }
        }
        false
    }

    /// Get operator precedence (higher = binds tighter)
    fn operator_precedence(&self, op: BinaryOp) -> u8 {
        match op {
            BinaryOp::Or => 1,
            BinaryOp::And => 2,
            BinaryOp::Eq | BinaryOp::Neq => 3,
            BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => 4,
            BinaryOp::Add | BinaryOp::Sub => 5,
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => 6,
            BinaryOp::Pow => 7,
            BinaryOp::Pipe => 0,
            BinaryOp::Comma => 0,
            BinaryOp::Range => 8,
            BinaryOp::Concat => 9, // tightest: juxtaposition binds tighter than arithmetic
        }
    }

    /// Format a unary expression
    fn format_unary(&mut self, unary: &UnaryExpr) {
        match unary.op {
            UnaryOp::Neg => self.output.write("-"),
            UnaryOp::Not => self.output.write("!"),
            UnaryOp::Pos => self.output.write("+"),
        }

        // Add parentheses for complex operands
        let needs_parens = matches!(unary.operand.as_ref(), Expr::Binary(_));
        if needs_parens {
            self.output.write("(");
        }
        self.format_expr(&unary.operand);
        if needs_parens {
            self.output.write(")");
        }
    }

    /// Format a range expression
    fn format_range(&mut self, range: &RangeExpr) {
        self.format_expr(&range.start);
        self.output.write("..");
        self.format_expr(&range.end);
        if let Some(ref step) = range.step {
            self.output.write(":");
            self.format_expr(step);
        }
    }

    /// Format an array literal
    fn format_array_literal(&mut self, arr: &ArrayLiteralExpr) {
        let config = self.output.config().clone();
        let should_inline = arr.elements.len() <= config.max_inline_array_elements
            && self.estimate_array_length(arr) <= config.max_inline_array_length;

        if should_inline || arr.elements.is_empty() {
            // Inline format
            self.output.write("[");
            for (i, elem) in arr.elements.iter().enumerate() {
                self.format_expr(elem);
                if i < arr.elements.len() - 1 {
                    self.output.write(", ");
                }
            }
            self.output.write("]");
        } else {
            // Multi-line format
            self.output.write("[");
            self.output.newline();
            self.output.indent();

            for (i, elem) in arr.elements.iter().enumerate() {
                self.format_expr(elem);
                if i < arr.elements.len() - 1 || config.trailing_commas {
                    self.output.write(",");
                }
                self.output.newline();
            }

            self.output.dedent();
            self.output.write("]");
        }
    }

    /// Estimate the length of an array when formatted inline
    fn estimate_array_length(&self, arr: &ArrayLiteralExpr) -> usize {
        let mut len = 2; // brackets
        for (i, elem) in arr.elements.iter().enumerate() {
            len += self.estimate_expr_length(elem);
            if i < arr.elements.len() - 1 {
                len += 2; // ", "
            }
        }
        len
    }

    /// Estimate the length of an expression
    fn estimate_expr_length(&self, expr: &Expr) -> usize {
        match expr {
            Expr::Literal(lit) => match &lit.value {
                Literal::Int(n) => n.to_string().len(),
                Literal::Float(f) => format_float(*f).len(),
                Literal::String(s) | Literal::InterpolatedString(s) => s.len() + 2,
                Literal::Char(_) => 3,
                Literal::Bool(_) => 2,
            },
            Expr::Identifier(ident) => ident.name.len(),
            _ => 20, // Conservative estimate for complex expressions
        }
    }

    /// Format a tuple expression
    fn format_tuple(&mut self, tuple: &TupleExpr) {
        self.output.write("(");

        let estimated_len = self.estimate_args_length(&tuple.elements);
        let should_break = self.output.would_exceed_line_length(estimated_len + 1);

        if should_break && tuple.elements.len() > 2 {
            // Multi-line tuple
            self.output.newline();
            self.output.indent();
            for (i, elem) in tuple.elements.iter().enumerate() {
                self.format_expr(elem);
                if i < tuple.elements.len() - 1 {
                    self.output.write(",");
                    self.output.newline();
                }
            }
            self.output.newline();
            self.output.dedent();
            self.output.write(")");
        } else {
            // Inline tuple
            for (i, elem) in tuple.elements.iter().enumerate() {
                self.format_expr(elem);
                if i < tuple.elements.len() - 1 {
                    self.output.write(", ");
                }
            }
            self.output.write(")");
        }
    }

    /// Format a named tuple expression
    fn format_named_tuple(&mut self, named_tuple: &NamedTupleExpr) {
        self.output.write("(");

        let estimated_len: usize = named_tuple.fields.iter()
            .map(|(name, value)| name.len() + 2 + self.estimate_expr_length(value) + 2)
            .sum();
        let should_break = self.output.would_exceed_line_length(estimated_len + 1);

        if should_break && named_tuple.fields.len() > 1 {
            // Multi-line named tuple
            self.output.newline();
            self.output.indent();
            for (i, (name, value)) in named_tuple.fields.iter().enumerate() {
                self.output.write(name);
                self.output.write(": ");
                self.format_expr(value);
                if i < named_tuple.fields.len() - 1 {
                    self.output.write(",");
                    self.output.newline();
                }
            }
            self.output.newline();
            self.output.dedent();
            self.output.write(")");
        } else {
            // Inline named tuple
            for (i, (name, value)) in named_tuple.fields.iter().enumerate() {
                self.output.write(name);
                self.output.write(": ");
                self.format_expr(value);
                if i < named_tuple.fields.len() - 1 {
                    self.output.write(", ");
                }
            }
            self.output.write(")");
        }
    }

    /// Format a member access expression
    fn format_member_access(&mut self, member: &MemberAccessExpr) {
        self.format_expr(&member.object);
        self.output.write(".");
        self.output.write(&member.field);
    }

    /// Format an index expression
    fn format_index(&mut self, index: &IndexExpr) {
        self.format_expr(&index.array);
        self.output.write("[");
        self.format_expr(&index.index);
        self.output.write("]");
    }

    /// Format a function call expression
    fn format_function_call(&mut self, call: &FunctionCallExpr) {
        self.format_expr(&call.callable);
        self.output.write("(");

        let estimated_len = self.estimate_args_length(&call.arguments);
        let should_break = self.output.would_exceed_line_length(estimated_len + 1);

        if should_break && !call.arguments.is_empty() {
            // Multi-line arguments
            self.output.newline();
            self.output.indent();
            for (i, arg) in call.arguments.iter().enumerate() {
                self.format_expr(arg);
                if i < call.arguments.len() - 1 {
                    self.output.write(",");
                    self.output.newline();
                }
            }
            self.output.newline();
            self.output.dedent();
            self.output.write(")");
        } else {
            // Inline arguments
            for (i, arg) in call.arguments.iter().enumerate() {
                self.format_expr(arg);
                if i < call.arguments.len() - 1 {
                    self.output.write(", ");
                }
            }
            self.output.write(")");
        }
    }

    /// Estimate the length of function arguments
    fn estimate_args_length(&self, args: &[Expr]) -> usize {
        let mut len = 0;
        for (i, arg) in args.iter().enumerate() {
            len += self.estimate_expr_length(arg);
            if i < args.len() - 1 {
                len += 2; // ", "
            }
        }
        len
    }

    /// Format a match expression
    fn format_match(&mut self, match_expr: &MatchExpr) {
        self.output.write("??");
        self.output.space();
        self.format_expr(&match_expr.scrutinee);
        self.output.open_brace();
        self.output.newline();
        self.output.indent();

        // Find max pattern width for alignment
        let max_pattern_width = match_expr
            .cases
            .iter()
            .map(|c| self.estimate_pattern_length(&c.pattern))
            .max()
            .unwrap_or(0);

        for case in &match_expr.cases {
            self.format_match_case(case, max_pattern_width);
            self.output.newline();
        }

        self.output.dedent();
        self.output.close_brace();
    }

    /// Format a match case
    fn format_match_case(&mut self, case: &MatchCase, align_width: usize) {
        let pattern_len = self.estimate_pattern_length(&case.pattern);
        self.format_pattern(&case.pattern);

        // Align colons
        let padding = align_width.saturating_sub(pattern_len);
        for _ in 0..padding {
            self.output.space();
        }

        self.output.write(" : ");

        if let Some(ref value) = case.value {
            self.format_expr(value);
        }

        if let Some(ref block) = case.block {
            self.format_block(block);
        }
    }

    /// Format a pattern
    fn format_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Literal(lit, _) => {
                self.output.write(&lit.to_string());
            }
            Pattern::Range(start, end, _) => {
                self.format_expr(start);
                self.output.write("..");
                self.format_expr(end);
            }
            Pattern::List(patterns, _) => {
                self.output.write("[");
                for (i, p) in patterns.iter().enumerate() {
                    self.format_pattern(p);
                    if i < patterns.len() - 1 {
                        self.output.write(", ");
                    }
                }
                self.output.write("]");
            }
            Pattern::Wildcard(_) => {
                self.output.write("_");
            }
            Pattern::Comparison(op, expr, _) => {
                self.output.write(&op.to_string());
                self.output.write(" ");
                self.format_expr(expr);
            }
            Pattern::Ident(name, _) => {
                self.output.write(name);
            }
        }
    }

    /// Estimate the length of a pattern
    fn estimate_pattern_length(&self, pattern: &Pattern) -> usize {
        Self::estimate_pattern_length_static(pattern)
    }

    fn estimate_pattern_length_static(pattern: &Pattern) -> usize {
        match pattern {
            Pattern::Literal(lit, _) => lit.to_string().len(),
            Pattern::Range(_, _, _) => 7,
            Pattern::List(patterns, _) => {
                2 + patterns.iter().map(|p| Self::estimate_pattern_length_static(p) + 2).sum::<usize>()
            }
            Pattern::Wildcard(_) => 1,
            Pattern::Comparison(_, _, _) => 6,
            Pattern::Ident(name, _) => name.len(),
        }
    }

    /// Format collection length operation
    fn format_collection_length(&mut self, op: &CollectionLengthExpr) {
        self.format_expr(&op.collection);
        self.output.write("$#");
    }

    /// Format collection append operation
    fn format_collection_append(&mut self, op: &CollectionAppendExpr) {
        self.format_expr(&op.collection);
        self.output.write("$+ ");
        self.format_expr(&op.element);
    }

    /// Format collection insert operation: collection$+[index] element
    fn format_collection_insert(&mut self, op: &CollectionInsertExpr) {
        self.format_expr(&op.collection);
        self.output.write("$+[");
        self.format_expr(&op.index);
        self.output.write("] ");
        self.format_expr(&op.element);
    }

    /// Format collection remove value operation: collection$- value
    fn format_collection_remove_value(&mut self, op: &CollectionRemoveValueExpr) {
        self.format_expr(&op.collection);
        self.output.write("$- ");
        self.format_expr(&op.value);
    }

    /// Format collection remove all operation: collection$-- value
    fn format_collection_remove_all(&mut self, op: &CollectionRemoveAllExpr) {
        self.format_expr(&op.collection);
        self.output.write("$-- ");
        self.format_expr(&op.value);
    }

    /// Format collection remove at operation: collection$-[index]
    fn format_collection_remove_at(&mut self, op: &CollectionRemoveAtExpr) {
        self.format_expr(&op.collection);
        self.output.write("$-[");
        self.format_expr(&op.index);
        self.output.write("]");
    }

    /// Format collection remove range operation: collection$-[start..end]
    fn format_collection_remove_range(&mut self, op: &CollectionRemoveRangeExpr) {
        self.format_expr(&op.collection);
        self.output.write("$-[");
        if let Some(ref start) = op.start {
            self.format_expr(start);
        }
        if op.count_based {
            // [start:count] form — preserve as written
            self.output.write(":");
            if let Some(ref count) = op.end {
                self.format_expr(count);
            }
        } else {
            self.output.write("..");
            if let Some(ref end) = op.end {
                self.format_expr(end);
            }
        }
        self.output.write("]");
    }

    /// Format collection find all operation: collection$?? value
    fn format_collection_find_all(&mut self, op: &CollectionFindAllExpr) {
        self.format_expr(&op.collection);
        self.output.write("$?? ");
        self.format_expr(&op.value);
    }

    /// Format collection contains operation
    fn format_collection_contains(&mut self, op: &CollectionContainsExpr) {
        self.format_expr(&op.collection);
        self.output.write("$? ");
        self.format_expr(&op.element);
    }

    /// Format collection update operation
    fn format_collection_update(&mut self, op: &CollectionUpdateExpr) {
        self.format_expr(&op.target);
        self.output.write("$~ ");
        self.format_expr(&op.value);
    }

    /// Format collection slice operation
    fn format_collection_slice(&mut self, op: &CollectionSliceExpr) {
        self.format_expr(&op.collection);
        self.output.write("$[");
        if let Some(ref start) = op.start {
            self.format_expr(start);
        }
        if op.count_based {
            // [start:count] form — preserve as written
            self.output.write(":");
            if let Some(ref count) = op.end {
                self.format_expr(count);
            }
        } else {
            self.output.write("..");
            if let Some(ref end) = op.end {
                self.format_expr(end);
            }
        }
        self.output.write("]");
    }

    /// Format string replace operation
    fn format_string_replace(&mut self, op: &StringReplaceExpr) {
        self.format_expr(&op.string);
        self.output.write("$~~[");
        self.format_expr(&op.pattern);
        self.output.write(":");
        self.format_expr(&op.replacement);
        if let Some(ref count) = op.count {
            self.output.write(":");
            self.format_expr(count);
        }
        self.output.write("]");
    }

    /// Format numeric eval operation
    fn format_numeric_eval(&mut self, op: &NumericEvalExpr) {
        self.output.write("#|");
        self.format_expr(&op.expr);
        self.output.write("|");
    }

    /// Format type metadata operation
    fn format_type_metadata(&mut self, op: &TypeMetadataExpr) {
        self.format_expr(&op.expr);
        self.output.write("#?");
    }

    /// Format format expression: #,|expr|, #^|expr|, #,.2|expr|, etc.
    fn format_format_expr(&mut self, op: &FormatExpr) {
        match op.kind {
            FormatKind::Thousands => self.output.write("#,"),
            FormatKind::Scientific => self.output.write("#^"),
        }
        match op.precision {
            Some(PrecisionOp::Round(n)) => self.output.write(&format!(".{}", n)),
            Some(PrecisionOp::Truncate(n)) => self.output.write(&format!("!{}", n)),
            None => {}
        }
        self.output.write("|");
        self.format_expr(&op.expr);
        self.output.write("|");
    }

    /// Format base conversion expression
    fn format_base_conversion(&mut self, op: &zymbol_ast::BaseConversionExpr) {
        match op.prefix {
            BasePrefix::Binary => self.output.write("0b|"),
            BasePrefix::Octal => self.output.write("0o|"),
            BasePrefix::Decimal => self.output.write("0d|"),
            BasePrefix::Hex => self.output.write("0x|"),
        }
        self.format_expr(&op.expr);
        self.output.write("|");
    }

    /// Format a lambda expression
    fn format_lambda(&mut self, lambda: &LambdaExpr) {
        if lambda.params.len() == 1 {
            self.output.write(&lambda.params[0]);
        } else {
            self.output.write("(");
            for (i, param) in lambda.params.iter().enumerate() {
                self.output.write(param);
                if i < lambda.params.len() - 1 {
                    self.output.write(", ");
                }
            }
            self.output.write(")");
        }

        self.output.write(" -> ");

        match &lambda.body {
            LambdaBody::Expr(expr) => self.format_expr(expr),
            LambdaBody::Block(block) => self.format_block(block),
        }
    }

    /// Format collection map operation
    fn format_collection_map(&mut self, op: &CollectionMapExpr) {
        self.format_expr(&op.collection);
        self.output.write("$> (");
        self.format_expr(&op.lambda);
        self.output.write(")");
    }

    /// Format collection filter operation
    fn format_collection_filter(&mut self, op: &CollectionFilterExpr) {
        self.format_expr(&op.collection);
        self.output.write("$| (");
        self.format_expr(&op.lambda);
        self.output.write(")");
    }

    /// Format collection sort operation
    fn format_collection_sort(&mut self, op: &CollectionSortExpr) {
        self.format_expr(&op.collection);
        let sym = if op.ascending { "$^+" } else { "$^-" };
        self.output.write(sym);
        if let Some(ref cmp) = op.comparator {
            self.output.write(" (");
            self.format_expr(cmp);
            self.output.write(")");
        }
    }

    /// Format collection reduce operation
    fn format_collection_reduce(&mut self, op: &CollectionReduceExpr) {
        self.format_expr(&op.collection);
        self.output.write("$< (");
        self.format_expr(&op.initial);
        self.output.write(", ");
        self.format_expr(&op.lambda);
        self.output.write(")");
    }

    /// Format pipe expression
    fn format_pipe(&mut self, pipe: &PipeExpr) {
        self.format_expr(&pipe.left);
        self.output.write(" |> ");
        self.format_expr(&pipe.callable);
        self.output.write("(");
        for (i, arg) in pipe.arguments.iter().enumerate() {
            match arg {
                zymbol_ast::PipeArg::Placeholder => self.output.write("_"),
                zymbol_ast::PipeArg::Expr(expr) => self.format_expr(expr),
            }
            if i < pipe.arguments.len() - 1 {
                self.output.write(", ");
            }
        }
        self.output.write(")");
    }

    /// Format execute expression
    fn format_execute(&mut self, exec: &ExecuteExpr) {
        self.output.write("</");
        self.output.write(&exec.path);
        self.output.write("/>");
    }

    /// Format bash execute expression
    fn format_bash_exec(&mut self, bash: &BashExecExpr) {
        self.output.write("<\\ ");
        for (i, arg) in bash.args.iter().enumerate() {
            if i > 0 {
                self.output.write(" ");
            }
            self.format_expr(arg);
        }
        self.output.write(" \\>");
    }

    /// Format round expression
    fn format_round(&mut self, round: &RoundExpr) {
        self.output.write("#.");
        self.output.write(&round.precision.to_string());
        self.output.write("|");
        self.format_expr(&round.expr);
        self.output.write("|");
    }

    /// Format trunc expression
    fn format_trunc(&mut self, trunc: &TruncExpr) {
        self.output.write("#!");
        self.output.write(&trunc.precision.to_string());
        self.output.write("|");
        self.format_expr(&trunc.expr);
        self.output.write("|");
    }

    /// Format error check expression
    fn format_error_check(&mut self, check: &ErrorCheckExpr) {
        self.format_expr(&check.expr);
        self.output.write("$!");
    }

    /// Format error propagate expression
    fn format_error_propagate(&mut self, prop: &ErrorPropagateExpr) {
        self.format_expr(&prop.expr);
        self.output.write("$!!");
    }

    /// Format interpolated string
    fn format_interpolated_string(&mut self, parts: &[StringPart]) {
        self.output.write("\"");
        for part in parts {
            match part {
                StringPart::Text(text) => {
                    self.output.write(&escape_string(text));
                }
                StringPart::Variable(var) => {
                    self.output.write("{");
                    self.output.write(var);
                    self.output.write("}");
                }
            }
        }
        self.output.write("\"");
    }
}

/// Escape a string for output
fn escape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\n'   => result.push_str("\\n"),
            '\r'   => result.push_str("\\r"),
            '\t'   => result.push_str("\\t"),
            '\\'   => result.push_str("\\\\"),
            '"'    => result.push_str("\\\""),
            // \x01 is the sentinel for \{ (escaped brace) — restore to source form
            '\x01' => result.push_str("\\{"),
            // { and } are NOT escaped: plain strings have no real {, interpolated strings
            // need { as-is for variable interpolation markers
            _ => result.push(ch),
        }
    }
    result
}

/// Escape a char for output
fn escape_char(c: char) -> String {
    match c {
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        '\\' => "\\\\".to_string(),
        '\'' => "\\'".to_string(),
        _ => c.to_string(),
    }
}

/// Format a float, removing unnecessary trailing zeros
fn format_float(f: f64) -> String {
    // Check if it's a whole number
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}.0", f as i64)
    } else {
        // Use scientific notation for very large/small numbers
        if f.abs() >= 1e15 || (f != 0.0 && f.abs() < 1e-4) {
            format!("{:e}", f)
        } else {
            let s = format!("{}", f);
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("hello"), "hello");
        assert_eq!(escape_string("hello\nworld"), "hello\\nworld");
        assert_eq!(escape_string("say \"hi\""), "say \\\"hi\\\"");
    }

    #[test]
    fn test_escape_char() {
        assert_eq!(escape_char('a'), "a");
        assert_eq!(escape_char('\n'), "\\n");
        assert_eq!(escape_char('\''), "\\'");
    }

    #[test]
    fn test_format_float() {
        #[allow(clippy::approx_constant)]
        let val = 3.14;
        assert_eq!(format_float(val), "3.14");
        assert_eq!(format_float(42.0), "42.0");
    }
}
