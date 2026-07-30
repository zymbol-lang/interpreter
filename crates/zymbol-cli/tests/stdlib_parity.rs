//! `zymbol_common::stdlib` must describe exactly what the engines implement.
//!
//! Three lists of stdlib names exist, one per role: the tree-walker registers
//! native functions, the compiler maps names to VM builtin ids, and
//! `zymbol_common::stdlib` tells the analyzer and `zymbol check` what `std/`
//! exports. The first two are what runs; the third is what the tooling
//! believes. If the third drifts, the editor either flags working code or
//! stays quiet about a call that cannot work — so it is checked here.
//!
//! This test is the reason adding a stdlib function has to touch all three
//! places: it fails on the first one that is missing.
//!
//! `zymbol-cli` is the only crate that depends on all three, hence the location.

use std::collections::BTreeSet;
use zymbol_common::stdlib;

/// Modules whose availability depends on a Cargo feature, and how to tell
/// whether this build has them.
fn module_is_built(module: &stdlib::StdModule) -> bool {
    if !module.feature_gated {
        return true;
    }
    // std/db is the only gated module; the engines simply do not answer for it
    // when built without the feature.
    zymbol_interpreter::stdlib_registered_names(module.path).is_some()
}

#[test]
fn table_matches_the_tree_walker() {
    for module in stdlib::MODULES {
        if !module_is_built(module) {
            continue;
        }
        let (functions, constants) = zymbol_interpreter::stdlib_registered_names(module.path)
            .unwrap_or_else(|| panic!("{} is in the table but the tree-walker has no such module", module.path));

        let declared: BTreeSet<&str> = module.functions.iter().map(|f| f.name).collect();
        let registered: BTreeSet<&str> = functions.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            declared, registered,
            "{}: function names in zymbol_common::stdlib differ from the tree-walker's",
            module.path
        );

        for (name, arity) in &functions {
            let declared_arity = module
                .function(name)
                .map(|f| f.arity)
                .expect("names were just proven equal");
            assert_eq!(
                declared_arity, *arity,
                "{}::{}: arity in zymbol_common::stdlib differs from the tree-walker's",
                module.path, name
            );
        }

        let declared_consts: BTreeSet<&str> = module.constants.iter().copied().collect();
        let registered_consts: BTreeSet<&str> = constants.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            declared_consts, registered_consts,
            "{}: constant names in zymbol_common::stdlib differ from the tree-walker's",
            module.path
        );
    }
}

#[test]
fn table_matches_the_vm_builtins() {
    for module in stdlib::MODULES {
        if !module_is_built(module) {
            continue;
        }
        let entries = zymbol_compiler::stdlib_builtin_entries(module.path).unwrap_or_else(|| {
            panic!("{} is in the table but the compiler has no builtins for it", module.path)
        });

        let declared: BTreeSet<&str> = module.functions.iter().map(|f| f.name).collect();
        let compiled: BTreeSet<&str> = entries.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            declared, compiled,
            "{}: function names in zymbol_common::stdlib differ from the VM's builtin table",
            module.path
        );
    }
}

#[test]
fn every_engine_module_is_in_the_table() {
    // Anything the compiler knows how to call must be describable by the table,
    // or the tooling has a blind spot for a whole module.
    for path in [
        "std/math",
        "std/random",
        "std/json",
        "std/io",
        "std/net",
        "std/term",
        "std/db",
    ] {
        let in_engine = zymbol_compiler::stdlib_builtin_entries(path).is_some();
        let in_table = stdlib::module(path).is_some();
        if in_engine {
            assert!(in_table, "{path} is callable but missing from zymbol_common::stdlib");
        }
    }
}
