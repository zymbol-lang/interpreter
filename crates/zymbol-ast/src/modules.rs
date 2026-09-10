//! Module system AST nodes for Zymbol-Lang
//!
//! Contains AST structures for the module system:
//! - Module declaration: # module_name (with optional dot prefix for folders)
//! - Export blocks: #> { items } (public API definition)
//! - Import statements: <# path <= alias (import with required alias)
//! - Module paths: ./relative, ../parent, absolute paths

use std::path::{Path, PathBuf};
use zymbol_span::Span;

/// Module declaration: # module_name [#> { exports }]
#[derive(Debug, Clone)]
pub struct ModuleDecl {
    pub name: String,
    pub export_block: Option<ExportBlock>,
    pub span: Span,
}

/// Export block: #> { items }
#[derive(Debug, Clone)]
pub struct ExportBlock {
    pub items: Vec<ExportItem>,
    /// `commas[i]` is true when item `i` was followed by a `,` in the source
    /// (commas are optional separators); used by the formatter for fidelity.
    pub commas: Vec<bool>,
    pub span: Span,
}

/// Items that can be exported
#[derive(Debug, Clone)]
pub enum ExportItem {
    /// Own item: identifier [<= public_name]
    Own {
        name: String,
        rename: Option<String>,
        span: Span,
    },
    /// Re-export: alias::function or alias.CONSTANT
    ReExport {
        module_alias: String,
        item_name: String,
        item_type: ItemType,
        rename: Option<String>,
        span: Span,
    },
}

/// Type of item being re-exported
#[derive(Debug, Clone, PartialEq)]
pub enum ItemType {
    /// Function (uses ::)
    Function,
    /// Constant (uses .)
    Constant,
}

/// Import statement: <# path <= alias
#[derive(Debug, Clone)]
pub struct ImportStmt {
    pub path: ModulePath,
    pub alias: String,
    pub span: Span,
}

/// Module path: ./dir/module, ../module, /absolute/path, ~/home/path
#[derive(Debug, Clone)]
pub struct ModulePath {
    pub components: Vec<String>,
    pub is_relative: bool,
    pub is_absolute: bool,   // true for /absolute and ~/home paths
    pub home_relative: bool, // true for ~/home paths (subset of is_absolute)
    pub parent_levels: usize, // 0 for ./, 1 for ../, 2 for ../../
    pub span: Span,
}

impl ModuleDecl {
    pub fn new(name: String, export_block: Option<ExportBlock>, span: Span) -> Self {
        Self {
            name,
            export_block,
            span,
        }
    }
}

impl ExportBlock {
    pub fn new(items: Vec<ExportItem>, span: Span) -> Self {
        let commas = vec![false; items.len()];
        Self { items, commas, span }
    }

    pub fn with_commas(items: Vec<ExportItem>, commas: Vec<bool>, span: Span) -> Self {
        Self { items, commas, span }
    }
}

impl ExportItem {
    pub fn own(name: String, rename: Option<String>, span: Span) -> Self {
        ExportItem::Own { name, rename, span }
    }

    pub fn re_export(
        module_alias: String,
        item_name: String,
        item_type: ItemType,
        rename: Option<String>,
        span: Span,
    ) -> Self {
        ExportItem::ReExport {
            module_alias,
            item_name,
            item_type,
            rename,
            span,
        }
    }
}

impl ImportStmt {
    pub fn new(path: ModulePath, alias: String, span: Span) -> Self {
        Self { path, alias, span }
    }
}

impl ModulePath {
    pub fn new(
        components: Vec<String>,
        is_relative: bool,
        parent_levels: usize,
        span: Span,
    ) -> Self {
        Self {
            components,
            is_relative,
            is_absolute: false,
            home_relative: false,
            parent_levels,
            span,
        }
    }

    pub fn new_absolute(components: Vec<String>, home_relative: bool, span: Span) -> Self {
        Self {
            components,
            is_relative: false,
            is_absolute: true,
            home_relative,
            parent_levels: 0,
            span,
        }
    }

    /// True for stdlib imports: a bare path (`is_relative == false && is_absolute == false`,
    /// i.e. no `./`, `../`, `/` or `~/` prefix) whose first component is literally `std`.
    /// Stdlib modules (`std/math`, `std/io`, ...) are synthetic — they are never resolved
    /// against the filesystem, so callers must check this *before* calling [`resolve_from`].
    ///
    /// [`resolve_from`]: ModulePath::resolve_from
    pub fn is_stdlib(&self) -> bool {
        !self.is_relative
            && !self.is_absolute
            && self.components.first().map(|s| s == "std").unwrap_or(false)
    }

    /// Resolves this module path to a `.zy` file path, given the directory that relative
    /// imports (`./foo`, `../foo`, and bare paths) are resolved against — normally the
    /// importing file's parent directory.
    ///
    /// Returns `None` if a `../` chain walks above the root of `base_dir`, or if this path
    /// `is_stdlib()` (stdlib paths have no file to resolve to; check that first).
    ///
    /// This is the single source of truth for module path resolution. It replaces three
    /// previously-divergent copies of the same logic: the tree-walking interpreter
    /// (`zymbol-interpreter/src/modules.rs`), the semantic analyzer
    /// (`zymbol-semantic/src/modules.rs`), and the register-VM compiler
    /// (`zymbol-compiler/src/lib.rs`'s `compile_import`, which used to ignore
    /// `is_absolute`/`home_relative` entirely and always resolve against `base_dir` —
    /// meaning `<# /opt/lib/x => x` resolved to different files under the tree-walker
    /// and the VM).
    pub fn resolve_from(&self, base_dir: &Path) -> Option<PathBuf> {
        if self.is_stdlib() {
            return None;
        }

        let mut resolved = if self.is_absolute {
            if self.home_relative {
                home_dir()
            } else {
                filesystem_root(base_dir)
            }
        } else {
            climb(base_dir, self.parent_levels)?
        };

        for component in &self.components {
            resolved.push(component);
        }
        resolved.set_extension("zy");
        Some(resolved)
    }
}

/// Walk `levels` directories up from `base_dir`, staying relative if it was.
///
/// The obvious `PathBuf::pop` is wrong on a base with nothing left to pop, and
/// that base is the common one: `zymbol run prog.zy` gives the interpreter a
/// file name with no directory in it, whose `parent()` is `""`. `pop()` on `""`
/// returns false, so `<# ../lib/util` failed outright — while the same file run
/// as `zymbol run sub/prog.zy` from one directory up resolved fine. The path
/// spelled on the command line decided whether an import worked (BUG-ZYB-004).
///
/// `./prog.zy` failed differently and worse: `parent()` is `"."`, `pop()` on it
/// succeeds and leaves `""`, so the `../` was silently *consumed* and the
/// module was looked for one directory too low, with a "not found" naming a
/// path the program had never asked for.
///
/// So climbing out of a name that is not a directory name appends `..` rather
/// than popping. Relative bases stay relative — resolving against the current
/// directory is what makes them work at all, and it keeps diagnostics (and the
/// goldens that record them) free of absolute paths that differ per machine.
fn climb(base_dir: &Path, levels: usize) -> Option<PathBuf> {
    use std::path::Component;
    // A leading `.` carries no meaning here and only clutters the result.
    let mut base = if base_dir == Path::new(".") {
        PathBuf::new()
    } else {
        base_dir.to_path_buf()
    };
    for _ in 0..levels {
        match base.components().next_back() {
            // Above the root there is nothing.
            Some(Component::RootDir) | Some(Component::Prefix(_)) => return None,
            // A real directory name: step out of it.
            Some(Component::Normal(_)) => {
                base.pop();
            }
            // `""`, `.` or an existing `..`: keep climbing symbolically.
            _ => base.push(".."),
        }
    }
    Some(base)
}

/// The user's home directory, for `~/mod` imports.
///
/// Windows does not set `HOME`; it uses `USERPROFILE`. Reading only `HOME` there
/// meant `~/mod` silently fell back to `/root` — a path that cannot exist on
/// Windows, so the import failed with a "not found" naming a directory the user
/// never mentioned.
///
/// `HOME` still comes first on every platform: a user who sets it (or a POSIX
/// shell that sets it, as Git Bash does) means it.
fn home_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        return PathBuf::from(home);
    }
    #[cfg(windows)]
    {
        if let Some(profile) = std::env::var_os("USERPROFILE").filter(|p| !p.is_empty()) {
            return PathBuf::from(profile);
        }
        // Last resort before giving up on `~`: the pair NT actually stores.
        if let (Some(drive), Some(path)) = (
            std::env::var_os("HOMEDRIVE").filter(|d| !d.is_empty()),
            std::env::var_os("HOMEPATH").filter(|p| !p.is_empty()),
        ) {
            let mut home = PathBuf::from(drive);
            home.push(PathBuf::from(path));
            return home;
        }
    }
    // Nothing to go on. `~/mod` will not resolve, which is the honest outcome.
    PathBuf::from("/")
}

/// The filesystem root that a leading-`/` import is resolved against.
///
/// On Unix that is `/`. On Windows a path has no meaning without a drive —
/// `PathBuf::from("/")` is root-relative to whichever drive happens to be current —
/// so `/mod` resolves against the root of the drive the importing file is on. That
/// keeps `<# /lib/x` pointing inside the project's own drive rather than wherever
/// the process was started from.
fn filesystem_root(base_dir: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        use std::path::Component;
        let mut components = base_dir.components();
        if let Some(Component::Prefix(prefix)) = components.next() {
            let mut root = PathBuf::from(prefix.as_os_str());
            root.push(std::path::MAIN_SEPARATOR_STR);
            return root;
        }
    }
    let _ = base_dir;
    PathBuf::from("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_span::{FileId, Position, Span};

    fn span() -> Span {
        Span::new(Position::start(), Position::start(), FileId(0))
    }

    /// `<# ../lib/util`, the import in every one of these cases.
    fn parent_import() -> ModulePath {
        ModulePath::new(
            vec!["lib".to_string(), "util".to_string()],
            true,
            1,
            span(),
        )
    }

    /// BUG-ZYB-004 — the base directory comes from the importing file's
    /// `parent()`, and how the file was *spelled* on the command line decides
    /// what that is: `sub/prog.zy` gives `"sub"`, `./prog.zy` gives `"."` and a
    /// bare `prog.zy` gives `""`. All three name the same file, so all three
    /// have to climb to the same place.
    #[test]
    fn parent_import_resolves_the_same_however_the_file_was_spelled() {
        let path = parent_import();

        // A real directory name: step out of it. This one always worked.
        assert_eq!(
            path.resolve_from(Path::new("sub")),
            Some(PathBuf::from("lib/util.zy"))
        );

        // `zymbol run prog.zy` from the program's own directory — the most
        // ordinary way there is to run something. `PathBuf::pop` on `""`
        // returns false, so this used to resolve to nothing at all and the
        // import failed with `module not found: ["lib", "util"]`.
        assert_eq!(
            path.resolve_from(Path::new("")),
            Some(PathBuf::from("../lib/util.zy"))
        );

        // `zymbol run ./prog.zy` — the failure that was worse, because it did
        // not look like one. `pop()` on `"."` succeeds and leaves `""`, so the
        // `../` was silently swallowed and the module was looked for one
        // directory too low.
        assert_eq!(
            path.resolve_from(Path::new(".")),
            Some(PathBuf::from("../lib/util.zy"))
        );

        // And the absolute form, which is unaffected either way.
        assert_eq!(
            path.resolve_from(Path::new("/home/u/rel/sub")),
            Some(PathBuf::from("/home/u/rel/lib/util.zy"))
        );
    }

    /// Climbing more levels than the base has names keeps going symbolically
    /// rather than giving up: `../../x` from a bare file name is two levels up
    /// from the current directory, which is a real place.
    #[test]
    fn climbing_past_the_start_of_a_relative_base_keeps_climbing() {
        let two_up = ModulePath::new(vec!["x".to_string()], true, 2, span());

        assert_eq!(
            two_up.resolve_from(Path::new("")),
            Some(PathBuf::from("../../x.zy"))
        );
        assert_eq!(
            two_up.resolve_from(Path::new("sub")),
            Some(PathBuf::from("../x.zy"))
        );
        assert_eq!(
            two_up.resolve_from(Path::new("a/b")),
            Some(PathBuf::from("x.zy"))
        );
    }

    /// An absolute base still has a top, and `None` is the honest answer there.
    #[test]
    fn climbing_above_the_filesystem_root_fails() {
        let two_up = ModulePath::new(vec!["x".to_string()], true, 2, span());
        assert_eq!(two_up.resolve_from(Path::new("/a")), None);
    }

    /// No `../` at all: the base is used as-is, including the empty one.
    #[test]
    fn a_sibling_import_needs_no_climbing() {
        let sibling = ModulePath::new(vec!["util".to_string()], true, 0, span());

        assert_eq!(
            sibling.resolve_from(Path::new("")),
            Some(PathBuf::from("util.zy"))
        );
        assert_eq!(
            sibling.resolve_from(Path::new("sub")),
            Some(PathBuf::from("sub/util.zy"))
        );
    }
}
