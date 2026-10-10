//! Variable and constant execution for Zymbol-Lang
//!
//! Handles runtime execution of:
//! - Assignment: name = expr (mutable variables)
//! - Constant declaration: name := expr (immutable)
//! - Constant validation: Prevents reassignment

use zymbol_ast::{Assignment, ConstDecl, Expr};
use zymbol_common::num;
use zymbol_common::BinaryOp;
use crate::{Interpreter, Result, RuntimeError, Value};
use std::io::Write;
use std::rc::Rc;

/// The refusal of an in-place edit on a positional tuple, in one place.
///
/// The VM spells the same text in `zymbol-vm::tuple_immutable_msg`, because
/// `zyq consensus` compares text and two engines refusing the same program with
/// different words are still a divergence.
pub(crate) fn tuple_immutable_msg(name: &str) -> String {
    format!(
        "cannot modify tuple '{}': tuples are immutable\nhelp: use 'new = {}[i]$~ value' for a functional update",
        name, name
    )
}

/// The refusal of an absent dictionary key, in one place and one wording.
///
/// The three engines used to spell this four different ways — `Named tuple has
/// no field 'z'. Available fields: a` through the dot, `named tuple has no field
/// 'z'. Available: a` through the bracket, and just `named tuple has no field
/// 'z'` in the VM, with no list at all. `forma/diccionarios.zy` § 2b asked for
/// one text with the available keys in all three; this is it.
///
/// The vocabulary is the decision too: it is a **dictionary**, not a named
/// tuple. A tuple is immutable by definition and this is not (decision 7).
///
/// Every site raises it with its kind declared, `##Key` (GLB-097): the message
/// carries the key asked for and the keys there are, and their words used to
/// choose the family — a key called `overflow` was a `##Range`.
pub(crate) fn missing_key_msg(key: &str, available: &[String]) -> String {
    if available.is_empty() {
        format!("no key '{}' in dictionary — it is empty", key)
    } else {
        format!("no key '{}' in dictionary — available: {}", key, available.join(", "))
    }
}

/// Infer the hot-definition neutral value from the assignment's RHS expression.
fn hot_neutral_from_value(value: &Expr, name: &str) -> Value {
    match value {
        Expr::CollectionAppend(op) => {
            if let Expr::Identifier(ident) = op.collection.unwrap_group() {
                if ident.name == name {
                    return Value::array(Vec::new());
                }
            }
            Value::Int(0)
        }
        Expr::Binary(bin) if bin.op == BinaryOp::Concat => Value::String(String::new()),
        Expr::Binary(bin)
            if matches!(bin.op, BinaryOp::Mul | BinaryOp::Div) =>
        {
            if let Expr::Identifier(ident) = bin.left.unwrap_group() {
                if ident.name == name {
                    return Value::Int(1);
                }
            }
            Value::Int(0)
        }
        _ => Value::Int(0),
    }
}

/// Whether `e` is the bare name `name` — the receiver of a self-assignment.
fn is_named(name: &str, e: &Expr) -> bool {
    matches!(e.unwrap_group(), Expr::Identifier(id) if id.name == name)
}

/// An operand that only reads: the shapes `arr$+ x`, `arr[i + 1]$~ v` and
/// `n = n + 1` have. Evaluating one cannot write a variable. Anything not
/// listed is not "impure" — it is asked properly, by
/// `zymbol_semantic::operand_may_write`.
pub(crate) fn plain_operand(e: &Expr) -> bool {
    match e {
        Expr::Literal(_) | Expr::Identifier(_) => true,
        Expr::Group(g) => plain_operand(&g.expr),
        Expr::Unary(u) => plain_operand(&u.operand),
        Expr::Binary(b) => plain_operand(&b.left) && plain_operand(&b.right),
        Expr::Index(i) => plain_operand(&i.array) && plain_operand(&i.index),
        Expr::MemberAccess(m) => plain_operand(&m.object),
        Expr::DeepIndex(d) => {
            plain_operand(&d.array)
                && d.path.steps.iter().all(|s| {
                    plain_operand(&s.index) && s.range_end.as_deref().is_none_or(plain_operand)
                })
        }
        Expr::CollectionLength(l) => plain_operand(&l.collection),
        _ => false,
    }
}

impl<W: Write> Interpreter<W> {
    /// Execute assignment statement: name = expr
    pub(crate) fn execute_assignment(&mut self, assign: &Assignment) -> Result<()> {
        // Check if trying to reassign a constant
        if self.is_const(&assign.name) {
            return Err(RuntimeError::Generic {
                message: format!(
                    "cannot reassign constant '{}' (declared with :=)",
                    assign.name
                ),
                span: assign.span,
            });
        }

        // A bare `$` edit statement modifies its receiver, and a positional
        // tuple does not change — whatever the operator. Checking the RECEIVER
        // once, here, rather than teaching each of `$+`, `$-`, `$^`… its own
        // exception, is what `forma/tuplas.zy` § 6 asks for: immutability is a
        // property of the value, not of the operator.
        //
        // The functional forms are untouched: `u = t$+ 3` derives a second
        // tuple and its sugar is `None`, exactly as `(1,2) + (3,)` works in
        // Python.
        if assign.sugar == zymbol_ast::AssignSugar::InPlaceEdit {
            if let Some(Value::Tuple(_)) = self.get_variable(&assign.name) {
                return Err(RuntimeError::Generic {
                    message: tuple_immutable_msg(&assign.name),
                    span: assign.span,
                });
            }
        }

        // Hot LHS (x°): auto-initialize to neutral in nearest @ scope on first use
        if assign.hot && self.get_variable(&assign.name).is_none() {
            let neutral = hot_neutral_from_value(&assign.value, &assign.name);
            self.set_at_nearest_loop(&assign.name, neutral);
        }

        // Pre-hot LHS (°x): auto-initialize to neutral in scope above nearest @ on first use
        if assign.pre_hot && self.get_variable(&assign.name).is_none() {
            let neutral = hot_neutral_from_value(&assign.value, &assign.name);
            self.set_above_nearest_loop(&assign.name, neutral);
        }

        // ── `name = name <op> operands…`: the receiver, edited where it lives ──
        //
        // An assignment whose value starts from its own target — `arr$+ x`,
        // `arr[i]$~ v`, `n = n + 1`, written as a statement or in full — does
        // not build a second value and replace the first: it works on the
        // variable itself. Three things hold on every path below:
        //
        //   * each operand is evaluated ONCE. These paths used to fall through
        //     to the general evaluation whenever their shortcut did not apply,
        //     and that evaluated the operand again: `n = n / f()`, `s = s f()`
        //     and `t$-[f()]` past the end all called `f` twice (ZYTW-011);
        //   * left to right. The receiver is read before its operands, so an
        //     operand that writes it is overwritten by the assignment
        //     (GLB-115). `receiver_read_first` keeps what was read — but only
        //     when an operand could write it at all, which is rare;
        //   * when nothing wrote it, the edit happens in place, with no copy.
        //
        // One method per shape. Each answers `true` when it has done the
        // assignment and `false` — with nothing evaluated — when the shape is
        // not its own, which leaves it to the general path below. They are
        // separate so that each stays small: folded into this function, the
        // variable lookups stopped being inlined and `s = s + i` paid for it.
        let done = match &assign.value {
            Expr::Index(idx) => self.assign_element_read(assign, idx)?,
            Expr::CollectionAppend(op) => self.assign_append(assign, op)?,
            Expr::CollectionRemoveAt(op) => self.assign_remove_at(assign, op)?,
            Expr::CollectionUpdate(op) => self.assign_update(assign, op)?,
            Expr::ConcatBuild(op) => self.assign_concat_build(assign, op)?,
            Expr::Binary(bin) => self.assign_binary(assign, bin)?,
            _ => false,
        };
        if done {
            return Ok(());
        }

        let value = self.eval_expr(&assign.value)?;
        self.set_variable(&assign.name, value);
        Ok(())
    }

    /// `x = arr[i]`: one element, read straight from the variable. When the
    /// index is one the shortcut does not take — negative, past the end, a key,
    /// a position in a string — the read is finished with the index already in
    /// hand; it used to be handed back and evaluated again (ZYTW-011).
    #[inline(never)]
    fn assign_element_read(&mut self, assign: &Assignment, idx: &zymbol_ast::IndexExpr) -> Result<bool> {
        if let Expr::Identifier(arr_ident) = idx.array.unwrap_group() {
            // An index that can write the collection: `eval_index` reads the
            // collection first (ZYTW-014).
            if self.operands_may_write(&arr_ident.name, &[&idx.index]) {
                return Ok(false);
            }
            let index_val = self.eval_expr(&idx.index)?;
            let element = match (self.get_variable(&arr_ident.name), &index_val) {
                (Some(Value::Array(arr)), Value::Int(i)) if *i > 0 && (*i as usize) <= arr.len() => {
                    Some(arr[(*i - 1) as usize].clone())
                }
                (Some(collection), _) => Some(Self::index_into(collection, &index_val, idx.span)?),
                (None, _) => None,
            };
            let value = match element {
                Some(v) => v,
                // Not a variable in view: the name says what it is.
                None => {
                    let collection = self.eval_expr(&idx.array)?;
                    Self::index_into(&collection, &index_val, idx.span)?
                }
            };
            self.set_variable(&assign.name, value);
            return Ok(true);
        }
        Ok(false)
    }

    /// `arr$+ x`, and `s$+ c` on a string: the element goes onto the collection
    /// where it is.
    #[inline(never)]
    fn assign_append(&mut self, assign: &Assignment, op: &zymbol_ast::CollectionAppendExpr) -> Result<bool> {
        let name = assign.name.as_str();
        if let Expr::Identifier(ident) = op.collection.unwrap_group() {
            if ident.name == assign.name {
                let read = self.receiver_read_first(name, &[&op.element]);
                let element = self.eval_expr(&op.element)?;
                // Hot/pre_hot RHS: auto-init on first use.
                // Char element → init to "" (String); anything else → init to [] (Array)
                if (ident.hot || ident.pre_hot) && self.get_variable(&assign.name).is_none() {
                    let neutral = if matches!(element, Value::Char(_)) {
                        Value::String(String::new())
                    } else {
                        Value::array(Vec::new())
                    };
                    if ident.pre_hot {
                        self.set_above_nearest_loop(&assign.name, neutral);
                    } else {
                        self.set_variable(&assign.name, neutral);
                    }
                }
                return self.edit_receiver_done(name, read, &op.collection, |c| {
                    Self::append_in(c, element, op.span)
                });
            }
        }
        Ok(false)
    }

    /// `arr$-[i]`, and `d$-["k"]` on a dictionary: removed where it is.
    #[inline(never)]
    fn assign_remove_at(&mut self, assign: &Assignment, op: &zymbol_ast::CollectionRemoveAtExpr) -> Result<bool> {
        let name = assign.name.as_str();
        if let Expr::Identifier(ident) = op.collection.unwrap_group() {
            if ident.name == assign.name {
                let read = self.receiver_read_first(name, &[&op.index]);
                let index_val = self.eval_expr(&op.index)?;
                return self.edit_receiver_done(name, read, &op.collection, |c| {
                    Self::remove_at_in(c, index_val, op.span)
                });
            }
        }
        Ok(false)
    }

    /// `arr[i]$~ v`, `d["k"]$~ v` and `d.k$~ v`: one element written in place,
    /// whatever the collection. The deep form, `m[i>j]$~ v`, takes the general
    /// path — it evaluates each operand once too.
    ///
    /// A positional tuple reaches this only as `t = t[i]$~ v`, the functional
    /// form bound back to its own name, which derives a tuple like
    /// `u = t[i]$~ v` does; the statement `t[i]$~ v` was refused before any of
    /// this ran. This path used to refuse both (ZYTW-012).
    #[inline(never)]
    fn assign_update(&mut self, assign: &Assignment, op: &zymbol_ast::CollectionUpdateExpr) -> Result<bool> {
        let name = assign.name.as_str();
        match op.target.unwrap_group() {
            Expr::Index(idx) if is_named(name, &idx.array) => {
                let read = self.receiver_read_first(name, &[&idx.index, &op.value]);
                let index_val = self.eval_expr(&idx.index)?;
                let new_value = self.eval_expr(&op.value)?;
                self.edit_receiver_done(name, read, &idx.array, |c| {
                    Self::update_in(c, index_val, new_value, op.span)
                })
            }
            Expr::MemberAccess(ma) if !ma.is_module_access && is_named(name, &ma.object) => {
                let read = self.receiver_read_first(name, &[&op.value]);
                let new_value = self.eval_expr(&op.value)?;
                let key = Value::String(ma.field.clone());
                self.edit_receiver_done(name, read, &ma.object, |c| {
                    Self::update_in(c, key, new_value, op.span)
                })
            }
            _ => Ok(false),
        }
    }

    /// `arr$++ a b` adds at the end, like `$+` does one at a time. It went
    /// through the general path, which holds a second reference to the
    /// collection while it pushes, so every one of them copied all of it
    /// (GLB-118). Only a base that `$++` takes comes here: anything else is
    /// refused before an item is evaluated, as it always was.
    #[inline(never)]
    fn assign_concat_build(&mut self, assign: &Assignment, op: &zymbol_ast::ConcatBuildExpr) -> Result<bool> {
        let name = assign.name.as_str();
        if !is_named(name, &op.base) {
            return Ok(false);
        }
        let text = match self.get_variable(name) {
            Some(Value::String(_)) => Some(true),
            Some(Value::Array(_)) => Some(false),
            _ => None,
        };
        if let Some(text) = text {
            let operands: Vec<&Expr> = op.items.iter().collect();
            let read = self.receiver_read_first(name, &operands);
            // A string takes each item as text, in the numeral mode of
            // the moment it is evaluated; an array takes the values.
            let mut pieces = String::new();
            let mut items = Vec::new();
            for item in &op.items {
                let v = self.eval_expr(item)?;
                if text {
                    pieces.push_str(&self.value_to_concat_str(&v));
                } else {
                    items.push(v);
                }
            }
            let span = op.span;
            return self.edit_receiver_done(name, read, &op.base, |c| match c {
                Value::String(s) => {
                    s.push_str(&pieces);
                    Ok(())
                }
                Value::Array(arr) => {
                    Rc::make_mut(arr).extend(items);
                    Ok(())
                }
                other => Err(RuntimeError::kinded(
                    "Type",
                    format!("$++ requires a string or array as base, got {}", other.type_label()),
                    span,
                )),
            });
        }
        Ok(false)
    }

    /// B12: `x = x OP y`, on the variable itself.
    ///
    /// Not `&&` and `||`: they decide whether the right side runs at all, and
    /// `eval_binary` is where that is decided — this path evaluated it first,
    /// so `v = v && f()` called `f` with `v` false (DM-19 again). And not a hot
    /// name on its first use: it is given its neutral by `eval_binary`, which
    /// has to see the right-hand side to choose it.
    #[inline(never)]
    fn assign_binary(&mut self, assign: &Assignment, bin: &zymbol_ast::BinaryExpr) -> Result<bool> {
        let name = assign.name.as_str();
        if matches!(bin.op, BinaryOp::And | BinaryOp::Or) {
            return Ok(false);
        }
        if let Expr::Identifier(lhs_ident) = bin.left.unwrap_group() {
            let first_use = (lhs_ident.hot || lhs_ident.pre_hot)
                && self.get_variable(name).is_none();
            if lhs_ident.name == assign.name && !first_use {
                let rhs_val = if plain_operand(&bin.right) {
                    self.eval_expr(&bin.right)?
                } else {
                    let read = self.receiver_read_first(name, &[&bin.right]);
                    let rhs_val = self.eval_expr(&bin.right)?;
                    // The right side wrote the receiver: the operator takes
                    // what was read, and the assignment overwrites the write.
                    if let Some(read) = read {
                        if !self.receiver_untouched(name, &read) {
                            let value = self.apply_binary(bin, &read, &rhs_val)?;
                            self.set_variable(name, value);
                            return Ok(true);
                        }
                    }
                    rhs_val
                };
                if bin.op == BinaryOp::Concat {
                    // `s = s x`: the text accumulator. Onto the string
                    // that is already there, when it is one.
                    let piece = self.value_to_concat_str(&rhs_val);
                    if let Some(Value::String(curr)) = self.get_variable_mut(name) {
                        curr.push_str(&piece);
                        return Ok(true);
                    }
                } else {
                    match (self.get_variable_mut(name), &rhs_val) {
                        (Some(Value::Int(curr)), Value::Int(rhs)) => {
                            // The i53 range is checked before the write.
                            // This path existed to skip the dispatch and
                            // skipped the range check with it, so
                            // `s = s + 1000000` in a loop walked straight
                            // out of the range in silence — the very
                            // "accumulator over a long loop" that
                            // REFERENCE.md cites as the scenario the
                            // check is for (DM-01, sonda A16).
                            let checked = match bin.op {
                                BinaryOp::Add => Some((num::add(*curr, *rhs), "+")),
                                BinaryOp::Sub => Some((num::sub(*curr, *rhs), "-")),
                                BinaryOp::Mul => Some((num::mul(*curr, *rhs), "*")),
                                // div/mod/pow: the operator below (edge cases like div-by-zero)
                                _ => None,
                            };
                            if let Some((result, op)) = checked {
                                let (a, b) = (*curr, *rhs);
                                match result {
                                    Some(v) => { *curr = v; return Ok(true); }
                                    None => return Err(RuntimeError::Generic {
                                        message: num::overflow_msg(a, op, b),
                                        span: assign.span,
                                    }),
                                }
                            }
                        }
                        (Some(Value::Float(curr)), Value::Float(rhs)) => match bin.op {
                            BinaryOp::Add => { *curr += rhs; return Ok(true); }
                            BinaryOp::Sub => { *curr -= rhs; return Ok(true); }
                            BinaryOp::Mul => { *curr *= rhs; return Ok(true); }
                            _ => {}
                        },
                        _ => {}
                    }
                }
                // Anything else is the operator itself, on the two values
                // in hand — the right side is not evaluated again.
                //
                // There was one more shortcut here: `s = s + t` on two
                // strings pushed `t` onto `s`. `+` is arithmetic only —
                // `u = s + t` is refused in this engine and all three
                // refuse `s = s + t` everywhere else — so a program that
                // joined text that way ran here and nowhere else
                // (ZYTW-009).
                let left = match self.get_variable(name) {
                    Some(v) => v.clone(),
                    // Not a variable in view: the name says so itself.
                    None => self.eval_expr(&bin.left)?,
                };
                let value = self.apply_binary(bin, &left, &rhs_val)?;
                self.set_variable(name, value);
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The receiver of `name = name <op> operands`, read before the operands —
    /// when one of them could write it. `None` when none can, which is nearly
    /// always, and then nothing is read ahead and nothing is shared.
    ///
    /// A local is written by an operand only through `f(name<~)`; module state
    /// by anything that runs the module's code. See
    /// `zymbol_semantic::operand_may_write`.
    #[inline(always)]
    fn receiver_read_first(&self, name: &str, operands: &[&Expr]) -> Option<Value> {
        if self.operands_may_write(name, operands) {
            self.get_variable(name).cloned()
        } else {
            None
        }
    }

    /// Whether evaluating any of `operands` can write the variable `name`.
    ///
    /// Nearly every operand runs no code — a literal, a name, arithmetic, an
    /// index — and this is on the path of every edit and every indexed read:
    /// those are told apart without the general walk or a lookup.
    #[inline(always)]
    pub(crate) fn operands_may_write(&self, name: &str, operands: &[&Expr]) -> bool {
        if operands.iter().all(|e| plain_operand(e)) {
            return false;
        }
        let shared = self.frame_module_vars.contains_key(name);
        operands.iter().any(|e| zymbol_semantic::operand_may_write(e, name, shared))
    }

    /// Whether the receiver still is what `receiver_read_first` read.
    ///
    /// A collection by identity: with the read alive there are two owners, so
    /// a write either rebinds the variable or copies the collection, and the
    /// pointer changes both ways. Anything else by value — writing back the
    /// value it already had is not a change.
    fn receiver_untouched(&self, name: &str, read: &Value) -> bool {
        match (self.get_variable(name), read) {
            (Some(Value::Array(now)), Value::Array(then)) => Rc::ptr_eq(now, then),
            (Some(Value::Tuple(now)), Value::Tuple(then)) => Rc::ptr_eq(now, then),
            (Some(Value::NamedTuple(now)), Value::NamedTuple(then)) => Rc::ptr_eq(now, then),
            (Some(now), then) => now == then,
            (None, _) => false,
        }
    }

    /// Apply an edit to the receiver of `name = name <edit> …`, the operands
    /// already evaluated and inside `edit`.
    ///
    /// If an operand wrote the receiver after it was read, the edit applies to
    /// what was read and the result replaces what the operand wrote: that is
    /// the assignment, left to right (GLB-115). Otherwise the edit runs on the
    /// variable where it lives. `edit` validates before it writes, so a
    /// refusal leaves the variable as it was — and whatever an operand wrote
    /// stays written, because the assignment never happened.
    #[inline(always)]
    fn edit_receiver(
        &mut self,
        name: &str,
        read: Option<Value>,
        receiver: &Expr,
        edit: impl FnOnce(&mut Value) -> Result<()>,
    ) -> Result<()> {
        if let Some(mut read) = read {
            if !self.receiver_untouched(name, &read) {
                edit(&mut read)?;
                self.set_variable(name, read);
                return Ok(());
            }
        }
        if let Some(slot) = self.get_variable_mut(name) {
            return edit(slot);
        }
        // Not a variable in view: the receiver says so in its own words.
        let mut value = self.eval_expr(receiver)?;
        edit(&mut value)?;
        self.set_variable(name, value);
        Ok(())
    }

    /// `edit_receiver`, for a caller that answers "done".
    #[inline(always)]
    fn edit_receiver_done(
        &mut self,
        name: &str,
        read: Option<Value>,
        receiver: &Expr,
        edit: impl FnOnce(&mut Value) -> Result<()>,
    ) -> Result<bool> {
        self.edit_receiver(name, read, receiver, edit)?;
        Ok(true)
    }

    /// Execute constant declaration: name := expr
    pub(crate) fn execute_const_decl(&mut self, const_decl: &ConstDecl) -> Result<()> {
        // Check if constant already declared
        if self.is_const(&const_decl.name) {
            return Err(RuntimeError::Generic {
                message: format!(
                    "constant '{}' already declared",
                    const_decl.name
                ),
                span: const_decl.span,
            });
        }

        // Evaluate the constant's value
        let value = self.eval_expr(&const_decl.value)?;

        // MM-9: a := at the root scope of top-level code is globally scoped —
        // record it so functions resolve it at any call depth. Constants
        // declared inside blocks or function bodies stay lexically scoped.
        if self.is_root_scope() {
            self.record_global_const(const_decl.name.clone(), value.clone());
        }

        // Store in variables and mark as constant
        self.set_variable(&const_decl.name, value);
        self.mark_const(const_decl.name.clone());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Interpreter;
    use zymbol_lexer::Lexer;
    use zymbol_parser::Parser;
    use zymbol_span::FileId;

    fn run(source: &str) -> String {
        let mut output = Vec::new();

        // Lex
        let lexer = Lexer::new(source, FileId(0));
        let (tokens, lex_diagnostics) = lexer.tokenize();
        assert!(lex_diagnostics.is_empty(), "Lexer errors: {:?}", lex_diagnostics);

        // Parse
        let parser = Parser::new(tokens);
        let program = parser.parse().expect("Parse error");

        // Execute
        let mut interpreter = Interpreter::with_output(&mut output);
        interpreter.execute(&program).expect("Runtime error");

        String::from_utf8(output).expect("Invalid UTF-8")
    }

    // ── A runtime error says where it happened ──────────────────────────────

    /// Run a program that is expected to fail, and report where the error says
    /// it happened. `set_current_file` is what the CLI does; without it there
    /// is nothing to name and the error stays unlocated, which is also correct.
    fn location_of(source: &str) -> Option<(String, u32)> {
        let mut output = Vec::new();
        let (tokens, diags) = Lexer::new(source, FileId(0)).tokenize();
        assert!(diags.is_empty(), "Lexer errors: {diags:?}");
        let program = Parser::new(tokens).parse().expect("Parse error");
        let mut interpreter = Interpreter::with_output(&mut output);
        interpreter.set_current_file(std::path::Path::new("prog.zy"));
        let err = interpreter.execute(&program).expect_err("this program must fail");
        err.location().map(|(f, l)| (f.to_string(), l))
    }

    #[test]
    fn a_runtime_error_names_its_file_and_line() {
        assert_eq!(
            location_of("a = 1\nb = 0\nc = a / b\n>> c ¶\n"),
            Some(("prog.zy".to_string(), 3))
        );
    }

    /// The line is the STATEMENT's, not the span the error happens to carry —
    /// several are built with a default span, and reporting those meant line 1
    /// for a failure four lines down. The register VM and the browser engine
    /// both answer the statement, and the three are compared on the text.
    #[test]
    fn the_line_is_the_statement_that_failed() {
        assert_eq!(location_of("// a comment\n\n\n>> (7 > ##_) ¶\n"), Some(("prog.zy".to_string(), 4)));
    }

    /// Inside a function, the line is the failing statement in the body.
    #[test]
    fn a_failure_inside_a_function_names_the_body_line() {
        assert_eq!(
            location_of("f(x) {\n    y = 0\n    <~ x / y\n}\n>> f(1) ¶\n"),
            Some(("prog.zy".to_string(), 3))
        );
    }

    /// Attaching a location must not change what the message says: `!?`
    /// classifies an error by its text, and `zyq consensus` compares it.
    #[test]
    fn locating_leaves_the_message_alone() {
        let mut output = Vec::new();
        let (tokens, _) = Lexer::new("x = 1 / 0\n", FileId(0)).tokenize();
        let program = Parser::new(tokens).parse().expect("Parse error");
        let mut interpreter = Interpreter::with_output(&mut output);
        interpreter.set_current_file(std::path::Path::new("prog.zy"));
        let err = interpreter.execute(&program).expect_err("this program must fail");
        assert_eq!(err.to_string(), "division by zero");
    }

    #[test]
    fn test_assignment() {
        let output = run("x = \"hello\"\n>> x ¶");
        assert_eq!(output, "hello\n");
    }

    #[test]
    fn test_reassignment() {
        let output = run("x = \"first\"\n>> x ¶\nx = \"second\"\n>> x ¶");
        assert_eq!(output, "first\nsecond\n");
    }

    #[test]
    fn test_multiple_variables() {
        let output = run("a = \"A\"\nb = \"B\"\n>> a ¶\n>> b ¶");
        assert_eq!(output, "A\nB\n");
    }

    /// Auto-free (v0.0.8): variables are destroyed right after their last use.
    #[test]
    fn test_auto_free_after_last_use() {
        let source = "x = 10\ny = 20\n>> x ¶\n>> y ¶\nK := 5\n>> K ¶";
        let mut output = Vec::new();
        let lexer = Lexer::new(source, FileId(0));
        let (tokens, lex_diagnostics) = lexer.tokenize();
        assert!(lex_diagnostics.is_empty());
        let parser = Parser::new(tokens);
        let program = parser.parse().expect("parse");
        let mut interp = Interpreter::with_output(&mut output);
        interp.execute(&program).expect("run");
        // x and y were auto-destroyed after their last uses; K is a constant
        // and is never auto-freed.
        assert!(interp.get_variable("x").is_none(), "x must be auto-freed");
        assert!(interp.get_variable("y").is_none(), "y must be auto-freed");
        assert!(interp.auto_dead_variables.contains("x"));
        assert!(interp.auto_dead_variables.contains("y"));
        assert!(interp.get_variable("K").is_some(), "constants survive");
        drop(interp);
        assert_eq!(String::from_utf8(output).unwrap(), "10\n20\n5\n");
    }

    /// Auto-free is invisible: interpolation uses keep the variable alive.
    #[test]
    fn test_auto_free_respects_interpolation() {
        let source = "n = 7\n>> n ¶\n>> \"v={n}\" ¶";
        let mut output = Vec::new();
        let lexer = Lexer::new(source, FileId(0));
        let (tokens, _) = lexer.tokenize();
        let parser = Parser::new(tokens);
        let program = parser.parse().expect("parse");
        let mut interp = Interpreter::with_output(&mut output);
        interp.execute(&program).expect("run");
        drop(interp);
        assert_eq!(String::from_utf8(output).unwrap(), "7\nv=7\n");
    }
}
