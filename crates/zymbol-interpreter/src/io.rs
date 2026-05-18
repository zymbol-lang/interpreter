//! IO execution for Zymbol-Lang
//!
//! Handles runtime execution of IO statements:
//! - Output: write expressions to output stream
//! - Input: read from stdin, store in variable
//! - Newline: write newline to output

use std::io::Write;
use zymbol_ast::{ClearScreen, Input, InputCast, InputPrompt, KeyInput, Newline, Output, OutputPos, TuiBlock};
use zymbol_span::Span;
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
        // In TUI mode (raw mode active) stdout is line-buffered: text without \n stays
        // invisible until the next flush. Force flush so >> output appears immediately.
        if self.tui_depth > 0 {
            self.output.flush()?;
        }
        Ok(())
    }

    /// Execute newline statement: ¶ OR \\
    pub(crate) fn execute_newline(&mut self, _newline: &Newline) -> Result<()> {
        // In raw mode (inside >>| TUI block) \n alone doesn't return to col 1 — need \r\n.
        if self.tui_depth > 0 {
            write!(self.output, "\r\n")?;
        } else {
            writeln!(self.output)?;
        }
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
        // Inside a TUI block (raw mode active), temporarily disable raw mode and show the
        // cursor so the user can type with echo and press Enter normally, then restore after.
        if self.tui_depth > 0 {
            crossterm::terminal::disable_raw_mode().map_err(|e| RuntimeError::Generic {
                message: format!("input: failed to disable raw mode: {}", e),
                span: input.span,
            })?;
            crossterm::execute!(std::io::stdout(), crossterm::cursor::Show).ok();
        }
        let line_result = (self.input_fn)();
        if self.tui_depth > 0 {
            crossterm::execute!(std::io::stdout(), crossterm::cursor::Hide).ok();
            crossterm::terminal::enable_raw_mode().map_err(|e| RuntimeError::Generic {
                message: format!("input: failed to restore raw mode: {}", e),
                span: input.span,
            })?;
        }
        let line = line_result.map_err(|e| RuntimeError::Generic {
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

    /// Positioned output: >>~ (fila, col, BKS, fg, bg) > items
    /// Sparse syntax: >>~(,,,15,0)> — None slot = do not touch that parameter.
    pub(crate) fn execute_output_pos(&mut self, op: &OutputPos) -> Result<()> {
        use crossterm::{execute, cursor, style};

        // Mode: single-slot variable → dense tuple evaluated at runtime
        if op.slots.len() == 1 {
            if let Some(expr) = &op.slots[0] {
                let val = self.eval_expr(expr)?;
                if let Value::Tuple(ref items) = val {
                    let get_int = |i: usize| -> Option<i64> {
                        match items.get(i) { Some(Value::Int(n)) => Some(*n), _ => None }
                    };
                    if let (Some(r), Some(c)) = (get_int(0), get_int(1)) {
                        execute!(std::io::stdout(), cursor::MoveTo(c as u16 - 1, r as u16 - 1))
                            .map_err(|e| RuntimeError::Generic { message: e.to_string(), span: op.span })?;
                    }
                    let bks = get_int(2).unwrap_or(0);
                    let mut styled = false;
                    if bks & 1 != 0 { execute!(std::io::stdout(), style::SetAttribute(style::Attribute::Bold)).ok();       styled = true; }
                    if bks & 2 != 0 { execute!(std::io::stdout(), style::SetAttribute(style::Attribute::Italic)).ok();     styled = true; }
                    if bks & 4 != 0 { execute!(std::io::stdout(), style::SetAttribute(style::Attribute::Underlined)).ok(); styled = true; }
                    let mut colored = false;
                    if let Some(fg) = get_int(3) {
                        execute!(std::io::stdout(), style::SetForegroundColor(style::Color::AnsiValue(fg as u8))).ok();
                        colored = true;
                    }
                    if let Some(bg) = get_int(4) {
                        execute!(std::io::stdout(), style::SetBackgroundColor(style::Color::AnsiValue(bg as u8))).ok();
                        colored = true;
                    }
                    for item in &op.items { print!("{}", self.eval_expr(item)?.to_display_string()); }
                    if styled || colored { execute!(std::io::stdout(), style::SetAttribute(style::Attribute::Reset)).ok(); }
                    std::io::stdout().flush().ok();
                    return Ok(());
                }
            }
        }

        // Normal sparse/inline mode: evaluate each slot independently
        let mut vals: Vec<Option<i64>> = Vec::with_capacity(5);
        for slot in &op.slots {
            match slot {
                None => vals.push(None),
                Some(expr) => {
                    let v = self.eval_expr(expr)?;
                    vals.push(match v {
                        Value::Int(n) => Some(n),
                        other => return Err(RuntimeError::Generic {
                            message: format!(">>~ slot expects Int, got {}", other.to_display_string()),
                            span: op.span,
                        }),
                    });
                }
            }
        }

        let get = |i: usize| vals.get(i).copied().flatten();

        if let (Some(r), Some(c)) = (get(0), get(1)) {
            execute!(std::io::stdout(), cursor::MoveTo(c as u16 - 1, r as u16 - 1))
                .map_err(|e| RuntimeError::Generic { message: e.to_string(), span: op.span })?;
        }

        let bks = get(2).unwrap_or(0);
        let mut styled = false;
        if bks & 1 != 0 { execute!(std::io::stdout(), style::SetAttribute(style::Attribute::Bold)).ok();       styled = true; }
        if bks & 2 != 0 { execute!(std::io::stdout(), style::SetAttribute(style::Attribute::Italic)).ok();     styled = true; }
        if bks & 4 != 0 { execute!(std::io::stdout(), style::SetAttribute(style::Attribute::Underlined)).ok(); styled = true; }

        let mut colored = false;
        if let Some(fg) = get(3) {
            execute!(std::io::stdout(), style::SetForegroundColor(style::Color::AnsiValue(fg as u8))).ok();
            colored = true;
        }
        if let Some(bg) = get(4) {
            execute!(std::io::stdout(), style::SetBackgroundColor(style::Color::AnsiValue(bg as u8))).ok();
            colored = true;
        }

        for expr in &op.items { print!("{}", self.eval_expr(expr)?.to_display_string()); }

        if styled || colored {
            execute!(std::io::stdout(), style::SetAttribute(style::Attribute::Reset)).ok();
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
        execute!(std::io::stdout(), terminal::EnterAlternateScreen, cursor::MoveTo(0, 0), cursor::Hide)
            .map_err(|e| RuntimeError::Generic {
                message: format!("failed to enter alternate screen: {}", e), span: tb.span,
            })?;
        self.tui_depth += 1;
        let result = self.execute_block(&tb.body);
        self.tui_depth -= 1;
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
