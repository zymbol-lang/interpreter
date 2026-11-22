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
pub fn put(frame: &mut Vec<u8>, bytes: &[u8]) {
    if frame.len() + bytes.len() > FRAME_LIMIT {
        show(frame);
    }
    frame.extend_from_slice(bytes);
}

/// Write the frame to the terminal in one write, and empty it. An empty frame
/// writes nothing — not even the synchronized-update brackets.
pub fn show(frame: &mut Vec<u8>) {
    if frame.is_empty() {
        return;
    }
    let mut bytes = Vec::with_capacity(frame.len() + SYNC_BEGIN.len() + SYNC_END.len());
    bytes.extend_from_slice(SYNC_BEGIN);
    bytes.append(frame);
    bytes.extend_from_slice(SYNC_END);
    write_terminal(&bytes);
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
