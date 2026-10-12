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

/// A frame holds what a function needs at once, not every temporary its body
/// ever asked for (ZYVM-010): the same statement written once and sixty-four
/// times needs the same registers, because each one's temporaries are handed
/// back when it ends.
#[test]
fn a_statement_hands_its_temporaries_back() {
    let body = |n: usize| {
        let stmts = "    >> ((x + a) * (b + 1) - (a * 2 + b)) % 97 ¶\n".repeat(n);
        format!("paso(x, a, b) {{\n{stmts}    <~ x + a\n}}\n>> paso(1, 2, 3) ¶")
    };
    let (one, many) = (compile(&body(1)), compile(&body(64)));
    assert_eq!(
        one.functions[0].num_registers,
        many.functions[0].num_registers,
        "64 statements in a row must fit in the frame of one"
    );
}

/// The instructions that take a register and read the next one keep their
/// pair adjacent even when single temporaries are being reused around them.
#[test]
fn a_slice_keeps_its_bounds_adjacent_among_reused_temporaries() {
    use zymbol_bytecode::Instruction;
    let compiled = compile(
        "f(a, i, j) {\n    >> (i + j) * (i - j) ¶\n    >> (i * 2 + j) % 7 ¶\n    <~ a$[(i + 1)..(j - 1)]\n}\n>> f([1, 2, 3, 4, 5], 1, 5) ¶",
    );
    let ins = &compiled.functions[0].instructions;
    let lo = ins
        .iter()
        .find_map(|i| match i {
            Instruction::ArraySlice(_, _, lo) => Some(*lo),
            _ => None,
        })
        .expect("the slice compiles to ArraySlice");
    let writes = |r| {
        ins.iter()
            .any(|i| matches!(i, Instruction::CopyReg(d, _) if *d == r))
    };
    assert!(writes(lo) && writes(lo + 1), "both bounds are written, at lo and lo + 1");
}
