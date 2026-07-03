//! Standalone Executable Builder for Zymbol-Lang
//!
//! Creates self-contained executables that embed source code + full compiler

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use zymbol_bytecode::CompiledProgram;
use zymbol_compiler::Compiler;
use zymbol_lexer::Lexer;
use zymbol_parser::Parser;
use zymbol_span::FileId;

/// Builder for standalone executables
pub struct StandaloneBuilder {
    source_code: String,
    base_dir: Option<PathBuf>,
    output_path: PathBuf,
    release: bool,
}

impl StandaloneBuilder {
    /// Create a new standalone builder from source code
    pub fn new_from_source(source_code: String, base_dir: Option<PathBuf>, output_path: PathBuf, release: bool) -> Self {
        Self {
            source_code,
            base_dir,
            output_path,
            release,
        }
    }

    /// Build the standalone executable
    pub fn build(&self) -> Result<()> {
        println!("Building standalone executable...");

        // Create temporary build directory
        let temp_dir = TempDir::new().context("Failed to create temp directory")?;
        let build_dir = temp_dir.path();

        println!("  → Temp build dir: {}", build_dir.display());

        // Setup project structure
        self.setup_project(build_dir)?;

        // Compile to bytecode and embed
        self.write_bytecode(build_dir)?;

        // Build with cargo
        self.cargo_build(build_dir)?;

        // Copy final executable
        self.copy_executable(build_dir)?;

        println!("✓ Standalone executable created: {}", self.output_path.display());

        Ok(())
    }

    fn setup_project(&self, build_dir: &Path) -> Result<()> {
        println!("  → Setting up project structure...");

        // Create src directory
        fs::create_dir_all(build_dir.join("src"))
            .context("Failed to create src directory")?;

        // Write main.rs from template
        let template = include_str!("../template/main.rs.template");
        fs::write(build_dir.join("src/main.rs"), template)
            .context("Failed to write main.rs")?;

        // Write Cargo.toml
        self.write_cargo_toml(build_dir)?;

        Ok(())
    }

    fn write_cargo_toml(&self, build_dir: &Path) -> Result<()> {
        // Get absolute paths to required crates
        let current_dir = std::env::current_dir()?;

        let bytecode_path = current_dir.join("crates/zymbol-bytecode").canonicalize()?;
        let vm_path = current_dir.join("crates/zymbol-vm").canonicalize()?;

        let cargo_toml = format!(r#"[package]
name = "zymbol-program"
version = "0.1.0"
edition = "2024"

[dependencies]
zymbol-bytecode = {{ path = "{}" }}
zymbol-vm = {{ path = "{}" }}
bincode = "1.3"
serde = {{ version = "1.0", features = ["derive"] }}

[profile.release]
opt-level = 3
lto = true
strip = true
codegen-units = 1
"#,
            bytecode_path.display(),
            vm_path.display()
        );

        fs::write(build_dir.join("Cargo.toml"), cargo_toml)
            .context("Failed to write Cargo.toml")?;

        Ok(())
    }

    fn compile_to_bytecode(&self) -> Result<CompiledProgram> {
        let lexer = Lexer::new(&self.source_code, FileId(0));
        let (tokens, _) = lexer.tokenize();
        let parser = Parser::new(tokens);
        let program = parser.parse().map_err(|e| anyhow::anyhow!("{:?}", e))?;
        Compiler::compile_with_dir(&program, self.base_dir.as_deref())
            .map_err(|e| anyhow::anyhow!("{:?}", e))
    }

    fn write_bytecode(&self, build_dir: &Path) -> Result<()> {
        let compiled = self.compile_to_bytecode()
            .context("Failed to compile source to bytecode")?;

        let bytes: Vec<u8> = bincode::serialize(&compiled)
            .context("Failed to serialize bytecode")?;

        println!("  → Embedding bytecode ({} bytes)...", bytes.len());

        let mut literal = String::from("pub static BYTECODE: &[u8] = &[");
        for (i, b) in bytes.iter().enumerate() {
            if i % 16 == 0 { literal.push_str("\n    "); }
            literal.push_str(&format!("{},", b));
        }
        literal.push_str("\n];\n");

        fs::write(build_dir.join("src/bytecode.rs"), literal)
            .context("Failed to write bytecode.rs")?;

        Ok(())
    }

    fn cargo_build(&self, build_dir: &Path) -> Result<()> {
        println!("  → Compiling with cargo (this may take a moment)...");

        let mut cmd = Command::new("cargo");
        cmd.arg("build")
            .arg("--quiet")
            .current_dir(build_dir);

        if self.release {
            cmd.arg("--release");
        }

        let status = cmd.status()
            .context("Failed to run cargo build")?;

        if !status.success() {
            anyhow::bail!("Cargo build failed");
        }

        Ok(())
    }

    fn copy_executable(&self, build_dir: &Path) -> Result<()> {
        let executable_name = if cfg!(windows) {
            "zymbol-program.exe"
        } else {
            "zymbol-program"
        };

        let profile = if self.release { "release" } else { "debug" };
        let source = build_dir
            .join(format!("target/{}", profile))
            .join(executable_name);

        if !source.exists() {
            anyhow::bail!("Built executable not found at: {}", source.display());
        }

        // Create parent directory if needed
        if let Some(parent) = self.output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(&source, &self.output_path)
            .with_context(|| format!("Failed to copy executable to {}", self.output_path.display()))?;

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&self.output_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&self.output_path, perms)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standalone_builder_creation() {
        let source = r#">> "Hello""#.to_string();
        let output = PathBuf::from("/tmp/test");
        let builder = StandaloneBuilder::new_from_source(source.clone(), None, output, true);

        assert_eq!(builder.source_code, source);
    }
}
