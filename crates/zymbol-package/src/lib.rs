//! Zymbol Package (`.zyp`): manifest parsing, transitive-dependency closure, and portable
//! ZIP archive read/write for Zymbol source projects.
//!
//! A `.zyp` is a ZIP file with a `zyp.toml` manifest at its root declaring one or more
//! `[[script]]` entry points, plus their source dependencies under `src/`. It is **not** a
//! binary or bytecode — running one still lexes/parses/executes plain `.zy` source, just
//! extracted from an archive instead of read loose off disk. Compiling to a standalone
//! native executable is a separate, unrelated feature (`zymbol build`, the
//! `zymbol-standalone` crate) and this crate has no dependency on it in either direction.
//!
//! This crate deliberately depends only on `zymbol-ast`/`zymbol-lexer`/`zymbol-parser` — it
//! never compiles or executes Zymbol code, so it stays usable from a future package manager
//! or the LSP without dragging in `zymbol-interpreter`/`zymbol-vm`/`zymbol-compiler` and
//! their transitive dependencies (`clap`, `tokio`, `odbc-api`, ...).
//!
//! Packaging is deliberately permissive: [`compute_closure`] never fails on something it
//! can't fully resolve (an absolute import, a dynamic `<\ \>` shell exec, a parse error in
//! one file) — it emits a [`PackageWarning`] and keeps going, because a warning-only policy
//! means `zymbol package` always produces *something* the author can inspect with
//! `--dry-run` rather than an opaque hard failure partway through a large project.
//!
//! Known gap, intentionally left as a `TODO`: only `.zy` files reachable via imports or
//! `</ />` are packaged. A script that reads a data file (CSV, JSON config, ...) via
//! `std/io` has no way today to declare that file as a dependency — the manifest schema
//! should eventually grow an `include = ["data/**/*.json"]` glob list for exactly that case.

mod closure;
mod error;
mod manifest;
mod path_safety;
mod reader;
mod writer;

pub use closure::{compute_closure, ClosureResult, PackageWarning, PackagedFile, WarningKind};
pub use error::PackageError;
pub use manifest::{EngineMode, Manifest, PackageMeta, ScriptEntry};
pub use reader::{open_zyp, Package};
pub use writer::write_zyp;
