//! Semantic Analyzer for Zymbol Module System
//!
//! This crate provides semantic validation for the Zymbol module system, including:
//! - File name validation (module name must match filename)
//! - Path resolution (./,  ../, subdirectories)
//! - Import validation (modules exist, no circular dependencies)
//! - Export validation (items exist and are visible)
//! - Re-export validation (correct types, items exist)
//! - Variable lifetime analysis (unused variables, dead code detection)
//! - Type checking and inference
//! - Definition-use chain analysis


mod call_arity;
mod modules;
mod stdlib_access;
mod variable_analysis;
mod cfg;
mod def_use;
mod last_use;
mod loop_context;
mod type_check;

pub use call_arity::{
    arities_of_module_file, module_arities, module_out_slots, resolved_import_path, AliasArities,
    AliasOutSlots, ModuleArities, ModuleOutSlots,
};
pub use modules::{SemanticError, ExportedItem, ExportTable, ModuleAnalyzer};
pub use stdlib_access::check_stdlib_access;
pub use variable_analysis::{VariableAnalyzer, VariableInfo, VariableDiagnostic, Severity};
pub use cfg::{ControlFlowGraph, CfgNode, CfgEdge, EdgeCondition, NodeId};
pub use def_use::{
    DefUseAnalyzer, DefUseChain, Definition, Use, UseType,
    AmbiguousLifetime, AmbiguityReason,
};
pub use type_check::{TypeChecker, TypeEnv, ZymbolType};
pub use last_use::{auto_free_exclusions, region_schedule};
pub use loop_context::check_loop_context;
