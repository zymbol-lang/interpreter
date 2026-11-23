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

/// The size `>>?` answers under `--keys`: the conventional terminal, the same
/// fallback the engines already give when there is no terminal to ask.
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
/// A line is a key, a key with modifiers, `WAIT <n>`, or `SHOW` — which writes
/// the current frame to the output when the program next asks for a key, so a
/// test can hold a screen the last frame no longer shows.
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
                    "line {}: '{}' is not one key — write one character, or SPACE, ENTER, ESC, TAB, BACKTAB, BACKSPACE, UP, DOWN, LEFT, RIGHT, HOME, END, PGUP, PGDN, DEL or F1…F12, with SHIFT+, ALT+ or CTRL+ before it if held, or WAIT n, or SHOW",
                    n + 1, line
                )
            })?;
            let (ch, m) = keys::key(code, mods);
            steps.push_back(Step::Key(ch, m));
        }
        Ok(KeyScript { steps })
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
        VirtualScreen { cells: vec![vec![" ".to_string(); COLS]; ROWS], row: 0, col: 0, caret: None }
    }

    /// `>>|` begins: an empty screen, and the cursor hidden.
    pub fn open(&mut self) {
        *self = VirtualScreen::new();
    }

    /// `>>!`: the screen is emptied. The cursor stays shown, where it was.
    pub fn clear(&mut self) {
        let caret = self.caret;
        *self = VirtualScreen::new();
        self.caret = caret;
    }

    /// `>>~ (row, col)` with nothing to print (D-16): shown there, or hidden.
    pub fn set_caret(&mut self, caret: Option<(u16, u16)>) {
        self.caret = caret;
    }

    /// `>>~ (fila, col) >`, 1-based, clamped to the screen.
    pub fn move_to(&mut self, row: i64, col: i64) {
        self.row = (row.max(1) as usize - 1).min(ROWS - 1);
        self.col = (col.max(1) as usize - 1).min(COLS);
    }

    /// Text at the cursor. A newline goes to the start of the next row; what
    /// falls off the right edge or the bottom is dropped, as a terminal in
    /// raw mode with the cursor parked would drop it.
    pub fn write(&mut self, text: &str) {
        for g in text.graphemes(true) {
            match g {
                "\n" | "\r\n" => {
                    self.row = (self.row + 1).min(ROWS - 1);
                    self.col = 0;
                }
                "\r" => self.col = 0,
                _ => {
                    let w = UnicodeWidthStr::width(g);
                    if w == 0 {
                        continue;
                    }
                    if self.col + w > COLS {
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
        (key, shown)
    }

    /// `<<|?`: the next key or `'\0'`, and the frames any `SHOW` asked for.
    pub fn read_poll(&mut self) -> (Key, Vec<String>) {
        let mut shown = Vec::new();
        let screen = &self.screen;
        let key = self.keys.next_poll_with(&mut |_| shown.push(screen.frame()));
        (key, shown)
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
