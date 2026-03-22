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

/// Convert a file path to a file:// URI
pub fn path_to_uri(path: &Path) -> Arc<str> {
    let uri = format!("file://{}", path.display());
    Arc::from(uri)
}

/// Convert a file:// URI to a path
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    uri.strip_prefix("file://").map(PathBuf::from)
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

    #[test]
    fn test_path_to_uri() {
        let path = PathBuf::from("/home/user/project/main.zy");
        let uri = path_to_uri(&path);

        assert_eq!(uri.as_ref(), "file:///home/user/project/main.zy");
    }

    #[test]
    fn test_uri_to_path() {
        let uri = "file:///home/user/project/main.zy";
        let path = uri_to_path(uri);

        assert!(path.is_some());
        assert_eq!(path.unwrap(), PathBuf::from("/home/user/project/main.zy"));
    }

    #[test]
    fn test_get_module_by_uri() {
        let (_temp, workspace) = setup_test_workspace();

        let main_uri = format!("file://{}", _temp.path().join("main.zy").display());
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
