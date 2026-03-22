//! IO execution for Zymbol-Lang
//!
//! Handles runtime execution of IO statements:
//! - Output: write expressions to output stream
//! - Input: read from stdin, store in variable
//! - Newline: write newline to output

use std::io::Write;
use zymbol_ast::{Input, InputPrompt, Newline, Output};
use zymbol_lexer::StringPart;
use crate::{Interpreter, Result, RuntimeError, Value};

impl<W: Write> Interpreter<W> {
    /// Execute output statement: >> expr1 expr2 ...
    pub(crate) fn execute_output(&mut self, output: &Output) -> Result<()> {
        // Evaluate and concatenate all expressions (Haskell-style)
        for expr in &output.exprs {
            let value = self.eval_expr(expr)?;
            write!(self.output, "{}", value.to_display_string())?;
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
        // Display prompt if present (handle both simple and interpolated)
        if let Some(prompt) = &input.prompt {
            let prompt_text = match prompt {
                InputPrompt::Simple(s) => s.clone(),
                InputPrompt::Interpolated(parts) => {
                    // Build the interpolated string by evaluating each part
                    let mut result = String::new();
                    for part in parts {
                        match part {
                            StringPart::Text(text) => result.push_str(text),
                            StringPart::Variable(var_name) => {
                                // Look up the variable and append its value
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

            print!("{}", prompt_text);
            std::io::stdout().flush()?;
        }

        // Read from stdin
        let mut buffer = String::new();
        std::io::stdin().read_line(&mut buffer)?;

        // Trim newline and store as string
        let value = buffer.trim().to_string();
        self.set_variable(&input.variable, Value::String(value));
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
