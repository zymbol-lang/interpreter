//! Integration tests for auto-destruction (Phase 5)
//!
//! Tests the complete pipeline:
//! 1. DefUseAnalyzer computes last uses
//! 2. Generate destruction schedule
//! 3. Interpreter destroys variables after last use
//! 4. Use-after-destruction detection

use zymbol_semantic::{DefUseAnalyzer, ControlFlowGraph};
use zymbol_parser::Parser;
use zymbol_lexer::Lexer;
use zymbol_span::FileId;
use zymbol_ast::Program;
use std::collections::HashMap;

/// Helper to parse source and generate destruction schedule
fn analyze_and_schedule(source: &str) -> (Program, HashMap<usize, Vec<String>>) {
    let file_id = FileId(0);
    let lexer = Lexer::new(source, file_id);
    let (tokens, _) = lexer.tokenize();

    let parser = Parser::new(tokens);
    let program = parser.parse().expect("parse failed");

    // Build CFG (use sequential for now - simpler mapping)
    let cfg = ControlFlowGraph::build_sequential(&program.statements);
    let mut analyzer = DefUseAnalyzer::new();
    let _chains = analyzer.analyze(&program.statements, &cfg);

    let schedule = analyzer.generate_destruction_schedule(&cfg);

    (program, schedule)
}

// ============================================================================
// BASIC AUTO-DESTRUCTION TESTS
// ============================================================================

#[test]
fn test_simple_auto_destruction() {
    let source = r#"
x = "test"
>> x ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // x should be destroyed after statement 1 (the output statement)
    assert!(schedule.contains_key(&1), "Expected destruction schedule for statement 1");
    assert_eq!(schedule[&1], vec!["x"], "Expected x to be destroyed after output");
}

#[test]
fn test_multiple_variables_sequential() {
    let source = r#"
a = 1
b = 2
c = 3
>> a ¶
>> b ¶
>> c ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // Each variable destroyed after its last use
    // Note: >> a ¶ is TWO statements (Output + Newline), so indices are:
    // stmt 0: a = 1, stmt 1: b = 2, stmt 2: c = 3
    // stmt 3: >> a, stmt 4: ¶
    // stmt 5: >> b, stmt 6: ¶
    // stmt 7: >> c, stmt 8: ¶
    assert_eq!(schedule.get(&3), Some(&vec!["a".to_string()]));
    assert_eq!(schedule.get(&5), Some(&vec!["b".to_string()]));
    assert_eq!(schedule.get(&7), Some(&vec!["c".to_string()]));
}

#[test]
fn test_variable_used_in_expression() {
    let source = r#"
x = 10
y = 20
sum = x + y
>> sum ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // x and y should be destroyed after being used in sum calculation (statement 2)
    assert!(schedule.contains_key(&2));
    let destroyed = &schedule[&2];
    assert!(destroyed.contains(&"x".to_string()));
    assert!(destroyed.contains(&"y".to_string()));

    // sum destroyed after output
    assert_eq!(schedule.get(&3), Some(&vec!["sum".to_string()]));
}

#[test]
fn test_no_destruction_for_reassigned_var() {
    let source = r#"
x = 10
>> x ¶
x = 20
>> x ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // x should be destroyed after final use (statement 5), not after first use
    // stmt 0: x = 10, stmt 1: >> x, stmt 2: ¶, stmt 3: x = 20, stmt 4: >> x, stmt 5: ¶
    assert!(!schedule.contains_key(&1), "x should not be destroyed after first output");
    assert_eq!(schedule.get(&4), Some(&vec!["x".to_string()]), "x should be destroyed after final output");
}

#[test]
fn test_variable_reused_multiple_times() {
    let source = r#"
name = "Alice"
>> "Hello " >> name ¶
>> "Goodbye " >> name ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // name destroyed after last output
    // NOTE: >> "X" >> name creates separate Output statements, so:
    // stmt 0: name = "Alice"
    // stmt 1: >> "Hello ", stmt 2: >> name, stmt 3: ¶
    // stmt 4: >> "Goodbye ", stmt 5: >> name, stmt 6: ¶
    assert_eq!(schedule.get(&5), Some(&vec!["name".to_string()]));
}

// ============================================================================
// UNDERSCORE VARIABLE TESTS (should NOT auto-destruct via schedule)
// ============================================================================

#[test]
fn test_underscore_var_not_in_schedule() {
    let source = r#"
_temp = 42
>> _temp ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // _temp should NOT be in destruction schedule (handled by block scoping)
    assert!(!schedule.values().any(|vars| vars.contains(&"_temp".to_string())),
            "Underscore variables should not be in destruction schedule");
}

#[test]
fn test_mixed_normal_and_underscore() {
    let source = r#"
x = 10
_y = 20
result = x + _y
>> result ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // Only x should be in schedule, not _y
    assert!(schedule.values().any(|vars| vars.contains(&"x".to_string())));
    assert!(!schedule.values().any(|vars| vars.contains(&"_y".to_string())));
}

// ============================================================================
// CONDITIONAL FLOW TESTS
// ============================================================================

#[test]
fn test_if_statement_simple() {
    let source = r#"
x = 10
? x > 5 {
    >> "yes" ¶
}
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // x used in condition, should be destroyed after IF
    // Note: May be ambiguous depending on implementation
    // For now just check it exists somewhere
    assert!(!schedule.is_empty());
}

#[test]
fn test_variable_used_before_if() {
    let source = r#"
x = 10
>> x ¶
? #1 {
    >> "done" ¶
}
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // x destroyed after output (statement 1)
    assert_eq!(schedule.get(&1), Some(&vec!["x".to_string()]));
}

// ============================================================================
// LOOP TESTS (likely ambiguous - need explicit annotations)
// ============================================================================

#[test]
fn test_loop_variable_might_be_ambiguous() {
    let source = r#"
@ i:1..5 {
    >> i ¶
}
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // Loop iterator typically ambiguous, shouldn't be in schedule
    // (This test verifies our skip logic for ambiguous vars)
    assert!(!schedule.values().any(|vars| vars.contains(&"i".to_string())));
}

#[test]
fn test_variable_used_before_loop() {
    let source = r#"
limit = 10
@ i:1..limit {
    >> i ¶
}
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // limit should be destroyed after loop starts (used in range)
    // Statement 0 is limit =, statement 1 is loop
    // The use is in evaluating the range
    assert!(!schedule.is_empty());
}

// ============================================================================
// COMPLEX EXPRESSION TESTS
// ============================================================================

#[test]
fn test_nested_expressions() {
    let source = r#"
a = 5
b = 10
c = 15
result = (a + b) * c
>> result ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // a, b, c all used in statement 3, should be destroyed there
    assert!(schedule.contains_key(&3));
    let destroyed = &schedule[&3];
    assert!(destroyed.contains(&"a".to_string()));
    assert!(destroyed.contains(&"b".to_string()));
    assert!(destroyed.contains(&"c".to_string()));
}

#[test]
fn test_array_construction() {
    let source = r#"
x = 1
y = 2
z = 3
arr = [x, y, z]
>> arr ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // x, y, z destroyed after array creation (statement 3)
    assert!(schedule.contains_key(&3));
    let destroyed = &schedule[&3];
    assert!(destroyed.contains(&"x".to_string()));
    assert!(destroyed.contains(&"y".to_string()));
    assert!(destroyed.contains(&"z".to_string()));
}

#[test]
fn test_tuple_construction() {
    let source = r#"
first = "Alice"
second = "Bob"
pair = (first, second)
>> pair ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // first and second destroyed after tuple creation
    assert!(schedule.contains_key(&2));
    let destroyed = &schedule[&2];
    assert!(destroyed.contains(&"first".to_string()));
    assert!(destroyed.contains(&"second".to_string()));
}

// ============================================================================
// FUNCTION TESTS
// ============================================================================

#[test]
fn test_function_parameters_not_in_global_schedule() {
    let source = r#"
calculate(x, y) {
    <~ x + y
}
result = calculate(5, 10)
>> result ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // Function parameters (x, y) are in function scope, not global
    // Only result should be in schedule
    assert!(schedule.values().any(|vars| vars.contains(&"result".to_string())));
    assert!(!schedule.values().any(|vars| vars.contains(&"x".to_string())));
    assert!(!schedule.values().any(|vars| vars.contains(&"y".to_string())));
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[test]
fn test_unused_variable() {
    let source = r#"
x = 10
>> "done" ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // x is never used, so no last use - shouldn't be in schedule
    // (Will be caught by unused variable warning, but not auto-destroyed)
    assert!(schedule.is_empty() || !schedule.values().any(|vars| vars.contains(&"x".to_string())));
}

#[test]
fn test_variable_only_assigned() {
    let source = r#"
x = 5
x = 10
x = 15
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // x is only assigned, never read - no last use
    assert!(schedule.is_empty() || !schedule.values().any(|vars| vars.contains(&"x".to_string())));
}

#[test]
fn test_chained_operations() {
    let source = r#"
a = 10
b = 20
c = a + b
d = c * 2
>> d ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // a, b destroyed after creating c
    assert!(schedule.get(&2).is_some_and(|v| v.contains(&"a".to_string())));
    assert!(schedule.get(&2).is_some_and(|v| v.contains(&"b".to_string())));

    // c destroyed after creating d
    assert_eq!(schedule.get(&3), Some(&vec!["c".to_string()]));

    // d destroyed after output
    assert_eq!(schedule.get(&4), Some(&vec!["d".to_string()]));
}

#[test]
fn test_same_variable_name_different_scopes() {
    let source = r#"
x = "outer"
>> x ¶
? #1 {
    x = "inner"
    >> x ¶
}
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // This is tricky - x is reassigned, so last use is in the IF block
    // Depends on how scoping is handled
    assert!(!schedule.is_empty());
}

#[test]
fn test_collection_operations() {
    let source = r#"
data = [1, 2, 3]
first = data[0]
>> first ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // data used in indexing (statement 1)
    assert!(schedule.contains_key(&1));

    // first used in output (statement 2)
    assert_eq!(schedule.get(&2), Some(&vec!["first".to_string()]));
}

#[test]
fn test_string_concatenation() {
    let source = r#"
first = "Hello"
second = "World"
message = first + " " + second
>> message ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // first and second destroyed after concatenation
    assert!(schedule.contains_key(&2));
    let destroyed = &schedule[&2];
    assert!(destroyed.contains(&"first".to_string()));
    assert!(destroyed.contains(&"second".to_string()));
}

#[test]
fn test_match_expression() {
    let source = r#"
x = 5
result = ?? x {
    1..3 : "low"
    4..6 : "mid"
    _ : "high"
}
>> result ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // x used in match scrutinee (statement 1)
    assert!(schedule.contains_key(&1));

    // result used in output (statement 2)
    assert_eq!(schedule.get(&2), Some(&vec!["result".to_string()]));
}

#[test]
fn test_const_declaration() {
    let source = r#"
PI := 3.14159
radius = 10
area = PI * radius * radius
>> area ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // PI and radius both used in area calculation
    assert!(schedule.contains_key(&2));
}

#[test]
fn test_multiple_outputs_same_var() {
    let source = r#"
name = "Alice"
>> "Name: " >> name ¶
>> "Again: " >> name ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // name used in both outputs, destroyed after second one
    // NOTE: >> "X" >> name creates separate Output statements, so:
    // stmt 0: name = "Alice"
    // stmt 1: >> "Name: ", stmt 2: >> name, stmt 3: ¶
    // stmt 4: >> "Again: ", stmt 5: >> name, stmt 6: ¶
    assert_eq!(schedule.get(&5), Some(&vec!["name".to_string()]));
}

// ============================================================================
// SCHEDULE CORRECTNESS TESTS
// ============================================================================

#[test]
fn test_schedule_has_correct_indices() {
    let source = r#"
a = 1
b = 2
c = a + b
>> c ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // Statement 0: a = 1
    // Statement 1: b = 2
    // Statement 2: c = a + b (a and b destroyed here)
    // Statement 3: >> c ¶ (c destroyed here)

    for (stmt_idx, vars) in &schedule {
        assert!(*stmt_idx < 4, "Statement index should be valid");
        assert!(!vars.is_empty(), "Should have at least one variable to destroy");
    }
}

#[test]
fn test_no_duplicate_destructions() {
    let source = r#"
x = 10
y = x + 5
>> y ¶
"#;
    let (_program, schedule) = analyze_and_schedule(source);

    // x should only appear once in the schedule
    let mut x_count = 0;
    for vars in schedule.values() {
        if vars.contains(&"x".to_string()) {
            x_count += 1;
        }
    }
    assert_eq!(x_count, 1, "Variable should only be scheduled for destruction once");
}
