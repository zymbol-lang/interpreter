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
    /// Exported functions
    pub(crate) functions: HashMap<String, Rc<FunctionDef>>,
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

        // Load the module if not already loaded
        if !self.loaded_modules.contains_key(&module_path) {
            self.load_module(&module_path)?;
        }

        // Register the import alias
        self.import_aliases
            .insert(import.alias.clone(), module_path);

        Ok(())
    }

    /// Resolve a module path to an absolute file path
    pub(crate) fn resolve_module_path(&self, module_path: &zymbol_ast::ModulePath) -> Result<PathBuf> {
        let current_dir = self
            .current_file
            .as_ref()
            .and_then(|p| p.parent())
            .unwrap_or(&self.base_dir);

        let mut resolved = current_dir.to_path_buf();

        // Handle parent directory navigation
        if module_path.is_relative {
            for _ in 0..module_path.parent_levels {
                if !resolved.pop() {
                    return Err(RuntimeError::ModuleNotFound {
                        path: format!("{:?}", module_path.components),
                    });
                }
            }
        }

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
            return Err(RuntimeError::ParseError(format!(
                "{} lexer errors in module",
                lex_diagnostics.len()
            )));
        }

        let parser = Parser::new(tokens);
        let program = parser.parse().map_err(|errors| {
            let error_msgs: Vec<String> = errors.iter()
                .map(|e| format!("{:?}", e))
                .collect();
            RuntimeError::ParseError(format!(
                "{} parser errors in module:\n{}",
                errors.len(),
                error_msgs.join("\n")
            ))
        })?;

        // Create a new interpreter for the module with a buffer to capture output
        let mut module_interp = Interpreter::with_output(Vec::new());
        module_interp.set_current_file(file_path);
        module_interp.set_base_dir(&self.base_dir);

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
                        zymbol_ast::ExportItem::Own { name, .. } => {
                            // Export own function or constant
                            if let Some(func) = module_interp.functions.get(name) {
                                loaded_module.functions.insert(name.clone(), func.clone());
                            } else if let Some(val) = module_interp.get_variable(name) {
                                loaded_module.constants.insert(name.clone(), val.clone());
                            }
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
