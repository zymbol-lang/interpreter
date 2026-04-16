//! Script execution evaluation for Zymbol-Lang
//!
//! Handles runtime execution of scripts:
//! - Execute expressions: </ file.zy /> (runs Zymbol scripts, captures output)
//! - Bash exec expressions: <\ expr1 expr2 ... \> (runs shell commands — expressions concatenated)

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use zymbol_ast::{BashExecExpr, ExecuteExpr};
use zymbol_lexer::Lexer;
use zymbol_parser::Parser;
use zymbol_span::{FileId, Position, Span};

use crate::{Interpreter, Result, RuntimeError, Value};

impl<W: Write> Interpreter<W> {
    /// Evaluate execute expression: </ file.zy />
    /// Executes a .zy file and captures its output
    pub(crate) fn eval_execute(&mut self, execute: &ExecuteExpr) -> Result<Value> {
        // Resolve the file path relative to the calling script's directory.
        // Absolute paths are used as-is; everything else is resolved from the
        // parent of the current file (or base_dir if there is no current file).
        let file_path = if execute.path.starts_with('/') {
            PathBuf::from(&execute.path)
        } else {
            let current_dir = self.current_file
                .as_ref()
                .and_then(|p| p.parent())
                .unwrap_or(&self.base_dir);
            current_dir.join(&execute.path)
        };

        // Check if file exists
        if !file_path.exists() {
            return Err(RuntimeError::Generic {
                message: format!("file not found: {}", file_path.display()),
                span: execute.span,
            });
        }

        // Read the file
        let source = std::fs::read_to_string(&file_path)
            .map_err(RuntimeError::Io)?;

        // Parse the file
        let lexer = Lexer::new(&source, FileId(0));
        let (tokens, lex_diagnostics) = lexer.tokenize();

        if !lex_diagnostics.is_empty() {
            return Err(RuntimeError::Generic {
                message: format!("{} lexer errors in {}", lex_diagnostics.len(), file_path.display()),
                span: execute.span,
            });
        }

        let parser = Parser::new(tokens);
        let program = parser.parse().map_err(|errors| {
            RuntimeError::Generic {
                message: format!("{} parser errors in {}", errors.len(), file_path.display()),
                span: execute.span,
            }
        })?;

        // Create a new interpreter with a buffer to capture output
        let mut buffer = Vec::new();
        let mut script_interp = Interpreter::with_output(&mut buffer);

        // Set the current file for the sub-interpreter
        script_interp.set_current_file(&file_path);

        // Pass CLI args to the sub-interpreter (if we have any)
        if let Some(ref cli_args) = self.cli_args {
            // Convert Value array to String array for set_cli_args
            let args_strings: Vec<String> = cli_args.iter()
                .filter_map(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .collect();
            script_interp.set_cli_args(args_strings);
        }

        // Execute the program
        script_interp.execute(&program)
            .map_err(|e| RuntimeError::Generic {
                message: format!("error executing {}: {:?}", file_path.display(), e),
                span: execute.span,
            })?;

        // Convert captured output to string
        let output_str = String::from_utf8_lossy(&buffer).to_string();

        Ok(Value::String(output_str))
    }

    /// Evaluate bash execute expression: <\ command \>
    /// Executes a bash command and captures its output.
    /// Each arg is evaluated as a Zymbol expression; results are concatenated to form the command.
    /// No implicit separator — use char/string literals for spacing: <\ "ls" ' ' dir \>
    pub(crate) fn eval_bash_exec(&mut self, bash: &BashExecExpr) -> Result<Value> {
        // Evaluate all args and concatenate to build the shell command
        let mut command = String::new();
        for arg in &bash.args {
            let value = self.eval_expr(arg)?;
            let s = Self::value_to_bash_str(&value)?;
            command.push_str(&s);
        }

        // Execute the shell command
        let output = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
            .map_err(|e| RuntimeError::Generic {
                message: format!("failed to execute bash command: {}", e),
                span: bash.span,
            })?;

        // Capture both stdout and stderr
        let mut result = String::from_utf8_lossy(&output.stdout).to_string();

        if !output.stderr.is_empty() {
            let stderr_str = String::from_utf8_lossy(&output.stderr);
            if !result.is_empty() {
                result.push_str(&stderr_str);
            } else {
                result = stderr_str.to_string();
            }
        }

        // Strip trailing newline (consistent with shell command substitution $(...) behavior)
        let result = result.trim_end_matches('\n').to_string();

        Ok(Value::String(result))
    }

    fn value_to_bash_str(value: &Value) -> Result<String> {
        match value {
            Value::String(s) => Ok(s.clone()),
            Value::Int(n) => Ok(n.to_string()),
            Value::Float(f) => Ok(f.to_string()),
            Value::Bool(b) => Ok(if *b { "#1" } else { "#0" }.to_string()),
            Value::Char(c) => Ok(c.to_string()),
            Value::Array(arr) => {
                // Join array elements with spaces
                let elements: Vec<String> = arr.iter()
                    .map(Self::value_to_bash_str)
                    .collect::<Result<Vec<_>>>()?;
                Ok(elements.join(" "))
            }
            Value::Tuple(elements) => {
                // Join tuple elements with spaces
                let strs: Vec<String> = elements.iter()
                    .map(Self::value_to_bash_str)
                    .collect::<Result<Vec<_>>>()?;
                Ok(strs.join(" "))
            }
            Value::NamedTuple(fields) => {
                // Join named tuple values with spaces
                let strs: Vec<String> = fields.iter()
                    .map(|(_, v)| Self::value_to_bash_str(v))
                    .collect::<Result<Vec<_>>>()?;
                Ok(strs.join(" "))
            }
            Value::Function(_) => {
                // Create a dummy span for error reporting
                let dummy_span = Span::new(
                    Position { line: 0, column: 0, byte_offset: 0 },
                    Position { line: 0, column: 0, byte_offset: 0 },
                    FileId(0)
                );
                Err(RuntimeError::Generic {
                    message: "cannot use function in bash command interpolation".to_string(),
                    span: dummy_span,
                })
            }
            Value::Error(err) => {
                // Return error representation
                Ok(format!("##{}({})", err.error_type, err.message))
            }
            Value::Unit => Ok(String::new()),
        }
    }
}
