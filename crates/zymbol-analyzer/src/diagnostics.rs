//! Diagnostic conversion for Zymbol-Lang LSP
//!
//! Converts Zymbol's internal diagnostic format to LSP diagnostic format,
//! handling the coordinate system conversion (1-indexed to 0-indexed).

use lsp_types::{DiagnosticSeverity, NumberOrString, Position, Range};
use zymbol_error::{Diagnostic as ZymbolDiagnostic, Severity as ZymbolSeverity};
use zymbol_semantic::{Severity as VarSeverity, VariableDiagnostic};
use zymbol_span::Span;

/// Convert a Zymbol Span to an LSP Range
///
/// Zymbol uses 1-indexed lines and columns, while LSP uses 0-indexed.
/// This function handles the conversion.
pub fn span_to_range(span: &Span) -> Range {
    Range {
        start: Position {
            line: span.start.line.saturating_sub(1),
            character: span.start.column.saturating_sub(1),
        },
        end: Position {
            line: span.end.line.saturating_sub(1),
            character: span.end.column.saturating_sub(1),
        },
    }
}

/// Convert a Zymbol Severity to LSP DiagnosticSeverity
fn severity_to_lsp(severity: ZymbolSeverity) -> DiagnosticSeverity {
    match severity {
        ZymbolSeverity::Error => DiagnosticSeverity::ERROR,
        ZymbolSeverity::Warning => DiagnosticSeverity::WARNING,
        ZymbolSeverity::Note => DiagnosticSeverity::INFORMATION,
    }
}

/// Convert a VariableAnalyzer Severity to LSP DiagnosticSeverity
fn var_severity_to_lsp(severity: VarSeverity) -> DiagnosticSeverity {
    match severity {
        VarSeverity::Warning => DiagnosticSeverity::WARNING,
        VarSeverity::Info => DiagnosticSeverity::INFORMATION,
    }
}

/// Convert a Zymbol Diagnostic to an LSP Diagnostic
pub fn to_lsp_diagnostic(diag: &ZymbolDiagnostic) -> lsp_types::Diagnostic {
    let range = diag
        .span
        .as_ref()
        .map(span_to_range)
        .unwrap_or_else(|| Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        });

    let mut message = diag.message.clone();

    // Append notes to the message
    if !diag.notes.is_empty() {
        message.push_str("\n\n");
        for note in &diag.notes {
            message.push_str("note: ");
            message.push_str(note);
            message.push('\n');
        }
    }

    // Append help to the message
    if let Some(help) = &diag.help {
        message.push_str("\nhelp: ");
        message.push_str(help);
    }

    lsp_types::Diagnostic {
        range,
        severity: Some(severity_to_lsp(diag.severity)),
        code: None,
        code_description: None,
        source: Some("zymbol".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Convert a VariableDiagnostic to an LSP Diagnostic
pub fn var_diagnostic_to_lsp(diag: &VariableDiagnostic) -> lsp_types::Diagnostic {
    let range = span_to_range(&diag.span);

    let mut message = diag.message.clone();
    if let Some(help) = &diag.help {
        message.push_str("\n\nhelp: ");
        message.push_str(help);
    }

    // Determine tags - unused variables get the "unnecessary" tag
    let tags = if diag.message.contains("unused") {
        Some(vec![lsp_types::DiagnosticTag::UNNECESSARY])
    } else {
        None
    };

    lsp_types::Diagnostic {
        range,
        severity: Some(var_severity_to_lsp(diag.severity)),
        code: Some(NumberOrString::String("unused-variable".to_string())),
        code_description: None,
        source: Some("zymbol".to_string()),
        message,
        related_information: None,
        tags,
        data: None,
    }
}

/// Pipeline for collecting all diagnostics from a document
pub struct DiagnosticPipeline;

impl DiagnosticPipeline {
    /// Collect all diagnostics for a document
    ///
    /// This runs the full pipeline:
    /// 1. Lexer diagnostics (from tokenization)
    /// 2. Parser diagnostics (from parsing)
    /// 3. Semantic diagnostics (from variable analysis)
    pub fn collect(document: &crate::document::Document) -> Vec<lsp_types::Diagnostic> {
        let mut lsp_diagnostics = Vec::new();

        // Get parse result (includes lexer + parser diagnostics)
        let parse_result = document.parse();

        // Convert lexer and parser diagnostics
        for diag in &parse_result.diagnostics {
            lsp_diagnostics.push(to_lsp_diagnostic(diag));
        }

        // Run semantic analysis if we have an AST
        if let Some(program) = &parse_result.program {
            // Variable analysis
            let mut var_analyzer = zymbol_semantic::VariableAnalyzer::new();
            let var_diagnostics = var_analyzer.analyze(program);

            // Convert variable diagnostics
            for var_diag in &var_diagnostics {
                lsp_diagnostics.push(var_diagnostic_to_lsp(var_diag));
            }

            // Include semantic errors from variable analyzer
            for semantic_error in var_analyzer.semantic_errors() {
                lsp_diagnostics.push(to_lsp_diagnostic(semantic_error));
            }

            // The document's path on disk, when it has one. Needed to resolve
            // imports — both for the arity table below and for module analysis.
            // Virtual documents have none and skip those passes.
            let path = crate::workspace::uri_to_path(&document.uri).or_else(|| {
                // Plain paths (no file:// scheme) are used by tests
                // and tooling — accept them when they exist on disk.
                let p = std::path::PathBuf::from(document.uri.as_ref());
                p.exists().then_some(p)
            });

            // Type checking. The arity table makes `alias::func(...)` checked
            // in the editor exactly as `zymbol check` checks it — without it
            // the two disagree, and the editor is the one people believe.
            let mut type_checker = zymbol_semantic::TypeChecker::new();
            if let Some(base_dir) = path.as_deref().and_then(|p| p.parent()) {
                type_checker.set_module_arities(zymbol_semantic::module_arities(
                    &program.imports,
                    base_dir,
                ));
            }
            let type_diagnostics = type_checker.check(program);

            // Convert type diagnostics
            for type_diag in &type_diagnostics {
                lsp_diagnostics.push(to_lsp_diagnostic(type_diag));
            }

            // Loop context: `@!`/`@>` need an enclosing loop, and `@:L!` needs
            // one labelled L. Same function `zymbol check` calls, so a mistyped
            // label is underlined as you write it rather than at run time.
            for diag in zymbol_semantic::check_loop_context(program) {
                lsp_diagnostics.push(to_lsp_diagnostic(&diag));
            }

            // Module analysis — same pass as `zymbol check` (E001 name
            // mismatch, E002 module not found, E009 duplicate export, export
            // validation). Resolving imported files needs a real filesystem
            // path, so virtual documents without one skip this pass.
            if program.module_decl.is_some() || !program.imports.is_empty() {
                if let Some(path) = path {
                    if let Some(base_dir) = path.parent() {
                        let mut module_analyzer =
                            zymbol_semantic::ModuleAnalyzer::new(base_dir);
                        // analyze() drains its findings into the Err; only
                        // validate_exports findings remain in diagnostics().
                        if let Err(module_errors) = module_analyzer.analyze(program, &path) {
                            for err in &module_errors {
                                lsp_diagnostics.push(to_lsp_diagnostic(err));
                            }
                        }
                        module_analyzer.validate_exports(program, &path);
                        for diag in module_analyzer.diagnostics() {
                            lsp_diagnostics.push(to_lsp_diagnostic(diag));
                        }
                    }
                }
            }

            // Uses of std/ modules, checked against their export table — the
            // same function `zymbol check` calls, so both agree.
            for diag in
                zymbol_semantic::check_stdlib_access(document.token_list(), &program.imports)
            {
                lsp_diagnostics.push(to_lsp_diagnostic(&diag));
            }

            // Def-use analysis for ambiguous lifetimes
            let cfg = zymbol_semantic::ControlFlowGraph::build_sequential(&program.statements);
            let mut def_use_analyzer = zymbol_semantic::DefUseAnalyzer::new();
            let _chains = def_use_analyzer.analyze(&program.statements, &cfg);

            // Report ambiguous lifetime warnings
            for chain in def_use_analyzer.get_ambiguous_variables() {
                if let Some(ambiguity) = &chain.ambiguity {
                    let reason_str = match ambiguity.reason {
                        zymbol_semantic::AmbiguityReason::LoopVariant => "variable is modified inside a loop",
                        zymbol_semantic::AmbiguityReason::ConditionalUse => "variable is used in some branches but not others",
                        zymbol_semantic::AmbiguityReason::MultipleExitPaths => "multiple possible last uses",
                    };
                    let message = format!(
                        "ambiguous lifetime for '{}': {}",
                        chain.variable,
                        reason_str
                    );
                    let range = span_to_range(&ambiguity.suggested_span);

                    lsp_diagnostics.push(lsp_types::Diagnostic {
                        range,
                        // WARNING to match `zymbol check`, which reports the
                        // same finding as a warning (severity parity audit,
                        // 2026-06-12).
                        severity: Some(DiagnosticSeverity::WARNING),
                        code: Some(NumberOrString::String("ambiguous-lifetime".to_string())),
                        code_description: None,
                        source: Some("zymbol".to_string()),
                        message,
                        related_information: None,
                        tags: None,
                        data: None,
                    });
                }
            }
        }

        lsp_diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_span::{FileId, Position as ZymbolPosition};

    fn create_span(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Span {
        Span::new(
            ZymbolPosition::new(start_line, start_col, 0),
            ZymbolPosition::new(end_line, end_col, 0),
            FileId(0),
        )
    }

    #[test]
    fn test_span_to_range_basic() {
        // Zymbol: line 1, column 1 -> LSP: line 0, character 0
        let span = create_span(1, 1, 1, 5);
        let range = span_to_range(&span);

        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 0);
        assert_eq!(range.end.line, 0);
        assert_eq!(range.end.character, 4);
    }

    #[test]
    fn test_span_to_range_multiline() {
        let span = create_span(5, 10, 7, 20);
        let range = span_to_range(&span);

        assert_eq!(range.start.line, 4);
        assert_eq!(range.start.character, 9);
        assert_eq!(range.end.line, 6);
        assert_eq!(range.end.character, 19);
    }

    #[test]
    fn test_span_to_range_saturating() {
        // Edge case: Zymbol line/column 0 should become LSP 0 (not underflow)
        let span = create_span(0, 0, 0, 0);
        let range = span_to_range(&span);

        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 0);
    }

    #[test]
    fn test_diagnostic_conversion() {
        let span = create_span(1, 1, 1, 10);
        let diag = ZymbolDiagnostic::error("test error")
            .with_span(span)
            .with_note("this is a note")
            .with_help("try this");

        let lsp_diag = to_lsp_diagnostic(&diag);

        assert_eq!(lsp_diag.severity, Some(DiagnosticSeverity::ERROR));
        assert!(lsp_diag.message.contains("test error"));
        assert!(lsp_diag.message.contains("this is a note"));
        assert!(lsp_diag.message.contains("try this"));
        assert_eq!(lsp_diag.source, Some("zymbol".to_string()));
    }

    #[test]
    fn test_diagnostic_without_span() {
        let diag = ZymbolDiagnostic::error("test error without span");
        let lsp_diag = to_lsp_diagnostic(&diag);

        // Should use default range (0, 0)
        assert_eq!(lsp_diag.range.start.line, 0);
        assert_eq!(lsp_diag.range.start.character, 0);
    }

    #[test]
    fn test_var_diagnostic_unused_tag() {
        let span = create_span(1, 1, 1, 5);
        let var_diag = VariableDiagnostic {
            severity: VarSeverity::Warning,
            message: "unused variable 'x'".to_string(),
            span,
            help: Some("remove or use the variable".to_string()),
        };

        let lsp_diag = var_diagnostic_to_lsp(&var_diag);

        assert_eq!(lsp_diag.severity, Some(DiagnosticSeverity::WARNING));
        assert!(lsp_diag.tags.is_some());
        assert!(lsp_diag
            .tags
            .as_ref()
            .unwrap()
            .contains(&lsp_types::DiagnosticTag::UNNECESSARY));
    }

    /// Build a Document from a real corpus fixture (module analysis needs a
    /// filesystem path to resolve imports).
    fn doc_from_fixture(rel: &str) -> crate::document::Document {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel)
            .canonicalize()
            .expect("fixture exists");
        let content = std::fs::read_to_string(&path).expect("read fixture");
        crate::document::Document::new(
            crate::workspace::path_to_uri(&path),
            content,
            0,
            FileId(0),
        )
    }

    /// Diagnostic-parity audit (2026-06-12): the LSP pipeline must surface the
    /// same module errors as `zymbol check` — exactly once each.
    #[test]
    fn test_module_errors_in_pipeline() {
        let cases = [
            ("tests/errors/semantic/E001_mismatch_mod.zy", "E001"),
            ("tests/errors/semantic/E002_mod_not_found.zy", "E002"),
            ("tests/errors/semantic/E009_dup_export.zy", "E009"),
        ];
        for (fixture, code) in cases {
            let doc = doc_from_fixture(fixture);
            let diags = DiagnosticPipeline::collect(&doc);
            let matches: Vec<_> = diags
                .iter()
                .filter(|d| d.message.starts_with(code))
                .collect();
            assert_eq!(
                matches.len(),
                1,
                "{fixture}: expected exactly one {code} diagnostic, got {}",
                matches.len()
            );
            assert_eq!(matches[0].severity, Some(DiagnosticSeverity::ERROR));
        }
    }

    /// Diagnostic-parity audit (2026-06-12): ambiguous-lifetime findings carry
    /// WARNING severity, matching `zymbol check`.
    #[test]
    fn test_ambiguous_lifetime_is_warning() {
        let doc = crate::document::Document::new(
            std::sync::Arc::from("untitled:lifetime"),
            "@ i:1..3 {\n    >> i ¶\n    i = i + 1\n}\n".to_string(),
            0,
            FileId(0),
        );
        let diags = DiagnosticPipeline::collect(&doc);
        let lifetime: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("ambiguous lifetime"))
            .collect();
        assert!(!lifetime.is_empty(), "expected an ambiguous-lifetime diagnostic");
        for d in lifetime {
            assert_eq!(d.severity, Some(DiagnosticSeverity::WARNING));
        }
    }
}
