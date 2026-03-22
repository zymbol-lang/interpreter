//! Data operation evaluation for Zymbol-Lang
//!
//! Handles runtime evaluation of data transformation and introspection:
//! - Numeric evaluation: #|expr| (safe string-to-number conversion)
//! - Type metadata: expr#? (type introspection tuple)
//! - Format expressions: e|expr| (scientific), c|expr| (comma-separated)
//! - Base conversions: 0x|expr|, 0b|expr|, 0o|expr|, 0d|expr| (char/int/text)
//! - Precision expressions: #.N|expr| (round), #!N|expr| (truncate)

use std::io::Write;
use zymbol_ast::{BaseConversionExpr, Expr, FormatExpr, NumericEvalExpr, RoundExpr, TruncExpr, TypeMetadataExpr};

use crate::{Interpreter, Result, RuntimeError, Value};

impl<W: Write> Interpreter<W> {
    pub(crate) fn eval_numeric_eval(&mut self, op: &NumericEvalExpr) -> Result<Value> {
        let value = self.eval_expr(&op.expr)?;

        // If it's a string, try to parse as number
        if let Value::String(s) = value {
            // Trim whitespace/newlines first — BashExec output always has trailing \n
            let trimmed = s.trim();
            // Try parsing as integer first
            if let Ok(n) = trimmed.parse::<i64>() {
                return Ok(Value::Int(n));
            }
            // Try parsing as float
            if let Ok(f) = trimmed.parse::<f64>() {
                return Ok(Value::Float(f));
            }
            // Parsing failed - return original string (fail-safe!)
            Ok(Value::String(s))
        } else {
            // Not a string - return as-is
            Ok(value)
        }
    }

    /// Evaluate type metadata expression: expr#?
    /// Returns tuple with (type_symbol, count, value)
    /// Type symbols are language-agnostic with ## prefix:
    /// ###, ##., ##", ##', ##?, ##], ##), ##_
    ///
    /// SAFE ACCESS: If expr is an undefined variable, returns ("##_", 0, Unit)
    /// instead of throwing an error. This allows checking variable existence.
    pub(crate) fn eval_type_metadata(&mut self, op: &TypeMetadataExpr) -> Result<Value> {
        // Special handling for identifiers - check if variable exists
        let value = if let Expr::Identifier(ident) = &*op.expr {
            // Try to get variable safely
            match self.get_variable(&ident.name) {
                Some(v) => v.clone(),
                None => {
                    // Variable undefined - return Unit metadata without error
                    return Ok(Value::Tuple(vec![
                        Value::String("##_".to_string()),  // Unit type symbol
                        Value::Int(0),                      // Count: 0
                        Value::Unit,                        // Value: Unit
                    ]));
                }
            }
        } else {
            // For other expressions, evaluate normally (can still error)
            self.eval_expr(&op.expr)?
        };

        let (type_symbol, count) = match &value {
            Value::Int(n) => {
                let count = n.to_string().len() as i64;
                ("###".to_string(), count)
            }
            Value::Float(f) => {
                let count = f.to_string().len() as i64;
                ("##.".to_string(), count)
            }
            Value::String(s) => {
                let count = s.len() as i64;
                ("##\"".to_string(), count)
            }
            Value::Char(_) => ("##'".to_string(), 1),
            Value::Bool(_) => ("##?".to_string(), 1),
            Value::Array(arr) => {
                let count = arr.len() as i64;
                ("##]".to_string(), count)
            }
            Value::Tuple(tup) => {
                let count = tup.len() as i64;
                ("##)".to_string(), count)
            }
            Value::NamedTuple(fields) => {
                let count = fields.len() as i64;
                ("##)".to_string(), count)  // Same symbol as positional tuples
            }
            Value::Function(func) => {
                let count = func.params.len() as i64;
                ("##->".to_string(), count)  // Function type with parameter count
            }
            Value::Error(err) => {
                let count = err.message.len() as i64;
                (format!("##{}", err.error_type), count)  // Error type symbol
            }
            Value::Unit => ("##_".to_string(), 0),
        };

        // Return tuple: (type_symbol, count, value)
        Ok(Value::Tuple(vec![
            Value::String(type_symbol),
            Value::Int(count),
            value,
        ]))
    }

    /// Evaluate format expression: e|expr| or c|expr|
    /// Formats the result of expression evaluation for display
    pub(crate) fn eval_format(&mut self, op: &FormatExpr) -> Result<Value> {
        use zymbol_ast::FormatPrefix;

        let value = self.eval_expr(&op.expr)?;

        // Extract numeric value
        let (int_val, float_val) = match value {
            Value::Int(n) => (Some(n), Some(n as f64)),
            Value::Float(f) => (None, Some(f)),
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

        // Format based on prefix
        let formatted = match op.prefix {
            FormatPrefix::Scientific => {
                // Scientific notation: e|1500000| → "1.5e6"
                let f = float_val.unwrap();
                format!("{:e}", f)
            }
            FormatPrefix::Comma => {
                // Comma-separated thousands: c|1500000| → "1,500,000"
                if let Some(i) = int_val {
                    // Integer formatting
                    let s = i.to_string();
                    let is_negative = s.starts_with('-');
                    let digits = if is_negative { &s[1..] } else { &s };

                    let mut result = String::new();
                    for (idx, ch) in digits.chars().rev().enumerate() {
                        if idx > 0 && idx % 3 == 0 {
                            result.push(',');
                        }
                        result.push(ch);
                    }

                    let formatted: String = result.chars().rev().collect();
                    if is_negative {
                        format!("-{}", formatted)
                    } else {
                        formatted
                    }
                } else {
                    // Float formatting with commas
                    let f = float_val.unwrap();
                    let s = f.to_string();
                    let parts: Vec<&str> = s.split('.').collect();
                    let int_part = parts[0];
                    let is_negative = int_part.starts_with('-');
                    let digits = if is_negative { &int_part[1..] } else { int_part };

                    let mut result = String::new();
                    for (idx, ch) in digits.chars().rev().enumerate() {
                        if idx > 0 && idx % 3 == 0 {
                            result.push(',');
                        }
                        result.push(ch);
                    }

                    let formatted_int: String = result.chars().rev().collect();
                    let formatted_int = if is_negative {
                        format!("-{}", formatted_int)
                    } else {
                        formatted_int
                    };

                    if parts.len() > 1 {
                        format!("{}.{}", formatted_int, parts[1])
                    } else {
                        formatted_int
                    }
                }
            }
        };

        // Return as string
        Ok(Value::String(formatted))
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

            // Case 2: int → char (create char from code)
            Value::Int(code) => {
                if !(0..=0x10FFFF).contains(&code) {
                    return Err(RuntimeError::Generic {
                        message: format!(
                            "character code must be in range 0..0x10FFFF, got {}",
                            code
                        ),
                        span: op.span,
                    });
                }

                let ch = char::from_u32(code as u32).ok_or_else(|| RuntimeError::Generic {
                    message: format!("invalid Unicode character code: {}", code),
                    span: op.span,
                })?;

                Ok(Value::Char(ch))
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
                // Try to parse string as number (like #|expr|)
                if let Ok(n) = s.parse::<i64>() {
                    n as f64
                } else if let Ok(f) = s.parse::<f64>() {
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
        let multiplier = 10_f64.powi(op.precision as i32);
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
                // Try to parse string as number (like #|expr|)
                if let Ok(n) = s.parse::<i64>() {
                    n as f64
                } else if let Ok(f) = s.parse::<f64>() {
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
        let multiplier = 10_f64.powi(op.precision as i32);
        let truncated = (float_val * multiplier).trunc() / multiplier;

        Ok(Value::Float(truncated))
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
>> info[0] ¶
>> info[1] ¶
>> info[2] ¶
"#;
        let output = run(code);
        assert_eq!(output, "###\n3\n456\n");
    }

    #[test]
    fn test_type_metadata_string() {
        let code = r#"
x = "Hello"
info = x#?
>> info[0] ¶
>> info[1] ¶
>> info[2] ¶
"#;
        let output = run(code);
        assert_eq!(output, "##\"\n5\nHello\n");
    }

    #[test]
    fn test_type_metadata_array() {
        let code = r#"
x = [1, 2, 3, 4, 5]
info = x#?
>> info[0] ¶
>> info[1] ¶
"#;
        let output = run(code);
        assert_eq!(output, "##]\n5\n");
    }

    #[test]
    fn test_type_metadata_bool() {
        let code = r#"
x = #1
info = x#?
>> info[0] ¶
>> info[1] ¶
>> info[2] ¶
"#;
        let output = run(code);
        assert_eq!(output, "##?\n1\n#1\n");
    }

    #[test]
    fn test_format_scientific_integer() {
        let code = r#"
x = 1500000
>> e|x| ¶
"#;
        let output = run(code);
        assert_eq!(output, "1.5e6\n");
    }

    #[test]
    fn test_format_scientific_float() {
        let code = r#"
x = 3.14159
>> e|x| ¶
"#;
        let output = run(code);
        // Float in scientific notation
        assert!(output.starts_with("3.14159e"));
    }

    #[test]
    fn test_format_scientific_literal() {
        let code = r#"
>> e|300000000| ¶
"#;
        let output = run(code);
        assert_eq!(output, "3e8\n");
    }

    #[test]
    fn test_format_comma_integer() {
        let code = r#"
x = 1500000
>> c|x| ¶
"#;
        let output = run(code);
        assert_eq!(output, "1,500,000\n");
    }

    #[test]
    fn test_format_comma_float() {
        let code = r#"
x = 12345.67
>> c|x| ¶
"#;
        let output = run(code);
        assert_eq!(output, "12,345.67\n");
    }

    #[test]
    fn test_format_comma_literal() {
        let code = r#"
>> c|1000000| ¶
"#;
        let output = run(code);
        assert_eq!(output, "1,000,000\n");
    }

    #[test]
    fn test_format_scientific_uppercase() {
        let code = r#"
>> E|1500000| ¶
"#;
        let output = run(code);
        assert_eq!(output, "1.5e6\n");
    }

    #[test]
    fn test_format_comma_uppercase() {
        let code = r#"
>> C|1500000| ¶
"#;
        let output = run(code);
        assert_eq!(output, "1,500,000\n");
    }

    #[test]
    fn test_format_with_expression() {
        let code = r#"
a = 1000000
b = 500000
>> e|a + b| ¶
"#;
        let output = run(code);
        assert_eq!(output, "1.5e6\n");
    }

    #[test]
    fn test_combined_numeric_eval_and_type_metadata() {
        let code = r#"
x = "789"
info = #|x|#?
>> info[0] ¶
>> info[1] ¶
>> info[2] ¶
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
>> info[0] ¶
>> info[2] ¶
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
