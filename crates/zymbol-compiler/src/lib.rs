//! AST → Bytecode compiler for Zymbol-Lang (Sprint 4A)
//!
//! Scope: literals, variables, arithmetic, if/else, loops (range/while/infinite), output.
//! Functions and arrays are compiled as stubs (Fase 4B / 4C).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use thiserror::Error;
use zymbol_ast::{
    Block, Expr, LiteralExpr, Statement,
    Output, IfStmt, Loop, Break, Continue,
    FunctionDecl, LambdaBody,
    TryStmt, FormatKind, PrecisionOp,
    DestructureAssign, DestructureItem, DestructurePattern,
    DeepIndexExpr, FlatExtractExpr, StructuredExtractExpr,
};
use zymbol_ast::Pattern;
use zymbol_ast::BasePrefix;
use zymbol_ast::CastKind;
use zymbol_bytecode::{BuildPart, Chunk, CompiledProgram, FuncIdx, Instruction, Label, Reg, StrIdx};
use zymbol_common::{BinaryOp, Literal, UnaryOp};

// ──────────────────────────────────────────────────────────────────────────────
// Simple static types for type-directed code generation
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StaticType {
    Int,
    Float,
    Bool,
    String,
    Char,
    Unknown,
}


// ──────────────────────────────────────────────────────────────────────────────
// Error
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("too many registers (>65535) in function '{0}'")]
    TooManyRegisters(String),
    #[error("undefined variable '{0}'")]
    UndefinedVariable(String),
    #[error("unsupported construct: {0}")]
    Unsupported(String),
    #[error("break outside loop")]
    BreakOutsideLoop,
    #[error("continue outside loop")]
    ContinueOutsideLoop,
    #[error("E004: Circular import detected: module '{0}' is already being loaded")]
    CircularImport(String),
    /// Module had parse errors; the message includes count + per-diagnostic lines.
    #[error("failed to parse module: {0}")]
    ModuleParse(String),
}

// ──────────────────────────────────────────────────────────────────────────────
// Loop context (for break/continue patching)
// ──────────────────────────────────────────────────────────────────────────────

struct LoopCtx {
    /// Instruction positions of `Jump(0)` placeholders that need patching to loop_end
    break_patches: Vec<usize>,
    /// Instruction positions of `Jump(0)` placeholders for `@>` (continue).
    /// Resolved to: loop_start (infinite/while) or increment label (range/foreach).
    continue_patches: Vec<usize>,
    /// Optional label for this loop (from `@ @label { }` syntax)
    label: Option<String>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Function compilation context
// ──────────────────────────────────────────────────────────────────────────────

struct FunctionCtx {
    /// Variable name → register index
    register_map: HashMap<String, Reg>,
    /// Register → static type (best-effort inference for code generation)
    reg_types: Vec<StaticType>,
    /// Next free register index
    next_reg: u16,
    /// Emitted instructions
    instructions: Vec<Instruction>,
    /// Stack of loop contexts (innermost last)
    loop_stack: Vec<LoopCtx>,
    /// Name of this function (for error messages)
    name: String,
}

impl FunctionCtx {
    fn new(name: impl Into<String>) -> Self {
        Self {
            register_map: HashMap::new(),
            reg_types: Vec::new(),
            next_reg: 0,
            instructions: Vec::new(),
            loop_stack: Vec::new(),
            name: name.into(),
        }
    }

    fn set_reg_type(&mut self, reg: Reg, ty: StaticType) {
        let idx = reg as usize;
        if self.reg_types.len() <= idx {
            self.reg_types.resize(idx + 1, StaticType::Unknown);
        }
        self.reg_types[idx] = ty;
    }

    fn get_reg_type(&self, reg: Reg) -> StaticType {
        self.reg_types.get(reg as usize).copied().unwrap_or(StaticType::Unknown)
    }

    /// Allocate a register for a named variable.
    /// If the variable already has a register, overwrite its binding (re-assignment).
    fn alloc_reg(&mut self, name: &str) -> Result<Reg, CompileError> {
        if let Some(&r) = self.register_map.get(name) {
            return Ok(r);
        }
        let r = self.next_reg;
        if r == u16::MAX {
            return Err(CompileError::TooManyRegisters(self.name.clone()));
        }
        self.register_map.insert(name.to_string(), r);
        self.next_reg += 1;
        Ok(r)
    }

    /// Allocate an anonymous temporary register (not bound to any variable).
    fn alloc_temp(&mut self) -> Result<Reg, CompileError> {
        let r = self.next_reg;
        if r == u16::MAX {
            return Err(CompileError::TooManyRegisters(self.name.clone()));
        }
        self.next_reg += 1;
        Ok(r)
    }

    /// Look up which register holds a variable.
    fn get_reg(&self, name: &str) -> Result<Reg, CompileError> {
        self.register_map
            .get(name)
            .copied()
            .ok_or_else(|| CompileError::UndefinedVariable(name.to_string()))
    }

    fn emit(&mut self, instr: Instruction) -> usize {
        let pos = self.instructions.len();
        self.instructions.push(instr);
        pos
    }

    fn current_label(&self) -> Label {
        self.instructions.len() as Label
    }

    /// Emit a Jump placeholder; returns the instruction position for later patching.
    fn emit_jump_placeholder(&mut self) -> usize {
        self.emit(Instruction::Jump(0))
    }

    /// Emit a JumpIfNot placeholder; returns position for later patching.
    fn emit_jump_if_not_placeholder(&mut self, cond: Reg) -> usize {
        self.emit(Instruction::JumpIfNot(cond, 0))
    }

    fn patch_jump(&mut self, pos: usize, target: Label) {
        match &mut self.instructions[pos] {
            Instruction::Jump(lbl) => *lbl = target,
            Instruction::JumpIf(_, lbl) => *lbl = target,
            Instruction::JumpIfNot(_, lbl) => *lbl = target,
            Instruction::MatchStr(_, _, lbl) => *lbl = target,
            Instruction::MatchInt(_, _, lbl) => *lbl = target,
            Instruction::MatchBool(_, _, lbl) => *lbl = target,
            _ => panic!("patch_jump called on non-jump instruction at {}", pos),
        }
    }

    /// Snapshot current variable names for block scoping.
    fn save_scope(&self) -> std::collections::HashSet<String> {
        self.register_map.keys().cloned().collect()
    }

    /// After compiling a block: zero out registers for variables newly introduced in that block
    /// and remove them from the register_map (they're now out of scope).
    fn zero_new_vars(&mut self, saved: &std::collections::HashSet<String>) {
        let new_vars: Vec<(String, Reg)> = self.register_map
            .iter()
            .filter(|(name, _)| !saved.contains(name.as_str()))
            .map(|(n, &r)| (n.clone(), r))
            .collect();
        for (name, reg) in new_vars {
            self.instructions.push(Instruction::LoadUnit(reg));
            self.register_map.remove(&name);
        }
    }

    fn patch_try_begin(&mut self, pos: usize, target: Label) {
        match &mut self.instructions[pos] {
            Instruction::TryBegin(lbl) => *lbl = target,
            _ => panic!("patch_try_begin called on non-TryBegin instruction at {}", pos),
        }
    }

    fn into_chunk(mut self, num_params: u16) -> Chunk {
        let old_num_registers = self.next_reg;
        self.emit(Instruction::Halt);
        let (instructions, num_registers) = eliminate_dead_code(self.instructions, old_num_registers);
        Chunk {
            name: self.name,
            instructions,
            num_registers,
            num_params,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Module constant value (evaluated at compile time for module imports)
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
enum ModuleConst {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Char(char),
}

// ──────────────────────────────────────────────────────────────────────────────
// Top-level Compiler
// ──────────────────────────────────────────────────────────────────────────────

pub struct Compiler {
    string_pool: Vec<String>,
    functions: Vec<Chunk>,
    function_index: HashMap<String, FuncIdx>,
    /// Module constants: "alias.NAME" → value (for module member access)
    module_constants: HashMap<String, ModuleConst>,
    /// Global constants: NAME → value (for inlining module-level vars into function bodies)
    global_consts: HashMap<String, ModuleConst>,
    /// Output param map: FuncIdx → Vec<bool> (true = output param at that position)
    output_param_map: HashMap<FuncIdx, Vec<bool>>,
    /// When true, undefined variable references emit RaiseError instead of compile error.
    /// Set when compiling function bodies (matches tree-walker's runtime detection).
    in_function_body: bool,
    /// Base directory of the source file (for resolving relative paths in Execute)
    base_dir: Option<PathBuf>,
    /// Files currently being compiled (for circular import detection)
    loading_stack: HashSet<PathBuf>,
    /// Local function scope active during module compilation (plain name → FuncIdx).
    /// Allows module functions to call private sibling functions.
    module_scope: HashMap<String, FuncIdx>,
    /// Module-level mutable variables: name → global var index.
    /// Active during module function compilation so references emit LoadGlobal/StoreGlobal.
    global_var_map: HashMap<String, u16>,
    /// Initial values for global module variables (indexed by u16)
    global_var_inits: Vec<zymbol_bytecode::GlobalInit>,
}

impl Compiler {
    pub fn compile(program: &zymbol_ast::Program) -> Result<CompiledProgram, CompileError> {
        Self::compile_with_dir(program, None)
    }

    pub fn compile_with_dir(program: &zymbol_ast::Program, base_dir: Option<&Path>) -> Result<CompiledProgram, CompileError> {
        let mut compiler = Compiler {
            string_pool: Vec::new(),
            functions: Vec::new(),
            function_index: HashMap::new(),
            module_constants: HashMap::new(),
            global_consts: HashMap::new(),
            output_param_map: HashMap::new(),
            in_function_body: false,
            base_dir: base_dir.map(|p| p.to_path_buf()),
            loading_stack: HashSet::new(),
            module_scope: HashMap::new(),
            global_var_map: HashMap::new(),
            global_var_inits: Vec::new(),
        };

        // Process imports first — register module functions as "alias::func"
        if let Some(base) = base_dir {
            for import in &program.imports {
                compiler.compile_import(import, base)?;
            }
        }

        // First pass: register function names so forward calls work (Fase 4B)
        // Also register output param info so compile_call can set up writeback.
        for stmt in &program.statements {
            if let Statement::FunctionDecl(decl) = stmt {
                let idx = compiler.functions.len() as FuncIdx;
                compiler.function_index.insert(decl.name.clone(), idx);
                // Register output param flags
                let out_flags: Vec<bool> = decl.parameters.iter()
                    .map(|p| p.kind == zymbol_ast::ParameterKind::Output)
                    .collect();
                if out_flags.iter().any(|&b| b) {
                    compiler.output_param_map.insert(idx, out_flags);
                }
                // placeholder — will be replaced in second pass
                compiler.functions.push(Chunk::new(&decl.name));
            }
        }

        // Collect global IMMUTABLE constants (:=) so function bodies can inline them.
        // Mutable assignments (=) are NOT inlined to preserve function scope isolation.
        for stmt in &program.statements {
            if let Statement::ConstDecl(c) = stmt {
                if let Some(mc) = Self::eval_const_expr(&c.value) {
                    compiler.global_consts.insert(c.name.clone(), mc);
                }
            }
        }

        // Compile function bodies
        let func_decls: Vec<&FunctionDecl> = program
            .statements
            .iter()
            .filter_map(|s| {
                if let Statement::FunctionDecl(d) = s {
                    Some(d)
                } else {
                    None
                }
            })
            .collect();

        for decl in func_decls {
            let chunk = compiler.compile_function(decl)?;
            let idx = compiler.function_index[&decl.name] as usize;
            compiler.functions[idx] = chunk;
        }

        // Compile main body
        let mut ctx = FunctionCtx::new("<main>");
        for stmt in &program.statements {
            if !matches!(stmt, Statement::FunctionDecl(_)) {
                compiler.compile_stmt(stmt, &mut ctx)?;
            }
        }
        let main_chunk = ctx.into_chunk(0);

        let mut compiled = CompiledProgram::new(main_chunk);
        compiled.functions = compiler.functions;
        compiled.string_pool = compiler.string_pool;
        compiled.global_var_inits = compiler.global_var_inits;
        Ok(compiled)
    }

    /// Process a module import: load, parse, and compile exported + private functions,
    /// registering exported ones as `alias::func_name` in function_index.
    /// Also handles: circular import detection, nested sub-imports, and re-exports.
    fn compile_import(&mut self, import: &zymbol_ast::ImportStmt, base_dir: &Path) -> Result<(), CompileError> {
        // Build file path from import path components
        let mut path = base_dir.to_path_buf();
        for _ in 0..import.path.parent_levels {
            path.pop();
        }
        for component in &import.path.components {
            path.push(component);
        }
        path.set_extension("zy");

        // Canonicalize for reliable circular import detection
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());

        // Circular import detection — use module stem name to match WT message format
        if self.loading_stack.contains(&canonical) {
            let module_name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            return Err(CompileError::CircularImport(module_name));
        }
        self.loading_stack.insert(canonical.clone());

        // Read module source
        let source = std::fs::read_to_string(&path)
            .map_err(|e| CompileError::Unsupported(format!("cannot read module '{}': {}", path.display(), e)))?;

        // Lex + parse
        let file_id = zymbol_span::FileId(0);
        let lexer = zymbol_lexer::Lexer::new(&source, file_id);
        let (tokens, _lex_errs) = lexer.tokenize();
        let parser = zymbol_parser::Parser::new(tokens);
        let module_prog = parser.parse().map_err(|errors| {
            let canon_path = canonical.display().to_string();
            let detail: Vec<String> = errors.iter().map(|d| {
                let loc = d.span
                    .map(|s| format!("{}:{}:{}", canon_path, s.start.line, s.start.column))
                    .unwrap_or_else(|| canon_path.clone());
                let mut msg = format!("  {}: {}", loc, d.message);
                if let Some(help) = &d.help {
                    msg.push_str(&format!("\n    help: {}", help));
                }
                msg
            }).collect();
            CompileError::ModuleParse(format!(
                "{} parse error(s) in '{}'\n{}",
                errors.len(),
                canon_path,
                detail.join("\n")
            ))
        })?;

        let module_base_dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();

        // Recursively process sub-imports of the module (for nested imports like i18n modules)
        for sub_import in &module_prog.imports {
            self.compile_import(sub_import, &module_base_dir)?;
        }

        let alias = import.alias.clone();

        // Collect ALL function names (exported + private) for module_scope
        let all_func_names: Vec<String> = module_prog.statements.iter().filter_map(|s| {
            if let Statement::FunctionDecl(d) = s { Some(d.name.clone()) } else { None }
        }).collect();

        // Collect exported items (Own = local, ReExport = from sub-module)
        let mut own_func_exports: Vec<(String, String)> = Vec::new(); // (internal, public)
        let mut own_const_exports: Vec<(String, String)> = Vec::new();
        let mut reexport_funcs: Vec<(String, String, String)> = Vec::new(); // (src_alias, item_name, public_name)
        let mut reexport_consts: Vec<(String, String, String)> = Vec::new();

        if let Some(decl) = &module_prog.module_decl {
            if let Some(export_block) = &decl.export_block {
                for item in &export_block.items {
                    match item {
                        zymbol_ast::ExportItem::Own { name, rename, .. } => {
                            let public = rename.as_ref().unwrap_or(name).clone();
                            let is_func = module_prog.statements.iter().any(|s| {
                                matches!(s, Statement::FunctionDecl(d) if &d.name == name)
                            });
                            if is_func {
                                own_func_exports.push((name.clone(), public));
                            } else {
                                own_const_exports.push((name.clone(), public));
                            }
                        }
                        zymbol_ast::ExportItem::ReExport { module_alias, item_name, item_type, rename, .. } => {
                            let public = rename.as_ref().unwrap_or(item_name).clone();
                            match item_type {
                                zymbol_ast::ItemType::Function =>
                                    reexport_funcs.push((module_alias.clone(), item_name.clone(), public)),
                                zymbol_ast::ItemType::Constant =>
                                    reexport_consts.push((module_alias.clone(), item_name.clone(), public)),
                            }
                        }
                    }
                }
            }
        }

        // Reserve slots for ALL module functions (exported + private)
        let start_idx = self.functions.len();
        let mut local_scope: HashMap<String, FuncIdx> = HashMap::new();
        for (i, name) in all_func_names.iter().enumerate() {
            let idx = (start_idx + i) as FuncIdx;
            local_scope.insert(name.clone(), idx);
            self.functions.push(Chunk::new(name.as_str()));
        }

        // Register exported functions in function_index as "alias::public_name"
        for (internal, public) in &own_func_exports {
            if let Some(&idx) = local_scope.get(internal) {
                let qualified = format!("{}::{}", alias, public);
                self.function_index.insert(qualified, idx);
            }
        }

        // Collect module-level immutable constants (:=) for function body inlining
        let saved_global_consts = std::mem::take(&mut self.global_consts);
        for stmt in &module_prog.statements {
            if let Statement::ConstDecl(c) = stmt {
                if let Some(mc) = Self::eval_const_expr(&c.value) {
                    self.global_consts.insert(c.name.clone(), mc);
                }
            }
        }

        // Register module-level mutable variables (= not :=) as global vars
        // so function bodies can read/write them across calls via LoadGlobal/StoreGlobal.
        let mut module_gvar_names: Vec<String> = Vec::new();
        for stmt in &module_prog.statements {
            if let Statement::Assignment(a) = stmt {
                if !self.global_var_map.contains_key(&a.name) {
                    let gvar_idx = self.global_var_inits.len() as u16;
                    let init = if let Some(mc) = Self::eval_const_expr(&a.value) {
                        match mc {
                            ModuleConst::Int(n) => zymbol_bytecode::GlobalInit::Int(n),
                            ModuleConst::Float(f) => zymbol_bytecode::GlobalInit::Float(f),
                            ModuleConst::Bool(b) => zymbol_bytecode::GlobalInit::Bool(b),
                            ModuleConst::Char(c) => zymbol_bytecode::GlobalInit::Char(c),
                            ModuleConst::String(s) => zymbol_bytecode::GlobalInit::Str(s),
                        }
                    } else {
                        zymbol_bytecode::GlobalInit::Unit
                    };
                    self.global_var_inits.push(init);
                    self.global_var_map.insert(a.name.clone(), gvar_idx);
                    module_gvar_names.push(a.name.clone());
                }
            }
        }

        // Activate module_scope so compile_call finds private sibling functions
        let saved_module_scope = std::mem::replace(&mut self.module_scope, local_scope);

        // Compile ALL function bodies (exported + private)
        for (i, name) in all_func_names.iter().enumerate() {
            let func_decl = module_prog.statements.iter().find_map(|s| {
                if let Statement::FunctionDecl(d) = s {
                    if &d.name == name { Some(d) } else { None }
                } else {
                    None
                }
            });
            if let Some(decl) = func_decl {
                let chunk = self.compile_function(decl)?;
                self.functions[start_idx + i] = chunk;
            }
        }

        // Restore module_scope, global_consts, and remove this module's global var entries
        self.module_scope = saved_module_scope;
        self.global_consts = saved_global_consts;
        for name in &module_gvar_names {
            self.global_var_map.remove(name);
        }

        // Collect own constant/variable exports
        for (internal_name, public_name) in &own_const_exports {
            let val_expr = module_prog.statements.iter().find_map(|s| {
                match s {
                    Statement::ConstDecl(c) if &c.name == internal_name => Some(&c.value),
                    Statement::Assignment(a) if &a.name == internal_name => Some(&a.value),
                    _ => None,
                }
            });
            if let Some(expr) = val_expr {
                if let Some(mc) = Self::eval_const_expr(expr) {
                    let key = format!("{}.{}", alias, public_name);
                    self.module_constants.insert(key, mc);
                }
            }
        }

        // Handle re-exports from sub-modules (e.g., mat::sumar <= προσθέτω)
        for (src_alias, item_name, public_name) in &reexport_funcs {
            let src_qualified = format!("{}::{}", src_alias, item_name);
            if let Some(&idx) = self.function_index.get(&src_qualified) {
                let dst_qualified = format!("{}::{}", alias, public_name);
                self.function_index.insert(dst_qualified, idx);
            }
        }
        for (src_alias, item_name, public_name) in &reexport_consts {
            let src_key = format!("{}.{}", src_alias, item_name);
            if let Some(mc) = self.module_constants.get(&src_key).cloned() {
                let dst_key = format!("{}.{}", alias, public_name);
                self.module_constants.insert(dst_key, mc);
            }
        }

        // Done with this module — remove from loading stack
        self.loading_stack.remove(&canonical);

        Ok(())
    }

    fn compile_function(&mut self, decl: &FunctionDecl) -> Result<Chunk, CompileError> {
        let mut ctx = FunctionCtx::new(&decl.name);
        // Bind parameters to the first N registers
        for param in &decl.parameters {
            ctx.alloc_reg(&param.name)?;
        }
        let num_params = decl.parameters.len() as u16;
        let prev_in_fn = self.in_function_body;
        self.in_function_body = true;
        let result = self.compile_block(&decl.body, &mut ctx);
        self.in_function_body = prev_in_fn;
        result?;
        // Implicit return Unit if no explicit <~
        let unit_reg = ctx.alloc_temp()?;
        ctx.emit(Instruction::LoadUnit(unit_reg));
        ctx.emit(Instruction::Return(unit_reg));
        let chunk = ctx.into_chunk(num_params);
        Ok(chunk)
    }

    // ── Statement compilation ───────────────────────────────────────────────

    fn compile_stmt(
        &mut self,
        stmt: &Statement,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        match stmt {
            Statement::Assignment(a) => self.compile_assignment(&a.name, &a.value, ctx),
            Statement::ConstDecl(c) => self.compile_assignment(&c.name, &c.value, ctx),
            Statement::Output(o) => self.compile_output(o, ctx),
            Statement::Newline(_n) => {
                ctx.emit(Instruction::PrintNewline);
                Ok(())
            }
            Statement::If(if_stmt) => self.compile_if(if_stmt, ctx),
            Statement::Loop(lp) => self.compile_loop(lp, ctx),
            Statement::Break(b) => self.compile_break(b, ctx),
            Statement::Continue(c) => self.compile_continue(c, ctx),
            Statement::Return(r) => {
                // TCO: if `<~ f(args)` where f is the current function → TailCall
                if let Some(val) = &r.value {
                    if let Expr::FunctionCall(call) = val.as_ref() {
                        if let Expr::Identifier(id) = call.callable.as_ref() {
                            if id.name == ctx.name {
                                if let Some(&func_idx) = self.function_index.get(&id.name) {
                                    let mut arg_regs = Vec::with_capacity(call.arguments.len());
                                    for arg in &call.arguments {
                                        arg_regs.push(self.compile_expr(arg, ctx)?);
                                    }
                                    ctx.emit(Instruction::TailCall(func_idx, arg_regs));
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
                let reg = if let Some(val) = &r.value {
                    self.compile_expr(val, ctx)?
                } else {
                    let t = ctx.alloc_temp()?;
                    ctx.emit(Instruction::LoadUnit(t));
                    t
                };
                ctx.emit(Instruction::Return(reg));
                Ok(())
            }
            Statement::FunctionDecl(_) => {
                // Already handled in first pass
                Ok(())
            }
            Statement::Expr(expr_stmt) => {
                // Evaluate for side effects, discard result
                self.compile_expr(&expr_stmt.expr, ctx)?;
                Ok(())
            }
            // 4C: Match statement (discards result)
            Statement::Match(m) => {
                self.compile_match_stmt(m, ctx)
            }
            Statement::DestructureAssign(d) => self.compile_destructure_assign(d, ctx),
            // Unsupported — produce meaningful error
            Statement::Input(_) => Err(CompileError::Unsupported("input (<<)".into())),
            Statement::Try(ts) => self.compile_try(ts, ctx),
            Statement::LifetimeEnd(lifetime_end) => {
                if let Ok(r) = ctx.get_reg(&lifetime_end.variable_name) {
                    ctx.emit(Instruction::LoadUnit(r));
                    ctx.register_map.remove(&lifetime_end.variable_name);
                }
                Ok(())
            }
            Statement::CliArgsCapture(_) => {
                Err(CompileError::Unsupported("CLI args capture — VM Fase 4C".into()))
            }
            Statement::SetNumeralMode { base, .. } => {
                ctx.emit(Instruction::SetNumeralMode(*base));
                Ok(())
            }
        }
    }

    fn compile_block(
        &mut self,
        block: &Block,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        for stmt in &block.statements {
            self.compile_stmt(stmt, ctx)?;
        }
        Ok(())
    }

    fn compile_assignment(
        &mut self,
        name: &str,
        value: &Expr,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        // Optimise: arr = arr$+ elem → ArrayPush in-place (O(1), no clone)
        if let Expr::CollectionAppend(ca) = value {
            if let Expr::Identifier(ident) = ca.collection.as_ref() {
                if ident.name == name {
                    if let Ok(arr_reg) = ctx.get_reg(name) {
                        let r_elem = self.compile_expr(&ca.element, ctx)?;
                        ctx.emit(Instruction::ArrayPush(arr_reg, r_elem));
                        return Ok(());
                    }
                }
            }
        }

        let src = self.compile_expr(value, ctx)?;
        let src_ty = ctx.get_reg_type(src);

        // If this name is a module global var, emit StoreGlobal instead of local register assign
        if let Some(&gvar_idx) = self.global_var_map.get(name) {
            ctx.emit(Instruction::StoreGlobal(gvar_idx, src));
            return Ok(());
        }

        // If re-assignment, get existing dst register; otherwise allocate new.
        let dst = if let Ok(existing) = ctx.get_reg(name) {
            existing
        } else {
            ctx.alloc_reg(name)?
        };
        // Propagate type to destination register
        if src_ty != StaticType::Unknown {
            ctx.set_reg_type(dst, src_ty);
        }
        if src != dst {
            ctx.emit(Instruction::CopyReg(dst, src));
        }
        Ok(())
    }

    fn compile_destructure_assign(
        &mut self,
        d: &DestructureAssign,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        let r_rhs = self.compile_expr(&d.value, ctx)?;
        match &d.pattern {
            DestructurePattern::Array(items) | DestructurePattern::Positional(items) => {
                let mut idx: i64 = 0;
                for item in items {
                    match item {
                        DestructureItem::Bind(name) => {
                            let r_idx = ctx.alloc_temp()?;
                            ctx.emit(Instruction::LoadInt(r_idx, idx + 1));
                            let dst = if let Ok(existing) = ctx.get_reg(name) {
                                existing
                            } else {
                                ctx.alloc_reg(name)?
                            };
                            ctx.emit(Instruction::ArrayGet(dst, r_rhs, r_idx));
                            idx += 1;
                        }
                        DestructureItem::Rest(name) => {
                            let r_lo = ctx.alloc_temp()?;
                            ctx.emit(Instruction::LoadInt(r_lo, idx + 1));
                            let r_hi = ctx.alloc_temp()?;
                            // Use array length as hi (slice to end)
                            ctx.emit(Instruction::ArrayLen(r_hi, r_rhs));
                            let dst = if let Ok(existing) = ctx.get_reg(name) {
                                existing
                            } else {
                                ctx.alloc_reg(name)?
                            };
                            ctx.emit(Instruction::ArraySlice(dst, r_rhs, r_lo));
                            idx += 1;
                        }
                        DestructureItem::Ignore => {
                            idx += 1;
                        }
                    }
                }
            }
            DestructurePattern::NamedTuple(fields) => {
                for (field, var_name) in fields {
                    let field_idx = self.intern_string(field);
                    let dst = if let Ok(existing) = ctx.get_reg(var_name) {
                        existing
                    } else {
                        ctx.alloc_reg(var_name)?
                    };
                    ctx.emit(Instruction::NamedTupleGet(dst, r_rhs, field_idx));
                }
            }
        }
        Ok(())
    }

    fn compile_output(
        &mut self,
        output: &Output,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        for expr in &output.exprs {
            // `¶` is a Newline literal inside the expr list — detect it
            if let Expr::Literal(lit) = expr {
                if matches!(&lit.value, Literal::String(s) if s == "\n") {
                    ctx.emit(Instruction::PrintNewline);
                    continue;
                }
            }
            let reg = self.compile_expr(expr, ctx)?;
            ctx.emit(Instruction::Print(reg));
        }
        Ok(())
    }

    fn compile_if(
        &mut self,
        if_stmt: &IfStmt,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        // Collect all end-of-branch jumps to patch to `end_label`
        let mut end_patches: Vec<usize> = Vec::new();

        // ── Primary branch ──
        let cond_reg = self.compile_expr(&if_stmt.condition, ctx)?;
        let skip_then = ctx.emit_jump_if_not_placeholder(cond_reg);
        let saved = ctx.save_scope();
        self.compile_block(&if_stmt.then_block, ctx)?;
        ctx.zero_new_vars(&saved);
        let jump_to_end = ctx.emit_jump_placeholder();
        end_patches.push(jump_to_end);
        let next_label = ctx.current_label();
        ctx.patch_jump(skip_then, next_label);

        // ── Else-if branches ──
        for elif in &if_stmt.else_if_branches {
            let cond_reg = self.compile_expr(&elif.condition, ctx)?;
            let skip = ctx.emit_jump_if_not_placeholder(cond_reg);
            let saved = ctx.save_scope();
            self.compile_block(&elif.block, ctx)?;
            ctx.zero_new_vars(&saved);
            let j = ctx.emit_jump_placeholder();
            end_patches.push(j);
            let next_label = ctx.current_label();
            ctx.patch_jump(skip, next_label);
        }

        // ── Else ──
        if let Some(else_block) = &if_stmt.else_block {
            let saved = ctx.save_scope();
            self.compile_block(else_block, ctx)?;
            ctx.zero_new_vars(&saved);
        }

        let end_label = ctx.current_label();
        for pos in end_patches {
            ctx.patch_jump(pos, end_label);
        }
        Ok(())
    }

    fn compile_loop(
        &mut self,
        lp: &Loop,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        // Four cases: infinite, while, range for-each, array for-each
        if lp.iterator_var.is_some() {
            // Check if iterable is a Range or an array/expression
            let is_range = lp.iterable.as_ref().map_or(false, |e| matches!(e.as_ref(), Expr::Range(_)));
            if is_range {
                return self.compile_range_loop(lp, ctx);
            } else {
                return self.compile_foreach_loop(lp, ctx);
            }
        } else if lp.condition.is_some() {
            // Detect TIMES loop: condition is a literal Int → repeat N times
            let cond = lp.condition.as_ref().unwrap().as_ref();
            let is_literal_times = matches!(cond, Expr::Literal(lit) if matches!(lit.value, Literal::Int(_)));
            // Dynamic times: condition is an identifier (variable holding an Int count)
            let is_dynamic_times = matches!(cond, Expr::Identifier(_));
            if is_literal_times {
                self.compile_times_loop(lp, ctx)
            } else if is_dynamic_times {
                self.compile_dynamic_times_loop(lp, ctx)
            } else {
                self.compile_while_loop(lp, ctx)
            }
        } else {
            self.compile_infinite_loop(lp, ctx)
        }
    }

    fn compile_times_loop(
        &mut self,
        lp: &Loop,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        let n = if let Some(cond) = lp.condition.as_ref() {
            if let Expr::Literal(lit) = cond.as_ref() {
                if let Literal::Int(n) = lit.value { n as i64 } else { 0 }
            } else { 0 }
        } else { 0 };

        if n <= 0 {
            // Zero or negative: body never runs
            return Ok(());
        }

        // Compile as: r_i = 0; while r_i < n { body; r_i++ }
        let r_i = ctx.alloc_temp()?;
        let r_end = ctx.alloc_temp()?;
        let r_cmp = ctx.alloc_temp()?;
        let r_one = ctx.alloc_temp()?;
        ctx.emit(Instruction::LoadInt(r_i, 0));
        ctx.emit(Instruction::LoadInt(r_end, n - 1));
        ctx.emit(Instruction::LoadInt(r_one, 1));

        let loop_start = ctx.current_label();
        ctx.loop_stack.push(LoopCtx { break_patches: Vec::new(), continue_patches: Vec::new(), label: lp.label.clone() });

        ctx.emit(Instruction::CmpGt(r_cmp, r_i, r_end));
        let exit_jump = ctx.emit(Instruction::JumpIf(r_cmp, 0));

        self.compile_block(&lp.body, ctx)?;

        let inc_label = ctx.current_label();
        ctx.emit(Instruction::AddInt(r_i, r_i, r_one));
        ctx.emit(Instruction::Jump(loop_start));

        let loop_end = ctx.current_label();
        ctx.patch_jump(exit_jump, loop_end);

        let lctx = ctx.loop_stack.pop().unwrap();
        for pos in lctx.break_patches { ctx.patch_jump(pos, loop_end); }
        for pos in lctx.continue_patches { ctx.patch_jump(pos, inc_label); }
        Ok(())
    }

    fn compile_dynamic_times_loop(
        &mut self,
        lp: &Loop,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        // Evaluate the count expression once
        let r_n = self.compile_expr(lp.condition.as_ref().unwrap(), ctx)?;
        let r_i = ctx.alloc_temp()?;
        let r_cmp = ctx.alloc_temp()?;
        ctx.emit(Instruction::LoadInt(r_i, 0));

        let loop_start = ctx.current_label();
        ctx.loop_stack.push(LoopCtx { break_patches: Vec::new(), continue_patches: Vec::new(), label: lp.label.clone() });

        ctx.emit(Instruction::CmpGe(r_cmp, r_i, r_n));
        let exit_jump = ctx.emit(Instruction::JumpIf(r_cmp, 0));

        self.compile_block(&lp.body, ctx)?;

        let inc_label = ctx.current_label();
        ctx.emit(Instruction::AddIntImm(r_i, r_i, 1));
        ctx.emit(Instruction::Jump(loop_start));

        let loop_end = ctx.current_label();
        ctx.patch_jump(exit_jump, loop_end);

        let lctx = ctx.loop_stack.pop().unwrap();
        for pos in lctx.break_patches { ctx.patch_jump(pos, loop_end); }
        for pos in lctx.continue_patches { ctx.patch_jump(pos, inc_label); }
        Ok(())
    }

    fn compile_infinite_loop(
        &mut self,
        lp: &Loop,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        let loop_start = ctx.current_label();
        ctx.loop_stack.push(LoopCtx {
            break_patches: Vec::new(),
            continue_patches: Vec::new(),
            label: lp.label.clone(),
        });

        self.compile_block(&lp.body, ctx)?;
        ctx.emit(Instruction::Jump(loop_start));

        let loop_end = ctx.current_label();
        let lctx = ctx.loop_stack.pop().unwrap();
        for pos in lctx.break_patches {
            ctx.patch_jump(pos, loop_end);
        }
        for pos in lctx.continue_patches {
            ctx.patch_jump(pos, loop_start);
        }
        Ok(())
    }

    fn compile_while_loop(
        &mut self,
        lp: &Loop,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        let cond_expr = lp.condition.as_ref().unwrap();
        let loop_start = ctx.current_label();

        ctx.loop_stack.push(LoopCtx {
            break_patches: Vec::new(),
            continue_patches: Vec::new(),
            label: lp.label.clone(),
        });

        let cond_reg = self.compile_expr(cond_expr, ctx)?;
        let skip_body = ctx.emit_jump_if_not_placeholder(cond_reg);

        self.compile_block(&lp.body, ctx)?;
        ctx.emit(Instruction::Jump(loop_start));

        let loop_end = ctx.current_label();
        ctx.patch_jump(skip_body, loop_end);

        let lctx = ctx.loop_stack.pop().unwrap();
        for pos in lctx.break_patches {
            ctx.patch_jump(pos, loop_end);
        }
        for pos in lctx.continue_patches {
            ctx.patch_jump(pos, loop_start);
        }
        Ok(())
    }

    fn compile_range_loop(
        &mut self,
        lp: &Loop,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        let iter_var = lp.iterator_var.as_ref().unwrap();
        let iterable = lp.iterable.as_ref().unwrap();

        // Expect `iterable` to be a Range expression: start..end
        let (start_expr, end_expr, step_expr_opt) = match iterable.as_ref() {
            Expr::Range(r) => (&r.start, &r.end, r.step.as_deref()),
            // Array for-each — Fase 4C
            _ => {
                return Err(CompileError::Unsupported(
                    "array for-each loop — VM Fase 4C".into(),
                ))
            }
        };

        // Allocate registers: r_i (iterator), r_end (inclusive bound), r_cmp, r_step, r_fwd
        let r_i = ctx.alloc_reg(iter_var)?;
        let r_end = ctx.alloc_temp()?;
        let r_cmp = ctx.alloc_temp()?;
        let r_step = ctx.alloc_temp()?;
        let r_fwd = ctx.alloc_temp()?; // Bool: true = forward, false = reverse

        // Initialize: r_i = start, r_end = end
        let r_start_tmp = self.compile_expr(start_expr, ctx)?;
        ctx.emit(Instruction::CopyReg(r_i, r_start_tmp));
        let r_end_tmp = self.compile_expr(end_expr, ctx)?;
        ctx.emit(Instruction::CopyReg(r_end, r_end_tmp));

        // Compute step magnitude (always positive)
        if let Some(step_e) = step_expr_opt {
            let r_step_tmp = self.compile_expr(step_e, ctx)?;
            ctx.emit(Instruction::CopyReg(r_step, r_step_tmp));
        } else {
            ctx.emit(Instruction::LoadInt(r_step, 1));
        }

        // Detect direction: r_fwd = (r_i <= r_end)  → Bool
        ctx.emit(Instruction::CmpLe(r_fwd, r_i, r_end));

        // Loop header: check exit condition using conditional branches
        // Range is INCLUSIVE: @ i:0..N → 0,1,...,N
        let loop_start = ctx.current_label();

        ctx.loop_stack.push(LoopCtx {
            break_patches: Vec::new(),
            continue_patches: Vec::new(),
            label: lp.label.clone(),
        });

        // Exit check:
        //   if r_fwd → exit when r_i > r_end
        //   else     → exit when r_i < r_end
        // JumpIfNot r_fwd, check_rev
        let fwd_branch_patch = ctx.emit(Instruction::JumpIfNot(r_fwd, 0));
        // Forward path: exit if r_i > r_end
        ctx.emit(Instruction::CmpGt(r_cmp, r_i, r_end));
        let fwd_exit_patch = ctx.emit(Instruction::JumpIf(r_cmp, 0));
        let skip_rev_patch = ctx.emit(Instruction::Jump(0)); // skip over reverse check
        // Reverse path:
        let check_rev_label = ctx.current_label();
        ctx.patch_jump(fwd_branch_patch, check_rev_label);
        ctx.emit(Instruction::CmpLt(r_cmp, r_i, r_end));
        let rev_exit_patch = ctx.emit(Instruction::JumpIf(r_cmp, 0));
        let body_label = ctx.current_label();
        ctx.patch_jump(skip_rev_patch, body_label);

        self.compile_block(&lp.body, ctx)?;

        // continue label = start of increment, so @> increments before re-checking
        let inc_label = ctx.current_label();

        // Increment/decrement:
        //   if r_fwd → r_i = r_i + r_step
        //   else     → r_i = r_i - r_step
        let inc_fwd_patch = ctx.emit(Instruction::JumpIfNot(r_fwd, 0));
        ctx.emit(Instruction::AddInt(r_i, r_i, r_step));
        let skip_sub_patch = ctx.emit(Instruction::Jump(0));
        let do_sub_label = ctx.current_label();
        ctx.patch_jump(inc_fwd_patch, do_sub_label);
        ctx.emit(Instruction::SubInt(r_i, r_i, r_step));
        let after_inc_label = ctx.current_label();
        ctx.patch_jump(skip_sub_patch, after_inc_label);

        // Back to loop_start
        ctx.emit(Instruction::Jump(loop_start));

        let loop_end = ctx.current_label();
        ctx.patch_jump(fwd_exit_patch, loop_end);
        ctx.patch_jump(rev_exit_patch, loop_end);

        let lctx = ctx.loop_stack.pop().unwrap();
        for pos in lctx.break_patches {
            ctx.patch_jump(pos, loop_end);
        }
        // @> jumps to increment so r_i advances before condition re-eval
        for pos in lctx.continue_patches {
            ctx.patch_jump(pos, inc_label);
        }
        Ok(())
    }

    fn compile_break(
        &mut self,
        b: &Break,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        if ctx.loop_stack.is_empty() {
            return Err(CompileError::BreakOutsideLoop);
        }
        let jump_pos = ctx.emit_jump_placeholder();
        // Find the innermost loop matching the label (or innermost if no label).
        let target = if let Some(lbl) = &b.label {
            ctx.loop_stack.iter_mut().rev()
                .find(|lctx| lctx.label.as_deref() == Some(lbl.as_str()))
        } else {
            ctx.loop_stack.last_mut()
        };
        match target {
            Some(lctx) => { lctx.break_patches.push(jump_pos); Ok(()) }
            None => Err(CompileError::Unsupported(format!("break label '{}' not found", b.label.as_deref().unwrap_or("?")))),
        }
    }

    fn compile_continue(
        &mut self,
        c: &Continue,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        if ctx.loop_stack.is_empty() {
            return Err(CompileError::ContinueOutsideLoop);
        }
        // Emit a placeholder; each loop type resolves the correct target
        // (range/foreach → increment label, infinite/while → loop_start).
        let jump_pos = ctx.emit_jump_placeholder();
        // Find the innermost loop matching the label (or innermost if no label).
        let target = if let Some(lbl) = &c.label {
            ctx.loop_stack.iter_mut().rev()
                .find(|lctx| lctx.label.as_deref() == Some(lbl.as_str()))
        } else {
            ctx.loop_stack.last_mut()
        };
        match target {
            Some(lctx) => { lctx.continue_patches.push(jump_pos); Ok(()) }
            None => Err(CompileError::Unsupported(format!("continue label '{}' not found", c.label.as_deref().unwrap_or("?")))),
        }
    }

    // ── Expression compilation ──────────────────────────────────────────────

    /// Compile an expression, returning the register that holds the result.
    fn compile_expr(
        &mut self,
        expr: &Expr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        match expr {
            Expr::Literal(lit) => self.compile_literal(lit, ctx),
            Expr::Identifier(id) => {
                if let Ok(r) = ctx.get_reg(&id.name) {
                    return Ok(r);
                }
                // Fall back to global constant inlining (module-level consts in function bodies)
                if let Some(mc) = self.global_consts.get(&id.name).cloned() {
                    let dst = ctx.alloc_temp()?;
                    let instr = match mc {
                        ModuleConst::Int(n) => Instruction::LoadInt(dst, n),
                        ModuleConst::Float(f) => { ctx.set_reg_type(dst, StaticType::Float); Instruction::LoadFloat(dst, f) }
                        ModuleConst::String(s) => { let idx = self.intern_string(&s); ctx.set_reg_type(dst, StaticType::String); Instruction::LoadStr(dst, idx) }
                        ModuleConst::Bool(b) => { ctx.set_reg_type(dst, StaticType::Bool); Instruction::LoadBool(dst, b) }
                        ModuleConst::Char(c) => { ctx.set_reg_type(dst, StaticType::Char); Instruction::LoadChar(dst, c) }
                    };
                    ctx.emit(instr);
                    return Ok(dst);
                }
                // Fall back to module global variable (mutable state shared across calls)
                if let Some(&gvar_idx) = self.global_var_map.get(&id.name) {
                    let dst = ctx.alloc_temp()?;
                    ctx.emit(Instruction::LoadGlobal(dst, gvar_idx));
                    return Ok(dst);
                }
                // In function bodies, defer to runtime (matches tree-walker behavior)
                if self.in_function_body {
                    let msg = format!("runtime error: undefined variable: '{}'", id.name);
                    let idx = self.intern_string(&msg);
                    let dst = ctx.alloc_temp()?;
                    ctx.emit(Instruction::RaiseError(idx));
                    return Ok(dst);
                }
                Err(CompileError::UndefinedVariable(id.name.clone()))
            }
            Expr::Binary(bin) => self.compile_binary(bin, ctx),
            Expr::Unary(un) => self.compile_unary(un, ctx),
            Expr::FunctionCall(call) => self.compile_call(call, ctx),
            Expr::Range(_) => Err(CompileError::Unsupported("range outside loop".into())),
            Expr::ArrayLiteral(arr) => self.compile_array_literal(arr, ctx),
            Expr::Tuple(t) => self.compile_tuple(t, ctx),
            Expr::NamedTuple(nt) => self.compile_named_tuple(nt, ctx),
            Expr::Index(idx) => self.compile_index(idx, ctx),
            Expr::MemberAccess(ma) => self.compile_member_access(ma, ctx),
            Expr::Match(m) => self.compile_match_expr(m, ctx),
            Expr::CollectionLength(cl) => self.compile_collection_length(cl, ctx),
            Expr::CollectionAppend(ca) => self.compile_collection_append(ca, ctx),
            Expr::CollectionInsert(ci) => self.compile_collection_insert(ci, ctx),
            Expr::CollectionRemoveValue(cv) => self.compile_collection_remove_value(cv, ctx),
            Expr::CollectionRemoveAll(ca) => self.compile_collection_remove_all(ca, ctx),
            Expr::CollectionRemoveAt(cr) => self.compile_collection_remove(cr, ctx),
            Expr::CollectionRemoveRange(cr) => self.compile_collection_remove_range(cr, ctx),
            Expr::CollectionFindAll(op) => {
                let r_coll = self.compile_expr(&op.collection, ctx)?;
                let r_val  = self.compile_expr(&op.value, ctx)?;
                let dst    = ctx.alloc_temp()?;
                ctx.emit(Instruction::StrFindPos(dst, r_coll, r_val));
                ctx.set_reg_type(dst, StaticType::Unknown);
                Ok(dst)
            }
            Expr::CollectionContains(cc) => self.compile_collection_contains(cc, ctx),
            Expr::CollectionUpdate(cu) => self.compile_collection_update(cu, ctx),
            Expr::CollectionSlice(cs) => self.compile_collection_slice(cs, ctx),
            Expr::CollectionMap(cm) => self.compile_collection_map(cm, ctx),
            Expr::CollectionFilter(cf) => self.compile_collection_filter(cf, ctx),
            Expr::CollectionReduce(cr2) => self.compile_collection_reduce(cr2, ctx),
            Expr::CollectionSortAsc(cs) => self.compile_collection_sort(cs, ctx),
            Expr::CollectionSortDesc(cs) => self.compile_collection_sort(cs, ctx),
            Expr::CollectionSortCustom(cs) => self.compile_collection_sort(cs, ctx),
            Expr::Lambda(lam) => self.compile_lambda(lam, ctx),
            Expr::NumericEval(ne) => {
                let r = self.compile_expr(&ne.expr, ctx)?;
                let dst = ctx.alloc_temp()?;
                ctx.emit(Instruction::NumericEval(dst, r));
                Ok(dst)
            }
            Expr::TypeMetadata(tm) => {
                // If the inner expr is an undefined identifier, treat as Unit (##_ type)
                // This matches tree-walker behavior: nonexistent#? → ("##_", 0, Unit)
                let r = if let Expr::Identifier(id) = tm.expr.as_ref() {
                    if ctx.get_reg(&id.name).is_err() && !self.global_consts.contains_key(&id.name) {
                        let tmp = ctx.alloc_temp()?;
                        ctx.emit(Instruction::LoadUnit(tmp));
                        tmp
                    } else {
                        self.compile_expr(&tm.expr, ctx)?
                    }
                } else {
                    self.compile_expr(&tm.expr, ctx)?
                };
                let dst = ctx.alloc_temp()?;
                ctx.emit(Instruction::TypeOf(dst, r));
                Ok(dst)
            }
            Expr::BaseConversion(bc) => {
                let r = self.compile_expr(&bc.expr, ctx)?;
                let dst = ctx.alloc_temp()?;
                let radix: u8 = match bc.prefix {
                    BasePrefix::Binary  => 2,
                    BasePrefix::Octal   => 8,
                    BasePrefix::Decimal => 10,
                    BasePrefix::Hex     => 16,
                };
                ctx.emit(Instruction::BaseConvert(dst, r, radix));
                Ok(dst)
            }
            Expr::BashExec(be) => self.compile_bash_exec(be, ctx),
            Expr::Format(fe) => self.compile_format(fe, ctx),
            Expr::Round(r) => {
                let src = self.compile_expr(&r.expr, ctx)?;
                let dst = ctx.alloc_temp()?;
                ctx.set_reg_type(dst, StaticType::Float);
                ctx.emit(Instruction::RoundFloat(dst, src, r.precision));
                Ok(dst)
            }
            Expr::Trunc(t) => {
                let src = self.compile_expr(&t.expr, ctx)?;
                let dst = ctx.alloc_temp()?;
                ctx.set_reg_type(dst, StaticType::Float);
                ctx.emit(Instruction::TruncFloat(dst, src, t.precision));
                Ok(dst)
            }
            Expr::ErrorCheck(ec) => {
                let src = self.compile_expr(&ec.expr, ctx)?;
                let dst = ctx.alloc_temp()?;
                ctx.emit(Instruction::IsError(dst, src));
                Ok(dst)
            }
            // ── 4F: Pipe operator (value |> callable(_, args)) ────────────────
            Expr::Pipe(pipe) => {
                // Evaluate the piped value first
                let r_val = self.compile_expr(&pipe.left, ctx)?;
                // Evaluate the callable
                let r_fn = self.compile_expr(&pipe.callable, ctx)?;
                // Build argument list: _ → r_val, Expr(e) → compile(e)
                let mut arg_regs = Vec::with_capacity(pipe.arguments.len());
                for arg in &pipe.arguments {
                    match arg {
                        zymbol_ast::PipeArg::Placeholder => arg_regs.push(r_val),
                        zymbol_ast::PipeArg::Expr(e) => {
                            arg_regs.push(self.compile_expr(e, ctx)?);
                        }
                    }
                }
                let dst = ctx.alloc_temp()?;
                ctx.emit(Instruction::CallDynamic(dst, r_fn, arg_regs));
                Ok(dst)
            }
            // ── 4H: Execute expression </ file.zy /> → Execute instruction ──
            Expr::Execute(exec) => {
                // Resolve path relative to base_dir (same as WT's eval_execute).
                // Absolute paths are used as-is; everything else is joined to base_dir.
                let abs_path = if exec.path.starts_with('/') {
                    exec.path.clone()
                } else if let Some(ref base) = self.base_dir {
                    base.join(&exec.path).to_string_lossy().to_string()
                } else {
                    exec.path.clone()
                };
                // Build: zymbol run <absolute-path>
                let cmd = format!("zymbol run \"{}\"", abs_path);
                let idx = self.intern_string(&cmd);
                let dst = ctx.alloc_temp()?;
                ctx.emit(Instruction::Execute(dst, vec![BuildPart::Lit(idx)]));
                ctx.set_reg_type(dst, StaticType::String);
                Ok(dst)
            }
            // ── String modification operators ──────────────────────────────
            Expr::StringReplace(op) => {
                let r_str = self.compile_expr(&op.string, ctx)?;
                let r_pat = self.compile_expr(&op.pattern, ctx)?;
                let r_rep = self.compile_expr(&op.replacement, ctx)?;
                let dst   = ctx.alloc_temp()?;
                if let Some(count_expr) = &op.count {
                    let r_n = self.compile_expr(count_expr, ctx)?;
                    ctx.emit(Instruction::StrReplaceN(dst, r_str, r_pat, r_rep, r_n));
                } else {
                    ctx.emit(Instruction::StrReplace(dst, r_str, r_pat, r_rep));
                }
                ctx.set_reg_type(dst, StaticType::String);
                Ok(dst)
            }

            Expr::StringSplit(op) => {
                let r_str = self.compile_expr(&op.string, ctx)?;
                let r_del = self.compile_expr(&op.delimiter, ctx)?;
                let dst   = ctx.alloc_temp()?;
                ctx.emit(Instruction::StrSplit(dst, r_str, r_del));
                Ok(dst)
            }

            Expr::ConcatBuild(op) => {
                let r_base = self.compile_expr(&op.base, ctx)?;
                let mut item_regs = Vec::with_capacity(op.items.len());
                for item in &op.items {
                    item_regs.push(self.compile_expr(item, ctx)?);
                }
                let dst = ctx.alloc_temp()?;
                ctx.emit(Instruction::ConcatBuild(dst, r_base, item_regs));
                Ok(dst)
            }

            Expr::NumericCast(cast) => {
                let r_src = self.compile_expr(&cast.expr, ctx)?;
                let dst = ctx.alloc_temp()?;
                match cast.kind {
                    CastKind::ToFloat => {
                        ctx.emit(Instruction::IntToFloat(dst, r_src));
                        ctx.set_reg_type(dst, StaticType::Float);
                    }
                    CastKind::ToIntRound => {
                        ctx.emit(Instruction::FloatToIntRound(dst, r_src));
                        ctx.set_reg_type(dst, StaticType::Int);
                    }
                    CastKind::ToIntTrunc => {
                        ctx.emit(Instruction::FloatToIntTrunc(dst, r_src));
                        ctx.set_reg_type(dst, StaticType::Int);
                    }
                }
                Ok(dst)
            }

            Expr::DeepIndex(di) => self.compile_deep_index(di, ctx),
            Expr::FlatExtract(fe) => self.compile_flat_extract(fe, ctx),
            Expr::StructuredExtract(se) => self.compile_structured_extract(se, ctx),

            _ => Err(CompileError::Unsupported(format!(
                "expression {:?}", std::mem::discriminant(expr)
            ))),
        }
    }

    // ── Nav-index helpers ────────────────────────────────────────────────────

    /// Compile a single nav-path starting from `r_base` register.
    /// Returns the register holding the final value.
    /// Returns Err if any step uses a range (range steps require dynamic loops).
    /// Scalar-only nav path: chain of plain ArrayGet, returns final register.
    /// Used by DeepIndexExpr (which guarantees no range steps by AST design).
    fn compile_nav_path(
        &mut self,
        r_base: Reg,
        path: &zymbol_ast::NavPath,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let mut current = r_base;
        for step in &path.steps {
            let r_idx = self.compile_expr(&step.index, ctx)?;
            let dst = ctx.alloc_temp()?;
            ctx.emit(Instruction::ArrayGet(dst, current, r_idx));
            current = dst;
        }
        Ok(current)
    }

    /// Compile a nav path segment, pushing all result value(s) into `r_collect`.
    /// Handles range steps by emitting an inline counted loop; recurses for
    /// additional steps after the range (fan-out semantics).
    fn compile_nav_path_collect(
        &mut self,
        r_base: Reg,
        steps: &[zymbol_ast::NavStep],
        r_collect: Reg,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        // Find first step that carries a range (..)
        let range_pos = steps.iter().position(|s| s.range_end.is_some());

        match range_pos {
            None => {
                // All plain steps: descend and push single value
                let mut current = r_base;
                for step in steps {
                    let r_idx = self.compile_expr(&step.index, ctx)?;
                    let dst = ctx.alloc_temp()?;
                    ctx.emit(Instruction::ArrayGet(dst, current, r_idx));
                    current = dst;
                }
                ctx.emit(Instruction::ArrayPush(r_collect, current));
                Ok(())
            }
            Some(k) => {
                // Apply plain prefix steps 0..k
                let mut r_mid = r_base;
                for step in &steps[..k] {
                    let r_idx = self.compile_expr(&step.index, ctx)?;
                    let dst = ctx.alloc_temp()?;
                    ctx.emit(Instruction::ArrayGet(dst, r_mid, r_idx));
                    r_mid = dst;
                }

                // Range step at k: loop i from start..=end (1-based, inclusive)
                let range_step = &steps[k];
                let r_start_tmp = self.compile_expr(&range_step.index, ctx)?;
                let r_end_tmp   = self.compile_expr(range_step.range_end.as_ref().unwrap(), ctx)?;

                let r_i   = ctx.alloc_temp()?;
                let r_end = ctx.alloc_temp()?;
                let r_cmp = ctx.alloc_temp()?;
                let r_one = ctx.alloc_temp()?;
                ctx.emit(Instruction::CopyReg(r_i, r_start_tmp));
                ctx.emit(Instruction::CopyReg(r_end, r_end_tmp));
                ctx.emit(Instruction::LoadInt(r_one, 1));

                let loop_start = ctx.current_label();
                ctx.emit(Instruction::CmpGt(r_cmp, r_i, r_end));
                let exit_patch = ctx.emit(Instruction::JumpIf(r_cmp, 0));

                // Get element at loop counter r_i
                let r_elem = ctx.alloc_temp()?;
                ctx.emit(Instruction::ArrayGet(r_elem, r_mid, r_i));

                // Recurse: apply remaining steps steps[k+1..] to r_elem
                self.compile_nav_path_collect(r_elem, &steps[k + 1..], r_collect, ctx)?;

                ctx.emit(Instruction::AddInt(r_i, r_i, r_one));
                ctx.emit(Instruction::Jump(loop_start));

                let loop_end = ctx.current_label();
                ctx.patch_jump(exit_patch, loop_end);
                Ok(())
            }
        }
    }

    fn compile_deep_index(
        &mut self,
        di: &DeepIndexExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let r_arr = self.compile_expr(&di.array, ctx)?;
        self.compile_nav_path(r_arr, &di.path, ctx)
    }

    fn compile_flat_extract(
        &mut self,
        fe: &FlatExtractExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let r_arr    = self.compile_expr(&fe.array, ctx)?;
        let r_result = ctx.alloc_temp()?;
        ctx.emit(Instruction::NewArray(r_result));
        for path in &fe.paths {
            self.compile_nav_path_collect(r_arr, &path.steps, r_result, ctx)?;
        }
        Ok(r_result)
    }

    fn compile_structured_extract(
        &mut self,
        se: &StructuredExtractExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let r_arr   = self.compile_expr(&se.array, ctx)?;
        let r_outer = ctx.alloc_temp()?;
        ctx.emit(Instruction::NewArray(r_outer));
        for group in &se.groups {
            let r_inner = ctx.alloc_temp()?;
            ctx.emit(Instruction::NewArray(r_inner));
            for path in &group.paths {
                self.compile_nav_path_collect(r_arr, &path.steps, r_inner, ctx)?;
            }
            ctx.emit(Instruction::ArrayPush(r_outer, r_inner));
        }
        Ok(r_outer)
    }

    fn compile_literal(
        &mut self,
        lit: &LiteralExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let dst = ctx.alloc_temp()?;
        match &lit.value {
            Literal::Int(n) => {
                ctx.emit(Instruction::LoadInt(dst, *n));
                ctx.set_reg_type(dst, StaticType::Int);
            }
            Literal::Float(n) => {
                ctx.emit(Instruction::LoadFloat(dst, *n));
                ctx.set_reg_type(dst, StaticType::Float);
            }
            Literal::Bool(b) => {
                ctx.emit(Instruction::LoadBool(dst, *b));
                ctx.set_reg_type(dst, StaticType::Bool);
            }
            Literal::String(s) => {
                // resolve \x01 sentinel (from \{ escape) to literal {
                let resolved = s.replace('\x01', "{");
                let idx = self.intern_string(&resolved);
                ctx.emit(Instruction::LoadStr(dst, idx));
                ctx.set_reg_type(dst, StaticType::String);
            }
            Literal::InterpolatedString(s) => {
                // {var} interpolation — use BuildStr; sentinel resolved after interpolation
                if s.contains('{') {
                    return self.compile_interpolated_string(s, ctx);
                }
                // No real {var} — just sentinel resolution
                let resolved = s.replace('\x01', "{");
                let idx = self.intern_string(&resolved);
                ctx.emit(Instruction::LoadStr(dst, idx));
                ctx.set_reg_type(dst, StaticType::String);
            }
            Literal::Char(c) => {
                ctx.emit(Instruction::LoadChar(dst, *c));
                ctx.set_reg_type(dst, StaticType::Char);
            }
        }
        Ok(dst)
    }

    fn compile_binary(
        &mut self,
        bin: &zymbol_ast::BinaryExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        // Short-circuit logical ops
        match bin.op {
            BinaryOp::And => return self.compile_and(bin, ctx),
            BinaryOp::Or => return self.compile_or(bin, ctx),
            _ => {}
        }

        // IMM fast path: right operand is a small integer literal
        // Eliminates a LoadInt + temp register for common patterns like `n - 1`, `n <= 1`
        if let Expr::Literal(lit) = bin.right.as_ref() {
            if let Literal::Int(imm) = lit.value {
                if (i32::MIN as i64..=i32::MAX as i64).contains(&imm) {
                    let imm = imm as i32;
                    let r_l = self.compile_expr(&bin.left, ctx)?;
                    let ty_l = ctx.get_reg_type(r_l);
                    if ty_l != StaticType::Float && ty_l != StaticType::String {
                        let dst = ctx.alloc_temp()?;
                        let opt_instr: Option<Instruction> = match bin.op {
                            BinaryOp::Add => { ctx.set_reg_type(dst, StaticType::Int); Some(Instruction::AddIntImm(dst, r_l, imm)) }
                            BinaryOp::Sub => { ctx.set_reg_type(dst, StaticType::Int); Some(Instruction::SubIntImm(dst, r_l, imm)) }
                            BinaryOp::Mul => { ctx.set_reg_type(dst, StaticType::Int); Some(Instruction::MulIntImm(dst, r_l, imm)) }
                            BinaryOp::Eq  => { ctx.set_reg_type(dst, StaticType::Bool); Some(Instruction::CmpEqImm(dst, r_l, imm)) }
                            BinaryOp::Neq => { ctx.set_reg_type(dst, StaticType::Bool); Some(Instruction::CmpNeImm(dst, r_l, imm)) }
                            BinaryOp::Lt  => { ctx.set_reg_type(dst, StaticType::Bool); Some(Instruction::CmpLtImm(dst, r_l, imm)) }
                            BinaryOp::Le  => { ctx.set_reg_type(dst, StaticType::Bool); Some(Instruction::CmpLeImm(dst, r_l, imm)) }
                            BinaryOp::Gt  => { ctx.set_reg_type(dst, StaticType::Bool); Some(Instruction::CmpGtImm(dst, r_l, imm)) }
                            BinaryOp::Ge  => { ctx.set_reg_type(dst, StaticType::Bool); Some(Instruction::CmpGeImm(dst, r_l, imm)) }
                            _ => None,
                        };
                        if let Some(instr) = opt_instr {
                            ctx.emit(instr);
                            return Ok(dst);
                        }
                        // Fall through for Div, Mod, Pow etc. — need the r_r register
                        // (we already alloc_temp'd dst, compensate by continuing normally)
                        // Reuse dst as r_r by loading the imm into it, then emit regular op
                        ctx.emit(Instruction::LoadInt(dst, imm as i64));
                        let dst2 = ctx.alloc_temp()?;
                        let instr = match bin.op {
                            BinaryOp::Div => Instruction::DivInt(dst2, r_l, dst),
                            BinaryOp::Mod => Instruction::ModInt(dst2, r_l, dst),
                            BinaryOp::Pow => Instruction::PowInt(dst2, r_l, dst),
                            _ => unreachable!(),
                        };
                        ctx.emit(instr);
                        return Ok(dst2);
                    }
                }
            }
        }

        let r_l = self.compile_expr(&bin.left, ctx)?;
        let r_r = self.compile_expr(&bin.right, ctx)?;
        let dst = ctx.alloc_temp()?;

        let ty_l = ctx.get_reg_type(r_l);
        let ty_r = ctx.get_reg_type(r_r);
        let is_float = ty_l == StaticType::Float || ty_r == StaticType::Float;
        let is_string = ty_l == StaticType::String || ty_r == StaticType::String
            || ty_l == StaticType::Char || ty_r == StaticType::Char;

        let instr = match bin.op {
            BinaryOp::Concat => {
                // Juxtaposition concatenation — always string concat
                ctx.set_reg_type(dst, StaticType::String);
                Instruction::ConcatStr(dst, r_l, r_r)
            }
            BinaryOp::Add => {
                if is_float {
                    ctx.set_reg_type(dst, StaticType::Float);
                    Instruction::AddFloat(dst, r_l, r_r)
                } else {
                    ctx.set_reg_type(dst, StaticType::Int);
                    Instruction::AddInt(dst, r_l, r_r)
                }
            }
            BinaryOp::Sub => if is_float { ctx.set_reg_type(dst, StaticType::Float); Instruction::SubFloat(dst, r_l, r_r) } else { Instruction::SubInt(dst, r_l, r_r) },
            BinaryOp::Mul => if is_float { ctx.set_reg_type(dst, StaticType::Float); Instruction::MulFloat(dst, r_l, r_r) } else { Instruction::MulInt(dst, r_l, r_r) },
            BinaryOp::Div => {
                if is_string {
                    // String split: "a,b" / ',' → Array
                    ctx.set_reg_type(dst, StaticType::Unknown); // Array type
                    Instruction::StrSplit(dst, r_l, r_r)
                } else if is_float {
                    ctx.set_reg_type(dst, StaticType::Float);
                    Instruction::DivFloat(dst, r_l, r_r)
                } else {
                    Instruction::DivInt(dst, r_l, r_r)
                }
            }
            BinaryOp::Mod => Instruction::ModInt(dst, r_l, r_r),
            BinaryOp::Pow => if is_float { ctx.set_reg_type(dst, StaticType::Float); Instruction::PowFloat(dst, r_l, r_r) } else { Instruction::PowInt(dst, r_l, r_r) },
            BinaryOp::Eq => Instruction::CmpEq(dst, r_l, r_r),
            BinaryOp::Neq => Instruction::CmpNe(dst, r_l, r_r),
            BinaryOp::Lt => Instruction::CmpLt(dst, r_l, r_r),
            BinaryOp::Le => Instruction::CmpLe(dst, r_l, r_r),
            BinaryOp::Gt => Instruction::CmpGt(dst, r_l, r_r),
            BinaryOp::Ge => Instruction::CmpGe(dst, r_l, r_r),
            BinaryOp::And | BinaryOp::Or => unreachable!(),
            BinaryOp::Pipe => {
                return Err(CompileError::Unsupported("pipe (|>) — VM Fase 4C".into()))
            }
            BinaryOp::Comma => {
                Instruction::ConcatStr(dst, r_l, r_r)
            }
            BinaryOp::Range => {
                return Err(CompileError::Unsupported("range (..) in expression — VM Fase 4C".into()))
            }
        };
        ctx.emit(instr);
        Ok(dst)
    }

    fn compile_and(
        &mut self,
        bin: &zymbol_ast::BinaryExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let dst = ctx.alloc_temp()?;
        let r_l = self.compile_expr(&bin.left, ctx)?;
        // Short-circuit: if left is false, skip right
        let skip = ctx.emit_jump_if_not_placeholder(r_l);
        let r_r = self.compile_expr(&bin.right, ctx)?;
        ctx.emit(Instruction::And(dst, r_l, r_r));
        // If skipped (short-circuit), dst = false
        let end_jump = ctx.emit_jump_placeholder();
        let false_label = ctx.current_label();
        ctx.patch_jump(skip, false_label);
        ctx.emit(Instruction::LoadBool(dst, false));
        let end_label = ctx.current_label();
        ctx.patch_jump(end_jump, end_label);
        Ok(dst)
    }

    fn compile_or(
        &mut self,
        bin: &zymbol_ast::BinaryExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let dst = ctx.alloc_temp()?;
        let r_l = self.compile_expr(&bin.left, ctx)?;
        // Short-circuit: if left is true, skip right
        let skip = ctx.emit(Instruction::JumpIf(r_l, 0)); // placeholder
        let r_r = self.compile_expr(&bin.right, ctx)?;
        ctx.emit(Instruction::Or(dst, r_l, r_r));
        let end_jump = ctx.emit_jump_placeholder();
        let true_label = ctx.current_label();
        ctx.patch_jump(skip, true_label);
        ctx.emit(Instruction::LoadBool(dst, true));
        let end_label = ctx.current_label();
        ctx.patch_jump(end_jump, end_label);
        Ok(dst)
    }

    fn compile_unary(
        &mut self,
        un: &zymbol_ast::UnaryExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let r = self.compile_expr(&un.operand, ctx)?;
        let dst = ctx.alloc_temp()?;
        let instr = match un.op {
            UnaryOp::Neg => {
                if ctx.get_reg_type(r) == StaticType::Float {
                    ctx.set_reg_type(dst, StaticType::Float);
                    Instruction::NegFloat(dst, r)
                } else {
                    Instruction::NegInt(dst, r)
                }
            }
            UnaryOp::Not => Instruction::Not(dst, r),
            UnaryOp::Pos => {
                ctx.emit(Instruction::CopyReg(dst, r));
                return Ok(dst);
            }
        };
        ctx.emit(instr);
        Ok(dst)
    }

    fn compile_call(
        &mut self,
        call: &zymbol_ast::FunctionCallExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        // Check if the callable is a known static function name or a module call (alias::func)
        let maybe_func_idx = match call.callable.as_ref() {
            Expr::Identifier(id) => self.function_index.get(&id.name)
                .or_else(|| self.module_scope.get(&id.name))
                .copied(),
            Expr::MemberAccess(ma) => {
                if let Expr::Identifier(obj) = ma.object.as_ref() {
                    // Module call: obj::func → look up "obj::func" in function_index
                    let qualified = format!("{}::{}", obj.name, ma.field);
                    self.function_index.get(&qualified).copied()
                } else {
                    None
                }
            }
            _ => None,
        };

        // Compile arguments
        let mut arg_regs = Vec::with_capacity(call.arguments.len());
        for arg in &call.arguments {
            let r = self.compile_expr(arg, ctx)?;
            arg_regs.push(r);
        }
        let dst = ctx.alloc_temp()?;

        if let Some(func_idx) = maybe_func_idx {
            // Emit SetupOutputWriteback if this function has output params
            if let Some(out_flags) = self.output_param_map.get(&func_idx).cloned() {
                let mut pairs: Vec<(u16, Reg)> = Vec::new();
                for (i, is_out) in out_flags.iter().enumerate() {
                    if *is_out && i < call.arguments.len() {
                        // The arg register IS the caller's variable register (for identifiers)
                        if matches!(&call.arguments[i], Expr::Identifier(_)) {
                            pairs.push((i as u16, arg_regs[i]));
                        }
                    }
                }
                if !pairs.is_empty() {
                    ctx.emit(Instruction::SetupOutputWriteback(pairs));
                }
            }
            ctx.emit(Instruction::Call(dst, func_idx, arg_regs));
        } else {
            // Dynamic call: callable is a variable holding a Function value
            let callee_reg = self.compile_expr(call.callable.as_ref(), ctx)?;
            ctx.emit(Instruction::CallDynamic(dst, callee_reg, arg_regs));
        }
        Ok(dst)
    }

    fn compile_array_literal(
        &mut self,
        arr: &zymbol_ast::ArrayLiteralExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let dst = ctx.alloc_temp()?;
        ctx.emit(Instruction::NewArray(dst));
        for elem in &arr.elements {
            let r = self.compile_expr(elem, ctx)?;
            ctx.emit(Instruction::ArrayPush(dst, r));
        }
        Ok(dst)
    }

    // ── Module constant evaluation ────────────────────────────────────────────

    /// Try to evaluate a literal expression to a ModuleConst at compile time.
    fn eval_const_expr(expr: &Expr) -> Option<ModuleConst> {
        match expr {
            Expr::Literal(lit) => match &lit.value {
                Literal::Int(n) => Some(ModuleConst::Int(*n as i64)),
                Literal::Float(f) => Some(ModuleConst::Float(*f)),
                Literal::String(s) | Literal::InterpolatedString(s) => Some(ModuleConst::String(s.replace('\x01', "{"))),
                Literal::Bool(b) => Some(ModuleConst::Bool(*b)),
                Literal::Char(c) => Some(ModuleConst::Char(*c)),
            },
            Expr::Unary(un) if un.op == UnaryOp::Neg => {
                if let Expr::Literal(lit) = un.operand.as_ref() {
                    match &lit.value {
                        Literal::Int(n) => Some(ModuleConst::Int(-(*n as i64))),
                        Literal::Float(f) => Some(ModuleConst::Float(-f)),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    // ── String pool ──────────────────────────────────────────────────────────

    fn intern_string(&mut self, s: &str) -> StrIdx {
        if let Some(pos) = self.string_pool.iter().position(|p| p == s) {
            return pos as StrIdx;
        }
        let idx = self.string_pool.len() as StrIdx;
        self.string_pool.push(s.to_string());
        idx
    }

    // ── 4C: Tuples ───────────────────────────────────────────────────────────

    fn compile_tuple(
        &mut self,
        t: &zymbol_ast::TupleExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let mut regs = Vec::with_capacity(t.elements.len());
        for elem in &t.elements {
            regs.push(self.compile_expr(elem, ctx)?);
        }
        let dst = ctx.alloc_temp()?;
        ctx.emit(Instruction::MakeTuple(dst, regs));
        Ok(dst)
    }

    fn compile_named_tuple(
        &mut self,
        nt: &zymbol_ast::NamedTupleExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let mut names = Vec::with_capacity(nt.fields.len());
        let mut regs = Vec::with_capacity(nt.fields.len());
        for (name, val_expr) in &nt.fields {
            let name_idx = self.intern_string(name);
            let r = self.compile_expr(val_expr, ctx)?;
            names.push(name_idx);
            regs.push(r);
        }
        let dst = ctx.alloc_temp()?;
        ctx.emit(Instruction::MakeNamedTuple(dst, names, regs));
        Ok(dst)
    }

    // ── 4C: Index + member access ─────────────────────────────────────────────

    fn compile_index(
        &mut self,
        idx: &zymbol_ast::IndexExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let r_arr = self.compile_expr(&idx.array, ctx)?;
        let r_idx = self.compile_expr(&idx.index, ctx)?;
        let dst = ctx.alloc_temp()?;
        ctx.emit(Instruction::ArrayGet(dst, r_arr, r_idx));
        Ok(dst)
    }

    fn compile_member_access(
        &mut self,
        ma: &zymbol_ast::MemberAccessExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        // Check if this is a module constant access (alias.CONST_NAME)
        if let Expr::Identifier(obj) = ma.object.as_ref() {
            let key = format!("{}.{}", obj.name, ma.field);
            if let Some(mc) = self.module_constants.get(&key).cloned() {
                let dst = ctx.alloc_temp()?;
                let instr = match mc {
                    ModuleConst::Int(n) => Instruction::LoadInt(dst, n),
                    ModuleConst::Float(f) => {
                        ctx.set_reg_type(dst, StaticType::Float);
                        Instruction::LoadFloat(dst, f)
                    }
                    ModuleConst::String(s) => {
                        let idx = self.intern_string(&s);
                        ctx.set_reg_type(dst, StaticType::String);
                        Instruction::LoadStr(dst, idx)
                    }
                    ModuleConst::Bool(b) => {
                        ctx.set_reg_type(dst, StaticType::Bool);
                        Instruction::LoadBool(dst, b)
                    }
                    ModuleConst::Char(c) => {
                        ctx.set_reg_type(dst, StaticType::Char);
                        Instruction::LoadChar(dst, c)
                    }
                };
                ctx.emit(instr);
                return Ok(dst);
            }
        }
        let r_obj = self.compile_expr(&ma.object, ctx)?;
        let field_idx = self.intern_string(&ma.field);
        let dst = ctx.alloc_temp()?;
        ctx.emit(Instruction::NamedTupleGet(dst, r_obj, field_idx));
        Ok(dst)
    }

    // ── 4C: Match ────────────────────────────────────────────────────────────

    fn compile_match_expr(
        &mut self,
        m: &zymbol_ast::MatchExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let r_sub = self.compile_expr(&m.scrutinee, ctx)?;
        let dst = ctx.alloc_temp()?;
        ctx.emit(Instruction::LoadUnit(dst)); // default result

        let mut end_patches: Vec<usize> = Vec::new();

        for case in &m.cases {
            match &case.pattern {
                Pattern::Wildcard(_) => {
                    // Compile body, store to dst, jump to end
                    if let Some(val) = &case.value {
                        let r = self.compile_expr(val, ctx)?;
                        ctx.emit(Instruction::CopyReg(dst, r));
                    } else if let Some(block) = &case.block {
                        self.compile_block(block, ctx)?;
                    }
                    let j = ctx.emit_jump_placeholder();
                    end_patches.push(j);
                    break; // Wildcard is always last
                }
                Pattern::Literal(lit, _) => {
                    let skip_patch = match lit {
                        zymbol_common::Literal::Int(n) => {
                            // Emit CmpEqImm + JumpIfNot
                            let r_cmp = ctx.alloc_temp()?;
                            ctx.emit(Instruction::CmpEqImm(r_cmp, r_sub, *n as i32));
                            ctx.emit_jump_if_not_placeholder(r_cmp)
                        }
                        zymbol_common::Literal::String(s) | zymbol_common::Literal::InterpolatedString(s) => {
                            let resolved = s.replace('\x01', "{");
                            let idx = self.intern_string(&resolved);
                            let body_label = ctx.current_label() + 2; // skip to MatchStr body
                            let _ms_pos = ctx.emit(Instruction::MatchStr(r_sub, idx, body_label as Label));
                            ctx.emit_jump_placeholder() // will patch to skip
                        }
                        zymbol_common::Literal::Bool(b) => {
                            let r_cmp = ctx.alloc_temp()?;
                            ctx.emit(Instruction::LoadBool(r_cmp, *b));
                            let r_eq = ctx.alloc_temp()?;
                            ctx.emit(Instruction::CmpEq(r_eq, r_sub, r_cmp));
                            ctx.emit_jump_if_not_placeholder(r_eq)
                        }
                        zymbol_common::Literal::Char(c) => {
                            let r_c = ctx.alloc_temp()?;
                            ctx.emit(Instruction::LoadChar(r_c, *c));
                            let r_eq = ctx.alloc_temp()?;
                            ctx.emit(Instruction::CmpEq(r_eq, r_sub, r_c));
                            ctx.emit_jump_if_not_placeholder(r_eq)
                        }
                        _ => ctx.emit_jump_placeholder(), // unsupported, always skip
                    };
                    // Body
                    if let Some(val) = &case.value {
                        let r = self.compile_expr(val, ctx)?;
                        ctx.emit(Instruction::CopyReg(dst, r));
                    } else if let Some(block) = &case.block {
                        self.compile_block(block, ctx)?;
                    }
                    let j = ctx.emit_jump_placeholder();
                    end_patches.push(j);
                    let next_case = ctx.current_label();
                    ctx.patch_jump(skip_patch, next_case);
                }
                Pattern::Range(start, end_expr, _) => {
                    // Range pattern: lo..hi
                    let lo = if let Expr::Literal(l) = start.as_ref() {
                        if let Literal::Int(n) = l.value { n }
                        else { return Err(CompileError::Unsupported("non-int range in match".into())); }
                    } else { return Err(CompileError::Unsupported("dynamic range in match".into())); };
                    let hi = if let Expr::Literal(l) = end_expr.as_ref() {
                        if let Literal::Int(n) = l.value { n }
                        else { return Err(CompileError::Unsupported("non-int range in match".into())); }
                    } else { return Err(CompileError::Unsupported("dynamic range in match".into())); };

                    let body_label = (ctx.current_label() + 2) as Label;
                    ctx.emit(Instruction::MatchRange(r_sub, lo, hi, body_label));
                    let skip_patch = ctx.emit_jump_placeholder();
                    // Body
                    if let Some(val) = &case.value {
                        let r = self.compile_expr(val, ctx)?;
                        ctx.emit(Instruction::CopyReg(dst, r));
                    } else if let Some(block) = &case.block {
                        self.compile_block(block, ctx)?;
                    }
                    let j = ctx.emit_jump_placeholder();
                    end_patches.push(j);
                    let next_case = ctx.current_label();
                    ctx.patch_jump(skip_patch, next_case);
                }
                Pattern::Comparison(op, expr, _) => {
                    // Comparison pattern: implicit scrutinee op rhs
                    let r_rhs = self.compile_expr(expr, ctx)?;
                    let r_cmp = ctx.alloc_temp()?;
                    let instr = match op {
                        BinaryOp::Lt  => Instruction::CmpLt(r_cmp, r_sub, r_rhs),
                        BinaryOp::Gt  => Instruction::CmpGt(r_cmp, r_sub, r_rhs),
                        BinaryOp::Le  => Instruction::CmpLe(r_cmp, r_sub, r_rhs),
                        BinaryOp::Ge  => Instruction::CmpGe(r_cmp, r_sub, r_rhs),
                        BinaryOp::Eq  => Instruction::CmpEq(r_cmp, r_sub, r_rhs),
                        BinaryOp::Neq => Instruction::CmpNe(r_cmp, r_sub, r_rhs),
                        _ => return Err(CompileError::Unsupported(
                            format!("unsupported op {:?} in comparison pattern", op)
                        )),
                    };
                    ctx.emit(instr);
                    let skip_patch = ctx.emit_jump_if_not_placeholder(r_cmp);
                    if let Some(val) = &case.value {
                        let r = self.compile_expr(val, ctx)?;
                        ctx.emit(Instruction::CopyReg(dst, r));
                    } else if let Some(block) = &case.block {
                        self.compile_block(block, ctx)?;
                    }
                    let j = ctx.emit_jump_placeholder();
                    end_patches.push(j);
                    let next_case = ctx.current_label();
                    ctx.patch_jump(skip_patch, next_case);
                }
                Pattern::Ident(name, _) => {
                    // Load the variable; if array → containment, else → equality
                    let r_var = if let Ok(r) = ctx.get_reg(name) {
                        r
                    } else if let Some(mc) = self.global_consts.get(name).cloned() {
                        let r = ctx.alloc_temp()?;
                        let instr = match mc {
                            ModuleConst::Int(n)    => Instruction::LoadInt(r, n),
                            ModuleConst::Float(f)  => { ctx.set_reg_type(r, StaticType::Float); Instruction::LoadFloat(r, f) }
                            ModuleConst::String(s) => { let idx = self.intern_string(&s); ctx.set_reg_type(r, StaticType::String); Instruction::LoadStr(r, idx) }
                            ModuleConst::Bool(b)   => { ctx.set_reg_type(r, StaticType::Bool); Instruction::LoadBool(r, b) }
                            ModuleConst::Char(c)   => { ctx.set_reg_type(r, StaticType::Char); Instruction::LoadChar(r, c) }
                        };
                        ctx.emit(instr);
                        r
                    } else if let Some(&gvar_idx) = self.global_var_map.get(name) {
                        let r = ctx.alloc_temp()?;
                        ctx.emit(Instruction::LoadGlobal(r, gvar_idx));
                        r
                    } else {
                        return Err(CompileError::UndefinedVariable(name.clone()));
                    };
                    // Runtime dispatch: array variable → containment check, scalar → equality
                    let r_is_arr = ctx.alloc_temp()?;
                    ctx.emit(Instruction::IsArray(r_is_arr, r_var));
                    let patch_to_eq = ctx.emit_jump_if_not_placeholder(r_is_arr);
                    // Array branch: ArrayContains(r_cmp, r_var, r_sub)
                    let r_cmp = ctx.alloc_temp()?;
                    ctx.emit(Instruction::ArrayContains(r_cmp, r_var, r_sub));
                    let patch_arr_skip = ctx.emit_jump_placeholder(); // jump over eq branch
                    // Scalar branch:
                    let eq_label = ctx.current_label();
                    ctx.patch_jump(patch_to_eq, eq_label);
                    ctx.emit(Instruction::CmpEq(r_cmp, r_sub, r_var));
                    // Merge point:
                    let merge_label = ctx.current_label();
                    ctx.patch_jump(patch_arr_skip, merge_label);
                    let skip_patch = ctx.emit_jump_if_not_placeholder(r_cmp);
                    if let Some(val) = &case.value {
                        let r = self.compile_expr(val, ctx)?;
                        ctx.emit(Instruction::CopyReg(dst, r));
                    } else if let Some(block) = &case.block {
                        self.compile_block(block, ctx)?;
                    }
                    let j = ctx.emit_jump_placeholder();
                    end_patches.push(j);
                    let next_case = ctx.current_label();
                    ctx.patch_jump(skip_patch, next_case);
                }
                Pattern::List(patterns, _) => {
                    // Runtime dual dispatch: structural for array scrutinee, containment for scalar
                    let r_is_arr = ctx.alloc_temp()?;
                    ctx.emit(Instruction::IsArray(r_is_arr, r_sub));
                    let patch_to_contain = ctx.emit_jump_if_not_placeholder(r_is_arr);

                    // === Structural path (scrutinee is array) ===
                    let mut struct_skip_patches: Vec<usize> = Vec::new();
                    let r_len = ctx.alloc_temp()?;
                    ctx.emit(Instruction::ArrayLen(r_len, r_sub));
                    let r_ok = ctx.alloc_temp()?;
                    ctx.emit(Instruction::CmpEqImm(r_ok, r_len, patterns.len() as i32));
                    struct_skip_patches.push(ctx.emit_jump_if_not_placeholder(r_ok));

                    for (i, sub_pat) in patterns.iter().enumerate() {
                        match sub_pat {
                            Pattern::Wildcard(_) => {}
                            Pattern::Literal(lit, _) => {
                                let r_idx = ctx.alloc_temp()?;
                                ctx.emit(Instruction::LoadInt(r_idx, (i + 1) as i64));
                                let r_elem = ctx.alloc_temp()?;
                                ctx.emit(Instruction::ArrayGet(r_elem, r_sub, r_idx));
                                match lit {
                                    zymbol_common::Literal::Int(n) => {
                                        let r_cmp = ctx.alloc_temp()?;
                                        ctx.emit(Instruction::CmpEqImm(r_cmp, r_elem, *n as i32));
                                        struct_skip_patches.push(ctx.emit_jump_if_not_placeholder(r_cmp));
                                    }
                                    zymbol_common::Literal::String(s) | zymbol_common::Literal::InterpolatedString(s) => {
                                        let resolved = s.replace('\x01', "{");
                                        let idx = self.intern_string(&resolved);
                                        let body_lbl = (ctx.current_label() + 2) as Label;
                                        ctx.emit(Instruction::MatchStr(r_elem, idx, body_lbl));
                                        struct_skip_patches.push(ctx.emit_jump_placeholder());
                                    }
                                    zymbol_common::Literal::Bool(b) => {
                                        let r_b = ctx.alloc_temp()?;
                                        ctx.emit(Instruction::LoadBool(r_b, *b));
                                        let r_cmp = ctx.alloc_temp()?;
                                        ctx.emit(Instruction::CmpEq(r_cmp, r_elem, r_b));
                                        struct_skip_patches.push(ctx.emit_jump_if_not_placeholder(r_cmp));
                                    }
                                    zymbol_common::Literal::Char(c) => {
                                        let r_c = ctx.alloc_temp()?;
                                        ctx.emit(Instruction::LoadChar(r_c, *c));
                                        let r_cmp = ctx.alloc_temp()?;
                                        ctx.emit(Instruction::CmpEq(r_cmp, r_elem, r_c));
                                        struct_skip_patches.push(ctx.emit_jump_if_not_placeholder(r_cmp));
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    // Structural matched — jump to body (placeholder, patched after body label)
                    let patch_struct_to_body = ctx.emit_jump_placeholder();

                    // === Containment path (scrutinee is scalar) ===
                    let containment_label = ctx.current_label();
                    ctx.patch_jump(patch_to_contain, containment_label);

                    let mut jump_to_body_patches: Vec<usize> = Vec::new();
                    for sub_pat in patterns.iter() {
                        match sub_pat {
                            Pattern::Wildcard(_) => {
                                // Wildcard in containment: always matches
                                jump_to_body_patches.push(ctx.emit_jump_placeholder());
                            }
                            Pattern::Literal(lit, _) => {
                                let r_cmp = ctx.alloc_temp()?;
                                match lit {
                                    zymbol_common::Literal::Int(n) => {
                                        ctx.emit(Instruction::CmpEqImm(r_cmp, r_sub, *n as i32));
                                        jump_to_body_patches.push(ctx.emit(Instruction::JumpIf(r_cmp, 0)));
                                    }
                                    zymbol_common::Literal::String(s) | zymbol_common::Literal::InterpolatedString(s) => {
                                        let resolved = s.replace('\x01', "{");
                                        let idx = self.intern_string(&resolved);
                                        jump_to_body_patches.push(ctx.emit(Instruction::MatchStr(r_sub, idx, 0)));
                                    }
                                    zymbol_common::Literal::Bool(b) => {
                                        let r_b = ctx.alloc_temp()?;
                                        ctx.emit(Instruction::LoadBool(r_b, *b));
                                        ctx.emit(Instruction::CmpEq(r_cmp, r_sub, r_b));
                                        jump_to_body_patches.push(ctx.emit(Instruction::JumpIf(r_cmp, 0)));
                                    }
                                    zymbol_common::Literal::Char(c) => {
                                        let r_c = ctx.alloc_temp()?;
                                        ctx.emit(Instruction::LoadChar(r_c, *c));
                                        ctx.emit(Instruction::CmpEq(r_cmp, r_sub, r_c));
                                        jump_to_body_patches.push(ctx.emit(Instruction::JumpIf(r_cmp, 0)));
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    // No containment match → skip to next case
                    let patch_contain_no_match = ctx.emit_jump_placeholder();

                    // === Body (shared by both paths) ===
                    let body_label = ctx.current_label();
                    ctx.patch_jump(patch_struct_to_body, body_label);
                    for p in jump_to_body_patches {
                        ctx.patch_jump(p, body_label);
                    }

                    if let Some(val) = &case.value {
                        let r = self.compile_expr(val, ctx)?;
                        ctx.emit(Instruction::CopyReg(dst, r));
                    } else if let Some(block) = &case.block {
                        self.compile_block(block, ctx)?;
                    }
                    let j = ctx.emit_jump_placeholder();
                    end_patches.push(j);

                    // Patch all "no match" skips to next case
                    let next_case = ctx.current_label();
                    for sp in struct_skip_patches {
                        ctx.patch_jump(sp, next_case);
                    }
                    ctx.patch_jump(patch_contain_no_match, next_case);
                }
            }
        }

        let end_label = ctx.current_label();
        for pos in end_patches {
            ctx.patch_jump(pos, end_label);
        }
        Ok(dst)
    }

    // ── 4C: Statement::Match ─────────────────────────────────────────────────

    fn compile_match_stmt(
        &mut self,
        m: &zymbol_ast::MatchExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        // Same as match expr but result is discarded (or stored in a named variable)
        // The MatchExpr can be used as both statement and expression
        self.compile_match_expr(m, ctx)?;
        Ok(())
    }

    // ── 4C: Collection ops ───────────────────────────────────────────────────

    fn compile_collection_length(
        &mut self,
        cl: &zymbol_ast::CollectionLengthExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        // Fusion: (s $/ sep)$#  →  StrSplitCount  (zero Vec<Value>, via intrinsics)
        if let zymbol_ast::Expr::StringSplit(split) = cl.collection.as_ref() {
            let r_str = self.compile_expr(&split.string, ctx)?;
            let r_sep = self.compile_expr(&split.delimiter, ctx)?;
            let dst   = ctx.alloc_temp()?;
            ctx.emit(Instruction::StrSplitCount(dst, r_str, r_sep));
            ctx.set_reg_type(dst, StaticType::Int);
            return Ok(dst);
        }
        let r = self.compile_expr(&cl.collection, ctx)?;
        let dst = ctx.alloc_temp()?;
        if ctx.get_reg_type(r) == StaticType::String {
            ctx.emit(Instruction::StrLen(dst, r));
        } else {
            ctx.emit(Instruction::ArrayLen(dst, r));
        }
        ctx.set_reg_type(dst, StaticType::Int);
        Ok(dst)
    }

    fn compile_collection_append(
        &mut self,
        ca: &zymbol_ast::CollectionAppendExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let r_coll = self.compile_expr(&ca.collection, ctx)?;
        let r_elem = self.compile_expr(&ca.element, ctx)?;
        // Copy collection first (non-destructive semantics), then push
        let dst = ctx.alloc_temp()?;
        ctx.emit(Instruction::CopyReg(dst, r_coll));
        ctx.emit(Instruction::ArrayPush(dst, r_elem));
        Ok(dst)
    }

    fn compile_collection_remove(
        &mut self,
        cr: &zymbol_ast::CollectionRemoveAtExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let r_coll = self.compile_expr(&cr.collection, ctx)?;
        let r_idx = self.compile_expr(&cr.index, ctx)?;
        let dst = ctx.alloc_temp()?;
        ctx.emit(Instruction::CopyReg(dst, r_coll));
        ctx.emit(Instruction::ArrayRemove(dst, r_idx));
        Ok(dst)
    }

    fn compile_collection_contains(
        &mut self,
        cc: &zymbol_ast::CollectionContainsExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let r_coll = self.compile_expr(&cc.collection, ctx)?;
        let r_elem = self.compile_expr(&cc.element, ctx)?;
        let ty_coll = ctx.get_reg_type(r_coll);
        let dst = ctx.alloc_temp()?;
        if ty_coll == StaticType::String {
            ctx.emit(Instruction::StrContains(dst, r_coll, r_elem));
        } else {
            ctx.emit(Instruction::ArrayContains(dst, r_coll, r_elem));
        }
        ctx.set_reg_type(dst, StaticType::Bool);
        Ok(dst)
    }

    fn compile_collection_update(
        &mut self,
        cu: &zymbol_ast::CollectionUpdateExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        // cu.target is an IndexExpr: arr[idx]
        if let Expr::Index(idx_expr) = cu.target.as_ref() {
            let r_arr = self.compile_expr(&idx_expr.array, ctx)?;
            let r_idx = self.compile_expr(&idx_expr.index, ctx)?;
            let r_val = self.compile_expr(&cu.value, ctx)?;
            // In-place update: copy arr, then set
            let dst = ctx.alloc_temp()?;
            ctx.emit(Instruction::CopyReg(dst, r_arr));
            ctx.emit(Instruction::ArraySet(dst, r_idx, r_val));
            Ok(dst)
        } else {
            Err(CompileError::Unsupported("collection update on non-index expr".into()))
        }
    }

    fn compile_collection_slice(
        &mut self,
        cs: &zymbol_ast::CollectionSliceExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let r_coll = self.compile_expr(&cs.collection, ctx)?;
        // Pre-allocate both bounds consecutively BEFORE any compile_expr calls, so
        // r_hi == r_lo + 1 is always guaranteed (VM reads lo_reg and lo_reg+1).
        let r_lo = ctx.alloc_temp()?;
        let r_hi = ctx.alloc_temp()?; // = r_lo + 1 guaranteed

        // Fill r_lo with start value
        if let Some(start) = &cs.start {
            let r_start = self.compile_expr(start, ctx)?;
            ctx.emit(Instruction::CopyReg(r_lo, r_start));
        } else {
            ctx.emit(Instruction::LoadInt(r_lo, 1));
        }

        // Fill r_hi with end value
        if let Some(end) = &cs.end {
            let r_end = self.compile_expr(end, ctx)?;
            if cs.count_based {
                // [start:count] → actual_end (0-based exclusive) = (start-1) + count = start + count - 1
                // VM normalizes lo as lo-1, so hi must account for 1-based offset too.
                ctx.emit(Instruction::AddInt(r_hi, r_lo, r_end));
                ctx.emit(Instruction::SubIntImm(r_hi, r_hi, 1));
            } else {
                ctx.emit(Instruction::CopyReg(r_hi, r_end));
            }
        } else {
            // slice to end: use length
            if ctx.get_reg_type(r_coll) == StaticType::String {
                ctx.emit(Instruction::StrLen(r_hi, r_coll));
            } else {
                ctx.emit(Instruction::ArrayLen(r_hi, r_coll));
            }
        }
        let dst = ctx.alloc_temp()?;
        let coll_ty = ctx.get_reg_type(r_coll);
        if coll_ty == StaticType::String {
            ctx.emit(Instruction::StrSlice(dst, r_coll, r_lo));
            ctx.set_reg_type(dst, StaticType::String);
        } else {
            ctx.emit(Instruction::ArraySlice(dst, r_coll, r_lo));
        }
        Ok(dst)
    }

    fn compile_collection_insert(
        &mut self,
        ci: &zymbol_ast::CollectionInsertExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let r_coll = self.compile_expr(&ci.collection, ctx)?;
        let r_idx  = self.compile_expr(&ci.index, ctx)?;
        let r_elem = self.compile_expr(&ci.element, ctx)?;
        let dst = ctx.alloc_temp()?;
        ctx.emit(Instruction::CopyReg(dst, r_coll));
        ctx.emit(Instruction::ArrayInsert(dst, r_idx, r_elem));
        Ok(dst)
    }

    fn compile_collection_remove_value(
        &mut self,
        cv: &zymbol_ast::CollectionRemoveValueExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let r_coll = self.compile_expr(&cv.collection, ctx)?;
        let r_val  = self.compile_expr(&cv.value, ctx)?;
        let dst = ctx.alloc_temp()?;
        ctx.emit(Instruction::CopyReg(dst, r_coll));
        ctx.emit(Instruction::ArrayRemoveValue(dst, r_val));
        Ok(dst)
    }

    fn compile_collection_remove_all(
        &mut self,
        ca: &zymbol_ast::CollectionRemoveAllExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let r_coll = self.compile_expr(&ca.collection, ctx)?;
        let r_val  = self.compile_expr(&ca.value, ctx)?;
        let dst = ctx.alloc_temp()?;
        ctx.emit(Instruction::CopyReg(dst, r_coll));
        ctx.emit(Instruction::ArrayRemoveAll(dst, r_val));
        Ok(dst)
    }

    fn compile_collection_remove_range(
        &mut self,
        cr: &zymbol_ast::CollectionRemoveRangeExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let r_coll = self.compile_expr(&cr.collection, ctx)?;
        // Pre-allocate both bounds consecutively BEFORE any compile_expr calls, so
        // r_hi == r_lo + 1 is always guaranteed (VM reads lo_reg and lo_reg+1).
        let r_lo = ctx.alloc_temp()?;
        let r_hi = ctx.alloc_temp()?; // = r_lo + 1 guaranteed

        // Fill r_lo with start value
        if let Some(start) = &cr.start {
            let r_start = self.compile_expr(start, ctx)?;
            ctx.emit(Instruction::CopyReg(r_lo, r_start));
        } else {
            ctx.emit(Instruction::LoadInt(r_lo, 1));
        }

        // Fill r_hi with end value
        if let Some(end) = &cr.end {
            let r_end = self.compile_expr(end, ctx)?;
            if cr.count_based {
                // [start:count] → actual_end (0-based exclusive) = (start-1) + count = start + count - 1
                // VM normalizes lo as lo-1, so hi must account for 1-based offset too.
                ctx.emit(Instruction::AddInt(r_hi, r_lo, r_end));
                ctx.emit(Instruction::SubIntImm(r_hi, r_hi, 1));
            } else {
                ctx.emit(Instruction::CopyReg(r_hi, r_end));
            }
        } else {
            ctx.emit(Instruction::ArrayLen(r_hi, r_coll));
        }
        let dst = ctx.alloc_temp()?;
        ctx.emit(Instruction::CopyReg(dst, r_coll));
        ctx.emit(Instruction::ArrayRemoveRange(dst, r_lo));
        Ok(dst)
    }

    fn compile_collection_map(
        &mut self,
        cm: &zymbol_ast::CollectionMapExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        // Fusion: (str $/ sep) $> fn  →  StrSplitMap (no intermediate Vec<Value>)
        if let zymbol_ast::Expr::StringSplit(split) = cm.collection.as_ref() {
            let r_str = self.compile_expr(&split.string, ctx)?;
            let r_sep = self.compile_expr(&split.delimiter, ctx)?;
            let r_fn  = self.compile_expr(&cm.lambda, ctx)?;
            let dst   = ctx.alloc_temp()?;
            ctx.emit(Instruction::StrSplitMap(dst, r_str, r_sep, r_fn));
            return Ok(dst);
        }
        let r_arr = self.compile_expr(&cm.collection, ctx)?;
        let r_fn  = self.compile_expr(&cm.lambda, ctx)?;
        let dst   = ctx.alloc_temp()?;
        ctx.emit(Instruction::ArrayMap(dst, r_arr, r_fn));
        Ok(dst)
    }

    fn compile_collection_filter(
        &mut self,
        cf: &zymbol_ast::CollectionFilterExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        // Fusion: (str $/ sep) $| fn  →  StrSplitFilter
        if let zymbol_ast::Expr::StringSplit(split) = cf.collection.as_ref() {
            let r_str = self.compile_expr(&split.string, ctx)?;
            let r_sep = self.compile_expr(&split.delimiter, ctx)?;
            let r_fn  = self.compile_expr(&cf.lambda, ctx)?;
            let dst   = ctx.alloc_temp()?;
            ctx.emit(Instruction::StrSplitFilter(dst, r_str, r_sep, r_fn));
            return Ok(dst);
        }
        let r_arr = self.compile_expr(&cf.collection, ctx)?;
        let r_fn  = self.compile_expr(&cf.lambda, ctx)?;
        let dst   = ctx.alloc_temp()?;
        ctx.emit(Instruction::ArrayFilter(dst, r_arr, r_fn));
        Ok(dst)
    }

    fn compile_collection_reduce(
        &mut self,
        cr: &zymbol_ast::CollectionReduceExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        // Fusion: (str $/ sep) $< (init, fn)  →  StrSplitReduce
        if let zymbol_ast::Expr::StringSplit(split) = cr.collection.as_ref() {
            let r_str  = self.compile_expr(&split.string, ctx)?;
            let r_sep  = self.compile_expr(&split.delimiter, ctx)?;
            let r_init = self.compile_expr(&cr.initial, ctx)?;
            let r_fn   = self.compile_expr(&cr.lambda, ctx)?;
            let dst    = ctx.alloc_temp()?;
            ctx.emit(Instruction::StrSplitReduce(dst, r_str, r_sep, r_init, r_fn));
            return Ok(dst);
        }
        let r_arr  = self.compile_expr(&cr.collection, ctx)?;
        let r_init = self.compile_expr(&cr.initial, ctx)?;
        let r_fn   = self.compile_expr(&cr.lambda, ctx)?;
        let dst    = ctx.alloc_temp()?;
        ctx.emit(Instruction::ArrayReduce(dst, r_arr, r_init, r_fn));
        Ok(dst)
    }

    fn compile_collection_sort(
        &mut self,
        cs: &zymbol_ast::CollectionSortExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let r_arr = self.compile_expr(&cs.collection, ctx)?;
        let dst = ctx.alloc_temp()?;
        let r_fn = if let Some(ref cmp) = cs.comparator {
            self.compile_expr(cmp, ctx)?
        } else {
            u16::MAX  // sentinel: no comparator → natural order
        };
        ctx.emit(Instruction::ArraySort(dst, r_arr, cs.ascending, r_fn));
        Ok(dst)
    }

    // ── 4E: Lambda with closure capture ──────────────────────────────────────

    fn compile_lambda(
        &mut self,
        lam: &zymbol_ast::LambdaExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        // Collect free variables (variables used in the body that come from the enclosing scope)
        let free_vars = collect_free_vars(&lam.body, &lam.params, ctx);

        let lambda_name = format!("<lambda#{}>", self.functions.len());
        let func_idx = self.functions.len() as FuncIdx;
        self.functions.push(Chunk::new(&lambda_name));
        self.function_index.insert(lambda_name.clone(), func_idx);

        let mut lambda_ctx = FunctionCtx::new(&lambda_name);
        // Params occupy registers [0..num_params)
        for param in &lam.params {
            lambda_ctx.alloc_reg(param)?;
        }
        let num_params = lam.params.len() as u16;
        // Upvalues occupy registers [num_params..num_params+k) in the lambda's chunk
        for fv in &free_vars {
            lambda_ctx.alloc_reg(fv)?;
        }

        match &lam.body {
            LambdaBody::Expr(body_expr) => {
                let r = self.compile_expr(body_expr, &mut lambda_ctx)?;
                lambda_ctx.emit(Instruction::Return(r));
            }
            LambdaBody::Block(block) => {
                self.compile_block(block, &mut lambda_ctx)?;
                let t = lambda_ctx.alloc_temp()?;
                lambda_ctx.emit(Instruction::LoadUnit(t));
                lambda_ctx.emit(Instruction::Return(t));
            }
        }

        let chunk = lambda_ctx.into_chunk(num_params);
        self.functions[func_idx as usize] = chunk;

        let dst = ctx.alloc_temp()?;
        if free_vars.is_empty() {
            ctx.emit(Instruction::MakeFunc(dst, func_idx));
        } else {
            // Capture the registers from the caller's frame
            let captured: Vec<Reg> = free_vars.iter()
                .map(|v| ctx.get_reg(v))
                .collect::<Result<Vec<_>, _>>()?;
            ctx.emit(Instruction::MakeClosure(dst, func_idx, captured));
        }
        Ok(dst)
    }

    // ── 4C: String interpolation ─────────────────────────────────────────────

    /// Compile a string that may contain `{var}` interpolation.
    fn compile_interpolated_string(
        &mut self,
        s: &str,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        // Parse `{...}` patterns
        let mut parts: Vec<BuildPart> = Vec::new();
        let mut current_lit = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '{' {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    current_lit.push('{');
                } else {
                    // Collect variable name until '}'
                    let mut var_name = String::new();
                    for vc in chars.by_ref() {
                        if vc == '}' { break; }
                        var_name.push(vc);
                    }
                    if !current_lit.is_empty() {
                        // Resolve \x01 sentinel (from \{ escape) to literal {
                        let resolved = current_lit.replace('\x01', "{");
                        let idx = self.intern_string(&resolved);
                        parts.push(BuildPart::Lit(idx));
                        current_lit.clear();
                    }
                    // Get the register for the variable
                    if let Ok(r) = ctx.get_reg(&var_name) {
                        parts.push(BuildPart::Reg(r));
                    } else {
                        // Variable not found — treat as literal text
                        let text = format!("{{{}}}", var_name);
                        let idx = self.intern_string(&text);
                        parts.push(BuildPart::Lit(idx));
                    }
                }
            } else if c == '}' && chars.peek() == Some(&'}') {
                chars.next();
                current_lit.push('}');
            } else {
                current_lit.push(c);
            }
        }
        if !current_lit.is_empty() {
            // Resolve \x01 sentinel (from \{ escape) to literal {
            let resolved = current_lit.replace('\x01', "{");
            let idx = self.intern_string(&resolved);
            parts.push(BuildPart::Lit(idx));
        }

        if parts.len() == 1 {
            if let BuildPart::Lit(idx) = &parts[0] {
                let dst = ctx.alloc_temp()?;
                ctx.emit(Instruction::LoadStr(dst, *idx));
                ctx.set_reg_type(dst, StaticType::String);
                return Ok(dst);
            }
        }
        let dst = ctx.alloc_temp()?;
        ctx.emit(Instruction::BuildStr(dst, parts));
        ctx.set_reg_type(dst, StaticType::String);
        Ok(dst)
    }

    // ── 4C: For-each over arrays ─────────────────────────────────────────────

    fn compile_foreach_loop(
        &mut self,
        lp: &zymbol_ast::Loop,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        let iter_var = lp.iterator_var.as_ref().unwrap();
        let iterable = lp.iterable.as_ref().unwrap();

        let r_coll = self.compile_expr(iterable, ctx)?;
        let coll_is_string = ctx.get_reg_type(r_coll) == StaticType::String;

        let r_len = ctx.alloc_temp()?;
        let r_idx = ctx.alloc_temp()?;
        let r_item = ctx.alloc_reg(iter_var)?;
        let r_cmp = ctx.alloc_temp()?;

        if coll_is_string {
            // String-specific path: StrLen + StrCharAt, 0-based index.
            // Avoids StrChars Vec<Value::Char> allocation — critical when this
            // loop appears inside another loop (N outer × O(len) allocs otherwise).
            ctx.emit(Instruction::StrLen(r_len, r_coll));
            ctx.emit(Instruction::LoadInt(r_idx, 0));
        } else {
            // Generic path: StrChars converts String→Array<Char> once, O(1) for arrays.
            ctx.emit(Instruction::StrChars(r_coll, r_coll));
            ctx.emit(Instruction::ArrayLen(r_len, r_coll));
            ctx.emit(Instruction::LoadInt(r_idx, 1));  // 1-based: start at 1
        }

        let r_one = ctx.alloc_temp()?;
        ctx.emit(Instruction::LoadInt(r_one, 1));

        let loop_start = ctx.current_label();
        ctx.loop_stack.push(LoopCtx {
            break_patches: Vec::new(),
            continue_patches: Vec::new(),
            label: lp.label.clone(),
        });

        if coll_is_string {
            // Exit if r_idx >= r_len (0-based: indices 0..len)
            ctx.emit(Instruction::CmpGe(r_cmp, r_idx, r_len));
            let exit_patch = ctx.emit(Instruction::JumpIf(r_cmp, 0));
            ctx.emit(Instruction::StrCharAt(r_item, r_coll, r_idx));
            self.compile_block(&lp.body, ctx)?;
            let inc_label = ctx.current_label();
            ctx.emit(Instruction::AddIntImm(r_idx, r_idx, 1));
            ctx.emit(Instruction::Jump(loop_start));
            let loop_end = ctx.current_label();
            ctx.patch_jump(exit_patch, loop_end);
            let lctx = ctx.loop_stack.pop().unwrap();
            for pos in lctx.break_patches    { ctx.patch_jump(pos, loop_end); }
            for pos in lctx.continue_patches { ctx.patch_jump(pos, inc_label); }
        } else {
            // Exit if r_idx > r_len (1-based: indices 1..=len)
            ctx.emit(Instruction::CmpGt(r_cmp, r_idx, r_len));
            let exit_patch = ctx.emit(Instruction::JumpIf(r_cmp, 0));
            ctx.emit(Instruction::ArrayGet(r_item, r_coll, r_idx));
            self.compile_block(&lp.body, ctx)?;
            let inc_label = ctx.current_label();
            ctx.emit(Instruction::AddInt(r_idx, r_idx, r_one));
            ctx.emit(Instruction::Jump(loop_start));
            let loop_end = ctx.current_label();
            ctx.patch_jump(exit_patch, loop_end);
            let lctx = ctx.loop_stack.pop().unwrap();
            for pos in lctx.break_patches    { ctx.patch_jump(pos, loop_end); }
            for pos in lctx.continue_patches { ctx.patch_jump(pos, inc_label); }
        }
        Ok(())
    }

    // ── 4C: Try / Catch / Finally ─────────────────────────────────────────────
    //
    // Bytecode layout (try + catch):
    //   TryBegin(catch_label)
    //   [try body]
    //   TryEnd(0)          ; clear catch state, fall through
    //   Jump(end_label)    ; skip catch on success
    //   catch_label:
    //   TryCatch(r_err)    ; bind _err = error value
    //   [catch body]
    //   end_label:
    //   [finally body if any]
    //
    // Bytecode layout (try + finally only):
    //   TryBegin(finally_label)
    //   [try body]
    //   TryEnd(0)          ; clear catch state, fall through to finally
    //   finally_label:
    //   [finally body]
    //   end_label:
    fn compile_try(
        &mut self,
        ts: &TryStmt,
        ctx: &mut FunctionCtx,
    ) -> Result<(), CompileError> {
        let has_catch = !ts.catch_clauses.is_empty();
        let has_finally = ts.finally_clause.is_some();

        // Allocate _err register (used by catch body via _err variable)
        let r_err = ctx.alloc_temp()?;

        // TryBegin: patch target after body is compiled
        let try_begin_pos = ctx.emit(Instruction::TryBegin(0));

        // Compile try body
        self.compile_block(&ts.try_block, ctx)?;

        // TryEnd: clear catch state, then fall through
        ctx.emit(Instruction::TryEnd(0));

        if has_catch {
            // On success: jump past catch block to finally/end
            let jump_past_catch = ctx.emit_jump_placeholder();

            // catch_label: where error jumps to
            let catch_label = ctx.current_label();
            ctx.patch_try_begin(try_begin_pos, catch_label);

            // Bind error to _err
            ctx.emit(Instruction::TryCatch(r_err));
            // Map "_err" to r_err so catch body can reference it
            ctx.register_map.insert("_err".to_string(), r_err);

            // Check if we have typed catch clauses (any with Some(error_type))
            let has_typed = ts.catch_clauses.iter().any(|c| c.error_type.is_some());

            if !has_typed {
                // Simple case: single generic catch (original behavior)
                self.compile_block(&ts.catch_clauses[0].block, ctx)?;
            } else {
                // Typed dispatch: LoadErrorKind → compare → jump to matching clause
                let r_kind = ctx.alloc_temp()?;
                let r_cmp_str = ctx.alloc_temp()?;
                let r_eq = ctx.alloc_temp()?;
                ctx.emit(Instruction::LoadErrorKind(r_kind));

                let mut end_patches: Vec<usize> = Vec::new();

                for clause in &ts.catch_clauses {
                    match &clause.error_type {
                        Some(et) if et.name != "_" => {
                            // Typed clause: compare error_kind with this type
                            let kind_idx = self.intern_string(&et.name);
                            ctx.emit(Instruction::LoadStr(r_cmp_str, kind_idx));
                            ctx.emit(Instruction::CmpEq(r_eq, r_kind, r_cmp_str));
                            let skip = ctx.emit_jump_if_not_placeholder(r_eq);
                            self.compile_block(&clause.block, ctx)?;
                            let j = ctx.emit_jump_placeholder();
                            end_patches.push(j);
                            let next_label = ctx.current_label();
                            ctx.patch_jump(skip, next_label);
                        }
                        _ => {
                            // Generic clause (no error_type or name == "_"): catch-all fallthrough
                            self.compile_block(&clause.block, ctx)?;
                            let j = ctx.emit_jump_placeholder();
                            end_patches.push(j);
                            break; // Generic must be last
                        }
                    }
                }

                let catch_end = ctx.current_label();
                for pos in end_patches {
                    ctx.patch_jump(pos, catch_end);
                }
            }

            let end_label = ctx.current_label();
            ctx.patch_jump(jump_past_catch, end_label);

            // Patch TryBegin target is already done above (catch_label)
        } else {
            // No catch: TryBegin target = finally_label (fall through)
            let finally_label = ctx.current_label();
            ctx.patch_try_begin(try_begin_pos, finally_label);
        }

        // Finally block: always executes
        if has_finally {
            self.compile_block(&ts.finally_clause.as_ref().unwrap().block, ctx)?;
        }

        Ok(())
    }

    // ── BashExec ─────────────────────────────────────────────────────────────

    fn compile_bash_exec(
        &mut self,
        be: &zymbol_ast::BashExecExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        // Compile each arg to a register; VM concatenates them to build the command string
        let mut parts: Vec<BuildPart> = Vec::new();
        for arg in &be.args {
            let r = self.compile_expr(arg, ctx)?;
            parts.push(BuildPart::Reg(r));
        }
        let dst = ctx.alloc_temp()?;
        ctx.emit(Instruction::BashExec(dst, parts));
        ctx.set_reg_type(dst, StaticType::String);
        Ok(dst)
    }

    // ── Format expressions ────────────────────────────────────────────────────

    fn compile_format(
        &mut self,
        fe: &zymbol_ast::FormatExpr,
        ctx: &mut FunctionCtx,
    ) -> Result<Reg, CompileError> {
        let r = self.compile_expr(&fe.expr, ctx)?;
        let dst = ctx.alloc_temp()?;

        let (prec_kind, prec_n) = match fe.precision {
            None => (0u8, 0u32),
            Some(PrecisionOp::Round(n)) => (1u8, n),
            Some(PrecisionOp::Truncate(n)) => (2u8, n),
        };

        match fe.kind {
            FormatKind::Thousands => ctx.emit(Instruction::FmtThousands(dst, r, prec_kind, prec_n)),
            FormatKind::Scientific => ctx.emit(Instruction::FmtScientific(dst, r, prec_kind, prec_n)),
        };
        ctx.set_reg_type(dst, StaticType::String);
        Ok(dst)
    }
}

// ── 4E: Free-variable collection for closure capture ─────────────────────────

/// Collect free variables in a lambda body: identifiers that appear in `outer_ctx`
/// but are not lambda parameters or locally assigned in the body.
fn collect_free_vars(
    body: &LambdaBody,
    params: &[String],
    outer_ctx: &FunctionCtx,
) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut free = Vec::new();
    let mut locals: std::collections::HashSet<String> = params.iter().cloned().collect();
    match body {
        LambdaBody::Expr(e) => {
            collect_free_in_expr(e, &locals, outer_ctx, &mut seen, &mut free);
        }
        LambdaBody::Block(block) => {
            collect_free_in_stmts(&block.statements, &mut locals, outer_ctx, &mut seen, &mut free);
        }
    }
    free
}

fn collect_free_in_expr(
    expr: &Expr,
    locals: &std::collections::HashSet<String>,
    outer_ctx: &FunctionCtx,
    seen: &mut std::collections::HashSet<String>,
    free: &mut Vec<String>,
) {
    match expr {
        Expr::Identifier(id) => {
            if !locals.contains(&id.name) && outer_ctx.register_map.contains_key(&id.name) {
                if seen.insert(id.name.clone()) {
                    free.push(id.name.clone());
                }
            }
        }
        Expr::Binary(b) => {
            collect_free_in_expr(&b.left, locals, outer_ctx, seen, free);
            collect_free_in_expr(&b.right, locals, outer_ctx, seen, free);
        }
        Expr::Unary(u) => {
            collect_free_in_expr(&u.operand, locals, outer_ctx, seen, free);
        }
        Expr::FunctionCall(call) => {
            collect_free_in_expr(&call.callable, locals, outer_ctx, seen, free);
            for arg in &call.arguments {
                collect_free_in_expr(arg, locals, outer_ctx, seen, free);
            }
        }
        Expr::ArrayLiteral(arr) => {
            for elem in &arr.elements {
                collect_free_in_expr(elem, locals, outer_ctx, seen, free);
            }
        }
        Expr::Tuple(t) => {
            for elem in &t.elements {
                collect_free_in_expr(elem, locals, outer_ctx, seen, free);
            }
        }
        Expr::NamedTuple(nt) => {
            for (_, val) in &nt.fields {
                collect_free_in_expr(val, locals, outer_ctx, seen, free);
            }
        }
        Expr::MemberAccess(m) => {
            collect_free_in_expr(&m.object, locals, outer_ctx, seen, free);
        }
        Expr::Index(idx) => {
            collect_free_in_expr(&idx.array, locals, outer_ctx, seen, free);
            collect_free_in_expr(&idx.index, locals, outer_ctx, seen, free);
        }
        Expr::Range(r) => {
            collect_free_in_expr(&r.start, locals, outer_ctx, seen, free);
            collect_free_in_expr(&r.end, locals, outer_ctx, seen, free);
            if let Some(step) = &r.step {
                collect_free_in_expr(step, locals, outer_ctx, seen, free);
            }
        }
        Expr::Match(m) => {
            collect_free_in_expr(&m.scrutinee, locals, outer_ctx, seen, free);
            for case in &m.cases {
                collect_free_in_pattern(&case.pattern, locals, outer_ctx, seen, free);
                if let Some(v) = &case.value {
                    collect_free_in_expr(v, locals, outer_ctx, seen, free);
                }
                if let Some(block) = &case.block {
                    let mut branch_locals = locals.clone();
                    collect_free_in_stmts(&block.statements, &mut branch_locals, outer_ctx, seen, free);
                }
            }
        }
        Expr::Lambda(lam) => {
            // Nested lambda: its params shadow the current locals
            let mut inner_locals = locals.clone();
            for p in &lam.params {
                inner_locals.insert(p.clone());
            }
            match &lam.body {
                LambdaBody::Expr(e) => {
                    collect_free_in_expr(e, &inner_locals, outer_ctx, seen, free);
                }
                LambdaBody::Block(block) => {
                    let mut inner_locals_mut = inner_locals;
                    collect_free_in_stmts(&block.statements, &mut inner_locals_mut, outer_ctx, seen, free);
                }
            }
        }
        Expr::CollectionLength(op) => {
            collect_free_in_expr(&op.collection, locals, outer_ctx, seen, free);
        }
        Expr::CollectionAppend(op) => {
            collect_free_in_expr(&op.collection, locals, outer_ctx, seen, free);
            collect_free_in_expr(&op.element, locals, outer_ctx, seen, free);
        }
        Expr::CollectionInsert(op) => {
            collect_free_in_expr(&op.collection, locals, outer_ctx, seen, free);
            collect_free_in_expr(&op.index, locals, outer_ctx, seen, free);
            collect_free_in_expr(&op.element, locals, outer_ctx, seen, free);
        }
        Expr::CollectionRemoveValue(op) => {
            collect_free_in_expr(&op.collection, locals, outer_ctx, seen, free);
            collect_free_in_expr(&op.value, locals, outer_ctx, seen, free);
        }
        Expr::CollectionRemoveAll(op) => {
            collect_free_in_expr(&op.collection, locals, outer_ctx, seen, free);
            collect_free_in_expr(&op.value, locals, outer_ctx, seen, free);
        }
        Expr::CollectionRemoveAt(op) => {
            collect_free_in_expr(&op.collection, locals, outer_ctx, seen, free);
            collect_free_in_expr(&op.index, locals, outer_ctx, seen, free);
        }
        Expr::CollectionRemoveRange(op) => {
            collect_free_in_expr(&op.collection, locals, outer_ctx, seen, free);
            if let Some(s) = &op.start {
                collect_free_in_expr(s, locals, outer_ctx, seen, free);
            }
            if let Some(e) = &op.end {
                collect_free_in_expr(e, locals, outer_ctx, seen, free);
            }
        }
        Expr::CollectionFindAll(op) => {
            collect_free_in_expr(&op.collection, locals, outer_ctx, seen, free);
            collect_free_in_expr(&op.value, locals, outer_ctx, seen, free);
        }
        Expr::CollectionContains(op) => {
            collect_free_in_expr(&op.collection, locals, outer_ctx, seen, free);
            collect_free_in_expr(&op.element, locals, outer_ctx, seen, free);
        }
        Expr::CollectionUpdate(op) => {
            collect_free_in_expr(&op.target, locals, outer_ctx, seen, free);
            collect_free_in_expr(&op.value, locals, outer_ctx, seen, free);
        }
        Expr::CollectionSlice(op) => {
            collect_free_in_expr(&op.collection, locals, outer_ctx, seen, free);
            if let Some(s) = &op.start {
                collect_free_in_expr(s, locals, outer_ctx, seen, free);
            }
            if let Some(e) = &op.end {
                collect_free_in_expr(e, locals, outer_ctx, seen, free);
            }
        }
        Expr::CollectionMap(op) => {
            collect_free_in_expr(&op.collection, locals, outer_ctx, seen, free);
            collect_free_in_expr(&op.lambda, locals, outer_ctx, seen, free);
        }
        Expr::CollectionFilter(op) => {
            collect_free_in_expr(&op.collection, locals, outer_ctx, seen, free);
            collect_free_in_expr(&op.lambda, locals, outer_ctx, seen, free);
        }
        Expr::CollectionReduce(op) => {
            collect_free_in_expr(&op.collection, locals, outer_ctx, seen, free);
            collect_free_in_expr(&op.initial, locals, outer_ctx, seen, free);
            collect_free_in_expr(&op.lambda, locals, outer_ctx, seen, free);
        }
        Expr::NumericEval(op) => collect_free_in_expr(&op.expr, locals, outer_ctx, seen, free),
        Expr::TypeMetadata(op) => collect_free_in_expr(&op.expr, locals, outer_ctx, seen, free),
        Expr::Format(op) => collect_free_in_expr(&op.expr, locals, outer_ctx, seen, free),
        Expr::BaseConversion(op) => collect_free_in_expr(&op.expr, locals, outer_ctx, seen, free),
        Expr::Round(op) => collect_free_in_expr(&op.expr, locals, outer_ctx, seen, free),
        Expr::Trunc(op) => collect_free_in_expr(&op.expr, locals, outer_ctx, seen, free),
        Expr::ErrorCheck(op) => collect_free_in_expr(&op.expr, locals, outer_ctx, seen, free),
        Expr::ErrorPropagate(op) => collect_free_in_expr(&op.expr, locals, outer_ctx, seen, free),
        Expr::Pipe(pipe) => {
            collect_free_in_expr(&pipe.left, locals, outer_ctx, seen, free);
            collect_free_in_expr(&pipe.callable, locals, outer_ctx, seen, free);
            for arg in &pipe.arguments {
                if let zymbol_ast::PipeArg::Expr(e) = arg {
                    collect_free_in_expr(e, locals, outer_ctx, seen, free);
                }
            }
        }
        Expr::StringReplace(op) => {
            collect_free_in_expr(&op.string, locals, outer_ctx, seen, free);
            collect_free_in_expr(&op.pattern, locals, outer_ctx, seen, free);
            collect_free_in_expr(&op.replacement, locals, outer_ctx, seen, free);
            if let Some(count) = &op.count {
                collect_free_in_expr(count, locals, outer_ctx, seen, free);
            }
        }
        Expr::StringSplit(op) => {
            collect_free_in_expr(&op.string, locals, outer_ctx, seen, free);
            collect_free_in_expr(&op.delimiter, locals, outer_ctx, seen, free);
        }
        Expr::ConcatBuild(op) => {
            collect_free_in_expr(&op.base, locals, outer_ctx, seen, free);
            for item in &op.items { collect_free_in_expr(item, locals, outer_ctx, seen, free); }
        }
        Expr::NumericCast(op) => collect_free_in_expr(&op.expr, locals, outer_ctx, seen, free),
        Expr::CollectionSortAsc(op) | Expr::CollectionSortDesc(op) | Expr::CollectionSortCustom(op) => {
            collect_free_in_expr(&op.collection, locals, outer_ctx, seen, free);
            if let Some(ref cmp) = op.comparator {
                collect_free_in_expr(cmp, locals, outer_ctx, seen, free);
            }
        }
        Expr::DeepIndex(di) => {
            collect_free_in_expr(&di.array, locals, outer_ctx, seen, free);
            for step in &di.path.steps {
                collect_free_in_expr(&step.index, locals, outer_ctx, seen, free);
                if let Some(end) = &step.range_end { collect_free_in_expr(end, locals, outer_ctx, seen, free); }
            }
        }
        Expr::FlatExtract(fe) => {
            collect_free_in_expr(&fe.array, locals, outer_ctx, seen, free);
            for path in &fe.paths {
                for step in &path.steps {
                    collect_free_in_expr(&step.index, locals, outer_ctx, seen, free);
                    if let Some(end) = &step.range_end { collect_free_in_expr(end, locals, outer_ctx, seen, free); }
                }
            }
        }
        Expr::StructuredExtract(se) => {
            collect_free_in_expr(&se.array, locals, outer_ctx, seen, free);
            for group in &se.groups {
                for path in &group.paths {
                    for step in &path.steps {
                        collect_free_in_expr(&step.index, locals, outer_ctx, seen, free);
                        if let Some(end) = &step.range_end { collect_free_in_expr(end, locals, outer_ctx, seen, free); }
                    }
                }
            }
        }
        // Literals and shell expressions have no capturable sub-expressions
        Expr::Literal(_) | Expr::Execute(_) | Expr::BashExec(_) => {}
    }
}

fn collect_free_in_pattern(
    pattern: &zymbol_ast::Pattern,
    locals: &std::collections::HashSet<String>,
    outer_ctx: &FunctionCtx,
    seen: &mut std::collections::HashSet<String>,
    free: &mut Vec<String>,
) {
    match pattern {
        zymbol_ast::Pattern::Comparison(_, expr, _) => {
            collect_free_in_expr(expr, locals, outer_ctx, seen, free);
        }
        zymbol_ast::Pattern::Ident(name, _) => {
            if !locals.contains(name) && outer_ctx.get_reg(name).is_ok() && !seen.contains(name) {
                seen.insert(name.clone());
                free.push(name.clone());
            }
        }
        zymbol_ast::Pattern::Range(lo, hi, _) => {
            collect_free_in_expr(lo, locals, outer_ctx, seen, free);
            collect_free_in_expr(hi, locals, outer_ctx, seen, free);
        }
        zymbol_ast::Pattern::List(pats, _) => {
            for p in pats {
                collect_free_in_pattern(p, locals, outer_ctx, seen, free);
            }
        }
        zymbol_ast::Pattern::Literal(_, _) | zymbol_ast::Pattern::Wildcard(_) => {}
    }
}

fn collect_free_in_stmts(
    stmts: &[Statement],
    locals: &mut std::collections::HashSet<String>,
    outer_ctx: &FunctionCtx,
    seen: &mut std::collections::HashSet<String>,
    free: &mut Vec<String>,
) {
    for stmt in stmts {
        match stmt {
            Statement::Assignment(a) => {
                collect_free_in_expr(&a.value, locals, outer_ctx, seen, free);
                locals.insert(a.name.clone());
            }
            Statement::ConstDecl(c) => {
                collect_free_in_expr(&c.value, locals, outer_ctx, seen, free);
                locals.insert(c.name.clone());
            }
            Statement::Return(ret) => {
                if let Some(e) = &ret.value {
                    collect_free_in_expr(e, locals, outer_ctx, seen, free);
                }
            }
            Statement::Output(out) => {
                for e in &out.exprs {
                    collect_free_in_expr(e, locals, outer_ctx, seen, free);
                }
            }
            Statement::If(if_stmt) => {
                collect_free_in_expr(&if_stmt.condition, locals, outer_ctx, seen, free);
                let mut branch_locals = locals.clone();
                collect_free_in_stmts(&if_stmt.then_block.statements, &mut branch_locals, outer_ctx, seen, free);
                for elif in &if_stmt.else_if_branches {
                    collect_free_in_expr(&elif.condition, locals, outer_ctx, seen, free);
                    let mut branch_locals = locals.clone();
                    collect_free_in_stmts(&elif.block.statements, &mut branch_locals, outer_ctx, seen, free);
                }
                if let Some(else_block) = &if_stmt.else_block {
                    let mut branch_locals = locals.clone();
                    collect_free_in_stmts(&else_block.statements, &mut branch_locals, outer_ctx, seen, free);
                }
            }
            Statement::Loop(loop_stmt) => {
                if let Some(cond) = &loop_stmt.condition {
                    collect_free_in_expr(cond, locals, outer_ctx, seen, free);
                }
                if let Some(iterable) = &loop_stmt.iterable {
                    collect_free_in_expr(iterable, locals, outer_ctx, seen, free);
                }
                let mut loop_locals = locals.clone();
                if let Some(iter_var) = &loop_stmt.iterator_var {
                    loop_locals.insert(iter_var.clone());
                }
                collect_free_in_stmts(&loop_stmt.body.statements, &mut loop_locals, outer_ctx, seen, free);
            }
            Statement::Try(try_stmt) => {
                let mut try_locals = locals.clone();
                collect_free_in_stmts(&try_stmt.try_block.statements, &mut try_locals, outer_ctx, seen, free);
                for catch in &try_stmt.catch_clauses {
                    let mut catch_locals = locals.clone();
                    catch_locals.insert("_err".to_string());
                    collect_free_in_stmts(&catch.block.statements, &mut catch_locals, outer_ctx, seen, free);
                }
                if let Some(finally) = &try_stmt.finally_clause {
                    let mut finally_locals = locals.clone();
                    collect_free_in_stmts(&finally.block.statements, &mut finally_locals, outer_ctx, seen, free);
                }
            }
            Statement::Match(m) => {
                collect_free_in_expr(&m.scrutinee, locals, outer_ctx, seen, free);
                for case in &m.cases {
                    collect_free_in_pattern(&case.pattern, locals, outer_ctx, seen, free);
                    if let Some(v) = &case.value {
                        collect_free_in_expr(v, locals, outer_ctx, seen, free);
                    }
                    if let Some(block) = &case.block {
                        let mut branch_locals = locals.clone();
                        collect_free_in_stmts(&block.statements, &mut branch_locals, outer_ctx, seen, free);
                    }
                }
            }
            Statement::Expr(expr_stmt) => {
                collect_free_in_expr(&expr_stmt.expr, locals, outer_ctx, seen, free);
            }
            Statement::DestructureAssign(d) => {
                collect_free_in_expr(&d.value, locals, outer_ctx, seen, free);
            }
            // No sub-expressions to scan
            Statement::Newline(_) | Statement::Break(_) | Statement::Continue(_)
            | Statement::FunctionDecl(_) | Statement::LifetimeEnd(_)
            | Statement::Input(_) | Statement::CliArgsCapture(_)
            | Statement::SetNumeralMode { .. } => {}
        }
    }
}

// ── Dead-code elimination (DCE) pass ─────────────────────────────────────────
// Forward reachability analysis: mark every instruction reachable from IP 0,
// remap jump targets, recalculate num_registers from surviving instructions.

fn eliminate_dead_code(instructions: Vec<Instruction>, old_num_regs: u16) -> (Vec<Instruction>, u16) {
    let n = instructions.len();
    if n == 0 { return (instructions, old_num_regs); }

    // --- Pass 1: mark reachable instructions via BFS/DFS ---
    let mut reachable = vec![false; n];
    let mut worklist = vec![0usize];

    while let Some(ip) = worklist.pop() {
        if ip >= n || reachable[ip] { continue; }
        reachable[ip] = true;

        match &instructions[ip] {
            // Unconditional jumps: only one successor (the target)
            Instruction::Jump(target) => {
                worklist.push(*target as usize);
            }
            // Conditional jumps: fall-through AND target
            Instruction::JumpIf(_, target) | Instruction::JumpIfNot(_, target) => {
                worklist.push(*target as usize);
                worklist.push(ip + 1);
            }
            // Match instructions: fall-through (no match) AND target (match)
            Instruction::MatchInt(_, _, target)
            | Instruction::MatchRange(_, _, _, target)
            | Instruction::MatchStr(_, _, target) => {
                worklist.push(*target as usize);
                worklist.push(ip + 1);
            }
            // Try/Finally control flow
            Instruction::TryBegin(target) | Instruction::TryEnd(target) => {
                worklist.push(*target as usize);
                worklist.push(ip + 1);
            }
            // Terminators: no successors
            Instruction::Return(_) | Instruction::TailCall(_, _) | Instruction::Halt => {}
            // All other instructions: fall-through only
            _ => { worklist.push(ip + 1); }
        }
    }

    // Quick exit: if everything is reachable, skip transformation
    if reachable.iter().all(|&r| r) {
        return (instructions, old_num_regs);
    }

    // --- Pass 2: build old-IP → new-IP mapping ---
    let mut ip_map = vec![0u32; n + 1];
    let mut new_ip = 0u32;
    for i in 0..n {
        ip_map[i] = new_ip;
        if reachable[i] { new_ip += 1; }
    }
    ip_map[n] = new_ip;

    // Helper: remap a label (old IP → new IP)
    let remap = |lbl: Label| -> Label { ip_map[lbl as usize] };

    // --- Pass 3: filter and remap jump targets ---
    let new_instructions: Vec<Instruction> = instructions
        .into_iter()
        .enumerate()
        .filter(|(i, _)| reachable[*i])
        .map(|(_, instr)| match instr {
            Instruction::Jump(t)               => Instruction::Jump(remap(t)),
            Instruction::JumpIf(r, t)          => Instruction::JumpIf(r, remap(t)),
            Instruction::JumpIfNot(r, t)       => Instruction::JumpIfNot(r, remap(t)),
            Instruction::MatchInt(r, v, t)     => Instruction::MatchInt(r, v, remap(t)),
            Instruction::MatchRange(r, lo, hi, t) => Instruction::MatchRange(r, lo, hi, remap(t)),
            Instruction::MatchStr(r, s, t)     => Instruction::MatchStr(r, s, remap(t)),
            Instruction::TryBegin(t)           => Instruction::TryBegin(remap(t)),
            Instruction::TryEnd(t)             => Instruction::TryEnd(remap(t)),
            other => other,
        })
        .collect();

    // --- Pass 4: recalculate num_registers from surviving instructions ---
    let max_reg = max_reg_used(&new_instructions);
    let num_registers = max_reg.map(|r| r + 1).unwrap_or(0).max(old_num_regs.min(1));

    (new_instructions, num_registers)
}

/// Return the highest register index referenced in `instructions`, if any.
fn max_reg_used(instructions: &[Instruction]) -> Option<u16> {
    let mut max: Option<u16> = None;
    let mut upd = |r: u16| { max = Some(max.map_or(r, |m: u16| m.max(r))); };
    for instr in instructions {
        match instr {
            Instruction::LoadInt(r, _) | Instruction::LoadFloat(r, _)
            | Instruction::LoadBool(r, _) | Instruction::LoadStr(r, _)
            | Instruction::LoadChar(r, _) | Instruction::LoadUnit(r)
            | Instruction::MakeFunc(r, _) => upd(*r),
            Instruction::CopyReg(d, s) | Instruction::MoveReg(d, s) => { upd(*d); upd(*s); }
            Instruction::AddInt(d, a, b) | Instruction::SubInt(d, a, b)
            | Instruction::MulInt(d, a, b) | Instruction::DivInt(d, a, b)
            | Instruction::ModInt(d, a, b) | Instruction::PowInt(d, a, b) => { upd(*d); upd(*a); upd(*b); }
            Instruction::NegInt(d, s) => { upd(*d); upd(*s); }
            Instruction::AddFloat(d, a, b) | Instruction::SubFloat(d, a, b)
            | Instruction::MulFloat(d, a, b) | Instruction::DivFloat(d, a, b)
            | Instruction::PowFloat(d, a, b) => { upd(*d); upd(*a); upd(*b); }
            Instruction::NegFloat(d, s) | Instruction::IntToFloat(d, s)
            | Instruction::FloatToIntRound(d, s) | Instruction::FloatToIntTrunc(d, s) => { upd(*d); upd(*s); }
            Instruction::CmpEq(d, a, b) | Instruction::CmpNe(d, a, b)
            | Instruction::CmpLt(d, a, b) | Instruction::CmpLe(d, a, b)
            | Instruction::CmpGt(d, a, b) | Instruction::CmpGe(d, a, b) => { upd(*d); upd(*a); upd(*b); }
            Instruction::AddIntImm(d, s, _) | Instruction::SubIntImm(d, s, _)
            | Instruction::MulIntImm(d, s, _) => { upd(*d); upd(*s); }
            Instruction::CmpEqImm(d, s, _) | Instruction::CmpNeImm(d, s, _)
            | Instruction::CmpLtImm(d, s, _) | Instruction::CmpLeImm(d, s, _)
            | Instruction::CmpGtImm(d, s, _) | Instruction::CmpGeImm(d, s, _) => { upd(*d); upd(*s); }
            Instruction::Not(d, s) => { upd(*d); upd(*s); }
            Instruction::And(d, a, b) | Instruction::Or(d, a, b) => { upd(*d); upd(*a); upd(*b); }
            Instruction::Return(r) | Instruction::Print(r)
            | Instruction::JumpIf(r, _) | Instruction::JumpIfNot(r, _) => upd(*r),
            Instruction::MatchInt(r, _, _) | Instruction::MatchRange(r, _, _, _)
            | Instruction::MatchStr(r, _, _) | Instruction::MatchBool(r, _, _) => upd(*r),
            Instruction::Call(d, _, args) => { upd(*d); for &a in args { upd(a); } }
            Instruction::TailCall(_, args) => { for &a in args { upd(a); } }
            Instruction::CallDynamic(d, f, args) => { upd(*d); upd(*f); for &a in args { upd(a); } }
            Instruction::MakeClosure(d, _, caps) => { upd(*d); for &c in caps { upd(c); } }
            Instruction::NewArray(d) => upd(*d),
            Instruction::ArrayPush(a, e) => { upd(*a); upd(*e); }
            Instruction::ArrayGet(d, a, i) | Instruction::ArraySet(d, a, i) => { upd(*d); upd(*a); upd(*i); }
            Instruction::ArrayRemove(d, a) | Instruction::ArrayRemoveValue(d, a)
            | Instruction::ArrayRemoveAll(d, a) | Instruction::ArrayRemoveRange(d, a) => { upd(*d); upd(*a); }
            Instruction::ArrayInsert(d, i, v) => { upd(*d); upd(*i); upd(*v); }
            Instruction::ArrayLen(d, a) | Instruction::ArrayContains(d, a, _)
            | Instruction::ArraySlice(d, a, _) => { upd(*d); upd(*a); }
            Instruction::ArrayMap(d, a, f) | Instruction::ArrayFilter(d, a, f) => { upd(*d); upd(*a); upd(*f); }
            Instruction::ArrayReduce(d, a, i, f) => { upd(*d); upd(*a); upd(*i); upd(*f); }
            Instruction::ArraySort(d, a, _, f) => { upd(*d); upd(*a); if *f != u16::MAX { upd(*f); } }
            Instruction::StrSplitCount(d, s, p) => { upd(*d); upd(*s); upd(*p); }
            Instruction::StrSplitMap(d, s, p, f) | Instruction::StrSplitFilter(d, s, p, f) => { upd(*d); upd(*s); upd(*p); upd(*f); }
            Instruction::StrSplitReduce(d, s, p, i, f) => { upd(*d); upd(*s); upd(*p); upd(*i); upd(*f); }
            Instruction::StrLen(d, s) | Instruction::StrChars(d, s) => { upd(*d); upd(*s); }
            Instruction::StrSplit(d, s, p) | Instruction::StrContains(d, s, p)
            | Instruction::StrSlice(d, s, p) | Instruction::StrFindPos(d, s, p)
            | Instruction::ConcatStr(d, s, p) | Instruction::StrCharAt(d, s, p) => { upd(*d); upd(*s); upd(*p); }
            Instruction::ConcatBuild(d, b, items) => { upd(*d); upd(*b); for &i in items { upd(i); } }
            Instruction::StrInsert(d, s, p, t) | Instruction::StrRemove(d, s, p, t)
            | Instruction::StrReplace(d, s, p, t) => { upd(*d); upd(*s); upd(*p); upd(*t); }
            Instruction::StrReplaceN(d, s, p, r, n) => { upd(*d); upd(*s); upd(*p); upd(*r); upd(*n); }
            Instruction::MakeTuple(d, elems) => { upd(*d); for &e in elems { upd(e); } }
            Instruction::MakeNamedTuple(d, _, fields) => { upd(*d); for &f in fields { upd(f); } }
            Instruction::NamedTupleGet(d, t, _) => { upd(*d); upd(*t); }
            Instruction::BashExec(d, _) | Instruction::BuildStr(d, _)
            | Instruction::Execute(d, _) => upd(*d),
            Instruction::FmtThousands(d, s, _, _) | Instruction::FmtScientific(d, s, _, _)
            | Instruction::NumericEval(d, s) | Instruction::TypeOf(d, s)
            | Instruction::IsError(d, s) | Instruction::IsArray(d, s)
            | Instruction::BaseConvert(d, s, _) => { upd(*d); upd(*s); }
            Instruction::RoundFloat(d, s, _) | Instruction::TruncFloat(d, s, _) => { upd(*d); upd(*s); }
            Instruction::LoadErrorKind(d) => upd(*d),
            Instruction::TryCatch(r) => upd(*r),
            Instruction::LoadGlobal(d, _) => upd(*d),
            Instruction::StoreGlobal(_, s) => upd(*s),
            // No-register instructions
            Instruction::SetupOutputWriteback(_) | Instruction::TryBegin(_)
            | Instruction::TryEnd(_) | Instruction::RaiseError(_)
            | Instruction::Jump(_) | Instruction::Halt | Instruction::PrintNewline
            | Instruction::SetNumeralMode(_) => {}
        }
    }
    max
}
