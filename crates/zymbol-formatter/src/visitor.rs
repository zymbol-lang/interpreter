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
    MatchExpr, MemberAccessExpr, NamedTupleExpr, NumericEvalExpr, Output,
    Parameter, ParameterKind, Pattern, Program, RangeExpr, ReturnStmt, RoundExpr, Statement,
    StringReplaceExpr, TruncExpr,
    TryStmt, TupleExpr, TypeMetadataExpr, UnaryExpr,
    ExecuteExpr, BashExecExpr, CliArgsCaptureStmt,
    DeepIndexExpr, FlatExtractExpr, StructuredExtractExpr,
    NavStep, NavPath, ExtractGroup,
};
use zymbol_ast::AssignSugar;
use zymbol_ast::InputCast;
use zymbol_ast::PipeExpr;

use crate::comments::{Comment, CommentStream};

/// Surface token for a compound-assignment operator (`+=`, `-=`, …).
fn compound_op_str(op: BinaryOp) -> Option<&'static str> {
    match op {
        BinaryOp::Add => Some("+="),
        BinaryOp::Sub => Some("-="),
        BinaryOp::Mul => Some("*="),
        BinaryOp::Div => Some("/="),
        BinaryOp::Mod => Some("%="),
        BinaryOp::Pow => Some("^="),
        _ => None,
    }
}
use zymbol_common::{BinaryOp, Literal, UnaryOp};
use zymbol_lexer::StringPart;

use crate::output::OutputBuilder;

/// AST visitor that formats Zymbol code
pub struct FormatVisitor<'a> {
    output: &'a mut OutputBuilder,
    /// Source comments in span order, consumed as statements are emitted.
    comments: CommentStream,
    /// Source end-line of the last emitted statement or comment; drives
    /// blank-line preservation (gap > 1 in the source → one blank line).
    last_src_line: u32,
    /// The last emitted line ended with a trailing `// comment` — joining
    /// anything onto it would swallow the joined token into the comment.
    last_had_trailing: bool,
}

impl<'a> FormatVisitor<'a> {
    /// Create a new format visitor
    pub fn new(output: &'a mut OutputBuilder, comments: CommentStream) -> Self {
        Self { output, comments, last_src_line: 0, last_had_trailing: false }
    }

    // ── Comment emission (span-ordered; replaces the old merge_comments) ────

    /// Write one comment (no trailing newline). Block-comment continuation
    /// lines lose the opening line's original indentation and inherit the
    /// current indent from the OutputBuilder (spec §9.3).
    fn emit_comment(&mut self, c: &Comment) {
        if !c.is_block {
            self.output.write("//");
            self.output.write(&c.text);
            return;
        }
        self.output.write("/*");
        let strip = c.start_col.saturating_sub(1) as usize;
        let mut first = true;
        for line in c.text.split('\n') {
            if first {
                self.output.write(line);
                first = false;
            } else {
                self.output.newline();
                let mut rest = line;
                for _ in 0..strip {
                    match rest.strip_prefix(' ') {
                        Some(r) => rest = r,
                        None => break,
                    }
                }
                self.output.write(rest);
            }
        }
        self.output.write("*/");
    }

    /// Emit every standalone comment that starts before `line`, preserving
    /// one blank line where the source had a gap.
    fn flush_comments_before(&mut self, line: u32) {
        while let Some(c) = self.comments.next_before_line(line) {
            if self.last_src_line > 0 && c.start_line > self.last_src_line + 1 {
                self.output.newline();
            }
            self.emit_comment(&c);
            self.output.newline();
            self.last_src_line = c.end_line;
        }
    }

    /// Emit comments trailing on `end_line` (call after the statement text,
    /// before its newline).
    fn emit_trailing_comments(&mut self, end_line: u32) {
        self.last_had_trailing = false;
        while let Some(c) = self.comments.next_on_line(end_line) {
            self.output.write(" ");
            self.emit_comment(&c);
            self.last_src_line = self.last_src_line.max(c.end_line);
            if !c.is_block {
                self.last_had_trailing = true;
            }
        }
    }

    /// One blank line when the source had one or more blank lines before `line`.
    fn blank_gap_before(&self, line: u32) -> bool {
        self.last_src_line > 0 && line > self.last_src_line + 1
    }

    /// Emit the module export block like a statement: comments before it,
    /// source-gap blank line, trailing comments, newline.
    fn emit_export_block_stmt(&mut self, eb: &ExportBlock) {
        self.flush_comments_before(eb.span.start.line);
        if self.blank_gap_before(eb.span.start.line) {
            self.output.newline();
        }
        self.format_export_block(eb);
        self.last_src_line = eb.span.end.line;
        self.emit_trailing_comments(eb.span.end.line);
        self.output.newline();
    }

    /// Format an entire program
    pub fn format_program(&mut self, program: &Program) {
        let in_module = program.module_decl.is_some();

        if let Some(ref module_decl) = program.module_decl {
            // Header comments above `# name {`
            self.flush_comments_before(module_decl.span.start.line);
            self.output.write("# ");
            self.output.write(&module_decl.name);
            self.output.open_brace();
            self.output.newline();
            self.output.indent();
            self.last_src_line = module_decl.span.start.line;

            // Imports first (as they appear in source), then export block
            for import in &program.imports {
                self.flush_comments_before(import.span.start.line);
                self.format_import(import);
                self.last_src_line = import.span.end.line;
                self.emit_trailing_comments(import.span.end.line);
                self.output.newline();
            }

            // The export block is NOT printed here: it is emitted in source
            // order relative to the module's statements (see the loop below),
            // because modules may declare constants before `#> { ... }`.
        } else {
            // Non-module file: imports at top level
            for import in &program.imports {
                self.flush_comments_before(import.span.start.line);
                self.format_import(import);
                self.last_src_line = import.span.end.line;
                self.emit_trailing_comments(import.span.end.line);
                self.output.newline();
            }
        }

        // Blank line after imports comes from source-gap preservation below
        // (an unconditional blank here would stack with the gap blank and
        // break idempotence).

        // Export block pending emission at its source position
        let mut pending_export = program
            .module_decl
            .as_ref()
            .and_then(|m| m.export_block.as_ref());

        // Format statements
        let mut prev_was_function = false;
        let mut prev_was_newline = false;
        let mut prev_stmt: Option<&Statement> = None;
        for (i, stmt) in program.statements.iter().enumerate() {
            if let Some(eb) = pending_export {
                if eb.span.start.line < stmt.span().start.line {
                    self.emit_export_block_stmt(eb);
                    pending_export = None;
                }
            }
            let is_function = matches!(stmt, Statement::FunctionDecl(_));
            let is_newline = matches!(stmt, Statement::Newline(_));
            let join_output = Self::joins_previous_output(prev_stmt, stmt);

            // Join ¶ onto the previous line (only when prev wasn't already a ¶),
            // and chained outputs that shared a source line: `>> a >> b`.
            // Never join onto a line that ends in a `//` comment.
            let join = ((is_newline && i > 0 && !prev_was_newline) || join_output)
                && !self.last_had_trailing;

            if join {
                self.output.backspace_newline();
                self.output.space();
            } else {
                let line = stmt.span().start.line;
                self.flush_comments_before(line);
                // Blank line: preserved source gap, or §5.3 around functions
                let func_blank = i > 0 && (is_function || prev_was_function) && !is_newline;
                if self.blank_gap_before(line) || func_blank {
                    self.output.newline();
                }
            }

            self.format_statement(stmt);
            self.last_src_line = stmt.span().end.line;
            self.emit_trailing_comments(stmt.span().end.line);
            self.output.newline();

            prev_was_function = is_function;
            prev_was_newline = is_newline;
            prev_stmt = Some(stmt);
        }

        // Export block after every statement (or module with no statements)
        if let Some(eb) = pending_export {
            self.emit_export_block_stmt(eb);
        }

        if in_module {
            // Comments between the last statement and the module's closing }
            if let Some(ref module_decl) = program.module_decl {
                self.flush_comments_before(module_decl.span.end.line);
            }
            self.output.dedent();
            self.output.close_brace();
            self.output.newline();
        }

        // End-of-file flush: comments after the last statement
        for c in self.comments.drain_rest() {
            if self.last_src_line > 0 && c.start_line > self.last_src_line + 1 {
                self.output.newline();
            }
            self.emit_comment(&c);
            self.output.newline();
            self.last_src_line = c.end_line;
        }
    }

    /// Format an export block, reprinting the user's optional `,` separators
    /// and keeping single-line blocks (`#> { add, PI }`) on one line.
    fn format_export_block(&mut self, block: &ExportBlock) {
        let comma_after = |i: usize| block.commas.get(i).copied().unwrap_or(false);

        if block.span.start.line == block.span.end.line {
            self.output.write("#> { ");
            for (i, item) in block.items.iter().enumerate() {
                if i > 0 {
                    self.output.write(" ");
                }
                self.format_export_item(item);
                if comma_after(i) {
                    self.output.write(",");
                }
            }
            self.output.write(" }");
            return;
        }

        self.output.write("#>");
        self.output.open_brace();
        self.output.newline();
        self.output.indent();

        for (i, item) in block.items.iter().enumerate() {
            self.format_export_item(item);
            if comma_after(i) {
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
                    self.output.write(" => ");
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
                    self.output.write(" => ");
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

        self.output.write(" => ");
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
            Statement::Newline(nl) => {
                self.output.write(if nl.backslash { "\\\\" } else { "¶" })
            }
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
            Statement::Sleep(s) => {
                self.output.write("@~ ");
                self.format_expr(&s.duration);
            }
            Statement::ClearScreen(_) => self.output.write(">>!"),
            Statement::KeyInput(ki) => {
                if ki.blocking {
                    self.output.write(&format!("<<| {}", ki.variable));
                } else {
                    self.output.write(&format!("<<|? {}", ki.variable));
                }
            }
            Statement::OutputPos(op) => {
                if op.parenthesized {
                    self.output.write(">>~ (");
                    let mut first = true;
                    for slot in &op.slots {
                        if !first { self.output.write(", "); }
                        first = false;
                        if let Some(expr) = slot { self.format_expr(expr); }
                    }
                    self.output.write(") >");
                } else {
                    // Bare-variable form: >>~ pos > items
                    self.output.write(">>~ ");
                    if let Some(Some(expr)) = op.slots.first() {
                        self.format_expr(expr);
                    }
                    self.output.write(" >");
                }
                for item in &op.items {
                    self.output.write(" ");
                    self.format_expr(item);
                }
            }
            Statement::TuiBlock(tb) => {
                self.output.write(">>| ");
                self.format_block(&tb.body);
            }
        }
    }

    /// Format an output statement
    fn format_output(&mut self, output: &Output) {
        self.output.write(">>");
        for expr in &output.exprs {
            self.output.space();
            // Only && and || cause a parse error in >> (parser sees two items).
            // Arithmetic binaries parse fine without parens — do not add them (§11).
            let needs_parens = matches!(
                expr,
                Expr::Binary(b) if matches!(b.op, BinaryOp::And | BinaryOp::Or)
            );
            if needs_parens { self.output.write("("); }
            self.format_expr(expr);
            if needs_parens { self.output.write(")"); }
        }
    }

    /// Format an assignment statement, reprinting the surface form the user
    /// wrote (`x += 1`, `x++`) from the parser's `sugar` record. The value
    /// itself is always the desugared Binary, so when the sugar shape does not
    /// match (defensive), fall back to plain `=` printing — the safety gate
    /// rejects any unfaithful result.
    fn format_assignment(&mut self, assign: &Assignment) {
        // A bare `$` edit statement reprints as the expression it was written
        // as — `arr$+ 3`, not `arr = arr$+ 3`. The receiver is already inside
        // `value`, so this has to run before the name is written.
        if assign.sugar == AssignSugar::InPlaceEdit {
            self.format_expr(&assign.value);
            return;
        }
        if assign.pre_hot {
            self.output.write("°");
        }
        self.output.write(&assign.name);
        if assign.hot {
            self.output.write("°");
        }

        match assign.sugar {
            AssignSugar::Increment => {
                self.output.write("++");
                return;
            }
            AssignSugar::Decrement => {
                self.output.write("--");
                return;
            }
            AssignSugar::Compound(op) => {
                if let Expr::Binary(bin) = &assign.value {
                    if bin.op == op {
                        if let Some(op_str) = compound_op_str(op) {
                            self.output.write(" ");
                            self.output.write(op_str);
                            self.output.write(" ");
                            self.format_expr(&bin.right);
                            return;
                        }
                    }
                }
                // Shape mismatch — fall through to plain printing
            }
            AssignSugar::IndexedAssign => {
                // name[i] = rhs   (value is CollectionUpdate{target: Index, value: rhs})
                if let Expr::CollectionUpdate(cu) = &assign.value {
                    if let Expr::Index(idx) = cu.target.as_ref() {
                        self.output.write("[");
                        self.format_expr(&idx.index);
                        self.output.write("] = ");
                        self.format_expr(&cu.value);
                        return;
                    }
                }
            }
            AssignSugar::IndexedCompound(op) => {
                // name[i] op= rhs (value is CollectionUpdate{target: Index,
                //                  value: Binary{op, left: Index, right: rhs}})
                if let Expr::CollectionUpdate(cu) = &assign.value {
                    if let (Expr::Index(idx), Expr::Binary(bin)) =
                        (cu.target.as_ref(), cu.value.as_ref())
                    {
                        if bin.op == op {
                            if let Some(op_str) = compound_op_str(op) {
                                self.output.write("[");
                                self.format_expr(&idx.index);
                                self.output.write("] ");
                                self.output.write(op_str);
                                self.output.write(" ");
                                self.format_expr(&bin.right);
                                return;
                            }
                        }
                    }
                }
            }
            AssignSugar::InPlaceEdit => unreachable!("handled above"),
            AssignSugar::None => {}
        }

        self.output.write(" = ");
        self.format_expr(&assign.value);
    }

    /// Format a destructure assignment statement
    fn format_destructure_assign(&mut self, d: &DestructureAssign) {
        self.format_destructure_pattern(&d.pattern);
        self.output.write(" = ");
        self.format_expr(&d.value);
    }

    /// Format a destructuring pattern on its own — shared by the assignment and
    /// by a loop head, `@ (k, v):pares`, which uses the very same pattern
    /// language and would otherwise have needed a second copy of this.
    fn format_destructure_pattern(&mut self, pattern: &DestructurePattern) {
        match pattern {
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

    /// Format an input statement: `<< [typespec] ["prompt"] var` or `<< #|var|`
    fn format_input(&mut self, input: &Input) {
        self.output.write("<<");

        // Typespec cast goes first, before the prompt (parser order)
        match &input.cast {
            InputCast::String | InputCast::Numeric => {}
            InputCast::Float => {
                self.output.space();
                self.output.write("##.");
            }
            InputCast::Decimal { total, decimals } => {
                self.output.space();
                self.output.write(&format!("##.({},{})", total, decimals));
            }
            InputCast::Int { max_digits } => {
                self.output.space();
                match max_digits {
                    Some(n) => self.output.write(&format!("###({})", n)),
                    None => self.output.write("###"),
                }
            }
            InputCast::Text { max } => {
                self.output.space();
                match max {
                    Some(n) => self.output.write(&format!("##\"({})", n)),
                    None => self.output.write("##\""),
                }
            }
            InputCast::Char => {
                self.output.space();
                self.output.write("##'");
            }
        }

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
        if matches!(input.cast, InputCast::Numeric) {
            // Legacy numeric form wraps the variable: `<< #|var|`
            self.output.write("#|");
            self.output.write(&input.variable);
            self.output.write("|");
        } else {
            self.output.write(&input.variable);
        }
    }

    /// Format an if statement
    fn format_if(&mut self, if_stmt: &IfStmt) {
        self.output.write("?");
        self.output.space();
        self.format_expr(&if_stmt.condition);

        self.format_block(&if_stmt.then_block);

        // Format else-if branches
        for branch in &if_stmt.else_if_branches {
            self.output.write(" _?");
            self.output.space();
            self.format_expr(&branch.condition);
            self.format_block(&branch.block);
        }

        // Format else block — always continue on same line as closing }
        if let Some(ref else_block) = if_stmt.else_block {
            self.output.write(" _");
            self.format_block(else_block);
        }
    }

    /// Format a loop statement
    fn format_loop(&mut self, loop_stmt: &Loop) {
        self.output.write("@");

        // Labeled loop: @:label
        if let Some(ref label) = loop_stmt.label {
            self.output.write(":");
            self.output.write(label);
        }

        // Handle for-each loop
        if let Some(ref pattern) = loop_stmt.iterator_pattern {
            // `@ (k, v):pares` — a pattern where a single name would go.
            self.output.space();
            self.format_destructure_pattern(pattern);
            self.output.write(":");
            if let Some(ref iterable) = loop_stmt.iterable {
                self.format_expr(iterable);
            }
        } else if let Some(ref iter_var) = loop_stmt.iterator_var {
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
        if let Some(ref label) = brk.label {
            self.output.write("@:");
            self.output.write(label);
            self.output.write("!");
        } else {
            self.output.write("@!");
        }
    }

    /// Format a continue statement
    fn format_continue(&mut self, cont: &Continue) {
        if let Some(ref label) = cont.label {
            self.output.write("@:");
            self.output.write(label);
            self.output.write(">");
        } else {
            self.output.write("@>");
        }
    }

    /// Format a try statement
    fn format_try(&mut self, try_stmt: &TryStmt) {
        self.output.write("!?");
        self.format_block(&try_stmt.try_block);

        // :! and :> appear on the same line as the preceding }, like } _ { (else) in §5.2
        for catch_clause in &try_stmt.catch_clauses {
            self.output.write(" ");
            self.format_catch_clause(catch_clause);
        }

        if let Some(ref finally_clause) = try_stmt.finally_clause {
            self.output.write(" ");
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
            ParameterKind::Normal => {
                self.output.write(&param.name);
            }
            ParameterKind::Mutable => {
                // Suffix form: `name~` (the parser only accepts the suffix)
                self.output.write(&param.name);
                self.output.write("~");
            }
            ParameterKind::Output => {
                self.output.write(&param.name);
                self.output.write("<~");
            }
        }
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

    fn format_block(&mut self, block: &Block) {
        let config = self.output.config().clone();
        let single_stmt = block.statements.len() == 1;
        let is_simple = single_stmt && self.is_simple_statement(&block.statements[0]);
        // A block holding a comment must stay multi-line so the comment has a home
        let has_comment = self
            .comments
            .has_within(block.span.start.line, block.span.end.line);

        if config.inline_single_statement && is_simple && !has_comment {
            self.output.write(" { ");
            self.format_statement(&block.statements[0]);
            self.output.write(" }");
        } else {
            self.output.open_brace();
            self.output.newline();
            self.output.indent();
            self.last_src_line = self.last_src_line.max(block.span.start.line);

            let stmts = &block.statements;
            let mut i = 0;
            let mut prev_was_newline = false;
            while i < stmts.len() {
                let is_newline = matches!(stmts[i], Statement::Newline(_));
                let join_output = i > 0
                    && Self::joins_previous_output(Some(&stmts[i - 1]), &stmts[i]);
                let join = ((is_newline && i > 0 && !prev_was_newline) || join_output)
                    && !self.last_had_trailing;
                if join {
                    self.output.backspace_newline();
                    self.output.space();
                } else {
                    let line = stmts[i].span().start.line;
                    self.flush_comments_before(line);
                    if self.blank_gap_before(line) {
                        self.output.newline();
                    }
                }
                self.format_statement(&stmts[i]);
                self.last_src_line = stmts[i].span().end.line;
                self.emit_trailing_comments(stmts[i].span().end.line);
                self.output.newline();
                prev_was_newline = is_newline;
                i += 1;
            }

            // Comments between the last statement and the closing }
            self.flush_comments_before(block.span.end.line);

            self.output.dedent();
            self.output.close_brace();
        }
    }

    /// Chained outputs written on one source line (`>> a >> b ¶`) parse as
    /// separate Output statements; keep them on one formatted line.
    fn joins_previous_output(prev: Option<&Statement>, current: &Statement) -> bool {
        match (prev, current) {
            (Some(p @ Statement::Output(_)), Statement::Output(_)) => {
                p.span().end.line == current.span().start.line
            }
            _ => false,
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
            | Statement::Expr(_)
        )
    }

    /// Format an expression
    pub fn format_expr(&mut self, expr: &Expr) {
        match expr {
            // User-written grouping parens, preserved by the parser
            Expr::Group(group) => {
                self.output.write("(");
                self.format_expr(&group.expr);
                self.output.write(")");
            }
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
            Expr::StringRepeat(op) => {
                self.format_expr(&op.string);
                self.output.write(" $* ");
                self.format_expr(&op.count);
            }
            Expr::StringReplace(op) => self.format_string_replace(op),
            Expr::StringSplit(op) => {
                self.format_expr(&op.string);
                self.output.write("$/ ");
                self.format_expr(&op.delimiter);
            }
            Expr::ConcatBuild(op) => {
                self.format_expr(&op.base);
                self.output.write(" $++");
                for item in &op.items {
                    self.output.write(" ");
                    // Binary expressions (e.g. n + 1) need parens so the $++ parser
                    // reads them as a single item — without parens the parser stops at
                    // the identifier before the operator, restructuring the AST.
                    let needs_parens = matches!(item, Expr::Binary(_));
                    if needs_parens { self.output.write("("); }
                    self.format_expr(item);
                    if needs_parens { self.output.write(")"); }
                }
            }
            Expr::NumericCast(op) => {
                let prefix = match op.kind {
                    zymbol_ast::CastKind::ToFloat    => "##.",
                    zymbol_ast::CastKind::ToIntRound => "###",
                    zymbol_ast::CastKind::ToIntTrunc => "##!",
                };
                self.output.write(prefix);
                let needs_parens = matches!(op.expr.as_ref(), Expr::Binary(_));
                if needs_parens { self.output.write("("); }
                self.format_expr(&op.expr);
                if needs_parens { self.output.write(")"); }
            }
            Expr::NumericEval(op) => self.format_numeric_eval(op),
            Expr::TypeMetadata(op) => self.format_type_metadata(op),
            Expr::Format(op) => self.format_format_expr(op),
            Expr::BaseConversion(op) => self.format_base_conversion(op),
            Expr::Lambda(lambda) => self.format_lambda(lambda),
            Expr::CollectionMap(op) => self.format_collection_map(op),
            Expr::CollectionFilter(op) => self.format_collection_filter(op),
            Expr::CollectionReduce(op) => self.format_collection_reduce(op),
            Expr::CollectionSortAsc(op) => self.format_collection_sort(op, "$^+"),
            Expr::CollectionSortDesc(op) => self.format_collection_sort(op, "$^-"),
            Expr::CollectionSortCustom(op) => self.format_collection_sort(op, "$^"),
            Expr::Pipe(pipe) => self.format_pipe(pipe),
            Expr::Execute(exec) => self.format_execute(exec),
            Expr::BashExec(bash) => self.format_bash_exec(bash),
            Expr::Round(round) => self.format_round(round),
            Expr::Trunc(trunc) => self.format_trunc(trunc),
            Expr::ErrorCheck(check) => self.format_error_check(check),
            Expr::ErrorPropagate(prop) => self.format_error_propagate(prop),
            Expr::DeepIndex(di) => self.format_deep_index(di),
            Expr::FlatExtract(fe) => self.format_flat_extract(fe),
            Expr::StructuredExtract(se) => self.format_structured_extract(se),
            Expr::TerminalSize(_) => self.output.write(">>?"),
        }
    }

    /// Format a literal expression
    fn format_literal(&mut self, lit: &LiteralExpr) {
        self.format_literal_value(&lit.value);
    }

    /// Write a literal's source form, escapes included.
    ///
    /// Patterns used to print literals through `Display`, which does not
    /// escape: a match arm on `'\n'` came back out as a real newline inside
    /// the quotes and the formatted file no longer lexed.
    fn format_literal_value(&mut self, value: &Literal) {
        match value {
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
        if ident.pre_hot {
            self.output.write("°");
        }
        self.output.write(&ident.name);
        if ident.hot {
            self.output.write("°");
        }
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
        let should_inline = self.estimate_array_length(arr) <= config.max_inline_array_length;

        // `#[…]` — the declared mix. Without the mark the formatter reprinted it
        // as `[…]`, which is a DIFFERENT program: the homogeneity check applies
        // to one and not the other. The safety gate caught it and refused to
        // write the file, which is the gate doing its job — and the reason
        // `zymbol fmt` simply failed on any file using the form.
        if arr.declared_mixed {
            self.output.write("#");
        }

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
                if i < arr.elements.len() - 1 {
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
            Expr::ArrayLiteral(arr) => self.estimate_array_length(arr),
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
            // 1-tuples need the trailing comma: (1,) — without it the source
            // would re-parse as a grouped expression.
            if tuple.elements.len() == 1 {
                self.output.write(",");
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
        if member.is_module_access {
            self.output.write("::");
        } else {
            self.output.write(".");
        }
        self.output.write(&member.field);
    }

    /// Format an index expression
    fn format_index(&mut self, index: &IndexExpr) {
        // TypeMetadata (#?) followed by [i] requires parens: (expr#?)[i]
        // Without parens the parser sees `expr#?` as complete statement then `[i]` at stmt level.
        let needs_parens = matches!(index.array.as_ref(), Expr::TypeMetadata(_))
            || Self::expr_needs_postfix_parens(&index.array);
        if needs_parens { self.output.write("("); }
        self.format_expr(&index.array);
        if needs_parens { self.output.write(")"); }
        self.output.write("[");
        self.format_expr(&index.index);
        self.output.write("]");
    }

    fn nav_step_index_needs_parens(expr: &Expr, in_multi_step: bool) -> bool {
        // In multi-step paths, `>` is ambiguous with comparison:
        // `a+1>b` parses as `a + (1>b)`, not as `(a+1) > b`
        // So any non-atomic index in a multi-step path needs parens.
        if !in_multi_step {
            return false;
        }
        matches!(expr, Expr::Binary(_) | Expr::Unary(_) | Expr::FunctionCall(_))
    }

    fn format_nav_step(&mut self, step: &NavStep, in_multi_step: bool) {
        let needs_parens = Self::nav_step_index_needs_parens(&step.index, in_multi_step);
        if needs_parens { self.output.write("("); }
        self.format_expr(&step.index);
        if needs_parens { self.output.write(")"); }
        if let Some(ref end) = step.range_end {
            self.output.write("..");
            self.format_expr(end);
        }
    }

    fn format_nav_path(&mut self, path: &NavPath) {
        let multi = path.steps.len() > 1;
        for (i, step) in path.steps.iter().enumerate() {
            if i > 0 { self.output.write(">"); }
            self.format_nav_step(step, multi);
        }
    }

    /// Returns true when a collection expression needs outer parens
    /// to prevent a following postfix operator from binding to the wrong sub-expression.
    fn expr_needs_postfix_parens(expr: &Expr) -> bool {
        matches!(expr,
            Expr::CollectionSortCustom(_)
            | Expr::CollectionSortAsc(_)
            | Expr::CollectionSortDesc(_)
            | Expr::CollectionMap(_)
            | Expr::CollectionFilter(_)
            | Expr::CollectionReduce(_)
            | Expr::CollectionAppend(_)
            | Expr::CollectionInsert(_)
            | Expr::CollectionRemoveAt(_)
            | Expr::CollectionRemoveAll(_)
            | Expr::CollectionUpdate(_)
        )
    }

    fn format_deep_index(&mut self, di: &DeepIndexExpr) {
        let needs_parens = Self::expr_needs_postfix_parens(&di.array);
        if needs_parens { self.output.write("("); }
        self.format_expr(&di.array);
        if needs_parens { self.output.write(")"); }
        self.output.write("[");
        self.format_nav_path(&di.path);
        self.output.write("]");
    }

    fn format_flat_extract(&mut self, fe: &FlatExtractExpr) {
        self.format_expr(&fe.array);
        if fe.paths.len() == 1 && !fe.double_bracket {
            // Single-bracket spelling: arr[i>a..b]
            self.output.write("[");
            self.format_nav_path(&fe.paths[0]);
            self.output.write("]");
        } else if fe.paths.len() == 1 {
            // Explicit double-bracket spelling: arr[[path]]
            self.output.write("[[");
            self.format_nav_path(&fe.paths[0]);
            self.output.write("]]");
        } else {
            // Multi-path flat extract: arr[p1 ; p2 ; p3]
            self.output.write("[");
            for (i, path) in fe.paths.iter().enumerate() {
                if i > 0 { self.output.write(" ; "); }
                self.format_nav_path(path);
            }
            self.output.write("]");
        }
    }

    fn format_structured_extract(&mut self, se: &StructuredExtractExpr) {
        self.format_expr(&se.array);
        self.output.write("[");
        for (i, group) in se.groups.iter().enumerate() {
            if i > 0 { self.output.write(" ; "); }
            self.format_extract_group(group);
        }
        self.output.write("]");
    }

    fn format_extract_group(&mut self, group: &ExtractGroup) {
        self.output.write("[");
        for (i, path) in group.paths.iter().enumerate() {
            if i > 0 { self.output.write(", "); }
            self.format_nav_path(path);
        }
        self.output.write("]");
    }

    /// Format a function call expression
    fn format_function_call(&mut self, call: &FunctionCallExpr) {
        // Lambda callables need parens: (x -> x*2)(arg)
        let needs_parens = matches!(call.callable.as_ref(), Expr::Lambda(_));
        if needs_parens { self.output.write("("); }
        self.format_expr(&call.callable);
        if needs_parens { self.output.write(")"); }
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

        for case in &match_expr.cases {
            self.flush_comments_before(case.span.start.line);
            if self.blank_gap_before(case.span.start.line) {
                self.output.newline();
            }
            self.format_match_case(case);
            self.last_src_line = case.span.end.line;
            self.emit_trailing_comments(case.span.end.line);
            self.output.newline();
        }

        // Comments between the last arm and the closing }
        self.flush_comments_before(match_expr.span.end.line);

        self.output.dedent();
        self.output.close_brace();
    }

    /// Format a match case
    fn format_match_case(&mut self, case: &MatchCase) {
        self.format_pattern(&case.pattern);
        self.output.write(" =>");

        if let Some(ref value) = case.value {
            self.output.space();
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
                self.format_literal_value(lit);
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
            Pattern::Or(alternatives, _) => {
                for (i, alt) in alternatives.iter().enumerate() {
                    if i > 0 {
                        self.output.write(" || ");
                    }
                    self.format_pattern(alt);
                }
            }
        }
    }

        /// Format collection length operation
    fn format_collection_length(&mut self, op: &CollectionLengthExpr) {
        let needs_parens = Self::expr_needs_postfix_parens(&op.collection);
        if needs_parens { self.output.write("("); }
        self.format_expr(&op.collection);
        if needs_parens { self.output.write(")"); }
        self.output.write("$#");
    }

    /// Format collection append operation
    fn format_collection_append(&mut self, op: &CollectionAppendExpr) {
        self.format_expr(&op.collection);
        self.output.write(" $+ ");
        let needs_parens = matches!(op.element.as_ref(), Expr::Binary(_));
        if needs_parens { self.output.write("("); }
        self.format_expr(&op.element);
        if needs_parens { self.output.write(")"); }
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
        // `(a, b) -> e` vs `a, b -> e` (the latter occurs inside a grouped
        // lambda `(a, b -> e)`) — reprint the user's form.
        if lambda.params_parenthesized {
            self.output.write("(");
            for (i, param) in lambda.params.iter().enumerate() {
                self.output.write(param);
                if i < lambda.params.len() - 1 {
                    self.output.write(", ");
                }
            }
            self.output.write(")");
        } else {
            for (i, param) in lambda.params.iter().enumerate() {
                self.output.write(param);
                if i < lambda.params.len() - 1 {
                    self.output.write(", ");
                }
            }
        }

        // `format_block` opens with its own leading space — `" { "` when it
        // inlines, `" {"` from `open_brace`, or a newline in brace-next-line
        // mode. So the arrow must not leave one behind, or the two add up:
        // `x ->  { … }` in the default mode, and a trailing space at end of
        // line in the other. An expression body has no such space of its own.
        match &lambda.body {
            LambdaBody::Expr(expr) => {
                self.output.write(" -> ");
                self.format_expr(expr);
            }
            LambdaBody::Block(block) => {
                self.output.write(" ->");
                self.format_block(block);
            }
        }
    }

    /// Format collection map operation
    fn format_collection_map(&mut self, op: &CollectionMapExpr) {
        self.format_expr(&op.collection);
        self.output.write("$> ");
        self.format_expr(&op.lambda);
    }

    /// Format collection filter operation
    fn format_collection_filter(&mut self, op: &CollectionFilterExpr) {
        self.format_expr(&op.collection);
        self.output.write("$| ");
        self.format_expr(&op.lambda);
    }

    /// Format collection sort operation
    fn format_collection_sort(&mut self, op: &CollectionSortExpr, sym: &str) {
        self.format_expr(&op.collection);
        self.output.write(sym);
        if let Some(ref cmp) = op.comparator {
            self.output.write(" ");
            self.format_expr(cmp);
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
        let needs_parens = matches!(pipe.callable.as_ref(), Expr::Lambda(_));
        if needs_parens { self.output.write("("); }
        self.format_expr(&pipe.callable);
        if needs_parens { self.output.write(")"); }
        // Implicit pipe (user wrote `|> f` with no arg list): do not emit `(_)` (§2.1)
        if !pipe.implicit {
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
    }

    /// Format execute expression
    fn format_execute(&mut self, exec: &ExecuteExpr) {
        self.output.write("</");
        if exec.quoted {
            self.output.write("\"");
            self.output.write(&exec.path);
            self.output.write("\"");
        } else {
            self.output.write(&exec.path);
        }
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
            // \x01/\x02 are sentinels for \{ and \} — restore to source form
            '\x01' => result.push_str("\\{"),
            '\x02' => result.push_str("\\}"),
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
        assert_eq!(escape_string("\x01name\x02"), "\\{name\\}");
    }

    #[test]
    fn test_escape_char() {
        assert_eq!(escape_char('a'), "a");
        assert_eq!(escape_char('\n'), "\\n");
        assert_eq!(escape_char('\''), "\\'");
    }

    /// Literals in a *pattern* went out through `Display`, which does not
    /// escape, so `'\n'` came back as a real newline and the formatted file
    /// stopped lexing (the safety gate caught it as a hard format failure).
    #[test]
    fn test_pattern_literals_keep_their_escapes() {
        let src = "c = \"\\n\"[1]\n?? c {\n    '\\n' => { >> \"nl\" ¶ }\n    _ => { >> \"other\" ¶ }\n}\n";
        let formatted = crate::format(src).expect("formatting must succeed");
        assert!(
            formatted.contains("'\\n'"),
            "escape lost in pattern: {formatted}"
        );
        assert!(
            !formatted.contains("'\n'"),
            "raw newline written inside a char literal: {formatted}"
        );
    }

    /// `format_block` supplies its own leading space, so the arrow must not
    /// leave one too. It did, and every block lambda came out `x ->  { … }`
    /// with two spaces — cosmetic, invisible to the property harness (which
    /// checks reparse, idempotence, semantics and comments, none of which a
    /// stray space breaks), and therefore able to sit there indefinitely.
    #[test]
    fn test_block_lambda_arrow_has_one_space() {
        for src in [
            "a = (x) -> { <~ x }\n",
            "b = (x, y) -> { <~ x }\n",
            "c = x -> { <~ x }\n",
            "d = () -> { <~ 1 }\n",
            "e = arr$> (x -> { <~ x })\n",
        ] {
            let formatted = crate::format(src).expect("formatting must succeed");
            assert!(
                !formatted.contains("->  "),
                "double space after the arrow in {src:?}: {formatted:?}"
            );
            assert!(
                formatted.contains("-> {"),
                "arrow and brace should be one space apart in {src:?}: {formatted:?}"
            );
        }
    }

    /// An expression body has no leading space of its own, so there the arrow
    /// keeps the one it always had.
    #[test]
    fn test_expr_lambda_arrow_keeps_its_space() {
        let formatted = crate::format("e = x -> x + 1\n").expect("formatting must succeed");
        assert!(
            formatted.contains("x -> x + 1"),
            "expression body lost its spacing: {formatted:?}"
        );
    }

    /// No line may end in whitespace. In brace-next-line mode `open_brace`
    /// writes the newline itself, so the arrow's trailing space landed at end
    /// of line — the same defect wearing a different hat.
    ///
    /// The body needs more than one statement: with the default
    /// `inline_single_statement`, a one-statement block takes the `" { … }"`
    /// path and never reaches `open_brace` at all.
    #[test]
    fn test_no_trailing_whitespace_around_lambda_braces() {
        let src = "a = (x) -> { <~ x }\nb = x -> {\n    y = x + 1\n    <~ y\n}\n";
        for formatted in [
            crate::format(src).expect("formatting must succeed"),
            crate::format_with_config(src, crate::FormatterConfig::new().with_brace_new_line())
                .expect("formatting must succeed"),
        ] {
            for (i, line) in formatted.lines().enumerate() {
                assert_eq!(
                    line.trim_end(),
                    line,
                    "line {} ends in whitespace: {line:?}",
                    i + 1
                );
            }
        }
    }

    #[test]
    fn test_format_float() {
        #[allow(clippy::approx_constant)]
        let val = 3.14;
        assert_eq!(format_float(val), "3.14");
        assert_eq!(format_float(42.0), "42.0");
    }
}
