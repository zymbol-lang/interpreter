//! Audit helper: print the LSP DiagnosticPipeline output for a .zy file as
//! plain text (severity + message), to diff against `zymbol check`.
use std::sync::Arc;
use zymbol_analyzer::diagnostics::DiagnosticPipeline;
use zymbol_analyzer::document::Document;
use zymbol_span::FileId;

fn main() {
    let path = std::env::args().nth(1).expect("usage: dump_diagnostics <file.zy>");
    let content = std::fs::read_to_string(&path).expect("read file");
    let doc = Document::new(Arc::from(path.as_str()), content, 0, FileId(0));
    for d in DiagnosticPipeline::collect(&doc) {
        let sev = match d.severity {
            Some(lsp_types::DiagnosticSeverity::ERROR) => "error",
            Some(lsp_types::DiagnosticSeverity::WARNING) => "warning",
            Some(lsp_types::DiagnosticSeverity::HINT) => "hint",
            Some(lsp_types::DiagnosticSeverity::INFORMATION) => "info",
            _ => "other",
        };
        // first line of the message only (notes/help are appended below it)
        let first = d.message.lines().next().unwrap_or("");
        println!("{}|{}:{}|{}", sev, d.range.start.line + 1, d.range.start.character + 1, first);
    }
}
