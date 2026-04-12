//! Collection evaluation for Zymbol-Lang
//!
//! Handles runtime evaluation of collection expressions:
//! - Array literals: [expr1, expr2, ...]
//! - Tuples: (expr1, expr2, ...)
//! - Named tuples: (name: value, name2: value2)

use zymbol_ast::{ArrayLiteralExpr, NamedTupleExpr, TupleExpr};
use crate::{Interpreter, Result, Value};
use std::io::Write;

impl<W: Write> Interpreter<W> {
    /// Evaluate array literal expression: [expr1, expr2, ...]
    pub(crate) fn eval_array_literal(&mut self, arr: &ArrayLiteralExpr) -> Result<Value> {
        let mut elements = Vec::new();
        for expr in &arr.elements {
            elements.push(self.eval_expr(expr)?);
        }
        Ok(Value::Array(elements))
    }

    /// Evaluate tuple expression: (expr1, expr2, ...)
    pub(crate) fn eval_tuple(&mut self, tuple: &TupleExpr) -> Result<Value> {
        let mut elements = Vec::new();
        for expr in &tuple.elements {
            elements.push(self.eval_expr(expr)?);
        }
        Ok(Value::Tuple(elements))
    }

    /// Evaluate named tuple expression: (name: expr, name2: expr2, ...)
    pub(crate) fn eval_named_tuple(&mut self, named_tuple: &NamedTupleExpr) -> Result<Value> {
        let mut fields = Vec::new();
        for (name, expr) in &named_tuple.fields {
            let value = self.eval_expr(expr)?;
            fields.push((name.clone(), value));
        }
        Ok(Value::NamedTuple(fields))
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
    fn test_tuple_basic() {
        let code = r#"
point = (10, 20)
>> point ¶
"#;
        let output = run(code);
        assert_eq!(output, "(10, 20)\n");
    }

    #[test]
    fn test_tuple_three_elements() {
        let code = r#"
person = ("Alice", 25, #1)
>> person ¶
"#;
        let output = run(code);
        assert_eq!(output, "(Alice, 25, #1)\n");
    }

    #[test]
    fn test_tuple_mixed_types() {
        let code = r#"
mixed = (42, "hello", 'X', #0)
>> mixed ¶
"#;
        let output = run(code);
        assert_eq!(output, "(42, hello, X, #0)\n");
    }

    #[test]
    fn test_grouping_vs_tuple() {
        let code = r#"
grouped = (5 + 3) * 2
tuple = (8, 2)
>> grouped ¶
>> tuple ¶
"#;
        let output = run(code);
        assert_eq!(output, "16\n(8, 2)\n");
    }

    #[test]
    fn test_tuple_nested() {
        let code = r#"
nested = ((1, 2), (3, 4))
>> nested ¶
"#;
        let output = run(code);
        assert_eq!(output, "((1, 2), (3, 4))\n");
    }

    #[test]
    fn test_tuple_in_array() {
        let code = r#"
points = [(0, 0), (10, 20), (30, 40)]
>> points[1] ¶
>> points[2] ¶
"#;
        let output = run(code);
        assert_eq!(output, "(0, 0)\n(10, 20)\n");
    }

    #[test]
    fn test_array_in_tuple() {
        let code = r#"
data = ([1, 2, 3], "numbers")
>> data ¶
"#;
        let output = run(code);
        assert_eq!(output, "([1, 2, 3], numbers)\n");
    }

    #[test]
    fn test_tuple_equality() {
        let code = r#"
t1 = (1, 2, 3)
t2 = (1, 2, 3)
t3 = (1, 2, 4)
? t1 == t2 {
    >> "equal" ¶
}
? t1 == t3 {
    >> "should not print" ¶
}
_ {
    >> "not equal" ¶
}
"#;
        let output = run(code);
        assert_eq!(output, "equal\nnot equal\n");
    }

    #[test]
    fn test_tuple_with_expressions() {
        let code = r#"
x = 10
y = 20
calc = (x + y, x * y, x - y)
>> calc ¶
"#;
        let output = run(code);
        assert_eq!(output, "(30, 200, -10)\n");
    }

    #[test]
    fn test_single_element_is_not_tuple() {
        let code = r#"
single = (42)
>> single ¶
"#;
        let output = run(code);
        assert_eq!(output, "42\n");
    }

    #[test]
    fn test_tuple_complex_nested() {
        let code = r#"
complex = (("A", 1), ("B", 2), ("C", 3))
>> complex ¶
"#;
        let output = run(code);
        assert_eq!(output, "((A, 1), (B, 2), (C, 3))\n");
    }
}
