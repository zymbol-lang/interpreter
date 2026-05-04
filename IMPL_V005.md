# Implementation Plan — v0.0.5 TUI Primitives

Features designed for the Znake project. All are purely additive — no existing behaviour changes.

**Dependency to do first:** add `crossterm.workspace = true` to
`crates/zymbol-interpreter/Cargo.toml` before touching any crossterm call.

---

## Feature map

| # | Feature | Syntax | Complexity |
|---|---------|--------|------------|
| 1 | Sleep | `@~ N` | Low |
| 2 | Clear screen | `>>!` | Low |
| 3 | Terminal size | `>>?` | Low |
| 4 | Key input | `<<\|` / `<<\|?` | Medium |
| 5 | Positioned output | `>>~ (r,c,fg,bg) > items` | Medium |
| 6 | TUI block | `>>\| { }` | Medium |

Recommended implementation order: 1 → 2 → 3 → 4 → 5 → 6.
Features 2–6 all depend on crossterm being wired in (step 0).

---

## Step 0 — Add crossterm to interpreter crate

**File:** `crates/zymbol-interpreter/Cargo.toml`

```toml
[dependencies]
crossterm = { workspace = true }
```

crossterm 0.28 is already declared in the workspace `Cargo.toml`.

---

## Feature 1 — Sleep: `@~ N`

### 1a. Lexer — new token `AtTilde`

**File:** `crates/zymbol-lexer/src/lib.rs`

Add to `TokenKind` enum (near `AtBreak`, `AtContinue`):

```rust
/// @~ (sleep — only valid inside @ block)
AtTilde,
```

**File:** `crates/zymbol-lexer/src/loops.rs` (or wherever `@` is dispatched)

Find where `@` is lexed. After emitting `AtBreak` / `AtContinue` / `AtLabel` lookahead,
add a branch: if next char is `~` → emit `AtTilde` and consume both chars.

```rust
// inside lex_at() or the @ branch of next_token():
if self.peek() == Some('~') {
    self.advance(); // consume ~
    return Token::new(TokenKind::AtTilde, self.span(start));
}
```

### 1b. AST — new node `Sleep`

**File:** `crates/zymbol-ast/src/loops.rs`

```rust
/// Sleep statement: @~ N  (only valid inside @ block)
#[derive(Debug, Clone)]
pub struct Sleep {
    pub duration: Box<Expr>,  // milliseconds — any integer expression
    pub span: Span,
}

impl Sleep {
    pub fn new(duration: Box<Expr>, span: Span) -> Self {
        Self { duration, span }
    }
}
```

**File:** `crates/zymbol-ast/src/lib.rs`

```rust
// in Statement enum:
/// Sleep statement: @~ N
Sleep(Sleep),

// in pub use loops::{}:
pub use loops::{Break, Continue, Loop, Sleep};
```

### 1c. Parser — `parse_sleep()`

**File:** `crates/zymbol-parser/src/loops.rs`

```rust
/// Parse sleep statement: @~ N
pub(crate) fn parse_sleep(&mut self) -> Result<Statement, Diagnostic> {
    let start_span = self.advance().span; // consume @~
    let duration = self.parse_expr()?;
    let span = start_span.to(&duration.span());
    Ok(Statement::Sleep(Sleep::new(Box::new(duration), span)))
}
```

**File:** `crates/zymbol-parser/src/lib.rs` — add dispatch:

```rust
TokenKind::AtTilde => self.parse_sleep(),
```

Note: loop-context validation (`@~` outside `@` is a semantic error) is done at
runtime in the interpreter, not the parser — consistent with how `@!` and `@>` work
today (the parser emits `Break`/`Continue` without checking nesting).

### 1d. Interpreter — `execute_sleep()`

**File:** `crates/zymbol-interpreter/src/loops.rs`

```rust
pub(crate) fn execute_sleep(&mut self, sleep: &Sleep) -> Result<()> {
    let ms = match self.eval_expr(&sleep.duration)? {
        Value::Int(n) if n >= 0 => n as u64,
        Value::Int(n) => {
            return Err(RuntimeError::Generic {
                message: format!("@~ requires a non-negative duration, got {}", n),
                span: sleep.span,
            });
        }
        other => {
            return Err(RuntimeError::Generic {
                message: format!("@~ requires an integer duration, got {}", other.type_name()),
                span: sleep.span,
            });
        }
    };
    std::thread::sleep(std::time::Duration::from_millis(ms));
    Ok(())
}
```

**File:** `crates/zymbol-interpreter/src/lib.rs` — add dispatch:

```rust
Statement::Sleep(s) => self.execute_sleep(s),
```

---

## Feature 2 — Key input: `<<|` and `<<|?`

### 2a. Lexer — new tokens `KeyBlock` and `KeyNonBlock`

**File:** `crates/zymbol-lexer/src/lib.rs`

```rust
/// <<| — blocking key read (blocks until a key is pressed)
KeyBlock,
/// <<|? — non-blocking key read (returns '' immediately if no key)
KeyNonBlock,
```

**Where `<<` is lexed** (look for `TokenKind::Input`): after emitting `Input`,
add lookahead before returning — or better, expand the `<<` branch:

```rust
// when we see '<' and next is '<':
self.advance(); self.advance(); // consume <<
if self.peek() == Some('|') {
    self.advance(); // consume |
    if self.peek() == Some('?') {
        self.advance(); // consume ?
        return Token::new(TokenKind::KeyNonBlock, self.span(start));
    }
    return Token::new(TokenKind::KeyBlock, self.span(start));
}
return Token::new(TokenKind::Input, self.span(start));
```

### 2b. AST — new node `KeyInput`

**File:** `crates/zymbol-ast/src/io.rs`

```rust
/// Key input statement: <<| var  OR  <<|? var
#[derive(Debug, Clone)]
pub struct KeyInput {
    pub variable: String,
    pub blocking: bool,   // true = <<|, false = <<|?
    pub span: Span,
}

impl KeyInput {
    pub fn new(variable: String, blocking: bool, span: Span) -> Self {
        Self { variable, blocking, span }
    }
}
```

**File:** `crates/zymbol-ast/src/lib.rs`

```rust
// Statement enum:
KeyInput(KeyInput),

// pub use io::{...}:
pub use io::{Input, InputCast, InputPrompt, KeyInput, Newline, Output};
```

### 2c. Parser — `parse_key_input()`

**File:** `crates/zymbol-parser/src/io.rs`

```rust
/// Parse key input: <<| var  OR  <<|? var
pub(crate) fn parse_key_input(&mut self, blocking: bool) -> Result<Statement, Diagnostic> {
    let start_span = self.advance().span; // consume <<| or <<|?

    let var_token = self.peek().clone();
    let variable = match &var_token.kind {
        TokenKind::Ident(name) => {
            self.advance();
            name.clone()
        }
        _ => {
            return Err(Diagnostic::error("expected variable name after key input operator")
                .with_span(var_token.span)
                .with_help(if blocking {
                    "blocking key input syntax: <<| var"
                } else {
                    "non-blocking key input syntax: <<|? var"
                }));
        }
    };

    let span = start_span.to(&var_token.span);
    Ok(Statement::KeyInput(KeyInput::new(variable, blocking, span)))
}
```

**File:** `crates/zymbol-parser/src/lib.rs` — dispatch:

```rust
TokenKind::KeyBlock    => self.parse_key_input(true),
TokenKind::KeyNonBlock => self.parse_key_input(false),
```

### 2d. Interpreter — `execute_key_input()`

**File:** `crates/zymbol-interpreter/src/io.rs`

```rust
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

pub(crate) fn execute_key_input(&mut self, ki: &KeyInput) -> Result<()> {
    let ch = if ki.blocking {
        // <<| — block until a key is pressed
        loop {
            match event::read().map_err(|e| RuntimeError::Generic {
                message: format!("key read error: {}", e),
                span: ki.span,
            })? {
                Event::Key(KeyEvent { code, .. }) => break map_key_code(code),
                _ => continue, // ignore resize and other events
            }
        }
    } else {
        // <<|? — non-blocking: return '' if no key available
        if event::poll(Duration::ZERO).unwrap_or(false) {
            match event::read().unwrap_or(Event::FocusLost) {
                Event::Key(KeyEvent { code, .. }) => map_key_code(code),
                _ => '\0',
            }
        } else {
            '\0' // no key pressed
        }
    };

    // '' in Zymbol = Char('\0') — the game loop checks: ? tecla != ''
    self.set_variable(&ki.variable, Value::Char(ch));
    Ok(())
}

fn map_key_code(code: KeyCode) -> char {
    match code {
        KeyCode::Char(c)  => c,
        KeyCode::Up       => '↑',
        KeyCode::Down     => '↓',
        KeyCode::Left     => '←',
        KeyCode::Right    => '→',
        KeyCode::Enter    => '\n',
        KeyCode::Esc      => '\0', // treat ESC as "no input"
        _                 => '\0',
    }
}
```

**Dispatch** (`lib.rs`):

```rust
Statement::KeyInput(ki) => self.execute_key_input(ki),
```

**Note on raw mode:** `event::read()` and `event::poll()` require the terminal to be
in raw mode to capture single keystrokes without waiting for Enter. Raw mode is
enabled/disabled by `execute_tui_block()` (Feature 7). Inside `>>| { }`, raw mode
is already active. If key input is used outside a TUI block, raw mode must be
enabled/disabled temporarily around the call (or error).

---

## Feature 3 — Clear screen: `>>!`

### 3a. Lexer — new token `OutputClear`

**File:** `crates/zymbol-lexer/src/lib.rs`

```rust
/// >>! — clear screen (only inside >>| { } block)
OutputClear,
```

**Where `>>` is lexed:** expand the `>>` branch with lookahead:

```rust
// after consuming >>:
match self.peek() {
    Some('!') => { self.advance(); Token::new(TokenKind::OutputClear, self.span(start)) }
    Some('?') => { self.advance(); Token::new(TokenKind::OutputQuery, self.span(start)) }
    Some('|') => { self.advance(); Token::new(TokenKind::OutputGate,  self.span(start)) }
    Some('~') => { self.advance(); Token::new(TokenKind::OutputPos,   self.span(start)) }
    _         => Token::new(TokenKind::Output, self.span(start))
}
```

This single lookahead point handles all four `>>` extensions at once.

### 3b. AST — new node `ClearScreen`

**File:** `crates/zymbol-ast/src/io.rs`

```rust
/// Clear screen statement: >>!
#[derive(Debug, Clone)]
pub struct ClearScreen {
    pub span: Span,
}

impl ClearScreen {
    pub fn new(span: Span) -> Self { Self { span } }
}
```

**`lib.rs` Statement enum:**

```rust
ClearScreen(ClearScreen),
```

### 3c. Parser

**File:** `crates/zymbol-parser/src/lib.rs`

```rust
TokenKind::OutputClear => {
    let span = self.advance().span;
    Ok(Statement::ClearScreen(ClearScreen::new(span)))
}
```

### 3d. Interpreter

**File:** `crates/zymbol-interpreter/src/io.rs`

```rust
use crossterm::{execute, terminal, cursor};

pub(crate) fn execute_clear_screen(&mut self, _cs: &ClearScreen) -> Result<()> {
    execute!(
        std::io::stdout(),
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    ).map_err(|e| RuntimeError::Generic {
        message: format!("clear screen error: {}", e),
        span: _cs.span,
    })
}
```

---

## Feature 4 — Terminal size: `>>?`

`>>?` is an **expression** (used on the RHS of assignment), not a statement.
It returns a positional tuple `(rows, cols)` — same order as `>>~(row, col)>`.

### 4a. Lexer — new token `OutputQuery`

Already covered in Feature 4 lexer lookahead table — emit `OutputQuery` when
`>>` is followed by `?`.

### 4b. AST — new expression variant `TerminalSize`

**File:** `crates/zymbol-ast/src/expressions.rs`

Add to `Expr` enum:

```rust
/// >>? — returns current terminal (rows, cols) as a positional tuple
TerminalSize(TerminalSizeExpr),
```

```rust
#[derive(Debug, Clone)]
pub struct TerminalSizeExpr {
    pub span: Span,
}
```

Add span() match arm:

```rust
Expr::TerminalSize(t) => t.span,
```

### 4c. Parser — handle in `parse_primary()`

**File:** `crates/zymbol-parser/src/expressions.rs`

In `parse_primary()`:

```rust
TokenKind::OutputQuery => {
    let span = self.advance().span;
    Ok(Expr::TerminalSize(TerminalSizeExpr { span }))
}
```

No dispatch needed in `parse_statement()` since `>>?` is never used standalone.

### 4d. Interpreter — `eval_terminal_size()`

**File:** `crates/zymbol-interpreter/src/expressions.rs` (or `io.rs`)

```rust
use crossterm::terminal;

pub(crate) fn eval_terminal_size(&mut self, span: Span) -> Result<Value> {
    let (cols, rows) = terminal::size().map_err(|e| RuntimeError::Generic {
        message: format!("terminal size error: {}", e),
        span,
    })?;
    // Return (rows, cols) — consistent with >>~(row, col) order
    Ok(Value::Tuple(vec![
        Value::Int(rows as i64),
        Value::Int(cols as i64),
    ]))
}
```

Add to `eval_expr()` dispatch:

```rust
Expr::TerminalSize(t) => self.eval_terminal_size(t.span),
```

---

## Feature 5 — Positioned output: `>>~ (pos) > items`

### 5a. Lexer — new token `OutputPos`

Already covered in Feature 4 lookahead — `>>~` → `OutputPos`.

### 5b. AST — new node `OutputPos`

**File:** `crates/zymbol-ast/src/io.rs`

```rust
/// Positioned output: >>~ (row, col [, fg [, bg]]) > items
#[derive(Debug, Clone)]
pub struct OutputPos {
    pub pos: Box<Expr>,      // must evaluate to a tuple (2–4 elements)
    pub items: Vec<Expr>,
    pub span: Span,
}

impl OutputPos {
    pub fn new(pos: Box<Expr>, items: Vec<Expr>, span: Span) -> Self {
        Self { pos, items, span }
    }
}
```

**`lib.rs` Statement enum:**

```rust
OutputPos(OutputPos),
```

### 5c. Parser — `parse_output_pos()`

**File:** `crates/zymbol-parser/src/io.rs`

```rust
/// Parse >>~ (pos) > items
pub(crate) fn parse_output_pos(&mut self) -> Result<Statement, Diagnostic> {
    let start_span = self.advance().span; // consume >>~

    // Parse position: either inline tuple (r,c,...) or identifier
    let pos = match &self.peek().kind {
        TokenKind::LParen | TokenKind::Ident(_) => self.parse_expr()?,
        _ => {
            let t = self.peek().clone();
            return Err(Diagnostic::error("expected position tuple or variable after >>~")
                .with_span(t.span)
                .with_help("syntax: >>~ (row, col) > items  OR  >>~ var > items"));
        }
    };

    // Expect closing > of the modifier
    let gt = self.peek().clone();
    if !matches!(gt.kind, TokenKind::Greater) {
        return Err(Diagnostic::error("expected '>' to close >>~ position modifier")
            .with_span(gt.span)
            .with_help("syntax: >>~ (row, col) > items"));
    }
    self.advance(); // consume >

    // Parse output items (same logic as parse_output, reuse helper)
    let items = self.parse_output_items()?;

    let span = start_span.to(
        items.last().map(|e| e.span()).unwrap_or(gt.span)
    );
    Ok(Statement::OutputPos(OutputPos::new(Box::new(pos), items, span)))
}
```

Extract item parsing from `parse_output()` into `parse_output_items()` so both
`parse_output()` and `parse_output_pos()` share the loop.

**Dispatch** (`lib.rs`):

```rust
TokenKind::OutputPos => self.parse_output_pos(),
```

### 5d. Interpreter — `execute_output_pos()`

**File:** `crates/zymbol-interpreter/src/io.rs`

```rust
use crossterm::{execute, cursor, style};

pub(crate) fn execute_output_pos(&mut self, op: &OutputPos) -> Result<()> {
    let pos_val = self.eval_expr(&op.pos)?;

    let (row, col, fg, bg) = extract_pos_tuple(pos_val, op.span)?;

    // crossterm is 0-based; Zymbol uses 1-based rows/cols
    execute!(std::io::stdout(), cursor::MoveTo(col - 1, row - 1))
        .map_err(|e| RuntimeError::Generic { message: e.to_string(), span: op.span })?;

    if fg > 0 {
        execute!(std::io::stdout(),
            style::SetForegroundColor(style::Color::AnsiValue(fg as u8)))
            .map_err(|e| RuntimeError::Generic { message: e.to_string(), span: op.span })?;
    }
    if bg > 0 {
        execute!(std::io::stdout(),
            style::SetBackgroundColor(style::Color::AnsiValue(bg as u8)))
            .map_err(|e| RuntimeError::Generic { message: e.to_string(), span: op.span })?;
    }

    // Output items to stdout directly (bypass W writer which may be buffered)
    for expr in &op.items {
        let value = self.eval_expr(expr)?;
        print!("{}", value.to_display_string());
    }

    if fg > 0 || bg > 0 {
        execute!(std::io::stdout(), style::ResetColor)
            .map_err(|e| RuntimeError::Generic { message: e.to_string(), span: op.span })?;
    }

    use std::io::Write;
    std::io::stdout().flush().ok();
    Ok(())
}

fn extract_pos_tuple(val: Value, span: Span) -> Result<(u16, u16, i64, i64)> {
    let items = match val {
        Value::Tuple(v) => v,
        other => return Err(RuntimeError::Generic {
            message: format!(">>~ expects a tuple, got {}", other.type_name()),
            span,
        }),
    };
    if items.len() < 2 || items.len() > 4 {
        return Err(RuntimeError::Generic {
            message: format!(">>~ tuple must have 2–4 elements (row, col [, fg [, bg]]), got {}", items.len()),
            span,
        });
    }
    let row = as_u16(&items[0], "row", span)?;
    let col = as_u16(&items[1], "col", span)?;
    let fg  = if items.len() > 2 { as_i64(&items[2], "fg",  span)? } else { 0 };
    let bg  = if items.len() > 3 { as_i64(&items[3], "bg",  span)? } else { 0 };
    Ok((row, col, fg, bg))
}
```

---

## Feature 6 — TUI block: `>>| { }`

This is the outermost context. It must be implemented last because:
- It enables raw mode (required by `<<|` / `<<|?`)
- It manages alternate screen lifecycle
- `execute_clear_screen` and `execute_output_pos` work correctly without it,
  but key input requires raw mode

### 6a. Lexer — new token `OutputGate`

Already covered in Feature 4 lookahead — `>>|` → `OutputGate`.

### 6b. AST — new node `TuiBlock`

**File:** `crates/zymbol-ast/src/io.rs`

```rust
/// TUI block: >>| { } — alternate screen + raw mode scope
#[derive(Debug, Clone)]
pub struct TuiBlock {
    pub body: zymbol_ast::Block,
    pub span: Span,
}

impl TuiBlock {
    pub fn new(body: zymbol_ast::Block, span: Span) -> Self {
        Self { body, span }
    }
}
```

**`lib.rs` Statement enum:**

```rust
TuiBlock(TuiBlock),
```

### 6c. Parser — `parse_tui_block()`

**File:** `crates/zymbol-parser/src/io.rs`

```rust
/// Parse >>| { body }
pub(crate) fn parse_tui_block(&mut self) -> Result<Statement, Diagnostic> {
    let start_span = self.advance().span; // consume >>|

    let lbrace = self.peek().clone();
    if !matches!(lbrace.kind, TokenKind::LBrace) {
        return Err(Diagnostic::error("expected '{' after >>|")
            .with_span(lbrace.span)
            .with_help("TUI block syntax: >>| { statements }"));
    }

    let body = self.parse_block()?;
    let span = start_span.to(&body.span);
    Ok(Statement::TuiBlock(TuiBlock::new(body, span)))
}
```

**Dispatch** (`lib.rs`):

```rust
TokenKind::OutputGate => self.parse_tui_block(),
```

### 6d. Interpreter — `execute_tui_block()`

**File:** `crates/zymbol-interpreter/src/io.rs`

```rust
use crossterm::{execute, terminal, cursor};

pub(crate) fn execute_tui_block(&mut self, tb: &TuiBlock) -> Result<()> {
    // Enter TUI context
    terminal::enable_raw_mode().map_err(|e| RuntimeError::Generic {
        message: format!("failed to enable raw mode: {}", e),
        span: tb.span,
    })?;
    execute!(std::io::stdout(), terminal::EnterAlternateScreen, cursor::Hide)
        .map_err(|e| RuntimeError::Generic {
            message: format!("failed to enter alternate screen: {}", e),
            span: tb.span,
        })?;

    // Execute body — catch ALL outcomes so we always restore
    let result = self.execute_block(&tb.body);

    // Restore terminal — unconditional (normal exit, @! break, or error)
    let _ = execute!(std::io::stdout(), terminal::LeaveAlternateScreen, cursor::Show);
    let _ = terminal::disable_raw_mode();

    // Re-raise break/continue/error after cleanup
    result
}
```

**Dispatch** (`lib.rs`):

```rust
Statement::TuiBlock(tb) => self.execute_tui_block(tb),
```

**Note on `@!` inside `>>| { }`:** When a `Break` propagates up through
`execute_block()`, `execute_tui_block()` catches the `Err(BreakSignal)`, restores
the terminal, then re-raises. This guarantees cleanup even on early exit.
The existing `execute_loop()` then catches the re-raised `Break` and stops normally.

---

## Summary of new tokens

| Token | Lexed from | Category |
|-------|-----------|----------|
| `AtTilde` | `@~` | loop control |
| `KeyBlock` | `<<\|` | input |
| `KeyNonBlock` | `<<\|?` | input |
| `OutputClear` | `>>!` | output |
| `OutputQuery` | `>>?` | output |
| `OutputPos` | `>>~` | output |
| `OutputGate` | `>>\|` | output |

All `>>X` tokens share one lookahead point when `>>` is lexed.
All `<<X` tokens share one lookahead point when `<<` is lexed.

## Summary of new AST nodes

| Node | File | Type |
|------|------|------|
| `Sleep` | `loops.rs` | Statement |
| `KeyInput` | `io.rs` | Statement |
| `ClearScreen` | `io.rs` | Statement |
| `OutputPos` | `io.rs` | Statement |
| `TuiBlock` | `io.rs` | Statement |
| `TerminalSizeExpr` | `expressions.rs` | Expr |

## Summary of new interpreter methods

| Method | File | Crossterm API |
|--------|------|---------------|
| `execute_sleep` | `loops.rs` | `std::thread::sleep` |
| `execute_key_input` | `io.rs` | `event::poll`, `event::read` |
| `execute_clear_screen` | `io.rs` | `terminal::Clear`, `cursor::MoveTo` |
| `eval_terminal_size` | `io.rs` | `terminal::size` |
| `execute_output_pos` | `io.rs` | `cursor::MoveTo`, `style::Set*Color` |
| `execute_tui_block` | `io.rs` | `enable_raw_mode`, `Enter/LeaveAlternateScreen` |

---

## Tests plan

For each feature, add tests in the crate where they live:

| Feature | Test file | What to test |
|---------|-----------|-------------|
| 1 | `lexer/literals.rs` | `0x1B` → error, `0x41` → ok |
| 2 | `interpreter/loops.rs` | `@~ 10` inside loop executes, `@~` outside loop → runtime error |
| 3 | (manual only — requires TTY) | `<<\|` blocks, `<<\|?` returns `'\0'` when no key |
| 4 | `interpreter/io.rs` | `>>!` executes without panic (stdout capture) |
| 5 | `interpreter/io.rs` | `[H, W] = >>?` produces Tuple of two non-zero Ints |
| 6 | `interpreter/io.rs` | `>>~ (1,1) > "x"` writes to stdout (crossterm calls) |
| 7 | `interpreter/io.rs` | `>>\ | { >>! }` enters and leaves alternate screen cleanly |
