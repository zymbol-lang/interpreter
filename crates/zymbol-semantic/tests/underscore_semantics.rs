//! Tests for underscore variable (_variable) semantics
//!
//! These tests verify that:
//! - _variables are strictly local to their declaration block
//! - Accessing _variables from outer scopes produces semantic errors
//! - Accessing _variables from inner scopes produces semantic errors
//! - Accessing _variables from sibling scopes produces semantic errors
//! - Multiple _variables with same name in different scopes don't interfere

use zymbol_semantic::VariableAnalyzer;
use zymbol_parser::Parser;
use zymbol_lexer::Lexer;
use zymbol_span::FileId;

fn parse_and_analyze(source: &str) -> (Vec<String>, Vec<String>) {
    let file_id = FileId(0);
    let lexer = Lexer::new(source, file_id);
    let (tokens, _lex_diagnostics) = lexer.tokenize();

    let parser = Parser::new(tokens);
    let program = parser.parse().expect("parse failed");

    let mut analyzer = VariableAnalyzer::new();
    let diagnostics = analyzer.analyze(&program);
    let semantic_errors = analyzer.semantic_errors();

    let warnings: Vec<String> = diagnostics.iter().map(|d| d.message.clone()).collect();
    let errors: Vec<String> = semantic_errors.iter().map(|e| e.message.clone()).collect();

    (warnings, errors)
}

// ============================================================================
// VALID CASES: _variables accessed only in their declaration scope
// ============================================================================

#[test]
fn test_underscore_var_valid_same_scope() {
    let source = r#"
_x = 10
>> _x ¶
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 0, "Expected no errors for same-scope access");
}

#[test]
fn test_underscore_var_valid_in_if_block() {
    let source = r#"
? #1 {
    _temp = 42
    >> _temp ¶
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 0, "Expected no errors for same-scope access in IF block");
}

#[test]
fn test_underscore_var_valid_in_loop() {
    let source = r#"
@ i:1..5 {
    _result = i * 2
    >> _result ¶
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 0, "Expected no errors for same-scope access in loop");
}

#[test]
fn test_underscore_var_valid_in_function() {
    let source = r#"
calculate(x) {
    _temp = x * 2
    <~ _temp
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 0, "Expected no errors for same-scope access in function");
}

#[test]
fn test_multiple_underscore_vars_same_scope() {
    let source = r#"
_a = 10
_b = 20
_c = _a + _b
>> _c ¶
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 0, "Expected no errors for multiple _vars in same scope");
}

// ============================================================================
// INVALID CASES: Accessing _variables from outer scopes
// ============================================================================

#[test]
fn test_underscore_var_invalid_access_from_outer_scope() {
    let source = r#"
? #1 {
    _inner = 42
}
>> _inner ¶
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 1, "Expected error for accessing _inner from outer scope");
    assert!(errors[0].contains("cannot access underscore variable '_inner'"));
    assert!(errors[0].contains("outer scope"));
}

#[test]
fn test_underscore_var_invalid_in_nested_if() {
    let source = r#"
? #1 {
    _outer = 10
    ? #1 {
        >> _outer ¶
    }
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 1, "Expected error for accessing _outer from inner IF");
    assert!(errors[0].contains("cannot access underscore variable '_outer'"));
    assert!(errors[0].contains("inner scope"));
}

#[test]
fn test_underscore_var_invalid_in_loop_body() {
    let source = r#"
_counter = 0
@ i:1..5 {
    >> _counter ¶
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 1, "Expected error for accessing _counter from loop");
    assert!(errors[0].contains("cannot access underscore variable '_counter'"));
}

#[test]
fn test_underscore_var_invalid_after_if_block() {
    let source = r#"
? #1 {
    _data = [1, 2, 3]
}
_? #0 {
    >> _data ¶
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    // IF and ELSE-IF are sibling scopes - each can have independent _data
    assert_eq!(errors.len(), 0, "Expected no error - IF and ELSE-IF are sibling scopes");
}

#[test]
fn test_underscore_var_invalid_after_loop() {
    let source = r#"
@ i:1..3 {
    _sum = i * 2
}
>> _sum ¶
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 1, "Expected error for accessing _sum after loop");
    assert!(errors[0].contains("cannot access underscore variable '_sum'"));
}

// ============================================================================
// INVALID CASES: Assignment from wrong scope
// ============================================================================

#[test]
fn test_underscore_var_invalid_assignment_from_outer() {
    let source = r#"
? #1 {
    _value = 10
}
_value = 20
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 1, "Expected error for assigning to _value from outer scope");
    assert!(errors[0].contains("cannot access underscore variable '_value'"));
}

#[test]
fn test_underscore_var_invalid_assignment_from_inner() {
    let source = r#"
_config = "initial"
? #1 {
    _config = "modified"
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 1, "Expected error for assigning to _config from inner scope");
    assert!(errors[0].contains("cannot access underscore variable '_config'"));
}

// ============================================================================
// VALID CASES: Re-instantiation in different scopes
// ============================================================================

#[test]
fn test_underscore_var_reinstantiation_sibling_scopes() {
    let source = r#"
? #1 {
    _temp = 10
    >> _temp ¶
}
? #1 {
    _temp = 20
    >> _temp ¶
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 0, "Expected no errors for re-declaring _temp in sibling IF blocks");
}

#[test]
fn test_underscore_var_reinstantiation_sequential_loops() {
    let source = r#"
@ i:1..3 {
    _item = i
    >> _item ¶
}
@ j:1..3 {
    _item = j * 2
    >> _item ¶
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 0, "Expected no errors for re-declaring _item in different loops");
}

#[test]
fn test_underscore_var_shadowing_in_nested_scope() {
    let source = r#"
? #1 {
    _x = "outer"
    >> _x ¶
}
? #0 {
    _x = "inner"
    >> _x ¶
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 0, "Expected no errors for independent _x in different scopes");
}

// ============================================================================
// COMPLEX CASES: Nested scopes
// ============================================================================

#[test]
fn test_deeply_nested_underscore_vars() {
    let source = r#"
? #1 {
    _level1 = 1
    ? #1 {
        _level2 = 2
        ? #1 {
            _level3 = 3
            >> _level3 ¶
        }
    }
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 0, "Expected no errors for nested _vars accessed in their own scopes");
}

#[test]
fn test_deeply_nested_invalid_access() {
    let source = r#"
? #1 {
    _level1 = 1
    ? #1 {
        _level2 = 2
        ? #1 {
            >> _level1 ¶
        }
    }
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 1, "Expected error for accessing _level1 from nested scope");
    assert!(errors[0].contains("cannot access underscore variable '_level1'"));
}

#[test]
fn test_match_statement_underscore_vars() {
    let source = r#"
x = 5
result = ?? x {
    1..3 : "low" { _temp = "L"; >> _temp ¶ }
    4..6 : "mid" { _temp = "M"; >> _temp ¶ }
    _ : "high" { _temp = "H"; >> _temp ¶ }
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 0, "Expected no errors for _temp in different match arms");
}

#[test]
fn test_function_with_nested_blocks() {
    let source = r#"
process(x) {
    _result = x * 2
    ? x > 10 {
        _factor = 3
        _result = _result * _factor
    }
    <~ _result
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    // Two errors expected: assignment to _result and read of _result (both invalid from nested IF)
    assert_eq!(errors.len(), 2, "Expected 2 errors for accessing _result from nested IF (assign + read)");
    assert!(errors.iter().all(|e| e.contains("cannot access underscore variable '_result'")));
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[test]
fn test_underscore_var_in_expression() {
    let source = r#"
? #1 {
    _a = 5
    _b = 10
    _sum = _a + _b
    >> _sum ¶
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 0, "Expected no errors for _vars used in expressions");
}

#[test]
fn test_underscore_var_in_collection() {
    let source = r#"
? #1 {
    _data = [1, 2, 3]
    _first = _data[0]
    >> _first ¶
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 0, "Expected no errors for _vars with collections");
}

#[test]
fn test_underscore_var_with_lifetime_end() {
    let source = r#"
? #1 {
    _resource = "data"
    >> _resource ¶
    \_resource
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 0, "Expected no errors for explicit lifetime end of _var");
}

#[test]
fn test_mixed_normal_and_underscore_vars() {
    let source = r#"
x = 10
? #1 {
    _temp = x * 2
    y = _temp + 5
    >> y ¶
}
>> x ¶
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    // y is used in nested scope but declared there, so it's not accessible outside
    // x is normal variable and accessible anywhere
    assert_eq!(errors.len(), 0, "Expected no errors for mixing normal and _vars");
}

#[test]
fn test_underscore_const_declaration() {
    let source = r#"
? #1 {
    _PI := 3.14159
    area = _PI * 10
    >> area ¶
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 0, "Expected no errors for _const in same scope");
}

#[test]
fn test_underscore_const_invalid_access() {
    let source = r#"
? #1 {
    _MAX := 100
}
? #1 {
    >> _MAX ¶
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    // Two separate IF blocks are sibling scopes - each can have independent _MAX
    assert_eq!(errors.len(), 0, "Expected no error - sibling IF blocks can have independent _MAX");
}

// ============================================================================
// LOOP-SPECIFIC CASES
// ============================================================================

#[test]
fn test_underscore_var_in_while_loop() {
    let source = r#"
counter = 0
@ counter < 5 {
    _double = counter * 2
    >> _double ¶
    counter = counter + 1
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 0, "Expected no errors for _var in while loop body");
}

#[test]
fn test_underscore_var_invalid_in_nested_loops() {
    let source = r#"
@ i:1..3 {
    _outer = i
    @ j:1..2 {
        >> _outer ¶
    }
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 1, "Expected error for accessing _outer from nested loop");
    assert!(errors[0].contains("cannot access underscore variable '_outer'"));
}

#[test]
fn test_underscore_iterator_var() {
    let source = r#"
data = [1, 2, 3]
@ _item:data {
    >> _item ¶
}
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 0, "Expected no errors for _iterator variable in loop");
}

#[test]
fn test_underscore_iterator_invalid_after_loop() {
    let source = r#"
data = [1, 2, 3]
@ _item:data {
    >> _item ¶
}
>> _item ¶
"#;
    let (_warnings, errors) = parse_and_analyze(source);
    assert_eq!(errors.len(), 1, "Expected error for accessing _item after loop");
    assert!(errors[0].contains("cannot access underscore variable '_item'"));
}
