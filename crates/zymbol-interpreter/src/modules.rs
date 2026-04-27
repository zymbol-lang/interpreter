//! Module system runtime for Zymbol-Lang
//!
//! Handles runtime module loading and resolution:
//! - Module loading from files (.zy extension)
//! - Path resolution (./relative, ../parent, absolute)
//! - Import processing and alias registration
//! - Export table extraction

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use zymbol_lexer::{Lexer};
use zymbol_parser::Parser;
use zymbol_span::FileId;

use crate::{FunctionDef, Interpreter, Result, RuntimeError, Value};

/// Loaded module with its exported items
#[derive(Debug, Clone)]
pub(crate) struct LoadedModule {
    /// Module name
    #[allow(dead_code)]
    pub(crate) name: String,
    /// Exported functions only (for external callers via alias::fn)
    pub(crate) functions: HashMap<String, Rc<FunctionDef>>,
    /// ALL module functions: exported + private (for intra-module calls — BUG-01)
    pub(crate) all_functions: HashMap<String, Rc<FunctionDef>>,
    /// Exported constants/variables (for external access via module.CONSTANT)
    pub(crate) constants: HashMap<String, Value>,
    /// All module variables (for function execution context - includes private variables)
    pub(crate) all_variables: HashMap<String, Value>,
    /// Module's import aliases (for function execution context)
    pub(crate) import_aliases: HashMap<String, PathBuf>,
    /// Module's loaded modules (for function execution context)
    #[allow(dead_code)]
    pub(crate) loaded_modules_refs: HashMap<PathBuf, ()>, // Just to track dependencies
}

impl<W: Write> Interpreter<W> {
    /// Load an import statement and register the module alias
    pub(crate) fn load_import(&mut self, import: &zymbol_ast::ImportStmt) -> Result<()> {
        // Resolve the module path
        let module_path = self.resolve_module_path(&import.path)?;

        // Circular import detection: if the module is currently being loaded, there is a cycle
        if self.loading_modules.contains(&module_path) {
            let cycle_name = module_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            return Err(RuntimeError::CircularImport { module: cycle_name });
        }

        // Load the module if not already loaded
        if !self.loaded_modules.contains_key(&module_path) {
            self.loading_modules.insert(module_path.clone());
            let result = self.load_module(&module_path);
            self.loading_modules.remove(&module_path);
            result?;
        }

        // Register the import alias
        self.import_aliases
            .insert(import.alias.clone(), module_path);

        Ok(())
    }

    /// Resolve a module path to an absolute file path
    pub(crate) fn resolve_module_path(&self, module_path: &zymbol_ast::ModulePath) -> Result<PathBuf> {
        let mut resolved = if module_path.is_absolute {
            // Absolute path: /foo/bar or ~/foo/bar
            if module_path.home_relative {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
                PathBuf::from(home)
            } else {
                PathBuf::from("/")
            }
        } else {
            // Relative path: ./foo or ../foo — start from current file's directory
            let current_dir = self
                .current_file
                .as_ref()
                .and_then(|p| p.parent())
                .unwrap_or(&self.base_dir);
            let mut base = current_dir.to_path_buf();
            for _ in 0..module_path.parent_levels {
                if !base.pop() {
                    return Err(RuntimeError::ModuleNotFound {
                        path: format!("{:?}", module_path.components),
                    });
                }
            }
            base
        };

        // Add path components
        for component in &module_path.components {
            resolved.push(component);
        }

        // Add .zy extension (Zymbol-Lang standard)
        resolved.set_extension("zy");

        Ok(resolved)
    }

    /// Load a module from file
    pub(crate) fn load_module(&mut self, file_path: &Path) -> Result<()> {
        // Check if file exists
        if !file_path.exists() {
            return Err(RuntimeError::ModuleNotFound {
                path: file_path.to_string_lossy().to_string(),
            });
        }

        // Read the file
        let source = std::fs::read_to_string(file_path).map_err(RuntimeError::Io)?;

        // Parse the module
        let lexer = Lexer::new(&source, FileId(0));
        let (tokens, lex_diagnostics) = lexer.tokenize();

        if !lex_diagnostics.is_empty() {
            let detail: Vec<String> = lex_diagnostics.iter().map(|d| {
                let loc = d.span
                    .map(|s| format!("{}:{}:{}", file_path.display(), s.start.line, s.start.column))
                    .unwrap_or_else(|| file_path.display().to_string());
                let mut msg = format!("  {}: {}", loc, d.message);
                if let Some(help) = &d.help {
                    msg.push_str(&format!("\n    help: {}", help));
                }
                msg
            }).collect();
            return Err(RuntimeError::ParseError(format!(
                "{} lexer error(s) in '{}'\n{}",
                lex_diagnostics.len(),
                file_path.display(),
                detail.join("\n")
            )));
        }

        let parser = Parser::new(tokens);
        let program = parser.parse().map_err(|errors| {
            let detail: Vec<String> = errors.iter().map(|d| {
                let loc = d.span
                    .map(|s| format!("{}:{}:{}", file_path.display(), s.start.line, s.start.column))
                    .unwrap_or_else(|| file_path.display().to_string());
                let mut msg = format!("  {}: {}", loc, d.message);
                if let Some(help) = &d.help {
                    msg.push_str(&format!("\n    help: {}", help));
                }
                msg
            }).collect();
            RuntimeError::ParseError(format!(
                "{} parse error(s) in '{}'\n{}",
                errors.len(),
                file_path.display(),
                detail.join("\n")
            ))
        })?;

        // Create a new interpreter for the module with a buffer to capture output
        let mut module_interp = Interpreter::with_output(Vec::new());
        module_interp.set_current_file(file_path);
        module_interp.set_base_dir(&self.base_dir);
        // Propagate the in-flight loading set so nested modules inherit cycle detection state
        module_interp.loading_modules = self.loading_modules.clone();

        // Execute the module (this will process its imports and statements)
        module_interp.execute(&program)?;

        // Store ALL module variables for function execution context
        let all_module_variables = module_interp.get_all_variables();
        // Store module's import context
        let module_import_aliases = module_interp.import_aliases.clone();
        let module_loaded_refs: HashMap<PathBuf, ()> = module_interp.loaded_modules.keys()
            .map(|path| (path.clone(), ()))
            .collect();

        // Extract exported items based on export block
        // Capture all functions before module_interp is consumed (for BUG-01 intra-module calls)
        let all_module_functions = module_interp.functions.clone();

        let mut loaded_module = LoadedModule {
            name: program
                .module_decl
                .as_ref()
                .map(|m| m.name.clone())
                .unwrap_or_else(|| {
                    file_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string()
                }),
            functions: HashMap::new(),
            all_functions: all_module_functions,
            constants: HashMap::new(),
            all_variables: all_module_variables,
            import_aliases: module_import_aliases,
            loaded_modules_refs: module_loaded_refs,
        };

        // If there's an export block, only export listed items
        if let Some(ref module_decl) = program.module_decl {
            if let Some(ref export_block) = module_decl.export_block {
                for export_item in &export_block.items {
                    match export_item {
                        zymbol_ast::ExportItem::Own { name, rename, .. } => {
                            let public_name = rename.as_ref().unwrap_or(name).clone();
                            // Export own function or := constant under public name.
                            // Mutable module variables (declared with `=`) are private — they
                            // cannot be exported directly and are silently skipped here.
                            // They remain accessible only via exported getter/setter functions.
                            if let Some(func) = module_interp.functions.get(name) {
                                loaded_module.functions.insert(public_name, func.clone());
                            } else if module_interp.is_const(name) {
                                if let Some(val) = module_interp.get_variable(name) {
                                    loaded_module.constants.insert(public_name, val.clone());
                                }
                            }
                            // else: mutable variable — silently excluded from exports
                        }
                        zymbol_ast::ExportItem::ReExport {
                            module_alias,
                            item_name,
                            rename,
                            ..
                        } => {
                            // Re-export from imported module
                            let export_name = rename.as_ref().unwrap_or(item_name);

                            // Get the imported module
                            if let Some(imported_path) = module_interp.import_aliases.get(module_alias)
                            {
                                if let Some(imported_module) =
                                    module_interp.loaded_modules.get(imported_path)
                                {
                                    // Re-export function
                                    if let Some(func) = imported_module.functions.get(item_name) {
                                        loaded_module
                                            .functions
                                            .insert(export_name.clone(), func.clone());
                                    }
                                    // Re-export constant
                                    else if let Some(val) =
                                        imported_module.constants.get(item_name)
                                    {
                                        loaded_module
                                            .constants
                                            .insert(export_name.clone(), val.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                // No export block - export everything
                loaded_module.functions = module_interp.functions.clone();
                loaded_module.constants = module_interp.get_all_variables();
            }
        } else {
            // No module declaration - export everything
            loaded_module.functions = module_interp.functions.clone();
            loaded_module.constants = module_interp.get_all_variables();
        }

        // Copy all modules loaded by this module to the global context
        // This ensures that dependencies are available when module functions are called
        for (dep_path, dep_module) in module_interp.loaded_modules {
            self.loaded_modules.entry(dep_path).or_insert(dep_module);
        }

        // Store the loaded module
        self.loaded_modules
            .insert(file_path.to_path_buf(), loaded_module);

        Ok(())
    }
}
