//! Standard library registry for Zymbol-Lang.
//!
//! Each submodule exposes a `register()` function that returns a
//! `HashMap<String, Rc<FunctionDef>>` of native functions.
//! `build_module` assembles a full `LoadedModule` from those entries.

use crate::modules::LoadedModule;
use crate::Value;
use std::collections::HashMap;

#[cfg(feature = "db")]
mod db;
mod io;
mod json;
mod math;
mod net;
mod random;
mod term;

/// Build a `LoadedModule` for the requested stdlib path.
/// Returns `None` if the path is not a recognized stdlib module.
pub(crate) fn build_module(name: &str) -> Option<LoadedModule> {
    match name {
        "std/math" => {
            let mut module = LoadedModule {
                name: "std/math".to_string(),
                functions: math::register(),
                all_functions: HashMap::new(),
                constants: HashMap::new(),
                all_variables: HashMap::new(),
                import_aliases: HashMap::new(),
                loaded_modules_refs: HashMap::new(),
            const_names: std::collections::HashSet::new(),
            };
            module.constants.insert("PI".into(), Value::Float(std::f64::consts::PI));
            module.constants.insert("E".into(),  Value::Float(std::f64::consts::E));
            Some(module)
        }
        "std/random" => Some(LoadedModule {
            name: "std/random".to_string(),
            functions: random::register(),
            all_functions: HashMap::new(),
            constants: HashMap::new(),
            all_variables: HashMap::new(),
            import_aliases: HashMap::new(),
            loaded_modules_refs: HashMap::new(),
            const_names: std::collections::HashSet::new(),
        }),
        "std/term" => Some(LoadedModule {
            name: "std/term".to_string(),
            functions: term::register(),
            all_functions: HashMap::new(),
            constants: HashMap::new(),
            all_variables: HashMap::new(),
            import_aliases: HashMap::new(),
            loaded_modules_refs: HashMap::new(),
            const_names: std::collections::HashSet::new(),
        }),
        "std/json" => Some(LoadedModule {
            name: "std/json".to_string(),
            functions: json::register(),
            all_functions: HashMap::new(),
            constants: HashMap::new(),
            all_variables: HashMap::new(),
            import_aliases: HashMap::new(),
            loaded_modules_refs: HashMap::new(),
            const_names: std::collections::HashSet::new(),
        }),
        "std/io" => Some(LoadedModule {
            name: "std/io".to_string(),
            functions: io::register(),
            all_functions: HashMap::new(),
            constants: HashMap::new(),
            all_variables: HashMap::new(),
            import_aliases: HashMap::new(),
            loaded_modules_refs: HashMap::new(),
            const_names: std::collections::HashSet::new(),
        }),
        "std/net" => Some(LoadedModule {
            name: "std/net".to_string(),
            functions: net::register(),
            all_functions: HashMap::new(),
            constants: HashMap::new(),
            all_variables: HashMap::new(),
            import_aliases: HashMap::new(),
            loaded_modules_refs: HashMap::new(),
            const_names: std::collections::HashSet::new(),
        }),
        // Without the `db` feature this falls through to `None` → the standard
        // module-not-found error (prebuilt static binaries exclude std/db).
        #[cfg(feature = "db")]
        "std/db" => Some(LoadedModule {
            name: "std/db".to_string(),
            functions: db::register(),
            all_functions: HashMap::new(),
            constants: HashMap::new(),
            all_variables: HashMap::new(),
            import_aliases: HashMap::new(),
            loaded_modules_refs: HashMap::new(),
            const_names: std::collections::HashSet::new(),
        }),
        _ => None,
    }
}

/// What a stdlib module registers: `(name, arity)` per function — arity `-1`
/// means variadic — plus its constant names. `None` if the path is not a
/// stdlib module in this build.
///
/// Exists so `zymbol_common::stdlib` — the export table the analyzer and
/// `zymbol check` read — can be tested against what the tree-walker really
/// registers, instead of the two drifting apart silently.
pub fn registered_names(path: &str) -> Option<(Vec<(String, i32)>, Vec<String>)> {
    let module = build_module(path)?;
    let mut functions: Vec<(String, i32)> = module
        .functions
        .iter()
        .map(|(name, def)| {
            let arity: i32 = match def.as_ref() {
                crate::FunctionDef::Native { arity, .. } => i32::from(*arity),
                _ => 0,
            };
            (name.clone(), arity)
        })
        .collect();
    let mut constants: Vec<String> = module.constants.keys().cloned().collect();
    functions.sort();
    constants.sort();
    Some((functions, constants))
}
