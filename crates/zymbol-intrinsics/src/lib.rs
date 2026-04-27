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

pub mod split;
pub mod search;
pub mod transform;
