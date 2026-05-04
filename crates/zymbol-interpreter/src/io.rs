//! IO execution for Zymbol-Lang
//!
//! Handles runtime execution of IO statements:
//! - Output: write expressions to output stream
//! - Input: read from stdin, store in variable
//! - Newline: write newline to output

use std::io::Write;
use zymbol_ast::{ClearScreen, Input, InputCast, InputPrompt, KeyInput, Newline, Output, OutputPos, TuiBlock};
use zymbol_lexer::StringPart;
use zymbol_span::Span;
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

    /// Clear screen: >>!
    pub(crate) fn execute_clear_screen(&mut self, cs: &ClearScreen) -> Result<()> {
        crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
            crossterm::cursor::MoveTo(0, 0)
        ).map_err(|e| RuntimeError::Generic { message: e.to_string(), span: cs.span })
    }

    /// Terminal size query: >>?  — returns (rows, cols)
    pub(crate) fn eval_terminal_size(&mut self, span: Span) -> Result<Value> {
        let (cols, rows) = crossterm::terminal::size()
            .map_err(|e| RuntimeError::Generic { message: e.to_string(), span })?;
        Ok(Value::Tuple(vec![Value::Int(rows as i64), Value::Int(cols as i64)]))
    }

    /// Blocking / non-blocking key input: <<| var  or  <<|? var
    pub(crate) fn execute_key_input(&mut self, ki: &KeyInput) -> Result<()> {
        use crossterm::event::{self, Event, KeyEvent};
        let ch = if ki.blocking {
            loop {
                match event::read().map_err(|e| RuntimeError::Generic {
                    message: e.to_string(), span: ki.span,
                })? {
                    Event::Key(KeyEvent { code, .. }) => break map_key_code(code),
                    _ => continue,
                }
            }
        } else {
            if event::poll(std::time::Duration::ZERO).unwrap_or(false) {
                match event::read().unwrap_or(Event::FocusLost) {
                    Event::Key(KeyEvent { code, .. }) => map_key_code(code),
                    _ => '\0',
                }
            } else { '\0' }
        };
        self.set_variable(&ki.variable, Value::Char(ch));
        Ok(())
    }

    /// Positioned output: >>~ (row, col [, fg [, bg]]) > items
    pub(crate) fn execute_output_pos(&mut self, op: &OutputPos) -> Result<()> {
        use crossterm::{execute, cursor, style};
        let pos_val = self.eval_expr(&op.pos)?;
        let (row, col, fg, bg) = extract_pos_tuple(pos_val, op.span)?;
        execute!(std::io::stdout(), cursor::MoveTo(col - 1, row - 1))
            .map_err(|e| RuntimeError::Generic { message: e.to_string(), span: op.span })?;
        if fg > 0 {
            execute!(std::io::stdout(),
                style::SetForegroundColor(style::Color::AnsiValue(fg as u8))).ok();
        }
        if bg > 0 {
            execute!(std::io::stdout(),
                style::SetBackgroundColor(style::Color::AnsiValue(bg as u8))).ok();
        }
        for expr in &op.items {
            let value = self.eval_expr(expr)?;
            print!("{}", value.to_display_string());
        }
        if fg > 0 || bg > 0 {
            execute!(std::io::stdout(), style::ResetColor).ok();
        }
        std::io::stdout().flush().ok();
        Ok(())
    }

    /// TUI block: >>| { } — alternate screen + raw mode
    pub(crate) fn execute_tui_block(&mut self, tb: &TuiBlock) -> Result<()> {
        use crossterm::{execute, terminal, cursor};
        terminal::enable_raw_mode().map_err(|e| RuntimeError::Generic {
            message: format!("failed to enable raw mode: {}", e), span: tb.span,
        })?;
        execute!(std::io::stdout(), terminal::EnterAlternateScreen, cursor::Hide)
            .map_err(|e| RuntimeError::Generic {
                message: format!("failed to enter alternate screen: {}", e), span: tb.span,
            })?;
        let result = self.execute_block(&tb.body);
        let _ = execute!(std::io::stdout(), terminal::LeaveAlternateScreen, cursor::Show);
        let _ = terminal::disable_raw_mode();
        result
    }
}

fn map_key_code(code: crossterm::event::KeyCode) -> char {
    use crossterm::event::KeyCode::*;
    match code {
        Char(c) => c,
        Up      => '↑',
        Down    => '↓',
        Left    => '←',
        Right   => '→',
        Enter   => '\n',
        Esc     => '\x1B',
        _       => '\0',
    }
}

fn extract_pos_tuple(val: Value, span: Span) -> Result<(u16, u16, i64, i64)> {
    let items = match val {
        Value::Tuple(v) => v,
        other => return Err(RuntimeError::Generic {
            message: format!(">>~ expects tuple, got {}", other.to_display_string()),
            span,
        }),
    };
    if items.len() < 2 || items.len() > 4 {
        return Err(RuntimeError::Generic {
            message: format!(">>~ tuple needs 2–4 elements, got {}", items.len()),
            span,
        });
    }
    let to_u16 = |v: &Value, name: &str| match v {
        Value::Int(n) if *n >= 1 => Ok(*n as u16),
        other => Err(RuntimeError::Generic {
            message: format!(">>~ {}: expected positive Int, got {}", name, other.to_display_string()),
            span,
        }),
    };
    let row = to_u16(&items[0], "row")?;
    let col = to_u16(&items[1], "col")?;
    let fg  = if items.len() > 2 { match &items[2] { Value::Int(n) => *n, _ => 0 } } else { 0 };
    let bg  = if items.len() > 3 { match &items[3] { Value::Int(n) => *n, _ => 0 } } else { 0 };
    Ok((row, col, fg, bg))
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
