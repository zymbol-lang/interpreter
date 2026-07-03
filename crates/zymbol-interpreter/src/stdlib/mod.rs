//! Standard library registry for Zymbol-Lang.
//!
//! Each submodule exposes a `register()` function that returns a
//! `HashMap<String, Rc<FunctionDef>>` of native functions.
//! `build_module` assembles a full `LoadedModule` from those entries.

use crate::modules::LoadedModule;
use crate::Value;
use std::collections::HashMap;

mod db;
mod io;
mod json;
mod math;
mod net;
mod random;

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
        }),
        "std/json" => Some(LoadedModule {
            name: "std/json".to_string(),
            functions: json::register(),
            all_functions: HashMap::new(),
            constants: HashMap::new(),
            all_variables: HashMap::new(),
            import_aliases: HashMap::new(),
            loaded_modules_refs: HashMap::new(),
        }),
        "std/io" => Some(LoadedModule {
            name: "std/io".to_string(),
            functions: io::register(),
            all_functions: HashMap::new(),
            constants: HashMap::new(),
            all_variables: HashMap::new(),
            import_aliases: HashMap::new(),
            loaded_modules_refs: HashMap::new(),
        }),
        "std/net" => Some(LoadedModule {
            name: "std/net".to_string(),
            functions: net::register(),
            all_functions: HashMap::new(),
            constants: HashMap::new(),
            all_variables: HashMap::new(),
            import_aliases: HashMap::new(),
            loaded_modules_refs: HashMap::new(),
        }),
        "std/db" => Some(LoadedModule {
            name: "std/db".to_string(),
            functions: db::register(),
            all_functions: HashMap::new(),
            constants: HashMap::new(),
            all_variables: HashMap::new(),
            import_aliases: HashMap::new(),
            loaded_modules_refs: HashMap::new(),
        }),
        _ => None,
    }
}
