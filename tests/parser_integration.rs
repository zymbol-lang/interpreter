/// Integration tests for parser - verifies example files parse correctly
use std::fs;
use zymbol_lexer::Lexer;
use zymbol_parser::Parser;
use zymbol_span::FileId;

fn parse_file(path: &str) -> Result<(), String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path, e))?;

    let lexer = Lexer::new(&source, FileId(0));
    let (tokens, lex_errors) = lexer.tokenize();

    if !lex_errors.is_empty() {
        return Err(format!("Lexer errors in {}: {} errors", path, lex_errors.len()));
    }

    let parser = Parser::new(tokens);
    let result = parser.parse();

    match result {
        Ok(_program) => Ok(()),
        Err(errors) => Err(format!("Parser errors in {}: {} errors", path, errors.len())),
    }
}

#[test]
fn test_phase01_examples() {
    let examples = vec![
        "examples/phase01/01_hello.z",
        "examples/phase01/02_multiline.z",
        "examples/phase01/03_assignment.z",
        "examples/phase01/04_concatenation.z",
        "examples/phase01/05_variables_concat.z",
        "examples/phase01/06_unicode.z",
    ];

    for example in examples {
        if let Err(e) = parse_file(example) {
            panic!("Failed to parse {}: {}", example, e);
        }
    }
}

#[test]
fn test_01_hello() {
    parse_file("examples/phase01/01_hello.z").expect("should parse");
}

#[test]
fn test_02_multiline() {
    parse_file("examples/phase01/02_multiline.z").expect("should parse");
}

#[test]
fn test_03_assignment() {
    parse_file("examples/phase01/03_assignment.z").expect("should parse");
}

#[test]
fn test_04_concatenation() {
    parse_file("examples/phase01/04_concatenation.z").expect("should parse");
}

#[test]
fn test_05_variables_concat() {
    parse_file("examples/phase01/05_variables_concat.z").expect("should parse");
}

#[test]
fn test_06_unicode() {
    parse_file("examples/phase01/06_unicode.z").expect("should parse");
}

#[test]
fn test_phase11_examples() {
    let examples = vec![
        "examples/phase11/01_match_basic.z",
        "examples/phase11/02_match_ranges.z",
        "examples/phase11/03_match_assignment.z",
        "examples/phase11/04_match_guards.z",
        "examples/phase11/05_match_nested.z",
        "examples/phase11/06_execution_only.z",
        "examples/phase11/07_unused_value_warning.z",
    ];

    for example in examples {
        if let Err(e) = parse_file(example) {
            panic!("Failed to parse {}: {}", example, e);
        }
    }
}

#[test]
fn test_execution_only_match() {
    parse_file("examples/phase11/06_execution_only.z").expect("should parse execution-only match");
}

#[test]
fn test_tuples() {
    parse_file("test_tuples.z").expect("should parse tuple test file");
}

#[test]
fn test_base_conversions() {
    parse_file("test_base_conversions.zy").expect("should parse base conversion test file");
}

#[test]
fn test_base_conversions_demo() {
    parse_file("examples/base_conversions_demo.zy").expect("should parse base conversion demo file");
}
