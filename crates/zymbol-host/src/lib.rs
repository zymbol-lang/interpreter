//! The native host of the register VM (ROADMAP_V0011_ES.md § 3.1): the
//! terminal through crossterm, `std/net` through ureq (feature `http`) and
//! `std/db` through ODBC (feature `db`).
//!
//! `zymbol-vm` depends on none of these, so it builds for a target that has
//! no terminal, no sockets and no ODBC — WebAssembly, which is what 3.4 needs.
//! Whoever embeds the VM installs a host: the CLI and the executable
//! `zymbol build` writes install these two.
//!
//! Everything here was code inside the VM's dispatch loop and stdlib table,
//! moved without changing what it does; the tree-walker keeps its own copy,
//! as it did.

use std::io::{BufRead, Write};

use zymbol_intrinsics::time::{Clock, SystemClock};
use zymbol_vm::host::{self, HttpError, HttpRequest, Style, Terminal, TerminalError};
use zymbol_vm::Value;

#[cfg(feature = "db")]
mod db;

// ── The terminal ─────────────────────────────────────────────────────────────

/// Restores the terminal on drop, regardless of control-flow path.
struct TuiGuard;
impl Drop for TuiGuard {
    fn drop(&mut self) {
        let _ = crossterm::execute!(std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show);
        let _ = crossterm::terminal::disable_raw_mode();
        zymbol_common::screen::closed();
    }
}

/// The process's own terminal.
#[derive(Default)]
pub struct NativeTerminal {
    /// One guard per open `>>|`: dropping one restores the terminal, so they
    /// live as long as the TUI they opened — across every nested run of the
    /// VM's loop.
    tui_stack: Vec<TuiGuard>,
}

impl NativeTerminal {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Terminal for NativeTerminal {
    fn sleep(&mut self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }

    fn clear(&mut self) {
        crossterm::execute!(std::io::stdout(),
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
            crossterm::cursor::MoveTo(0, 0)).ok();
    }

    fn size(&mut self) -> (u16, u16) {
        crossterm::terminal::size().unwrap_or((80, 24))
    }

    fn read_key(&mut self, blocking: bool, _out: &mut dyn Write) -> Result<char, TerminalError> {
        use crossterm::event::{self, Event};
        if blocking {
            loop {
                match event::read() {
                    Ok(Event::Key(key)) if is_key_press(&key) => {
                        return Ok(map_key_code(&key))
                    }
                    Ok(_) => continue,
                    Err(e) => return Err(TerminalError::Abort(e.to_string())),
                }
            }
        }
        // Drain to the first keypress rather than giving up on the first event
        // that is not one — see the tree-walker's execute_key_input for why a
        // single read per call makes a game loop fall a fixed number of ticks
        // behind on Windows.
        let mut found = '\0';
        while event::poll(std::time::Duration::ZERO).unwrap_or(false) {
            match event::read() {
                Ok(Event::Key(key)) if is_key_press(&key) => {
                    found = map_key_code(&key);
                    break;
                }
                Ok(_) => continue,
                Err(_) => break,
            }
        }
        Ok(found)
    }

    fn read_line(&mut self, prompt: Option<&str>) -> Result<String, String> {
        let in_tui = !self.tui_stack.is_empty();
        if let Some(p) = prompt {
            print!("{}", p);
            std::io::stdout().flush().ok();
        }
        if in_tui {
            crossterm::terminal::disable_raw_mode().ok();
            crossterm::execute!(std::io::stdout(), crossterm::cursor::Show).ok();
        }
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line).ok();
        if in_tui {
            crossterm::execute!(std::io::stdout(), crossterm::cursor::Hide).ok();
            if let Err(e) = crossterm::terminal::enable_raw_mode() {
                return Err(format!("input: failed to restore raw mode: {}", e));
            }
        }
        Ok(line)
    }

    fn input_hint(&mut self, hint: &str) {
        host::stdout_hint(hint);
    }

    /// One `>>~`: the move, the style, the text and the reset are queued into one buffer
    /// and reach the terminal in a single write. Each used to be an `execute!` of its own,
    /// and every `execute!` flushes, so the terminal was free to show every state in
    /// between (GAP-ZYE-015, ZyEdit/HALLAZGOS.md). The tree-walker's `emit_positioned`
    /// writes the same bytes the same way; the two must stay alike.
    fn print_at(&mut self, at: Option<(u16, u16)>, style: Style, text: &str) {
        use crossterm::{cursor, queue, style as st};
        let mut buf: Vec<u8> = Vec::new();
        if let Some((r, c)) = at { queue!(buf, cursor::MoveTo(c - 1, r - 1)).ok(); }
        if style.bold      { queue!(buf, st::SetAttribute(st::Attribute::Bold)).ok(); }
        if style.italic    { queue!(buf, st::SetAttribute(st::Attribute::Italic)).ok(); }
        if style.underline { queue!(buf, st::SetAttribute(st::Attribute::Underlined)).ok(); }
        if let Some(fg) = style.fg { queue!(buf, st::SetForegroundColor(st::Color::AnsiValue(fg))).ok(); }
        if let Some(bg) = style.bg { queue!(buf, st::SetBackgroundColor(st::Color::AnsiValue(bg))).ok(); }
        buf.extend_from_slice(text.as_bytes());
        if style.bold || style.italic || style.underline || style.fg.is_some() || style.bg.is_some() {
            queue!(buf, st::SetAttribute(st::Attribute::Reset)).ok();
        }
        let mut out = std::io::stdout();
        out.write_all(&buf).ok();
        out.flush().ok();
    }

    fn enter_tui(&mut self) -> Result<bool, String> {
        // The language's words when there is no terminal, as in the
        // tree-walker and zyjs (P4-3 E5, 2026-09-26).
        {
            use std::io::IsTerminal;
            if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
                return Err("failed to enable raw mode: not a terminal".to_string());
            }
        }
        if let Err(e) = crossterm::terminal::enable_raw_mode() {
            return Err(format!("failed to enable raw mode: {}", e));
        }
        if let Err(e) = crossterm::execute!(std::io::stdout(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::cursor::MoveTo(0, 0),
            crossterm::cursor::Hide)
        {
            let _ = crossterm::terminal::disable_raw_mode();
            return Err(format!("failed to enter alternate screen: {}", e));
        }
        self.tui_stack.push(TuiGuard);
        zymbol_common::screen::opened();
        Ok(false)
    }

    /// Pop the guard — its Drop performs cleanup. If the VM's `ExitTui` is
    /// skipped (`@:label!`, an error), the guard is dropped by `release`.
    fn exit_tui(&mut self, _out: &mut dyn Write) -> std::io::Result<()> {
        self.tui_stack.pop();
        Ok(())
    }

    fn write(&mut self, _text: &str) {}

    fn release(&mut self) {
        self.tui_stack.clear();
    }
}

/// Is this a key going *down*, as opposed to coming back up?
///
/// Mirrors the tree-walker's `is_key_press` — Windows reports key releases as well
/// as presses, so `<<|` counted every keystroke twice there. See the comment on the
/// tree-walker copy for the full story; the two must agree or the engines diverge on
/// exactly the platform where the behaviour is hard to notice.
fn is_key_press(key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyEventKind;
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

/// Translate a key event into the single character `<<|` yields.
///
/// Mirrors the tree-walker's `map_key_code`; the two must agree or the engines
/// diverge on the keyboard, which only the pty harness can catch.
fn map_key_code(key: &crossterm::event::KeyEvent) -> char {
    use crossterm::event::{KeyCode::*, KeyModifiers};

    // Ctrl+letter is a control character, and that is what the terminal puts on
    // the wire: Ctrl+A is 0x01, Ctrl+S is 0x13. crossterm hands it over
    // decoded — `Char('a')` with CONTROL set — and this function used to read
    // the code and drop the modifiers, so Ctrl+A arrived as the letter `a`
    // (BUG-ZYB-006). Not "the combination never arrived": it arrived wearing
    // another key's clothes, which is worse — a Ctrl+X shortcut fired when the
    // user typed an x into a text field, and no full-screen program could offer
    // Ctrl+S, Ctrl+Q or Ctrl+C at all.
    //
    // Handing back the control character adds nothing to the language: `0d1`
    // already writes it, and `##!t < 32` already asks the question.
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if let Char(c) = key.code {
            let lower = c.to_ascii_lowercase();
            if lower.is_ascii_lowercase() {
                return (lower as u8 - b'a' + 1) as char;
            }
        }
    }

    match key.code {
        Char(c) => c,
        Up      => '\u{2191}',
        Down    => '\u{2193}',
        Left    => '\u{2190}',
        Right   => '\u{2192}',
        Enter   => '\n',
        Esc     => '\x1B',
        // Tab and Backspace used to fall through to `'\0'` together, so a
        // program could not tell them apart — harmless in a numeric field,
        // which is why ZyBank treated both as "delete", and impossible in a
        // form where Tab moves between fields: every jump would erase a
        // character. Both now carry what the terminal sends for them.
        Tab       => '\t',      // 0d9
        Backspace => '\x7F',    // 0d127 — DEL, which is what a terminal sends
        _         => '\0',
    }
}

// ── The rest of the system ───────────────────────────────────────────────────

/// The machine's clock, its network (feature `http`) and its databases
/// (feature `db`).
#[derive(Default)]
pub struct NativeHost;

impl NativeHost {
    pub fn new() -> Self {
        Self
    }
}

impl host::Host for NativeHost {
    fn clock(&self) -> &dyn Clock {
        &SystemClock
    }

    fn entropy(&mut self) -> u64 {
        host::clock_entropy()
    }

    #[cfg(feature = "http")]
    fn http(&mut self, request: HttpRequest<'_>) -> Result<String, HttpError> {
        let limit = request.timeout.map(std::time::Duration::from_secs_f64);
        let resp = match (request.method, request.body) {
            (host::HttpMethod::Get, _) => {
                let mut req = ureq::get(request.url).config().timeout_global(limit).build();
                for (k, v) in request.headers {
                    req = req.header(k, v);
                }
                req.call()
            }
            (host::HttpMethod::Post, body) => {
                let (content_type, body) = body.unwrap_or(("text/plain", ""));
                let mut req = ureq::post(request.url).config().timeout_global(limit).build()
                    .header("Content-Type", content_type);
                for (k, v) in request.headers {
                    req = req.header(k, v);
                }
                req.send(body)
            }
        };
        // Out of time, in the words every engine uses (ZethyCLI G1).
        let said = |e: ureq::Error| match (e, request.timeout) {
            (ureq::Error::Timeout(_), Some(s)) => zymbol_common::net::timeout_message(s),
            (e, _) => e.to_string(),
        };
        match resp {
            Ok(mut resp) => resp.body_mut().read_to_string().map_err(|e| HttpError::Network(said(e))),
            Err(e) => Err(HttpError::Network(said(e))),
        }
    }

    #[cfg(not(feature = "http"))]
    fn http(&mut self, _request: HttpRequest<'_>) -> Result<String, HttpError> {
        Err(HttpError::Unavailable(host::NET_UNAVAILABLE.to_string()))
    }

    #[cfg(feature = "http")]
    fn http_head(&mut self, url: &str, timeout: Option<f64>) -> Result<bool, HttpError> {
        let limit = timeout.map(std::time::Duration::from_secs_f64);
        Ok(ureq::head(url).config().timeout_global(limit).build().call().is_ok())
    }

    #[cfg(not(feature = "http"))]
    fn http_head(&mut self, _url: &str, _timeout: Option<f64>) -> Result<bool, HttpError> {
        Err(HttpError::Unavailable(host::NET_UNAVAILABLE.to_string()))
    }

    #[cfg(feature = "db")]
    fn db(&mut self, builtin_id: u16, args: Vec<Value>) -> Result<Value, String> {
        db::db_dispatch(builtin_id, args)
    }

    #[cfg(not(feature = "db"))]
    fn db(&mut self, _builtin_id: u16, _args: Vec<Value>) -> Result<Value, String> {
        Err(host::DB_UNAVAILABLE.to_string())
    }
}
