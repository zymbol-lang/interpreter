//! Variable and constant execution for Zymbol-Lang
//!
//! Handles runtime execution of:
//! - Assignment: name = expr (mutable variables)
//! - Constant declaration: name := expr (immutable)
//! - Constant validation: Prevents reassignment

use zymbol_ast::{Assignment, ConstDecl, Expr};
use zymbol_common::BinaryOp;
use crate::{Interpreter, Result, RuntimeError, Value};
use std::io::Write;

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

        // B3: fast path for self-assign collection mutation (e.g. arr = arr$+ elem)
        // Mutates in-place instead of clone + replace → O(1) append instead of O(n)
        match &assign.value {
            // Fast path: x = arr[i] — clone only the element, not the whole array
            // Avoids O(n) array clone when reading a single element by index.
            Expr::Index(idx) => {
                if let Expr::Identifier(arr_ident) = idx.array.as_ref() {
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
                if let Expr::Identifier(ident) = op.collection.as_ref() {
                    if ident.name == assign.name {
                        let element = self.eval_expr(&op.element)?;
                        if let Some(Value::Array(arr)) = self.get_variable_mut(&assign.name) {
                            arr.push(element);
                            return Ok(());
                        }
                        // fallthrough: not an Array — eval_expr will produce the correct error
                    }
                }
            }
            Expr::CollectionRemoveAt(op) => {
                if let Expr::Identifier(ident) = op.collection.as_ref() {
                    if ident.name == assign.name {
                        let index_val = self.eval_expr(&op.index)?;
                        if let Value::Int(i) = &index_val {
                            if *i > 0 {
                                if let Some(Value::Array(arr)) = self.get_variable_mut(&assign.name) {
                                    let idx = (*i - 1) as usize;
                                    if idx < arr.len() {
                                        arr.remove(idx);
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
                if let Expr::Index(idx) = op.target.as_ref() {
                    if let Expr::Identifier(ident) = idx.array.as_ref() {
                        if ident.name == assign.name {
                            // Tuples are immutable — indexed assignment is forbidden
                            match self.get_variable(&assign.name) {
                                Some(Value::Tuple(_)) | Some(Value::NamedTuple(_)) => {
                                    return Err(RuntimeError::Generic {
                                        message: format!(
                                            "cannot modify tuple '{}': tuples are immutable\nhelp: use 'new = {}[i]$~ value' for a functional update",
                                            assign.name, assign.name
                                        ),
                                        span: assign.span,
                                    });
                                }
                                _ => {}
                            }
                            let index_val = self.eval_expr(&idx.index)?;
                            let new_value = self.eval_expr(&op.value)?;
                            if let Value::Int(i) = &index_val {
                                if *i > 0 {
                                    if let Some(Value::Array(arr)) = self.get_variable_mut(&assign.name) {
                                        let idx_pos = (*i - 1) as usize;
                                        if idx_pos < arr.len() {
                                            arr[idx_pos] = new_value;
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
                if let Expr::Identifier(lhs_ident) = bin.left.as_ref() {
                    if lhs_ident.name == assign.name {
                        let rhs_val = self.eval_expr(&bin.right)?;
                        // Int fast path
                        if let (Some(Value::Int(curr)), Value::Int(rhs)) =
                            (self.get_variable_mut(&assign.name), &rhs_val)
                        {
                            match bin.op {
                                BinaryOp::Add => { *curr += rhs; return Ok(()); }
                                BinaryOp::Sub => { *curr -= rhs; return Ok(()); }
                                BinaryOp::Mul => { *curr *= rhs; return Ok(()); }
                                _ => {} // div/mod/pow: fallthrough (edge cases like div-by-zero)
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
}
