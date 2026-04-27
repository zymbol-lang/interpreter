//! IO execution for Zymbol-Lang
//!
//! Handles runtime execution of IO statements:
//! - Output: write expressions to output stream
//! - Input: read from stdin, store in variable
//! - Newline: write newline to output

use std::io::Write;
use zymbol_ast::{Input, InputCast, InputPrompt, Newline, Output};
use zymbol_lexer::StringPart;
use crate::numeral_mode::{to_numeral_int, to_numeral_float, to_numeral_bool};
use crate::data_ops::parse_numeric_string;
use crate::{Interpreter, Result, RuntimeError, Value};

impl<W: Write> Interpreter<W> {
    /// Execute output statement: >> expr1 expr2 ...
    ///
    /// Numeric values (`Int`, `Float`, `Bool`) are rendered using the active
    /// numeral mode; all other values use their standard display form.
    pub(crate) fn execute_output(&mut self, output: &Output) -> Result<()> {
        let mode = self.numeral_mode;
        for expr in &output.exprs {
            let value = self.eval_expr(expr)?;
            let s = match &value {
                Value::Int(n)   => to_numeral_int(*n, mode),
                Value::Float(f) => to_numeral_float(*f, mode),
                Value::Bool(b)  => to_numeral_bool(*b, mode),
                _               => value.to_display_string(),
            };
            write!(self.output, "{}", s)?;
        }
        Ok(())
    }

    /// Execute newline statement: ¶ OR \\
    pub(crate) fn execute_newline(&mut self, _newline: &Newline) -> Result<()> {
        // Explicit newline: ¶ or \\
        writeln!(self.output)?;
        Ok(())
    }

    /// Execute input statement: << variable (with optional prompt)
    pub(crate) fn execute_input(&mut self, input: &Input) -> Result<()> {
        // Display prompt through the interpreter's writer (handles raw mode via RawModeWriter)
        if let Some(prompt) = &input.prompt {
            let prompt_text = match prompt {
                InputPrompt::Simple(s) => s.clone(),
                InputPrompt::Interpolated(parts) => {
                    let mut result = String::new();
                    for part in parts {
                        match part {
                            StringPart::Text(text) => result.push_str(text),
                            StringPart::Variable(var_name) => {
                                if let Some(value) = self.get_variable(var_name) {
                                    result.push_str(&value.to_display_string());
                                } else {
                                    return Err(RuntimeError::Generic {
                                        message: format!(
                                            "undefined variable in input prompt: '{}'",
                                            var_name
                                        ),
                                        span: input.span,
                                    });
                                }
                            }
                        }
                    }
                    result
                }
            };
            write!(self.output, "{}", prompt_text)?;
            self.output.flush()?;
        }

        // Delegate reading to the injected input function.
        // In normal execution this reads from stdin; in the REPL it temporarily
        // disables raw mode so the user can type with echo and press Enter normally.
        let line = (self.input_fn)()
            .map_err(|e| RuntimeError::Generic {
                message: format!("input read error: {}", e),
                span: input.span,
            })?;

        let value = match input.cast {
            InputCast::String  => Value::String(line.trim().to_string()),
            InputCast::Numeric => parse_numeric_string(line.trim().to_string()),
        };
        self.set_variable(&input.variable, value);
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
    fn test_output_string() {
        let output = run(">> \"Hello, World!\" ¶");
        assert_eq!(output, "Hello, World!\n");
    }

    #[test]
    fn test_multiple_outputs() {
        let output = run(">> \"Line 1\" ¶\n>> \"Line 2\" ¶");
        assert_eq!(output, "Line 1\nLine 2\n");
    }

    #[test]
    fn test_output_with_escapes() {
        let output = run(r#">> "Hello\nWorld" ¶"#);
        assert_eq!(output, "Hello\nWorld\n");
    }
}
