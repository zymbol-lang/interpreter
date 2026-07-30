//! What the standard library exports — the one list every tool reads.
//!
//! `std/` modules have no file on disk: the tree-walker registers native
//! functions (`zymbol-interpreter/src/stdlib/`) and the VM maps names to
//! builtin ids (`zymbol-compiler::stdlib_builtin_entries`). Neither list is
//! reachable from the analyzer, so the editor and `zymbol check` used to know
//! nothing about `std/`: a call to a function that does not exist —
//! `math::inventada(2.0)` — passed every static check and failed at run time.
//!
//! This table is the shared answer to "what does `std/x` export". It lives in
//! `zymbol-common` because every crate already depends on it. It is kept in
//! step with the two engines by `tests/stdlib_parity.rs` in `zymbol-cli`,
//! which fails if a name is added to one and not the others.
//!
//! Adding a stdlib function means touching three places: the engine that
//! implements it, the compiler's builtin table, and this list.

/// A function a `std/` module exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdFunction {
    /// Name as written after `::`
    pub name: &'static str,
    /// Argument count; `-1` when the function is variadic
    pub arity: i32,
}

/// A `std/` module and everything it exports.
#[derive(Debug, Clone, Copy)]
pub struct StdModule {
    /// Import path, e.g. `std/math`
    pub path: &'static str,
    /// Functions, called with `::`
    pub functions: &'static [StdFunction],
    /// Constants, read with `.`
    pub constants: &'static [&'static str],
    /// Whether the module is only present in builds with the `db` feature
    pub feature_gated: bool,
}

impl StdModule {
    /// The named function, if this module has it.
    pub fn function(&self, name: &str) -> Option<&'static StdFunction> {
        self.functions.iter().find(|f| f.name == name)
    }

    /// Whether this module exports `name` as a constant.
    pub fn has_constant(&self, name: &str) -> bool {
        self.constants.contains(&name)
    }
}

const fn f(name: &'static str, arity: i32) -> StdFunction {
    StdFunction { name, arity }
}

/// Every `std/` module, in import-path order.
pub const MODULES: &[StdModule] = &[
    StdModule {
        path: "std/math",
        functions: &[
            f("sqrt", 1),
            f("exp", 1),
            f("ln", 1),
            f("log", -1),
            f("pow", 2),
            f("sin", 1),
            f("cos", 1),
            f("tan", 1),
            f("asin", 1),
            f("acos", 1),
            f("atan", 1),
            f("atan2", 2),
            f("tanh", 1),
            f("sinh", 1),
            f("cosh", 1),
            f("sigmoid", 1),
            f("abs", 1),
            f("max", 2),
            f("min", 2),
            f("floor", 1),
            f("ceil", 1),
            f("round", 1),
        ],
        constants: &["PI", "E"],
        feature_gated: false,
    },
    StdModule {
        path: "std/random",
        functions: &[f("entero", 2), f("rango", 1), f("peso_f64", 0)],
        constants: &[],
        feature_gated: false,
    },
    StdModule {
        path: "std/json",
        functions: &[f("decode", 1), f("decode_map", 2), f("encode", 1)],
        constants: &[],
        feature_gated: false,
    },
    StdModule {
        path: "std/io",
        functions: &[
            f("read", 1),
            f("write", 2),
            f("append", 2),
            f("exists", 1),
            f("delete", 1),
            f("list", 1),
            f("mkdir", 1),
        ],
        constants: &[],
        feature_gated: false,
    },
    StdModule {
        path: "std/net",
        functions: &[f("get", -1), f("post", -1), f("post_json", -1), f("head", 1)],
        constants: &[],
        feature_gated: false,
    },
    StdModule {
        path: "std/term",
        functions: &[
            f("width", 1),
            f("pad_left", 2),
            f("pad_right", 2),
            f("center", 2),
            f("truncate", 2),
        ],
        constants: &[],
        feature_gated: false,
    },
    StdModule {
        path: "std/db",
        functions: &[
            f("connect", 2),
            f("disconnect", 1),
            f("exec", -1),
            f("query", -1),
            f("query_one", -1),
            f("query_value", -1),
            f("tx", 2),
            f("begin", 1),
            f("commit", 1),
            f("rollback", 1),
            f("savepoint", 2),
            f("release", 2),
            f("rollback_to", 2),
            f("exec_script", 2),
            f("table_exists", 2),
        ],
        constants: &[],
        feature_gated: true,
    },
];

/// The `std/` module for an import path, e.g. `"std/math"`.
///
/// Feature-gated modules are included: a tool that only reads names should
/// describe `std/db` the same way in every build, and `<# std/db` in a build
/// without the feature already fails with module-not-found at load time.
pub fn module(path: &str) -> Option<&'static StdModule> {
    MODULES.iter().find(|m| m.path == path)
}

/// Whether `path` names a `std/` module.
pub fn is_std_module(path: &str) -> bool {
    module(path).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_by_path() {
        let math = module("std/math").expect("std/math is a stdlib module");
        assert_eq!(math.function("sin").map(|f| f.arity), Some(1));
        assert!(math.has_constant("PI"));
        assert!(math.function("inventada").is_none());
        assert!(module("std/nope").is_none());
    }

    #[test]
    fn variadic_arity_is_negative() {
        let net = module("std/net").expect("std/net is a stdlib module");
        assert_eq!(net.function("get").map(|f| f.arity), Some(-1));
    }

    #[test]
    fn no_duplicate_names_within_a_module() {
        for m in MODULES {
            for (i, func) in m.functions.iter().enumerate() {
                assert!(
                    !m.functions[..i].iter().any(|p| p.name == func.name),
                    "{} lists {} twice",
                    m.path,
                    func.name
                );
            }
        }
    }
}
