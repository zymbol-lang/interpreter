//! IO execution for Zymbol-Lang
//!
//! Handles runtime execution of IO statements:
//! - Output: write expressions to output stream
//! - Input: read from stdin, store in variable
//! - Newline: write newline to output

use std::io::Write;
use zymbol_ast::{ClearScreen, Expr, Input, InputCast, InputPrompt, KeyInput, Newline, Output, OutputPos, TuiBlock};
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
            // Inside `>>|` under --keys the text lands on the virtual screen.
            if self.tui_depth > 0 {
                if let Some(h) = self.headless.as_mut() {
                    h.screen.write(&s);
                    continue;
                }
            }
            // Inside `>>|`, into the frame, in the order it was written.
            if self.tui_depth > 0 {
                self.draw(s.as_bytes());
                continue;
            }
            write!(self.output, "{}", s)?;
        }
        Ok(())
    }

    /// Execute newline statement: ¶ OR \\
    pub(crate) fn execute_newline(&mut self, _newline: &Newline) -> Result<()> {
        if self.tui_depth > 0 {
            if let Some(h) = self.headless.as_mut() {
                h.screen.write("\n");
                return Ok(());
            }
        }
        // In raw mode (inside >>| TUI block) \n alone doesn't return to col 1 — need \r\n.
        // The VM wrote \n until ZYVM-033.
        if self.tui_depth > 0 {
            self.draw(b"\r\n");
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
            // A wait: inside `>>|` the prompt joins the frame, and the frame is shown.
            if self.tui_depth > 0 && self.headless.is_none() {
                if input.prompt.is_some() {
                    self.draw(prompt_text.as_bytes());
                }
                self.show_frame();
            } else if input.prompt.is_some() {
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
                // Back where the program left it: hidden, or at its `>>~ (row, col)`.
                let mut out = std::io::stdout();
                out.write_all(&zymbol_common::screen::caret_bytes(self.caret)).ok();
                out.flush().ok();
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
        if let Some(h) = self.headless.as_mut() {
            h.screen.clear();
            return Ok(());
        }
        let mut buf: Vec<u8> = Vec::new();
        crossterm::queue!(
            buf,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
            crossterm::cursor::MoveTo(0, 0)
        ).map_err(|e| RuntimeError::Generic { message: e.to_string(), span: cs.span })?;
        self.draw(&buf);
        Ok(())
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
        let (cols, rows) = if let Some(h) = self.headless.as_ref() {
            h.size_cols_rows()
        } else {
            crossterm::terminal::size().unwrap_or((80, 24))
        };
        Ok(Value::tuple(vec![Value::Int(rows as i64), Value::Int(cols as i64)]))
    }

    /// Blocking / non-blocking key input: <<| var  or  <<|? var, and the
    /// modifiers with <<| var, mods (D-14)
    pub(crate) fn execute_key_input(&mut self, ki: &KeyInput) -> Result<()> {
        use crossterm::event::{self, Event};
        // Under --keys the keys come from the script (D11).
        if let Some(h) = self.headless.as_mut() {
            let (key, frames) = if ki.blocking {
                h.read_blocking()
            } else {
                let (k, f) = h.read_poll();
                (Some(k), f)
            };
            for frame in frames {
                writeln!(self.output, "{}", frame)?;
            }
            let key = key.ok_or_else(|| RuntimeError::Generic {
                message: zymbol_common::vscreen::OUT_OF_KEYS.to_string(), span: ki.span,
            })?;
            self.set_key(ki, key);
            return Ok(());
        }
        // A wait: the terminal gets the frame.
        self.show_frame();
        let key = if ki.blocking {
            loop {
                match event::read().map_err(|e| RuntimeError::Generic {
                    message: e.to_string(), span: ki.span,
                })? {
                    Event::Key(key) if is_key_press(&key) => break map_key_code(&key),
                    // The window changed size: `⤢` (D-17, GAP-ZYE-012).
                    Event::Resize(..) => break resized(),
                    _ => continue,
                }
            }
        } else {
            // Drain until a keypress or until nothing is pending — do not stop at the
            // first event that is not one.
            //
            // Reading a single event per call was enough while every event was a
            // keypress. On Windows each keystroke also delivers a release, so a call
            // that happened to pull the release returned "no key" and left the queue
            // one event longer than it found it. In a game loop calling this once per
            // tick, the backlog grows and every turn arrives late — the same two-cell
            // lag whether the tick is 40 ms or 160 ms, which is what gives it away as
            // a queue and not a timing problem.
            //
            // Draining also covers focus events, which could waste a tick on any
            // platform. A resize is not drained: it is `⤢` (D-17).
            let mut found = ('\0', 0);
            while event::poll(std::time::Duration::ZERO).unwrap_or(false) {
                match event::read() {
                    Ok(Event::Key(key)) if is_key_press(&key) => {
                        found = map_key_code(&key);
                        break;
                    }
                    Ok(Event::Resize(..)) => {
                        found = resized();
                        break;
                    }
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
            found
        };
        self.set_key(ki, key);
        Ok(())
    }

    /// The key into the first variable and, with `<<| k, m`, its modifiers
    /// into the second.
    fn set_key(&mut self, ki: &KeyInput, (ch, mods): (char, i64)) {
        self.set_variable(&ki.variable, Value::Char(ch));
        if let Some(m) = &ki.modifiers {
            self.set_variable(m, Value::Int(mods));
        }
    }

    /// Positioned output: >>~ (fila, col, BKS, fg, bg) > items
    /// Sparse syntax: >>~(,,,15,0)> — None slot = do not touch that parameter.
    pub(crate) fn execute_output_pos(&mut self, op: &OutputPos) -> Result<()> {
        if op.caret {
            return self.execute_caret(op);
        }
        // Mode: single-slot variable → dense tuple evaluated at runtime
        if op.slots.len() == 1 {
            if let Some(expr) = &op.slots[0] {
                let val = self.eval_expr(expr)?;
                if let Value::Tuple(ref items) = val {
                    let get_int = |i: usize| -> Option<i64> {
                        match items.get(i) { Some(Value::Int(n)) => Some(*n), _ => None }
                    };
                    let at = match (get_int(0), get_int(1)) {
                        (Some(r), Some(c)) => Some((r, c)),
                        _ => None,
                    };
                    if self.headless.is_some() {
                        if let (Some((r, c)), Some(h)) = (at, self.headless.as_mut()) { h.screen.move_to(r, c); }
                        for item in &op.items {
                            let v = self.eval_expr(item)?;
                            let s = self.format_value(&v);
                            if let Some(h) = self.headless.as_mut() { h.screen.write(&s); }
                        }
                        return Ok(());
                    }
                    let text = self.positioned_text(&op.items)?;
                    let bytes = positioned(at, get_int(2).unwrap_or(0), get_int(3), get_int(4), &text)
                        .map_err(|e| RuntimeError::Generic { message: e.to_string(), span: op.span })?;
                    self.draw(&bytes);
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
                        // A slot of another type is a wrong TYPE: named by its
                        // type, never its value (GLB-033), and a `##Type` (D1;
                        // GLB-042, decided 2026-10-05). It printed the value —
                        // `got fila`, `got 2.5`.
                        other => return Err(RuntimeError::kinded(
                            "Type",
                            format!(">>~ slot expects Int, got {}", other.type_ident()),
                            op.span,
                        )),
                    });
                }
            }
        }

        let get = |i: usize| vals.get(i).copied().flatten();

        // Under --keys: position and text on the virtual screen; a frame is
        // read for what it says, so styles and colours are not kept (D11).
        if self.headless.is_some() {
            if let (Some(r), Some(c)) = (get(0), get(1)) {
                if let Some(h) = self.headless.as_mut() { h.screen.move_to(r, c); }
            }
            for expr in &op.items {
                let v = self.eval_expr(expr)?;
                let s = self.format_value(&v);
                if let Some(h) = self.headless.as_mut() { h.screen.write(&s); }
            }
            return Ok(());
        }

        let at = match (get(0), get(1)) {
            (Some(r), Some(c)) => Some((r, c)),
            _ => None,
        };
        let text = self.positioned_text(&op.items)?;
        let bytes = positioned(at, get(2).unwrap_or(0), get(3), get(4), &text)
            .map_err(|e| RuntimeError::Generic { message: e.to_string(), span: op.span })?;
        self.draw(&bytes);
        Ok(())
    }

    /// Bytes for the real terminal: inside `>>|`, into the frame; outside it,
    /// written at once, as they always were.
    fn draw(&mut self, bytes: &[u8]) {
        if self.tui_depth > 0 {
            zymbol_common::screen::put(&mut self.frame, bytes, self.caret);
        } else {
            let mut out = std::io::stdout();
            out.write_all(bytes).ok();
            out.flush().ok();
        }
    }

    /// `>>~ (row, col)` with nothing to print: the terminal's cursor is shown
    /// there, and `>>~ (0, 0)` hides it (D-16, DIS-ZYE-004). It is a place on
    /// the full screen, so it is refused outside `>>|`, where the cursor is
    /// the shell's. The VM's `ShowCursor` says the same, in the same words.
    fn execute_caret(&mut self, op: &OutputPos) -> Result<()> {
        let mut place: Vec<i64> = Vec::with_capacity(2);
        let lone = op.slots.len() == 1;
        for slot in op.slots.iter().flatten() {
            let v = self.eval_expr(slot)?;
            match (v, lone) {
                (Value::Int(n), false) => place.push(n),
                (Value::Tuple(ref items), true) if items.len() == 2 => {
                    for it in items.iter() {
                        match it {
                            Value::Int(n) => place.push(*n),
                            other => return Err(RuntimeError::kinded("Type",
                                format!(">>~ slot expects Int, got {}", other.type_ident()), op.span)),
                        }
                    }
                }
                (other, true) => return Err(RuntimeError::kinded("Type",
                    format!("{}, got {}", zymbol_common::screen::CARET_SHAPE, other.type_ident()), op.span)),
                (other, false) => return Err(RuntimeError::kinded("Type",
                    format!(">>~ slot expects Int, got {}", other.type_ident()), op.span)),
            }
        }
        let caret = zymbol_common::screen::caret(place[0], place[1])
            .map_err(|m| RuntimeError::kinded("Range", m.to_string(), op.span))?;
        if self.tui_depth == 0 {
            return Err(RuntimeError::Generic {
                message: zymbol_common::screen::CARET_OUTSIDE.to_string(),
                span: op.span,
            });
        }
        self.caret = caret;
        if let Some(h) = self.headless.as_mut() {
            h.screen.set_caret(caret);
            return Ok(());
        }
        let bytes = zymbol_common::screen::caret_bytes(caret);
        self.draw(&bytes);
        Ok(())
    }

    /// The program waits, or its `>>|` ends: the terminal gets the frame
    /// (GAP-ZYE-015). The VM's native terminal does the same at the same points.
    pub(crate) fn show_frame(&mut self) {
        zymbol_common::screen::show(&mut self.frame, self.caret);
    }

    /// The items of a `>>~`, evaluated and joined before anything is written — as the VM
    /// does, which compiles them to one string before it moves the cursor.
    fn positioned_text(&mut self, items: &[Expr]) -> Result<String> {
        let mut text = String::new();
        for item in items {
            let v = self.eval_expr(item)?;
            text.push_str(&self.format_value(&v));
        }
        Ok(text)
    }

    /// TUI block: >>| { } — alternate screen + raw mode
    pub(crate) fn execute_tui_block(&mut self, tb: &TuiBlock) -> Result<()> {
        use crossterm::{execute, terminal, cursor};
        // --keys (D11): no terminal is taken over. The block draws on the
        // virtual screen, and its last frame is written to the output when it
        // ends. Only when it ends normally: a block that fails is reported by
        // its error, and the VM, which unwinds past the end of the block, could
        // not print a frame there — the two engines must agree.
        if self.headless.is_some() {
            if let Some(h) = self.headless.as_mut() { h.screen.open(); }
            self.tui_depth += 1;
            let result = self.execute_block(&tb.body);
            self.tui_depth -= 1;
            if result.is_ok() {
                let frame = self.headless.as_ref().map(|h| h.screen.frame()).unwrap_or_default();
                writeln!(self.output, "{}", frame)?;
                self.output.flush()?;
            }
            return result;
        }
        // Without a terminal on both ends the refusal is the language's, not the
        // operating system's — `No such device or address (os error 6)` here is
        // something else on Windows. zyjs says the same (P4-3 E5, 2026-09-26).
        {
            use std::io::IsTerminal;
            if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
                return Err(RuntimeError::Generic {
                    message: "failed to enable raw mode: not a terminal".to_string(), span: tb.span,
                });
            }
        }
        // A `>>|` inside another: what the outer one drew is shown first.
        self.show_frame();
        terminal::enable_raw_mode().map_err(|e| RuntimeError::Generic {
            message: format!("failed to enable raw mode: {}", e), span: tb.span,
        })?;
        execute!(std::io::stdout(), terminal::EnterAlternateScreen, cursor::MoveTo(0, 0), cursor::Hide)
            .map_err(|e| RuntimeError::Generic {
                message: format!("failed to enter alternate screen: {}", e), span: tb.span,
            })?;
        self.tui_depth += 1;
        zymbol_common::screen::opened();
        // A `>>|` begins with the cursor hidden; an outer one gets its own back.
        let outer_caret = self.caret.take();
        let result = self.execute_block(&tb.body);
        // However the block ended — normally, by `@!`, by an error — what it
        // drew reaches the terminal before the terminal is given back.
        self.show_frame();
        self.caret = outer_caret;
        self.tui_depth -= 1;
        let _ = execute!(std::io::stdout(), terminal::LeaveAlternateScreen, cursor::Show);
        let _ = terminal::disable_raw_mode();
        zymbol_common::screen::closed();
        result
    }
}

/// One `>>~` on the real terminal: the move, the style, the text and the reset, as one
/// piece — into the frame inside `>>|`, one write outside it.
///
/// Each piece used to be an `execute!` of its own, and every `execute!` flushes, so one
/// `>>~` with colours was four writes and the terminal was free to show every state in
/// between — a redraw was seen while it was drawn (GAP-ZYE-015, ZyEdit/HALLAZGOS.md). The
/// VM's host writes the same bytes the same way; the two must stay alike.
fn positioned(at: Option<(i64, i64)>, bks: i64, fg: Option<i64>, bg: Option<i64>, text: &str) -> std::io::Result<Vec<u8>> {
    use crossterm::{queue, cursor, style};
    let mut buf: Vec<u8> = Vec::new();
    if let Some((r, c)) = at { queue!(buf, cursor::MoveTo(c as u16 - 1, r as u16 - 1))?; }
    if bks & 1 != 0 { queue!(buf, style::SetAttribute(style::Attribute::Bold))?; }
    if bks & 2 != 0 { queue!(buf, style::SetAttribute(style::Attribute::Italic))?; }
    if bks & 4 != 0 { queue!(buf, style::SetAttribute(style::Attribute::Underlined))?; }
    if let Some(fg) = fg { queue!(buf, style::SetForegroundColor(style::Color::AnsiValue(fg as u8)))?; }
    if let Some(bg) = bg { queue!(buf, style::SetBackgroundColor(style::Color::AnsiValue(bg as u8)))?; }
    buf.extend_from_slice(text.as_bytes());
    if bks & 7 != 0 || fg.is_some() || bg.is_some() {
        queue!(buf, style::SetAttribute(style::Attribute::Reset))?;
    }
    Ok(buf)
}

/// What `<<|` hands a program when the window changed size: `⤢`, with no
/// modifiers (D-17). The VM's host gives the same, from the same rule.
fn resized() -> (char, i64) {
    use zymbol_common::keys::{self, Code, Mods};
    keys::key(Code::Resize, Mods::default())
}

/// Is this a key going *down*, as opposed to coming back up?
///
/// Windows is the only platform that reports key releases: its console API delivers
/// a `KEY_EVENT` for the press and another for the release, and crossterm passes
/// both through. Reading every `Event::Key` therefore counted each keystroke twice —
/// a menu selection skipped an entry, and the snake moved two cells per arrow, which
/// made it impossible to line up a one-cell gap. Unix never sends releases, so this
/// filter changes nothing there.
///
/// `Repeat` counts as a press: a held key auto-repeats on Unix too, arriving as a
/// run of ordinary presses, so accepting it is what keeps the two platforms feeling
/// the same.
fn is_key_press(key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyEventKind;
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

/// Translate a key event into the single character `<<|` yields.
///
/// The tree-walker's copy; `zymbol-vm`'s `vm_map_key_code` must stay identical
/// or the two engines disagree about the keyboard, which no pipe-based suite
/// can see — only `zyquality/tui/run.sh`, through a real pty.
fn map_key_code(key: &crossterm::event::KeyEvent) -> (char, i64) {
    use crossterm::event::{KeyCode as K, KeyModifiers as M};
    use zymbol_common::keys::{self, Code, Mods};
    // What each key is, and which modifiers `m` reports, is decided once in
    // `zymbol_common::keys` (D-13, D-14, D-15): this only names crossterm's
    // event in its terms.
    let code = match key.code {
        K::Char(c) => Code::Char(c),
        K::Up => Code::Up,
        K::Down => Code::Down,
        K::Left => Code::Left,
        K::Right => Code::Right,
        K::Enter => Code::Enter,
        K::Esc => Code::Esc,
        K::Tab => Code::Tab,
        K::BackTab => Code::BackTab,
        K::Backspace => Code::Backspace,
        K::Home => Code::Home,
        K::End => Code::End,
        K::PageUp => Code::PageUp,
        K::PageDown => Code::PageDown,
        K::Delete => Code::Delete,
        K::F(n) => Code::F(n),
        _ => Code::Other,
    };
    let mods = Mods {
        shift: key.modifiers.contains(M::SHIFT),
        alt: key.modifiers.contains(M::ALT),
        ctrl: key.modifiers.contains(M::CONTROL),
    };
    keys::key(code, mods)
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
        InputCast::String => Ok(Value::String(s.to_string().into())),
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
            if too_long { Err(describe_input_cast(cast)) } else { Ok(Value::String(s.to_string().into())) }
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
