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
| 5 | Positioned output | `>>~ (r,c,BKS,fg,bg) > items` — sparse: `>>~(,,,fg,bg)>` | Medium |
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

## Feature 5 — Positioned output: `>>~ (fila,col,BKS,fg,bg) > items`

### Diseño (v2 — revisado)

**Tupla de posición:** `(fila, col, BKS, fg, bg)` — hasta 5 slots, todos opcionales
excepto que fila y col deben ir juntos (ambos presentes o ambos ausentes).

**Slot BKS** (posición 3) — máscara de bits de atributos de texto:

| Valor | Atributos |
|-------|-----------|
| 0 | texto normal (sin atributos) |
| 1 | **negrita** (Bold) |
| 2 | *cursiva* (Italic / K) |
| 3 | negrita + cursiva |
| 4 | subrayado (Underline / S) |
| 5 | negrita + subrayado |
| 6 | cursiva + subrayado |
| 7 | negrita + cursiva + subrayado |

**Slots fg/bg** (posiciones 4 y 5) — color ANSI 0–255.
La **presencia** del slot determina si se aplica; su **valor** es el color real.
El color 0 es gris oscuro válido (no significa "sin color").

**Sintaxis sparse** — específica de la declaración inline de `>>~`.
Las comas son marcadores de posición; un slot vacío = ausente = no tocar ese parámetro:

```
>>~(3, 5, 1, 15, 0)>   ← completo: fila=3 col=5 BKS=1(B) fg=15 bg=0
>>~(3, 5, 0, 15)>       ← sin bg
>>~(3, 5, 1)>           ← solo posición + negrita, sin color
>>~(3, 5)>              ← solo posición
>>~(,,, 15, 0)>         ← sin mover cursor, sin BKS; fg=15 bg=0
>>~(,,,, 11)>           ← solo cambiar bg a 11, nada más
>>~(3, 5,, 10)>         ← posición + sin BKS + fg=10
```

**Semántica de slots ausentes:**

| Slot ausente | Efecto en runtime |
|---|---|
| fila **o** col | no ejecutar `MoveTo` (mantener posición actual) |
| BKS | no aplicar ningún atributo de texto |
| fg | no cambiar color de texto |
| bg | no cambiar color de fondo |

**Reset:** Si se aplicó cualquier atributo o color → `SetAttribute(Attribute::Reset)`
(ESC[0m) que limpia colores Y atributos en una sola secuencia.

**Breaking change vs v1:** el tercer slot era `fg`, ahora es `BKS`.
Todas las llamadas existentes `>>~(fila, col, fg)` deben actualizar a `>>~(fila, col, 0, fg)`.

---

### 5a. Lexer — token `OutputPos`

Sin cambios respecto a v1 — `>>~` → `OutputPos`, ya cubierto en el lookahead de Feature 4.

---

### 5b. AST — node `OutputPos` (actualizado)

**File:** `crates/zymbol-ast/src/io.rs`

El nodo ya no almacena un `Box<Expr>` genérico para la posición.
Almacena directamente los 5 slots como `Vec<Option<Expr>>` para evitar
introducir una construcción sparse en el lenguaje general.

```rust
/// Positioned output: >>~ (fila, col, BKS, fg, bg) > items
/// Sparse inline syntax: >>~(,,,15,0)> — None = slot ausente
#[derive(Debug, Clone)]
pub struct OutputPos {
    pub slots: Vec<Option<Expr>>,  // [fila, col, BKS, fg, bg] — hasta 5, None = ausente
    pub items: Vec<Expr>,
    pub span: Span,
}

impl OutputPos {
    pub fn new(slots: Vec<Option<Expr>>, items: Vec<Expr>, span: Span) -> Self {
        Self { slots, items, span }
    }
}
```

**`lib.rs` Statement enum:** sin cambio — `OutputPos(OutputPos)`.

---

### 5c. Parser — `parse_output_pos()` (actualizado)

**File:** `crates/zymbol-parser/src/io.rs`

Dos rutas de parseo:
- **Inline sparse** `(...)` — parser dedicado que produce `Vec<Option<Expr>>`
- **Variable** `ident` — evalúa en runtime como tupla densa (backward compat)

```rust
pub(crate) fn parse_output_pos(&mut self) -> Result<Statement, Diagnostic> {
    let start_span = self.advance().span; // consume >>~

    let slots: Vec<Option<Expr>> = if matches!(self.peek().kind, TokenKind::LParen) {
        self.parse_sparse_pos_tuple()?          // >>~(fila, col, BKS, fg, bg)
    } else if matches!(self.peek().kind, TokenKind::Ident(_)) {
        // Variable — se evalúa en runtime como tupla densa
        let expr = self.parse_expr()?;
        vec![Some(expr)]                        // señal al intérprete: modo variable
    } else {
        let t = self.peek().clone();
        return Err(Diagnostic::error("expected '(' or variable after >>~")
            .with_span(t.span)
            .with_help("syntax: >>~ (fila, col [, BKS [, fg [, bg]]]) > items"));
    };

    // consumir >
    if !matches!(self.peek().kind, TokenKind::Greater) {
        let t = self.peek().clone();
        return Err(Diagnostic::error("expected '>' after >>~ position")
            .with_span(t.span));
    }
    let gt_span = self.advance().span;

    let items = self.parse_output_items_same_line(gt_span.start.line)?;
    let span = start_span.to(items.last().map(|e| e.span()).unwrap_or(gt_span));
    Ok(Statement::OutputPos(OutputPos::new(slots, items, span)))
}

/// Parsea (slot0, slot1, slot2, slot3, slot4) donde cada slot puede estar vacío.
/// Vacío = None. Máximo 5 slots: [fila, col, BKS, fg, bg].
fn parse_sparse_pos_tuple(&mut self) -> Result<Vec<Option<Expr>>, Diagnostic> {
    self.advance(); // consume (
    let mut slots: Vec<Option<Expr>> = Vec::new();

    loop {
        match self.peek().kind {
            TokenKind::RParen => {
                self.advance(); // consume )
                break;
            }
            TokenKind::Comma => {
                slots.push(None);           // slot vacío
                self.advance();             // consume ,
            }
            _ => {
                let expr = self.parse_expr()?;
                slots.push(Some(expr));
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.advance();         // consume ,
                }
            }
        }
        if slots.len() > 5 {
            let t = self.peek().clone();
            return Err(Diagnostic::error(">>~ position tuple has at most 5 slots: (fila, col, BKS, fg, bg)")
                .with_span(t.span));
        }
    }
    Ok(slots)
}
```

---

### 5d. Interpreter — `execute_output_pos()` (actualizado)

**File:** `crates/zymbol-interpreter/src/io.rs`

```rust
pub(crate) fn execute_output_pos(&mut self, op: &OutputPos) -> Result<()> {
    use crossterm::{execute, cursor, style};

    // Evaluar cada slot presente
    let mut vals: Vec<Option<i64>> = Vec::with_capacity(5);
    for slot in &op.slots {
        match slot {
            None       => vals.push(None),
            Some(expr) => {
                let v = self.eval_expr(expr)?;
                vals.push(match v {
                    Value::Int(n) => Some(n),
                    other => return Err(RuntimeError::Generic {
                        message: format!(">>~ slot expects Int, got {}", other.type_name()),
                        span: op.span,
                    }),
                });
            }
        }
    }

    let get = |i: usize| vals.get(i).copied().flatten();

    // Mover cursor solo si fila Y col están presentes
    let fila = get(0);
    let col  = get(1);
    if let (Some(r), Some(c)) = (fila, col) {
        execute!(std::io::stdout(), cursor::MoveTo(c as u16 - 1, r as u16 - 1))
            .map_err(|e| RuntimeError::Generic { message: e.to_string(), span: op.span })?;
    }

    // Atributos BKS
    let bks = get(2).unwrap_or(0);
    let mut styled = false;
    if bks & 1 != 0 { execute!(std::io::stdout(), style::SetAttribute(style::Attribute::Bold)).ok();       styled = true; }
    if bks & 2 != 0 { execute!(std::io::stdout(), style::SetAttribute(style::Attribute::Italic)).ok();     styled = true; }
    if bks & 4 != 0 { execute!(std::io::stdout(), style::SetAttribute(style::Attribute::Underlined)).ok(); styled = true; }

    // Colores — presencia del slot determina aplicación (0 = gris oscuro válido)
    let mut colored = false;
    if let Some(fg) = get(3) {
        execute!(std::io::stdout(),
            style::SetForegroundColor(style::Color::AnsiValue(fg as u8))).ok();
        colored = true;
    }
    if let Some(bg) = get(4) {
        execute!(std::io::stdout(),
            style::SetBackgroundColor(style::Color::AnsiValue(bg as u8))).ok();
        colored = true;
    }

    // Imprimir items
    for expr in &op.items {
        print!("{}", self.eval_expr(expr)?.to_display_string());
    }

    // Reset total si se aplicó algo (ESC[0m limpia colores Y atributos)
    if styled || colored {
        execute!(std::io::stdout(), style::SetAttribute(style::Attribute::Reset)).ok();
    }

    std::io::stdout().flush().ok();
    Ok(())
}
```

**Nota sobre modo variable** (`>>~ ident > items`): cuando `op.slots` tiene
exactamente un elemento `Some(expr)` que evalúa a un `Value::Tuple`, usar la
lógica de tupla densa — slots presentes por longitud, no por `Option`.
Esta ruta no soporta sparse ni BKS; es solo compat con variables pre-computadas.

---

### 5e. VM — `PrintAt` + `vm_extract_pos` (actualizado)

El compilador emite `Instruction::PrintAt(r_pos, item_regs)`.
El `r_pos` apunta a un `Value::Tuple` con hasta 5 elementos donde `Value::Unit`
representa slot ausente (el compilador emite `LoadUnit` para slots `None`).

```rust
fn vm_extract_pos(val: Value) -> (Option<u16>, Option<u16>, i64, Option<i64>, Option<i64>) {
    let items = match val {
        Value::Tuple(v) => (*v).clone(),
        _ => return (None, None, 0, None, None),
    };
    let get_int = |i: usize| -> Option<i64> {
        match items.get(i) {
            Some(Value::Int(n)) => Some(*n),
            _ => None,                          // Unit o ausente = None
        }
    };
    let fila = get_int(0).map(|n| n as u16);
    let col  = get_int(1).map(|n| n as u16);
    let bks  = get_int(2).unwrap_or(0);
    let fg   = get_int(3);
    let bg   = get_int(4);
    (fila, col, bks, fg, bg)
}
```

---

### Actualización de `dibujo.zy` (breaking change)

Todas las llamadas `>>~(fila, col, fg)` existentes deben actualizar el tercer slot:

```zymbol
// Antes (v1)
>>~ (1, col_m, 8) > "┤"

// Después (v2)
>>~ (1, col_m, 0, 8) > "┤"
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
