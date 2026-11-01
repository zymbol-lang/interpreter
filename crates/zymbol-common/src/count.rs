//! `zymbol run --count` — what a program cost, in units that do not depend on
//! the machine or on what else it was doing.
//!
//! Time cannot gate anything at this size: two runs of the benchmark suite on
//! one day moved different rows by ±40 % on programs that last 20 ms. What is
//! counted here is exact — the same program gives the same five numbers on
//! every run — so a ratio between two sizes of one program is a fact and not a
//! reading.
//!
//! Two kinds of number, because each is blind to what the other sees:
//!
//! * **steps** and **calls** are the engine's own unit of work: a bytecode
//!   instruction dispatched in the register VM, a statement executed or an
//!   expression evaluated in the tree-walker. They are NOT comparable between
//!   engines, and they do not see a copy: appending to a collection that has
//!   two owners is one step whether it holds ten elements or ten thousand.
//! * **allocs**, **bytes** and **copied** are the work underneath. `copied`
//!   is the number of elements duplicated because a collection that was about
//!   to be written had another owner — the one shape behind every quadratic
//!   cost found in v0.0.10 (ZYVM-003, ZYTW-010, GLB-117), none of which a step
//!   counter would have noticed.
//!
//! Always compiled, and cheap by construction: each counter is a thread-local
//! `Cell<u64>` with a constant initializer and no destructor, so an increment
//! is a load, an add and a store against a fixed offset — no lock, no atomic,
//! no lazy initialization, and safe to touch from inside the allocator.

use std::cell::Cell;
use std::rc::Rc;

thread_local! {
    static STEPS:  Cell<u64> = const { Cell::new(0) };
    static CALLS:  Cell<u64> = const { Cell::new(0) };
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
    static BYTES:  Cell<u64> = const { Cell::new(0) };
    static COPIED: Cell<u64> = const { Cell::new(0) };
}

/// One unit of the engine's work.
#[inline(always)]
pub fn step() { STEPS.with(|c| c.set(c.get().wrapping_add(1))) }

/// One call of a function or a lambda.
#[inline(always)]
pub fn call() { CALLS.with(|c| c.set(c.get().wrapping_add(1))) }

/// One request to the allocator, of `bytes` bytes.
#[inline(always)]
pub fn alloc(bytes: usize) {
    // `try_with`: the allocator also runs while a thread is being torn down.
    let _ = ALLOCS.try_with(|c| c.set(c.get().wrapping_add(1)));
    let _ = BYTES.try_with(|c| c.set(c.get().wrapping_add(bytes as u64)));
}

/// `n` elements duplicated because the collection being written was shared.
#[inline(always)]
pub fn copied(n: usize) { COPIED.with(|c| c.set(c.get().wrapping_add(n as u64))) }

/// `Rc::make_mut`, counting what it copies.
///
/// `make_mut` clones the contents when the `Rc` has another owner, and that
/// clone is the cost this module exists to see. Every site of the engines that
/// writes a collection goes through here instead of calling it directly.
#[inline]
pub fn make_mut<T: Clone>(rc: &mut Rc<Vec<T>>) -> &mut Vec<T> {
    if Rc::strong_count(rc) != 1 || Rc::weak_count(rc) != 0 {
        copied(rc.len());
    }
    Rc::make_mut(rc)
}

/// `make_mut` for a dictionary, counted the same way: a write to one with
/// another owner copies every pair.
#[inline]
pub fn make_mut_dict<V: Clone>(rc: &mut Rc<crate::Dict<V>>) -> &mut crate::Dict<V> {
    if Rc::strong_count(rc) != 1 || Rc::weak_count(rc) != 0 {
        copied(rc.len());
    }
    Rc::make_mut(rc)
}

/// The five counters at one moment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub steps:  u64,
    pub calls:  u64,
    pub allocs: u64,
    pub bytes:  u64,
    pub copied: u64,
}

impl Counts {
    /// What happened between `earlier` and `self`.
    pub fn since(&self, earlier: &Counts) -> Counts {
        Counts {
            steps:  self.steps.wrapping_sub(earlier.steps),
            calls:  self.calls.wrapping_sub(earlier.calls),
            allocs: self.allocs.wrapping_sub(earlier.allocs),
            bytes:  self.bytes.wrapping_sub(earlier.bytes),
            copied: self.copied.wrapping_sub(earlier.copied),
        }
    }

    /// One line, `key=value` pairs, for a person and for a harness alike.
    ///
    /// Built from its parts on purpose: `zyquality/messages` harvests every
    /// string literal of twelve characters or more in the Rust sources as a
    /// diagnostic, and this is a report, not something an engine says about a
    /// program.
    pub fn line(&self) -> String {
        let pairs = [
            ("steps", self.steps), ("calls", self.calls), ("allocs", self.allocs),
            ("bytes", self.bytes), ("copied", self.copied),
        ];
        let mut out = String::from("count");
        for (name, value) in pairs {
            out.push_str("  ");
            out.push_str(name);
            out.push('=');
            out.push_str(&value.to_string());
        }
        out
    }
}

/// Read the counters of the calling thread.
pub fn now() -> Counts {
    Counts {
        steps:  STEPS.with(Cell::get),
        calls:  CALLS.with(Cell::get),
        allocs: ALLOCS.with(Cell::get),
        bytes:  BYTES.with(Cell::get),
        copied: COPIED.with(Cell::get),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sole_owner_is_written_in_place_and_copies_nothing() {
        let before = now();
        let mut a = Rc::new(vec![1, 2, 3]);
        make_mut(&mut a).push(4);
        assert_eq!(now().since(&before).copied, 0);
        assert_eq!(*a, vec![1, 2, 3, 4]);
    }

    #[test]
    fn a_shared_collection_is_copied_and_every_element_counts() {
        let before = now();
        let mut a = Rc::new(vec![1, 2, 3]);
        let b = Rc::clone(&a);
        make_mut(&mut a).push(4);
        assert_eq!(now().since(&before).copied, 3);
        assert_eq!(*b, vec![1, 2, 3]);
    }

    #[test]
    fn the_line_is_key_value_pairs() {
        let c = Counts { steps: 5, calls: 1, allocs: 2, bytes: 64, copied: 0 };
        assert_eq!(c.line(), "count  steps=5  calls=1  allocs=2  bytes=64  copied=0");
    }
}
