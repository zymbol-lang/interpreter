//! Transitive-dependency closure: given a package's `[[script]]` entry points, find every
//! `.zy` file they need (module imports, plus `</ file.zy />` script-execution targets) and
//! nothing else.
//!
//! Two things make this decidable without a full compile: import paths are 100% static —
//! `ModulePath` only admits identifiers joined by `/` (no string interpolation, no
//! expressions) — and `Program.imports` is only ever populated at the start of a file or
//! module block, never inside a function body. So reading `program.imports` after a single
//! lex+parse pass is exact, not an approximation.
//!
//! `</ file.zy />` is different: it's a raw literal captured by the lexer as a single
//! `TokenKind::ExecuteCommand` token, and it can appear as an expression at any nesting
//! depth. Rather than writing an `Expr` visitor (which silently rots every time a new `Expr`
//! variant is added — there are 72 today), this module scans the token stream directly:
//! the lexer is the only producer of that token kind, so a linear scan is exact and immune
//! to AST changes.
//!
//! ## `</ />` resolves relative to the *running script*, not the file it's written in
//!
//! This is easy to get wrong, and worth spelling out because an earlier draft of this
//! module got it wrong: naively, you'd expect the tree-walker to resolve `</ />` relative to
//! the directory of whichever file the token is lexically written in. It doesn't. Verified
//! empirically (a module function called cross-module, containing `</ ./x.zy />`, resolves
//! `x.zy` relative to the *caller's* directory, not the module's own directory) and
//! confirmed by reading the code: `Interpreter.current_file` is set exactly twice — once at
//! CLI startup for the entry file, and once inside `eval_execute` when `</ />` itself spawns
//! a sub-interpreter for the target script. Calling a function from an imported module does
//! *not* change it; the function body runs in the caller's interpreter, using whatever
//! `current_file` that interpreter already had. And a `</ />` written directly at a module's
//! top level (which *would* run with `current_file` pointing at the module, via the
//! throwaway interpreter `load_module` uses to collect exports) can't occur in any program
//! that actually passes semantic analysis — module-level variable initializers must be
//! literals (`E013`), so a module can't have a bare `</ />` statement at its top level.
//!
//! The VM has the same shape of fix-point: the compiler's `self.base_dir` is set once for
//! the whole compile and never varies per-module, and `</ />` doesn't get inlined into the
//! VM at all — it compiles to a shelled-out `zymbol run <path>` (always tree-walker, a fresh
//! process with its own fresh `base_dir`). So both engines agree: `</ />` always resolves
//! relative to the directory of the nearest enclosing *script* (the entry, or a `</ />`
//! target reached earlier in the chain) — never relative to an imported module's own
//! directory. This module tracks that as `script_base`, threaded through the BFS queue,
//! updated only when crossing a `</ />` boundary and left unchanged across `<#` imports.
//!
//! The one genuine cross-engine risk that *does* survive this: the same module can be
//! reached from two different `[[script]]` entries with two different `script_base`
//! directories. Whichever entry's BFS visits it first wins (files are deduplicated by
//! canonical path), so a `</ />` inside that module is only ever resolved and packaged
//! relative to the *first* entry that reached it — see [`WarningKind::ExecuteEntryDependent`].
//!
//! Packaging is permissive (a deliberate policy, not an oversight): anything this module
//! can't resolve or verify becomes a [`PackageWarning`], never a hard failure. The one
//! exception — enforced by the CLI layer, not here — is a `[[script]]` that turns out to be
//! a module file; a package whose entry point can't run isn't "permissive", it's broken.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use zymbol_lexer::{Lexer, TokenKind};
use zymbol_parser::Parser;
use zymbol_span::FileId;

/// One file that will be written into the archive's `src/` tree.
#[derive(Debug, Clone)]
pub struct PackagedFile {
    /// Forward-slash path relative to [`ClosureResult::root`] — becomes the zip entry name
    /// under `src/`. Built lexically by joining the literal string components of import
    /// paths (via `std::path::absolute`, which never touches the filesystem), *never* from
    /// `canonicalize()` or `read_dir()`. That distinction matters: macOS returns directory
    /// listings in NFD, so a Hangul file name like `한국어.zy` would come back
    /// decomposed into individual jamo if we named entries from `read_dir()`. An import
    /// written in a source file is NFC (as typed), so naming from the import text keeps the
    /// archive consistent across platforms — an archive built on macOS still extracts to
    /// names that Linux imports can find.
    pub rel_path: String,
    /// The real, lexically-absolute filesystem path to read the file's bytes from.
    pub abs_path: PathBuf,
}

/// Result of [`compute_closure`]: the files to package, the common root they're relative
/// to, and everything that couldn't be resolved or verified (see module docs on why those
/// are warnings, not errors).
#[derive(Debug, Clone)]
pub struct ClosureResult {
    pub root: PathBuf,
    pub files: Vec<PackagedFile>,
    pub warnings: Vec<PackageWarning>,
}

#[derive(Debug, Clone)]
pub struct PackageWarning {
    /// Absolute path of the file that triggered this warning. Empty for warnings that are
    /// about the closure as a whole rather than one file (W007, W008).
    pub file: PathBuf,
    pub kind: WarningKind,
}

#[derive(Debug, Clone)]
pub enum WarningKind {
    /// W001 — `/abs` or `~/home` import: not reproducible on another machine ($HOME).
    AbsoluteImport { components: String },
    /// W002 — a module import (or the entry file itself) resolves to a path that doesn't
    /// exist on disk.
    ModuleNotFound { module: String },
    /// W003 — `<\ ... \>` present: its arguments are arbitrary expressions, so any `.zy`
    /// dependency it might invoke at runtime is not statically visible.
    BashExec,
    /// W004 — a module containing `</ />` was reached from more than one `[[script]]` entry,
    /// each with a different script directory to resolve it against (see the module docs).
    /// Only the first entry's resolution was traced and packaged; if the module is meant to
    /// be runnable from the other entry too, its `</ />` target may be missing.
    ExecuteEntryDependent { first_base: PathBuf, other_base: PathBuf },
    /// W005 — the resolved `</ />` target does not exist on disk.
    ExecuteMissing { path: String },
    /// W006 — the file has lexer or parser errors. It is still packaged (permissive
    /// policy), but its own imports/`</ />` targets could not be traced, so its part of the
    /// closure may be incomplete.
    ParseError { detail: String },
    /// W007 — a dependency reached via `../` lives above the first entry's own directory,
    /// so the archive root had to widen to include it.
    RootEscaped { expected: PathBuf, actual: PathBuf },
    /// W008 — `.zy` files sitting in the entry scripts' own directory that are not reachable
    /// from any `[[script]]` entry (per the strict-closure policy: not listed, not a
    /// dependency ⇒ not packaged). Named explicitly so the author isn't surprised by what's
    /// missing. Scoped to the entries' directory rather than the archive root on purpose —
    /// see the scan in [`compute_closure`] for why.
    Unreached(Vec<String>),
    /// W009 — `std/db` is imported. Never packaged (stdlib is synthetic, not a file), but
    /// worth flagging because it is also unavailable in the web playground.
    StdDb,
    /// W010 — the archive's total uncompressed size exceeds a recommended ceiling.
    /// Informational only; nothing is left out because of it. Raised by the writer (it's
    /// the one that knows the actual byte sizes), not by [`compute_closure`].
    SizeLimit(u64),
    /// W011 — the same file on disk was reached through two different lexical paths (most
    /// commonly a symlink). It's packaged once, under the name it was first reached with.
    DuplicateCanonical { canonical: PathBuf, kept_as: PathBuf },
}

impl WarningKind {
    pub fn code(&self) -> &'static str {
        match self {
            WarningKind::AbsoluteImport { .. } => "W001",
            WarningKind::ModuleNotFound { .. } => "W002",
            WarningKind::BashExec => "W003",
            WarningKind::ExecuteEntryDependent { .. } => "W004",
            WarningKind::ExecuteMissing { .. } => "W005",
            WarningKind::ParseError { .. } => "W006",
            WarningKind::RootEscaped { .. } => "W007",
            WarningKind::Unreached(_) => "W008",
            WarningKind::StdDb => "W009",
            WarningKind::SizeLimit(_) => "W010",
            WarningKind::DuplicateCanonical { .. } => "W011",
        }
    }
}

impl std::fmt::Display for WarningKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WarningKind::AbsoluteImport { components } => write!(
                f,
                "import '{components}' is absolute or home-relative (/ or ~/) — \
                 not reproducible on another machine ($HOME); not packaged"
            ),
            WarningKind::ModuleNotFound { module } => {
                write!(f, "'{module}' referenced but not found on disk; not packaged")
            }
            WarningKind::BashExec => write!(
                f,
                "uses <\\ ... \\> (shell exec) — its dependencies are dynamic expressions \
                 and cannot be tracked statically"
            ),
            WarningKind::ExecuteEntryDependent { first_base, other_base } => write!(
                f,
                "contains </ /> and is reachable from another [[script]] whose directory \
                 differs ({} vs {}); only the first entry's resolution was packaged",
                first_base.display(),
                other_base.display()
            ),
            WarningKind::ExecuteMissing { path } => write!(
                f,
                "</ {path} /> target not found relative to the running script's directory"
            ),
            WarningKind::ParseError { detail } => write!(
                f,
                "lex/parse error — file is packaged anyway, but its own dependencies could \
                 not be traced: {detail}"
            ),
            WarningKind::RootEscaped { expected, actual } => write!(
                f,
                "a dependency lives above the entry's directory ({}); archive root widened \
                 to {}",
                expected.display(),
                actual.display()
            ),
            WarningKind::Unreached(names) => write!(
                f,
                "{} .zy file(s) in the entry script(s)' directory are not reachable from any \
                 [[script]] and were not packaged: {}",
                names.len(),
                names.join(", ")
            ),
            WarningKind::StdDb => write!(
                f,
                "imports std/db — not available in the web playground (requires ODBC)"
            ),
            WarningKind::SizeLimit(bytes) => write!(
                f,
                "package is {bytes} uncompressed bytes, above the 5 MB recommended ceiling — \
                 nothing was left out, this is informational"
            ),
            WarningKind::DuplicateCanonical { canonical, kept_as } => write!(
                f,
                "same file as '{}' (reached via a different path); kept once, as '{}': {}",
                kept_as.display(),
                kept_as.display(),
                canonical.display()
            ),
        }
    }
}

impl std::fmt::Display for PackageWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.file.as_os_str().is_empty() {
            write!(f, "{}: {}", self.kind.code(), self.kind)
        } else {
            write!(f, "{}: {}: {}", self.kind.code(), self.file.display(), self.kind)
        }
    }
}

/// One already-visited file: its lexical absolute path (for naming/reading), the directory
/// any `</ />` inside it resolves against (the nearest enclosing script — see module docs),
/// and whether it contains any `</ />` at all (so a later cross-entry visit with a
/// different `script_base` knows whether [`WarningKind::ExecuteEntryDependent`] applies).
struct Visited {
    abs: PathBuf,
    script_base: PathBuf,
    has_execute: bool,
}

/// Computes the transitive closure of `.zy` files needed by `entries` (a package's
/// `[[script]]` paths, already resolved to filesystem paths). Runs one breadth-first walk
/// per entry, each starting with its own `script_base` (its own parent directory — see
/// module docs on why that's the correct resolution base for `</ />` in *both* engines),
/// and unions the results, deduplicating files reached from more than one entry.
pub fn compute_closure(entries: &[PathBuf]) -> std::io::Result<ClosureResult> {
    // canonical path (identity/dedup key only, never used for naming) -> visit record.
    let mut visited: HashMap<PathBuf, Visited> = HashMap::new();
    let mut warnings: Vec<PackageWarning> = Vec::new();

    for entry in entries {
        let abs_entry = lexical_absolute(entry)?;
        let script_base = abs_entry
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        walk_entry(&abs_entry, &script_base, &mut visited, &mut warnings);
    }

    if visited.is_empty() {
        return Ok(ClosureResult { root: PathBuf::from("."), files: Vec::new(), warnings });
    }

    let root = common_ancestor(visited.values().filter_map(|v| v.abs.parent()));

    if let Some(first_entry) = entries.first() {
        if let Ok(abs_first) = lexical_absolute(first_entry) {
            let expected_root = abs_first
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            if root != expected_root {
                warnings.push(PackageWarning {
                    file: PathBuf::new(),
                    kind: WarningKind::RootEscaped { expected: expected_root, actual: root.clone() },
                });
            }
        }
    }

    let mut files: Vec<PackagedFile> = visited
        .values()
        .map(|v| {
            let rel = v
                .abs
                .strip_prefix(&root)
                .unwrap_or(&v.abs)
                .to_string_lossy()
                .replace('\\', "/");
            PackagedFile { rel_path: rel, abs_path: v.abs.clone() }
        })
        .collect();
    files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    // W008: .zy files the author might be surprised to find missing. Scanned from the
    // *entries'* common directory, not from `root`.
    //
    // Those two differ whenever a dependency lives above the entry (W007), and scanning
    // `root` there is both noisy and slow: an entry at `proj/main.zy` importing
    // `../shared/lib` widens `root` to the parent of both, so the scan would walk every
    // unrelated sibling project under it and report their files as "not packaged" — and if
    // a project imports from a directory high up, `root` can climb to the home directory
    // and the scan walks all of it. The entries' own directory is what the author thinks of
    // as "the project", which is the only scope where "you might have expected this to be
    // included" is a meaningful thing to say.
    let entry_scan_root = common_ancestor(
        entries
            .iter()
            .filter_map(|e| lexical_absolute(e).ok())
            .filter_map(|abs| abs.parent().map(Path::to_path_buf))
            .collect::<Vec<_>>()
            .iter()
            .map(PathBuf::as_path),
    );

    // Compared by canonical path (visited's keys) rather than by the walked path's raw
    // bytes, so this check is immune to the same NFC/NFD divergence that naming (above)
    // deliberately avoids relying on.
    let reached_canonical: HashSet<&PathBuf> = visited.keys().collect();
    let mut unreached = Vec::new();
    collect_zy_files(&entry_scan_root, &mut |p: &Path| {
        let canon = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        if !reached_canonical.contains(&canon) {
            let rel = p
                .strip_prefix(&entry_scan_root)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/");
            unreached.push(rel);
        }
    });
    if !unreached.is_empty() {
        unreached.sort();
        warnings.push(PackageWarning { file: PathBuf::new(), kind: WarningKind::Unreached(unreached) });
    }

    Ok(ClosureResult { root, files, warnings })
}

fn walk_entry(
    entry_abs: &Path,
    entry_script_base: &Path,
    visited: &mut HashMap<PathBuf, Visited>,
    warnings: &mut Vec<PackageWarning>,
) {
    // (abs path to visit, script_base to resolve any </ /> inside it against)
    let mut queue: VecDeque<(PathBuf, PathBuf)> = VecDeque::new();
    queue.push_back((entry_abs.to_path_buf(), entry_script_base.to_path_buf()));

    while let Some((abs, script_base)) = queue.pop_front() {
        let canonical = abs.canonicalize().unwrap_or_else(|_| abs.clone());
        if let Some(existing) = visited.get(&canonical) {
            if existing.abs != abs {
                warnings.push(PackageWarning {
                    file: abs.clone(),
                    kind: WarningKind::DuplicateCanonical { canonical, kept_as: existing.abs.clone() },
                });
            }
            if existing.has_execute && existing.script_base != script_base {
                warnings.push(PackageWarning {
                    file: existing.abs.clone(),
                    kind: WarningKind::ExecuteEntryDependent {
                        first_base: existing.script_base.clone(),
                        other_base: script_base,
                    },
                });
            }
            continue;
        }

        let source = match fs::read_to_string(&abs) {
            Ok(s) => s,
            Err(_) => {
                warnings.push(PackageWarning {
                    file: abs.clone(),
                    kind: WarningKind::ModuleNotFound { module: abs.display().to_string() },
                });
                continue;
            }
        };

        let (tokens, _lex_diags) = Lexer::new(&source, FileId(0)).tokenize();
        // Lexer errors don't stop token production, so ExecuteCommand/BashOpen tokens are
        // still scanned below even for a file with e.g. an unterminated string elsewhere.

        let has_execute = tokens.iter().any(|t| matches!(t.kind, TokenKind::ExecuteCommand(_)));
        visited.insert(
            canonical,
            Visited { abs: abs.clone(), script_base: script_base.clone(), has_execute },
        );

        let file_dir = abs.parent().unwrap_or(Path::new("."));

        for tok in &tokens {
            match &tok.kind {
                TokenKind::ExecuteCommand(raw) => {
                    let raw_path = strip_quotes(raw);
                    if raw_path.is_empty() {
                        continue;
                    }
                    handle_execute(&abs, &script_base, &raw_path, &mut queue, warnings);
                }
                TokenKind::BashOpen => {
                    warnings.push(PackageWarning { file: abs.clone(), kind: WarningKind::BashExec });
                }
                _ => {}
            }
        }

        let program = match Parser::new(tokens).parse() {
            Ok(p) => p,
            Err(diags) => {
                let detail = diags
                    .iter()
                    .map(|d| d.message.clone())
                    .collect::<Vec<_>>()
                    .join("; ");
                warnings.push(PackageWarning { file: abs.clone(), kind: WarningKind::ParseError { detail } });
                continue;
            }
        };

        for import in &program.imports {
            if import.path.is_stdlib() {
                if import.path.components.join("/") == "std/db" {
                    warnings.push(PackageWarning { file: abs.clone(), kind: WarningKind::StdDb });
                }
                continue;
            }
            if import.path.is_absolute {
                warnings.push(PackageWarning {
                    file: abs.clone(),
                    kind: WarningKind::AbsoluteImport { components: import.path.components.join("/") },
                });
                continue;
            }
            match import.path.resolve_from(file_dir) {
                // Imports inherit the *containing file's* directory for further resolution,
                // but keep the same script_base — a module's own directory never becomes a
                // script_base; only crossing a </ /> boundary does (see handle_execute).
                Some(dep) => queue.push_back((dep, script_base.clone())),
                None => warnings.push(PackageWarning {
                    file: abs.clone(),
                    kind: WarningKind::ModuleNotFound { module: import.path.components.join("/") },
                }),
            }
        }
    }
}

/// Resolves one `</ raw_path />` relative to `script_base` — the directory of the nearest
/// enclosing script, which both engines agree on (see module docs) — and queues the target
/// with a *new* script_base of its own (its own parent directory), since executing it starts
/// a fresh script context for any `</ />` found inside *it*.
fn handle_execute(
    containing_file: &Path,
    script_base: &Path,
    raw_path: &str,
    queue: &mut VecDeque<(PathBuf, PathBuf)>,
    warnings: &mut Vec<PackageWarning>,
) {
    let target = if raw_path.starts_with('/') {
        PathBuf::from(raw_path)
    } else {
        script_base.join(raw_path)
    };

    if !target.is_file() {
        warnings.push(PackageWarning {
            file: containing_file.to_path_buf(),
            kind: WarningKind::ExecuteMissing { path: raw_path.to_string() },
        });
        return;
    }

    let target_script_base = target.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    queue.push_back((target, target_script_base));
}


fn strip_quotes(raw: &str) -> String {
    // Mirrors zymbol-parser's parse_execute_expr: </ "path.zy" /> is written with quotes
    // for the formatter's benefit, but the path itself is unquoted.
    if raw.starts_with('"') && raw.ends_with('"') && raw.len() > 1 {
        raw[1..raw.len() - 1].to_string()
    } else {
        raw.to_string()
    }
}

/// Makes `p` absolute *lexically* — joining with the cwd and resolving `.`/`..` components
/// without touching the filesystem or resolving symlinks. Deliberately not
/// `Path::canonicalize()`: canonicalize would also normalize Unicode on some platforms
/// (macOS/APFS returns NFD), which would corrupt CJK/Hangul file names used as zip entry
/// names. See the [`PackagedFile::rel_path`] doc comment.
fn lexical_absolute(p: &Path) -> std::io::Result<PathBuf> {
    std::path::absolute(p)
}

fn common_ancestor<'a>(mut dirs: impl Iterator<Item = &'a Path>) -> PathBuf {
    let mut root = match dirs.next() {
        Some(d) => d.to_path_buf(),
        None => return PathBuf::from("."),
    };
    for dir in dirs {
        while !dir.starts_with(&root) {
            if !root.pop() {
                break;
            }
        }
    }
    root
}

fn collect_zy_files(dir: &Path, visit: &mut impl FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_zy_files(&path, visit);
        } else if path.extension().is_some_and(|e| e == "zy") {
            visit(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Builds, under a fresh tempdir:
    ///
    /// ```text
    /// entry.zy            <# ./lib/a => a ; </ ./sub.zy /> ; <\ "echo hi" \> ; a::hola()
    /// sub.zy               (a plain script, reached via </ />)
    /// lib/a.zy             module: <# ../lib2/b => b ; hola() calls </ ./nested.zy /> and b::saludo()
    /// lib/nested.zy         (reached via </ /> INSIDE the module — script_base must be
    ///                        the entry's dir, not lib/, per the module docs; this is the
    ///                        regression test for the bug an earlier draft had)
    /// lib2/b.zy             module, no imports
    /// orphan.zy             not reachable from entry.zy at all — must trigger W008
    /// ```
    ///
    /// This mirrors real GO structure (entry -> module -> module, plus one `</ />` and one
    /// `<\ \>`) closely enough to exercise every warning code this crate can produce except
    /// W001/W007/W009/W010/W011, which need their own narrower fixtures below.
    fn build_fixture(dir: &Path) {
        fs::create_dir_all(dir.join("lib")).unwrap();
        fs::create_dir_all(dir.join("lib2")).unwrap();

        fs::write(
            dir.join("entry.zy"),
            "<# ./lib/a => a\n\
             salida_bash = <\\ \"echo hi\" \\>\n\
             resultado = </ ./sub.zy />\n\
             a::hola()\n",
        )
        .unwrap();

        fs::write(dir.join("sub.zy"), ">> \"soy sub\" \u{b6}\n").unwrap();

        fs::write(
            dir.join("lib/a.zy"),
            "# a {\n\
             \n\
             <# ../lib2/b => b\n\
             \n\
             #> {\n\
             \x20\x20\x20\x20hola\n\
             }\n\
             \n\
             hola() {\n\
             \x20\x20\x20\x20nested_result = </ ./nested.zy />\n\
             \x20\x20\x20\x20<~ b::saludo()\n\
             }\n\
             }\n",
        )
        .unwrap();

        // Reached only through the module's own `</ />` — the whole point of the fixture is
        // that this must resolve relative to `dir` (the entry's script_base), NOT relative
        // to `dir/lib` (where the token is lexically written).
        fs::write(dir.join("nested.zy"), ">> \"soy nested\" \u{b6}\n").unwrap();

        fs::write(
            dir.join("lib2/b.zy"),
            "# b {\n\
             \n\
             #> {\n\
             \x20\x20\x20\x20saludo\n\
             }\n\
             \n\
             saludo() {\n\
             \x20\x20\x20\x20<~ \"hola desde b\"\n\
             }\n\
             }\n",
        )
        .unwrap();

        fs::write(dir.join("orphan.zy"), ">> \"nadie me importa\" \u{b6}\n").unwrap();
    }

    #[test]
    fn closure_reaches_exactly_the_expected_files() {
        let tmp = tempfile::tempdir().unwrap();
        build_fixture(tmp.path());

        let result = compute_closure(&[tmp.path().join("entry.zy")]).unwrap();

        let mut names: Vec<&str> = result.files.iter().map(|f| f.rel_path.as_str()).collect();
        names.sort();
        assert_eq!(
            names,
            vec!["entry.zy", "lib/a.zy", "lib2/b.zy", "nested.zy", "sub.zy"],
            "orphan.zy must NOT be in the closure (strict-closure policy); \
             nested.zy MUST be, reached only via </ /> inside the module"
        );
    }

    #[test]
    fn execute_inside_a_module_resolves_against_the_entry_directory() {
        // The regression test for the bug this module's doc comment describes: an earlier
        // implementation resolved `</ ./nested.zy />` (written inside lib/a.zy) relative to
        // `lib/`, which is what the real tree-walker and VM do NOT do.
        let tmp = tempfile::tempdir().unwrap();
        build_fixture(tmp.path());

        let result = compute_closure(&[tmp.path().join("entry.zy")]).unwrap();
        let nested = result.files.iter().find(|f| f.rel_path == "nested.zy");
        assert!(nested.is_some(), "nested.zy (at the root, not lib/nested.zy) must be packaged");
    }

    #[test]
    fn warns_bash_exec_and_reports_unreached_sibling() {
        let tmp = tempfile::tempdir().unwrap();
        build_fixture(tmp.path());

        let result = compute_closure(&[tmp.path().join("entry.zy")]).unwrap();

        assert!(
            result.warnings.iter().any(|w| w.kind.code() == "W003"),
            "the <\\ \"echo hi\" \\> in entry.zy must produce a W003 warning"
        );
        let unreached = result
            .warnings
            .iter()
            .find_map(|w| match &w.kind {
                WarningKind::Unreached(names) => Some(names),
                _ => None,
            })
            .expect("orphan.zy must produce a W008 warning");
        assert_eq!(unreached, &vec!["orphan.zy".to_string()]);
    }

    #[test]
    fn absolute_import_is_warned_and_not_packaged() {
        // W001 fires before the target is ever looked up on disk (`is_absolute` is checked
        // ahead of `resolve_from`), so the import target doesn't need to exist — a fixed,
        // plain-ASCII synthetic path keeps this test independent of the real tempdir name
        // (which on this machine is dot-prefixed, e.g. `.tmpXXXX` — not a valid path
        // component, so using the *actual* tempdir path here would itself be a parse error
        // rather than the absolute-import case this test wants to exercise).
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("entry.zy"),
            "<# /some/madeup/lib => lib\n>> \"hi\" \u{b6}\n",
        )
        .unwrap();

        let result = compute_closure(&[tmp.path().join("entry.zy")]).unwrap();
        assert_eq!(result.files.len(), 1, "only entry.zy itself, the absolute import is not packaged");
        assert!(result.warnings.iter().any(|w| w.kind.code() == "W001"));
    }

    #[test]
    fn missing_module_is_warned_and_entry_still_packaged() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("entry.zy"), "<# ./does_not_exist => x\n>> \"hi\" \u{b6}\n").unwrap();

        let result = compute_closure(&[tmp.path().join("entry.zy")]).unwrap();
        assert_eq!(result.files.len(), 1);
        assert!(result.warnings.iter().any(|w| w.kind.code() == "W002"));
    }

    #[test]
    fn root_widens_above_the_entry_when_a_dependency_lives_higher_up() {
        // layout: tmp/proj/entry.zy imports ../shared/lib.zy (a sibling of proj/, not
        // under it) — the archive root must widen from tmp/proj to tmp, and W007 must fire.
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("proj")).unwrap();
        fs::create_dir_all(tmp.path().join("shared")).unwrap();
        fs::write(tmp.path().join("proj/entry.zy"), "<# ../shared/lib => lib\n>> \"hi\" \u{b6}\n").unwrap();
        fs::write(tmp.path().join("shared/lib.zy"), "# lib {\n#> { }\n}\n").unwrap();

        let result = compute_closure(&[tmp.path().join("proj/entry.zy")]).unwrap();
        assert_eq!(result.root, tmp.path());
        assert!(result.warnings.iter().any(|w| w.kind.code() == "W007"));
        let mut names: Vec<&str> = result.files.iter().map(|f| f.rel_path.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["proj/entry.zy", "shared/lib.zy"]);
    }

    /// W008's scan is bounded to the entries' own directory, not the (possibly much wider)
    /// archive root. Before this, an entry importing `../shared/lib` widened the root to the
    /// parent of both and the scan reported every unrelated sibling project's `.zy` files as
    /// "not packaged" — noise at best, and a walk of the whole home directory at worst.
    #[test]
    fn unreached_scan_does_not_leak_into_unrelated_sibling_projects() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("proj")).unwrap();
        fs::create_dir_all(tmp.path().join("shared")).unwrap();
        fs::create_dir_all(tmp.path().join("unrelated/sub")).unwrap();

        fs::write(tmp.path().join("proj/entry.zy"), "<# ../shared/lib => lib\n>> \"hi\" \u{b6}\n").unwrap();
        fs::write(tmp.path().join("proj/sibling.zy"), ">> \"mine, unreached\" \u{b6}\n").unwrap();
        fs::write(tmp.path().join("shared/lib.zy"), "# lib {\n#> { }\n}\n").unwrap();
        fs::write(tmp.path().join("unrelated/sub/theirs.zy"), ">> \"not mine\" \u{b6}\n").unwrap();

        let result = compute_closure(&[tmp.path().join("proj/entry.zy")]).unwrap();

        // Root still widens (that's W007's job) so the archive can hold shared/lib.zy...
        assert_eq!(result.root, tmp.path());
        assert!(result.warnings.iter().any(|w| w.kind.code() == "W007"));

        // ...but the "you might have expected these" scan stays inside proj/.
        let unreached = result
            .warnings
            .iter()
            .find_map(|w| match &w.kind {
                WarningKind::Unreached(names) => Some(names.clone()),
                _ => None,
            })
            .expect("proj/sibling.zy should still be reported");
        assert_eq!(
            unreached,
            vec!["sibling.zy".to_string()],
            "must report only the entry directory's own unreached file, never the unrelated project's"
        );
    }

    #[test]
    fn stdlib_imports_are_never_packaged_and_std_db_is_flagged() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("entry.zy"),
            "<# std/math => m\n<# std/db => d\n>> m.PI \u{b6}\n",
        )
        .unwrap();

        let result = compute_closure(&[tmp.path().join("entry.zy")]).unwrap();
        assert_eq!(result.files.len(), 1, "std/* is synthetic, never a file to package");
        assert!(result.warnings.iter().any(|w| w.kind.code() == "W009"));
    }

    #[test]
    fn same_module_from_two_entries_with_different_bases_is_flagged() {
        // Two entries in different directories both import the same shared module, which
        // itself uses </ />. Only the first entry's script_base is used to resolve it, and
        // the mismatch must be flagged (W004) rather than silently packaging the wrong file.
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("a")).unwrap();
        fs::create_dir_all(tmp.path().join("b")).unwrap();
        fs::create_dir_all(tmp.path().join("shared")).unwrap();

        fs::write(tmp.path().join("a/entry.zy"), "<# ../shared/mod => m\nm::go()\n").unwrap();
        fs::write(tmp.path().join("b/entry.zy"), "<# ../shared/mod => m\nm::go()\n").unwrap();
        fs::write(
            tmp.path().join("shared/mod.zy"),
            "# mod {\n#> { go }\ngo() {\n\x20\x20\x20\x20<~ </ ./x.zy />\n}\n}\n",
        )
        .unwrap();
        fs::write(tmp.path().join("a/x.zy"), ">> \"a\" \u{b6}\n").unwrap();
        fs::write(tmp.path().join("b/x.zy"), ">> \"b\" \u{b6}\n").unwrap();

        let result = compute_closure(&[tmp.path().join("a/entry.zy"), tmp.path().join("b/entry.zy")]).unwrap();
        assert!(
            result.warnings.iter().any(|w| w.kind.code() == "W004"),
            "warnings were: {:#?}",
            result.warnings
        );
    }
}
