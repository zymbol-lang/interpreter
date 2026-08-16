//! Argument counts for the functions an `alias::` prefix can reach.
//!
//! A call to a local function is checked against its declaration by
//! [`TypeChecker`](crate::TypeChecker): `f("a", "b")` where `f` takes one
//! parameter is a hard error. The same call written `m::f("a", "b")` was not
//! checked at all — the type checker only ever looked at a bare identifier as
//! the callable, so every qualified call was a blind spot. The mistake then
//! survived `zymbol check` and surfaced only when the program ran, and only if
//! that branch ran: the bug this module exists for sat in a "server not
//! reachable" arm, the one path nobody exercises.
//!
//! Resolving `alias::f` to a parameter count needs the module's source, so the
//! table is built here and handed to the type checker with
//! [`TypeChecker::set_module_arities`](crate::TypeChecker::set_module_arities)
//! rather than looked up mid-traversal. The type checker stays free of file
//! I/O, and the LSP can supply the same table from unsaved editor buffers
//! instead of from disk.
//!
//! Two sources feed it:
//!
//! - `std/` modules are native, and `zymbol_common::stdlib` already records an
//!   arity per function — `-1` for the variadic ones such as `net::get`, which
//!   accepts a URL with or without a header map.
//! - A user module is parsed, and each name in its export block is matched to
//!   the declaration it exports. A re-export (`t::width => ancho`) is followed
//!   to the module it came from, up to [`MAX_REEXPORT_DEPTH`] hops.
//!
//! Anything that cannot be resolved statically — a missing file, a parse error,
//! an alias bound to a module that re-exports through a cycle — contributes no
//! entry. A call is only ever reported when the arity is known, so an
//! unresolvable import degrades to today's behaviour instead of inventing an
//! error.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use zymbol_ast::{ExportItem, ImportStmt, Program, Statement};
use zymbol_common::stdlib;

/// Parameter counts for one module's exported functions, keyed by the name a
/// caller writes after `::`. A `-1` marks a variadic function, which accepts
/// any number of arguments and is therefore never reported.
pub type ModuleArities = HashMap<String, i32>;

/// Exported-function arities per import alias.
pub type AliasArities = HashMap<String, ModuleArities>;

/// How many re-export hops to follow before giving up.
///
/// An i18n layer module is one hop (`capa_es` re-exports `std/term`); a stack of
/// them is a handful. The limit is what stops a cycle — `a` re-exporting from
/// `b` while `b` re-exports from `a` — from recursing forever. Circular imports
/// are reported separately by [`ModuleAnalyzer`](crate::ModuleAnalyzer); here
/// the only job is to terminate.
const MAX_REEXPORT_DEPTH: usize = 8;

/// Build the arity table for every import in `program`.
///
/// `base_dir` is the directory the file being checked lives in — the same base
/// `ModulePath::resolve_from` takes, so an import resolves to the file the
/// interpreter and the VM would load.
pub fn module_arities(imports: &[ImportStmt], base_dir: &Path) -> AliasArities {
    let mut table = AliasArities::new();
    for import in imports {
        if let Some(arities) = arities_of_import(import, base_dir, 0) {
            table.insert(import.alias.clone(), arities);
        }
    }
    table
}

/// Arities exported by the module one import points at, native or on disk.
fn arities_of_import(import: &ImportStmt, base_dir: &Path, depth: usize) -> Option<ModuleArities> {
    if import.path.is_stdlib() {
        let path = format!("std/{}", import.path.components[1..].join("/"));
        let module = stdlib::module(&path)?;
        return Some(
            module
                .functions
                .iter()
                .map(|f| (f.name.to_string(), f.arity))
                .collect(),
        );
    }

    let resolved = import.path.resolve_from(base_dir)?;
    arities_of_file(&resolved, depth)
}

/// Arities exported by a module file.
///
/// Only the names in the export block are collected: a private helper is not
/// reachable through the alias, so recording it would let a typo that happens
/// to match one pass as a real call.
fn arities_of_file(path: &Path, depth: usize) -> Option<ModuleArities> {
    if depth > MAX_REEXPORT_DEPTH {
        return None;
    }

    let source = std::fs::read_to_string(path).ok()?;
    let program = parse(&source)?;
    let module_decl = program.module_decl.as_ref()?;
    let export_block = module_decl.export_block.as_ref()?;

    // Parameter counts of everything declared in the module, exported or not —
    // the export block names them, and a rename points back to the original.
    let declared: HashMap<&str, i32> = program
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Statement::FunctionDecl(decl) => {
                Some((decl.name.as_str(), decl.parameters.len() as i32))
            }
            _ => None,
        })
        .collect();

    let module_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut arities = ModuleArities::new();

    for item in &export_block.items {
        match item {
            ExportItem::Own { name, rename, .. } => {
                if let Some(&arity) = declared.get(name.as_str()) {
                    let public = rename.as_ref().unwrap_or(name);
                    arities.insert(public.clone(), arity);
                }
            }

            // `t::width => ancho` — the arity lives in whatever `t` resolves
            // to, so follow the import that bound `t` inside *this* module.
            ExportItem::ReExport {
                module_alias,
                item_name,
                rename,
                ..
            } => {
                let Some(source_import) =
                    program.imports.iter().find(|i| &i.alias == module_alias)
                else {
                    continue;
                };
                let Some(source_arities) = arities_of_import(source_import, module_dir, depth + 1)
                else {
                    continue;
                };
                if let Some(&arity) = source_arities.get(item_name.as_str()) {
                    let public = rename.as_ref().unwrap_or(item_name);
                    arities.insert(public.clone(), arity);
                }
            }
        }
    }

    Some(arities)
}

/// Parse module source, discarding the diagnostics.
///
/// A module that does not parse is reported by whichever check follows the
/// import; contributing no arities here just means its callers are not checked.
fn parse(source: &str) -> Option<Program> {
    let (tokens, _) = zymbol_lexer::Lexer::new(source, zymbol_span::FileId(0)).tokenize();
    zymbol_parser::Parser::new(tokens).parse().ok()
}

/// Resolve a module file to its exported arities, for callers that already know
/// the path — the LSP, which holds resolved module paths in its document cache.
pub fn arities_of_module_file(path: &Path) -> Option<ModuleArities> {
    arities_of_file(path, 0)
}

// ── Output-parameter slots, per alias ────────────────────────────────────────
//
// The call-site mark `m::f(x<~)` is checked against the callee's signature the
// same way a bare `f(x<~)` is (REFERENCE.md L36). This is a table beside the
// arity one rather than a wider value type: `ModuleArities` is read in six
// places across three crates and mirrored in `zymbol.js`, and none of them has
// anything to say about output parameters.

/// Which parameter slots of each exported function are `<~` outputs.
pub type ModuleOutSlots = HashMap<String, Vec<usize>>;

/// The same, per import alias.
pub type AliasOutSlots = HashMap<String, ModuleOutSlots>;

/// Build the output-slot table for every import in `program`.
///
/// A `std/` module contributes nothing: the standard library declares no output
/// parameter. A module that cannot be resolved or parsed is simply absent, and
/// its calls go unchecked rather than guessed at — the same rule the arity check
/// already follows.
pub fn module_out_slots(imports: &[ImportStmt], base_dir: &Path) -> AliasOutSlots {
    let mut table = AliasOutSlots::new();
    for import in imports {
        if import.path.is_stdlib() {
            continue;
        }
        let Some(path) = import.path.resolve_from(base_dir) else { continue };
        let Some(slots) = out_slots_of_file(&path) else { continue };
        if !slots.is_empty() {
            table.insert(import.alias.clone(), slots);
        }
    }
    table
}

/// The exported functions of one module file that take output parameters.
fn out_slots_of_file(path: &Path) -> Option<ModuleOutSlots> {
    let source = std::fs::read_to_string(path).ok()?;
    let program = parse(&source)?;
    let module_decl = program.module_decl.as_ref()?;
    let export_block = module_decl.export_block.as_ref()?;

    // Slots of everything declared in the module, exported or not — the export
    // block names them, and a rename points back to the original.
    let declared: HashMap<&str, Vec<usize>> = program
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Statement::FunctionDecl(decl) => {
                let slots: Vec<usize> = decl.parameters.iter().enumerate()
                    .filter(|(_, p)| matches!(p.kind, zymbol_ast::ParameterKind::Output))
                    .map(|(i, _)| i)
                    .collect();
                Some((decl.name.as_str(), slots))
            }
            _ => None,
        })
        .collect();

    let mut out = ModuleOutSlots::new();
    for item in &export_block.items {
        if let ExportItem::Own { name, rename, .. } = item {
            if let Some(slots) = declared.get(name.as_str()) {
                if !slots.is_empty() {
                    let public = rename.clone().unwrap_or_else(|| name.clone());
                    out.insert(public, slots.clone());
                }
            }
        }
    }
    Some(out)
}

/// The file an import resolves to, or `None` for `std/` and unresolvable paths.
pub fn resolved_import_path(import: &ImportStmt, base_dir: &Path) -> Option<PathBuf> {
    import.path.resolve_from(base_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn imports_of(source: &str) -> Vec<ImportStmt> {
        let (tokens, _) = zymbol_lexer::Lexer::new(source, zymbol_span::FileId(0)).tokenize();
        zymbol_parser::Parser::new(tokens)
            .parse()
            .expect("test source must parse")
            .imports
    }

    fn write(dir: &Path, name: &str, source: &str) {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(source.as_bytes()).unwrap();
    }

    /// A scratch directory that cleans itself up.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let mut path = std::env::temp_dir();
            path.push(format!(
                "zymbol-arity-{}-{}-{:?}",
                tag,
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn reads_stdlib_arities_including_variadic() {
        let table = module_arities(&imports_of("<# std/math => math\n"), Path::new("."));
        assert_eq!(table["math"].get("sqrt"), Some(&1));

        let table = module_arities(&imports_of("<# std/net => net\n"), Path::new("."));
        assert_eq!(table["net"].get("get"), Some(&-1));
    }

    #[test]
    fn reads_exported_functions_of_a_user_module() {
        let dir = TempDir::new("own");
        write(
            &dir.0,
            "lib/ui.zy",
            "# ui {\n    #> { show_error }\n\n    show_error(msg) {\n        >> msg ¶\n    }\n\n    hidden(a, b) {\n        >> a b ¶\n    }\n}\n",
        );

        let table = module_arities(&imports_of("<# ./lib/ui => ui\n"), &dir.0);
        assert_eq!(table["ui"].get("show_error"), Some(&1));
        // A private helper is not reachable through the alias.
        assert_eq!(table["ui"].get("hidden"), None);
    }

    #[test]
    fn a_rename_is_keyed_by_its_public_name() {
        let dir = TempDir::new("rename");
        write(
            &dir.0,
            "m.zy",
            "# m {\n    #> { draw => dibuja }\n\n    draw(x, y) {\n        >> x y ¶\n    }\n}\n",
        );

        let table = module_arities(&imports_of("<# ./m => m\n"), &dir.0);
        assert_eq!(table["m"].get("dibuja"), Some(&2));
        assert_eq!(table["m"].get("draw"), None);
    }

    #[test]
    fn follows_a_re_export_to_the_module_it_came_from() {
        let dir = TempDir::new("reexport");
        write(
            &dir.0,
            "capa.zy",
            "# capa {\n    #> {\n        t::width => ancho\n    }\n\n    <# std/term => t\n}\n",
        );

        let table = module_arities(&imports_of("<# ./capa => capa\n"), &dir.0);
        assert_eq!(
            table["capa"].get("ancho"),
            stdlib::module("std/term")
                .and_then(|m| m.function("width"))
                .map(|f| &f.arity)
        );
    }

    #[test]
    fn an_unresolvable_import_contributes_nothing() {
        let dir = TempDir::new("missing");
        let table = module_arities(&imports_of("<# ./no_existe => x\n"), &dir.0);
        assert!(table.get("x").is_none());
    }

    /// Two modules re-exporting from each other must terminate, not recurse.
    #[test]
    fn a_re_export_cycle_terminates() {
        let dir = TempDir::new("cycle");
        write(
            &dir.0,
            "a.zy",
            "# a {\n    #> {\n        b::f => f\n    }\n\n    <# ./b => b\n}\n",
        );
        write(
            &dir.0,
            "b.zy",
            "# b {\n    #> {\n        a::f => f\n    }\n\n    <# ./a => a\n}\n",
        );

        let table = module_arities(&imports_of("<# ./a => a\n"), &dir.0);
        assert_eq!(table["a"].get("f"), None);
    }
}
