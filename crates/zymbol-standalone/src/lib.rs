//! Standalone Executable Builder for Zymbol-Lang
//!
//! Creates self-contained executables that embed source code + full compiler

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// Builder for standalone executables
pub struct StandaloneBuilder {
    source_code: String,
    output_path: PathBuf,
    release: bool,
}

impl StandaloneBuilder {
    /// Create a new standalone builder from source code
    pub fn new_from_source(source_code: String, output_path: PathBuf, release: bool) -> Self {
        Self {
            source_code,
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

        // Embed source code
        self.write_source(build_dir)?;

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

        let span_path = current_dir.join("crates/zymbol-span").canonicalize()?;
        let error_path = current_dir.join("crates/zymbol-error").canonicalize()?;
        let common_path = current_dir.join("crates/zymbol-common").canonicalize()?;
        let ast_path = current_dir.join("crates/zymbol-ast").canonicalize()?;
        let lexer_path = current_dir.join("crates/zymbol-lexer").canonicalize()?;
        let parser_path = current_dir.join("crates/zymbol-parser").canonicalize()?;
        let interpreter_path = current_dir.join("crates/zymbol-interpreter").canonicalize()?;

        // Create Cargo.toml
        let cargo_toml = format!(r#"[package]
name = "zymbol-program"
version = "0.1.0"
edition = "2021"

[dependencies]
zymbol-span = {{ path = "{}" }}
zymbol-error = {{ path = "{}" }}
zymbol-common = {{ path = "{}" }}
zymbol-ast = {{ path = "{}" }}
zymbol-lexer = {{ path = "{}" }}
zymbol-parser = {{ path = "{}" }}
zymbol-interpreter = {{ path = "{}" }}

[profile.release]
opt-level = 3
lto = true
strip = true
codegen-units = 1
"#,
            span_path.display(),
            error_path.display(),
            common_path.display(),
            ast_path.display(),
            lexer_path.display(),
            parser_path.display(),
            interpreter_path.display()
        );

        fs::write(build_dir.join("Cargo.toml"), cargo_toml)
            .context("Failed to write Cargo.toml")?;

        Ok(())
    }

    fn write_source(&self, build_dir: &Path) -> Result<()> {
        println!("  → Embedding source code ({} bytes)...", self.source_code.len());

        // Escape the source code for Rust string literal
        let escaped = self.source_code
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");

        let source_file = format!("pub const SOURCE: &str = \"{}\";\n", escaped);

        fs::write(build_dir.join("src/source.rs"), source_file)
            .context("Failed to write source.rs")?;

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
        let builder = StandaloneBuilder::new_from_source(source.clone(), output, true);

        assert_eq!(builder.source_code, source);
    }
}
