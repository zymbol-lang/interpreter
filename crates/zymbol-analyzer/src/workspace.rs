//! Workspace management for Zymbol-Lang LSP
//!
//! Provides functionality for:
//! - Scanning workspace directories for .zy files
//! - Resolving import paths relative to files
//! - Managing multiple workspace roots

use dashmap::DashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use walkdir::WalkDir;

/// Information about a module file in the workspace
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    /// Absolute path to the file
    pub path: PathBuf,
    /// URI representation (file://)
    pub uri: Arc<str>,
    /// Module name extracted from `# module_name` declaration (if present)
    pub module_name: Option<String>,
    /// Last modification time
    pub modified: SystemTime,
}

impl ModuleInfo {
    /// Create a new ModuleInfo from a path
    pub fn from_path(path: PathBuf) -> std::io::Result<Self> {
        let metadata = std::fs::metadata(&path)?;
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let uri = path_to_uri(&path);

        Ok(Self {
            path,
            uri,
            module_name: None,
            modified,
        })
    }

    /// Update the module name from parsed content
    pub fn with_module_name(mut self, name: Option<String>) -> Self {
        self.module_name = name;
        self
    }
}

/// Workspace manager for discovering and tracking .zy files
#[derive(Debug, Default)]
pub struct Workspace {
    /// Workspace root directories
    roots: Vec<PathBuf>,
    /// All discovered modules by their absolute path
    modules: DashMap<PathBuf, ModuleInfo>,
}

impl Workspace {
    /// Create a new empty workspace
    pub fn new() -> Self {
        Self {
            roots: Vec::new(),
            modules: DashMap::new(),
        }
    }

    /// Create a workspace with initial roots
    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        let workspace = Self {
            roots,
            modules: DashMap::new(),
        };
        workspace.scan();
        workspace
    }

    /// Add a workspace root directory
    pub fn add_root(&mut self, path: PathBuf) {
        if !self.roots.contains(&path) {
            self.roots.push(path.clone());
            self.scan_directory(&path);
        }
    }

    /// Remove a workspace root directory
    pub fn remove_root(&mut self, path: &Path) {
        if let Some(pos) = self.roots.iter().position(|r| r == path) {
            self.roots.remove(pos);
            // Remove modules that were under this root
            self.modules.retain(|p, _| !p.starts_with(path));
        }
    }

    /// Get all workspace roots
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Scan all workspace roots for .zy files
    pub fn scan(&self) {
        for root in &self.roots {
            self.scan_directory(root);
        }
    }

    /// Scan a specific directory for .zy files
    fn scan_directory(&self, dir: &Path) {
        if !dir.exists() || !dir.is_dir() {
            return;
        }

        for entry in WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Only process .zy files
            if path.extension().is_some_and(|ext| ext == "zy") {
                if let Ok(info) = ModuleInfo::from_path(path.to_path_buf()) {
                    self.modules.insert(path.to_path_buf(), info);
                }
            }
        }
    }

    /// Add or update a module in the workspace
    pub fn add_module(&self, path: PathBuf) {
        if let Ok(info) = ModuleInfo::from_path(path.clone()) {
            self.modules.insert(path, info);
        }
    }

    /// Remove a module from the workspace
    pub fn remove_module(&self, path: &Path) {
        self.modules.remove(path);
    }

    /// Update module info (e.g., after parsing to extract module name)
    pub fn update_module_name(&self, path: &Path, module_name: Option<String>) {
        if let Some(mut info) = self.modules.get_mut(path) {
            info.module_name = module_name;
        }
    }

    /// Get module info by path
    pub fn get_module(&self, path: &Path) -> Option<ModuleInfo> {
        self.modules.get(path).map(|r| r.clone())
    }

    /// Get module info by URI
    pub fn get_module_by_uri(&self, uri: &str) -> Option<ModuleInfo> {
        self.modules
            .iter()
            .find(|entry| entry.uri.as_ref() == uri)
            .map(|entry| entry.clone())
    }

    /// Check if a path is in the workspace
    pub fn contains(&self, path: &Path) -> bool {
        self.modules.contains_key(path)
    }

    /// Get all modules in the workspace
    pub fn all_modules(&self) -> Vec<ModuleInfo> {
        self.modules.iter().map(|entry| entry.clone()).collect()
    }

    /// Get module count
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// Resolve an import path relative to a source file
    ///
    /// Handles:
    /// - `./module` - current directory
    /// - `../module` - parent directory
    /// - Nested paths like `./lib/math`
    pub fn resolve_import(&self, import_path: &str, from_file: &Path) -> Option<PathBuf> {
        let from_dir = from_file.parent()?;

        // Parse the import path
        let mut resolved = from_dir.to_path_buf();
        let path_parts: Vec<&str> = import_path.split('/').collect();

        for (i, part) in path_parts.iter().enumerate() {
            match *part {
                "." => {
                    // Current directory - no change
                }
                ".." => {
                    // Parent directory
                    if !resolved.pop() {
                        return None; // Can't go above root
                    }
                }
                name => {
                    // Regular path component
                    if i == path_parts.len() - 1 {
                        // Last component - this is the module name
                        resolved.push(format!("{}.zy", name));
                    } else {
                        // Intermediate directory
                        resolved.push(name);
                    }
                }
            }
        }

        // Canonicalize to resolve symlinks and normalize
        resolved.canonicalize().ok().or_else(|| {
            // If file doesn't exist, return the computed path anyway
            if resolved.exists() {
                Some(resolved)
            } else {
                // Construct what the path should be
                Some(resolved)
            }
        })
    }

    /// Find module by name across all roots
    pub fn find_module_by_name(&self, name: &str) -> Option<ModuleInfo> {
        self.modules
            .iter()
            .find(|entry| {
                entry.module_name.as_deref() == Some(name)
                    || (entry
                        .path
                        .file_stem()
                        .and_then(|s| s.to_str()) == Some(name))
            })
            .map(|entry| entry.clone())
    }

    /// Check if a path is within any workspace root
    pub fn is_in_workspace(&self, path: &Path) -> bool {
        self.roots.iter().any(|root| path.starts_with(root))
    }
}

/// Convert a file path to a `file://` URI.
///
/// Goes through `Url::from_file_path`, which is the only thing that gets a Windows
/// path right: `D:\dir\x.zy` has to become `file:///D:/dir/x.zy`, with the drive
/// after three slashes and the separators flipped. Formatting `file://{path}` by
/// hand produced `file://D:\dir\x.zy`, which is not a URI VS Code will ever match
/// against the document it has open — so diagnostics published under it landed
/// nowhere.
pub fn path_to_uri(path: &Path) -> Arc<str> {
    if let Ok(url) = lsp_types::Url::from_file_path(path) {
        return Arc::from(url.as_str());
    }
    // `from_file_path` only rejects relative paths. Anchor it and try again; a URI
    // is absolute by definition, so there is nothing else it could mean.
    if let Ok(abs) = std::path::absolute(path) {
        if let Ok(url) = lsp_types::Url::from_file_path(&abs) {
            return Arc::from(url.as_str());
        }
    }
    Arc::from(format!("file://{}", path.display()).as_str())
}

/// Convert a `file://` URI to a path.
///
/// Also accepts a bare filesystem path, because several callers hold a string that
/// is a URI when it came from the editor and a path when it came from module
/// resolution.
///
/// The parsing is `Url`'s, not ours. Stripping `file://` by hand left the leading
/// slash of `file:///D:/dir/x.zy` in place, yielding `/D:/dir/x.zy` — which Windows
/// reads as a *relative* path, since an absolute one must start at the drive. Every
/// import in the file was then looked for under a directory that does not exist,
/// which is where the false `E002: Module ... not found` diagnostics came from.
/// Percent-decoding comes along for free, so the BUG-003 Unicode-directory fix is
/// preserved without a hand-written decoder.
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    match parse_uri(uri) {
        Some(url) if url.scheme() == "file" => url.to_file_path().ok(),
        // A URI in any other scheme names no file on this disk.
        Some(_) => None,
        None => Some(PathBuf::from(uri)),
    }
}

/// Parse a string that is either a `file://` URI or a filesystem path into a `Url`.
///
/// The `if starts_with("file://") { parse } else { format!("file://{}") }` dance was
/// repeated at half a dozen call sites, and the `else` branch carried the same
/// Windows bug as the old `path_to_uri`.
pub fn uri_str_to_url(uri: &str) -> Option<lsp_types::Url> {
    match parse_uri(uri) {
        Some(url) => Some(url),
        None => lsp_types::Url::parse(&path_to_uri(Path::new(uri))).ok(),
    }
}

/// Parse `s` as a URI, or `None` if it is a filesystem path.
///
/// The `None` case is not just "`Url::parse` failed". On Windows a path starting
/// with a drive letter parses *successfully* as a URI, because `D:` is a
/// syntactically valid scheme — `D:\proyecto\main.zy` comes back as a `d:` URL with
/// an opaque body. Anything treating that as a real URI concludes it does not name a
/// local file, which is wrong in the one place it matters.
///
/// So a single-letter scheme is read as a drive letter. No scheme anyone uses is one
/// character long, and on Unix nothing is lost: a path there never looks like `X:`.
fn parse_uri(s: &str) -> Option<lsp_types::Url> {
    let bytes = s.as_bytes();
    let is_drive_path = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if is_drive_path {
        return None;
    }
    lsp_types::Url::parse(s).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::TempDir;

    fn setup_test_workspace() -> (TempDir, Workspace) {
        let temp = TempDir::new().unwrap();

        // Create test files
        let lib_dir = temp.path().join("lib");
        fs::create_dir(&lib_dir).unwrap();

        File::create(temp.path().join("main.zy")).unwrap();
        File::create(lib_dir.join("math.zy")).unwrap();
        File::create(lib_dir.join("utils.zy")).unwrap();

        let workspace = Workspace::with_roots(vec![temp.path().to_path_buf()]);

        (temp, workspace)
    }

    #[test]
    fn test_workspace_scan() {
        let (_temp, workspace) = setup_test_workspace();

        assert_eq!(workspace.module_count(), 3);
    }

    #[test]
    fn test_resolve_import_current_dir() {
        let (_temp, workspace) = setup_test_workspace();
        let main_path = _temp.path().join("main.zy");

        let resolved = workspace.resolve_import("./lib/math", &main_path);
        assert!(resolved.is_some());

        let resolved_path = resolved.unwrap();
        assert!(resolved_path.ends_with("lib/math.zy"));
    }

    #[test]
    fn test_resolve_import_parent_dir() {
        let (_temp, workspace) = setup_test_workspace();
        let math_path = _temp.path().join("lib").join("math.zy");

        let resolved = workspace.resolve_import("../main", &math_path);
        assert!(resolved.is_some());

        let resolved_path = resolved.unwrap();
        assert!(resolved_path.ends_with("main.zy"));
    }

    /// An absolute path that exists as such on the platform the test runs on.
    /// The URI helpers are the one place where a Unix-shaped literal is not a
    /// portable stand-in: `/home/user/x.zy` is not absolute on Windows, so tests
    /// written that way passed on Linux while asserting nothing about Windows —
    /// which is how W-2 survived to a release.
    fn abs(rest: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(format!(r"D:\{}", rest.replace('/', r"\")))
        } else {
            PathBuf::from(format!("/{}", rest))
        }
    }

    #[test]
    fn test_path_to_uri() {
        let uri = path_to_uri(&abs("home/user/project/main.zy"));

        if cfg!(windows) {
            // The drive goes *after* three slashes, and separators become forward
            // slashes. `file://D:\...` — what the old hand-rolled version produced —
            // is not a URI any editor will match.
            assert_eq!(uri.as_ref(), "file:///D:/home/user/project/main.zy");
        } else {
            assert_eq!(uri.as_ref(), "file:///home/user/project/main.zy");
        }
    }

    #[test]
    fn test_uri_to_path() {
        let expected = abs("home/user/project/main.zy");
        let uri = path_to_uri(&expected);

        assert_eq!(uri_to_path(&uri).unwrap(), expected);
    }

    #[test]
    fn test_uri_to_path_round_trips_spaces() {
        // The reported failure was on `D:\OneDrive - Abastible S.A\...`: spaces get
        // percent-encoded on the way out and must survive the way back.
        let expected = abs("OneDrive - Abastible S.A/Documentos/serpiente.zy");
        let uri = path_to_uri(&expected);

        assert!(uri.contains("%20"), "spaces should be encoded: {}", uri);
        assert_eq!(uri_to_path(&uri).unwrap(), expected);
    }

    #[test]
    #[cfg(windows)]
    fn test_uri_to_path_windows_drive_letter() {
        // Exactly what VS Code sends. The old code stripped only `file://`, leaving
        // `/D:/...` — a *relative* path on Windows, so every import under it was
        // looked for in a directory that does not exist (the false E002s).
        let uri = "file:///D:/OneDrive%20-%20Abastible%20S.A/Documentos/juego.zy";
        let path = uri_to_path(uri).unwrap();

        assert_eq!(
            path,
            PathBuf::from(r"D:\OneDrive - Abastible S.A\Documentos\juego.zy")
        );
        assert!(path.is_absolute(), "a drive-letter path must be absolute");
    }

    #[test]
    #[cfg(windows)]
    fn test_uri_to_path_windows_lowercase_drive() {
        // Some clients lowercase the drive and encode the colon.
        let path = uri_to_path("file:///d%3A/proyecto/main.zy").unwrap();
        assert_eq!(path, PathBuf::from(r"d:\proyecto\main.zy"));
        assert!(path.is_absolute());
    }

    #[test]
    fn test_uri_to_path_unicode_encoded() {
        // BUG-003: VS Code sends percent-encoded URIs for directories with Unicode
        // names. 源码 encodes as %E6%BA%90%E7%A0%81 in UTF-8 percent-encoding.
        let uri = if cfg!(windows) {
            "file:///D:/user/%E6%BA%90%E7%A0%81/mod.zy"
        } else {
            "file:///home/user/%E6%BA%90%E7%A0%81/mod.zy"
        };
        assert_eq!(uri_to_path(uri).unwrap(), abs("user/源码/mod.zy"));
    }

    #[test]
    fn test_uri_to_path_unicode_plain() {
        // URIs with unencoded Unicode must also work.
        let uri = if cfg!(windows) {
            "file:///D:/user/源码/mod.zy"
        } else {
            "file:///home/user/源码/mod.zy"
        };
        assert_eq!(uri_to_path(uri).unwrap(), abs("user/源码/mod.zy"));
    }

    #[test]
    fn test_uri_to_path_accepts_bare_path() {
        // Several callers hold a string that is a URI when it came from the editor
        // and a plain path when it came from module resolution.
        let path = abs("proyecto/main.zy");
        assert_eq!(uri_to_path(&path.to_string_lossy()).unwrap(), path);
    }

    #[test]
    fn test_drive_letter_is_a_path_not_a_scheme() {
        // `Url::parse("D:\\x.zy")` succeeds — `D:` is a syntactically valid scheme —
        // so a bare Windows path must be recognised before parsing, or it comes back
        // as a `d:` URL naming no local file.
        assert!(parse_uri(r"D:\proyecto\main.zy").is_none());
        assert!(parse_uri("D:/proyecto/main.zy").is_none());
        assert!(parse_uri("file:///D:/proyecto/main.zy").is_some());
        // Two-letter schemes are real and must still parse.
        assert!(parse_uri("ab:whatever").is_some());
    }

    #[test]
    fn test_uri_to_path_rejects_other_schemes() {
        // An `untitled:` or `http:` document names no file on this disk.
        assert!(uri_to_path("untitled:Untitled-1").is_none());
        assert!(uri_to_path("https://example.com/main.zy").is_none());
    }

    #[test]
    fn test_uri_str_to_url_takes_uris_and_paths() {
        let path = abs("proyecto/main.zy");
        let from_uri = uri_str_to_url(&path_to_uri(&path)).unwrap();
        let from_path = uri_str_to_url(&path.to_string_lossy()).unwrap();

        assert_eq!(from_uri, from_path);
        assert_eq!(from_uri.to_file_path().unwrap(), path);
    }

    #[test]
    fn test_get_module_by_uri() {
        let (_temp, workspace) = setup_test_workspace();

        let main_uri = path_to_uri(&_temp.path().join("main.zy"));
        let module = workspace.get_module_by_uri(&main_uri);

        assert!(module.is_some());
    }

    #[test]
    fn test_add_remove_root() {
        let temp = TempDir::new().unwrap();
        let mut workspace = Workspace::new();

        File::create(temp.path().join("test.zy")).unwrap();

        workspace.add_root(temp.path().to_path_buf());
        assert_eq!(workspace.module_count(), 1);

        workspace.remove_root(temp.path());
        assert_eq!(workspace.module_count(), 0);
    }
}
