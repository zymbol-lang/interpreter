//! Literal tokenization for Zymbol-Lang
//!
//! Handles tokenization of all literal types:
//! - Strings (with interpolation support)
//! - Characters (including Unicode and emojis)
//! - Numbers (integers and floats with scientific notation)
//! - Booleans (#1 and #0)
//! - Base character literals (0x, 0b, 0o, 0d)

use zymbol_error::Diagnostic;
use zymbol_span::Position;

use crate::digit_blocks::{digit_block_base, digit_value};
use crate::{Lexer, Token, TokenKind};

/// Parts of an interpolated string
#[derive(Debug, Clone, PartialEq)]
pub enum StringPart {
    /// Plain text
    Text(String),
    /// Variable interpolation {var}
    Variable(String),
}

impl Lexer {
    /// Lex a string literal
    pub(crate) fn lex_string(&mut self, start: Position) -> Token {
        self.advance(); // consume opening "

        let mut parts: Vec<StringPart> = Vec::new();
        let mut current_text = String::new();
        let mut has_interpolation = false;

        while !self.is_at_end() && self.current_char() != '"' {
            let ch = self.current_char();

            if ch == '\\' {
                // Handle escape sequences
                self.advance();
                if self.is_at_end() {
                    break;
                }
                let escaped = match self.current_char() {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    '"' => '"',
                    '\\' => '\\',
                    '{' => '{',  // \{ → literal {
                    '}' => '}',  // \} → literal }
                    _ => self.current_char(),
                };
                current_text.push(escaped);
                self.advance();
            } else if ch == '{' {
                // Start of interpolation {var}
                has_interpolation = true;

                // Save current text part if any
                if !current_text.is_empty() {
                    parts.push(StringPart::Text(current_text.clone()));
                    current_text.clear();
                }

                self.advance(); // consume {

                // Parse variable name
                let mut var_name = String::new();
                while !self.is_at_end() && self.current_char() != '}' {
                    let var_ch = self.current_char();
                    if var_ch.is_alphanumeric() || var_ch == '_' {
                        var_name.push(var_ch);
                        self.advance();
                    } else {
                        // Invalid character in interpolation
                        break;
                    }
                }

                if self.is_at_end() || self.current_char() != '}' {
                    let span = self.span(start);
                    self.diagnostics.push(
                        Diagnostic::error("unterminated interpolation in string")
                            .with_span(span)
                            .with_help("add closing } to end the interpolation"),
                    );
                    return Token::new(TokenKind::Error("unterminated interpolation".to_string()), span);
                }

                self.advance(); // consume }

                if var_name.is_empty() {
                    let span = self.span(start);
                    self.diagnostics.push(
                        Diagnostic::error("empty interpolation {} in string")
                            .with_span(span)
                            .with_help("provide a variable name inside {}"),
                    );
                    return Token::new(TokenKind::Error("empty interpolation".to_string()), span);
                }

                parts.push(StringPart::Variable(var_name));
            } else {
                current_text.push(ch);
                self.advance();
            }
        }

        if self.is_at_end() {
            let span = self.span(start);
            self.diagnostics.push(
                Diagnostic::error("unterminated string literal")
                    .with_span(span)
                    .with_help("add closing \" to end the string"),
            );
            return Token::new(TokenKind::Error("unterminated string".to_string()), span);
        }

        self.advance(); // consume closing "

        // Add final text part if any
        if !current_text.is_empty() {
            parts.push(StringPart::Text(current_text));
        }

        // Return simple string if no interpolation, otherwise interpolated
        if !has_interpolation {
            // Extract the single text part if exists, otherwise empty string
            let text = if parts.is_empty() {
                String::new()
            } else if let StringPart::Text(t) = &parts[0] {
                t.clone()
            } else {
                String::new()
            };
            Token::new(TokenKind::String(text), self.span(start))
        } else {
            Token::new(TokenKind::StringInterpolated(parts), self.span(start))
        }
    }

    /// Lex a char literal
    pub(crate) fn lex_char(&mut self, start: Position) -> Token {
        self.advance(); // consume opening '

        if self.is_at_end() {
            let span = self.span(start);
            self.diagnostics.push(
                Diagnostic::error("unterminated char literal".to_string())
                    .with_span(span),
            );
            return Token::new(TokenKind::Error("unterminated char".to_string()), span);
        }

        let ch = self.current_char();
        let char_value = if ch == '\\' {
            // Handle escape sequences
            self.advance(); // consume \
            if self.is_at_end() {
                let span = self.span(start);
                self.diagnostics.push(
                    Diagnostic::error("unterminated char literal".to_string())
                        .with_span(span),
                );
                return Token::new(TokenKind::Error("unterminated char".to_string()), span);
            }
            let escaped = match self.current_char() {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '\'' => '\'',
                '\\' => '\\',
                '0' => '\0',
                _ => {
                    let span = self.span(start);
                    self.diagnostics.push(
                        Diagnostic::error(format!("invalid escape sequence: '\\{}'", self.current_char()))
                            .with_span(span),
                    );
                    self.current_char()
                }
            };
            self.advance(); // consume escaped char
            escaped
        } else {
            self.advance(); // consume char
            ch
        };

        // Expect closing '
        if self.is_at_end() || self.current_char() != '\'' {
            let span = self.span(start);
            self.diagnostics.push(
                Diagnostic::error("expected closing ' for char literal".to_string())
                    .with_span(span),
            );
            return Token::new(TokenKind::Error("unterminated char".to_string()), span);
        }

        self.advance(); // consume closing '

        Token::new(TokenKind::Char(char_value), self.span(start))
    }

    /// Lex a number (integer or float).
    ///
    /// Accepts digits from any supported Unicode numeral system (see
    /// [`digit_blocks`]).  All digits within a single literal must belong to
    /// the same script; mixing scripts (e.g. `४2`) is a lex error.
    ///
    /// Digits are normalised to their ASCII equivalents before parsing so the
    /// rest of the pipeline sees plain `i64` / `f64` values regardless of
    /// script.  The decimal separator (`.`) and scientific-notation marker
    /// (`e`/`E`) are always ASCII.
    pub(crate) fn lex_number(&mut self, start: Position) -> Token {
        // Base prefixes (0b, 0o, 0d, 0x) are always ASCII — check before the
        // Unicode digit path so `0b101` etc. still work.
        if self.current_char() == '0' && !self.is_at_end() {
            if let Some(next_ch) = self.peek() {
                match next_ch {
                    'b' | 'o' | 'd' | 'x' => {
                        let peek_ahead = self.peek_ahead(2);
                        if peek_ahead == Some('|') {
                            // Base conversion expression: 0x|expr|
                            self.advance(); // consume '0'
                            self.advance(); // consume base char
                            let span = self.span(start);
                            let kind = match next_ch {
                                'b' => TokenKind::BaseBinary,
                                'o' => TokenKind::BaseOctal,
                                'd' => TokenKind::BaseDecimal,
                                'x' => TokenKind::BaseHex,
                                _ => unreachable!(),
                            };
                            return Token::new(kind, span);
                        } else {
                            // Base character literal: 0x41
                            let (radix, base_name) = match next_ch {
                                'b' => (2, "binary"),
                                'o' => (8, "octal"),
                                'd' => (10, "decimal"),
                                'x' => (16, "hexadecimal"),
                                _ => unreachable!(),
                            };
                            return self.lex_base_char_literal(start, radix, base_name);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Builds an ASCII-normalised string for parsing; tracks the active
        // block base to enforce single-script consistency.
        let mut number_str = String::new();
        let mut is_float = false;
        let mut active_block: Option<u32> = None;

        // ── integer part ──────────────────────────────────────────────────────
        while !self.is_at_end() {
            let ch = self.current_char();
            if let Some(d) = digit_value(ch) {
                let block = digit_block_base(ch).unwrap();
                if let Some(b) = active_block {
                    if b != block {
                        return self.mixed_script_error(start);
                    }
                } else {
                    active_block = Some(block);
                }
                number_str.push(char::from_u32('0' as u32 + d as u32).unwrap());
                self.advance();
            } else {
                break;
            }
        }

        // ── decimal point ─────────────────────────────────────────────────────
        // Require the char after '.' to be a recognised digit (any script) to
        // avoid mistaking the range operator `..` for a float separator.
        if !self.is_at_end() && self.current_char() == '.' {
            if let Some(next_ch) = self.peek() {
                if digit_value(next_ch).is_some() {
                    is_float = true;
                    number_str.push('.');
                    self.advance(); // consume '.'

                    while !self.is_at_end() {
                        let ch = self.current_char();
                        if let Some(d) = digit_value(ch) {
                            let block = digit_block_base(ch).unwrap();
                            if let Some(b) = active_block {
                                if b != block {
                                    return self.mixed_script_error(start);
                                }
                            } else {
                                active_block = Some(block);
                            }
                            number_str.push(char::from_u32('0' as u32 + d as u32).unwrap());
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }
            }
        }

        // ── scientific notation (always ASCII) ────────────────────────────────
        if !self.is_at_end() {
            let ch = self.current_char();
            if ch == 'e' || ch == 'E' {
                is_float = true;
                number_str.push(ch);
                self.advance();

                if !self.is_at_end() {
                    let sign_ch = self.current_char();
                    if sign_ch == '+' || sign_ch == '-' {
                        number_str.push(sign_ch);
                        self.advance();
                    }
                }

                while !self.is_at_end() {
                    let ch = self.current_char();
                    if ch.is_ascii_digit() {
                        number_str.push(ch);
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
        }

        // ── parse normalised ASCII string ─────────────────────────────────────
        if is_float {
            match number_str.parse::<f64>() {
                Ok(f) => Token::new(TokenKind::Float(f), self.span(start)),
                Err(_) => {
                    let span = self.span(start);
                    self.diagnostics.push(
                        Diagnostic::error(format!("invalid float literal: '{}'", number_str))
                            .with_span(span),
                    );
                    Token::new(TokenKind::Error(format!("invalid float: '{}'", number_str)), span)
                }
            }
        } else {
            match number_str.parse::<i64>() {
                Ok(n) => Token::new(TokenKind::Integer(n), self.span(start)),
                Err(_) => {
                    let span = self.span(start);
                    self.diagnostics.push(
                        Diagnostic::error(format!("invalid integer literal: '{}'", number_str))
                            .with_span(span),
                    );
                    Token::new(TokenKind::Error(format!("invalid integer: '{}'", number_str)), span)
                }
            }
        }
    }

    /// Emits a `MixedDigitScripts` diagnostic and returns an error token.
    fn mixed_script_error(&mut self, start: Position) -> Token {
        let span = self.span(start);
        self.diagnostics.push(
            Diagnostic::error("mixed digit scripts in numeric literal")
                .with_span(span)
                .with_help(
                    "all digits in a literal must belong to the same numeral system \
                     (e.g. all ASCII or all Devanagari)",
                ),
        );
        Token::new(TokenKind::Error("mixed digit scripts".to_string()), span)
    }

    /// Lex a base character literal (0x41, 0b01000001, 0o0101, 0d65)
    pub(crate) fn lex_base_char_literal(&mut self, start: Position, radix: u32, base_name: &str) -> Token {
        self.advance(); // consume '0'
        self.advance(); // consume base char ('b', 'o', 'd', or 'x')

        let mut digits = String::new();

        // Collect valid digits for the base
        while !self.is_at_end() {
            let ch = self.current_char();
            let is_valid_digit = match radix {
                2 => ch == '0' || ch == '1',
                8 => ('0'..='7').contains(&ch),
                10 => ch.is_ascii_digit(),
                16 => ch.is_ascii_hexdigit(),
                _ => false,
            };

            if is_valid_digit {
                digits.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        // Check if we got any digits
        if digits.is_empty() {
            let span = self.span(start);
            self.diagnostics.push(
                Diagnostic::error(format!("expected {} digits after base prefix", base_name))
                    .with_span(span),
            );
            return Token::new(
                TokenKind::Error(format!("invalid {} literal", base_name)),
                span,
            );
        }

        // Parse the digits as a number
        match u32::from_str_radix(&digits, radix) {
            Ok(code) => {
                // Convert code point to char
                match char::from_u32(code) {
                    Some(ch) => Token::new(TokenKind::Char(ch), self.span(start)),
                    None => {
                        let span = self.span(start);
                        self.diagnostics.push(
                            Diagnostic::error(format!(
                                "invalid Unicode code point: 0x{:X} ({} {})",
                                code, base_name, digits
                            ))
                            .with_span(span),
                        );
                        Token::new(
                            TokenKind::Error(format!("invalid code point: 0x{:X}", code)),
                            span,
                        )
                    }
                }
            }
            Err(_) => {
                let span = self.span(start);
                self.diagnostics.push(
                    Diagnostic::error(format!("invalid {} literal: {}", base_name, digits))
                        .with_span(span),
                );
                Token::new(
                    TokenKind::Error(format!("invalid {} literal", base_name)),
                    span,
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use zymbol_span::FileId;
    use crate::{Lexer, TokenKind};

    fn lex_first(src: &str) -> TokenKind {
        let (tokens, diags) = Lexer::new(src, FileId(0)).tokenize();
        assert!(diags.is_empty(), "unexpected lex errors: {:?}", diags);
        tokens[0].kind.clone()
    }

    fn lex_error(src: &str) -> bool {
        let (_, diags) = Lexer::new(src, FileId(0)).tokenize();
        !diags.is_empty()
    }

    // ── Integer normalization ─────────────────────────────────────────────────

    #[test]
    fn ascii_integer() {
        assert_eq!(lex_first("42"), TokenKind::Integer(42));
    }

    #[test]
    fn devanagari_integer() {
        assert_eq!(lex_first("४२"), TokenKind::Integer(42));
    }

    #[test]
    fn arabic_indic_integer() {
        // U+0660 = ٠, U+0664 = ٤, U+0662 = ٢ → 42
        assert_eq!(lex_first("٤٢"), TokenKind::Integer(42));
    }

    #[test]
    fn thai_integer() {
        // U+0E54 = ๔, U+0E52 = ๒ → 42
        assert_eq!(lex_first("๔๒"), TokenKind::Integer(42));
    }

    #[test]
    fn adlam_integer() {
        // U+1E954 = 𞥔, U+1E952 = 𞥒 → 42
        let four = char::from_u32(0x1E950 + 4).unwrap();
        let two  = char::from_u32(0x1E950 + 2).unwrap();
        let src: String = [four, two].iter().collect();
        assert_eq!(lex_first(&src), TokenKind::Integer(42));
    }

    #[test]
    fn zero_in_any_script() {
        // Devanagari zero
        assert_eq!(lex_first("०"), TokenKind::Integer(0));
    }

    // ── Float normalization ───────────────────────────────────────────────────

    #[test]
    fn devanagari_float() {
        // ३.१४ → 3.14
        assert_eq!(lex_first("३.१४"), TokenKind::Float(3.14));
    }

    #[test]
    fn ascii_float_unchanged() {
        assert_eq!(lex_first("3.14"), TokenKind::Float(3.14));
    }

    #[test]
    fn scientific_notation_unchanged() {
        assert_eq!(lex_first("1e10"), TokenKind::Float(1e10));
    }

    // ── Mixed-script error ────────────────────────────────────────────────────

    #[test]
    fn mixed_scripts_integer_is_error() {
        // ASCII '4' (U+0034) followed by Devanagari '२' (U+0968)
        assert!(lex_error("4२"));
    }

    #[test]
    fn mixed_scripts_float_fractional_is_error() {
        // Integer part ASCII, fractional part Devanagari
        assert!(lex_error("3.१४"));
    }

    #[test]
    fn homogeneous_devanagari_float_is_ok() {
        assert!(!lex_error("३.१४"));
    }
}
