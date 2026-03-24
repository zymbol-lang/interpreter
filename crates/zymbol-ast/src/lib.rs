//! Abstract Syntax Tree for Zymbol-Lang
//!
//! Phase 0: Only supports output statements with string literals

#[cfg(test)]
use zymbol_common::Literal;

use zymbol_span::Span;

mod literals;
pub use literals::LiteralExpr;

mod io;
pub use io::{Input, InputPrompt, Newline, Output};

mod if_stmt;
pub use if_stmt::{ElseIfBranch, IfStmt};

mod loops;
pub use loops::{Break, Continue, Loop};

mod match_stmt;
pub use match_stmt::{MatchCase, MatchExpr, Pattern};

mod variables;
pub use variables::{Assignment, ConstDecl, LifetimeEnd, DestructureAssign, DestructurePattern, DestructureItem};

mod functions;
pub use functions::{FunctionDecl, LambdaBody, LambdaExpr, Parameter, ParameterKind, ReturnStmt};

mod collections;
pub use collections::{ArrayLiteralExpr, NamedTupleExpr, TupleExpr};

mod collection_ops;
pub use collection_ops::{
    CollectionLengthExpr, CollectionAppendExpr,
    CollectionInsertExpr,
    CollectionRemoveValueExpr, CollectionRemoveAllExpr,
    CollectionRemoveAtExpr, CollectionRemoveRangeExpr,
    CollectionContainsExpr, CollectionFindAllExpr,
    CollectionUpdateExpr, CollectionSliceExpr,
    CollectionMapExpr, CollectionFilterExpr, CollectionReduceExpr,
    CollectionSortExpr,
};

mod string_ops;
pub use string_ops::StringReplaceExpr;

mod data_ops;
pub use data_ops::{
    NumericEvalExpr, TypeMetadataExpr, FormatExpr, FormatPrefix,
    BaseConversionExpr, BasePrefix, RoundExpr, TruncExpr,
};

mod expressions;
pub use expressions::{BinaryExpr, UnaryExpr, PipeExpr, PipeArg};

mod script_exec;
pub use script_exec::{ExecuteExpr, BashExecExpr};

mod modules;
pub use modules::{ModuleDecl, ExportBlock, ExportItem, ItemType, ImportStmt, ModulePath};

mod error_handling;
pub use error_handling::{
    TryStmt, CatchClause, ErrorType, FinallyClause,
    ErrorCheckExpr, ErrorPropagateExpr,
};

/// A complete Zymbol program
#[derive(Debug, Clone)]
pub struct Program {
    pub module_decl: Option<ModuleDecl>,
    pub imports: Vec<ImportStmt>,
    pub statements: Vec<Statement>,
}

/// A statement in Zymbol
#[derive(Debug, Clone)]
pub enum Statement {
    /// Output statement: >> expr1 expr2 ...
    Output(Output),
    /// Assignment statement: name = expr
    Assignment(Assignment),
    /// Constant declaration: name := expr (immutable)
    ConstDecl(ConstDecl),
    /// Destructure assignment: [a, b] = expr / (name: n) = expr
    DestructureAssign(DestructureAssign),
    /// Lifetime end: \variable (explicit destruction)
    LifetimeEnd(LifetimeEnd),
    /// Input statement: << variable
    Input(Input),
    /// If statement: ? expr { } _ { }
    If(IfStmt),
    /// Loop statement: @ condition { }
    Loop(Loop),
    /// Break statement: @!
    Break(Break),
    /// Continue statement: @>
    Continue(Continue),
    /// Try statement: !? { } :! { } :> { }
    Try(TryStmt),
    /// Newline statement: ¶ or \\
    Newline(Newline),
    /// Function declaration: name(params) { }
    FunctionDecl(FunctionDecl),
    /// Return statement: <~ expr
    Return(ReturnStmt),
    /// Match statement: ?? expr { cases } (as statement, not expression)
    Match(MatchExpr),
    /// Expression statement: expr (evaluated for side effects, result discarded)
    Expr(ExprStatement),
    /// CLI args capture: ><variable
    CliArgsCapture(CliArgsCaptureStmt),
}

/// Expression statement: expr (evaluated for side effects, result discarded)
/// Used for function calls without assignment: println("Hello")
#[derive(Debug, Clone)]
pub struct ExprStatement {
    pub expr: Expr,
    pub span: Span,
}



/// A block of statements
#[derive(Debug, Clone)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub span: Span,
}

/// An expression
#[derive(Debug, Clone)]
pub enum Expr {
    /// Literal value (strings, integers, booleans)
    Literal(LiteralExpr),
    /// Identifier (variable reference)
    Identifier(IdentifierExpr),
    /// Binary expression: left op right
    Binary(BinaryExpr),
    /// Unary expression: op operand
    Unary(UnaryExpr),
    /// Range expression: start..end
    Range(RangeExpr),
    /// Array literal: [expr1, expr2, ...]
    ArrayLiteral(ArrayLiteralExpr),
    /// Tuple expression: (expr1, expr2, ...) - positional tuple
    Tuple(TupleExpr),
    /// Named tuple expression: (name: expr, name2: expr2, ...)
    NamedTuple(NamedTupleExpr),
    /// Member access expression: object.field
    MemberAccess(MemberAccessExpr),
    /// Index expression: array[index]
    Index(IndexExpr),
    /// Function call: name(args)
    FunctionCall(FunctionCallExpr),
    /// Match expression: ?? expr { cases }
    Match(MatchExpr),
    /// Collection length: collection$#
    CollectionLength(CollectionLengthExpr),
    /// Collection append: collection$+ element
    CollectionAppend(CollectionAppendExpr),
    /// Collection insert: collection$+[index] element
    CollectionInsert(CollectionInsertExpr),
    /// Collection remove value: collection$- value (removes first occurrence)
    CollectionRemoveValue(CollectionRemoveValueExpr),
    /// Collection remove all: collection$-- value (removes all occurrences)
    CollectionRemoveAll(CollectionRemoveAllExpr),
    /// Collection remove at: collection$-[index]
    CollectionRemoveAt(CollectionRemoveAtExpr),
    /// Collection remove range: collection$-[start..end]
    CollectionRemoveRange(CollectionRemoveRangeExpr),
    /// Collection contains: collection$? element
    CollectionContains(CollectionContainsExpr),
    /// Collection find all: collection$?? value - returns array of indices
    CollectionFindAll(CollectionFindAllExpr),
    /// Collection update: collection[index]$~ value
    CollectionUpdate(CollectionUpdateExpr),
    /// Collection slice: collection$[start..end]
    CollectionSlice(CollectionSliceExpr),
    /// String replace: string$~~[pattern:replacement:count?] - replace pattern with replacement
    StringReplace(StringReplaceExpr),
    /// Numeric evaluation: #|expr| - safe string to number conversion
    NumericEval(NumericEvalExpr),
    /// Type metadata: expr? - returns (type, count, value) tuple
    TypeMetadata(TypeMetadataExpr),
    /// Format expression: e|expr| or c|expr| - display formatting
    Format(FormatExpr),
    /// Base conversion expression: 0x|expr| - tridirectional conversion
    BaseConversion(BaseConversionExpr),
    /// Lambda expression: x -> expr or (a, b) -> expr
    Lambda(LambdaExpr),
    /// Collection map: collection$> (x -> x * 2)
    CollectionMap(CollectionMapExpr),
    /// Collection filter: collection$| (x -> x > 0)
    CollectionFilter(CollectionFilterExpr),
    /// Collection reduce: collection$< (0, (acc, x) -> acc + x)
    CollectionReduce(CollectionReduceExpr),
    /// Collection sort ascending: collection$^+ (natural order, no comparator)
    CollectionSortAsc(CollectionSortExpr),
    /// Collection sort descending: collection$^- (natural order, no comparator)
    CollectionSortDesc(CollectionSortExpr),
    /// Collection sort with custom comparator: collection$^ (a, b -> a.field < b.field)
    CollectionSortCustom(CollectionSortExpr),
    /// Pipe expression: value |> func(_) or value |> (x -> x * 2)(_)
    Pipe(PipeExpr),
    /// Execute expression: </ file.zy /> - execute .zy file and capture output
    Execute(ExecuteExpr),
    /// Bash execute expression: <\ command \> - execute bash command and capture output
    BashExec(BashExecExpr),
    /// Round expression: #.N|expr| - round to N decimal places
    Round(RoundExpr),
    /// Truncate expression: #!N|expr| - truncate to N decimal places
    Trunc(TruncExpr),
    /// Error check expression: expr$! - returns #1 if error, #0 otherwise
    ErrorCheck(ErrorCheckExpr),
    /// Error propagate expression: expr$!! - propagates error to caller
    ErrorPropagate(ErrorPropagateExpr),
}



/// Identifier expression
#[derive(Debug, Clone)]
pub struct IdentifierExpr {
    pub name: String,
    pub span: Span,
}


/// Range expression: start..end or start..end:step
#[derive(Debug, Clone)]
pub struct RangeExpr {
    pub start: Box<Expr>,
    pub end: Box<Expr>,
    pub step: Option<Box<Expr>>,  // Optional step value (e.g., 1..10:2)
    pub span: Span,
}

/// Member access expression: object.field
/// Used for both module member access (math.PI) and named tuple field access (person.name)
#[derive(Debug, Clone)]
pub struct MemberAccessExpr {
    pub object: Box<Expr>,
    pub field: String,
    pub span: Span,
}

/// Index expression: array[index]
#[derive(Debug, Clone)]
pub struct IndexExpr {
    pub array: Box<Expr>,
    pub index: Box<Expr>,
    pub span: Span,
}


/// Function call expression: expr(args) - can be any expression
#[derive(Debug, Clone)]
pub struct FunctionCallExpr {
    pub callable: Box<Expr>,  // The expression being called (identifier, lambda, chained call, etc.)
    pub arguments: Vec<Expr>,
    pub span: Span,
}

impl IdentifierExpr {
    pub fn new(name: String, span: Span) -> Self {
        Self { name, span }
    }
}

impl ExprStatement {
    pub fn new(expr: Expr, span: Span) -> Self {
        Self { expr, span }
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

impl FunctionCallExpr {
    pub fn new(callable: Box<Expr>, arguments: Vec<Expr>, span: Span) -> Self {
        Self {
            callable,
            arguments,
            span,
        }
    }
}

impl Block {
    pub fn new(statements: Vec<Statement>, span: Span) -> Self {
        Self { statements, span }
    }
}

impl RangeExpr {
    pub fn new(start: Box<Expr>, end: Box<Expr>, span: Span) -> Self {
        Self { start, end, step: None, span }
    }

    pub fn with_step(start: Box<Expr>, end: Box<Expr>, step: Box<Expr>, span: Span) -> Self {
        Self { start, end, step: Some(step), span }
    }
}

impl MemberAccessExpr {
    pub fn new(object: Box<Expr>, field: String, span: Span) -> Self {
        Self { object, field, span }
    }
}

impl IndexExpr {
    pub fn new(array: Box<Expr>, index: Box<Expr>, span: Span) -> Self {
        Self { array, index, span }
    }
}




impl Expr {
    /// Get the span of an expression
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal(lit) => lit.span,
            Expr::Identifier(ident) => ident.span,
            Expr::Binary(binary) => binary.span,
            Expr::Unary(unary) => unary.span,
            Expr::Range(range) => range.span,
            Expr::ArrayLiteral(arr) => arr.span,
            Expr::Tuple(tuple) => tuple.span,
            Expr::NamedTuple(named_tuple) => named_tuple.span,
            Expr::MemberAccess(member) => member.span,
            Expr::Index(idx) => idx.span,
            Expr::FunctionCall(call) => call.span,
            Expr::Match(match_expr) => match_expr.span,
            Expr::CollectionLength(op) => op.span,
            Expr::CollectionAppend(op) => op.span,
            Expr::CollectionInsert(op) => op.span,
            Expr::CollectionRemoveValue(op) => op.span,
            Expr::CollectionRemoveAll(op) => op.span,
            Expr::CollectionRemoveAt(op) => op.span,
            Expr::CollectionRemoveRange(op) => op.span,
            Expr::CollectionContains(op) => op.span,
            Expr::CollectionFindAll(op) => op.span,
            Expr::CollectionUpdate(op) => op.span,
            Expr::CollectionSlice(op) => op.span,
            Expr::StringReplace(op) => op.span,
            Expr::NumericEval(op) => op.span,
            Expr::TypeMetadata(op) => op.span,
            Expr::Format(op) => op.span,
            Expr::BaseConversion(op) => op.span,
            Expr::Lambda(lambda) => lambda.span,
            Expr::CollectionMap(op) => op.span,
            Expr::CollectionFilter(op) => op.span,
            Expr::CollectionReduce(op) => op.span,
            Expr::CollectionSortAsc(op) => op.span,
            Expr::CollectionSortDesc(op) => op.span,
            Expr::CollectionSortCustom(op) => op.span,
            Expr::Pipe(pipe) => pipe.span,
            Expr::Execute(execute) => execute.span,
            Expr::BashExec(bash) => bash.span,
            Expr::Round(round) => round.span,
            Expr::Trunc(trunc) => trunc.span,
            Expr::ErrorCheck(check) => check.span,
            Expr::ErrorPropagate(prop) => prop.span,
        }
    }
}

/// CLI args capture statement: ><variable
#[derive(Debug, Clone)]
pub struct CliArgsCaptureStmt {
    pub variable_name: String,
    pub span: Span,
}

impl Program {
    pub fn new(statements: Vec<Statement>) -> Self {
        Self {
            module_decl: None,
            imports: Vec::new(),
            statements,
        }
    }

    pub fn new_with_module(
        module_decl: Option<ModuleDecl>,
        imports: Vec<ImportStmt>,
        statements: Vec<Statement>,
    ) -> Self {
        Self {
            module_decl,
            imports,
            statements,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zymbol_span::{FileId, Position};

    fn dummy_span() -> Span {
        Span::new(
            Position::start(),
            Position::start(),
            FileId(0),
        )
    }

    #[test]
    fn test_program_creation() {
        let program = Program::new(vec![]);
        assert_eq!(program.statements.len(), 0);
    }

    #[test]
    fn test_output_statement() {
        let literal = LiteralExpr::string("Hello".to_string(), dummy_span());
        let expr = Expr::Literal(literal);
        let output = Output::new(vec![expr], dummy_span());

        assert_eq!(output.exprs.len(), 1);
        match &output.exprs[0] {
            Expr::Literal(lit) => match &lit.value {
                Literal::String(s) => assert_eq!(s, "Hello"),
                _ => panic!("Expected string literal"),
            },
            _ => panic!("Expected literal expression"),
        }
    }

    #[test]
    fn test_haskell_style_output() {
        // Test Output with multiple expressions (Haskell-style)
        let expr1 = Expr::Literal(LiteralExpr::string("Hello".to_string(), dummy_span()));
        let expr2 = Expr::Literal(LiteralExpr::string(" ".to_string(), dummy_span()));
        let expr3 = Expr::Literal(LiteralExpr::string("World".to_string(), dummy_span()));

        let output = Output::new(vec![expr1, expr2, expr3], dummy_span());
        assert_eq!(output.exprs.len(), 3);
    }

    #[test]
    fn test_assignment_statement() {
        let literal = LiteralExpr::string("value".to_string(), dummy_span());
        let expr = Expr::Literal(literal);
        let assignment = Assignment::new("x".to_string(), expr, dummy_span());

        assert_eq!(assignment.name, "x");
    }
}
