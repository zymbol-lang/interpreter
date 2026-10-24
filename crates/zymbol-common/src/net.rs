//! What every engine says about a `std/net` time limit (ZethyCLI G1, decided
//! by the author on 2026-10-24, option A): a request given `seconds` that has
//! not finished by then is a soft `##Network` error with these words, in the
//! tree-walker, the VM and zyjs alike.

/// The message of the `##Network` error a request that ran out of time gives.
pub fn timeout_message(seconds: f64) -> String {
    format!("timeout: no answer within {} s", seconds)
}

/// The hard error for a time limit that is not a positive number of seconds.
pub fn bad_limit_message(fname: &str) -> String {
    format!("{}: the time limit must be a positive number of seconds", fname)
}
