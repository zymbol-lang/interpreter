//! Lexical analysis for Zymbol-Lang
//!
//! Phase 0: Only tokenizes >> and string literals

mod literals;
mod io;
mod variables;
mod if_stmt;
mod match_stmt;
mod loops;
mod functions;
mod collections;
mod collection_ops;
pub mod digit_blocks;

pub use literals::StringPart;
pub use digit_blocks::{digit_value, digit_block_base, DIGIT_BLOCKS};

use zymbol_error::Diagnostic;
use zymbol_span::{FileId, Position, Span};

/// Token types
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // I/O operators
    /// >> (output operator)
    Output,
    /// << (input operator)
    Input,

    // Literals
    /// String literal (simple)
    String(String),
    /// Interpolated string literal with {var}
    StringInterpolated(Vec<StringPart>),
    /// Integer literal
    Integer(i64),
    /// Float literal
    Float(f64),
    /// Char literal
    Char(char),
    /// Boolean literal (#1 or #0, or Unicode equivalent)
    Boolean(bool),
    /// Numeral mode switch: #<digit_0><digit_9># — sets the active output
    /// numeral system. Carries the block base codepoint of the chosen script
    /// (e.g. 0x0030 for ASCII, 0x0966 for Devanagari).
    SetNumeralMode(u32),

    // Identifiers
    /// Identifier (variable name)
    Ident(String),
    /// Identifier with hot-definition marker ° — auto-initialize on first use
    HotIdent(String),

    // Operators
    /// = (assignment operator)
    Assign,
    /// := (constant declaration operator)
    ConstAssign,
    /// , (comma for concatenation)
    Comma,
    /// : (colon for for-each)
    Colon,
    /// ; (semicolon - statement separator)
    Semicolon,

    // Arithmetic operators
    /// + (addition)
    Plus,
    /// - (subtraction)
    Minus,
    /// * (multiplication)
    Star,
    /// / (division)
    Slash,
    /// % (modulo)
    Percent,
    /// ^ (power/exponentiation)
    Caret,

    // Collection operators
    /// $# (length/size)
    DollarHash,
    /// $+ (append element)
    DollarPlus,
    /// $- (remove by index)
    DollarMinus,
    /// $? (contains/search)
    DollarQuestion,
    /// $~ (update element)
    DollarTilde,
    /// $[ (slice start)
    DollarLBracket,
    /// $> (map - transform collection)
    DollarGt,
    /// $| (filter - select elements)
    DollarPipe,
    /// $< (reduce - accumulate)
    DollarLt,

    // Positional collection operators (v0.0.2)
    /// $+[ (insert element at position — arrays, tuples, strings)
    DollarPlusLBracket,
    /// $-[ (remove element at position or range — arrays, tuples, strings)
    DollarMinusLBracket,

    // Sort operators (v0.0.2)
    /// $^+ (sort ascending — natural order, no comparator)
    DollarCaretPlus,
    /// $^- (sort descending — natural order, no comparator)
    DollarCaretMinus,
    /// $^ (sort with custom comparator lambda)
    DollarCaret,

    // String operators
    /// $?? (find all indices where pattern occurs — arrays, tuples, strings)
    DollarQuestionQuestion,
    /// $++ (insert text at position) — RETIRED in v0.0.2, use $+[ instead
    DollarPlusPlus,
    /// $-- (remove all occurrences of value — arrays, tuples, strings)
    DollarMinusMinus,
    /// $~~ (replace pattern with text, with optional count)
    DollarTildeTilde,
    /// $/ (split string by delimiter)
    DollarSlash,

    // Error handling operators
    /// $! (is_error - check if value is an error)
    DollarExclaim,
    /// $!! (error propagate - rethrow error to caller)
    DollarExclaimExclaim,

    // Compound assignment operators
    /// += (add and assign)
    PlusAssign,
    /// -= (subtract and assign)
    MinusAssign,
    /// *= (multiply and assign)
    StarAssign,
    /// /= (divide and assign)
    SlashAssign,
    /// %= (modulo and assign)
    PercentAssign,
    /// ^= (power and assign)
    CaretAssign,

    // Increment/Decrement operators
    /// ++ (increment)
    PlusPlus,
    /// -- (decrement)
    MinusMinus,

    // Comparison operators
    /// > (greater than)
    Gt,
    /// < (less than)
    Lt,
    /// >= (greater than or equal)
    Ge,
    /// <= (less than or equal)
    Le,
    /// == (equal)
    Eq,
    /// <> (not equal)
    Neq,

    // Logical operators
    /// && (logical AND)
    And,
    /// || (logical OR)
    Or,
    /// ! (logical NOT)
    Not,

    // Control flow
    /// ? (if)
    Question,
    /// ?? (match)
    DoubleQuestion,
    /// _ (underscore - else/wildcard)
    Underscore,
    /// _? (else-if)
    ElseIf,

    // Error handling control flow
    /// !? (try block - error query)
    TryBlock,
    /// :! (catch block - error else)
    CatchBlock,
    /// :> (finally block - flow continues)
    FinallyBlock,

    // Loop operators
    /// @ (universal loop)
    At,
    /// @! (break)
    AtBreak,
    /// @> (continue)
    AtContinue,
    /// @label (labeled loop declaration, legacy — fused without colon)
    AtLabel(String),
    /// @:label (labeled loop declaration: @:outer i:1..5 { })
    AtColonLabel(String),
    /// @:label! (labeled break: @:outer!)
    AtColonLabelBreak(String),
    /// @:label> (labeled continue: @:outer>)
    AtColonLabelContinue(String),

    // Member access and range operators
    /// . (member access)
    Dot,
    /// .. (range)
    DotDot,

    // Newline operators
    /// ¶ (explicit newline - pilcrow)
    Newline,
    /// \\ (explicit newline - double backslash alternative)
    Backslash2,
    /// \ (single backslash - for lifetime end)
    Backslash,

    // Format and Base prefixes
    /// #, (thousands separator format prefix)
    HashComma,
    /// #^ (scientific notation format prefix)
    HashCaret,
    /// 0b (binary base prefix for char literals)
    BaseBinary,
    /// 0o (octal base prefix for char literals)
    BaseOctal,
    /// 0d (decimal base prefix for char literals)
    BaseDecimal,
    /// 0x (hexadecimal/Unicode base prefix for char literals)
    BaseHex,
    /// | (pipe for format/base expressions)
    Pipe,
    /// |> (pipe operator for function composition)
    PipeOp,
    /// #| (numeric evaluation - safe string to number conversion)
    HashPipe,
    /// #? (type metadata - returns tuple with type, count, value)
    HashQuestion,
    /// #. (round prefix - for precision rounding: #.2|expr|)
    HashDot,
    /// #! (trunc prefix - for precision truncation: #!2|expr|)
    HashExclaim,
    /// ##. (cast to Float: ##.expr)
    HashHashDot,
    /// ### (cast to Int rounding: ###expr)
    HashHashHash,
    /// ##! (cast to Int truncating: ##!expr)
    HashHashBang,

    // Delimiters
    /// { (left brace)
    LBrace,
    /// } (right brace)
    RBrace,
    /// [ (left bracket)
    LBracket,
    /// ] (right bracket)
    RBracket,
    /// ( (left parenthesis)
    LParen,
    /// ) (right parenthesis)
    RParen,

    // Function-related operators
    /// <~ (return statement / output parameter)
    Return,
    /// ~ (mutable parameter modifier)
    Tilde,
    /// -> (arrow - lambda expression)
    Arrow,

    // Module system operators
    /// # (module declaration)
    Hash,
    /// #> (export block)
    ExportBlock,
    /// <# (module import)
    ModuleImport,
    /// :: (scope resolution for module function calls)
    ScopeResolution,

    // Script execution operators
    /// </ path /> (execute — raw path string)
    ExecuteCommand(String),
    /// >< (CLI args capture)
    CliArgsCapture,
    /// <\ (bash execute open — content is normal Zymbol tokens until \>)
    BashOpen,
    /// \> (bash execute close)
    BashClose,

    // Comments (for formatter preservation)
    /// Single-line comment // ...
    LineComment(String),
    /// Multi-line comment /* ... */
    BlockComment(String),

    /// End of file
    Eof,

    /// Error token
    Error(String),
}

/// A token with its kind and source location
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Lexer for Zymbol source code
pub struct Lexer {
    source: Vec<char>,
    current: usize,
    line: u32,
    column: u32,
    file_id: FileId,
    diagnostics: Vec<Diagnostic>,
    bash_depth: u32,
}

impl Lexer {
    pub fn new(source: &str, file_id: FileId) -> Self {
        Self {
            source: source.chars().collect(),
            current: 0,
            line: 1,
            column: 1,
            file_id,
            diagnostics: Vec::new(),
            bash_depth: 0,
        }
    }

    /// Tokenize the entire source
    pub fn tokenize(mut self) -> (Vec<Token>, Vec<Diagnostic>) {
        let mut tokens = Vec::new();

        loop {
            let token = self.next_token();
            let is_eof = matches!(token.kind, TokenKind::Eof);
            tokens.push(token);

            if is_eof {
                break;
            }
        }

        (tokens, self.diagnostics)
    }

    /// Check if a character can start an identifier
    /// Allows: Unicode letters, underscore, and any non-operator Unicode character
    /// Excludes: digits (can't start with digit), whitespace, and known operators
    fn is_ident_start(ch: char) -> bool {
        // Allow underscore
        if ch == '_' {
            return true;
        }

        // Allow any Unicode letter (covers all languages: English, Chinese, Arabic, Hindi, etc.)
        if ch.is_alphabetic() {
            return true;
        }

        // For emojis and other Unicode symbols: allow if not whitespace, digit, or operator.
        // digit_value covers all supported numeral systems, not just ASCII.
        !ch.is_whitespace()
            && digit_blocks::digit_value(ch).is_none()
            && !Self::is_operator_char(ch)
    }

    /// Check if a character can continue an identifier
    /// More permissive: allows letters, digits, underscore, and Unicode symbols
    fn is_ident_continue(ch: char) -> bool {
        // Allow alphanumeric and underscore
        if ch.is_alphanumeric() || ch == '_' {
            return true;
        }

        // Allow any non-whitespace, non-operator Unicode character
        !ch.is_whitespace() && !Self::is_operator_char(ch)
    }

    /// Check if a character is a Zymbol operator
    /// These characters cannot be part of identifiers
    fn is_operator_char(ch: char) -> bool {
        matches!(ch,
            '>' | '<' | '=' | '!' | '+' | '-' | '*' | '/' | '%' | '^' |
            '&' | '|' | '?' | ':' | '.' | ',' | ';' |
            '(' | ')' | '[' | ']' | '{' | '}' |
            '@' | '~' | '#' | '$' | '¶' | '\\'
        )
    }

    /// Get the next token
    fn next_token(&mut self) -> Token {
        // Skip whitespace only (not comments)
        self.skip_whitespace_only();

        let start = self.position();

        // Check for comments and emit them as tokens
        if let Some(comment_token) = self.try_parse_comment(start) {
            return comment_token;
        }

        let start = self.position();

        if self.is_at_end() {
            return Token::new(TokenKind::Eof, self.span(start));
        }

        let ch = self.current_char();

        // Try parsing IO tokens first (>>, <<, ><, \\, ¶)
        if let Some(token) = self.try_parse_io_token(ch, start) {
            return token;
        }

        // Try parsing variable/assignment tokens (=, :=, +=, -=, *=, /=, %=, ++, --)
        if let Some(token) = self.try_parse_variable_token(ch, start) {
            return token;
        }

        // Try parsing IF-related tokens (?, _?, _)
        if let Some(token) = self.try_parse_if_token(ch, start) {
            return token;
        }

        // Try parsing MATCH-related tokens (??)
        if let Some(token) = self.try_parse_match_token(ch, start) {
            return token;
        }

        // Try parsing loop-related tokens (@, @!, @>)
        if let Some(token) = self.try_parse_loop_token(ch, start) {
            return token;
        }

        // Try parsing function-related tokens (->, <~)
        if let Some(token) = self.try_parse_function_token(ch, start) {
            return token;
        }

        // Try parsing collection-related tokens ([, ], (, ), ,)
        if let Some(token) = self.try_parse_collection_token(ch, start) {
            return token;
        }

        // Check for >= (greater or equal)
        if ch == '>' && self.peek() == Some('=') {
            self.advance();
            self.advance();
            return Token::new(TokenKind::Ge, self.span(start));
        }

        // Check for > (greater than)
        if ch == '>' {
            self.advance();
            return Token::new(TokenKind::Gt, self.span(start));
        }

        // Check for <# (module import)
        if ch == '<' && self.peek() == Some('#') {
            self.advance();
            self.advance();
            return Token::new(TokenKind::ModuleImport, self.span(start));
        }

        // Check for <= (less or equal)
        if ch == '<' && self.peek() == Some('=') {
            self.advance();
            self.advance();
            return Token::new(TokenKind::Le, self.span(start));
        }

        // Check for <> (not equal)
        if ch == '<' && self.peek() == Some('>') {
            self.advance();
            self.advance();
            return Token::new(TokenKind::Neq, self.span(start));
        }

        // Check for </ (execute) — raw-string mode: read path literally until />
        if ch == '<' && self.peek() == Some('/') {
            self.advance(); // consume <
            self.advance(); // consume /
            let mut raw = String::new();
            loop {
                if self.is_at_end() {
                    self.diagnostics.push(
                        Diagnostic::error("unterminated execute expression")
                            .with_span(self.span(start))
                            .with_help("execute syntax: </ path.zy />"),
                    );
                    break;
                }
                let c = self.current_char();
                if c == '/' && self.peek() == Some('>') {
                    self.advance(); // consume /
                    self.advance(); // consume >
                    break;
                }
                raw.push(c);
                self.advance();
            }
            return Token::new(TokenKind::ExecuteCommand(raw.trim().to_string()), self.span(start));
        }

        // Check for <\ (bash execute open) — normal tokenization until \>
        if ch == '<' && self.peek() == Some('\\') {
            self.advance(); // consume <
            self.advance(); // consume \
            self.bash_depth += 1;
            return Token::new(TokenKind::BashOpen, self.span(start));
        }

        // Check for < (less than)
        if ch == '<' {
            self.advance();
            return Token::new(TokenKind::Lt, self.span(start));
        }

        // Check for :: (scope resolution) or : (colon)
        if ch == ':' {
            if self.peek() == Some(':') {
                self.advance();
                self.advance();
                return Token::new(TokenKind::ScopeResolution, self.span(start));
            } else {
                self.advance();
                return Token::new(TokenKind::Colon, self.span(start));
            }
        }

        // Check for ; (semicolon)
        if ch == ';' {
            self.advance();
            return Token::new(TokenKind::Semicolon, self.span(start));
        }

        // Check for + (addition)
        if ch == '+' {
            self.advance();
            return Token::new(TokenKind::Plus, self.span(start));
        }

        // Check for - (subtraction)
        if ch == '-' {
            self.advance();
            return Token::new(TokenKind::Minus, self.span(start));
        }

        // Check for * (multiplication)
        if ch == '*' {
            self.advance();
            return Token::new(TokenKind::Star, self.span(start));
        }

        // Check for / operator (division) - comments are handled by try_parse_comment
        if ch == '/' {
            self.advance();
            return Token::new(TokenKind::Slash, self.span(start));
        }

        // Check for % (modulo)
        if ch == '%' {
            self.advance();
            return Token::new(TokenKind::Percent, self.span(start));
        }

        // Check for ^ operators (^=, ^)
        if ch == '^' {
            if self.peek() == Some('=') {
                self.advance(); // consume ^
                self.advance(); // consume =
                return Token::new(TokenKind::CaretAssign, self.span(start));
            }
            self.advance();
            return Token::new(TokenKind::Caret, self.span(start));
        }

        // Check for && (logical AND)
        if ch == '&' && self.peek() == Some('&') {
            self.advance();
            self.advance();
            return Token::new(TokenKind::And, self.span(start));
        }

        // Check for || (logical OR)
        if ch == '|' && self.peek() == Some('|') {
            self.advance();
            self.advance();
            return Token::new(TokenKind::Or, self.span(start));
        }

        // Check for !? (try block), != (invalid — guide to <>), or ! (logical NOT)
        if ch == '!' {
            if self.peek() == Some('?') {
                self.advance(); // consume !
                self.advance(); // consume ?
                return Token::new(TokenKind::TryBlock, self.span(start));
            }
            if self.peek() == Some('=') {
                self.advance(); // consume !
                self.advance(); // consume =
                let span = self.span(start);
                self.diagnostics.push(
                    Diagnostic::error("'!=' is not a valid Zymbol operator")
                        .with_span(span)
                        .with_help("use '<>' for not-equal  →  a <> b"),
                );
                return Token::new(TokenKind::Error("invalid operator '!='".to_string()), span);
            }
            self.advance();
            return Token::new(TokenKind::Not, self.span(start));
        }

        // Check for .. (range) or . (member access)
        if ch == '.' {
            if self.peek() == Some('.') {
                self.advance();
                self.advance();
                return Token::new(TokenKind::DotDot, self.span(start));
            } else {
                self.advance();
                return Token::new(TokenKind::Dot, self.span(start));
            }
        }

        // Check for { (left brace)
        if ch == '{' {
            self.advance();
            return Token::new(TokenKind::LBrace, self.span(start));
        }

        // Check for } (right brace)
        if ch == '}' {
            self.advance();
            return Token::new(TokenKind::RBrace, self.span(start));
        }

        // Check for ~ (tilde - mutable parameter)
        if ch == '~' {
            self.advance();
            return Token::new(TokenKind::Tilde, self.span(start));
        }

        // Check for $ (collection operators) - MUST come before # check
        if ch == '$' {
            if let Some(token) = self.try_parse_collection_op(start) {
                return token;
            }
        }

        // Check for # (boolean, numeric eval, type metadata, precision ops, export block, or module declaration)
        if ch == '#' {
            if let Some(next) = self.peek() {
                // Check for #> (export block)
                if next == '>' {
                    self.advance(); // consume #
                    self.advance(); // consume >
                    return Token::new(TokenKind::ExportBlock, self.span(start));
                }
                // Check for #| (numeric evaluation)
                else if next == '|' {
                    self.advance(); // consume #
                    self.advance(); // consume |
                    return Token::new(TokenKind::HashPipe, self.span(start));
                }
                // Check for #? (type metadata)
                else if next == '?' {
                    self.advance(); // consume #
                    self.advance(); // consume ?
                    return Token::new(TokenKind::HashQuestion, self.span(start));
                }
                // Check for ##. / ### / ##! (numeric cast operators)
                else if next == '#' {
                    let third = self.peek_ahead(2);
                    if third == Some('.') {
                        self.advance(); // consume first #
                        self.advance(); // consume second #
                        self.advance(); // consume .
                        return Token::new(TokenKind::HashHashDot, self.span(start));
                    } else if third == Some('#') {
                        self.advance(); // consume first #
                        self.advance(); // consume second #
                        self.advance(); // consume third #
                        return Token::new(TokenKind::HashHashHash, self.span(start));
                    } else if third == Some('!') {
                        self.advance(); // consume first #
                        self.advance(); // consume second #
                        self.advance(); // consume !
                        return Token::new(TokenKind::HashHashBang, self.span(start));
                    }
                    // unrecognized ##X — fall through to emit lone Hash
                }
                // Check for #. (round prefix for precision)
                else if next == '.' {
                    self.advance(); // consume #
                    self.advance(); // consume .
                    return Token::new(TokenKind::HashDot, self.span(start));
                }
                // Check for #! (trunc prefix for precision)
                else if next == '!' {
                    self.advance(); // consume #
                    self.advance(); // consume !
                    return Token::new(TokenKind::HashExclaim, self.span(start));
                }
                // Check for #, (thousands format)
                else if next == ',' {
                    self.advance(); // consume #
                    self.advance(); // consume ,
                    return Token::new(TokenKind::HashComma, self.span(start));
                }
                // Check for #^ (scientific notation format)
                else if next == '^' {
                    self.advance(); // consume #
                    self.advance(); // consume ^
                    return Token::new(TokenKind::HashCaret, self.span(start));
                }
                // Check for digits after #.
                //
                // Two cases share the same first character (a digit with value 0):
                //
                //   1. Numeral mode switch  #<d0><d9>#
                //      d0 has digit_value 0, d9 has digit_value 9, same block, then '#'.
                //      Example: #09# (ASCII reset), #०९# (Devanagari).
                //
                //   2. Boolean false  #<d0>
                //      Any other context where digit_value of next == 0.
                //
                // We resolve the ambiguity with a 3-char lookahead before consuming
                // any characters.
                else if let Some(dv) = digit_blocks::digit_value(next) {
                    // ── mode-switch check (only when first digit has value 0) ──
                    if dv == 0 {
                        let maybe_d9   = self.peek_ahead(2); // char after next
                        let maybe_hash = self.peek_ahead(3); // char after that
                        if let (Some(d9_char), Some('#')) = (maybe_d9, maybe_hash) {
                            if digit_blocks::digit_value(d9_char) == Some(9)
                                && digit_blocks::digit_block_base(next)
                                    == digit_blocks::digit_block_base(d9_char)
                            {
                                let block_base =
                                    digit_blocks::digit_block_base(next).unwrap();
                                self.advance(); // consume #
                                self.advance(); // consume digit0
                                self.advance(); // consume digit9
                                self.advance(); // consume closing #
                                return Token::new(
                                    TokenKind::SetNumeralMode(block_base),
                                    self.span(start),
                                );
                            }
                        }
                    }

                    // ── boolean ───────────────────────────────────────────────
                    self.advance(); // consume #
                    self.advance(); // consume digit
                    match dv {
                        0 => return Token::new(TokenKind::Boolean(false), self.span(start)),
                        1 => return Token::new(TokenKind::Boolean(true), self.span(start)),
                        _ => {
                            let span = self.span(start);
                            self.diagnostics.push(
                                Diagnostic::error(format!(
                                    "invalid boolean literal: digit {} is not valid after '#'",
                                    dv
                                ))
                                .with_span(span)
                                .with_help("use '#0' (or its Unicode equivalent) for false, '#1' for true"),
                            );
                            return Token::new(
                                TokenKind::Error("invalid boolean literal".to_string()),
                                span,
                            );
                        }
                    }
                }
            }
            // Standalone # (module declaration)
            self.advance();
            return Token::new(TokenKind::Hash, self.span(start));
        }

        // Check for |> (pipe operator)
        if ch == '|' && self.peek() == Some('>') {
            self.advance();
            self.advance();
            return Token::new(TokenKind::PipeOp, self.span(start));
        }

        // Check for | (pipe for format/base expressions)
        if ch == '|' {
            self.advance();
            return Token::new(TokenKind::Pipe, self.span(start));
        }

        // Check for string literal
        if ch == '"' {
            return self.lex_string(start);
        }

        // Check for char literal
        if ch == '\'' {
            return self.lex_char(start);
        }

        // Check for number — any digit from any supported numeral system
        if digit_blocks::digit_value(ch).is_some() {
            return self.lex_number(start);
        }

        // Check for identifier (letters, Unicode, or emojis)
        if Self::is_ident_start(ch) {
            return self.lex_identifier(start);
        }

        // Unknown character
        self.advance();
        let span = self.span(start);
        self.diagnostics.push(
            Diagnostic::error(format!("unexpected character: '{}'", ch))
                .with_span(span),
        );
        Token::new(TokenKind::Error(format!("unexpected character: '{}'", ch)), span)
    }



    /// Lex an identifier
    fn lex_identifier(&mut self, start: Position) -> Token {
        let mut ident = String::new();

        while !self.is_at_end() {
            let ch = self.current_char();
            if Self::is_ident_continue(ch) {
                ident.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        // Strip ° suffix → emit HotIdent (auto-initialize on first use)
        if ident.ends_with('°') {
            let stripped = &ident[..ident.len() - '°'.len_utf8()];
            if !stripped.is_empty() {
                return Token::new(TokenKind::HotIdent(stripped.to_string()), self.span(start));
            }
        }

        Token::new(TokenKind::Ident(ident), self.span(start))
    }



    /// Skip whitespace only (not comments - those are captured as tokens)
    fn skip_whitespace_only(&mut self) {
        while !self.is_at_end() {
            let ch = self.current_char();
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Try to parse a comment token
    fn try_parse_comment(&mut self, start: Position) -> Option<Token> {
        if self.is_at_end() {
            return None;
        }

        let ch = self.current_char();

        // Single-line comment //
        if ch == '/' && self.peek() == Some('/') {
            self.advance(); // consume first /
            self.advance(); // consume second /

            let mut content = String::new();
            while !self.is_at_end() && self.current_char() != '\n' {
                content.push(self.current_char());
                self.advance();
            }

            return Some(Token::new(TokenKind::LineComment(content), self.span(start)));
        }

        // Multi-line comment /*
        if ch == '/' && self.peek() == Some('*') {
            self.advance(); // consume /
            self.advance(); // consume *

            let mut content = String::new();
            let mut depth = 1;

            while !self.is_at_end() && depth > 0 {
                let c = self.current_char();

                // Check for nested opening /*
                if c == '/' && self.peek() == Some('*') {
                    depth += 1;
                    content.push(c);
                    self.advance();
                    content.push(self.current_char());
                    self.advance();
                }
                // Check for closing */
                else if c == '*' && self.peek() == Some('/') {
                    depth -= 1;
                    if depth > 0 {
                        content.push(c);
                        self.advance();
                        content.push(self.current_char());
                    }
                    self.advance();
                    self.advance();
                }
                // Regular character
                else {
                    content.push(c);
                    self.advance();
                }
            }

            if depth > 0 {
                // Unterminated comment
                self.diagnostics.push(
                    Diagnostic::error("Unterminated multi-line comment")
                        .with_span(self.span(start))
                        .with_help("add */ to close the comment"),
                );
            }

            return Some(Token::new(TokenKind::BlockComment(content), self.span(start)));
        }

        None
    }

    /// Get current character
    fn current_char(&self) -> char {
        self.source[self.current]
    }

    /// Peek at next character
    fn peek(&self) -> Option<char> {
        if self.current + 1 < self.source.len() {
            Some(self.source[self.current + 1])
        } else {
            None
        }
    }

    /// Peek ahead by offset characters
    fn peek_ahead(&self, offset: usize) -> Option<char> {
        if self.current + offset < self.source.len() {
            Some(self.source[self.current + offset])
        } else {
            None
        }
    }

    /// Advance to next character
    fn advance(&mut self) {
        if !self.is_at_end() {
            if self.current_char() == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            self.current += 1;
        }
    }

    /// Check if at end of source
    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    /// Get current position
    fn position(&self) -> Position {
        Position::new(self.line, self.column, self.current as u32)
    }

    /// Create a span from start to current position
    fn span(&self, start: Position) -> Span {
        Span::new(start, self.position(), self.file_id)
    }
}

#[cfg(test)]
#[allow(clippy::approx_constant)]
mod tests {
    use super::*;

    fn lex(source: &str) -> Vec<TokenKind> {
        let lexer = Lexer::new(source, FileId(0));
        let (tokens, _) = lexer.tokenize();
        tokens
            .into_iter()
            .map(|t| t.kind)
            .filter(|k| !matches!(k, TokenKind::LineComment(_) | TokenKind::BlockComment(_)))
            .collect()
    }

    #[test]
    fn test_output_operator() {
        let tokens = lex(">>");
        assert_eq!(tokens.len(), 2); // Output + Eof
        assert!(matches!(tokens[0], TokenKind::Output));
    }

    #[test]
    fn test_string_literal() {
        let tokens = lex("\"hello\"");
        assert_eq!(tokens.len(), 2); // String + Eof
        match &tokens[0] {
            TokenKind::String(s) => assert_eq!(s, "hello"),
            _ => panic!("Expected string token"),
        }
    }

    #[test]
    fn test_output_with_string() {
        let tokens = lex(">> \"Hello, World!\"");
        assert_eq!(tokens.len(), 3); // Output + String + Eof
        assert!(matches!(tokens[0], TokenKind::Output));
        match &tokens[1] {
            TokenKind::String(s) => assert_eq!(s, "Hello, World!"),
            _ => panic!("Expected string token"),
        }
    }

    #[test]
    fn test_multiple_statements() {
        let tokens = lex(">> \"Line 1\"\n>> \"Line 2\"");
        assert_eq!(tokens.len(), 5); // Output + String + Output + String + Eof
    }

    #[test]
    fn test_string_escapes() {
        let tokens = lex(r#""hello\nworld""#);
        match &tokens[0] {
            TokenKind::String(s) => assert_eq!(s, "hello\nworld"),
            _ => panic!("Expected string token"),
        }
    }

    #[test]
    fn test_comments() {
        let tokens = lex(">> \"test\" // comment\n>> \"test2\"");
        assert_eq!(tokens.len(), 5); // Output + String + Output + String + Eof
    }

    #[test]
    fn test_identifier() {
        let tokens = lex("mensaje");
        assert_eq!(tokens.len(), 2); // Ident + Eof
        match &tokens[0] {
            TokenKind::Ident(s) => assert_eq!(s, "mensaje"),
            _ => panic!("Expected identifier token"),
        }
    }

    #[test]
    fn test_assignment() {
        let tokens = lex("x = \"hello\"");
        assert_eq!(tokens.len(), 4); // Ident + Assign + String + Eof
        assert!(matches!(tokens[0], TokenKind::Ident(_)));
        assert!(matches!(tokens[1], TokenKind::Assign));
        assert!(matches!(tokens[2], TokenKind::String(_)));
    }

    #[test]
    fn test_unicode_identifier() {
        let tokens = lex("año");
        match &tokens[0] {
            TokenKind::Ident(s) => assert_eq!(s, "año"),
            _ => panic!("Expected identifier"),
        }
    }

    #[test]
    fn test_comma() {
        let tokens = lex(">> \"Hello\", \" \", \"World\"");
        // Output + String + Comma + String + Comma + String + Eof = 7 tokens
        assert_eq!(tokens.len(), 7);
        assert!(matches!(tokens[0], TokenKind::Output));
        assert!(matches!(tokens[1], TokenKind::String(_)));
        assert!(matches!(tokens[2], TokenKind::Comma));
        assert!(matches!(tokens[3], TokenKind::String(_)));
        assert!(matches!(tokens[4], TokenKind::Comma));
        assert!(matches!(tokens[5], TokenKind::String(_)));
    }

    #[test]
    fn test_input_operator() {
        let tokens = lex("<<");
        assert_eq!(tokens.len(), 2); // Input + Eof
        assert!(matches!(tokens[0], TokenKind::Input));
    }

    #[test]
    fn test_booleans() {
        let tokens = lex("#1 #0");
        assert_eq!(tokens.len(), 3); // Bool(true) + Bool(false) + Eof
        match &tokens[0] {
            TokenKind::Boolean(b) => assert!(*b),
            _ => panic!("Expected boolean true"),
        }
        match &tokens[1] {
            TokenKind::Boolean(b) => assert!(!(*b)),
            _ => panic!("Expected boolean false"),
        }
    }

    // ── Boolean Unicode literals ──────────────────────────────────────────────

    fn lex_bool_token(src: &str) -> Option<bool> {
        use zymbol_span::FileId;
        let (tokens, _) = Lexer::new(src, FileId(0)).tokenize();
        match tokens.first() {
            Some(t) => match &t.kind {
                TokenKind::Boolean(b) => Some(*b),
                _ => None,
            },
            None => None,
        }
    }

    fn lex_has_error(src: &str) -> bool {
        use zymbol_span::FileId;
        let (_, diags) = Lexer::new(src, FileId(0)).tokenize();
        !diags.is_empty()
    }

    #[test]
    fn boolean_devanagari_false() {
        assert_eq!(lex_bool_token("#०"), Some(false)); // U+0966
    }

    #[test]
    fn boolean_devanagari_true() {
        assert_eq!(lex_bool_token("#१"), Some(true)); // U+0967
    }

    #[test]
    fn boolean_arabic_indic_false() {
        assert_eq!(lex_bool_token("#٠"), Some(false)); // U+0660
    }

    #[test]
    fn boolean_arabic_indic_true() {
        assert_eq!(lex_bool_token("#١"), Some(true)); // U+0661
    }

    #[test]
    fn boolean_thai_false() {
        assert_eq!(lex_bool_token("#๐"), Some(false)); // U+0E50
    }

    #[test]
    fn boolean_thai_true() {
        assert_eq!(lex_bool_token("#๑"), Some(true)); // U+0E51
    }

    #[test]
    fn boolean_adlam_false() {
        let zero = char::from_u32(0x1E950).unwrap();
        let src = format!("#{}", zero);
        assert_eq!(lex_bool_token(&src), Some(false));
    }

    #[test]
    fn boolean_adlam_true() {
        let one = char::from_u32(0x1E951).unwrap();
        let src = format!("#{}", one);
        assert_eq!(lex_bool_token(&src), Some(true));
    }

    #[test]
    fn boolean_klingon_piqad_false() {
        // U+F8F0 — CSUR PUA klingon zero digit
        let zero = char::from_u32(0xF8F0).unwrap();
        let src = format!("#{}", zero);
        assert_eq!(lex_bool_token(&src), Some(false));
    }

    #[test]
    fn boolean_klingon_piqad_true() {
        // U+F8F1 — CSUR PUA klingon one digit
        let one = char::from_u32(0xF8F1).unwrap();
        let src = format!("#{}", one);
        assert_eq!(lex_bool_token(&src), Some(true));
    }

    #[test]
    fn boolean_digit_2_to_9_is_error() {
        // ASCII digits 2-9 after # must be a lex error
        for d in '2'..='9' {
            let src = format!("#{}", d);
            assert!(lex_has_error(&src), "expected error for '{}'", src);
        }
    }

    #[test]
    fn boolean_devanagari_digit_2_to_9_is_error() {
        // Devanagari digits 2-9 (U+0968-U+096F) after # must also be an error
        for offset in 2u32..=9 {
            let d = char::from_u32(0x0966 + offset).unwrap();
            let src = format!("#{}", d);
            assert!(lex_has_error(&src), "expected error for '#{}'", d);
        }
    }

    // ── SetNumeralMode token ──────────────────────────────────────────────────

    fn lex_mode(src: &str) -> Option<u32> {
        use zymbol_span::FileId;
        let (tokens, diags) = Lexer::new(src, FileId(0)).tokenize();
        assert!(diags.is_empty(), "unexpected lex errors: {:?}", diags);
        match tokens.first() {
            Some(t) => match t.kind {
                TokenKind::SetNumeralMode(base) => Some(base),
                _ => None,
            },
            None => None,
        }
    }

    #[test]
    fn mode_switch_ascii_reset() {
        assert_eq!(lex_mode("#09#"), Some(0x0030));
    }

    #[test]
    fn mode_switch_devanagari() {
        assert_eq!(lex_mode("#०९#"), Some(0x0966));
    }

    #[test]
    fn mode_switch_thai() {
        assert_eq!(lex_mode("#๐๙#"), Some(0x0E50));
    }

    #[test]
    fn mode_switch_tibetan() {
        assert_eq!(lex_mode("#༠༩#"), Some(0x0F20));
    }

    #[test]
    fn mode_switch_adlam() {
        let zero  = char::from_u32(0x1E950).unwrap();
        let nine  = char::from_u32(0x1E959).unwrap();
        let src   = format!("#{}{}#", zero, nine);
        assert_eq!(lex_mode(&src), Some(0x1E950));
    }

    #[test]
    fn mode_switch_segmented_lcd() {
        assert_eq!(lex_mode("#🯰🯹#"), Some(0x1FBF0));
    }

    // ── Disambiguation: #0 alone must remain Boolean(false) ──────────────────

    #[test]
    fn hash_zero_alone_is_boolean_false() {
        // #0 with nothing after → Boolean(false), NOT a mode switch
        assert_eq!(lex_bool_token("#0"), Some(false));
    }

    #[test]
    fn hash_zero_followed_by_space_is_boolean_false() {
        // #0 followed by space — the 4-char pattern #09# is not complete
        assert_eq!(lex_bool_token("#0 "), Some(false));
    }

    #[test]
    fn hash_zero_nine_without_closing_hash_is_boolean_false() {
        // #09 without trailing # → Boolean(false) then Integer(9)
        use zymbol_span::FileId;
        let (tokens, _) = Lexer::new("#09", FileId(0)).tokenize();
        assert!(matches!(tokens[0].kind, TokenKind::Boolean(false)));
        assert!(matches!(tokens[1].kind, TokenKind::Integer(9)));
    }

    #[test]
    fn mixed_script_mode_switch_is_not_recognised() {
        // #0९# — ASCII '0' and Devanagari '९' → different blocks → NOT a mode switch
        // Resolves as Boolean(false) then the remaining chars as tokens
        let devanagari_nine = char::from_u32(0x0969).unwrap(); // ९
        let src = format!("#0{}#", devanagari_nine);
        use zymbol_span::FileId;
        let (tokens, _) = Lexer::new(&src, FileId(0)).tokenize();
        // First token must be Boolean(false), not SetNumeralMode
        assert!(matches!(tokens[0].kind, TokenKind::Boolean(false)));
    }

    #[test]
    fn mode_switch_produces_single_token() {
        // #09# should be exactly one SetNumeralMode token + Eof
        use zymbol_span::FileId;
        let (tokens, diags) = Lexer::new("#09#", FileId(0)).tokenize();
        assert!(diags.is_empty());
        assert_eq!(tokens.len(), 2); // SetNumeralMode + Eof
        assert!(matches!(tokens[0].kind, TokenKind::SetNumeralMode(0x0030)));
    }

    #[test]
    fn test_integers() {
        let tokens = lex("42 100 0");
        assert_eq!(tokens.len(), 4); // Int + Int + Int + Eof
        match &tokens[0] {
            TokenKind::Integer(n) => assert_eq!(*n, 42),
            _ => panic!("Expected integer"),
        }
        match &tokens[1] {
            TokenKind::Integer(n) => assert_eq!(*n, 100),
            _ => panic!("Expected integer"),
        }
    }

    #[test]
    fn test_comparison_operators() {
        let tokens = lex("> < >= <= == <>");
        assert_eq!(tokens.len(), 7); // 6 operators + Eof
        assert!(matches!(tokens[0], TokenKind::Gt));
        assert!(matches!(tokens[1], TokenKind::Lt));
        assert!(matches!(tokens[2], TokenKind::Ge));
        assert!(matches!(tokens[3], TokenKind::Le));
        assert!(matches!(tokens[4], TokenKind::Eq));
        assert!(matches!(tokens[5], TokenKind::Neq));
    }

    #[test]
    fn test_if_tokens() {
        let tokens = lex("? { }");
        assert_eq!(tokens.len(), 4); // Question + LBrace + RBrace + Eof
        assert!(matches!(tokens[0], TokenKind::Question));
        assert!(matches!(tokens[1], TokenKind::LBrace));
        assert!(matches!(tokens[2], TokenKind::RBrace));
    }

    #[test]
    fn test_underscore() {
        let tokens = lex("_");
        assert_eq!(tokens.len(), 2); // Underscore + Eof
        assert!(matches!(tokens[0], TokenKind::Underscore));
    }

    #[test]
    fn test_underscore_identifier() {
        let tokens = lex("_variable");
        assert_eq!(tokens.len(), 2); // Ident + Eof
        match &tokens[0] {
            TokenKind::Ident(s) => assert_eq!(s, "_variable"),
            _ => panic!("Expected identifier"),
        }
    }

    #[test]
    fn test_arithmetic_operators() {
        let tokens = lex("+ - * / %");
        assert_eq!(tokens.len(), 6); // 5 operators + Eof
        assert!(matches!(tokens[0], TokenKind::Plus));
        assert!(matches!(tokens[1], TokenKind::Minus));
        assert!(matches!(tokens[2], TokenKind::Star));
        assert!(matches!(tokens[3], TokenKind::Slash));
        assert!(matches!(tokens[4], TokenKind::Percent));
    }

    #[test]
    fn test_arithmetic_expression() {
        let tokens = lex("5 + 3 * 2");
        assert_eq!(tokens.len(), 6); // Int + Plus + Int + Star + Int + Eof
        assert!(matches!(tokens[0], TokenKind::Integer(5)));
        assert!(matches!(tokens[1], TokenKind::Plus));
        assert!(matches!(tokens[2], TokenKind::Integer(3)));
        assert!(matches!(tokens[3], TokenKind::Star));
        assert!(matches!(tokens[4], TokenKind::Integer(2)));
    }

    #[test]
    fn test_division_not_comment() {
        let tokens = lex("10 / 2");
        assert_eq!(tokens.len(), 4); // Int + Slash + Int + Eof
        assert!(matches!(tokens[0], TokenKind::Integer(10)));
        assert!(matches!(tokens[1], TokenKind::Slash));
        assert!(matches!(tokens[2], TokenKind::Integer(2)));
    }

    #[test]
    fn test_string_interpolation_simple() {
        // {var} is interpolation
        let tokens = lex(r#""Hello {name}!""#);
        assert_eq!(tokens.len(), 2); // StringInterpolated + Eof

        match &tokens[0] {
            TokenKind::StringInterpolated(parts) => {
                assert_eq!(parts.len(), 3);
                assert!(matches!(&parts[0], StringPart::Text(t) if t == "Hello "));
                assert!(matches!(&parts[1], StringPart::Variable(v) if v == "name"));
                assert!(matches!(&parts[2], StringPart::Text(t) if t == "!"));
            }
            _ => panic!("Expected StringInterpolated"),
        }
    }

    #[test]
    fn test_string_interpolation_multiple() {
        let tokens = lex(r#""{name} is {age} years old""#);
        assert_eq!(tokens.len(), 2);

        match &tokens[0] {
            TokenKind::StringInterpolated(parts) => {
                assert_eq!(parts.len(), 4);
                assert!(matches!(&parts[0], StringPart::Variable(v) if v == "name"));
                assert!(matches!(&parts[1], StringPart::Text(t) if t == " is "));
                assert!(matches!(&parts[2], StringPart::Variable(v) if v == "age"));
                assert!(matches!(&parts[3], StringPart::Text(t) if t == " years old"));
            }
            _ => panic!("Expected StringInterpolated"),
        }
    }

    #[test]
    fn test_string_literal_braces() {
        // Two-phase design: \{ stores sentinel \x01 in the lexer token;
        // zymbol-interpreter/src/literals.rs resolves \x01 → '{' at runtime.
        // This test verifies the LEXER contract (the sentinel), not the runtime output.
        let tokens = lex(r#""Use \{curly\} braces literally""#);
        assert_eq!(tokens.len(), 2);

        match &tokens[0] {
            TokenKind::String(s) => {
                assert_eq!(s, "Use \x01curly\x02 braces literally");
            }
            _ => panic!("Expected plain String with literal braces"),
        }
    }

    #[test]
    fn test_string_interpolation_only_variable() {
        let tokens = lex(r#""{x}""#);
        assert_eq!(tokens.len(), 2);

        match &tokens[0] {
            TokenKind::StringInterpolated(parts) => {
                assert_eq!(parts.len(), 1);
                assert!(matches!(&parts[0], StringPart::Variable(v) if v == "x"));
            }
            _ => panic!("Expected StringInterpolated"),
        }
    }

    #[test]
    fn test_loop_operator() {
        let tokens = lex("@");
        assert_eq!(tokens.len(), 2); // @ and EOF
        assert!(matches!(tokens[0], TokenKind::At));
    }

    #[test]
    fn test_break_operator() {
        let tokens = lex("@!");
        assert_eq!(tokens.len(), 2); // @! and EOF
        assert!(matches!(tokens[0], TokenKind::AtBreak));
    }

    #[test]
    fn test_continue_operator() {
        let tokens = lex("@>");
        assert_eq!(tokens.len(), 2); // @> and EOF
        assert!(matches!(tokens[0], TokenKind::AtContinue));
    }

    #[test]
    fn test_loop_statement() {
        let tokens = lex("@ x < 5 { }");
        assert_eq!(tokens.len(), 7); // @, x, <, 5, {, }, EOF
        assert!(matches!(tokens[0], TokenKind::At));
        assert!(matches!(tokens[1], TokenKind::Ident(_)));
        assert!(matches!(tokens[2], TokenKind::Lt));
        assert!(matches!(tokens[3], TokenKind::Integer(5)));
        assert!(matches!(tokens[4], TokenKind::LBrace));
        assert!(matches!(tokens[5], TokenKind::RBrace));
    }

    #[test]
    fn test_else_if_token() {
        let tokens = lex("_?");
        assert_eq!(tokens.len(), 2); // _? and EOF
        assert!(matches!(tokens[0], TokenKind::ElseIf));
    }

    #[test]
    fn test_if_else_if_else() {
        let tokens = lex("? x > 10 { } _? x > 5 { } _{ }");
        // ?, x, >, 10, {, }, _?, x, >, 5, {, }, _, {, }, EOF
        assert_eq!(tokens.len(), 16);
        assert!(matches!(tokens[0], TokenKind::Question));
        assert!(matches!(tokens[6], TokenKind::ElseIf));
        assert!(matches!(tokens[12], TokenKind::Underscore));
    }

    #[test]
    fn test_range_operator() {
        let tokens = lex("..");
        assert_eq!(tokens.len(), 2); // .. and EOF
        assert!(matches!(tokens[0], TokenKind::DotDot));
    }

    #[test]
    fn test_range_literal() {
        let tokens = lex("1..10");
        assert_eq!(tokens.len(), 4); // 1, .., 10, EOF
        assert!(matches!(tokens[0], TokenKind::Integer(1)));
        assert!(matches!(tokens[1], TokenKind::DotDot));
        assert!(matches!(tokens[2], TokenKind::Integer(10)));
    }

    #[test]
    fn test_colon_token() {
        let tokens = lex(":");
        assert_eq!(tokens.len(), 2); // : and EOF
        assert!(matches!(tokens[0], TokenKind::Colon));
    }

    #[test]
    fn test_for_each_syntax() {
        let tokens = lex("@ i:1..10 { }");
        // @, i, :, 1, .., 10, {, }, EOF
        assert_eq!(tokens.len(), 9);
        assert!(matches!(tokens[0], TokenKind::At));
        assert!(matches!(tokens[1], TokenKind::Ident(_)));
        assert!(matches!(tokens[2], TokenKind::Colon));
        assert!(matches!(tokens[3], TokenKind::Integer(1)));
        assert!(matches!(tokens[4], TokenKind::DotDot));
        assert!(matches!(tokens[5], TokenKind::Integer(10)));
        assert!(matches!(tokens[6], TokenKind::LBrace));
        assert!(matches!(tokens[7], TokenKind::RBrace));
    }

    #[test]
    fn test_float_simple() {
        let tokens = lex("3.14");
        assert_eq!(tokens.len(), 2); // Float + Eof
        match &tokens[0] {
            TokenKind::Float(f) => assert_eq!(*f, 3.14),
            _ => panic!("Expected float token"),
        }
    }

    #[test]
    fn test_float_scientific_notation() {
        let tokens = lex("3e8 2.5e10 1.5E-3");
        assert_eq!(tokens.len(), 4); // 3 floats + Eof

        match &tokens[0] {
            TokenKind::Float(f) => assert_eq!(*f, 3e8),
            _ => panic!("Expected float token"),
        }

        match &tokens[1] {
            TokenKind::Float(f) => assert_eq!(*f, 2.5e10),
            _ => panic!("Expected float token"),
        }

        match &tokens[2] {
            TokenKind::Float(f) => assert_eq!(*f, 1.5E-3),
            _ => panic!("Expected float token"),
        }
    }

    #[test]
    fn test_float_vs_range() {
        // Make sure we don't confuse float with range operator
        let tokens = lex("1..10");
        assert_eq!(tokens.len(), 4); // Int, DotDot, Int, Eof
        assert!(matches!(tokens[0], TokenKind::Integer(1)));
        assert!(matches!(tokens[1], TokenKind::DotDot));
        assert!(matches!(tokens[2], TokenKind::Integer(10)));
    }

    #[test]
    fn test_char_simple() {
        let tokens = lex("'A'");
        assert_eq!(tokens.len(), 2); // Char + Eof
        match &tokens[0] {
            TokenKind::Char(c) => assert_eq!(*c, 'A'),
            _ => panic!("Expected char token"),
        }
    }

    #[test]
    fn test_char_unicode() {
        let tokens = lex("'😀'");
        assert_eq!(tokens.len(), 2); // Char + Eof
        match &tokens[0] {
            TokenKind::Char(c) => assert_eq!(*c, '😀'),
            _ => panic!("Expected char token"),
        }
    }

    #[test]
    fn test_base_char_hexadecimal() {
        let tokens = lex("0x41"); // 'A' in hexadecimal
        assert_eq!(tokens.len(), 2); // Char + Eof
        match &tokens[0] {
            TokenKind::Char(c) => assert_eq!(*c, 'A'),
            _ => panic!("Expected char token, got {:?}", tokens[0]),
        }
    }

    #[test]
    fn test_base_char_binary() {
        let tokens = lex("0b01000001"); // 'A' in binary
        assert_eq!(tokens.len(), 2); // Char + Eof
        match &tokens[0] {
            TokenKind::Char(c) => assert_eq!(*c, 'A'),
            _ => panic!("Expected char token"),
        }
    }

    #[test]
    fn test_base_char_octal() {
        let tokens = lex("0o0101"); // 'A' in octal
        assert_eq!(tokens.len(), 2); // Char + Eof
        match &tokens[0] {
            TokenKind::Char(c) => assert_eq!(*c, 'A'),
            _ => panic!("Expected char token"),
        }
    }

    #[test]
    fn test_base_char_decimal() {
        let tokens = lex("0d65"); // 'A' in decimal
        assert_eq!(tokens.len(), 2); // Char + Eof
        match &tokens[0] {
            TokenKind::Char(c) => assert_eq!(*c, 'A'),
            _ => panic!("Expected char token"),
        }
    }

    #[test]
    fn test_base_char_unicode_emoji() {
        let tokens = lex("0x1F600"); // '😀' in hexadecimal
        assert_eq!(tokens.len(), 2); // Char + Eof
        match &tokens[0] {
            TokenKind::Char(c) => assert_eq!(*c, '😀'),
            _ => panic!("Expected char token"),
        }
    }

    #[test]
    fn test_pipe_token() {
        let tokens = lex("|");
        assert_eq!(tokens.len(), 2); // Pipe + Eof
        assert!(matches!(tokens[0], TokenKind::Pipe));
    }

    #[test]
    fn test_hash_pipe_token() {
        let tokens = lex("#|");
        assert_eq!(tokens.len(), 2); // HashPipe + Eof
        assert!(matches!(tokens[0], TokenKind::HashPipe));
    }

    #[test]
    fn test_numeric_eval_syntax() {
        let tokens = lex("#|\"123\"|");
        assert_eq!(tokens.len(), 4); // HashPipe, String, Pipe, Eof
        assert!(matches!(tokens[0], TokenKind::HashPipe));
        assert!(matches!(tokens[1], TokenKind::String(_)));
        assert!(matches!(tokens[2], TokenKind::Pipe));
    }

    #[test]
    fn test_hash_question_token() {
        let tokens = lex("#?");
        assert_eq!(tokens.len(), 2); // HashQuestion + Eof
        assert!(matches!(tokens[0], TokenKind::HashQuestion));
    }

    #[test]
    fn test_type_metadata_syntax() {
        let tokens = lex("x#?");
        assert_eq!(tokens.len(), 3); // Ident, HashQuestion, Eof
        assert!(matches!(tokens[0], TokenKind::Ident(_)));
        assert!(matches!(tokens[1], TokenKind::HashQuestion));
    }

    #[test]
    fn test_combined_numeric_eval_and_type_metadata() {
        let tokens = lex("#|x|#?");
        assert_eq!(tokens.len(), 5); // HashPipe, Ident, Pipe, HashQuestion, Eof
        assert!(matches!(tokens[0], TokenKind::HashPipe));
        assert!(matches!(tokens[1], TokenKind::Ident(_)));
        assert!(matches!(tokens[2], TokenKind::Pipe));
        assert!(matches!(tokens[3], TokenKind::HashQuestion));
    }

    #[test]
    fn test_hash_comma_token() {
        // #, followed by | (open pipe) — two separate tokens
        let tokens = lex("#,|");
        assert_eq!(tokens.len(), 3); // HashComma, Pipe, Eof
        assert!(matches!(tokens[0], TokenKind::HashComma));
        assert!(matches!(tokens[1], TokenKind::Pipe));
    }

    #[test]
    fn test_hash_caret_token() {
        let tokens = lex("#^|");
        assert_eq!(tokens.len(), 3); // HashCaret, Pipe, Eof
        assert!(matches!(tokens[0], TokenKind::HashCaret));
        assert!(matches!(tokens[1], TokenKind::Pipe));
    }

    #[test]
    fn test_hash_comma_with_dot_precision() {
        // #,.2|val|  →  HashComma, Dot, Integer(2), Pipe, Ident, Pipe, Eof
        let tokens = lex("#,.2|val|");
        assert_eq!(tokens.len(), 7);
        assert!(matches!(tokens[0], TokenKind::HashComma));
        assert!(matches!(tokens[1], TokenKind::Dot));
        assert!(matches!(tokens[2], TokenKind::Integer(2)));
        assert!(matches!(tokens[3], TokenKind::Pipe));
        assert!(matches!(tokens[4], TokenKind::Ident(_)));
        assert!(matches!(tokens[5], TokenKind::Pipe));
    }

    #[test]
    fn test_hash_caret_with_exclaim_precision() {
        // #^!2|val|  →  HashCaret, Not, Integer(2), Pipe, Ident, Pipe, Eof
        // bare '!' (not preceded by '#') produces Not token
        let tokens = lex("#^!2|val|");
        assert_eq!(tokens.len(), 7);
        assert!(matches!(tokens[0], TokenKind::HashCaret));
        assert!(matches!(tokens[1], TokenKind::Not));
        assert!(matches!(tokens[2], TokenKind::Integer(2)));
        assert!(matches!(tokens[3], TokenKind::Pipe));
        assert!(matches!(tokens[4], TokenKind::Ident(_)));
        assert!(matches!(tokens[5], TokenKind::Pipe));
    }

    #[test]
    fn test_hash_comma_expression() {
        let tokens = lex("#,|1500000|");
        assert_eq!(tokens.len(), 5); // HashComma, Pipe, Integer, Pipe, Eof
        assert!(matches!(tokens[0], TokenKind::HashComma));
        assert!(matches!(tokens[1], TokenKind::Pipe));
        assert!(matches!(tokens[2], TokenKind::Integer(1500000)));
        assert!(matches!(tokens[3], TokenKind::Pipe));
    }

    #[test]
    fn test_hash_caret_expression() {
        let tokens = lex("#^|total|");
        assert_eq!(tokens.len(), 5); // HashCaret, Pipe, Ident, Pipe, Eof
        assert!(matches!(tokens[0], TokenKind::HashCaret));
        assert!(matches!(tokens[1], TokenKind::Pipe));
        assert!(matches!(tokens[2], TokenKind::Ident(_)));
        assert!(matches!(tokens[3], TokenKind::Pipe));
    }

    #[test]
    fn test_e_is_identifier() {
        // 'e' is now always an identifier (no longer overloaded as format prefix)
        let tokens = lex("e");
        assert_eq!(tokens.len(), 2); // Ident + Eof
        match &tokens[0] {
            TokenKind::Ident(name) => assert_eq!(name, "e"),
            _ => panic!("Expected identifier token"),
        }
    }

    #[test]
    fn test_c_is_identifier() {
        // 'c' is now always an identifier (no longer overloaded as format prefix)
        let tokens = lex("c");
        assert_eq!(tokens.len(), 2); // Ident + Eof
        match &tokens[0] {
            TokenKind::Ident(name) => assert_eq!(name, "c"),
            _ => panic!("Expected identifier token"),
        }
    }

    #[test]
    fn test_char_escape_sequences() {
        // Test newline escape
        let tokens = lex(r"'\n'");
        match &tokens[0] {
            TokenKind::Char(c) => assert_eq!(*c, '\n'),
            _ => panic!("Expected char token"),
        }

        // Test tab escape
        let tokens = lex(r"'\t'");
        match &tokens[0] {
            TokenKind::Char(c) => assert_eq!(*c, '\t'),
            _ => panic!("Expected char token"),
        }

        // Test quote escape
        let tokens = lex(r"'\''");
        match &tokens[0] {
            TokenKind::Char(c) => assert_eq!(*c, '\''),
            _ => panic!("Expected char token"),
        }
    }

    #[test]
    fn test_mixed_types() {
        let tokens = lex("42 3.14 \"hello\" 'A' #1");
        assert_eq!(tokens.len(), 6); // Int + Float + String + Char + Bool + Eof
        assert!(matches!(tokens[0], TokenKind::Integer(42)));
        match &tokens[1] {
            TokenKind::Float(f) => assert_eq!(*f, 3.14),
            _ => panic!("Expected float"),
        }
        assert!(matches!(tokens[2], TokenKind::String(_)));
        match &tokens[3] {
            TokenKind::Char(c) => assert_eq!(*c, 'A'),
            _ => panic!("Expected char"),
        }
        assert!(matches!(tokens[4], TokenKind::Boolean(true)));
    }

    #[test]
    fn test_double_question_token() {
        let tokens = lex("??");
        assert_eq!(tokens.len(), 2); // DoubleQuestion + Eof
        assert!(matches!(tokens[0], TokenKind::DoubleQuestion));
    }

    #[test]
    fn test_question_vs_double_question() {
        // Ensure ?? is recognized as DoubleQuestion, not Question + Question
        let tokens = lex("? ??");
        assert_eq!(tokens.len(), 3); // Question, DoubleQuestion, Eof
        assert!(matches!(tokens[0], TokenKind::Question));
        assert!(matches!(tokens[1], TokenKind::DoubleQuestion));
    }

    #[test]
    fn test_const_assign_token() {
        let tokens = lex(":=");
        assert_eq!(tokens.len(), 2); // ConstAssign + Eof
        assert!(matches!(tokens[0], TokenKind::ConstAssign));
    }

    #[test]
    fn test_const_assignment_statement() {
        let tokens = lex("PI := 3.14159");
        assert_eq!(tokens.len(), 4); // Ident + ConstAssign + Float + Eof
        assert!(matches!(tokens[0], TokenKind::Ident(_)));
        assert!(matches!(tokens[1], TokenKind::ConstAssign));
        match &tokens[2] {
            TokenKind::Float(f) => assert_eq!(*f, 3.14159),
            _ => panic!("Expected float token"),
        }
    }

    #[test]
    fn test_colon_vs_const_assign() {
        // Ensure := is recognized as ConstAssign, not Colon + Assign
        let tokens = lex(": :=");
        assert_eq!(tokens.len(), 3); // Colon, ConstAssign, Eof
        assert!(matches!(tokens[0], TokenKind::Colon));
        assert!(matches!(tokens[1], TokenKind::ConstAssign));
    }

    // ===== MODULE SYSTEM TESTS =====

    #[test]
    fn test_hash_token() {
        let tokens = lex("#");
        assert_eq!(tokens.len(), 2); // Hash + Eof
        assert!(matches!(tokens[0], TokenKind::Hash));
    }

    #[test]
    fn test_export_block_token() {
        let tokens = lex("#>");
        assert_eq!(tokens.len(), 2); // ExportBlock + Eof
        assert!(matches!(tokens[0], TokenKind::ExportBlock));
    }

    #[test]
    fn test_module_import_token() {
        let tokens = lex("<#");
        assert_eq!(tokens.len(), 2); // ModuleImport + Eof
        assert!(matches!(tokens[0], TokenKind::ModuleImport));
    }

    #[test]
    fn test_scope_resolution_token() {
        let tokens = lex("::");
        assert_eq!(tokens.len(), 2); // ScopeResolution + Eof
        assert!(matches!(tokens[0], TokenKind::ScopeResolution));
    }

    #[test]
    fn test_module_declaration() {
        let tokens = lex("# math_utils");
        assert_eq!(tokens.len(), 3); // Hash + Ident + Eof
        assert!(matches!(tokens[0], TokenKind::Hash));
        match &tokens[1] {
            TokenKind::Ident(s) => assert_eq!(s, "math_utils"),
            _ => panic!("Expected identifier"),
        }
    }

    #[test]
    fn test_export_block_with_items() {
        let tokens = lex("#> { add, subtract }");
        // ExportBlock, LBrace, Ident, Comma, Ident, RBrace, Eof
        assert_eq!(tokens.len(), 7);
        assert!(matches!(tokens[0], TokenKind::ExportBlock));
        assert!(matches!(tokens[1], TokenKind::LBrace));
        assert!(matches!(tokens[2], TokenKind::Ident(_)));
        assert!(matches!(tokens[3], TokenKind::Comma));
        assert!(matches!(tokens[4], TokenKind::Ident(_)));
        assert!(matches!(tokens[5], TokenKind::RBrace));
    }

    #[test]
    fn test_module_import_statement() {
        let tokens = lex("<# ./math_utils <= math");
        // ModuleImport, Dot, Slash, Ident, Le, Ident, Eof
        assert_eq!(tokens.len(), 7);
        assert!(matches!(tokens[0], TokenKind::ModuleImport));
        assert!(matches!(tokens[1], TokenKind::Dot));
        assert!(matches!(tokens[2], TokenKind::Slash));
        assert!(matches!(tokens[3], TokenKind::Ident(_)));
        assert!(matches!(tokens[4], TokenKind::Le)); // <= for alias
        assert!(matches!(tokens[5], TokenKind::Ident(_)));
    }

    #[test]
    fn test_module_function_call() {
        let tokens = lex("math::add(5, 3)");
        // Ident, ScopeResolution, Ident, LParen, Int, Comma, Int, RParen, Eof
        assert_eq!(tokens.len(), 9);
        match &tokens[0] {
            TokenKind::Ident(s) => assert_eq!(s, "math"),
            _ => panic!("Expected identifier"),
        }
        assert!(matches!(tokens[1], TokenKind::ScopeResolution));
        match &tokens[2] {
            TokenKind::Ident(s) => assert_eq!(s, "add"),
            _ => panic!("Expected identifier"),
        }
        assert!(matches!(tokens[3], TokenKind::LParen));
        assert!(matches!(tokens[4], TokenKind::Integer(5)));
        assert!(matches!(tokens[5], TokenKind::Comma));
        assert!(matches!(tokens[6], TokenKind::Integer(3)));
        assert!(matches!(tokens[7], TokenKind::RParen));
    }

    #[test]
    fn test_module_constant_access() {
        let tokens = lex("math.PI");
        // Ident, Dot, Ident, Eof
        assert_eq!(tokens.len(), 4);
        match &tokens[0] {
            TokenKind::Ident(s) => assert_eq!(s, "math"),
            _ => panic!("Expected identifier"),
        }
        assert!(matches!(tokens[1], TokenKind::Dot));
        match &tokens[2] {
            TokenKind::Ident(s) => assert_eq!(s, "PI"),
            _ => panic!("Expected identifier"),
        }
    }

    #[test]
    fn test_re_export_function() {
        let tokens = lex("math::add");
        // Ident, ScopeResolution, Ident, Eof
        assert_eq!(tokens.len(), 4);
        match &tokens[0] {
            TokenKind::Ident(s) => assert_eq!(s, "math"),
            _ => panic!("Expected identifier"),
        }
        assert!(matches!(tokens[1], TokenKind::ScopeResolution));
        match &tokens[2] {
            TokenKind::Ident(s) => assert_eq!(s, "add"),
            _ => panic!("Expected identifier"),
        }
    }

    #[test]
    fn test_re_export_constant() {
        let tokens = lex("math.PI");
        // Ident, Dot, Ident, Eof
        assert_eq!(tokens.len(), 4);
        match &tokens[0] {
            TokenKind::Ident(s) => assert_eq!(s, "math"),
            _ => panic!("Expected identifier"),
        }
        assert!(matches!(tokens[1], TokenKind::Dot));
        match &tokens[2] {
            TokenKind::Ident(s) => assert_eq!(s, "PI"),
            _ => panic!("Expected identifier"),
        }
    }

    #[test]
    fn test_re_export_renamed() {
        let tokens = lex("math::add <= sum");
        // Ident, ScopeResolution, Ident, Le, Ident, Eof
        assert_eq!(tokens.len(), 6);
        match &tokens[0] {
            TokenKind::Ident(s) => assert_eq!(s, "math"),
            _ => panic!("Expected identifier"),
        }
        assert!(matches!(tokens[1], TokenKind::ScopeResolution));
        match &tokens[2] {
            TokenKind::Ident(s) => assert_eq!(s, "add"),
            _ => panic!("Expected identifier"),
        }
        assert!(matches!(tokens[3], TokenKind::Le)); // <= for rename
        match &tokens[4] {
            TokenKind::Ident(s) => assert_eq!(s, "sum"),
            _ => panic!("Expected identifier"),
        }
    }

    #[test]
    fn test_hash_not_confused_with_export() {
        // Ensure # module and #> export are distinct
        let tokens = lex("# #>");
        assert_eq!(tokens.len(), 3); // Hash, ExportBlock, Eof
        assert!(matches!(tokens[0], TokenKind::Hash));
        assert!(matches!(tokens[1], TokenKind::ExportBlock));
    }

    #[test]
    fn test_hash_not_confused_with_booleans() {
        // Ensure # module, #1, and #0 are distinct
        let tokens = lex("# #1 #0");
        assert_eq!(tokens.len(), 4); // Hash, Bool(true), Bool(false), Eof
        assert!(matches!(tokens[0], TokenKind::Hash));
        assert!(matches!(tokens[1], TokenKind::Boolean(true)));
        assert!(matches!(tokens[2], TokenKind::Boolean(false)));
    }

    #[test]
    fn test_colon_vs_scope_resolution() {
        // Ensure :: is recognized as ScopeResolution, not Colon + Colon
        let tokens = lex(": ::");
        assert_eq!(tokens.len(), 3); // Colon, ScopeResolution, Eof
        assert!(matches!(tokens[0], TokenKind::Colon));
        assert!(matches!(tokens[1], TokenKind::ScopeResolution));
    }

    #[test]
    fn test_lt_vs_module_import() {
        // Ensure <# is recognized as ModuleImport, not Lt + Hash
        let tokens = lex("< <#");
        assert_eq!(tokens.len(), 3); // Lt, ModuleImport, Eof
        assert!(matches!(tokens[0], TokenKind::Lt));
        assert!(matches!(tokens[1], TokenKind::ModuleImport));
    }

    #[test]
    fn test_complete_module_example() {
        let source = r#"
# math_utils

#> {
    add
    PI
}

PI := 3.14159

add(a, b) {
    <~ a + b
}
"#;
        let tokens = lex(source);
        // Verify key tokens are present
        let has_hash = tokens.iter().any(|t| matches!(t, TokenKind::Hash));
        let has_export_block = tokens.iter().any(|t| matches!(t, TokenKind::ExportBlock));
        let has_const_assign = tokens.iter().any(|t| matches!(t, TokenKind::ConstAssign));
        let has_return = tokens.iter().any(|t| matches!(t, TokenKind::Return));

        assert!(has_hash, "Should have # token for module declaration");
        assert!(has_export_block, "Should have #> token for export block");
        assert!(has_const_assign, "Should have := token for constant");
        assert!(has_return, "Should have <~ token for return");
    }

    // ========== Multiline Comment Tests ==========

    #[test]
    fn test_multiline_comment_basic() {
        let tokens = lex(">> \"test\" /* comment */ >> \"test2\"");
        assert_eq!(tokens.len(), 5); // Output + String + Output + String + Eof
        assert!(matches!(tokens[0], TokenKind::Output));
        assert!(matches!(tokens[1], TokenKind::String(_)));
        assert!(matches!(tokens[2], TokenKind::Output));
        assert!(matches!(tokens[3], TokenKind::String(_)));
    }

    #[test]
    fn test_multiline_comment_multiple_lines() {
        let source = r#">> "before"
/* This is a
   multiline
   comment */
>> "after""#;
        let tokens = lex(source);
        assert_eq!(tokens.len(), 5); // Output + String + Output + String + Eof
    }

    #[test]
    fn test_multiline_comment_nested_single() {
        let tokens = lex("/* outer /* inner */ still outer */ x");
        assert_eq!(tokens.len(), 2); // Ident("x") + Eof
        match &tokens[0] {
            TokenKind::Ident(s) => assert_eq!(s, "x"),
            _ => panic!("Expected identifier after nested comment"),
        }
    }

    #[test]
    fn test_multiline_comment_nested_deep() {
        let tokens = lex("/* level1 /* level2 /* level3 */ back2 */ back1 */ value");
        assert_eq!(tokens.len(), 2); // Ident("value") + Eof
        match &tokens[0] {
            TokenKind::Ident(s) => assert_eq!(s, "value"),
            _ => panic!("Expected identifier after deeply nested comment"),
        }
    }

    #[test]
    fn test_multiline_comment_unterminated() {
        let lexer = Lexer::new("x = 5 /* unterminated comment", FileId(0));
        let (tokens, diagnostics) = lexer.tokenize();

        // Should have exactly one error diagnostic
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.to_lowercase().contains("unterminated"));
        assert!(diagnostics[0].message.to_lowercase().contains("multi-line") ||
                diagnostics[0].message.to_lowercase().contains("multiline"));

        // Tokens before comment should still be generated
        assert!(tokens.iter().any(|t| matches!(&t.kind, TokenKind::Ident(s) if s == "x")));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Assign)));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Integer(5))));
    }

    #[test]
    fn test_multiline_comment_unterminated_nested() {
        let lexer = Lexer::new("/* outer /* inner */", FileId(0));
        let (_, diagnostics) = lexer.tokenize();

        // Should report unterminated (missing final */)
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.to_lowercase().contains("unterminated"));
    }

    #[test]
    fn test_division_not_multiline_comment() {
        let tokens = lex("x / * y"); // division followed by multiply
        assert_eq!(tokens.len(), 5); // Ident + Slash + Star + Ident + Eof
        match &tokens[0] {
            TokenKind::Ident(s) => assert_eq!(s, "x"),
            _ => panic!("Expected identifier"),
        }
        assert!(matches!(tokens[1], TokenKind::Slash));
        assert!(matches!(tokens[2], TokenKind::Star));
        match &tokens[3] {
            TokenKind::Ident(s) => assert_eq!(s, "y"),
            _ => panic!("Expected identifier"),
        }
    }

    #[test]
    fn test_comment_syntax_in_string() {
        let tokens = lex(r#""This /* is not a comment */""#);
        assert_eq!(tokens.len(), 2); // String + Eof
        match &tokens[0] {
            TokenKind::String(s) => assert_eq!(s, "This /* is not a comment */"),
            _ => panic!("Expected string token"),
        }
    }

    #[test]
    fn test_multiline_comment_empty() {
        let tokens = lex("/**/ x");
        assert_eq!(tokens.len(), 2); // Ident("x") + Eof
        match &tokens[0] {
            TokenKind::Ident(s) => assert_eq!(s, "x"),
            _ => panic!("Expected identifier"),
        }
    }

    #[test]
    fn test_multiline_comment_between_tokens() {
        let tokens = lex("x /* comment */ = /* another */ 5");
        assert_eq!(tokens.len(), 4); // Ident + Assign + Integer + Eof
        assert!(matches!(tokens[0], TokenKind::Ident(_)));
        assert!(matches!(tokens[1], TokenKind::Assign));
        match &tokens[2] {
            TokenKind::Integer(n) => assert_eq!(*n, 5),
            _ => panic!("Expected integer"),
        }
    }

    #[test]
    fn test_multiline_comment_in_whitespace() {
        let tokens = lex("   /* comment */   x");
        assert_eq!(tokens.len(), 2); // Ident("x") + Eof
        match &tokens[0] {
            TokenKind::Ident(s) => assert_eq!(s, "x"),
            _ => panic!("Expected identifier"),
        }
    }

    #[test]
    fn test_divide_assign_not_comment() {
        let tokens = lex("x /= 5");
        assert_eq!(tokens.len(), 4); // Ident + SlashAssign + Integer + Eof
        assert!(matches!(tokens[0], TokenKind::Ident(_)));
        assert!(matches!(tokens[1], TokenKind::SlashAssign));
        assert!(matches!(tokens[2], TokenKind::Integer(5)));
    }

    // ========== Precision Expression Tests ==========

    #[test]
    fn test_hash_dot_token() {
        let tokens = lex("#.");
        assert_eq!(tokens.len(), 2); // HashDot + Eof
        assert!(matches!(tokens[0], TokenKind::HashDot));
    }

    #[test]
    fn test_hash_exclaim_token() {
        let tokens = lex("#!");
        assert_eq!(tokens.len(), 2); // HashExclaim + Eof
        assert!(matches!(tokens[0], TokenKind::HashExclaim));
    }

    #[test]
    fn test_round_expression_tokens() {
        let tokens = lex("#.2|x|");
        // HashDot, Integer(2), Pipe, Ident("x"), Pipe, Eof
        assert_eq!(tokens.len(), 6);
        assert!(matches!(tokens[0], TokenKind::HashDot));
        assert!(matches!(tokens[1], TokenKind::Integer(2)));
        assert!(matches!(tokens[2], TokenKind::Pipe));
        assert!(matches!(tokens[3], TokenKind::Ident(_)));
        assert!(matches!(tokens[4], TokenKind::Pipe));
    }

    #[test]
    fn test_trunc_expression_tokens() {
        let tokens = lex("#!3|value|");
        // HashExclaim, Integer(3), Pipe, Ident("value"), Pipe, Eof
        assert_eq!(tokens.len(), 6);
        assert!(matches!(tokens[0], TokenKind::HashExclaim));
        assert!(matches!(tokens[1], TokenKind::Integer(3)));
        assert!(matches!(tokens[2], TokenKind::Pipe));
        assert!(matches!(tokens[3], TokenKind::Ident(_)));
        assert!(matches!(tokens[4], TokenKind::Pipe));
    }

    #[test]
    fn test_round_expression_with_expression() {
        let tokens = lex("#.2|x * y|");
        // HashDot, Integer(2), Pipe, Ident("x"), Star, Ident("y"), Pipe, Eof
        assert_eq!(tokens.len(), 8);
        assert!(matches!(tokens[0], TokenKind::HashDot));
        assert!(matches!(tokens[1], TokenKind::Integer(2)));
        assert!(matches!(tokens[2], TokenKind::Pipe));
        assert!(matches!(tokens[3], TokenKind::Ident(_)));
        assert!(matches!(tokens[4], TokenKind::Star));
        assert!(matches!(tokens[5], TokenKind::Ident(_)));
        assert!(matches!(tokens[6], TokenKind::Pipe));
    }

    #[test]
    fn test_hash_operators_disambiguation() {
        // Ensure all # variants are correctly recognized
        let tokens = lex("#. #! #| #? #> # #1 #0");
        assert_eq!(tokens.len(), 9); // 8 tokens + Eof
        assert!(matches!(tokens[0], TokenKind::HashDot));
        assert!(matches!(tokens[1], TokenKind::HashExclaim));
        assert!(matches!(tokens[2], TokenKind::HashPipe));
        assert!(matches!(tokens[3], TokenKind::HashQuestion));
        assert!(matches!(tokens[4], TokenKind::ExportBlock));
        assert!(matches!(tokens[5], TokenKind::Hash));
        assert!(matches!(tokens[6], TokenKind::Boolean(true)));
        assert!(matches!(tokens[7], TokenKind::Boolean(false)));
    }

    // ========== Error Handling Tests ==========

    #[test]
    fn test_try_block_token() {
        let tokens = lex("!?");
        assert_eq!(tokens.len(), 2); // TryBlock + Eof
        assert!(matches!(tokens[0], TokenKind::TryBlock));
    }

    #[test]
    fn test_catch_block_token() {
        let tokens = lex(":!");
        assert_eq!(tokens.len(), 2); // CatchBlock + Eof
        assert!(matches!(tokens[0], TokenKind::CatchBlock));
    }

    #[test]
    fn test_finally_block_token() {
        let tokens = lex(":>");
        assert_eq!(tokens.len(), 2); // FinallyBlock + Eof
        assert!(matches!(tokens[0], TokenKind::FinallyBlock));
    }

    #[test]
    fn test_dollar_exclaim_token() {
        let tokens = lex("x$!");
        assert_eq!(tokens.len(), 3); // Ident + DollarExclaim + Eof
        assert!(matches!(tokens[0], TokenKind::Ident(_)));
        assert!(matches!(tokens[1], TokenKind::DollarExclaim));
    }

    #[test]
    fn test_dollar_exclaim_exclaim_token() {
        let tokens = lex("x$!!");
        assert_eq!(tokens.len(), 3); // Ident + DollarExclaimExclaim + Eof
        assert!(matches!(tokens[0], TokenKind::Ident(_)));
        assert!(matches!(tokens[1], TokenKind::DollarExclaimExclaim));
    }

    #[test]
    fn test_try_catch_finally_structure() {
        let tokens = lex("!?{ } :! { } :>{ }");
        // TryBlock, LBrace, RBrace, CatchBlock, LBrace, RBrace, FinallyBlock, LBrace, RBrace, Eof
        assert_eq!(tokens.len(), 10);
        assert!(matches!(tokens[0], TokenKind::TryBlock));
        assert!(matches!(tokens[1], TokenKind::LBrace));
        assert!(matches!(tokens[2], TokenKind::RBrace));
        assert!(matches!(tokens[3], TokenKind::CatchBlock));
        assert!(matches!(tokens[4], TokenKind::LBrace));
        assert!(matches!(tokens[5], TokenKind::RBrace));
        assert!(matches!(tokens[6], TokenKind::FinallyBlock));
        assert!(matches!(tokens[7], TokenKind::LBrace));
        assert!(matches!(tokens[8], TokenKind::RBrace));
    }

    #[test]
    fn test_typed_catch_with_error_type() {
        let tokens = lex(":! ##IO { }");
        // CatchBlock, Hash, Hash, Ident("IO"), LBrace, RBrace, Eof
        assert_eq!(tokens.len(), 7);
        assert!(matches!(tokens[0], TokenKind::CatchBlock));
        assert!(matches!(tokens[1], TokenKind::Hash));
        assert!(matches!(tokens[2], TokenKind::Hash));
        match &tokens[3] {
            TokenKind::Ident(s) => assert_eq!(s, "IO"),
            _ => panic!("Expected identifier 'IO'"),
        }
        assert!(matches!(tokens[4], TokenKind::LBrace));
        assert!(matches!(tokens[5], TokenKind::RBrace));
    }

    #[test]
    fn test_error_check_in_condition() {
        let tokens = lex("? result$! { }");
        // Question, Ident, DollarExclaim, LBrace, RBrace, Eof
        assert_eq!(tokens.len(), 6);
        assert!(matches!(tokens[0], TokenKind::Question));
        assert!(matches!(tokens[1], TokenKind::Ident(_)));
        assert!(matches!(tokens[2], TokenKind::DollarExclaim));
        assert!(matches!(tokens[3], TokenKind::LBrace));
        assert!(matches!(tokens[4], TokenKind::RBrace));
    }

    #[test]
    fn test_error_propagate_statement() {
        let tokens = lex("? x$! { x$!! }");
        // Question, Ident, DollarExclaim, LBrace, Ident, DollarExclaimExclaim, RBrace, Eof
        assert_eq!(tokens.len(), 8);
        assert!(matches!(tokens[0], TokenKind::Question));
        assert!(matches!(tokens[1], TokenKind::Ident(_)));
        assert!(matches!(tokens[2], TokenKind::DollarExclaim));
        assert!(matches!(tokens[3], TokenKind::LBrace));
        assert!(matches!(tokens[4], TokenKind::Ident(_)));
        assert!(matches!(tokens[5], TokenKind::DollarExclaimExclaim));
        assert!(matches!(tokens[6], TokenKind::RBrace));
    }

    #[test]
    fn test_not_vs_try_block_disambiguation() {
        // Ensure !x is Not + Ident, but !? is TryBlock
        let tokens = lex("!x !?{ }");
        // Not, Ident("x"), TryBlock, LBrace, RBrace, Eof
        assert_eq!(tokens.len(), 6);
        assert!(matches!(tokens[0], TokenKind::Not));
        assert!(matches!(tokens[1], TokenKind::Ident(_)));
        assert!(matches!(tokens[2], TokenKind::TryBlock));
        assert!(matches!(tokens[3], TokenKind::LBrace));
        assert!(matches!(tokens[4], TokenKind::RBrace));
    }

    #[test]
    fn test_colon_operators_disambiguation() {
        // Ensure : := :: :! :> are all correctly recognized
        let tokens = lex(": := :: :! :>");
        assert_eq!(tokens.len(), 6); // 5 tokens + Eof
        assert!(matches!(tokens[0], TokenKind::Colon));
        assert!(matches!(tokens[1], TokenKind::ConstAssign));
        assert!(matches!(tokens[2], TokenKind::ScopeResolution));
        assert!(matches!(tokens[3], TokenKind::CatchBlock));
        assert!(matches!(tokens[4], TokenKind::FinallyBlock));
    }

    #[test]
    fn test_dollar_operators_with_exclaim() {
        // Ensure $! and $!! don't conflict with other $ operators
        let tokens = lex("$# $+ $- $? $~ $! $!!");
        assert_eq!(tokens.len(), 8); // 7 tokens + Eof
        assert!(matches!(tokens[0], TokenKind::DollarHash));
        assert!(matches!(tokens[1], TokenKind::DollarPlus));
        assert!(matches!(tokens[2], TokenKind::DollarMinus));
        assert!(matches!(tokens[3], TokenKind::DollarQuestion));
        assert!(matches!(tokens[4], TokenKind::DollarTilde));
        assert!(matches!(tokens[5], TokenKind::DollarExclaim));
        assert!(matches!(tokens[6], TokenKind::DollarExclaimExclaim));
    }

    // ── v0.0.2 positional token tests ────────────────────────────────────────

    #[test]
    fn test_dollar_plus_lbracket() {
        // $+[ (no space) must lex as a single DollarPlusLBracket token
        let tokens = lex("arr$+[2] 99");
        assert!(matches!(tokens[1], TokenKind::DollarPlusLBracket),
            "expected DollarPlusLBracket, got {:?}", tokens[1]);
    }

    #[test]
    fn test_dollar_minus_lbracket() {
        // $-[ (no space) must lex as a single DollarMinusLBracket token
        let tokens = lex("arr$-[0]");
        assert!(matches!(tokens[1], TokenKind::DollarMinusLBracket),
            "expected DollarMinusLBracket, got {:?}", tokens[1]);
    }

    #[test]
    fn test_dollar_minus_lbracket_range() {
        // $-[ used with a range: arr$-[1..3]
        let tokens = lex("arr$-[1..3]");
        assert!(matches!(tokens[1], TokenKind::DollarMinusLBracket),
            "expected DollarMinusLBracket, got {:?}", tokens[1]);
    }

    #[test]
    fn test_dollar_plus_space_lbracket_stays_separate() {
        // $+ followed by space then [ → DollarPlus + LBracket (append array literal)
        let tokens = lex("arr$+ [1]");
        assert!(matches!(tokens[1], TokenKind::DollarPlus),
            "expected DollarPlus, got {:?}", tokens[1]);
        assert!(matches!(tokens[2], TokenKind::LBracket),
            "expected LBracket, got {:?}", tokens[2]);
    }

    #[test]
    fn test_dollar_minus_space_lbracket_stays_separate() {
        // $- followed by space then [ → DollarMinus + LBracket (remove array-literal value)
        let tokens = lex("arr$- [1]");
        assert!(matches!(tokens[1], TokenKind::DollarMinus),
            "expected DollarMinus, got {:?}", tokens[1]);
        assert!(matches!(tokens[2], TokenKind::LBracket),
            "expected LBracket, got {:?}", tokens[2]);
    }

    #[test]
    fn test_dollar_plus_plus_unaffected() {
        // $++ still lexes correctly (kept as retired token for parser migration error)
        let tokens = lex("s$++");
        assert!(matches!(tokens[1], TokenKind::DollarPlusPlus),
            "expected DollarPlusPlus, got {:?}", tokens[1]);
    }

    #[test]
    fn test_dollar_minus_minus_unaffected() {
        // $-- still lexes correctly (repurposed as remove-all by value)
        let tokens = lex("arr$-- 30");
        assert!(matches!(tokens[1], TokenKind::DollarMinusMinus),
            "expected DollarMinusMinus, got {:?}", tokens[1]);
    }

    #[test]
    fn test_dollar_plus_lbracket_not_confused_with_dollar_plus_plus() {
        // $++ must NOT be confused with $+[
        let tokens = lex("s$++");
        assert!(matches!(tokens[1], TokenKind::DollarPlusPlus));
        let tokens2 = lex("arr$+[0]");
        assert!(matches!(tokens2[1], TokenKind::DollarPlusLBracket));
    }

    #[test]
    fn test_dollar_minus_lbracket_not_confused_with_dollar_minus_minus() {
        // $-- must NOT be confused with $-[
        let tokens = lex("arr$--");
        assert!(matches!(tokens[1], TokenKind::DollarMinusMinus));
        let tokens2 = lex("arr$-[0]");
        assert!(matches!(tokens2[1], TokenKind::DollarMinusLBracket));
    }
}
