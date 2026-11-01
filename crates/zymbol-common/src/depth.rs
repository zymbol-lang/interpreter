//! How deep a program may call (D7, decided by the author 2026-10-16; GLB-133).
//!
//! At most `MAX_ACTIVE_CALLS` calls — of a function or of a lambda, direct or
//! through `$>`, `$|`, `$<` — may be active at once, the same number in the
//! three engines, so a program at a given depth behaves the same in all of
//! them. The next call is a `##Depth` error that `!?` catches like any other. A
//! call in tail position replaces the one that made it and does not count.
//!
//! It used to be no number at all: each engine recursed until the Rust stack
//! ran out, and the process aborted with "stack overflow" and no Zymbol error —
//! the tree-walker near 9 000 levels, the VM near 2 500 through `$>`.
//!
//! The counter is what decides. The stack is the backstop: a program runs on a
//! thread of `PROGRAM_STACK` bytes, sized so the counter is reached long before
//! the stack is, and `stack_nearly_spent` raises the same error should some
//! other path recurse in Rust without passing a call.

use std::cell::Cell;

/// Calls that may be active at once.
pub const MAX_ACTIVE_CALLS: usize = 10_000;

/// The stack a program's thread is given: reserved, not committed — a program
/// that does not recurse does not pay it. 512 MiB, because a call through `$>`
/// costs the VM some 23 KB of Rust stack, and 10 000 of them must fit twice.
pub const PROGRAM_STACK: usize = 512 * 1024 * 1024;

/// What is left of the stack when the backstop fires: room to unwind, format
/// the error and print it.
const STACK_MARGIN: usize = 8 * 1024 * 1024;

/// The message of the `##Depth` error, built once for every engine.
pub fn exceeded_message() -> String {
    format!("call depth exceeded: more than {} calls active at once", MAX_ACTIVE_CALLS)
}

thread_local! {
    /// Where the program thread's stack starts, and how large it is; 0 when
    /// the thread did not say (the REPL, a test), and then only the counter
    /// guards.
    static STACK: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}

/// Called first thing on a program's thread, with the size it was given.
#[inline(never)]
pub fn mark_stack_base(size: usize) {
    let here = 0u8;
    STACK.with(|s| s.set((std::ptr::addr_of!(here) as usize, size)));
}

/// True when the thread has used all but `STACK_MARGIN` of its stack.
#[inline]
pub fn stack_nearly_spent() -> bool {
    let (base, size) = STACK.with(|s| s.get());
    if base == 0 {
        return false;
    }
    let here = 0u8;
    let used = base.saturating_sub(std::ptr::addr_of!(here) as usize);
    used + STACK_MARGIN > size
}
