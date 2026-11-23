//! A screen with no terminal behind it, and the keys that drive it.
//!
//! `zymbol run app.zy --keys app.keys` (GAP-GOL-009, decided 2026-09-30 as D11):
//! `>>|` draws on a [`VirtualScreen`] of 24 rows by 80 columns instead of taking
//! over a terminal, `<<|` and `<<|?` read from a [`KeyScript`], and when the
//! block ends the last frame is written to the program's ordinary output as
//! plain text — so a golden can hold it and a subscript can capture it.
//!
//! Without `--keys` nothing here is used, and `>>|` with no terminal still
//! fails, as GLB-018 C decided: the virtual screen is asked for, never guessed.
//!
//! Both Rust engines hold one of these; it lives here so they cannot disagree
//! about what a frame looks like.

use std::collections::VecDeque;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// The size `>>?` answers under `--keys` until a `RESIZE` step changes it: the
/// conventional terminal, the same fallback the engines already give when
/// there is no terminal to ask.
pub const ROWS: usize = 24;
pub const COLS: usize = 80;

/// One step of a key script.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    /// The character and the modifier number `<<| k, m` receives.
    Key(char, i64),
    /// This many `<<|?` polls find no key — so a running loop advances without
    /// the script depending on a clock.
    Wait(u32),
    /// Write the screen as it stands to the output, then go on: the frame of a
    /// picker or a menu that the last frame, after it closes, no longer shows.
    Show,
    /// The window becomes this many rows and columns, and `<<|` receives `⤢`
    /// (D-17).
    Resize(u16, u16),
}

/// A key a script delivers: its character and its modifier number.
pub type Key = (char, i64);

/// The keys a program under `--keys` receives, in order.
///
/// The file has one step per line; blank lines and `#` comments are ignored:
///
/// ```text
/// # advance one generation, then quit
/// n
/// WAIT 10
/// q
/// ```
///
/// A line is a key, a key with modifiers, `WAIT <n>`, `SHOW` — which writes
/// the current frame to the output when the program next asks for a key, so a
/// test can hold a screen the last frame no longer shows — or `RESIZE <rows>
/// <cols>`, the window changing size: the screen takes the new size, `>>?`
/// answers it, and `<<|` receives `⤢` (D-17).
///
/// A key is a single character, or one of `SPACE`, `ENTER`, `ESC`, `TAB`,
/// `BACKTAB`, `BACKSPACE`, `UP`, `DOWN`, `LEFT`, `RIGHT`, `HOME`, `END`, `PGUP`,
/// `PGDN`, `DEL`, `F1`…`F12`. Any of `SHIFT+`, `ALT+`, `CTRL+` may stand before
/// it, in any order: `CTRL+S`, `SHIFT+RIGHT`, `CTRL+ALT+DEL`. The names give
/// what `<<|` yields from a real keyboard, through the same rule
/// ([`crate::keys::key`]): `↑` for UP, `⇱` for HOME, `①` for F1, `'\n'` for
/// ENTER, `0d27` for ESC, `0d127` for BACKSPACE, `0d1`… for CTRL+letter.
#[derive(Debug, Clone, Default)]
pub struct KeyScript {
    steps: VecDeque<Step>,
    /// The size a `RESIZE` just delivered as `⤢`, for the screen to take.
    resized: Option<(u16, u16)>,
}

impl KeyScript {
    pub fn parse(text: &str) -> Result<Self, String> {
        use crate::keys::{self, Mods};
        let mut steps = VecDeque::new();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line == "SHOW" {
                steps.push_back(Step::Show);
                continue;
            }
            if let Some(size) = line.strip_prefix("RESIZE ") {
                let parts: Vec<Option<u16>> = size
                    .split_whitespace()
                    .map(|p| p.parse::<u16>().ok().filter(|n| *n >= 1))
                    .collect();
                match parts.as_slice() {
                    [Some(r), Some(c)] => steps.push_back(Step::Resize(*r, *c)),
                    _ => {
                        return Err(format!(
                            "line {}: RESIZE takes the new rows and columns, two whole numbers from 1, got '{}'",
                            n + 1, line
                        ))
                    }
                }
                continue;
            }
            if let Some(count) = line.strip_prefix("WAIT ") {
                let n_polls = count.trim().parse::<u32>().map_err(|_| {
                    format!("line {}: WAIT takes a whole number of polls, got '{}'", n + 1, line)
                })?;
                steps.push_back(Step::Wait(n_polls));
                continue;
            }
            // The prefixes, then the key. A lone `+` is the plus key, and so
            // is the `+` after one: `SHIFT++`.
            let mut mods = Mods::default();
            let mut rest = line;
            loop {
                if let Some(r) = rest.strip_prefix("SHIFT+").filter(|r| !r.is_empty()) {
                    mods.shift = true;
                    rest = r;
                } else if let Some(r) = rest.strip_prefix("ALT+").filter(|r| !r.is_empty()) {
                    mods.alt = true;
                    rest = r;
                } else if let Some(r) = rest.strip_prefix("CTRL+").filter(|r| !r.is_empty()) {
                    mods.ctrl = true;
                    rest = r;
                } else {
                    break;
                }
            }
            let code = keys::named(rest).ok_or_else(|| {
                format!(
                    "line {}: '{}' is not one key — write one character, or SPACE, ENTER, ESC, TAB, BACKTAB, BACKSPACE, UP, DOWN, LEFT, RIGHT, HOME, END, PGUP, PGDN, DEL or F1…F12, with SHIFT+, ALT+ or CTRL+ before it if held, or WAIT n, SHOW or RESIZE rows cols",
                    n + 1, line
                )
            })?;
            let (ch, m) = keys::key(code, mods);
            steps.push_back(Step::Key(ch, m));
        }
        Ok(KeyScript { steps, resized: None })
    }

    /// `<<|`: the next key, past any waits — a program that blocks is waiting
    /// for exactly this. `None` when the script has run out.
    pub fn next_blocking(&mut self) -> Option<Key> {
        self.next_blocking_with(&mut |_| {})
    }

    fn next_blocking_with(&mut self, show: &mut dyn FnMut(())) -> Option<Key> {
        while let Some(step) = self.steps.pop_front() {
            match step {
                Step::Key(c, m) => return Some((c, m)),
                Step::Resize(r, c) => {
                    self.resized = Some((r, c));
                    return Some((crate::keys::RESIZE, 0));
                }
                Step::Show => show(()),
                Step::Wait(_) => {}
            }
        }
        None
    }

    /// `<<|?`: the next key if no wait stands before it, else `'\0'` (with no
    /// modifiers) and one poll off the wait. `'\0'` too when the script has
    /// run out.
    pub fn next_poll(&mut self) -> Key {
        self.next_poll_with(&mut |_| {})
    }

    fn next_poll_with(&mut self, show: &mut dyn FnMut(())) -> Key {
        loop {
            match self.steps.front_mut() {
                None => return ('\0', 0),
                Some(Step::Show) => {
                    self.steps.pop_front();
                    show(());
                }
                Some(Step::Wait(0)) => {
                    self.steps.pop_front();
                }
                Some(Step::Wait(n)) => {
                    *n -= 1;
                    return ('\0', 0);
                }
                Some(&mut Step::Key(c, m)) => {
                    self.steps.pop_front();
                    return (c, m);
                }
                Some(&mut Step::Resize(r, c)) => {
                    self.steps.pop_front();
                    self.resized = Some((r, c));
                    return (crate::keys::RESIZE, 0);
                }
            }
        }
    }
}

/// A grid of cells the TUI primitives draw on when there is no terminal.
///
/// A cell holds one grapheme; a grapheme two columns wide takes its cell and
/// leaves the next one empty, as a terminal does. Styles and colours are not
/// kept: a frame is read for what it says.
#[derive(Debug, Clone)]
pub struct VirtualScreen {
    cells: Vec<Vec<String>>,
    rows: usize,
    cols: usize,
    row: usize,
    col: usize,
    /// Where `>>~ (row, col)` showed the cursor (D-16), `None` while hidden.
    caret: Option<(u16, u16)>,
}

impl Default for VirtualScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualScreen {
    pub fn new() -> Self {
        Self::sized(ROWS, COLS)
    }

    fn sized(rows: usize, cols: usize) -> Self {
        VirtualScreen { cells: vec![vec![" ".to_string(); cols]; rows], rows, cols, row: 0, col: 0, caret: None }
    }

    /// What `>>?` answers: (rows, columns).
    pub fn size(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    /// `>>|` begins: an empty screen, and the cursor hidden. The size is the
    /// window's, so it stays.
    pub fn open(&mut self) {
        *self = Self::sized(self.rows, self.cols);
    }

    /// `>>!`: the screen is emptied. The cursor stays shown, where it was.
    pub fn clear(&mut self) {
        let caret = self.caret;
        *self = Self::sized(self.rows, self.cols);
        self.caret = caret;
    }

    /// The window changed size (D-17): what was drawn keeps its place, and
    /// what no longer fits is cut, as on a terminal's alternate screen.
    pub fn resize(&mut self, rows: usize, cols: usize) {
        let (rows, cols) = (rows.max(1), cols.max(1));
        self.cells.resize(rows, vec![" ".to_string(); cols]);
        for r in &mut self.cells {
            r.resize(cols, " ".to_string());
        }
        self.rows = rows;
        self.cols = cols;
        self.row = self.row.min(rows - 1);
        self.col = self.col.min(cols);
    }

    /// `>>~ (row, col)` with nothing to print (D-16): shown there, or hidden.
    pub fn set_caret(&mut self, caret: Option<(u16, u16)>) {
        self.caret = caret;
    }

    /// `>>~ (fila, col) >`, 1-based, clamped to the screen.
    pub fn move_to(&mut self, row: i64, col: i64) {
        self.row = (row.max(1) as usize - 1).min(self.rows - 1);
        self.col = (col.max(1) as usize - 1).min(self.cols);
    }

    /// Text at the cursor. A newline goes to the start of the next row; what
    /// falls off the right edge or the bottom is dropped, as a terminal in
    /// raw mode with the cursor parked would drop it.
    pub fn write(&mut self, text: &str) {
        for g in text.graphemes(true) {
            match g {
                "\n" | "\r\n" => {
                    self.row = (self.row + 1).min(self.rows - 1);
                    self.col = 0;
                }
                "\r" => self.col = 0,
                _ => {
                    let w = UnicodeWidthStr::width(g);
                    if w == 0 {
                        continue;
                    }
                    if self.col + w > self.cols {
                        continue;
                    }
                    self.cells[self.row][self.col] = g.to_string();
                    if w == 2 {
                        self.cells[self.row][self.col + 1] = String::new();
                    }
                    self.col += w;
                }
            }
        }
    }

    /// The screen as text: each row without its trailing blanks, and no blank
    /// rows at the bottom.
    pub fn frame(&self) -> String {
        let mut rows: Vec<String> = self
            .cells
            .iter()
            .map(|r| r.concat().trim_end().to_string())
            .collect();
        while rows.last().is_some_and(|r| r.is_empty()) {
            rows.pop();
        }
        // A cursor shown is part of what the user sees: one line after the
        // frame says where (D-16). A frame with the cursor hidden — every
        // frame written before v0.0.12 — has no such line.
        if let Some((r, c)) = self.caret {
            rows.push(format!("[cursor {},{}]", r, c));
        }
        rows.join("\n")
    }
}

/// What an engine holds under `--keys`.
#[derive(Debug, Clone, Default)]
pub struct Headless {
    pub keys: KeyScript,
    pub screen: VirtualScreen,
}

impl Headless {
    pub fn new(keys: KeyScript) -> Self {
        Headless { keys, screen: VirtualScreen::new() }
    }

    /// `<<|`: the next key, and the frames any `SHOW` before it asked for.
    pub fn read_blocking(&mut self) -> (Option<Key>, Vec<String>) {
        let mut shown = Vec::new();
        let screen = &self.screen;
        let key = self.keys.next_blocking_with(&mut |_| shown.push(screen.frame()));
        self.take_resize();
        (key, shown)
    }

    /// `<<|?`: the next key or `'\0'`, and the frames any `SHOW` asked for.
    pub fn read_poll(&mut self) -> (Key, Vec<String>) {
        let mut shown = Vec::new();
        let screen = &self.screen;
        let key = self.keys.next_poll_with(&mut |_| shown.push(screen.frame()));
        self.take_resize();
        (key, shown)
    }

    /// A `RESIZE` the key just read came from: the screen takes its size.
    fn take_resize(&mut self) {
        if let Some((r, c)) = self.keys.resized.take() {
            self.screen.resize(r as usize, c as usize);
        }
    }

    /// What `>>?` answers under `--keys`: (columns, rows), the VM's order.
    pub fn size_cols_rows(&self) -> (u16, u16) {
        let (r, c) = self.screen.size();
        (c as u16, r as u16)
    }
}

/// The message a blocking `<<|` raises when the script has no key left: the
/// program is waiting for a key nobody wrote, which in a test is a failure to
/// report, not a hang to wait out.
pub const OUT_OF_KEYS: &str =
    "--keys: the key script ran out while the program waited for a key";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_names_and_waits() {
        let mut k = KeyScript::parse("# c\nn\nWAIT 2\nUP\nSPACE\nCTRL+S\n").unwrap();
        assert_eq!(k.next_poll(), ('n', 0));
        assert_eq!(k.next_poll(), ('\0', 0));
        assert_eq!(k.next_poll(), ('\0', 0));
        assert_eq!(k.next_poll(), ('\u{2191}', 0));
        assert_eq!(k.next_blocking(), Some((' ', 0)));
        assert_eq!(k.next_blocking(), Some(('\x13', 4)));
        assert_eq!(k.next_blocking(), None);
        assert_eq!(k.next_poll(), ('\0', 0));
    }

    #[test]
    fn script_modifiers_and_d13_names() {
        let mut k = KeyScript::parse(
            "SHIFT+RIGHT\nALT+x\nCTRL+ALT+DEL\nHOME\nF5\nSHIFT+F5\nBACKTAB\nSHIFT+TAB\nSHIFT+A\n+\nSHIFT++\n",
        )
        .unwrap();
        let got: Vec<Key> = std::iter::from_fn(|| k.next_blocking()).collect();
        assert_eq!(
            got,
            vec![
                ('→', 1), ('x', 2), ('⌦', 6), ('⇱', 0), ('⑤', 0), ('⑤', 1),
                ('⇤', 1), ('⇤', 1), ('A', 0), ('+', 0), ('+', 0),
            ]
        );
        assert!(KeyScript::parse("SHIFT+\n").is_err());
        assert!(KeyScript::parse("META+x\n").is_err());
    }

    #[test]
    fn show_hands_back_the_frame_of_that_moment() {
        let mut h = Headless::new(KeyScript::parse("SHOW\nq\n").unwrap());
        h.screen.write("menú");
        let (k, frames) = h.read_blocking();
        assert_eq!(k, Some(('q', 0)));
        assert_eq!(frames, vec!["menú".to_string()]);
    }

    #[test]
    fn blocking_skips_waits() {
        let mut k = KeyScript::parse("WAIT 5\nq\n").unwrap();
        assert_eq!(k.next_blocking(), Some(('q', 0)));
    }

    #[test]
    fn a_line_that_is_not_a_key_is_refused() {
        assert!(KeyScript::parse("hola\n").is_err());
        assert!(KeyScript::parse("WAIT x\n").is_err());
    }

    #[test]
    fn frame_places_and_trims() {
        let mut s = VirtualScreen::new();
        s.move_to(2, 3);
        s.write("ab");
        s.move_to(1, 1);
        s.write("漢x");
        assert_eq!(s.frame(), "漢x\n  ab");
    }

    #[test]
    fn resize_is_a_key_and_a_new_size() {
        let mut h = Headless::new(KeyScript::parse("a\nRESIZE 10 20\nb").unwrap());
        h.screen.write("0123456789012345678901234");
        assert_eq!(h.read_blocking().0, Some(('a', 0)));
        assert_eq!(h.size_cols_rows(), (80, 24));
        assert_eq!(h.read_poll().0, ('⤢', 0));
        assert_eq!(h.size_cols_rows(), (20, 10));
        assert_eq!(h.screen.frame(), "01234567890123456789");
        h.screen.open();
        assert_eq!(h.screen.size(), (10, 20));
        for bad in ["RESIZE 0 5", "RESIZE 5", "RESIZE a b", "RESIZE 5 5 5", "RESIZE 70000 5"] {
            assert!(KeyScript::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn caret_is_a_line_after_the_frame_and_survives_clear() {
        let mut s = VirtualScreen::new();
        s.write("hello");
        s.set_caret(Some((1, 6)));
        assert_eq!(s.frame(), "hello\n[cursor 1,6]");
        s.clear();
        assert_eq!(s.frame(), "[cursor 1,6]");
        s.set_caret(None);
        assert_eq!(s.frame(), "");
        s.set_caret(Some((2, 2)));
        s.open();
        assert_eq!(s.frame(), "");
    }
}
