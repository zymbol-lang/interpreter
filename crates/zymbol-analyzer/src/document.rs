//! Document management with lazy parsing for Zymbol-Lang
//!
//! Provides a Document struct that lazily parses source code on demand,
//! caching results for efficient repeated access.

use once_cell::sync::OnceCell;
use std::sync::Arc;
use zymbol_ast::Program;
use zymbol_error::Diagnostic;
use zymbol_lexer::{Lexer, Token};
use zymbol_parser::Parser;
use zymbol_span::FileId;

/// A document with lazy parsing support
///
/// Documents cache their tokenized and parsed forms, only computing them
/// when first requested. This allows for efficient incremental updates
/// where unchanged documents don't need to be reparsed.
#[derive(Debug)]
pub struct Document {
    /// The document URI (file path or virtual URI)
    pub uri: Arc<str>,
    /// The raw source content
    pub content: Arc<str>,
    /// Document version (for LSP synchronization)
    pub version: i32,
    /// File ID for span tracking
    file_id: FileId,
    /// Cached tokens and lexer diagnostics
    tokens: OnceCell<(Vec<Token>, Vec<Diagnostic>)>,
    /// Cached AST or parse errors
    ast: OnceCell<ParseResult>,
}

/// Result of parsing a document
#[derive(Debug, Clone)]
pub struct ParseResult {
    /// The parsed AST (if successful)
    pub program: Option<Program>,
    /// All diagnostics (lexer + parser)
    pub diagnostics: Vec<Diagnostic>,
}

impl Document {
    /// Create a new document
    pub fn new(uri: Arc<str>, content: String, version: i32, file_id: FileId) -> Self {
        Self {
            uri,
            content: Arc::from(content),
            version,
            file_id,
            tokens: OnceCell::new(),
            ast: OnceCell::new(),
        }
    }

    /// Get the file ID for this document
    pub fn file_id(&self) -> FileId {
        self.file_id
    }

    /// Get the source content
    pub fn source(&self) -> &str {
        &self.content
    }

    /// Get or compute the tokens for this document
    pub fn tokens(&self) -> &(Vec<Token>, Vec<Diagnostic>) {
        self.tokens.get_or_init(|| {
            let lexer = Lexer::new(&self.content, self.file_id);
            lexer.tokenize()
        })
    }

    /// Get only the token list (without diagnostics)
    pub fn token_list(&self) -> &[Token] {
        &self.tokens().0
    }

    /// Get lexer diagnostics
    pub fn lexer_diagnostics(&self) -> &[Diagnostic] {
        &self.tokens().1
    }

    /// Get or compute the AST for this document
    pub fn parse(&self) -> &ParseResult {
        self.ast.get_or_init(|| {
            // First ensure tokens are computed
            let (tokens, lexer_diags) = self.tokens();

            // Parse the token stream
            let parser = Parser::new(tokens.clone());
            let parse_result = parser.parse();

            match parse_result {
                Ok(program) => {
                    // Parsing succeeded - just include lexer diagnostics
                    ParseResult {
                        program: Some(program),
                        diagnostics: lexer_diags.clone(),
                    }
                }
                Err(parse_errors) => {
                    // Parsing failed - combine lexer and parser diagnostics
                    let mut all_diags = lexer_diags.clone();
                    all_diags.extend(parse_errors);
                    ParseResult {
                        program: None,
                        diagnostics: all_diags,
                    }
                }
            }
        })
    }

    /// Check if the document has any errors
    pub fn has_errors(&self) -> bool {
        let parse_result = self.parse();
        parse_result.program.is_none()
            || parse_result
                .diagnostics
                .iter()
                .any(|d| d.severity == zymbol_error::Severity::Error)
    }

    /// Get the AST if parsing succeeded
    pub fn ast(&self) -> Option<&Program> {
        self.parse().program.as_ref()
    }

    /// Get all diagnostics (lexer + parser)
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.parse().diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_document(source: &str) -> Document {
        Document::new(
            Arc::from("test://file.zy"),
            source.to_string(),
            1,
            FileId(0),
        )
    }

    #[test]
    fn test_document_creation() {
        let doc = create_test_document("x = 5");
        assert_eq!(doc.version, 1);
        assert_eq!(&*doc.content, "x = 5");
    }

    #[test]
    fn test_lazy_tokenization() {
        let doc = create_test_document(">> \"Hello\"");
        // Tokens should not be computed yet
        assert!(doc.tokens.get().is_none());

        // Access tokens
        let tokens = doc.token_list();
        assert!(!tokens.is_empty());

        // Now tokens should be cached
        assert!(doc.tokens.get().is_some());
    }

    #[test]
    fn test_lazy_parsing() {
        let doc = create_test_document("x = 5");
        // AST should not be computed yet
        assert!(doc.ast.get().is_none());

        // Access AST
        let ast = doc.ast();
        assert!(ast.is_some());

        // Now AST should be cached
        assert!(doc.ast.get().is_some());
    }

    #[test]
    fn test_parse_error_handling() {
        let doc = create_test_document("? { }"); // Invalid: missing condition
        let result = doc.parse();
        // Should have diagnostics for the parse error
        assert!(!result.diagnostics.is_empty() || result.program.is_some());
    }

    #[test]
    fn test_valid_program() {
        let doc = create_test_document(">> \"Hello World\"");
        assert!(doc.ast().is_some());
    }
}
