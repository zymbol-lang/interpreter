//! End-to-end tests for Zymbol-Lang
//!
//! These tests run full Zymbol programs through the complete pipeline:
//! source code -> lexer -> parser -> interpreter -> output
//! And also through the Register VM pipeline (Sprint 4C).

use zymbol_compiler::Compiler;
use zymbol_interpreter::Interpreter;
use zymbol_lexer::Lexer;
use zymbol_parser::Parser;
use zymbol_span::FileId;
use zymbol_vm::VM;

/// Run a Zymbol program and return its output as a string
fn run(source: &str) -> String {
    let mut output = Vec::new();
    let lexer = Lexer::new(source, FileId(0));
    let (tokens, lex_diagnostics) = lexer.tokenize();
    assert!(
        lex_diagnostics.is_empty(),
        "Lexer errors: {:?}",
        lex_diagnostics
    );
    let parser = Parser::new(tokens);
    let program = parser.parse().expect("Parse error");
    let mut interpreter = Interpreter::with_output(&mut output);
    interpreter.execute(&program).expect("Runtime error");
    String::from_utf8(output).expect("Invalid UTF-8")
}

/// Run a Zymbol program through the Register VM and return output.
/// Returns Err(msg) if compilation or runtime fails.
fn run_vm(source: &str) -> Result<String, String> {
    let lexer = Lexer::new(source, FileId(0));
    let (tokens, lex_diagnostics) = lexer.tokenize();
    if !lex_diagnostics.is_empty() {
        return Err(format!("Lex errors: {:?}", lex_diagnostics));
    }
    let parser = Parser::new(tokens);
    let program = parser.parse().map_err(|e| format!("Parse error: {:?}", e))?;
    let compiled = Compiler::compile(&program).map_err(|e| format!("Compile error: {}", e))?;
    let mut output = Vec::new();
    let mut vm = VM::new(&mut output);
    vm.run(&compiled).map_err(|e| format!("VM error: {}", e))?;
    String::from_utf8(output).map_err(|e| e.to_string())
}

/// Run a program and expect a runtime error
#[allow(dead_code)]
fn run_err(source: &str) -> String {
    let mut output = Vec::new();
    let lexer = Lexer::new(source, FileId(0));
    let (tokens, lex_diagnostics) = lexer.tokenize();
    assert!(
        lex_diagnostics.is_empty(),
        "Lexer errors: {:?}",
        lex_diagnostics
    );
    let parser = Parser::new(tokens);
    let program = parser.parse().expect("Parse error");
    let mut interpreter = Interpreter::with_output(&mut output);
    let err = interpreter
        .execute(&program)
        .expect_err("Expected runtime error");
    err.to_string()
}

// ─── Variables & Constants ───────────────────────────────────────────

#[test]
fn test_variable_assignment_and_output() {
    let out = run(r#"
x = 42
>> x ¶
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn test_constant_declaration() {
    let out = run(r#"
PI := 3.14
>> PI ¶
"#);
    assert_eq!(out, "3.14\n");
}

#[test]
fn test_variable_reassignment() {
    let out = run(r#"
x = 1
x = 2
>> x ¶
"#);
    assert_eq!(out, "2\n");
}

#[test]
fn test_string_interpolation() {
    let out = run(r#"
name = "World"
>> "Hello, {name}!" ¶
"#);
    assert_eq!(out, "Hello, World!\n");
}

// ─── Arithmetic ──────────────────────────────────────────────────────

#[test]
fn test_arithmetic_operations() {
    let out = run(r#"
a = 2 + 3
b = 10 - 4
c = 3 * 7
d = 15 / 4
e = 17 % 5
>> a ¶
>> b ¶
>> c ¶
>> d ¶
>> e ¶
"#);
    assert_eq!(out, "5\n6\n21\n3\n2\n");
}

#[test]
fn test_float_arithmetic() {
    let out = run(r#"
a = 1.5 + 2.5
b = 10.0 / 3.0
>> a ¶
>> b ¶
"#);
    let lines: Vec<&str> = out.trim().split('\n').collect();
    assert_eq!(lines[0], "4");
    assert!(lines[1].starts_with("3.333"));
}

#[test]
fn test_operator_precedence() {
    let out = run(r#"
a = 2 + 3 * 4
b = (2 + 3) * 4
>> a ¶
>> b ¶
"#);
    assert_eq!(out, "14\n20\n");
}

// ─── Booleans & Comparison ───────────────────────────────────────────

#[test]
fn test_boolean_literals() {
    let out = run(r#"
>> #1 ¶
>> #0 ¶
"#);
    assert_eq!(out, "#1\n#0\n");
}

#[test]
fn test_comparison_operators() {
    let out = run(r#"
a = 5 > 3
b = 5 < 3
c = 5 == 5
d = 5 <> 3
>> a ¶
>> b ¶
>> c ¶
>> d ¶
"#);
    assert_eq!(out, "#1\n#0\n#1\n#1\n");
}

// ─── Control Flow ────────────────────────────────────────────────────

#[test]
fn test_if_else() {
    let out = run(r#"
x = 10
? x > 5 {
    >> "big" ¶
} _ {
    >> "small" ¶
}
"#);
    assert_eq!(out, "big\n");
}

#[test]
fn test_if_else_if_else() {
    let out = run(r#"
x = 0
? x > 0 {
    >> "positive" ¶
} _? x < 0 {
    >> "negative" ¶
} _ {
    >> "zero" ¶
}
"#);
    assert_eq!(out, "zero\n");
}

#[test]
fn test_match_expression() {
    let out = run(r#"
score = 85
grade = ?? score {
    90..100 : "A"
    80..89  : "B"
    70..79  : "C"
    _       : "F"
}
>> grade ¶
"#);
    assert_eq!(out, "B\n");
}

// ─── Loops ───────────────────────────────────────────────────────────

#[test]
fn test_while_loop() {
    let out = run(r#"
i = 0
@ i < 5 {
    >> i
    i = i + 1
}
>> ¶
"#);
    assert_eq!(out, "01234\n");
}

#[test]
fn test_foreach_loop() {
    let out = run(r#"
@ item:[10, 20, 30] {
    >> item
    >> " "
}
>> ¶
"#);
    assert_eq!(out, "10 20 30 \n");
}

#[test]
fn test_range_loop() {
    let out = run(r#"
@ i:1..3 {
    >> i " "
}
>> ¶
"#);
    assert_eq!(out, "1 2 3 \n");
}

#[test]
fn test_loop_break() {
    let out = run(r#"
i = 0
@ {
    ? i == 3 { @! }
    >> i
    i = i + 1
}
>> ¶
"#);
    assert_eq!(out, "012\n");
}

#[test]
fn test_loop_continue() {
    let out = run(r#"
@ i:0..4 {
    ? i == 2 { @> }
    >> i
}
>> ¶
"#);
    assert_eq!(out, "0134\n");
}

// ─── Functions ───────────────────────────────────────────────────────

#[test]
fn test_function_declaration_and_call() {
    let out = run(r#"
add(a, b) {
    <~ a + b
}
result = add(3, 4)
>> result ¶
"#);
    assert_eq!(out, "7\n");
}

#[test]
fn test_recursive_function() {
    let out = run(r#"
factorial(n) {
    ? n <= 1 { <~ 1 }
    <~ n * factorial(n - 1)
}
>> factorial(5) ¶
"#);
    assert_eq!(out, "120\n");
}

#[test]
fn test_lambda_expression() {
    let out = run(r#"
double = x -> x * 2
>> double(5) ¶
"#);
    assert_eq!(out, "10\n");
}

#[test]
fn test_lambda_block() {
    let out = run(r#"
clamp = (x, lo, hi) -> {
    ? x < lo { <~ lo }
    ? x > hi { <~ hi }
    <~ x
}
>> clamp(15, 0, 10) ¶
"#);
    assert_eq!(out, "10\n");
}

// ─── Collections ─────────────────────────────────────────────────────

#[test]
fn test_array_operations() {
    let out = run(r#"
arr = [1, 2, 3]
len1 = arr$#
arr = arr$+ 4
len2 = arr$#
has2 = arr$? 2
has99 = arr$? 99
>> len1 ¶
>> len2 ¶
>> has2 ¶
>> has99 ¶
"#);
    assert_eq!(out, "3\n4\n#1\n#0\n");
}

#[test]
fn test_array_indexing() {
    let out = run(r#"
arr = [10, 20, 30]
>> arr[0] ¶
>> arr[2] ¶
"#);
    assert_eq!(out, "10\n30\n");
}

#[test]
fn test_array_slice() {
    let out = run(r#"
arr = [1, 2, 3, 4, 5]
slice = arr$[1..3]
>> slice ¶
"#);
    assert_eq!(out, "[2, 3]\n");
}

#[test]
fn test_named_tuple() {
    let out = run(r#"
person = (name: "Alice", age: 25)
>> person.name ¶
>> person.age ¶
"#);
    assert_eq!(out, "Alice\n25\n");
}

#[test]
fn test_tuple_basic() {
    let out = run(r#"
pair = (1, "hello")
>> pair[0] ¶
>> pair[1] ¶
"#);
    assert_eq!(out, "1\nhello\n");
}

// ─── Strings ─────────────────────────────────────────────────────────

#[test]
fn test_string_concatenation() {
    let out = run(r#"
>> "Hello" + " " + "World" ¶
"#);
    assert_eq!(out, "Hello World\n");
}

#[test]
fn test_string_auto_convert_concat() {
    let out = run(r#"
>> "Score: " + 95 ¶
"#);
    assert_eq!(out, "Score: 95\n");
}

#[test]
fn test_string_split() {
    let out = run(r#"
parts = "a,b,c" / ','
len = parts$#
>> parts ¶
>> len ¶
"#);
    assert_eq!(out, "[a, b, c]\n3\n");
}

#[test]
fn test_haskell_style_output() {
    let out = run(r#"
>> "a" "b" "c" ¶
"#);
    assert_eq!(out, "abc\n");
}

// ─── Error Handling ──────────────────────────────────────────────────

#[test]
fn test_try_catch_basic() {
    let out = run(r#"
!? {
    arr = [1, 2, 3]
    x = arr[99]
} :! {
    >> "caught error" ¶
}
"#);
    assert_eq!(out, "caught error\n");
}

#[test]
fn test_try_finally() {
    let out = run(r#"
!? {
    >> "try" ¶
} :> {
    >> "finally" ¶
}
"#);
    assert_eq!(out, "try\nfinally\n");
}

#[test]
fn test_error_check() {
    let out = run(r#"
!? {
    arr = [1]
    val = arr[99]
} :! {
    >> "error caught" ¶
}
"#);
    assert_eq!(out, "error caught\n");
}

// ─── Data Operations ─────────────────────────────────────────────────

#[test]
fn test_type_metadata() {
    // Type metadata returns tuples: (type_symbol, size, value)
    let out = run("x = 42\nt = x#?\n>> t ¶\n");
    assert!(out.starts_with("(###,"), "Int type metadata: {}", out);

    let out = run("x = \"hello\"\nt = x#?\n>> t ¶\n");
    assert!(out.contains("##\""), "String type metadata: {}", out);

    let out = run("x = #1\nt = x#?\n>> t ¶\n");
    assert!(out.contains("##?"), "Bool type metadata: {}", out);

    let out = run("x = [1,2]\nt = x#?\n>> t ¶\n");
    assert!(out.contains("##]"), "Array type metadata: {}", out);
}

#[test]
fn test_numeric_eval() {
    let out = run(r#"
x = "42"
>> #|x| ¶
"#);
    assert_eq!(out, "42\n");
}

// ─── Char Literals ───────────────────────────────────────────────────

#[test]
fn test_char_literal() {
    let out = run("c = 'A'\n>> c ¶\n");
    assert_eq!(out, "A\n");

    let out = run("c = 'A'\nt = c#?\n>> t ¶\n");
    assert!(out.contains("##'"), "Char type metadata: {}", out);
}

// ─── Complex Programs ────────────────────────────────────────────────

#[test]
fn test_fizzbuzz() {
    let out = run(r#"
@ i:1..15 {
    r15 = i % 15
    r3 = i % 3
    r5 = i % 5
    ? r15 == 0 {
        >> "FizzBuzz"
    } _? r3 == 0 {
        >> "Fizz"
    } _? r5 == 0 {
        >> "Buzz"
    } _ {
        >> i
    }
    >> " "
}
>> ¶
"#);
    assert_eq!(
        out,
        "1 2 Fizz 4 Buzz Fizz 7 8 Fizz Buzz 11 Fizz 13 14 FizzBuzz \n"
    );
}

#[test]
fn test_fibonacci() {
    let out = run(r#"
fib(n) {
    ? n <= 0 { <~ 0 }
    ? n == 1 { <~ 1 }
    <~ fib(n - 1) + fib(n - 2)
}
@ i:0..7 {
    >> fib(i) " "
}
>> ¶
"#);
    assert_eq!(out, "0 1 1 2 3 5 8 13 \n");
}

#[test]
fn test_higher_order_function_map() {
    let out = run(r#"
nums = [1, 2, 3, 4, 5]
doubled = nums$> (x -> x * 2)
>> doubled ¶
"#);
    assert_eq!(out, "[2, 4, 6, 8, 10]\n");
}

#[test]
fn test_higher_order_function_filter() {
    let out = run(r#"
nums = [1, 2, 3, 4, 5, 6]
evens = nums$| (x -> x % 2 == 0)
>> evens ¶
"#);
    assert_eq!(out, "[2, 4, 6]\n");
}

#[test]
fn test_higher_order_function_reduce() {
    let out = run(r#"
nums = [1, 2, 3, 4, 5]
sum = nums$< (0, (acc, x) -> acc + x)
>> sum ¶
"#);
    assert_eq!(out, "15\n");
}

#[test]
fn test_scope_isolation() {
    let out = run(r#"
x = "outer"
? #1 {
    x = "inner"
    >> x ¶
}
>> x ¶
"#);
    assert_eq!(out, "inner\ninner\n");
}

#[test]
fn test_nested_functions() {
    let out = run(r#"
apply(f, x) {
    <~ f(x)
}
inc = x -> x + 1
>> apply(inc, 41) ¶
"#);
    assert_eq!(out, "42\n");
}

#[test]
fn test_complex_match_with_execution() {
    let out = run(r#"
action = "greet"
?? action {
    "greet" : {
        >> "Hello!" ¶
    }
    "bye" : {
        >> "Goodbye!" ¶
    }
    _ : {
        >> "Unknown" ¶
    }
}
"#);
    assert_eq!(out, "Hello!\n");
}

// ─── Register VM Tests (Sprint 4C) ───────────────────────────────────
// Each test mirrors the tree-walker test above but runs through the VM pipeline.

#[test]
fn test_variable_assignment_and_output_vm() {
    let src = r#"
x = 42
>> x ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_constant_declaration_vm() {
    let src = r#"
PI := 3.14
>> PI ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_variable_reassignment_vm() {
    let src = r#"
x = 1
x = 2
>> x ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_string_interpolation_vm() {
    let src = r#"
name = "World"
>> "Hello, {name}!" ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_arithmetic_operations_vm() {
    let src = r#"
a = 2 + 3
b = 10 - 4
c = 3 * 7
d = 15 / 4
e = 17 % 5
>> a ¶
>> b ¶
>> c ¶
>> d ¶
>> e ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_float_arithmetic_vm() {
    let src = r#"
a = 1.5 + 2.5
b = 10.0 / 3.0
>> a ¶
>> b ¶
"#;
    let out = run_vm(src).expect("VM");
    let lines: Vec<&str> = out.trim().split('\n').collect();
    assert_eq!(lines[0], "4");
    assert!(lines[1].starts_with("3.333"));
}

#[test]
fn test_operator_precedence_vm() {
    let src = r#"
a = 2 + 3 * 4
b = (2 + 3) * 4
>> a ¶
>> b ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_boolean_literals_vm() {
    let src = r#"
>> #1 ¶
>> #0 ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_comparison_operators_vm() {
    let src = r#"
a = 5 > 3
b = 5 < 3
c = 5 == 5
d = 5 <> 3
>> a ¶
>> b ¶
>> c ¶
>> d ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_if_else_vm() {
    let src = r#"
x = 10
? x > 5 {
    >> "big" ¶
} _ {
    >> "small" ¶
}
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_if_else_if_else_vm() {
    let src = r#"
x = 0
? x > 0 {
    >> "positive" ¶
} _? x < 0 {
    >> "negative" ¶
} _ {
    >> "zero" ¶
}
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_match_expression_vm() {
    let src = r#"
score = 85
grade = ?? score {
    90..100 : "A"
    80..89  : "B"
    70..79  : "C"
    _       : "F"
}
>> grade ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_while_loop_vm() {
    let src = r#"
i = 0
@ i < 5 {
    >> i
    i = i + 1
}
>> ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_foreach_loop_vm() {
    let src = r#"
@ item:[10, 20, 30] {
    >> item
    >> " "
}
>> ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_range_loop_vm() {
    let src = r#"
@ i:1..3 {
    >> i " "
}
>> ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_loop_break_vm() {
    let src = r#"
i = 0
@ {
    ? i == 3 { @! }
    >> i
    i = i + 1
}
>> ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_loop_continue_vm() {
    let src = r#"
@ i:0..4 {
    ? i == 2 { @> }
    >> i
}
>> ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_function_declaration_and_call_vm() {
    let src = r#"
add(a, b) {
    <~ a + b
}
result = add(3, 4)
>> result ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_recursive_function_vm() {
    let src = r#"
factorial(n) {
    ? n <= 1 { <~ 1 }
    <~ n * factorial(n - 1)
}
>> factorial(5) ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_lambda_expression_vm() {
    let src = r#"
double = x -> x * 2
>> double(5) ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_lambda_block_vm() {
    let src = r#"
clamp = (x, lo, hi) -> {
    ? x < lo { <~ lo }
    ? x > hi { <~ hi }
    <~ x
}
>> clamp(15, 0, 10) ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_array_operations_vm() {
    let src = r#"
arr = [1, 2, 3]
len1 = arr$#
arr = arr$+ 4
len2 = arr$#
has2 = arr$? 2
has99 = arr$? 99
>> len1 ¶
>> len2 ¶
>> has2 ¶
>> has99 ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_array_indexing_vm() {
    let src = r#"
arr = [10, 20, 30]
>> arr[0] ¶
>> arr[2] ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_array_slice_vm() {
    let src = r#"
arr = [1, 2, 3, 4, 5]
slice = arr$[1..3]
>> slice ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_named_tuple_vm() {
    let src = r#"
person = (name: "Alice", age: 25)
>> person.name ¶
>> person.age ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_tuple_basic_vm() {
    let src = r#"
pair = (1, "hello")
>> pair[0] ¶
>> pair[1] ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_string_concatenation_vm() {
    let src = r#"
>> "Hello" + " " + "World" ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_string_auto_convert_concat_vm() {
    let src = r#"
>> "Score: " + 95 ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_string_split_vm() {
    let src = r#"
parts = "a,b,c" / ','
len = parts$#
>> parts ¶
>> len ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_haskell_style_output_vm() {
    let src = r#"
>> "a" "b" "c" ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_try_catch_basic_vm() {
    let src = r#"
!? {
    arr = [1, 2, 3]
    x = arr[99]
} :! {
    >> "caught error" ¶
}
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_try_finally_vm() {
    let src = r#"
!? {
    >> "try" ¶
} :> {
    >> "finally" ¶
}
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_error_check_vm() {
    let src = r#"
!? {
    arr = [1]
    val = arr[99]
} :! {
    >> "error caught" ¶
}
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_type_metadata_vm() {
    let out = run_vm("x = 42\nt = x#?\n>> t ¶\n").expect("VM");
    assert!(out.starts_with("(###,"), "Int type metadata: {}", out);

    let out = run_vm("x = \"hello\"\nt = x#?\n>> t ¶\n").expect("VM");
    assert!(out.contains("##\""), "String type metadata: {}", out);

    let out = run_vm("x = #1\nt = x#?\n>> t ¶\n").expect("VM");
    assert!(out.contains("##?"), "Bool type metadata: {}", out);

    let out = run_vm("x = [1,2]\nt = x#?\n>> t ¶\n").expect("VM");
    assert!(out.contains("##]"), "Array type metadata: {}", out);
}

#[test]
fn test_numeric_eval_vm() {
    let src = r#"
x = "42"
>> #|x| ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_char_literal_vm() {
    assert_eq!(run_vm("c = 'A'\n>> c ¶\n").expect("VM"), "A\n");
    let out = run_vm("c = 'A'\nt = c#?\n>> t ¶\n").expect("VM");
    assert!(out.contains("##'"), "Char type metadata: {}", out);
}

#[test]
fn test_fizzbuzz_vm() {
    let src = r#"
@ i:1..15 {
    r15 = i % 15
    r3 = i % 3
    r5 = i % 5
    ? r15 == 0 {
        >> "FizzBuzz"
    } _? r3 == 0 {
        >> "Fizz"
    } _? r5 == 0 {
        >> "Buzz"
    } _ {
        >> i
    }
    >> " "
}
>> ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_fibonacci_vm() {
    let src = r#"
fib(n) {
    ? n <= 0 { <~ 0 }
    ? n == 1 { <~ 1 }
    <~ fib(n - 1) + fib(n - 2)
}
@ i:0..7 {
    >> fib(i) " "
}
>> ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_higher_order_function_map_vm() {
    let src = r#"
nums = [1, 2, 3, 4, 5]
doubled = nums$> (x -> x * 2)
>> doubled ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_higher_order_function_filter_vm() {
    let src = r#"
nums = [1, 2, 3, 4, 5, 6]
evens = nums$| (x -> x % 2 == 0)
>> evens ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_higher_order_function_reduce_vm() {
    let src = r#"
nums = [1, 2, 3, 4, 5]
sum = nums$< (0, (acc, x) -> acc + x)
>> sum ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_scope_isolation_vm() {
    let src = r#"
x = "outer"
? #1 {
    x = "inner"
    >> x ¶
}
>> x ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_nested_functions_vm() {
    let src = r#"
apply(f, x) {
    <~ f(x)
}
inc = x -> x + 1
>> apply(inc, 41) ¶
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}

#[test]
fn test_complex_match_with_execution_vm() {
    let src = r#"
action = "greet"
?? action {
    "greet" : {
        >> "Hello!" ¶
    }
    "bye" : {
        >> "Goodbye!" ¶
    }
    _ : {
        >> "Unknown" ¶
    }
}
"#;
    assert_eq!(run_vm(src).expect("VM"), run(src));
}
