//! Whether a `>>|` full screen is open — for whoever has to put the terminal
//! back when the program is stopped from outside (`zymbol run --timeout`, G3).
//!
//! An engine restores the terminal itself when its `>>|` block ends, by any
//! path. A program stopped by the clock ends no block: the process exits from
//! another thread, and without this the shell is left in raw mode on the
//! alternate screen. Restoring when nothing is open would write escape codes
//! into the program's output, so the stopper asks first.

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
