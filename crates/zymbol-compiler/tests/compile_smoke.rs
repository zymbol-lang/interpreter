//! Targeted unit tests for the AST → bytecode compiler.
//!
//! The 498-file E2E parity suite exercises the compiler end to end, but a
//! regression there surfaces as an output diff. These tests pin the basic
//! compiler contract directly so failures point at the compile stage, and
//! guard the bincode wire format the standalone pipeline depends on.

use zymbol_compiler::Compiler;
use zymbol_lexer::Lexer;
use zymbol_parser::Parser;
use zymbol_span::FileId;

fn compile(src: &str) -> zymbol_bytecode::CompiledProgram {
    let (tokens, diags) = Lexer::new(src, FileId(0)).tokenize();
    assert!(diags.is_empty(), "lex diagnostics: {diags:?}");
    let program = Parser::new(tokens)
        .parse()
        .unwrap_or_else(|d| panic!("parse diagnostics: {d:?}"));
    Compiler::compile(&program).expect("compile failed")
}

#[test]
fn simple_program_produces_instructions() {
    let compiled = compile("x := 5\n>> x + 1 ¶");
    assert!(
        !compiled.main.instructions.is_empty(),
        "main chunk must contain instructions"
    );
    assert!(
        compiled.main.num_registers > 0,
        "register allocator must reserve registers"
    );
    assert!(compiled.functions.is_empty(), "no functions were defined");
}

#[test]
fn function_definition_gets_own_chunk() {
    let compiled = compile("add(a, b) { <~ a + b }\n>> add(3, 4) ¶");
    assert_eq!(
        compiled.functions.len(),
        1,
        "one function chunk expected, got: {:?}",
        compiled.functions.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
    let chunk = &compiled.functions[0];
    assert_eq!(chunk.num_params, 2, "add/2 must record its arity");
    assert!(!chunk.instructions.is_empty());
}

#[test]
fn string_literals_land_in_string_pool() {
    let compiled = compile(">> \"hello bytecode\" ¶");
    assert!(
        compiled
            .string_pool
            .iter()
            .any(|s| s.contains("hello bytecode")),
        "string literal missing from pool: {:?}",
        compiled.string_pool
    );
}

/// The standalone pipeline (`zymbol build`) serializes a CompiledProgram with
/// bincode 1.x and the generated binary deserializes it. This roundtrip pins
/// that wire format: if an Instruction/Chunk change or a bincode major bump
/// breaks it, this fails here instead of inside a generated project.
#[test]
fn bincode_roundtrip_is_stable() {
    let compiled = compile("f(n) { <~ n * 2 }\nx := f(21)\n>> \"x={x}\" ¶");
    let bytes = bincode::serialize(&compiled).expect("serialize");
    let decoded: zymbol_bytecode::CompiledProgram =
        bincode::deserialize(&bytes).expect("deserialize");
    let bytes2 = bincode::serialize(&decoded).expect("re-serialize");
    assert_eq!(bytes, bytes2, "roundtrip must be byte-identical");
    assert_eq!(decoded.main.instructions.len(), compiled.main.instructions.len());
    assert_eq!(decoded.functions.len(), compiled.functions.len());
    assert_eq!(decoded.string_pool, compiled.string_pool);
}
