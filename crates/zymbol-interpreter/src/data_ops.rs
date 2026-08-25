//! Data operation evaluation for Zymbol-Lang
//!
//! Handles runtime evaluation of data transformation and introspection:
//! - Numeric evaluation: #|expr| (safe string-to-number conversion)
//! - Type metadata: expr#? (type introspection tuple)
//! - Format expressions: e|expr| (scientific), c|expr| (comma-separated)
//! - Base conversions: 0x|expr|, 0b|expr|, 0o|expr|, 0d|expr| (char/int/text)
//! - Precision expressions: #.N|expr| (round), #!N|expr| (truncate)

use std::io::Write;
use zymbol_ast::{BaseConversionExpr, CastKind, FormatExpr, NumericCastExpr, NumericEvalExpr, RoundExpr, TruncExpr, TypeMetadataExpr};
use zymbol_common::num;

use crate::{Interpreter, Result, RuntimeError, Value};

fn value_type(v: Value) -> &'static str {
    match v {
        Value::Int(_) => "Int",
        Value::Float(_) => "Float",
        Value::String(_) => "String",
        Value::Char(_) => "Char",
        Value::Bool(_) => "Bool",
        Value::Array(_) => "Array",
        Value::Tuple(_) | Value::NamedTuple(_) => "Tuple",
        Value::Function(_) => "Function",
        Value::Error(_) => "Error",
        Value::Unit => "Unit",
    }
}

// The one normalizer, shared with the VM and with the lexer's own literal
// scanner. It used to be a hand-written copy in each engine; they agreed, which
// is exactly what makes that shape dangerous — nothing would have said so.
use zymbol_lexer::digit_blocks::ascii_number as normalize_unicode_digits;

/// The ASCII form of a numeric string written in any of the 69 supported digit
/// scripts: `"४२"` → `"42"`, `"42"` → `"42"` (borrowed, no allocation).
///
/// Every numeric cast normalizes through this first, so a number the program
/// rendered under an active numeral mode parses back exactly like its ASCII
/// twin — the round trip a multilingual application depends on. Strings that
/// are not numeric at all are handed back untouched for the caller to reject.
pub(crate) fn ascii_digits(s: &str) -> std::borrow::Cow<'_, str> {
    if s.is_ascii() {
        return std::borrow::Cow::Borrowed(s);
    }
    match normalize_unicode_digits(s) {
        Some(normalized) => std::borrow::Cow::Owned(normalized),
        None => std::borrow::Cow::Borrowed(s),
    }
}

/// Try to parse a trimmed string as `Int` or `Float`.
/// Returns the original `String` value if parsing fails (fail-safe).
/// Also handles Unicode digit scripts (Thai, Arabic, Devanagari, etc.).
pub(crate) fn parse_numeric_string(s: String) -> Value {
    // What the text says is `num::parse`'s call, shared with the VM so the two
    // engines cannot read the same string differently.
    fn numeric(t: &str) -> Option<Value> {
        match num::parse(t) {
            num::Num::Int(n) => Some(Value::Int(n)),
            num::Num::Float(f) => Some(Value::Float(f)),
            num::Num::None => None,
        }
    }
    let trimmed = s.trim();
    if let Some(v) = numeric(trimmed) {
        return v;
    }
    if let Some(normalized) = normalize_unicode_digits(trimmed) {
        if let Some(v) = numeric(&normalized) {
            return v;
        }
    }
    Value::String(s)
}

/// A float already rounded or truncated, as an integer.
///
/// `f as i64` in Rust *saturates*: `1e300` becomes `i64::MAX` and `nan` becomes
/// `0`, so the cast used to answer with a number the program never computed.
/// Out of range is a `##Range` error like any other integer overflow.
fn cast_to_int(f: f64, cast: &str, span: zymbol_span::Span) -> Result<Value> {
    match num::from_f64(f) {
        Some(n) => Ok(Value::Int(n)),
        None => Err(RuntimeError::Generic {
            // The offending float is deliberately not quoted: the engines still
            // print large floats differently (`1e+300` in the browser, 301
            // digits here), and an error message is the last place a parity gate
            // should have to tolerate that. The span already points at it.
            message: format!("integer overflow: {} cannot represent this float", cast),
            span,
        }),
    }
}

impl<W: Write> Interpreter<W> {
    pub(crate) fn eval_numeric_eval(&mut self, op: &NumericEvalExpr) -> Result<Value> {
        let value = self.eval_expr(&op.expr)?;
        match value {
            Value::String(s) => Ok(parse_numeric_string(s)),
            // GAP-ZYB-012: a Char reads like the one-character string it is.
            //
            // `#|"७"|` gave 7 and `#|'७'|` gave back the glyph — the same
            // operator, the same character, two answers depending on which of
            // the two ways it had been written. `#|'7'|` did not convert
            // either. `parse_numeric_string` already knows all 69 digit
            // scripts, so this is the same reading applied to the same
            // character, and a Char that is not a digit comes back untouched,
            // which is what "safe conversion" means for a string too.
            Value::Char(c) => Ok(match parse_numeric_string(c.to_string()) {
                Value::String(_) => Value::Char(c),
                converted => converted,
            }),
            other => Ok(other),
        }
    }

    /// Evaluate numeric cast: ##.expr / ###expr / ##!expr
    /// ##.  → Float (lossless from Int, identity from Float)
    /// ###  → Int rounding  (Float 3.7 → 4, Int identity)
    /// ##!  → Int truncating (Float 3.7 → 3, Int identity)
    pub(crate) fn eval_numeric_cast(&mut self, op: &NumericCastExpr) -> Result<Value> {
        let value = self.eval_expr(&op.expr)?;
        match op.kind {
            CastKind::ToFloat => match value {
                Value::Float(_) => Ok(value),
                Value::Int(n) => Ok(Value::Float(n as f64)),
                other => Err(RuntimeError::Generic {
                    message: format!("##. requires a numeric value, got {}", value_type(other)),
                    span: op.span,
                }),
            },
            CastKind::ToIntRound => match value {
                Value::Int(_) => Ok(value),
                Value::Float(f) => cast_to_int(f.round(), "###", op.span),
                other => Err(RuntimeError::Generic {
                    message: format!("### requires a numeric value, got {}", value_type(other)),
                    span: op.span,
                }),
            },
            CastKind::ToIntTrunc => match value {
                Value::Int(_) => Ok(value),
                Value::Float(f) => cast_to_int(f.trunc(), "##!", op.span),
                // A Char casts to its Unicode code point. This is the only
                // direct Char→Int route (the alternative was inverting a base
                // literal and stripping its prefix), and it makes characters
                // classifiable by range.
                Value::Char(c) => Ok(Value::Int(c as u32 as i64)),
                other => Err(RuntimeError::Generic {
                    message: format!("##! requires a numeric value or Char, got {}", value_type(other)),
                    span: op.span,
                }),
            },
        }
    }

    /// Evaluate type metadata expression: expr#?
    /// Returns tuple with (type_symbol, count, value)
    /// Type symbols are language-agnostic with ## prefix:
    /// ###, ##., ##", ##', ##?, ##], ##), ##_
    ///
    /// The operand is evaluated like any other expression.
    ///
    /// There used to be a special case here: an `Expr::Identifier` was looked up
    /// with `get_variable` and, when that found nothing, `#?` answered
    /// `("##_", 0, Unit)` instead of erroring — "so variable existence can be
    /// checked". That case is unreachable: an undefined name is refused by the
    /// semantic analyzer before anything runs, so nothing ever arrived at it.
    ///
    /// What it did reach was a **named function**, which is not a variable — it
    /// lives in the function table — so the lookup failed and `f#?` reported
    /// that a function was Unit, while `g = f` then `g#?` reported `##(), 2`.
    /// The browser engine, which has no such case, was right about both.
    /// A workaround for a situation that cannot happen, paid for with a false
    /// answer to one that does (GAP-ZYB-009 § 6, D-4).
    pub(crate) fn eval_type_metadata(&mut self, op: &TypeMetadataExpr) -> Result<Value> {
        let value = self.eval_expr(&op.expr)?;

        // The symbol comes from `type_symbol_of`, which every engine shares
        // through `zymbol_common::typesym`; only the count is decided here,
        // because what "how many" means differs per type.
        let type_symbol = crate::type_symbol_of(&value);
        let count: i64 = match &value {
            Value::Int(n) => n.to_string().len() as i64,
            Value::Float(f) => f.to_string().len() as i64,
            Value::String(s) => s.len() as i64,
            Value::Char(_) | Value::Bool(_) => 1,
            Value::Array(arr) => arr.len() as i64,
            Value::Tuple(tup) => tup.len() as i64,
            Value::NamedTuple(fields) => fields.len() as i64,
            Value::Function(func) => func.params.len() as i64,
            Value::Error(err) => err.message.len() as i64,
            Value::Unit => 0,
        };

        // Return tuple: (type_symbol, count, value)
        Ok(Value::Tuple(vec![
            Value::String(type_symbol),
            Value::Int(count),
            value,
        ]))
    }

    /// The decimal count a precision operator was given, evaluated if it was
    /// written as an expression (GAP-ZYB-001).
    fn eval_precision(&mut self, p: &zymbol_ast::Precision, span: zymbol_span::Span) -> Result<u32> {
        match p {
            zymbol_ast::Precision::Literal(n) => Ok(*n),
            zymbol_ast::Precision::Dynamic(expr) => match self.eval_expr(expr)? {
                Value::Int(n) if n >= 0 => Ok(n as u32),
                Value::Int(n) => Err(RuntimeError::Generic {
                    message: format!("decimal count must not be negative, got {}", n),
                    span,
                }),
                other => Err(RuntimeError::Generic {
                    message: format!(
                        "decimal count must be a whole number, got {}",
                        other.type_name()
                    ),
                    span,
                }),
            },
        }
    }

    fn eval_precision_op(
        &mut self,
        op: &Option<zymbol_ast::PrecisionOp>,
        span: zymbol_span::Span,
    ) -> Result<Option<zymbol_ast::ResolvedPrecision>> {
        use zymbol_ast::{PrecisionOp, ResolvedPrecision};
        Ok(match op {
            None => None,
            Some(PrecisionOp::Round(p)) => Some(ResolvedPrecision::Round(self.eval_precision(p, span)?)),
            Some(PrecisionOp::Truncate(p)) => {
                Some(ResolvedPrecision::Truncate(self.eval_precision(p, span)?))
            }
        })
    }

    /// Evaluate format expression: #,|expr| or #^|expr| with optional precision.
    /// Always returns a String value.
    pub(crate) fn eval_format(&mut self, op: &FormatExpr) -> Result<Value> {
        use zymbol_ast::FormatKind;

        let value = self.eval_expr(&op.expr)?;

        let f: f64 = match value {
            Value::Int(n) => n as f64,
            Value::Float(f) => f,
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!(
                        "format expressions only work with numbers, got {:?}",
                        value
                    ),
                    span: op.span,
                });
            }
        };

        let precision = self.eval_precision_op(&op.precision, op.span)?;
        let formatted = match op.kind {
            FormatKind::Thousands => interp_fmt_thousands(f, precision),
            FormatKind::Scientific => interp_fmt_scientific(f, precision),
        };

        // Digits *and* separators follow the active numeral mode, as `>>` and
        // the precision operators do. They did not: `#,` and `#^` build their
        // text with `format!`, which writes ASCII, so under `#०९#` a program
        // printed `१२३४५६७.८९` with `>>` and `1,234,567.89` one line later with
        // `#,` — two spellings of one number inside one output. The same in all
        // three engines, so no consensus run could see it.
        //
        // `#,` is the only operator in the language that emits a thousands
        // separator at all, because it is the only one whose result is text
        // rather than a number.
        Ok(Value::String(crate::numeral_mode::map_numeral_number(
            formatted,
            self.numeral_mode,
        )))
    }

    /// Evaluate base conversion expression: 0x|expr| or 0b|expr| or 0o|expr| or 0d|expr|
    /// Tridirectional conversion:
    /// - char → text: displays character code in specified base
    /// - int → char: creates char from numeric code
    /// - text → char: parses string as number in base, creates char
    pub(crate) fn eval_base_conversion(&mut self, op: &BaseConversionExpr) -> Result<Value> {
        use zymbol_ast::BasePrefix;

        let value = self.eval_expr(&op.expr)?;

        match value {
            // Case 1: char → text (display character code)
            Value::Char(ch) => {
                let code = ch as u32;
                let formatted = match op.prefix {
                    BasePrefix::Binary => format!("0b{:b}", code),
                    BasePrefix::Octal => format!("0o{:o}", code),
                    BasePrefix::Decimal => format!("0d{:04}", code),
                    BasePrefix::Hex => format!("0x{:04X}", code),
                };
                Ok(Value::String(formatted))
            }

            // Case 2: int → string (format integer in specified base)
            Value::Int(n) => {
                let formatted = match op.prefix {
                    BasePrefix::Binary => format!("0b{:b}", n),
                    BasePrefix::Octal => format!("0o{:o}", n),
                    BasePrefix::Decimal => format!("0d{:04}", n),
                    BasePrefix::Hex => format!("0x{:04X}", n),
                };
                Ok(Value::String(formatted))
            }

            // Case 3: text → char (parse string as number in base, create char)
            Value::String(s) => {
                // Remove base prefix if present
                let s = s.trim_start_matches("0b")
                    .trim_start_matches("0o")
                    .trim_start_matches("0d")
                    .trim_start_matches("0x")
                    .trim_start_matches("0B")
                    .trim_start_matches("0O")
                    .trim_start_matches("0D")
                    .trim_start_matches("0X");

                // Parse based on prefix
                let code = match op.prefix {
                    BasePrefix::Binary => u32::from_str_radix(s, 2),
                    BasePrefix::Octal => u32::from_str_radix(s, 8),
                    BasePrefix::Decimal => s.parse::<u32>(),
                    BasePrefix::Hex => u32::from_str_radix(s, 16),
                }
                .map_err(|_| RuntimeError::Generic {
                    message: format!(
                        "failed to parse '{}' as {} number",
                        s,
                        match op.prefix {
                            BasePrefix::Binary => "binary",
                            BasePrefix::Octal => "octal",
                            BasePrefix::Decimal => "decimal",
                            BasePrefix::Hex => "hexadecimal",
                        }
                    ),
                    span: op.span,
                })?;

                if code > 0x10FFFF {
                    return Err(RuntimeError::Generic {
                        message: format!(
                            "character code must be in range 0..0x10FFFF, got {}",
                            code
                        ),
                        span: op.span,
                    });
                }

                let ch = char::from_u32(code).ok_or_else(|| RuntimeError::Generic {
                    message: format!("invalid Unicode character code: {}", code),
                    span: op.span,
                })?;

                Ok(Value::Char(ch))
            }

            _ => Err(RuntimeError::Generic {
                message: format!(
                    "base conversion expressions work with char, int, or string, got {:?}",
                    value
                ),
                span: op.span,
            }),
        }
    }

    /// Evaluate round expression: #.N|expr|
    /// Rounds the result to N decimal places using standard mathematical rounding.
    /// - #.2|19.876| → 19.88 (0.006 rounds up)
    /// - #.2|19.874| → 19.87 (0.004 rounds down)
    /// - #.0|19.5| → 20.0 (rounds to nearest integer)
    /// - #.2|"19.876"| → 19.88 (strings are auto-converted to numbers)
    ///
    /// Always returns Float.
    pub(crate) fn eval_round(&mut self, op: &RoundExpr) -> Result<Value> {
        let value = self.eval_expr(&op.expr)?;

        // Extract numeric value (with auto-conversion from string)
        let float_val = match value {
            Value::Int(n) => n as f64,
            Value::Float(f) => f,
            Value::String(s) => {
                // Try to parse string as number (like #|expr|) — digits in any
                // script, so a value rendered under an active numeral mode and
                // folded back into a string still rounds.
                let normalized = ascii_digits(s.trim());
                if let Ok(n) = normalized.parse::<i64>() {
                    n as f64
                } else if let Ok(f) = normalized.parse::<f64>() {
                    f
                } else {
                    return Err(RuntimeError::Generic {
                        message: format!(
                            "cannot convert string '{}' to number for rounding",
                            s
                        ),
                        span: op.span,
                    });
                }
            }
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!(
                        "round expressions only work with numbers or numeric strings, got {:?}",
                        value
                    ),
                    span: op.span,
                });
            }
        };

        // Round to N decimal places
        let digits = self.eval_precision(&op.precision, op.span)?;
        let multiplier = 10_f64.powi(digits as i32);
        let rounded = (float_val * multiplier).round() / multiplier;

        Ok(Value::Float(rounded))
    }

    /// Evaluate truncate expression: #!N|expr|
    /// Truncates the result to N decimal places (cuts without rounding).
    /// - #!2|19.879| → 19.87 (simply cuts)
    /// - #!0|19.9| → 19.0 (truncates to integer)
    /// - #!2|"19.879"| → 19.87 (strings are auto-converted to numbers)
    ///
    /// Always returns Float.
    pub(crate) fn eval_trunc(&mut self, op: &TruncExpr) -> Result<Value> {
        let value = self.eval_expr(&op.expr)?;

        // Extract numeric value (with auto-conversion from string)
        let float_val = match value {
            Value::Int(n) => n as f64,
            Value::Float(f) => f,
            Value::String(s) => {
                // Try to parse string as number (like #|expr|) — digits in any script.
                let normalized = ascii_digits(s.trim());
                if let Ok(n) = normalized.parse::<i64>() {
                    n as f64
                } else if let Ok(f) = normalized.parse::<f64>() {
                    f
                } else {
                    return Err(RuntimeError::Generic {
                        message: format!(
                            "cannot convert string '{}' to number for truncation",
                            s
                        ),
                        span: op.span,
                    });
                }
            }
            _ => {
                return Err(RuntimeError::Generic {
                    message: format!(
                        "truncate expressions only work with numbers or numeric strings, got {:?}",
                        value
                    ),
                    span: op.span,
                });
            }
        };

        // Truncate to N decimal places
        let digits = self.eval_precision(&op.precision, op.span)?;
        let multiplier = 10_f64.powi(digits as i32);
        let truncated = (float_val * multiplier).trunc() / multiplier;

        Ok(Value::Float(truncated))
    }
}

// ── Format helpers (free functions) ──────────────────────────────────────────

/// Format number with thousands separators and optional precision.
fn interp_fmt_thousands(num: f64, precision: Option<zymbol_ast::ResolvedPrecision>) -> String {
    use zymbol_ast::ResolvedPrecision as PrecisionOp;

    // Apply precision first
    let num = match precision {
        Some(PrecisionOp::Round(n)) => {
            let m = 10f64.powi(n as i32);
            (num * m).round() / m
        }
        Some(PrecisionOp::Truncate(n)) => {
            let m = 10f64.powi(n as i32);
            (num * m).trunc() / m
        }
        None => num,
    };

    let neg = num < 0.0;
    let abs_f = num.abs();
    let int_part = abs_f.floor() as i64;

    // Format integer part with commas (sign handled separately)
    let int_s = {
        let digits = format!("{}", int_part);
        let mut out = String::with_capacity(digits.len() + digits.len() / 3);
        for (i, c) in digits.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 { out.push(','); }
            out.push(c);
        }
        out.chars().rev().collect::<String>()
    };
    let mut s = int_s;

    // Append fractional part
    match precision {
        None => {
            // No precision: derive fractional part from float's natural representation
            let full_s = format!("{}", abs_f);
            if let Some(dot_pos) = full_s.find('.') {
                s.push_str(&full_s[dot_pos..]);
            }
        }
        Some(PrecisionOp::Round(n)) | Some(PrecisionOp::Truncate(n)) => {
            if n > 0 {
                let frac = abs_f - int_part as f64;
                let frac_s = format!("{:.prec$}", frac, prec = n as usize);
                if let Some(dot_pos) = frac_s.find('.') {
                    s.push_str(&frac_s[dot_pos..]);
                }
            }
            // n == 0: no decimal part
        }
    }

    if neg { s.insert(0, '-'); }
    s
}

/// Format number in scientific notation with optional precision.
fn interp_fmt_scientific(num: f64, precision: Option<zymbol_ast::ResolvedPrecision>) -> String {
    use zymbol_ast::ResolvedPrecision as PrecisionOp;
    match precision {
        None => format!("{:e}", num),
        Some(PrecisionOp::Round(n)) => format!("{:.prec$e}", num, prec = n as usize),
        Some(PrecisionOp::Truncate(n)) => interp_fmt_scientific_truncate(num, n),
    }
}

/// Scientific notation with truncation (no rounding) of mantissa decimal places.
fn interp_fmt_scientific_truncate(num: f64, n: u32) -> String {
    if num == 0.0 {
        if n == 0 { return "0e0".to_string(); }
        return format!("{:.prec$e}", 0.0f64, prec = n as usize);
    }
    let exp = num.abs().log10().floor() as i32;
    let mantissa = num / 10f64.powi(exp);
    let m = 10f64.powi(n as i32);
    let truncated = (mantissa * m).trunc() / m;
    if n == 0 {
        format!("{}e{}", truncated as i64, exp)
    } else {
        format!("{:.prec$}e{}", truncated, exp, prec = n as usize)
    }
}

#[cfg(test)]
mod tests {
    use crate::Interpreter;
    use zymbol_lexer::Lexer;
    use zymbol_parser::Parser;
    use zymbol_span::FileId;

    fn run(source: &str) -> String {
        let mut output = Vec::new();

        // Lex
        let lexer = Lexer::new(source, FileId(0));
        let (tokens, lex_diagnostics) = lexer.tokenize();
        assert!(lex_diagnostics.is_empty(), "Lexer errors: {:?}", lex_diagnostics);

        // Parse
        let parser = Parser::new(tokens);
        let program = parser.parse().expect("Parse error");

        // Execute
        let mut interpreter = Interpreter::with_output(&mut output);
        interpreter.execute(&program).expect("Runtime error");

        String::from_utf8(output).expect("Invalid UTF-8")
    }

    #[test]
    fn test_numeric_eval_string_to_int() {
        let code = r#"
x = #|"123"|
>> x ¶
"#;
        let output = run(code);
        assert_eq!(output, "123\n");
    }

    #[test]
    fn test_numeric_eval_string_to_float() {
        let code = r#"
x = #|"3.14"|
>> x ¶
"#;
        let output = run(code);
        assert_eq!(output, "3.14\n");
    }

    #[test]
    fn test_numeric_eval_fail_safe() {
        let code = r#"
x = #|"abc"|
>> x ¶
"#;
        let output = run(code);
        assert_eq!(output, "abc\n");
    }

    #[test]
    fn test_numeric_eval_non_string() {
        let code = r#"
x = 456
y = #|x|
>> y ¶
"#;
        let output = run(code);
        assert_eq!(output, "456\n");
    }

    #[test]
    fn test_type_metadata_int() {
        let code = r#"
x = 456
info = x#?
>> info[1] ¶
>> info[2] ¶
>> info[3] ¶
"#;
        let output = run(code);
        assert_eq!(output, "###\n3\n456\n");
    }

    #[test]
    fn test_type_metadata_string() {
        let code = r#"
x = "Hello"
info = x#?
>> info[1] ¶
>> info[2] ¶
>> info[3] ¶
"#;
        let output = run(code);
        assert_eq!(output, "##\"\n5\nHello\n");
    }

    #[test]
    fn test_type_metadata_array() {
        let code = r#"
x = [1, 2, 3, 4, 5]
info = x#?
>> info[1] ¶
>> info[2] ¶
"#;
        let output = run(code);
        assert_eq!(output, "##]\n5\n");
    }

    #[test]
    fn test_type_metadata_bool() {
        let code = r#"
x = #1
info = x#?
>> info[1] ¶
>> info[2] ¶
>> info[3] ¶
"#;
        let output = run(code);
        // info[3] is the bool value; >> renders it with # prefix to distinguish from integer
        assert_eq!(output, "##?\n1\n#1\n");
    }

    #[test]
    fn test_format_scientific_integer() {
        let code = r#"
x = 1500000
>> #^|x| ¶
"#;
        let output = run(code);
        assert_eq!(output, "1.5e6\n");
    }

    #[test]
    fn test_format_scientific_float() {
        let code = r#"
x = 3.14159
>> #^|x| ¶
"#;
        let output = run(code);
        assert!(output.starts_with("3.14159e"));
    }

    #[test]
    fn test_format_scientific_literal() {
        let code = r#"
>> #^|300000000| ¶
"#;
        let output = run(code);
        assert_eq!(output, "3e8\n");
    }

    #[test]
    fn test_format_comma_integer() {
        let code = r#"
x = 1500000
>> #,|x| ¶
"#;
        let output = run(code);
        assert_eq!(output, "1,500,000\n");
    }

    #[test]
    fn test_format_comma_float() {
        let code = r#"
x = 12345.67
>> #,|x| ¶
"#;
        let output = run(code);
        assert_eq!(output, "12,345.67\n");
    }

    #[test]
    fn test_format_comma_literal() {
        let code = r#"
>> #,|1000000| ¶
"#;
        let output = run(code);
        assert_eq!(output, "1,000,000\n");
    }

    #[test]
    fn test_format_thousands_with_precision_round() {
        let code = r#"
>> #,.2|123456.789| ¶
"#;
        let output = run(code);
        assert_eq!(output, "123,456.79\n");
    }

    #[test]
    fn test_format_thousands_with_precision_truncate() {
        let code = r#"
>> #,!2|123456.789| ¶
"#;
        let output = run(code);
        assert_eq!(output, "123,456.78\n");
    }

    #[test]
    fn test_format_scientific_with_precision_round() {
        let code = r#"
>> #^.2|12345.678| ¶
"#;
        let output = run(code);
        assert_eq!(output, "1.23e4\n");
    }

    #[test]
    fn test_format_scientific_with_precision_truncate() {
        let code = r#"
>> #^!2|12345.678| ¶
"#;
        let output = run(code);
        assert_eq!(output, "1.23e4\n");
    }

    #[test]
    fn test_format_with_expression() {
        let code = r#"
a = 1000000
b = 500000
>> #^|a + b| ¶
"#;
        let output = run(code);
        assert_eq!(output, "1.5e6\n");
    }

    #[test]
    fn test_combined_numeric_eval_and_type_metadata() {
        let code = r#"
x = "789"
info = #|x|#?
>> info[1] ¶
>> info[2] ¶
>> info[3] ¶
"#;
        let output = run(code);
        assert_eq!(output, "###\n3\n789\n");
    }

    #[test]
    fn test_combined_fail_safe_numeric_eval_and_metadata() {
        let code = r#"
x = "notanumber"
result = #|x|
info = result#?
>> info[1] ¶
>> info[3] ¶
"#;
        let output = run(code);
        assert_eq!(output, "##\"\nnotanumber\n");
    }

    // ===== Round Expression Tests =====

    #[test]
    fn test_round_basic() {
        let code = r#"
x = #.2|19.876|
>> x ¶
"#;
        let output = run(code);
        assert_eq!(output, "19.88\n");
    }

    #[test]
    fn test_round_down() {
        let code = r#"
x = #.2|19.874|
>> x ¶
"#;
        let output = run(code);
        assert_eq!(output, "19.87\n");
    }

    #[test]
    fn test_round_integer() {
        let code = r#"
x = #.0|19.5|
>> x ¶
"#;
        let output = run(code);
        assert_eq!(output, "20\n");
    }

    #[test]
    fn test_round_with_expression() {
        let code = r#"
precio = 19.876543
cantidad = 4
total = #.2|precio * cantidad|
>> total ¶
"#;
        let output = run(code);
        assert_eq!(output, "79.51\n");
    }

    #[test]
    fn test_round_from_int() {
        let code = r#"
x = #.2|10|
>> x ¶
"#;
        let output = run(code);
        assert_eq!(output, "10\n");
    }

    // ===== Truncate Expression Tests =====

    #[test]
    fn test_trunc_basic() {
        let code = r#"
x = #!2|19.879|
>> x ¶
"#;
        let output = run(code);
        assert_eq!(output, "19.87\n");
    }

    #[test]
    fn test_trunc_integer() {
        let code = r#"
x = #!0|19.9|
>> x ¶
"#;
        let output = run(code);
        assert_eq!(output, "19\n");
    }

    #[test]
    fn test_trunc_with_expression() {
        let code = r#"
precio = 19.876543
cantidad = 4
total = #!2|precio * cantidad|
>> total ¶
"#;
        let output = run(code);
        assert_eq!(output, "79.5\n");
    }

    #[test]
    fn test_trunc_vs_round() {
        // Demonstrate the difference between truncate and round
        let code = r#"
x = 19.999
round_result = #.2|x|
trunc_result = #!2|x|
>> round_result ¶
>> trunc_result ¶
"#;
        let output = run(code);
        assert_eq!(output, "20\n19.99\n");
    }

    #[test]
    fn test_precision_multiple_decimals() {
        let code = r#"
x = 3.141592653
>> #.1|x| ¶
>> #.2|x| ¶
>> #.3|x| ¶
>> #.4|x| ¶
"#;
        let output = run(code);
        assert_eq!(output, "3.1\n3.14\n3.142\n3.1416\n");
    }

    // ===== String Auto-Conversion Tests =====

    #[test]
    fn test_round_from_string() {
        let code = r#"
texto = "1234.5678"
x = #.2|texto|
>> x ¶
"#;
        let output = run(code);
        assert_eq!(output, "1234.57\n");
    }

    #[test]
    fn test_trunc_from_string() {
        let code = r#"
texto = "1234.5678"
x = #!2|texto|
>> x ¶
"#;
        let output = run(code);
        assert_eq!(output, "1234.56\n");
    }

    #[test]
    fn test_round_from_integer_string() {
        let code = r#"
texto = "42"
x = #.2|texto|
>> x ¶
"#;
        let output = run(code);
        assert_eq!(output, "42\n");
    }

    #[test]
    fn test_trunc_from_integer_string() {
        let code = r#"
texto = "42"
x = #!2|texto|
>> x ¶
"#;
        let output = run(code);
        assert_eq!(output, "42\n");
    }
}
