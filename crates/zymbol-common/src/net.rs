//! `std/net`'s time limit, the same in every engine (ZethyCLI G1, decided by
//! the author on 2026-10-24).
//!
//! A limit is a number of seconds, an Int or a Float, `0` or more; `0` is none.
//! A request takes the one it is given after its headers (`net::get(url, [], 5)`,
//! `net::head(url, 5)`), and otherwise the process's own, set with
//! `net::timeout(s)` — which returns the one it replaces, so that a module can
//! put it back. A request that has not finished by its limit is a soft
//! `##Network` error with [`timeout_message`]'s words, in the tree-walker, the
//! VM and zyjs alike.
//!
//! The process's limit is the process's, as `random::semilla` is: a module that
//! sets it sets it for whoever imports it too.

use std::cell::Cell;

/// A limit as the program wrote it: Int seconds or Float seconds. `timeout`
/// gives back the one it replaces, and an Int has to come back an Int.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Limit {
    Int(i64),
    Float(f64),
}

impl Limit {
    fn seconds(self) -> f64 {
        match self {
            Limit::Int(n) => n as f64,
            Limit::Float(f) => f,
        }
    }
    fn is_none(self) -> bool {
        self.seconds() == 0.0
    }
}

thread_local! {
    static DEFAULT: Cell<Option<Limit>> = const { Cell::new(None) };
}

/// `net::timeout(s)`: the process's limit from now on; `0` removes it. The one
/// it replaces comes back, `Int(0)` when there was none.
pub fn set_default(limit: Limit) -> Limit {
    let new = if limit.is_none() { None } else { Some(limit) };
    DEFAULT.with(|d| d.replace(new)).unwrap_or(Limit::Int(0))
}

/// The seconds a request waits: its own limit when it gave one (`0` is none,
/// whatever the process's), the process's otherwise, and `None` for no limit.
pub fn effective(own: Option<Limit>) -> Option<f64> {
    match own {
        Some(l) if l.is_none() => None,
        Some(l) => Some(l.seconds()),
        None => DEFAULT.with(|d| d.get()).map(Limit::seconds),
    }
}

/// The message of the `##Network` error a request that ran out of time gives.
pub fn timeout_message(seconds: f64) -> String {
    format!("timeout: no answer within {} s", seconds)
}

/// The hard error for a limit that is not a number of seconds, `0` or more.
pub fn bad_limit_message(fname: &str) -> String {
    format!("{}: the time limit must be a number of seconds, 0 or more", fname)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_call_wins_and_zero_is_none() {
        assert_eq!(set_default(Limit::Int(0)), Limit::Int(0));
        assert_eq!(effective(None), None);
        assert_eq!(set_default(Limit::Int(10)), Limit::Int(0));
        assert_eq!(effective(None), Some(10.0));
        assert_eq!(effective(Some(Limit::Float(0.5))), Some(0.5));
        assert_eq!(effective(Some(Limit::Int(0))), None);
        assert_eq!(set_default(Limit::Float(2.5)), Limit::Int(10));
        assert_eq!(set_default(Limit::Int(0)), Limit::Float(2.5));
        assert_eq!(effective(None), None);
    }
}
