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


mod modules;
mod variable_analysis;
mod cfg;
mod def_use;
mod type_check;

pub use modules::{SemanticError, ExportedItem, ExportTable, ModuleAnalyzer};
pub use variable_analysis::{VariableAnalyzer, VariableInfo, VariableDiagnostic, Severity};
pub use cfg::{ControlFlowGraph, CfgNode, CfgEdge, EdgeCondition, NodeId};
pub use def_use::{
    DefUseAnalyzer, DefUseChain, Definition, Use, UseType,
    AmbiguousLifetime, AmbiguityReason,
};
pub use type_check::{TypeChecker, TypeEnv, ZymbolType};
