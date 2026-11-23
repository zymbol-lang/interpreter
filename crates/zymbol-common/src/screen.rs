//! Whether a `>>|` full screen is open — for whoever has to put the terminal
//! back when the program is stopped from outside (`zymbol run --timeout`, G3).
//!
//! An engine restores the terminal itself when its `>>|` block ends, by any
//! path. A program stopped by the clock ends no block: the process exits from
//! another thread, and without this the shell is left in raw mode on the
//! alternate screen. Restoring when nothing is open would write escape codes
//! into the program's output, so the stopper asks first.
//!
//! And the frame a `>>|` block draws into, and how it reaches the terminal
//! (GAP-ZYE-015): see [`show`].

use std::sync::atomic::{AtomicUsize, Ordering};

static OPEN: AtomicUsize = AtomicUsize::new(0);

/// A `>>|` block entered the alternate screen.
pub fn opened() {
    OPEN.fetch_add(1, Ordering::SeqCst);
}

/// A `>>|` block left it.
pub fn closed() {
    let mut n = OPEN.load(Ordering::SeqCst);
    while n > 0 {
        match OPEN.compare_exchange(n, n - 1, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => break,
            Err(now) => n = now,
        }
    }
}

/// Is a `>>|` block open now?
pub fn is_open() -> bool {
    OPEN.load(Ordering::SeqCst) > 0
}

// ── The frame (GAP-ZYE-015, D-09) ────────────────────────────────────────────
//
// Inside `>>|`, what a program draws — `>>`, `¶`, `>>!`, `>>~` — is kept in a
// frame and reaches the terminal when the program waits: `<<|`, `<<|?`, `@~`
// (`@~ 0` included, which is how a program says "show it now") and `<<`, and
// when the block ends, by any path. One frame is one write, wrapped in the
// synchronized update that modern terminals honour (`ESC [ ? 2026 h` … `l`:
// kitty, WezTerm, foot, iTerm2, Windows Terminal) and the rest ignore, so the
// terminal never paints a screen half drawn.
//
// Both Rust engines keep their frame in a `Vec<u8>` of their own and hand it
// here; this is the one place that decides how it is written.

/// Begin a synchronized update (DEC private mode 2026).
pub const SYNC_BEGIN: &[u8] = b"\x1b[?2026h";
/// End it: the terminal paints what arrived in between, at once.
pub const SYNC_END: &[u8] = b"\x1b[?2026l";

/// A frame that grows past this is shown without waiting for the program to
/// wait. It is a guard on memory, not a rule a program can notice: a loop that
/// draws forever and never waits would otherwise grow its frame without end.
pub const FRAME_LIMIT: usize = 1 << 20;

/// Add `bytes` to `frame`, showing it first when it would pass [`FRAME_LIMIT`].
pub fn put(frame: &mut Vec<u8>, bytes: &[u8], caret: Option<(u16, u16)>) {
    if frame.len() + bytes.len() > FRAME_LIMIT {
        show(frame, caret);
    }
    frame.extend_from_slice(bytes);
}

/// Write the frame to the terminal in one write, and empty it. An empty frame
/// writes nothing — not even the synchronized-update brackets.
///
/// `caret` is where the program asked for the terminal's cursor (D-16, see
/// [`caret`]). Drawing moves the cursor, so a frame that ends with it shown
/// puts it back there last; and it is hidden while the frame is painted, for
/// the terminals that do not honour the synchronized update.
pub fn show(frame: &mut Vec<u8>, caret: Option<(u16, u16)>) {
    if frame.is_empty() {
        return;
    }
    let mut bytes = Vec::with_capacity(frame.len() + SYNC_BEGIN.len() + SYNC_END.len() + 24);
    bytes.extend_from_slice(SYNC_BEGIN);
    let back = caret.map(|_| caret_bytes(caret));
    if back.is_some() {
        bytes.extend_from_slice(CURSOR_HIDE);
    }
    // A frame that ends by placing the cursor — the `>>~ (row, col)` itself —
    // needs it placed only once.
    let ends_placed = back.as_ref().is_some_and(|b| frame.ends_with(b));
    bytes.append(frame);
    if let (Some(b), false) = (back, ends_placed) {
        bytes.extend_from_slice(&b);
    }
    bytes.extend_from_slice(SYNC_END);
    write_terminal(&bytes);
}

// ── The cursor (DIS-ZYE-004, D-16) ───────────────────────────────────────────
//
// `>>|` hides the terminal's cursor: a game wants none. `>>~ (row, col)` with
// nothing to print shows it there, and `>>~ (0, 0)` hides it again. It stays
// where it was put until the program moves or hides it — every frame shown in
// between ends with the cursor back at that place, whatever the frame drew
// elsewhere. It is the terminal's own cursor, so it blinks, and has the shape,
// the user chose, and a screen reader follows it.

/// Hide the terminal's cursor (DEC private mode 25).
pub const CURSOR_HIDE: &[u8] = b"\x1b[?25l";
/// Show it.
pub const CURSOR_SHOW: &[u8] = b"\x1b[?25h";

/// The message of a `>>~ (row, col)` whose place is not one: rows and columns
/// count from 1, and only `(0, 0)` means "hidden". It says "out of range", so
/// the error is a `##Range` in every engine.
pub const CARET_RANGE: &str =
    "cursor position out of range: >>~ (row, col) counts both from 1, and (0, 0) hides the cursor";

/// The message of a `>>~ pos` with nothing to print whose `pos` is not a
/// place: a positional tuple of two Ints. The engine adds ", got <Type>".
pub const CARET_SHAPE: &str = ">>~ pos with nothing to print takes the cursor's place, a tuple (row, col)";

/// `>>~ (row, col)` outside `>>|`: the cursor there is the shell's.
pub const CARET_OUTSIDE: &str =
    ">>~ (row, col) shows the cursor of a full screen, and is written inside >>| { }";

/// The place `>>~ (row, col)` asks for: `None` for `(0, 0)`, which hides the
/// cursor, and an error for any other place outside the screen's numbering.
pub fn caret(row: i64, col: i64) -> Result<Option<(u16, u16)>, &'static str> {
    if row == 0 && col == 0 {
        return Ok(None);
    }
    match (u16::try_from(row), u16::try_from(col)) {
        (Ok(r), Ok(c)) if r >= 1 && c >= 1 => Ok(Some((r, c))),
        _ => Err(CARET_RANGE),
    }
}

/// What the terminal is sent for a cursor: moved there and shown, or hidden.
pub fn caret_bytes(caret: Option<(u16, u16)>) -> Vec<u8> {
    match caret {
        None => CURSOR_HIDE.to_vec(),
        Some((r, c)) => {
            let mut b = format!("\x1b[{};{}H", r, c).into_bytes();
            b.extend_from_slice(CURSOR_SHOW);
            b
        }
    }
}

/// One write, past `std::io::stdout`'s line buffering: a `LineWriter` writes a
/// buffer up to its last newline and keeps the rest for a second write, so a
/// frame with a `¶` in it would reach the terminal in two. Whatever is still in
/// that buffer goes first, so nothing arrives out of order.
fn write_terminal(bytes: &[u8]) {
    use std::io::Write;
    std::io::stdout().flush().ok();
    #[cfg(unix)]
    {
        use std::os::unix::io::FromRawFd;
        // fd 1 is the process's; ManuallyDrop keeps this File from closing it.
        let mut out = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(1) });
        out.write_all(bytes).ok();
    }
    #[cfg(not(unix))]
    {
        let mut out = std::io::stdout();
        out.write_all(bytes).ok();
        out.flush().ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_counts_from_one_and_zero_zero_hides() {
        assert_eq!(caret(1, 6), Ok(Some((1, 6))));
        assert_eq!(caret(0, 0), Ok(None));
        for (r, c) in [(0, 5), (5, 0), (-1, 1), (1, -1), (70000, 1)] {
            assert_eq!(caret(r, c), Err(CARET_RANGE), "({r}, {c})");
        }
        assert_eq!(crate::errkind::error_kind_of_message(CARET_RANGE), "Range");
    }

    #[test]
    fn caret_bytes_move_then_show_or_hide() {
        assert_eq!(caret_bytes(Some((3, 10))), b"\x1b[3;10H\x1b[?25h".to_vec());
        assert_eq!(caret_bytes(None), CURSOR_HIDE.to_vec());
    }
}
