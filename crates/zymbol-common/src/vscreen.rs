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
    Key(char),
    /// This many `<<|?` polls find no key — so a running loop advances without
    /// the script depending on a clock.
    Wait(u32),
    /// Write the screen as it stands to the output, then go on: the frame of a
    /// picker or a menu that the last frame, after it closes, no longer shows.
    Show,
}

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
/// A line is a single character, or one of `SPACE`, `ENTER`, `ESC`, `TAB`,
/// `BACKSPACE`, `UP`, `DOWN`, `LEFT`, `RIGHT`, `CTRL+<letter>`, `WAIT <n>`, or
/// `SHOW` — which writes the current frame to the output when the program next
/// asks for a key, so a test can hold a screen the last frame no longer shows.
/// The names give the same characters `<<|` yields from a real keyboard: `↑` for
/// UP, `'\n'` for ENTER, `0d27` for ESC, `0d127` for BACKSPACE, `0d1`… for CTRL.
#[derive(Debug, Clone, Default)]
pub struct KeyScript {
    steps: VecDeque<Step>,
}

impl KeyScript {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut steps = VecDeque::new();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let step = match line {
                "SPACE" => Step::Key(' '),
                "ENTER" => Step::Key('\n'),
                "ESC" => Step::Key('\x1B'),
                "TAB" => Step::Key('\t'),
                "BACKSPACE" => Step::Key('\x7F'),
                "UP" => Step::Key('\u{2191}'),
                "DOWN" => Step::Key('\u{2193}'),
                "LEFT" => Step::Key('\u{2190}'),
                "RIGHT" => Step::Key('\u{2192}'),
                "SHOW" => Step::Show,
                _ if line.starts_with("WAIT ") => {
                    let n_polls = line[5..].trim().parse::<u32>().map_err(|_| {
                        format!("line {}: WAIT takes a whole number of polls, got '{}'", n + 1, line)
                    })?;
                    Step::Wait(n_polls)
                }
                _ if line.starts_with("CTRL+") => {
                    let rest: Vec<char> = line[5..].chars().collect();
                    match rest.as_slice() {
                        [c] if c.is_ascii_alphabetic() => {
                            Step::Key((c.to_ascii_lowercase() as u8 - b'a' + 1) as char)
                        }
                        _ => return Err(format!("line {}: CTRL+ takes one letter, got '{}'", n + 1, line)),
                    }
                }
                _ => {
                    let mut chars = line.chars();
                    match (chars.next(), chars.next()) {
                        (Some(c), None) => Step::Key(c),
                        _ => {
                            return Err(format!(
                                "line {}: '{}' is not one key — write one character, or SPACE, \
                                 ENTER, ESC, TAB, BACKSPACE, UP, DOWN, LEFT, RIGHT, CTRL+X, WAIT n or SHOW",
                                n + 1, line
                            ))
                        }
                    }
                }
            };
            steps.push_back(step);
        }
        Ok(KeyScript { steps })
    }

    /// `<<|`: the next key, past any waits — a program that blocks is waiting
    /// for exactly this. `None` when the script has run out.
    pub fn next_blocking(&mut self) -> Option<char> {
        self.next_blocking_with(&mut |_| {})
    }

    fn next_blocking_with(&mut self, show: &mut dyn FnMut(())) -> Option<char> {
        while let Some(step) = self.steps.pop_front() {
            match step {
                Step::Key(c) => return Some(c),
                Step::Show => show(()),
                Step::Wait(_) => {}
            }
        }
        None
    }

    /// `<<|?`: the next key if no wait stands before it, else `'\0'` and one
    /// poll off the wait. `'\0'` too when the script has run out.
    pub fn next_poll(&mut self) -> char {
        self.next_poll_with(&mut |_| {})
    }

    fn next_poll_with(&mut self, show: &mut dyn FnMut(())) -> char {
        loop {
            match self.steps.front_mut() {
                None => return '\0',
                Some(Step::Show) => {
                    self.steps.pop_front();
                    show(());
                }
                Some(Step::Wait(0)) => {
                    self.steps.pop_front();
                }
                Some(Step::Wait(n)) => {
                    *n -= 1;
                    return '\0';
                }
                Some(Step::Key(c)) => {
                    let c = *c;
                    self.steps.pop_front();
                    return c;
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
}

impl Default for VirtualScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualScreen {
    pub fn new() -> Self {
        VirtualScreen { cells: vec![vec![" ".to_string(); COLS]; ROWS], row: 0, col: 0 }
    }

    /// `>>!`
    pub fn clear(&mut self) {
        *self = VirtualScreen::new();
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
    pub fn read_blocking(&mut self) -> (Option<char>, Vec<String>) {
        let mut shown = Vec::new();
        let screen = &self.screen;
        let key = self.keys.next_blocking_with(&mut |_| shown.push(screen.frame()));
        (key, shown)
    }

    /// `<<|?`: the next key or `'\0'`, and the frames any `SHOW` asked for.
    pub fn read_poll(&mut self) -> (char, Vec<String>) {
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
        assert_eq!(k.next_poll(), 'n');
        assert_eq!(k.next_poll(), '\0');
        assert_eq!(k.next_poll(), '\0');
        assert_eq!(k.next_poll(), '\u{2191}');
        assert_eq!(k.next_blocking(), Some(' '));
        assert_eq!(k.next_blocking(), Some('\x13'));
        assert_eq!(k.next_blocking(), None);
        assert_eq!(k.next_poll(), '\0');
    }

    #[test]
    fn show_hands_back_the_frame_of_that_moment() {
        let mut h = Headless::new(KeyScript::parse("SHOW\nq\n").unwrap());
        h.screen.write("menú");
        let (k, frames) = h.read_blocking();
        assert_eq!(k, Some('q'));
        assert_eq!(frames, vec!["menú".to_string()]);
    }

    #[test]
    fn blocking_skips_waits() {
        let mut k = KeyScript::parse("WAIT 5\nq\n").unwrap();
        assert_eq!(k.next_blocking(), Some('q'));
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
}
