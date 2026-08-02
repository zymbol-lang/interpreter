//! Print the diagnostics the LSP would publish for the given files.
//!
//! Usage: cargo run -p zymbol-analyzer --example lsp_scan -- <file.zy|dir>...
//!
//! A directory argument is walked recursively for `.zy` files, which makes this
//! usable as a workspace-wide sweep to compare against `zymbol check`.
//!
//! Each diagnostic is printed as `path:line:col: SEVERITY [code] message`,
//! with the message flattened to a single line so the output can be diffed
//! against `zymbol check`.

use std::path::{Path, PathBuf};
use zymbol_analyzer::Analyzer;

/// Every `.zy` file under `path`, or `path` itself when it is a file.
fn collect_zy(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        out.push(path.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    let mut children: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    children.sort();
    for child in children {
        if child.is_dir() {
            collect_zy(&child, out);
        } else if child.extension().is_some_and(|e| e == "zy") {
            out.push(child);
        }
    }
}

fn main() {
    let mut files: Vec<PathBuf> = Vec::new();
    for arg in std::env::args().skip(1) {
        collect_zy(Path::new(&arg), &mut files);
    }
    if files.is_empty() {
        eprintln!("usage: lsp_scan <file.zy|dir>...");
        std::process::exit(2);
    }

    let analyzer = Analyzer::new();
    let roots: Vec<PathBuf> = files
        .iter()
        .filter_map(|f| std::fs::canonicalize(f).ok())
        .filter_map(|f| f.parent().map(|p| p.to_path_buf()))
        .collect();
    analyzer.initialize_workspace(roots);
    analyzer.scan_workspace();

    for file in &files {
        let abs = std::fs::canonicalize(file).unwrap_or_else(|_| file.clone());
        let content = match std::fs::read_to_string(&abs) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{}: {}", abs.display(), e);
                continue;
            }
        };
        let uri = format!("file://{}", abs.display());
        analyzer.open_document(uri.clone().into(), content, 1);

        for d in analyzer.get_diagnostics(&uri) {
            let sev = match d.severity {
                Some(lsp_types::DiagnosticSeverity::ERROR) => "ERROR",
                Some(lsp_types::DiagnosticSeverity::WARNING) => "WARN",
                Some(lsp_types::DiagnosticSeverity::INFORMATION) => "INFO",
                _ => "HINT",
            };
            let code = match &d.code {
                Some(lsp_types::NumberOrString::String(s)) => s.clone(),
                Some(lsp_types::NumberOrString::Number(n)) => n.to_string(),
                None => "-".to_string(),
            };
            let msg = d.message.replace('\n', " | ");
            println!(
                "{}:{}:{}: {} [{}] {}",
                file.display(),
                d.range.start.line + 1,
                d.range.start.character + 1,
                sev,
                code,
                msg
            );
        }
        analyzer.close_document(&uri);
    }
}
