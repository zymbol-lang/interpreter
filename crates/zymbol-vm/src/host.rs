//! What the VM asks of the system it runs on (ROADMAP_V0011_ES.md § 3.1).
//!
//! The VM itself depends on nothing a system has to provide: no terminal
//! library, no HTTP client, no ODBC. What a program needs from outside —
//! a terminal for `>>|` and its kin, a clock, the network, a database —
//! comes through two interfaces:
//!
//! - [`Terminal`]: the seven TUI primitives, `@~` and `<<`. The native one
//!   (crossterm) is in `zymbol-host`; [`Headless`] is `zymbol run --keys`;
//!   [`BasicTerminal`] is what a VM has before anyone installs one.
//! - [`Host`]: the clock, the seed of `std/random`, `std/net` and `std/db`.
//!   The native one is in `zymbol-host`; [`BasicHost`] has the machine's clock
//!   and neither the network nor a database.
//!
//! They are two and not one because `--keys` replaces the terminal and
//! nothing else: under it a program still reads the real clock and reaches
//! the real network.
//!
//! `>>` does not go through either. It writes to the VM's own output, which is
//! the hot path, and which the embedder chose already (`VM::new(output)`).

use std::io::{BufRead, Write};

use zymbol_intrinsics::time::{Clock, SystemClock};

use crate::Value;

/// What `>>~` asks for besides the position.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// ANSI colour index, foreground.
    pub fg: Option<u8>,
    /// ANSI colour index, background.
    pub bg: Option<u8>,
}

/// How a terminal operation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalError {
    /// Raised to the program: a `:!` can catch it.
    Raise(String),
    /// Ends the run: no handler sees it.
    Abort(String),
}

/// The terminal a program draws on and reads keys from.
pub trait Terminal {
    /// `@~ ms`.
    fn sleep(&mut self, ms: u64);
    /// `>>!`.
    fn clear(&mut self);
    /// `>>?`: (columns, rows).
    fn size(&mut self) -> (u16, u16);
    /// `<<|` (blocking) and `<<|?` (not). `out` is the VM's output, for a
    /// terminal that reports there what the program drew.
    fn read_key(&mut self, blocking: bool, out: &mut dyn Write) -> Result<char, TerminalError>;
    /// One line for `<<`, after writing `prompt`. An empty string is the end
    /// of input; a typed blank line is `"\n"`. `Err` is raised.
    fn read_line(&mut self, prompt: Option<&str>) -> Result<String, String>;
    /// What `<<` says when the line it read does not fit, before asking again.
    fn input_hint(&mut self, hint: &str);
    /// `>>~`: `at` is (row, column), counted from 1, as the program wrote them.
    fn print_at(&mut self, at: Option<(u16, u16)>, style: Style, text: &str);
    /// The start of a `>>|` block. `Ok(true)` when this terminal is virtual
    /// and a `>>` inside the block is drawn on it with [`Terminal::write`]
    /// instead of going to the VM's output. `Err` is raised.
    fn enter_tui(&mut self) -> Result<bool, String>;
    /// The end of a `>>|` block.
    fn exit_tui(&mut self, out: &mut dyn Write) -> std::io::Result<()>;
    /// A `>>` inside a block that [`Terminal::enter_tui`] said it draws.
    fn write(&mut self, text: &str);
    /// The run is over, however it ended: give the terminal back.
    fn release(&mut self);
}

/// One `std/net` request.
#[derive(Debug, Clone, Copy)]
pub struct HttpRequest<'a> {
    pub method: HttpMethod,
    pub url: &'a str,
    pub headers: &'a [(String, String)],
    /// `(content type, body)` for a POST.
    pub body: Option<(&'a str, &'a str)>,
    /// The time limit the program gave, in seconds; `None` is none (ZethyCLI G1).
    pub timeout: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

/// How a `std/net` request failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// The network said no: a soft `##Network` error, a value.
    Network(String),
    /// This host has no network: a hard error.
    Unavailable(String),
}

/// What [`BasicHost`] answers for `std/net`. Also the native host's without
/// its `http` feature. Unreachable from a compiler without `http`, which
/// refuses `<# std/net`.
pub const NET_UNAVAILABLE: &str = "std/net is not available in this build (compiled without HTTP support)";

/// What [`BasicHost`] answers for `std/db`. Also the native host's without its
/// `db` feature. Unreachable from a compiler without `db`, which refuses
/// `<# std/db`.
pub const DB_UNAVAILABLE: &str = "std/db is not available in this build (compiled without ODBC support)";

/// The system services a program reaches beyond the terminal.
pub trait Host {
    /// `std/time`'s clock and local zone.
    fn clock(&self) -> &dyn Clock;
    /// The seed of `std/random`, read once per thread on its first draw.
    fn entropy(&mut self) -> u64;
    /// `net::get`, `net::post`, `net::post_json`: the response body.
    fn http(&mut self, request: HttpRequest<'_>) -> Result<String, HttpError>;
    /// `net::head`: whether the URL answered with a success.
    fn http_head(&mut self, url: &str, timeout: Option<f64>) -> Result<bool, HttpError>;
    /// A `std/db` builtin, by its id in `zymbol_bytecode::builtins`. `Err` is a
    /// hard error; a database's own failure is a soft `##DB` value in `Ok`.
    fn db(&mut self, builtin_id: u16, args: Vec<Value>) -> Result<Value, String>;
}

/// The seed every host but a test's wants: the sub-second nanoseconds of now.
pub fn clock_entropy() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345) as u64
}

// ── What a VM has before anyone installs a host ──────────────────────────────

/// The machine's clock, and no network and no database.
pub struct BasicHost;

impl Host for BasicHost {
    fn clock(&self) -> &dyn Clock {
        &SystemClock
    }
    fn entropy(&mut self) -> u64 {
        clock_entropy()
    }
    fn http(&mut self, _request: HttpRequest<'_>) -> Result<String, HttpError> {
        Err(HttpError::Unavailable(NET_UNAVAILABLE.to_string()))
    }
    fn http_head(&mut self, _url: &str, _timeout: Option<f64>) -> Result<bool, HttpError> {
        Err(HttpError::Unavailable(NET_UNAVAILABLE.to_string()))
    }
    fn db(&mut self, _builtin_id: u16, _args: Vec<Value>) -> Result<Value, String> {
        Err(DB_UNAVAILABLE.to_string())
    }
}

/// Standard input and output, and no full-screen terminal: a `>>|` block is
/// refused the way the native terminal refuses it in a pipe.
pub struct BasicTerminal;

impl Terminal for BasicTerminal {
    fn sleep(&mut self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
    fn clear(&mut self) {}
    fn size(&mut self) -> (u16, u16) {
        (80, 24)
    }
    fn read_key(&mut self, _blocking: bool, _out: &mut dyn Write) -> Result<char, TerminalError> {
        Err(TerminalError::Abort("no terminal to read a key from".to_string()))
    }
    fn read_line(&mut self, prompt: Option<&str>) -> Result<String, String> {
        Ok(stdin_line(prompt))
    }
    fn input_hint(&mut self, hint: &str) {
        stdout_hint(hint);
    }
    fn print_at(&mut self, _at: Option<(u16, u16)>, _style: Style, text: &str) {
        print!("{}", text);
        std::io::stdout().flush().ok();
    }
    fn enter_tui(&mut self) -> Result<bool, String> {
        Err("failed to enable raw mode: not a terminal".to_string())
    }
    fn exit_tui(&mut self, _out: &mut dyn Write) -> std::io::Result<()> {
        Ok(())
    }
    fn write(&mut self, _text: &str) {}
    fn release(&mut self) {}
}

/// `<<` on the process's standard input: write the prompt, read one line.
pub fn stdin_line(prompt: Option<&str>) -> String {
    if let Some(p) = prompt {
        print!("{}", p);
        std::io::stdout().flush().ok();
    }
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).ok();
    line
}

/// `<<`'s hint on the process's standard output.
pub fn stdout_hint(hint: &str) {
    println!("  ({})", hint);
    std::io::stdout().flush().ok();
}

// ── `zymbol run --keys` ──────────────────────────────────────────────────────

/// `zymbol run --keys FILE` (D11): a virtual screen and a script of keys, so
/// a full-screen program can be run without a terminal and its frames
/// compared. `<<` still reads standard input, as it always did under `--keys`.
pub struct Headless {
    state: zymbol_common::vscreen::Headless,
}

impl Headless {
    pub fn new(keys: zymbol_common::vscreen::KeyScript) -> Self {
        Self { state: zymbol_common::vscreen::Headless::new(keys) }
    }
}

impl Terminal for Headless {
    /// The script's WAIT counts polls, not time.
    fn sleep(&mut self, _ms: u64) {}
    fn clear(&mut self) {
        self.state.screen.clear();
    }
    fn size(&mut self) -> (u16, u16) {
        (zymbol_common::vscreen::COLS as u16, zymbol_common::vscreen::ROWS as u16)
    }
    fn read_key(&mut self, blocking: bool, out: &mut dyn Write) -> Result<char, TerminalError> {
        let (ch, frames) = if blocking {
            self.state.read_blocking()
        } else {
            let (k, f) = self.state.read_poll();
            (Some(k), f)
        };
        for frame in frames {
            writeln!(out, "{}", frame).map_err(|e| TerminalError::Abort(e.to_string()))?;
        }
        ch.ok_or_else(|| TerminalError::Raise(zymbol_common::vscreen::OUT_OF_KEYS.to_string()))
    }
    fn read_line(&mut self, prompt: Option<&str>) -> Result<String, String> {
        Ok(stdin_line(prompt))
    }
    fn input_hint(&mut self, hint: &str) {
        stdout_hint(hint);
    }
    /// Position and text on the virtual screen; no styles.
    fn print_at(&mut self, at: Option<(u16, u16)>, _style: Style, text: &str) {
        if let Some((r, c)) = at {
            self.state.screen.move_to(r as i64, c as i64);
        }
        self.state.screen.write(text);
    }
    /// No terminal is taken over.
    fn enter_tui(&mut self) -> Result<bool, String> {
        self.state.screen.clear();
        Ok(true)
    }
    fn exit_tui(&mut self, out: &mut dyn Write) -> std::io::Result<()> {
        writeln!(out, "{}", self.state.screen.frame())?;
        out.flush()
    }
    fn write(&mut self, text: &str) {
        self.state.screen.write(text);
    }
    fn release(&mut self) {}
}
