//! Command-line interface for Zymbol-Lang Compiler
//!
//! Supports interpreter (debug) and native compilation (release)

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
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
use zymbol_package::{
    compute_closure, open_zyp, write_zyp, EngineMode, Manifest, PackageError, PackageMeta, ScriptEntry,
};

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
    /// Run a Zymbol program with interpreter — a .zy file directly, or a .zyp package
    Run {
        /// Path to the .zy file, or a .zyp package, to run
        file: PathBuf,

        /// Execute using the register VM (experimental, Sprint 4). For a .zyp package this
        /// is already the default unless its manifest says `mode = "tw"`; the flag exists
        /// mainly to override that.
        #[arg(long, help = "Execute using the register VM (experimental)")]
        vm: bool,

        /// Force the tree-walking interpreter. For a plain .zy file this is already the
        /// default (only useful to make that explicit); for a .zyp package it overrides
        /// both `--vm` and the manifest's `mode`.
        #[arg(long, conflicts_with = "vm")]
        tw: bool,

        /// Which [[script]] of a .zyp package to run (default: the one marked
        /// `default = true`, or the only one if there's just one). Ignored for a plain .zy
        /// file.
        #[arg(long)]
        script: Option<String>,

        /// Keep a .zyp's extraction directory instead of deleting it on exit, and print its
        /// path — useful for debugging a package. Ignored for a plain .zy file.
        #[arg(long)]
        keep_temp: bool,

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

    /// Package Zymbol source into a portable .zyp archive (source, not a compiled binary —
    /// see `build` for that). `path` is a directory containing zyp.toml, or a manifest file
    /// directly.
    Package {
        /// Directory containing zyp.toml, or the manifest file itself
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output .zyp path (default: <name>-<version>.zyp in the current directory)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Entry script(s), relative to `path` — used to synthesize a manifest when `path`
        /// has no zyp.toml. Repeatable; the first one becomes the default script.
        #[arg(long = "script")]
        scripts: Vec<String>,

        /// Package name when synthesizing a manifest (default: path's directory name)
        #[arg(long)]
        name: Option<String>,

        /// Package version when synthesizing a manifest (default: "0.1.0")
        #[arg(long)]
        version: Option<String>,

        /// Print the closure and warnings without writing the archive
        #[arg(long)]
        dry_run: bool,
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
        Commands::Run { file, vm, tw, script, keep_temp, args } => {
            run_file(file, args, vm, tw, script, keep_temp)
        }
        Commands::Build { file, output, release } => build_file(file, output, release),
        Commands::Package { path, output, scripts, name, version, dry_run } => {
            package_cmd(path, output, scripts, name, version, dry_run)
        }
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

/// Options for [`run_file_inner`]. Kept separate from the plain `run_file(path, args, vm)`
/// entry point so that callers with special needs (e.g. running an extracted `.zyp` script
/// from a temp directory) can override the display name without duplicating the whole
/// compile/run pipeline.
struct RunOpts {
    /// Overrides the path shown in diagnostics and "Runtime error:" messages.
    /// Used by `.zyp` execution so errors read as `go.zyp!核/盤.zy:31:4` instead of a
    /// temp-directory path that means nothing to the user.
    display_name: Option<String>,
    args: Vec<String>,
    use_vm: bool,
}

fn run_file(
    path: PathBuf,
    args: Vec<String>,
    use_vm: bool,
    use_tw: bool,
    script: Option<String>,
    keep_temp: bool,
) -> Result<()> {
    if is_zyp(&path) {
        return run_zyp(path, args, use_vm, use_tw, script, keep_temp);
    }
    let code = run_file_inner(&path, RunOpts { display_name: None, args, use_vm })?;
    std::process::exit(code);
}

/// A `.zyp` is a ZIP archive, so this checks both the extension and (in case someone
/// renamed or extension-less'd it) the ZIP local-file-header magic bytes `PK\x03\x04`.
fn is_zyp(path: &Path) -> bool {
    if path.extension().is_some_and(|e| e == "zyp") {
        return true;
    }
    use std::io::Read;
    let Ok(mut f) = fs::File::open(path) else { return false };
    let mut magic = [0u8; 4];
    f.read_exact(&mut magic).is_ok() && magic == *b"PK\x03\x04"
}

/// Extracts a `.zyp` to an ephemeral temp directory and runs one of its `[[script]]`
/// entries out of there — never `chdir`s, so a script's `std/io` writes (relative paths)
/// still land in the user's real working directory while its *code* is read from the temp
/// extraction. That split is the whole point of ephemeral extraction: code is disposable,
/// data the script writes is not.
fn run_zyp(
    path: PathBuf,
    args: Vec<String>,
    force_vm: bool,
    force_tw: bool,
    script: Option<String>,
    keep_temp: bool,
) -> Result<()> {
    // No `.with_context("failed to open …")` here: every PackageError already names the
    // package and says what was wrong with it, so adding a wrapper only stacks a second
    // line that repeats the path.
    let pkg = open_zyp(&path)?;
    // Fail before touching disk any further: an incompatible package shouldn't even get to
    // the point of being extracted.
    pkg.manifest.check_engine(env!("CARGO_PKG_VERSION"))?;

    let entry = pkg.manifest.resolve_script(script.as_deref())?;
    let (entry_name, entry_path) = (entry.name.clone(), entry.path.clone());

    // Precedence: --tw > --vm > manifest's `mode` > Vm (the .zyp default; see the `Run`
    // command's doc comment for why plain .zy files keep defaulting to the tree-walker
    // instead).
    let use_vm = if force_tw {
        false
    } else if force_vm {
        true
    } else {
        !matches!(pkg.manifest.package.mode, Some(EngineMode::Tw))
    };

    let temp = tempfile::Builder::new()
        .prefix("zymbol-zyp-")
        .tempdir()
        .with_context(|| "failed to create a temp directory to extract the package into")?;
    pkg.extract_to(temp.path())
        .with_context(|| format!("failed to extract {}", path.display()))?;

    if use_vm {
        // Only needed for the VM path: </ /> compiles to a shelled-out `zymbol run <path>`
        // (see zymbol-compiler's Expr::Execute codegen), which needs `zymbol` on $PATH.
        prepend_self_to_path();
    }

    let entry_abs = pkg.script_abs_path(temp.path(), &entry_name, &entry_path)?;
    let display_name = format!("{}!{}", path.display(), entry_path);

    let code = run_file_inner(&entry_abs, RunOpts { display_name: Some(display_name), args, use_vm })?;

    if keep_temp {
        let kept = temp.keep();
        eprintln!("kept extraction directory: {}", kept.display());
    } else {
        // `std::process::exit` below does NOT run destructors — it's a hard process
        // termination, not a normal return. Letting `temp` merely fall out of scope right
        // before calling it would silently leak the extraction directory on every run (this
        // was caught empirically: a first version of this function did exactly that, and
        // every invocation left a `zymbol-zyp-*` directory behind in the temp dir). Calling
        // `drop` explicitly runs `TempDir`'s destructor *now*, as an ordinary function call,
        // strictly before `exit` ends the process.
        drop(temp);
    }

    std::process::exit(code);
}

/// `</ />` in the register VM shells out to `zymbol run <path>` in a child process (rather
/// than executing inline) — so if this binary was invoked via a path not on `$PATH` (e.g.
/// `./target/debug/zymbol run go.zyp --vm`), that child would fail with "zymbol: not
/// found". Prepending this process's own directory to `$PATH` fixes that without requiring
/// a global install.
fn prepend_self_to_path() {
    let Ok(exe) = std::env::current_exe() else { return };
    let Some(exe_dir) = exe.parent() else { return };
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let mut paths: Vec<PathBuf> = std::env::split_paths(&existing).collect();
    if paths.first().map(PathBuf::as_path) != Some(exe_dir) {
        paths.insert(0, exe_dir.to_path_buf());
        if let Ok(new_path) = std::env::join_paths(paths) {
            // SAFETY: called once, early in `main`, strictly before any additional thread
            // or child process exists in this program — mutating the environment is only
            // unsound when something else might be reading it concurrently.
            unsafe {
                std::env::set_var("PATH", new_path);
            }
        }
    }
}

/// Compiles and executes `path`. Never calls `std::process::exit` — every early-return path
/// that used to `exit(1)` now returns `Ok(1)` instead, so a caller holding a `TempDir` (or any
/// other RAII guard) gets to run its `Drop` before the process actually exits.
fn run_file_inner(path: &Path, opts: RunOpts) -> Result<i32> {
    let RunOpts { display_name, args, use_vm } = opts;

    // Read source file
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read file: {}", path.display()))?;

    // Setup source map
    let mut source_map = SourceMap::new();
    let display_name = display_name.unwrap_or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|cwd| path.strip_prefix(&cwd).ok().map(|p| p.to_string_lossy().into_owned()))
            .unwrap_or_else(|| path.display().to_string())
    });
    let file_id = source_map.add_file(display_name.clone(), source.clone());

    // Lex
    let lexer = Lexer::new(&source, file_id);
    let (tokens, lex_diagnostics) = lexer.tokenize();

    if !lex_diagnostics.is_empty() {
        let mut bag = DiagnosticBag::new();
        for diag in lex_diagnostics {
            bag.add(diag);
        }
        bag.emit_all(&source_map);
        return Ok(1);
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
            return Ok(1);
        }
    };

    // Module files are not directly executable
    if program.module_decl.is_some() {
        let module_name = program.module_decl.as_ref().map(|m| m.name.as_str()).unwrap_or("?");
        eprintln!("warning: '{}' is a module file and cannot be run directly", display_name);
        eprintln!("  = help: module '{}' is meant to be imported with <# ./{} <= alias", module_name, path.file_stem().and_then(|s| s.to_str()).unwrap_or("module"));
        return Ok(1);
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
        return Ok(1);
    }

    // Show variable analysis warnings but continue
    if !warnings.is_empty() {
        for warning in &warnings {
            eprintln!("warning: {}", warning.message);
            eprintln!("  --> {}:{}:{}",
                display_name,
                warning.span.start.line,
                warning.span.start.column
            );
            if let Some(help) = &warning.help {
                eprintln!("  = help: {}", help);
            }
            eprintln!();
        }
    }

    // Run type checking. The arity table must be supplied here too, not only in
    // `check`: an argument-count mismatch is fatal before execution, and it has
    // to be fatal for `alias::func(...)` exactly as it already was for a bare
    // `func(...)`. Without it `run` would reject one and execute the other.
    let mut type_checker = TypeChecker::new();
    type_checker.set_module_arities(zymbol_semantic::module_arities(
        &program.imports,
        path.parent().unwrap_or(std::path::Path::new(".")),
    ));
    let type_errors = type_checker.check_errors(&program);

    // Type errors are fatal - stop execution
    if !type_errors.is_empty() {
        let mut bag = DiagnosticBag::new();
        for err in type_errors {
            bag.add(err);
        }
        bag.emit_all(&source_map);
        return Ok(1);
    }

    // Show type warnings but continue execution
    for warning in type_checker.get_warnings() {
        eprintln!("warning: {}", warning.message);
        if let Some(span) = &warning.span {
            eprintln!("  --> {}:{}:{}",
                display_name,
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
                return Ok(1);
            }
        };
        let mut vm = VM::new(std::io::stdout());
        vm.set_cli_args(args.clone());
        if let Err(e) = vm.run(&compiled) {
            eprintln!("Runtime error: {}", e);
            return Ok(1);
        }
    } else {
        // Execute with tree-walker interpreter
        let mut interpreter = Interpreter::new();

        // Set the current file path for module resolution
        interpreter.set_current_file(path);

        // Set the base directory (parent of the file)
        if let Some(parent) = path.parent() {
            interpreter.set_base_dir(parent);
        }

        // Pass CLI arguments to the interpreter
        interpreter.set_cli_args(args);

        if let Err(e) = interpreter.execute(&program) {
            eprintln!("Runtime error: {}", e);
            return Ok(1);
        }
    }

    Ok(0)
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

    // Run type checking, with the same arity table `run` and `check` use.
    let mut type_checker = TypeChecker::new();
    type_checker.set_module_arities(zymbol_semantic::module_arities(
        &program.imports,
        path.parent().unwrap_or(std::path::Path::new(".")),
    ));
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
    let base_dir = path.canonicalize().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let builder = StandaloneBuilder::new_from_source(source, base_dir, output_path, release);
    builder.build()
        .with_context(|| "failed to build executable")?;

    Ok(())
}

/// `zymbol package`: builds a `.zyp` from a `zyp.toml` (found at `path` if it's a directory,
/// or `path` itself if it's a manifest file), or — if no manifest exists — synthesizes one
/// from `--script` flags, prints it so the author can save it as `zyp.toml` for next time,
/// and packages with it anyway. Either way this only *reads* the source tree; the only
/// thing written to disk is the `.zyp` itself (never a `zyp.toml` the user didn't ask for).
fn package_cmd(
    path: PathBuf,
    output: Option<PathBuf>,
    scripts: Vec<String>,
    name: Option<String>,
    version: Option<String>,
    dry_run: bool,
) -> Result<()> {
    let (manifest_dir, manifest_source) = if path.is_dir() {
        let candidate = path.join("zyp.toml");
        if candidate.is_file() {
            let source = fs::read_to_string(&candidate)
                .with_context(|| format!("failed to read {}", candidate.display()))?;
            (path.clone(), Some(source))
        } else {
            (path.clone(), None)
        }
    } else if path.is_file() {
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        (dir, Some(source))
    } else {
        anyhow::bail!("path not found: {}", path.display());
    };

    let manifest = match manifest_source {
        Some(toml_src) => {
            // --script/--name/--version only feed manifest *synthesis*. With a zyp.toml
            // present they have nothing to act on, and silently ignoring them is a trap:
            // `--script other.zy` looks like it selected something and instead packaged
            // whatever the manifest already said. Refuse, and point at the two real options.
            let mut ignored = Vec::new();
            if !scripts.is_empty() {
                ignored.push("--script");
            }
            if name.is_some() {
                ignored.push("--name");
            }
            if version.is_some() {
                ignored.push("--version");
            }
            if !ignored.is_empty() {
                anyhow::bail!(
                    "{} cannot be combined with an existing zyp.toml ({})\n  \
                     = help: these flags only apply when synthesizing a manifest; edit the \
                     zyp.toml instead, or point `zymbol package` at a directory without one",
                    ignored.join("/"),
                    path.display(),
                );
            }
            Manifest::from_toml(&toml_src)
                .with_context(|| format!("invalid manifest at {}", path.display()))?
        }
        None => {
            if scripts.is_empty() {
                anyhow::bail!(
                    "no zyp.toml found under {}, and no --script given\n  \
                     = help: pass --script <file.zy> (repeatable) to synthesize a manifest for this run",
                    manifest_dir.display()
                );
            }
            let synthesized = synthesize_manifest(&manifest_dir, &scripts, name, version);
            println!("# no zyp.toml found — synthesized one for this run.");
            println!("# save this as {}/zyp.toml to reuse it next time:", manifest_dir.display());
            println!();
            print!("{}", synthesized.to_toml());
            println!();
            synthesized
        }
    };

    if manifest.scripts.is_empty() {
        anyhow::bail!("zyp.toml declares no [[script]] entries");
    }

    let mut script_abs_paths = Vec::with_capacity(manifest.scripts.len());
    for script in &manifest.scripts {
        let abs = manifest_dir.join(&script.path);
        if !abs.is_file() {
            anyhow::bail!("script '{}' not found: {}", script.name, abs.display());
        }
        reject_if_module_file(&script.name, &abs)?;
        script_abs_paths.push(abs);
    }

    let closure = compute_closure(&script_abs_paths)
        .with_context(|| "failed to walk the dependency closure")?;

    for w in &closure.warnings {
        eprintln!("warning: {w}");
    }

    if dry_run {
        println!("root: {}", closure.root.display());
        println!("{} file(s) would be packaged:", closure.files.len());
        for f in &closure.files {
            println!("  {}", f.rel_path);
        }
        println!("{} warning(s)", closure.warnings.len());
        return Ok(());
    }

    let output_path = output.unwrap_or_else(|| {
        PathBuf::from(format!("{}-{}.zyp", manifest.package.name, manifest.package.version))
    });

    let extra_warnings = write_zyp(&manifest, &closure, &output_path)
        .with_context(|| format!("failed to write {}", output_path.display()))?;
    for w in &extra_warnings {
        eprintln!("warning: {w}");
    }

    let total_bytes: u64 = closure
        .files
        .iter()
        .map(|f| fs::metadata(&f.abs_path).map(|m| m.len()).unwrap_or(0))
        .sum();
    println!(
        "✓ {} ({} file(s), {} bytes)",
        output_path.display(),
        closure.files.len(),
        total_bytes
    );

    Ok(())
}

/// Builds a manifest from `--script` flags when no `zyp.toml` exists. The first script
/// becomes `default = true`; each script's `name` is its file stem (so `go.zy` becomes
/// `go`, `囲碁.zy` becomes `囲碁`). `engine` always uses `>=`, never a bare version — a bare
/// pre-1.0 version is a semver caret requirement that matches *only* that exact version
/// (see `Manifest::check_engine`'s doc comment), which would break this package against
/// every other patch release of the interpreter.
fn synthesize_manifest(manifest_dir: &Path, scripts: &[String], name: Option<String>, version: Option<String>) -> Manifest {
    let name = name.unwrap_or_else(|| {
        manifest_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("package")
            .to_string()
    });
    let version = version.unwrap_or_else(|| "0.1.0".to_string());

    let script_entries = scripts
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let script_name = Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(path)
                .to_string();
            ScriptEntry { name: script_name, path: path.clone(), default: i == 0, desc: None }
        })
        .collect();

    Manifest {
        package: PackageMeta {
            name,
            version,
            engine: Some(format!(">={}", env!("CARGO_PKG_VERSION"))),
            mode: Some(EngineMode::Vm),
            desc: None,
            authors: Vec::new(),
            license: None,
        },
        scripts: script_entries,
    }
}

/// Enforces the one hard rule in an otherwise-permissive packaging policy: a `[[script]]`
/// that turns out to be a module file (has a `#` module declaration) can never run —
/// `run_file_inner` rejects it the same way for a loose `.zy` file (see the module-file
/// guard above in `run_file_inner`) — so a package built from one would never work.
fn reject_if_module_file(script_name: &str, abs_path: &Path) -> Result<()> {
    let source = fs::read_to_string(abs_path)
        .with_context(|| format!("failed to read script '{}': {}", script_name, abs_path.display()))?;
    let lexer = Lexer::new(&source, zymbol_span::FileId(0));
    let (tokens, _lex_diagnostics) = lexer.tokenize();
    let parser = ZParser::new(tokens);
    if let Ok(program) = parser.parse() {
        if program.module_decl.is_some() {
            return Err(PackageError::ScriptIsModule {
                name: script_name.to_string(),
                path: abs_path.display().to_string(),
            }
            .into());
        }
    }
    // A parse error here isn't this check's job to report — run_file_inner will surface it
    // properly (with real diagnostics) the moment someone tries to run the packaged script.
    Ok(())
}

/// What checking one file produced.
struct FileCheck {
    has_errors: bool,
    /// Style warnings printed (only counted for the file named on the command line)
    warnings: usize,
    /// Non-stdlib imports, resolved to real paths, with the span of the import
    imports: Vec<(PathBuf, zymbol_span::Span)>,
}

/// Path as the user would type it, for diagnostics.
fn display_path(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(&cwd).ok().map(|p| p.to_string_lossy().into_owned()))
        .unwrap_or_else(|| path.display().to_string())
}

/// Check the whole program, not just the file named on the command line.
///
/// A module's errors only surfaced at run time before: `zymbol check main.zy`
/// reported "No errors or warnings" while `main.zy`'s import target failed to
/// parse. Imports are followed transitively (stdlib excluded — it isn't on
/// disk) and each module is checked with the same rules as the entry file.
/// Style warnings stay with the entry file: they are per-file advice, and
/// checking a module directly still reports its own.
fn check_file(path: PathBuf) -> Result<()> {
    let source = fs::read_to_string(&path)
        .with_context(|| format!("failed to read file: {}", path.display()))?;

    let mut source_map = SourceMap::new();
    let entry = check_source(&path, &source, &mut source_map, true)?;
    let mut has_errors = entry.has_errors;

    // Walk the import graph depth-first so a cycle can be reported with the
    // chain that produced it, the way the interpreter reports E004.
    let mut visited: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    visited.insert(canonical(&path));
    let mut stack: Vec<(PathBuf, zymbol_span::Span, PathBuf)> = entry
        .imports
        .into_iter()
        .map(|(target, span)| (target, span, path.clone()))
        .collect();
    stack.reverse();

    while let Some((module_path, import_span, importer)) = stack.pop() {
        if !visited.insert(canonical(&module_path)) {
            continue; // already checked through another import
        }

        let module_source = match fs::read_to_string(&module_path) {
            Ok(s) => s,
            Err(_) => {
                // The importing file's own module analysis already reports an
                // unreadable import as E002; nothing to add here.
                continue;
            }
        };

        let module = check_source(&module_path, &module_source, &mut source_map, false)?;
        if module.has_errors {
            eprintln!(
                "  = note: reached from {} ({}:{})",
                display_path(&importer),
                import_span.start.line,
                import_span.start.column
            );
            eprintln!();
            has_errors = true;
        }
        for (target, span) in module.imports.into_iter().rev() {
            stack.push((target, span, module_path.clone()));
        }
    }

    if has_errors {
        std::process::exit(1);
    }

    if entry.warnings > 0 {
        println!("Checked with {} warning(s)", entry.warnings);
    } else {
        println!("No errors or warnings");
    }
    Ok(())
}

/// Absolute, symlink-free form of `path`, falling back to the path itself.
fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Check a single file. `report_warnings` prints style warnings (unused
/// variables, ambiguous lifetimes); errors are always reported.
fn check_source(
    path: &Path,
    source: &str,
    source_map: &mut SourceMap,
    report_warnings: bool,
) -> Result<FileCheck> {
    let file_id = source_map.add_file(display_path(path), source.to_string());

    // Lex
    let lexer = Lexer::new(source, file_id);
    let (tokens, lex_diagnostics) = lexer.tokenize();
    // Kept for the stdlib check below, which runs over tokens (the parser
    // consumes the vector).
    let tokens_for_stdlib = tokens.clone();

    let mut has_errors = false;

    if !lex_diagnostics.is_empty() {
        let mut bag = DiagnosticBag::new();
        for diag in lex_diagnostics {
            bag.add(diag);
        }
        bag.emit_all(source_map);
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
            bag.emit_all(source_map);
            // Without an AST there is nothing further to analyse in this file,
            // and no imports to follow.
            return Ok(FileCheck {
                has_errors: true,
                warnings: 0,
                imports: Vec::new(),
            });
        }
    };

    if has_errors {
        return Ok(FileCheck {
            has_errors: true,
            warnings: 0,
            imports: Vec::new(),
        });
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
        bag.emit_all(source_map);
        has_errors = true;
    }

    // Report variable warnings (unused variables, write-only variables)
    if report_warnings && !warnings.is_empty() {
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

    // Run type checking. Feeding it the imports' arities extends the existing
    // argument-count check to `alias::func(...)`, which used to go unchecked —
    // a wrong count there only ever failed at runtime, on whichever branch
    // happened to make the call.
    let mut type_checker = TypeChecker::new();
    type_checker.set_module_arities(zymbol_semantic::module_arities(
        &program.imports,
        path.parent().unwrap_or(std::path::Path::new(".")),
    ));
    let type_errors = type_checker.check_errors(&program);

    // Type errors are fatal
    if !type_errors.is_empty() {
        let mut bag = DiagnosticBag::new();
        for err in type_errors {
            bag.add(err);
        }
        bag.emit_all(source_map);
        has_errors = true;
    }

    // Report type warnings
    let mut type_warning_count = 0;
    for diag in type_checker.get_warnings() {
        if !report_warnings {
            continue;
        }
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

    if let Err(module_errors) = module_analyzer.analyze(&program, path) {
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
        module_analyzer.validate_exports(&program, path);
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

    // Validate uses of std/ modules against their export table. Same function
    // the LSP calls, so the editor and the CLI flag the same things.
    let stdlib_diagnostics = zymbol_semantic::check_stdlib_access(&tokens_for_stdlib, &program.imports);
    if !stdlib_diagnostics.is_empty() {
        let mut bag = DiagnosticBag::new();
        for diag in stdlib_diagnostics {
            bag.add(diag);
        }
        bag.emit_all(source_map);
        has_errors = true;
    }

    // Run def-use analysis for lifetime detection
    let cfg = ControlFlowGraph::build_sequential(&program.statements);
    let mut def_use_analyzer = DefUseAnalyzer::new();
    let _chains = def_use_analyzer.analyze(&program.statements, &cfg);

    // Report ambiguous lifetime warnings
    let ambiguous_vars = def_use_analyzer.get_ambiguous_variables();
    let mut lifetime_warning_count = 0;
    for chain in &ambiguous_vars {
        if !report_warnings {
            break;
        }
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

    // Collect the imports to follow. Stdlib modules have no file on disk, so
    // `resolve_from` returns None for them and they drop out here.
    let base_dir = path.parent().unwrap_or(std::path::Path::new("."));
    let imports = program
        .imports
        .iter()
        .filter_map(|import| {
            import
                .path
                .resolve_from(base_dir)
                .map(|resolved| (resolved, import.span))
        })
        .collect();

    let total_warnings = if report_warnings {
        warnings.len() + type_warning_count + lifetime_warning_count
    } else {
        0
    };

    Ok(FileCheck {
        has_errors,
        warnings: total_warnings,
        imports,
    })
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
