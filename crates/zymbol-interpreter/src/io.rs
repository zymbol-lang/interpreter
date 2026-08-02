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
use crate::data_ops::{ascii_digits, parse_numeric_string};
use crate::{Interpreter, Result, RuntimeError, Value};

impl<W: Write> Interpreter<W> {
    /// Execute output statement: >> expr1 expr2 ...
    ///
    /// Numeric values (`Int`, `Float`, `Bool`) are rendered using the active
    /// numeral mode, nested ones included; all other values use their standard
    /// display form.
    pub(crate) fn execute_output(&mut self, output: &Output) -> Result<()> {
        for expr in &output.exprs {
            let value = self.eval_expr(expr)?;
            let s = self.format_value(&value);
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
        // Build the prompt text once; it is re-printed on every (re-)prompt.
        let prompt_text = match &input.prompt {
            None => String::new(),
            Some(InputPrompt::Simple(s)) => s.clone(),
            Some(InputPrompt::Interpolated(parts)) => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        StringPart::Text(text) => result.push_str(text),
                        StringPart::Variable(var_name) => {
                            if let Some(value) = self.get_variable(var_name) {
                                result.push_str(&self.format_value(&value));
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

        // Read / validate / re-prompt loop. The raw (untrimmed) line is empty *only*
        // at EOF (a blank line the user typed comes back as "\n"); EOF aborts so a
        // failed constraint cannot spin forever on a closed pipe.
        loop {
            if input.prompt.is_some() {
                write!(self.output, "{}", prompt_text)?;
                self.output.flush()?;
            }

            // Inside a TUI block (raw mode active), temporarily disable raw mode and show
            // the cursor so the user can type with echo and press Enter, then restore.
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

            // EOF: no constraint can be satisfied from here, abort instead of looping.
            if line.is_empty() {
                return Err(RuntimeError::Generic {
                    message: format!(
                        "end of input while waiting for {}",
                        describe_input_cast(&input.cast)
                    ),
                    span: input.span,
                });
            }

            match validate_input(line.trim(), &input.cast) {
                Ok(value) => {
                    self.set_variable(&input.variable, value);
                    return Ok(());
                }
                Err(hint) => {
                    // Re-prompt: show what was expected, then loop and ask again.
                    writeln!(self.output, "  ({})", hint)?;
                    self.output.flush()?;
                }
            }
        }
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
    ///
    /// With no terminal — a pipe, a container, CI — this falls back to the
    /// conventional 80x24 rather than failing, so a TUI program stays runnable
    /// when its output is redirected. This used to propagate the OS error while
    /// the VM already fell back, which meant `>>?` aborted under one engine and
    /// returned a size under the other; identical in a real terminal, so the
    /// parity suite never saw it until it ran inside a container.
    pub(crate) fn eval_terminal_size(&mut self, _span: Span) -> Result<Value> {
        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
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
                    for item in &op.items {
                        let v = self.eval_expr(item)?;
                        print!("{}", self.format_value(&v));
                    }
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

        for expr in &op.items {
            let v = self.eval_expr(expr)?;
            print!("{}", self.format_value(&v));
        }

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


/// Human-readable description of what an input cast expects (used in re-prompt hints
/// and the EOF error). Kept in sync with the VM's equivalent message in `stdlib`/`lib`.
fn describe_input_cast(cast: &InputCast) -> String {
    match cast {
        InputCast::String | InputCast::Text { max: None } => "text".to_string(),
        InputCast::Numeric | InputCast::Float => "a number".to_string(),
        InputCast::Decimal { total, decimals } => format!(
            "a number with up to {} digits and {} decimals", total, decimals
        ),
        InputCast::Int { max_digits: Some(n) } => format!("an integer of up to {} digits", n),
        InputCast::Int { max_digits: None } => "an integer".to_string(),
        InputCast::Text { max: Some(n) } => format!("text of up to {} characters", n),
        InputCast::Char => "a single character".to_string(),
    }
}

/// Validate a trimmed input line against a cast, producing the typed value or an
/// `Err(hint)` describing what was expected (the caller re-prompts on `Err`).
///
/// The numeric casts accept digits from any of the 69 supported scripts: an
/// application that prints `४२` must also accept `४२` back, or its own user
/// cannot type what the program just showed them.
fn validate_input(s: &str, cast: &InputCast) -> std::result::Result<Value, String> {
    match cast {
        InputCast::String => Ok(Value::String(s.to_string())),
        InputCast::Numeric => Ok(parse_numeric_string(s.to_string())),
        InputCast::Float => ascii_digits(s).parse::<f64>().map(Value::Float).map_err(|_| describe_input_cast(cast)),
        InputCast::Decimal { total, decimals } => validate_decimal(&ascii_digits(s), *total, *decimals)
            .map(Value::Float)
            .ok_or_else(|| describe_input_cast(cast)),
        InputCast::Int { max_digits } => validate_int(&ascii_digits(s), *max_digits)
            .map(Value::Int)
            .ok_or_else(|| describe_input_cast(cast)),
        InputCast::Text { max } => {
            let too_long = matches!(max, Some(n) if s.chars().count() > *n as usize);
            if too_long { Err(describe_input_cast(cast)) } else { Ok(Value::String(s.to_string())) }
        }
        InputCast::Char => {
            let mut it = s.chars();
            match (it.next(), it.next()) {
                (Some(c), None) => Ok(Value::Char(c)),
                _ => Err(describe_input_cast(cast)),
            }
        }
    }
}

/// Parse `s` as an integer with at most `max_digits` digits (ignoring an optional
/// leading sign). Returns `None` if it is not an integer or exceeds the digit budget.
fn validate_int(s: &str, max_digits: Option<u32>) -> Option<i64> {
    let n: i64 = s.parse().ok()?;
    if let Some(maxd) = max_digits {
        let digits = s.chars().filter(|c| c.is_ascii_digit()).count();
        if digits > maxd as usize {
            return None;
        }
    }
    Some(n)
}

/// Parse `s` as a fixed-format decimal: an optional sign, digits, at most one `.`,
/// at most `decimals` fractional digits and at most `total` digits overall. Rejects
/// scientific notation. Returns the value as `f64`, or `None` if any rule is broken.
fn validate_decimal(s: &str, total: u32, decimals: u32) -> Option<f64> {
    let value: f64 = s.parse().ok()?;
    let body = s.strip_prefix(['+', '-']).unwrap_or(s);
    let mut int_digits = 0u32;
    let mut frac_digits = 0u32;
    let mut seen_dot = false;
    for c in body.chars() {
        if c == '.' {
            if seen_dot {
                return None;
            }
            seen_dot = true;
        } else if c.is_ascii_digit() {
            if seen_dot { frac_digits += 1; } else { int_digits += 1; }
        } else {
            return None;
        }
    }
    if frac_digits > decimals || int_digits + frac_digits > total {
        return None;
    }
    Some(value)
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
