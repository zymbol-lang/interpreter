//! Semantic token classification for Zymbol-Lang
//!
//! Maps Zymbol tokens to LSP semantic token types for syntax highlighting.
//! Supports delta encoding for efficient transmission.

use lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens,
    SemanticTokensLegend,
};
use zymbol_lexer::{Token, TokenKind};

/// Standard semantic token types used by Zymbol
pub const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::KEYWORD,
    SemanticTokenType::OPERATOR,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::FUNCTION,
    SemanticTokenType::PARAMETER,
    SemanticTokenType::STRING,
    SemanticTokenType::NUMBER,
    SemanticTokenType::TYPE,
    SemanticTokenType::COMMENT,
    SemanticTokenType::PROPERTY,
    SemanticTokenType::NAMESPACE,
];

/// Standard semantic token modifiers used by Zymbol
pub const TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::DECLARATION,
    SemanticTokenModifier::DEFINITION,
    SemanticTokenModifier::READONLY,
    SemanticTokenModifier::STATIC,
    SemanticTokenModifier::MODIFICATION,
];

/// Get the semantic tokens legend for Zymbol
pub fn semantic_tokens_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TOKEN_TYPES.to_vec(),
        token_modifiers: TOKEN_MODIFIERS.to_vec(),
    }
}

/// Token type indices for quick lookup
#[allow(dead_code)]
mod token_type_index {
    pub const KEYWORD: u32 = 0;
    pub const OPERATOR: u32 = 1;
    pub const VARIABLE: u32 = 2;
    pub const FUNCTION: u32 = 3;
    pub const PARAMETER: u32 = 4;
    pub const STRING: u32 = 5;
    pub const NUMBER: u32 = 6;
    pub const TYPE: u32 = 7;
    pub const COMMENT: u32 = 8;
    pub const PROPERTY: u32 = 9;
    pub const NAMESPACE: u32 = 10;
}

/// Classify a Zymbol token kind to LSP semantic token type
fn classify_token(kind: &TokenKind) -> Option<u32> {
    match kind {
        // Control flow keywords
        TokenKind::Question          // ?  (if)
        | TokenKind::ElseIf          // _? (else if)
        | TokenKind::Underscore      // _  (else/wildcard)
        | TokenKind::DoubleQuestion  // ?? (match)
        | TokenKind::At              // @  (loop)
        | TokenKind::AtBreak         // @! (break)
        | TokenKind::AtContinue      // @> (continue)
        | TokenKind::TryBlock        // !? (try)
        | TokenKind::CatchBlock      // :! (catch)
        | TokenKind::FinallyBlock    // :> (finally)
        | TokenKind::Return          // <~ (return)
        | TokenKind::Hash            // #  (module declaration)
        | TokenKind::ExportBlock     // #> (export)
        | TokenKind::ModuleImport    // <# (import)
        => Some(token_type_index::KEYWORD),

        // I/O and flow operators
        TokenKind::Output            // >>
        | TokenKind::Input           // <<
        | TokenKind::Newline         // ¶
        | TokenKind::Backslash2      // \\
        | TokenKind::Arrow           // ->
        | TokenKind::ScopeResolution // ::
        | TokenKind::PipeOp          // |>
        | TokenKind::CliArgsCapture   // ><
        | TokenKind::BashCommand(_)   // <\ command \>
        | TokenKind::ExecuteCommand(_) // </ path />
        => Some(token_type_index::OPERATOR),

        // Assignment operators
        TokenKind::Assign            // =
        | TokenKind::ConstAssign     // :=
        | TokenKind::PlusAssign      // +=
        | TokenKind::MinusAssign     // -=
        | TokenKind::StarAssign      // *=
        | TokenKind::SlashAssign     // /=
        | TokenKind::PercentAssign   // %=
        | TokenKind::CaretAssign     // ^=
        => Some(token_type_index::OPERATOR),

        // Arithmetic operators
        TokenKind::Plus              // +
        | TokenKind::Minus           // -
        | TokenKind::Star            // *
        | TokenKind::Slash           // /
        | TokenKind::Percent         // %
        | TokenKind::Caret           // ^
        | TokenKind::PlusPlus        // ++
        | TokenKind::MinusMinus      // --
        => Some(token_type_index::OPERATOR),

        // Comparison operators
        TokenKind::Gt                // >
        | TokenKind::Lt              // <
        | TokenKind::Ge              // >=
        | TokenKind::Le              // <=
        | TokenKind::Eq              // ==
        | TokenKind::Neq             // <>
        => Some(token_type_index::OPERATOR),

        // Logical operators
        TokenKind::And               // &&
        | TokenKind::Or              // ||
        | TokenKind::Not             // !
        => Some(token_type_index::OPERATOR),

        // Collection operators
        TokenKind::DollarHash        // $#
        | TokenKind::DollarPlus      // $+
        | TokenKind::DollarMinus     // $-
        | TokenKind::DollarQuestion  // $?
        | TokenKind::DollarTilde     // $~
        | TokenKind::DollarLBracket  // $[
        | TokenKind::DollarGt        // $>
        | TokenKind::DollarPipe      // $|
        | TokenKind::DollarLt        // $<
        | TokenKind::DollarPlusLBracket  // $+[
        | TokenKind::DollarMinusLBracket // $-[
        | TokenKind::DollarQuestionQuestion // $??
        | TokenKind::DollarPlusPlus  // $++
        | TokenKind::DollarMinusMinus // $--
        | TokenKind::DollarTildeTilde // $~~
        | TokenKind::DollarExclaim   // $!
        | TokenKind::DollarExclaimExclaim // $!!
        | TokenKind::DollarCaretPlus  // $^+
        | TokenKind::DollarCaretMinus // $^-
        | TokenKind::DollarCaret      // $^
        => Some(token_type_index::OPERATOR),

        // Format and base operators
        TokenKind::HashPipe          // #|
        | TokenKind::HashQuestion    // #?
        | TokenKind::HashDot         // #.
        | TokenKind::HashExclaim     // #!
        | TokenKind::HashComma       // #,
        | TokenKind::HashCaret       // #^
        | TokenKind::BaseBinary      // 0b
        | TokenKind::BaseOctal       // 0o
        | TokenKind::BaseDecimal     // 0d
        | TokenKind::BaseHex         // 0x
        | TokenKind::Pipe            // |
        => Some(token_type_index::OPERATOR),

        // Other operators
        TokenKind::Dot               // .
        | TokenKind::DotDot          // ..
        | TokenKind::Colon           // :
        | TokenKind::Comma           // ,
        | TokenKind::Semicolon       // ;
        | TokenKind::Tilde           // ~
        | TokenKind::Backslash       // \
        => Some(token_type_index::OPERATOR),

        // Boolean literals - treat as type
        TokenKind::Boolean(_) => Some(token_type_index::TYPE),

        // String literals
        TokenKind::String(_)
        | TokenKind::StringInterpolated(_)
        => Some(token_type_index::STRING),

        // Character literal
        TokenKind::Char(_) => Some(token_type_index::STRING),

        // Numeric literals
        TokenKind::Integer(_)
        | TokenKind::Float(_)
        => Some(token_type_index::NUMBER),

        // Identifiers - default to variable (context can refine later)
        TokenKind::Ident(_) => Some(token_type_index::VARIABLE),

        // Delimiters - no semantic meaning for highlighting
        TokenKind::LBrace
        | TokenKind::RBrace
        | TokenKind::LBracket
        | TokenKind::RBracket
        | TokenKind::LParen
        | TokenKind::RParen
        => None,

        // Comments
        TokenKind::LineComment(_)
        | TokenKind::BlockComment(_)
        => Some(token_type_index::COMMENT),

        // Error, EOF, and runtime-mode tokens
        TokenKind::Eof
        | TokenKind::Error(_)
        | TokenKind::SetNumeralMode(_)
        => None,
    }
}

/// Compute the length of a token in characters
fn token_length(kind: &TokenKind) -> u32 {
    match kind {
        // Single character tokens
        TokenKind::Question
        | TokenKind::Underscore
        | TokenKind::At
        | TokenKind::Assign
        | TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::Star
        | TokenKind::Slash
        | TokenKind::Percent
        | TokenKind::Caret
        | TokenKind::Gt
        | TokenKind::Lt
        | TokenKind::Not
        | TokenKind::Dot
        | TokenKind::Colon
        | TokenKind::Comma
        | TokenKind::Semicolon
        | TokenKind::Tilde
        | TokenKind::Backslash
        | TokenKind::Pipe
        | TokenKind::Hash
        | TokenKind::LBrace
        | TokenKind::RBrace
        | TokenKind::LBracket
        | TokenKind::RBracket
        | TokenKind::LParen
        | TokenKind::RParen
        | TokenKind::Newline // ¶ is one Unicode character
        => 1,

        // Two character tokens
        TokenKind::Output           // >>
        | TokenKind::Input          // <<
        | TokenKind::Arrow          // ->
        | TokenKind::Return         // <~
        | TokenKind::ScopeResolution // ::
        | TokenKind::PipeOp         // |>
        | TokenKind::ElseIf         // _?
        | TokenKind::DoubleQuestion // ??
        | TokenKind::AtBreak        // @!
        | TokenKind::AtContinue     // @>
        | TokenKind::TryBlock       // !?
        | TokenKind::CatchBlock     // :!
        | TokenKind::FinallyBlock   // :>
        | TokenKind::ExportBlock    // #>
        | TokenKind::ModuleImport   // <#
        | TokenKind::ConstAssign    // :=
        | TokenKind::PlusAssign     // +=
        | TokenKind::MinusAssign    // -=
        | TokenKind::StarAssign     // *=
        | TokenKind::SlashAssign    // /=
        | TokenKind::PercentAssign  // %=
        | TokenKind::CaretAssign    // ^=
        | TokenKind::PlusPlus       // ++
        | TokenKind::MinusMinus     // --
        | TokenKind::Ge             // >=
        | TokenKind::Le             // <=
        | TokenKind::Eq             // ==
        | TokenKind::Neq            // <>
        | TokenKind::And            // &&
        | TokenKind::Or             // ||
        | TokenKind::DotDot         // ..
        | TokenKind::Backslash2     // \\
        | TokenKind::DollarHash     // $#
        | TokenKind::DollarPlus     // $+
        | TokenKind::DollarMinus    // $-
        | TokenKind::DollarQuestion // $?
        | TokenKind::DollarTilde    // $~
        | TokenKind::DollarLBracket // $[
        | TokenKind::DollarGt       // $>
        | TokenKind::DollarPipe     // $|
        | TokenKind::DollarLt       // $<
        | TokenKind::DollarExclaim  // $!
        | TokenKind::HashPipe       // #|
        | TokenKind::HashQuestion   // #?
        | TokenKind::HashDot        // #.
        | TokenKind::HashExclaim    // #!
        | TokenKind::CliArgsCapture // ><
        | TokenKind::BaseBinary     // 0b
        | TokenKind::BaseOctal      // 0o
        | TokenKind::BaseDecimal    // 0d
        | TokenKind::BaseHex        // 0x
        | TokenKind::Boolean(true)  // #1
        | TokenKind::Boolean(false) // #0
        => 2,

        // Three character tokens
        TokenKind::DollarQuestionQuestion // $??
        | TokenKind::DollarPlusPlus       // $++
        | TokenKind::DollarMinusMinus     // $--
        | TokenKind::DollarTildeTilde     // $~~
        | TokenKind::DollarExclaimExclaim // $!!
        | TokenKind::DollarPlusLBracket   // $+[
        | TokenKind::DollarMinusLBracket  // $-[
        | TokenKind::DollarCaretPlus      // $^+
        | TokenKind::DollarCaretMinus     // $^-
        => 3,
        TokenKind::DollarCaret            // $^ (2 chars)
        => 2,

        // Variable-length tokens - use span information
        TokenKind::String(s) => (s.len() + 2) as u32, // +2 for quotes
        TokenKind::StringInterpolated(parts) => {
            // Estimate: sum of parts + braces + quotes
            let content_len: usize = parts.iter().map(|p| match p {
                zymbol_lexer::StringPart::Text(t) => t.len(),
                zymbol_lexer::StringPart::Variable(v) => v.len() + 2, // {var}
            }).sum();
            (content_len + 2) as u32
        }
        TokenKind::Char(_) => 3, // 'c'
        TokenKind::Integer(n) => format!("{}", n).len() as u32,
        TokenKind::Float(f) => format!("{}", f).len() as u32,
        TokenKind::Ident(name) => name.len() as u32,
        TokenKind::HashComma | TokenKind::HashCaret => 2, // #, or #^ (two-char tokens)
        TokenKind::BashCommand(cmd) => (cmd.len() + 4) as u32,    // +4 for <\ and \>
        TokenKind::ExecuteCommand(path) => (path.len() + 4) as u32, // +4 for </ and />
        TokenKind::LineComment(content) => (content.len() + 2) as u32, // +2 for //
        TokenKind::BlockComment(content) => (content.len() + 4) as u32, // +4 for /* */
        TokenKind::Error(msg) => msg.len() as u32,
        TokenKind::Eof => 0,
        // #<digit0><digit9># — always 4 codepoints regardless of script
        TokenKind::SetNumeralMode(_) => 4,
    }
}

/// Generate semantic tokens from a list of Zymbol tokens
pub fn generate_semantic_tokens(tokens: &[Token]) -> SemanticTokens {
    let mut semantic_tokens = Vec::new();
    let mut prev_line = 0u32;
    let mut prev_char = 0u32;

    for token in tokens {
        // Skip tokens without semantic meaning
        let token_type = match classify_token(&token.kind) {
            Some(t) => t,
            None => continue,
        };

        // Convert to 0-indexed
        let line = token.span.start.line.saturating_sub(1);
        let char = token.span.start.column.saturating_sub(1);

        // Calculate deltas
        let delta_line = line.saturating_sub(prev_line);
        let delta_start = if delta_line == 0 {
            char.saturating_sub(prev_char)
        } else {
            char
        };

        // Calculate length from span
        let length = if token.span.start.line == token.span.end.line {
            token.span.end.column.saturating_sub(token.span.start.column).max(1)
        } else {
            // Multi-line token - use estimated length
            token_length(&token.kind)
        };

        semantic_tokens.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset: 0, // No modifiers by default
        });

        prev_line = line;
        prev_char = char;
    }

    SemanticTokens {
        result_id: None,
        data: semantic_tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_lexer::Lexer;
    use zymbol_span::FileId;

    fn tokenize(source: &str) -> Vec<Token> {
        let lexer = Lexer::new(source, FileId(0));
        let (tokens, _) = lexer.tokenize();
        tokens
    }

    #[test]
    fn test_semantic_tokens_legend() {
        let legend = semantic_tokens_legend();
        assert!(!legend.token_types.is_empty());
        assert!(!legend.token_modifiers.is_empty());
    }

    #[test]
    fn test_classify_keywords() {
        assert_eq!(classify_token(&TokenKind::Question), Some(token_type_index::KEYWORD));
        assert_eq!(classify_token(&TokenKind::At), Some(token_type_index::KEYWORD));
        assert_eq!(classify_token(&TokenKind::Return), Some(token_type_index::KEYWORD));
    }

    #[test]
    fn test_classify_operators() {
        assert_eq!(classify_token(&TokenKind::Plus), Some(token_type_index::OPERATOR));
        assert_eq!(classify_token(&TokenKind::Output), Some(token_type_index::OPERATOR));
        assert_eq!(classify_token(&TokenKind::DollarHash), Some(token_type_index::OPERATOR));
    }

    #[test]
    fn test_classify_literals() {
        assert_eq!(classify_token(&TokenKind::String("test".to_string())), Some(token_type_index::STRING));
        assert_eq!(classify_token(&TokenKind::Integer(42)), Some(token_type_index::NUMBER));
        assert_eq!(classify_token(&TokenKind::Boolean(true)), Some(token_type_index::TYPE));
    }

    #[test]
    fn test_generate_semantic_tokens() {
        let tokens = tokenize("x = 5");
        let semantic = generate_semantic_tokens(&tokens);

        // Should have tokens for: x (variable), = (operator), 5 (number)
        assert_eq!(semantic.data.len(), 3);
    }

    #[test]
    fn test_delta_encoding() {
        let tokens = tokenize("a = 1\nb = 2");
        let semantic = generate_semantic_tokens(&tokens);

        // First token should have delta_line = 0
        assert_eq!(semantic.data[0].delta_line, 0);

        // Token on second line should have delta_line > 0
        let second_line_token = semantic.data.iter()
            .find(|t| t.delta_line > 0);
        assert!(second_line_token.is_some());
    }

    #[test]
    fn test_delimiters_excluded() {
        let tokens = tokenize("[1, 2]");
        let semantic = generate_semantic_tokens(&tokens);

        // Should only have tokens for: 1, comma, 2
        // Brackets should be excluded
        assert!(semantic.data.len() <= 3);
    }
}
