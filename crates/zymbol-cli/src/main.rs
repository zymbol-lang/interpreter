//! Command-line interface for Zymbol-Lang Compiler
//!
//! Supports interpreter (debug) and native compilation (release)

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use zymbol_compiler::Compiler;
use zymbol_error::DiagnosticBag;
use zymbol_formatter::{format_with_config, FormatterConfig};
use zymbol_interpreter::Interpreter;
use zymbol_lexer::Lexer;
use zymbol_parser::Parser as ZParser;
use zymbol_repl::Repl;
use zymbol_semantic::{VariableAnalyzer, TypeChecker, ControlFlowGraph, DefUseAnalyzer, AmbiguityReason, ModuleAnalyzer};
use zymbol_span::SourceMap;
use zymbol_standalone::StandaloneBuilder;
use zymbol_vm::VM;

#[derive(Parser)]
#[command(name = "zymbol")]
#[command(about = "Zymbol-Lang compiler and interpreter", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a Zymbol program with interpreter
    Run {
        /// Path to the .zy file to run
        file: PathBuf,

        /// Execute using the register VM (experimental, Sprint 4)
        #[arg(long, help = "Execute using the register VM (experimental)")]
        vm: bool,

        /// Arguments to pass to the script
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Build/compile a Zymbol program to standalone executable
    Build {
        /// Path to the .z file to compile
        file: PathBuf,

        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Build in release mode (optimized)
        #[arg(short, long)]
        release: bool,
    },

    /// Check a Zymbol program for errors without running
    Check {
        /// Path to the .z file to check
        file: PathBuf,
    },

    /// Format Zymbol source code
    Fmt {
        /// Path to the .zy file to format (use "-" for stdin)
        file: PathBuf,

        /// Write the formatted result back to the file
        #[arg(short, long)]
        write: bool,

        /// Check if the file is already formatted (exit with error if not)
        #[arg(short, long)]
        check: bool,

        /// Number of spaces for indentation (default: 4)
        #[arg(long, default_value = "4")]
        indent: usize,
    },

    /// Start interactive REPL
    Repl,

    /// Start the Language Server Protocol server (reads from stdin, writes to stdout)
    Lsp {
        /// Use stdio transport — accepted for LSP client compatibility (this is always the mode)
        #[arg(long, hide = true)]
        stdio: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { file, vm, args } => run_file(file, args, vm),
        Commands::Build { file, output, release } => build_file(file, output, release),
        Commands::Check { file } => check_file(file),
        Commands::Fmt { file, write, check, indent } => format_file(file, write, check, indent),
        Commands::Repl => start_repl(),
        Commands::Lsp { .. } => start_lsp(),
    }
}

fn start_repl() -> Result<()> {
    let mut repl = Repl::new();
    repl.start().map_err(|e| anyhow::anyhow!("REPL error: {}", e))
}

fn start_lsp() -> Result<()> {
    tokio::runtime::Runtime::new()
        .map_err(|e| anyhow::anyhow!("failed to create tokio runtime: {}", e))?
        .block_on(zymbol_lsp::run());
    Ok(())
}

fn run_file(path: PathBuf, args: Vec<String>, use_vm: bool) -> Result<()> {
    // Read source file
    let source = fs::read_to_string(&path)
        .with_context(|| format!("failed to read file: {}", path.display()))?;

    // Setup source map
    let mut source_map = SourceMap::new();
    let display_name = std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(&cwd).ok().map(|p| p.to_string_lossy().into_owned()))
        .unwrap_or_else(|| path.display().to_string());
    let file_id = source_map.add_file(display_name, source.clone());

    // Lex
    let lexer = Lexer::new(&source, file_id);
    let (tokens, lex_diagnostics) = lexer.tokenize();

    if !lex_diagnostics.is_empty() {
        let mut bag = DiagnosticBag::new();
        for diag in lex_diagnostics {
            bag.add(diag);
        }
        bag.emit_all(&source_map);
        std::process::exit(1);
    }

    // Parse
    let parser = ZParser::new(tokens);
    let program = match parser.parse() {
        Ok(prog) => prog,
        Err(diagnostics) => {
            let mut bag = DiagnosticBag::new();
            for diag in diagnostics {
                bag.add(diag);
            }
            bag.emit_all(&source_map);
            std::process::exit(1);
        }
    };

    // Module files are not directly executable
    if program.module_decl.is_some() {
        let module_name = program.module_decl.as_ref().map(|m| m.name.as_str()).unwrap_or("?");
        eprintln!("warning: '{}' is a module file and cannot be run directly", path.display());
        eprintln!("  = help: module '{}' is meant to be imported with <# ./{} <= alias", module_name, path.file_stem().and_then(|s| s.to_str()).unwrap_or("module"));
        std::process::exit(1);
    }

    // Run semantic analysis before execution
    let mut analyzer = VariableAnalyzer::new();
    let warnings = analyzer.analyze(&program);

    // Check for semantic errors (these are hard errors, not warnings)
    let semantic_errors = analyzer.semantic_errors();
    if !semantic_errors.is_empty() {
        let mut bag = DiagnosticBag::new();
        for err in semantic_errors {
            bag.add(err.clone());
        }
        bag.emit_all(&source_map);
        std::process::exit(1);
    }

    // Show variable analysis warnings but continue
    if !warnings.is_empty() {
        for warning in &warnings {
            eprintln!("warning: {}", warning.message);
            eprintln!("  --> {}:{}:{}",
                path.display(),
                warning.span.start.line,
                warning.span.start.column
            );
            if let Some(help) = &warning.help {
                eprintln!("  = help: {}", help);
            }
            eprintln!();
        }
    }

    // Run type checking
    let mut type_checker = TypeChecker::new();
    let type_errors = type_checker.check_errors(&program);

    // Type errors are fatal - stop execution
    if !type_errors.is_empty() {
        let mut bag = DiagnosticBag::new();
        for err in type_errors {
            bag.add(err);
        }
        bag.emit_all(&source_map);
        std::process::exit(1);
    }

    // Show type warnings but continue execution
    for warning in type_checker.get_warnings() {
        eprintln!("warning: {}", warning.message);
        if let Some(span) = &warning.span {
            eprintln!("  --> {}:{}:{}",
                path.display(),
                span.start.line,
                span.start.column
            );
        }
        if let Some(help) = &warning.help {
            eprintln!("  = help: {}", help);
        }
        eprintln!();
    }

    if use_vm {
        // Sprint 4: Register VM path
        let compiled = match Compiler::compile_with_dir(&program, path.parent()) {
            Ok(c) => c,
            Err(e) => {
                // These errors match the tree-walker "Runtime error:" format
                if matches!(e,
                    zymbol_compiler::CompileError::CircularImport(_) |
                    zymbol_compiler::CompileError::ModuleParse(_) |
                    zymbol_compiler::CompileError::ModuleNotFound(_)
                ) {
                    eprintln!("Runtime error: {}", e);
                } else {
                    eprintln!("VM compile error: {}", e);
                }
                std::process::exit(1);
            }
        };
        let mut vm = VM::new(std::io::stdout());
        if let Err(e) = vm.run(&compiled) {
            eprintln!("Runtime error: {}", e);
            std::process::exit(1);
        }
    } else {
        // Execute with tree-walker interpreter
        let mut interpreter = Interpreter::new();

        // Set the current file path for module resolution
        interpreter.set_current_file(&path);

        // Set the base directory (parent of the file)
        if let Some(parent) = path.parent() {
            interpreter.set_base_dir(parent);
        }

        // Pass CLI arguments to the interpreter
        interpreter.set_cli_args(args);

        if let Err(e) = interpreter.execute(&program) {
            eprintln!("Runtime error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}

fn build_file(path: PathBuf, output: Option<PathBuf>, release: bool) -> Result<()> {
    // Read source file
    let source = fs::read_to_string(&path)
        .with_context(|| format!("failed to read file: {}", path.display()))?;

    // Verify it compiles (early error detection)
    let mut source_map = SourceMap::new();
    let display_name = std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(&cwd).ok().map(|p| p.to_string_lossy().into_owned()))
        .unwrap_or_else(|| path.display().to_string());
    let file_id = source_map.add_file(display_name, source.clone());

    let lexer = Lexer::new(&source, file_id);
    let (tokens, lex_diagnostics) = lexer.tokenize();

    if !lex_diagnostics.is_empty() {
        let mut bag = DiagnosticBag::new();
        for diag in lex_diagnostics {
            bag.add(diag);
        }
        bag.emit_all(&source_map);
        std::process::exit(1);
    }

    let parser = ZParser::new(tokens);
    let program = match parser.parse() {
        Ok(prog) => prog,
        Err(diagnostics) => {
            let mut bag = DiagnosticBag::new();
            for diag in diagnostics {
                bag.add(diag);
            }
            bag.emit_all(&source_map);
            std::process::exit(1);
        }
    };

    // Run semantic analysis before building
    let mut analyzer = VariableAnalyzer::new();
    let warnings = analyzer.analyze(&program);

    // Check for semantic errors
    let semantic_errors = analyzer.semantic_errors();
    if !semantic_errors.is_empty() {
        let mut bag = DiagnosticBag::new();
        for err in semantic_errors {
            bag.add(err.clone());
        }
        bag.emit_all(&source_map);
        std::process::exit(1);
    }

    // Show variable analysis warnings
    if !warnings.is_empty() {
        for warning in &warnings {
            eprintln!("warning: {}", warning.message);
            eprintln!("  --> {}:{}:{}",
                path.display(),
                warning.span.start.line,
                warning.span.start.column
            );
            if let Some(help) = &warning.help {
                eprintln!("  = help: {}", help);
            }
            eprintln!();
        }
    }

    // Run type checking
    let mut type_checker = TypeChecker::new();
    let type_errors = type_checker.check_errors(&program);

    // Type errors are fatal - stop build
    if !type_errors.is_empty() {
        let mut bag = DiagnosticBag::new();
        for err in type_errors {
            bag.add(err);
        }
        bag.emit_all(&source_map);
        std::process::exit(1);
    }

    // Show type warnings but continue build
    for warning in type_checker.get_warnings() {
        eprintln!("warning: {}", warning.message);
        if let Some(span) = &warning.span {
            eprintln!("  --> {}:{}:{}",
                path.display(),
                span.start.line,
                span.start.column
            );
        }
        if let Some(help) = &warning.help {
            eprintln!("  = help: {}", help);
        }
        eprintln!();
    }

    // Determine output path
    let output_path = output.unwrap_or_else(|| {
        let mut p = path.clone();
        p.set_extension("");
        p
    });

    // Build standalone executable
    let builder = StandaloneBuilder::new_from_source(source, output_path, release);
    builder.build()
        .with_context(|| "failed to build executable")?;

    Ok(())
}

fn check_file(path: PathBuf) -> Result<()> {
    // Read source file
    let source = fs::read_to_string(&path)
        .with_context(|| format!("failed to read file: {}", path.display()))?;

    // Setup source map
    let mut source_map = SourceMap::new();
    let display_name = std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(&cwd).ok().map(|p| p.to_string_lossy().into_owned()))
        .unwrap_or_else(|| path.display().to_string());
    let file_id = source_map.add_file(display_name, source.clone());

    // Lex
    let lexer = Lexer::new(&source, file_id);
    let (tokens, lex_diagnostics) = lexer.tokenize();

    let mut has_errors = false;

    if !lex_diagnostics.is_empty() {
        let mut bag = DiagnosticBag::new();
        for diag in lex_diagnostics {
            bag.add(diag);
        }
        bag.emit_all(&source_map);
        has_errors = true;
    }

    // Parse
    let parser = ZParser::new(tokens);
    let program = match parser.parse() {
        Ok(prog) => prog,
        Err(diagnostics) => {
            let mut bag = DiagnosticBag::new();
            for diag in diagnostics {
                bag.add(diag);
            }
            bag.emit_all(&source_map);
            // Exit early if parsing failed - can't run semantic analysis
            std::process::exit(1);
        }
    };

    if has_errors {
        std::process::exit(1);
    }

    // Run variable liveness analysis
    let mut analyzer = VariableAnalyzer::new();
    let warnings = analyzer.analyze(&program);

    // Check for semantic errors (e.g., _variable scope violations)
    let semantic_errors = analyzer.semantic_errors();
    if !semantic_errors.is_empty() {
        let mut bag = DiagnosticBag::new();
        for err in semantic_errors {
            bag.add(err.clone());
        }
        bag.emit_all(&source_map);
        has_errors = true;
    }

    // Report variable warnings (unused variables, write-only variables)
    if !warnings.is_empty() {
        eprintln!();
        for warning in &warnings {
            eprintln!("warning: {}", warning.message);
            eprintln!("  --> {}:{}:{}",
                path.display(),
                warning.span.start.line,
                warning.span.start.column
            );
            if let Some(help) = &warning.help {
                eprintln!("  = help: {}", help);
            }
            eprintln!();
        }
    }

    // Run type checking
    let mut type_checker = TypeChecker::new();
    let type_errors = type_checker.check_errors(&program);

    // Type errors are fatal
    if !type_errors.is_empty() {
        let mut bag = DiagnosticBag::new();
        for err in type_errors {
            bag.add(err);
        }
        bag.emit_all(&source_map);
        has_errors = true;
    }

    // Report type warnings
    let mut type_warning_count = 0;
    for diag in type_checker.get_warnings() {
        eprintln!("warning: {}", diag.message);
        if let Some(span) = &diag.span {
            eprintln!("  --> {}:{}:{}",
                path.display(),
                span.start.line,
                span.start.column
            );
        }
        if let Some(help) = &diag.help {
            eprintln!("  = help: {}", help);
        }
        eprintln!();
        type_warning_count += 1;
    }

    // Run module analysis if the file has module declarations
    if program.module_decl.is_some() || !program.imports.is_empty() {
        let base_dir = path.parent().unwrap_or(std::path::Path::new("."));
        let mut module_analyzer = ModuleAnalyzer::new(base_dir);

        if let Err(module_errors) = module_analyzer.analyze(&program, &path) {
            for err in module_errors {
                eprintln!("error: {}", err.message);
                if let Some(span) = &err.span {
                    eprintln!("  --> {}:{}:{}",
                        path.display(),
                        span.start.line,
                        span.start.column
                    );
                }
                if let Some(help) = &err.help {
                    eprintln!("  = help: {}", help);
                }
                eprintln!();
            }
            has_errors = true;
        }

        // Validate exports exist
        module_analyzer.validate_exports(&program, &path);
        for diag in module_analyzer.diagnostics() {
            eprintln!("error: {}", diag.message);
            if let Some(span) = &diag.span {
                eprintln!("  --> {}:{}:{}",
                    path.display(),
                    span.start.line,
                    span.start.column
                );
            }
            eprintln!();
            has_errors = true;
        }
    }

    // Run def-use analysis for lifetime detection
    let cfg = ControlFlowGraph::build_sequential(&program.statements);
    let mut def_use_analyzer = DefUseAnalyzer::new();
    let _chains = def_use_analyzer.analyze(&program.statements, &cfg);

    // Report ambiguous lifetime warnings
    let ambiguous_vars = def_use_analyzer.get_ambiguous_variables();
    let mut lifetime_warning_count = 0;
    for chain in &ambiguous_vars {
        if let Some(ambiguity) = &chain.ambiguity {
            let reason_str = match ambiguity.reason {
                crate::AmbiguityReason::LoopVariant => "variable is modified inside a loop",
                crate::AmbiguityReason::ConditionalUse => "variable is used in some branches but not others",
                crate::AmbiguityReason::MultipleExitPaths => "multiple possible last uses",
            };
            eprintln!("warning: ambiguous lifetime for '{}'", chain.variable);
            eprintln!("  --> {}:{}:{}",
                path.display(),
                ambiguity.suggested_span.start.line,
                ambiguity.suggested_span.start.column
            );
            eprintln!("  = note: {}", reason_str);
            eprintln!("  = help: consider using explicit lifetime annotation");
            eprintln!();
            lifetime_warning_count += 1;
        }
    }

    if has_errors {
        std::process::exit(1);
    }

    let total_warnings = warnings.len() + type_warning_count + lifetime_warning_count;
    if total_warnings > 0 {
        println!("Checked with {} warning(s)", total_warnings);
    } else {
        println!("No errors or warnings");
    }
    Ok(())
}

fn format_file(path: PathBuf, write: bool, check: bool, indent: usize) -> Result<()> {
    use std::io::Read;

    // Read source: from stdin if path is "-", otherwise from file
    let (source, is_stdin) = if path.as_os_str() == "-" {
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)
            .with_context(|| "failed to read from stdin")?;
        (buffer, true)
    } else {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read file: {}", path.display()))?;
        (content, false)
    };

    // Create formatter config
    let config = FormatterConfig::new().with_indent_size(indent);

    // Format the source
    let formatted = format_with_config(&source, config)
        .with_context(|| {
            if is_stdin {
                "failed to format input".to_string()
            } else {
                format!("failed to format file: {}", path.display())
            }
        })?;

    if check {
        // Check mode: exit with error if not formatted
        if formatted != source {
            if is_stdin {
                eprintln!("✗ Input is not formatted");
            } else {
                eprintln!("✗ {} is not formatted", path.display());
            }
            std::process::exit(1);
        }
        if is_stdin {
            println!("✓ Input is formatted");
        } else {
            println!("✓ {} is formatted", path.display());
        }
    } else if write {
        if is_stdin {
            // Cannot write back to stdin, just print
            print!("{}", formatted);
        } else {
            // Write mode: write formatted output back to file
            if formatted != source {
                fs::write(&path, &formatted)
                    .with_context(|| format!("failed to write file: {}", path.display()))?;
                println!("✓ Formatted {}", path.display());
            } else {
                println!("✓ {} already formatted", path.display());
            }
        }
    } else {
        // Default mode: print formatted output to stdout
        print!("{}", formatted);
    }

    Ok(())
}
