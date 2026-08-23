//! Zymbol string and collection intrinsics.
//!
//! Pure Rust functions that operate directly on `&str` and primitive types —
//! no `Value` boxing, no VM types, no heap allocations beyond the final result.
//!
//! # Architecture
//!
//! ```text
//! Zymbol VM  →  adapter (unbox ZyStr → &str)  →  intrinsic fn  →  primitive result
//!                                                                  adapter (box → Value)
//! ```
//!
//! This mirrors how CPython's string methods are implemented in `Objects/unicodeobject.c`:
//! the C functions receive raw `char*`/`Py_ssize_t`, the Python runtime handles boxing.
//! The intrinsics are independently optimizable (SIMD, Aho-Corasick, etc.) without
//! touching the VM dispatch layer.
//!
//! [`time`] joined on the same terms and for a second reason: the civil calendar
//! is an *answer*, not a table of names, and the two Rust engines cannot be kept
//! agreeing about leap years by inspection the way they are kept agreeing about
//! `std/term`. It carries the crate's only dependency, for reading the machine's
//! own time zone.

pub mod split;
pub mod search;
pub mod transform;
pub mod time;
