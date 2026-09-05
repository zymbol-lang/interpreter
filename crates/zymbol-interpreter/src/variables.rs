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

        // B3: fast path for self-assign collection mutation (e.g. arr = arr$+ elem)
        // Mutates in-place instead of clone + replace → O(1) append instead of O(n)
        match &assign.value {
            // Fast path: x = arr[i] — clone only the element, not the whole array
            // Avoids O(n) array clone when reading a single element by index.
            Expr::Index(idx) => {
                if let Expr::Identifier(arr_ident) = idx.array.unwrap_group() {
                    let index_val = self.eval_expr(&idx.index)?;
                    if let Value::Int(i) = &index_val {
                        if *i > 0 {
                            let idx_pos = (*i - 1) as usize;
                            let elem = {
                                match self.get_variable(&arr_ident.name) {
                                    Some(Value::Array(arr)) if idx_pos < arr.len() => {
                                        Some(arr[idx_pos].clone())
                                    }
                                    Some(Value::Tuple(tup)) if idx_pos < tup.len() => {
                                        Some(tup[idx_pos].clone())
                                    }
                                    _ => None,
                                }
                            };
                            if let Some(v) = elem {
                                self.set_variable(&assign.name, v);
                                return Ok(());
                            }
                        }
                        // i <= 0 or out-of-bounds: fallthrough to eval_index for proper 1-based error handling
                    }
                }
            }
            Expr::CollectionAppend(op) => {
                if let Expr::Identifier(ident) = op.collection.unwrap_group() {
                    if ident.name == assign.name {
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
                        // Array $+ Value
                        if let Some(Value::Array(arr)) = self.get_variable_mut(&assign.name) {
                            Rc::make_mut(arr).push(element);
                            return Ok(());
                        }
                        // String $+ Char
                        if let Value::Char(c) = element {
                            if let Some(Value::String(s)) = self.get_variable_mut(&assign.name) {
                                s.push(c);
                                return Ok(());
                            }
                        }
                        // fallthrough: incompatible types — normal eval will produce the error
                    }
                }
            }
            Expr::CollectionRemoveAt(op) => {
                if let Expr::Identifier(ident) = op.collection.unwrap_group() {
                    if ident.name == assign.name {
                        let index_val = self.eval_expr(&op.index)?;
                        if let Value::Int(i) = &index_val {
                            if *i > 0 {
                                if let Some(Value::Array(arr)) = self.get_variable_mut(&assign.name) {
                                    let idx = (*i - 1) as usize;
                                    if idx < arr.len() {
                                        Rc::make_mut(arr).remove(idx);
                                        return Ok(());
                                    }
                                }
                            }
                        }
                        // i <= 0 or out-of-bounds: fallthrough so eval normal generates the error
                    }
                }
            }
            // Fast path: arr = arr[i]$~ v — update single element in-place, no array clone
            // O(1) vs O(n) clone when the LHS variable matches the collection being updated.
            Expr::CollectionUpdate(op) => {
                if let Expr::Index(idx) = op.target.unwrap_group() {
                    if let Expr::Identifier(ident) = idx.array.unwrap_group() {
                        if ident.name == assign.name {
                            // Positional tuples are immutable — an in-place
                            // indexed write is forbidden. The NAMED tuple used
                            // to be refused here too, and no longer is:
                            // decisions 7-11 of Divergente_ES/forma/README.md
                            // make it the dictionary, which is mutable, and
                            // `d["k"]$~ v` as a statement is how you modify one
                            // (DM-21). The VM never refused it, so this is the
                            // tree-walker coming into line, not the reverse.
                            if let Some(Value::Tuple(_)) = self.get_variable(&assign.name) {
                                return Err(RuntimeError::Generic {
                                    message: tuple_immutable_msg(&assign.name),
                                    span: assign.span,
                                });
                            }
                            let index_val = self.eval_expr(&idx.index)?;
                            let new_value = self.eval_expr(&op.value)?;
                            if let Value::Int(i) = &index_val {
                                if *i > 0 {
                                    if let Some(Value::Array(arr)) = self.get_variable_mut(&assign.name) {
                                        let idx_pos = (*i - 1) as usize;
                                        if idx_pos < arr.len() {
                                            Rc::make_mut(arr)[idx_pos] = new_value;
                                            return Ok(());
                                        }
                                    }
                                }
                            }
                            // i <= 0 or out-of-bounds: fallthrough to normal eval for proper error
                        }
                    }
                }
            }
            // B12: fast path for x = x OP y (integer/float arithmetic self-assign).
            // Avoids Value::clone() of LHS and full eval_expr dispatch for simple loops.
            Expr::Binary(bin) => {
                if let Expr::Identifier(lhs_ident) = bin.left.unwrap_group() {
                    if lhs_ident.name == assign.name {
                        let rhs_val = self.eval_expr(&bin.right)?;
                        // Int fast path
                        if let (Some(Value::Int(curr)), Value::Int(rhs)) =
                            (self.get_variable_mut(&assign.name), &rhs_val)
                        {
                            // The i53 range is checked before the write. This
                            // path existed to skip the dispatch and skipped the
                            // range check with it, so `s = s + 1000000` in a
                            // loop walked straight out of the range in silence —
                            // the very "accumulator over a long loop" that
                            // REFERENCE.md cites as the scenario the check is
                            // for (DM-01, sonda A16).
                            let checked = match bin.op {
                                BinaryOp::Add => Some((num::add(*curr, *rhs), "+")),
                                BinaryOp::Sub => Some((num::sub(*curr, *rhs), "-")),
                                BinaryOp::Mul => Some((num::mul(*curr, *rhs), "*")),
                                // div/mod/pow: fallthrough (edge cases like div-by-zero)
                                _ => None,
                            };
                            if let Some((result, op)) = checked {
                                let (a, b) = (*curr, *rhs);
                                match result {
                                    Some(v) => { *curr = v; return Ok(()); }
                                    None => return Err(RuntimeError::Generic {
                                        message: num::overflow_msg(a, op, b),
                                        span: assign.span,
                                    }),
                                }
                            }
                        }
                        // Float fast path
                        if let (Some(Value::Float(curr)), Value::Float(rhs)) =
                            (self.get_variable_mut(&assign.name), &rhs_val)
                        {
                            match bin.op {
                                BinaryOp::Add => { *curr += rhs; return Ok(()); }
                                BinaryOp::Sub => { *curr -= rhs; return Ok(()); }
                                BinaryOp::Mul => { *curr *= rhs; return Ok(()); }
                                _ => {}
                            }
                        }
                        // String concat self-assign: str = str + other_str → push_str O(1) amortized
                        // Fixes O(n²) → O(n) for str = str + "a" loops (3k appends: ~5ms → ~1ms).
                        // Only String+String; String+other falls through to eval_binary (auto-convert).
                        if bin.op == BinaryOp::Add {
                            if let Value::String(rhs_str) = &rhs_val {
                                let rhs_owned = rhs_str.clone();
                                if let Some(Value::String(curr)) = self.get_variable_mut(&assign.name) {
                                    curr.push_str(&rhs_owned);
                                    return Ok(());
                                }
                            }
                            // String + Char: push single char (common in char iteration loops)
                            if let Value::Char(c) = &rhs_val {
                                let c_owned = *c;
                                if let Some(Value::String(curr)) = self.get_variable_mut(&assign.name) {
                                    curr.push(c_owned);
                                    return Ok(());
                                }
                            }
                        }
                        // type mismatch or unsupported op: fallthrough to normal eval
                    }
                }
            }
            _ => {}
        }

        let value = self.eval_expr(&assign.value)?;
        self.set_variable(&assign.name, value);
        Ok(())
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
