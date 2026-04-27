# Zymbol-Lang — Implementation Notes

Internal reference for contributors and tooling authors: EBNF grammar, feature coverage, and execution model details.

**Interpreter version**: v0.0.4

See also: [GUIDE.md](GUIDE.md) — language guide for users  
See also: [REFERENCE.md](REFERENCE.md) — limitations and symbol table

---

## Execution Model

Zymbol has two execution strategies that produce identical output for all supported features.

| Mode | Invocation | Description |
|------|-----------|-------------|
| Tree-walker | `zymbol run file.zy` | Walks the AST directly. Default. Supports all language features. |
| Register VM | `zymbol run --vm file.zy` | Compiles to bytecode first, then executes. ~4× faster. Module system support is partial. |

All examples in GUIDE.md are verified against both modes. A feature listed as "TW only" in the coverage table below is not yet supported by the VM.

---

## Table of Contents

23. [EBNF Coverage Status](#23-ebnf-coverage-status)
A. [Normative EBNF Grammar](#appendix-a-normative-ebnf-grammar)

---

## 23. EBNF Coverage Status

The authoritative formal grammar is in [`zymbol-lang.ebnf`](zymbol-lang.ebnf) and reproduced in full in [Appendix A](#appendix-a-normative-ebnf-grammar). The table below summarizes implementation status per feature.

> **What "393/393 parity" means**: all 393 parity tests exercise features marked ✅|✅ and produce identical output in both tree-walker and VM. Tests for features marked ⚠ (VM unsupported) or `—` (VM not applicable) run against the tree-walker only and are not part of the parity count.
>
> **Legend**: ✅ fully supported · ⚠ tree-walker only · ❌ not implemented · `—` not applicable to this mode

| Feature | Tree-walker | VM | Notes |
|---------|:-----------:|:--:|-------|
| Variables / constants | ✅ | ✅ | |
| Primitive types | ✅ | ✅ | |
| String interpolation (any context) | ✅ | ✅ | Sprint 5I |
| Multi-item output `>>` | ✅ | ✅ | All expression types valid |
| Input with prompt | ✅ | ✅ | |
| Arithmetic / comparison / logical | ✅ | ✅ | |
| Compound assignment operators | ✅ | ✅ | |
| if / else-if / else | ✅ | ✅ | |
| match (literal, range, wildcard) | ✅ | ✅ | |
| match comparison pattern `< expr` | ✅ | ✅ | v0.0.4 |
| match ident pattern (scalar/array) | ✅ | ✅ | v0.0.4 |
| match list pattern `[a, b, _]` (structural) | ✅ | ✅ | v0.0.3 |
| match list pattern `[v1, v2]` (containment) | ✅ | ✅ | v0.0.4 |
| match identifier binding | ❌ | ❌ | Not implemented |
| Loops (all types) | ✅ | ✅ | |
| Range with step and reverse | ✅ | ✅ | Sprint 5I |
| Labeled loops | ✅ | ✅ | Sprint 5I |
| Functions + output params | ✅ | ✅ | |
| Lambdas / closures | ✅ | ✅ | |
| Arrays (full CRUD) | ✅ | ✅ | |
| `arr[i] = val` (direct update) | ✅ | ✅ | Sprint 5I |
| `arr[i] += val` (compound indexed update) | ✅ | ✅ | |
| `arr[i>j]` scalar deep access | ✅ | ✅ | v0.0.4 |
| `arr[p;q]` flat extraction | ✅ | ✅ | v0.0.4 |
| `arr[[g];[g]]` structured extraction | ✅ | ✅ | v0.0.4 |
| `arr[i>r1..r2]` range on last step | ✅ | ✅ | v0.0.4 |
| `arr[r1..r2>j]` range fan-out | ✅ | ✅ | v0.0.4 |
| Computed nav atoms `(expr)` | ✅ | ✅ | v0.0.4 |
| Negative nav indices `-1` | ✅ | ✅ | v0.0.4 |
| Tuple immutability (`t[i]=val` → runtime error) | ✅ | ✅ | |
| Named tuples | ✅ | ✅ | |
| HOF: map / filter / reduce | ✅ | ✅ | |
| Pipe `\|>` | ✅ | ✅ | |
| Error handling (full) | ✅ | ✅ | |
| Typed catch `:! ##Type` | ✅ | ✅ | |
| Modules (functions via `::`) | ✅ | ✅ | |
| Modules (constants via `.`) | ✅ | ✅ | Fixed in v0.0.4 (~~L3~~) |
| Advanced string operators (`$+`, `$-`, `$~~`, `$??`) | ✅ | ✅ | |
| Tuple `$+` append | ✅ | ✅ | Fixed v0.0.4 audit: `ArrayPush` extended to handle `Value::Tuple` |
| String split `$/` | ✅ | ✅ | Corrected v0.0.4 audit — was incorrectly marked ⚠ |
| String build `$++` | ✅ | ✅ | Corrected v0.0.4 audit — was incorrectly marked ⚠ |
| Numeric eval `#\|x\|` (ASCII + Unicode) | ✅ | ✅ | Unicode normalization via digit_blocks |
| Type metadata `x#?` | ✅ | ✅ | |
| Precision `#.N` / `#!N` | ✅ | ✅ | |
| Casts `##.` / `###` / `##!` | ✅ | ✅ | Corrected v0.0.4 audit — was incorrectly marked ⚠ |
| Format `#,\|x\|` / `#^\|x\|` | ✅ | ✅ | Corrected v0.0.4 audit — was incorrectly marked ⚠ |
| Base literals / conversions | ✅ | ✅ | |
| BashExec / Execute script | ✅ | ✅ | |
| CLI args capture `><` | ✅ | — | VM not supported |
| Negative array indices | ✅ | ✅ | `arr[-1]` normalized in both modes (v0.0.2) |
| Destructuring assignment | ✅ | ✅ | `[a, b] = arr`, `(name: n) = t` (v0.0.2) |
| Named functions as first-class values | ✅ | ✅ | Fixed v0.0.4 audit: identifier → `MakeFunc`; fns with outer-scope captures remain TW-only |
| `$!` error check | ✅ | ✅ | Fixed v0.0.4 audit: `Value::Error` variant + real `IsError` check |
| `$!!` error propagation | ✅ | ✅ | Fixed v0.0.4 audit: `Expr::ErrorPropagate` compiled (IsError + branch + Return) |
| Times loop `@ N { }` | ✅ | ✅ | Int condition evaluated once → repeat exactly N times |
| `do-while ~>` (post-cond loop) | ❌ | ❌ | Not implemented — EBNF spec, planned |

---

## Appendix A. Normative EBNF Grammar

Source file: [`zymbol-lang.ebnf`](zymbol-lang.ebnf) — version 2.3.0, sprint v0.0.4_1.

All rules below are implemented unless explicitly marked `[NOT IMPLEMENTED]`. Rules marked `[WT only]` are tree-walker only; the register VM (`--vm`) does not support them.

```ebnf
(*
  Zymbol-Lang EBNF Grammar
  Version: 2.3.0 — Sprint v0.0.4_1
  No keywords — pure symbolic syntax with full Unicode support.
  All rules implemented unless marked [NOT IMPLEMENTED].
*)

(* =========================================================================
   PROGRAM STRUCTURE
   ========================================================================= *)

program        = [ module_decl ] ,
                 { import_stmt } ,
                 [ export_block ] ,
                 { statement } ;

module_decl    = "#" , identifier ;

export_block   = "#>" , "{" , { export_item , [ "," ] } , "}" ;

export_item    = identifier
               | identifier , "<=" , identifier
               | identifier , "::" , identifier
               | identifier , "." , identifier
               | identifier , "::" , identifier , "<=" , identifier
               | identifier , "." , identifier , "<=" , identifier ;

import_stmt    = "<#" , import_path , "<=" , identifier ;

import_path    = [ "./" | "../" | { "../" } ] ,
                 identifier ,
                 { "/" , identifier } ;

(* =========================================================================
   STATEMENTS
   ========================================================================= *)

statement      = ( assignment
               | indexed_assign
               | destructure_assign
               | const_decl
               | update_op
               | output_stmt
               | input_stmt
               | cli_args_capture
               | numeral_mode_stmt
               | newline_stmt
               | if_stmt
               | match_stmt
               | loop_stmt
               | break_stmt
               | continue_stmt
               | try_stmt
               | function_def
               | return_stmt
               | lifetime_end
               | expr_stmt ) , [ ";" ] ;

(* Semicolons are optional — statements may be separated by whitespace *)

assignment     = identifier , "=" , expr ;

indexed_assign = identifier , { "[" , expr , "]" } , "=" , expr ;

destructure_assign = array_destructure_pattern  , "=" , expr
                   | tuple_destructure_pattern , "=" , expr ;

array_destructure_pattern  = "[" , destructure_item , { "," , destructure_item } , "]" ;
tuple_destructure_pattern  = positional_destr_pattern | named_destr_pattern ;
positional_destr_pattern   = "(" , destructure_item , { "," , destructure_item } , ")" ;
named_destr_pattern        = "(" , named_destr_field , { "," , named_destr_field } , ")" ;
named_destr_field          = identifier , ":" , identifier ;

destructure_item = identifier
                 | "*" , identifier
                 | "_" ;

const_decl     = identifier , ":=" , expr ;

(* x += n  →  x = x + n  etc. *)
update_op      = identifier , ( "+=" | "-=" | "*=" | "/=" | "%=" | "^=" ) , expr
               | identifier , ( "++" | "--" ) ;

output_stmt    = ">>" , primary_item , { primary_item } ;

primary_item   = grouped_expr | literal | identifier | function_call
               | bash_exec_expr | script_exec_expr
               | numeric_eval_expr | numeric_cast_expr | format_expr
               | base_conv_expr | round_expr | trunc_expr ;
(* Postfix operators (e.g. $#, #?) require parentheses in >>: >> (arr$#) ¶ *)

(* ¶ = pilcrow U+00B6 or \\ (double backslash) *)
newline_stmt   = ( "¶" | "\\\\" ) ;

input_stmt     = "<<" , [ string_literal ] , identifier ;

cli_args_capture = "><" , identifier ;   (* tree-walker only *)

numeral_mode_stmt = "#" , numeral_zero , numeral_nine , "#" ;
numeral_zero      = ? "0" digit of a Unicode decimal-digit block ? ;
numeral_nine      = ? "9" digit of the same block as numeral_zero ? ;

lifetime_end   = "\\" , identifier ;

expr_stmt      = expr ;

(* =========================================================================
   CONTROL FLOW
   ========================================================================= *)

if_stmt        = "?" , expr , block ,
                 { "_?" , expr , block } ,
                 [ "_" , block ] ;

block          = "{" , { statement } , "}" ;

match_stmt     = "??" , expr , "{" , { match_case } , "}" ;

match_case     = pattern , ":" , ( expr | block ) ;

pattern        = range_pattern
               | comparison_pattern
               | ident_pattern
               | list_pattern
               | literal_pattern
               | wildcard_pattern ;

literal_pattern     = literal ;
range_pattern       = range_bound , ".." , range_bound ;
range_bound         = literal | identifier ;
comparison_pattern  = ( "<" | ">" | "<=" | ">=" | "==" | "<>" ) , expr ;
ident_pattern       = identifier ;
wildcard_pattern    = "_" ;
list_pattern        = "[" , [ pattern , { "," , pattern } ] , "]" ;

(* [NOT IMPLEMENTED] binding pattern: identifier ":" expr *)

(* =========================================================================
   LOOPS
   ========================================================================= *)

loop_stmt      = loop_head , [ loop_spec ] , block ;
loop_head      = "@:label"   (* labeled: @:name, e.g. @:outer *)
               | "@" ;       (* unlabeled *)

loop_spec      = identifier , ":" , iterable
               | expr ;
(* Int n > 0 → TIMES loop (n executions, condition evaluated once)
   Bool expr → WHILE loop (re-evaluated each iteration)
   "@" or "@:name" alone → infinite loop *)

iterable       = range_expr | expr ;

break_stmt     = "@:label!"  (* labeled break:    @:name!  (v0.0.4) *)
               | "@!" ;      (* unlabeled break *)
continue_stmt  = "@:label>"  (* labeled continue: @:name>  (v0.0.4) *)
               | "@>" ;      (* unlabeled continue *)

(* =========================================================================
   ERROR HANDLING
   ========================================================================= *)

try_stmt       = "!?" , block ,
                 { catch_clause } ,
                 [ finally_clause ] ;

catch_clause   = ":!" , [ error_type ] , block ;
error_type     = "##" , identifier ;
(* Built-in: ##IO  ##Network  ##Parse  ##Index  ##Type  ##Div  ##_ (generic)
   _err is bound to the error value inside :! blocks *)

finally_clause = ":>" , block ;

(* =========================================================================
   FUNCTIONS
   ========================================================================= *)

function_def   = identifier , "(" , [ param_list ] , ")" , func_block ;

param_list     = param , { "," , param } ;
param          = identifier
               | identifier , "~"
               | identifier , "<~" ;

func_block     = "{" , { statement } , "}" ;

return_stmt    = "<~" , [ expr ] ;

lambda         = simple_lambda | block_lambda ;
simple_lambda  = lambda_params , "->" , expr ;
block_lambda   = lambda_params , "->" , func_block ;
lambda_params  = identifier
               | "(" , identifier , { "," , identifier } , ")" ;

(* =========================================================================
   EXPRESSIONS — OPERATOR PRECEDENCE (lowest → highest)
   =========================================================================
   1.  |>   pipe
   2.  ||   logical or
   3.  &&   logical and
   4.  == <> equality
   5.  < > <= >= comparison
   6.  + -  addition
   7.  * / % multiplication
   8.  ^    exponentiation (right-associative)
   9.  ! - + unary
   10. .    member access
   11. [] () $op #?   postfix
   ========================================================================= *)

expr           = pipe_expr ;

pipe_expr      = logic_or , { "|>" , pipe_call } ;

pipe_call      = ( identifier | lambda ) , [ "(" , pipe_args , ")" ] ;
(* Omitting ( pipe_args ) is implicit first-position: x |> f  ≡  f(x)  (v0.0.4).
   Named functions are first-class (v0.0.4): fn = myFunc; arr$> myFunc *)

pipe_args      = pipe_arg , { "," , pipe_arg } ;
pipe_arg       = "_" | base_expr ;

base_expr      = pipe_expr ;

logic_or       = logic_and , { "||" , logic_and } ;
logic_and      = equality  , { "&&" , equality } ;
equality       = comparison , { ( "==" | "<>" ) , comparison } ;
comparison     = addition  , { ( "<" | ">" | "<=" | ">=" ) , addition } ;
addition       = multiplication , { ( "+" | "-" ) , multiplication } ;
multiplication = exponentiation , { ( "*" | "/" | "%" ) , exponentiation } ;
exponentiation = unary , { "^" , unary } ;

unary          = [ "!" | "-" | "+" ] , member_access_expr ;

member_access_expr = postfix_expr , { "." , identifier } ;

postfix_expr   = primary_expr ,
                 { index_suffix
                 | call_suffix
                 | collection_op_suffix
                 | error_op_suffix
                 | type_metadata_suffix } ;

index_suffix   = "[" , expr , "]"
               | nav_index_suffix ;
call_suffix    = "(" , [ arg_list ] , ")" ;

arg_list       = arg , { "," , arg } ;
arg            = expr | identifier , "<~" ;

(* =========================================================================
   PRIMARY EXPRESSIONS
   ========================================================================= *)

primary_expr   = literal
               | identifier
               | array_literal
               | positional_tuple
               | named_tuple
               | match_stmt
               | lambda
               | grouped_expr
               | numeric_eval_expr
               | numeric_cast_expr
               | format_expr
               | base_conv_expr
               | round_expr
               | trunc_expr
               | bash_exec_expr
               | script_exec_expr
               | function_call ;
(* #? is postfix — NOT a primary; ranges only valid inside loop_spec *)

grouped_expr   = "(" , expr , ")" ;

(* =========================================================================
   COLLECTIONS
   ========================================================================= *)

array_literal    = "[" , [ expr , { "," , expr } ] , "]" ;
positional_tuple = "(" , expr , "," , expr , { "," , expr } , ")" ;
named_tuple      = "(" , named_field , { "," , named_field } , ")" ;
named_field      = identifier , ":" , expr ;

range_expr     = range_bound , ".." , range_bound , [ ":" , range_bound ] ;
(* Both ends inclusive. Step with ":" suffix. Reverse: high..low *)

function_call  = callable , "(" , [ arg_list ] , ")" ;
callable       = identifier
               | module_call
               | function_call
               | index_suffix
               | member_access_expr ;

module_call    = identifier , "::" , identifier ;

(* =========================================================================
   COLLECTION OPERATORS  (all postfix, all return new collection)
   ========================================================================= *)

collection_op_suffix
               = "$#"                                              (* length *)
               | "$+" , expr                                       (* append *)
               | "$+[" , expr , "]" , expr                        (* insert at index *)
               | "$-" , expr                                       (* remove first by value *)
               | "$--" , expr                                      (* remove all by value *)
               | "$-[" , [ expr ] , ".." , [ expr ] , "]"         (* remove range *)
               | "$-[" , expr , ":" , expr , "]"                  (* remove count-based *)
               | "$?" , expr                                       (* contains → Bool *)
               | "$??" , expr                                      (* all indices of value → [Int] *)
               | "$[" , [ expr ] , ".." , [ expr ] , "]"          (* slice *)
               | "$[" , expr , ":" , expr , "]"                   (* slice count-based *)
               | "$^+"                                             (* sort ascending *)
               | "$^-"                                             (* sort descending *)
               | "$^"  , lambda                                    (* sort with comparator *)
               | "$>" , lambda                                     (* map *)
               | "$|" , lambda                                     (* filter *)
               | "$<" , "(" , expr , "," , lambda , ")"           (* reduce *)
               | "$[" , expr , "]" , "$~" , expr                  (* functional update *)
               | "$~~" , "[" , expr , ":" , expr , [ ":" , expr ] , "]"  (* string replace *)
               | "$/" , expr                                       (* string split [WT only] *)
               | "$++" , { primary_expr } ;                       (* concat-build [WT only] *)

(* =========================================================================
   MULTI-DIMENSIONAL INDEXING  (v0.0.4)
   =========================================================================
   Inside [...], ">" is a depth separator, never a comparison operator.
   ========================================================================= *)

nav_index_suffix
               = "[" , nav_single_bracket , "]"
               | "[" , nav_double_bracket , "]" ;

nav_single_bracket = nav_path , { ";" , nav_path } ;

nav_double_bracket = struct_group
                   | struct_group , { ";" , struct_group } ;

struct_group   = "[" , nav_path , { "," , nav_path } , "]" ;
nav_path       = nav_step , { ">" , nav_step } ;
nav_step       = nav_atom , [ ".." , nav_atom ] ;
nav_atom       = integer
               | "-" , integer
               | identifier
               | "(" , expr , ")" ;

error_op_suffix = "$!" | "$!!" ;

(* =========================================================================
   DATA OPERATORS
   ========================================================================= *)

numeric_eval_expr = "#|" , expr , "|" ;

numeric_cast_expr = "##." , primary_expr    (* Int/Float → Float *)
                  | "###" , primary_expr    (* Float → Int, round *)
                  | "##!" , primary_expr ;  (* Float → Int, truncate *)

type_metadata_suffix = "#?" ;

format_expr    = format_kind , [ precision_mod ] , "|" , expr , "|" ;
format_kind    = "#," | "#^" ;
precision_mod  = "." , precision | "!" , precision ;

round_expr     = "#." , precision , "|" , expr , "|" ;
trunc_expr     = "#!" , precision , "|" , expr , "|" ;
precision      = digit , { digit } ;

base_conv_expr = base_prefix , "|" , expr , "|" ;
base_prefix    = "0b" | "0o" | "0d" | "0x" ;

(* =========================================================================
   SHELL INTEGRATION
   ========================================================================= *)

bash_exec_expr   = "<\\" , { bash_content } , "\\>" ;
bash_content     = bash_interpolation | ? any character except \> ? ;
bash_interpolation = "{" , identifier , "}" ;

script_exec_expr = "</" , script_path , "/>" ;
script_path    = [ "./" | "../" , { "../" } ] ,
                 identifier ,
                 { "/" , identifier } ,
                 ".zy" ;

(* =========================================================================
   LITERALS
   ========================================================================= *)

literal        = integer | float | string_literal | char_literal
               | boolean | base_char_literal ;

integer        = [ "-" ] , digit , { digit } ;

float          = [ "-" ] , digit , { digit } , "." , digit , { digit } , [ exponent ]
               | [ "-" ] , digit , { digit } , exponent ;
exponent       = ( "e" | "E" ) , [ "+" | "-" ] , digit , { digit } ;

string_literal = '"' , { string_part } , '"' ;
string_part    = string_char | "{" , identifier , "}" ;
string_char    = ? any Unicode character except " { \ ?
               | escape_sequence ;
escape_sequence = "\\n" | "\\t" | '\\"' | "\\'" | "\\\\" | "\\{" | "\\}" ;

char_literal   = "'" , ( char_content | escape_sequence ) , "'" ;
char_content   = ? any Unicode character except ' and \ ? ;

base_char_literal = "0b" , binary_digit , { binary_digit }
                  | "0o" , octal_digit  , { octal_digit }
                  | "0d" , digit        , { digit }
                  | "0x" , hex_digit    , { hex_digit } ;

boolean        = "#1" | "#0" ;

(* =========================================================================
   LEXICAL ELEMENTS
   ========================================================================= *)

identifier     = id_start , { id_continue } ;
id_start       = unicode_letter | "_" ;
id_continue    = unicode_letter | unicode_digit | "_" ;
unicode_letter = ? Unicode letter (Lu Ll Lt Lm Lo) or emoji ? ;
unicode_digit  = ? Unicode decimal digit (Nd) ? ;

digit        = "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" ;
binary_digit = "0" | "1" ;
octal_digit  = "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" ;
hex_digit    = digit | "A".."F" | "a".."f" ;

line_comment   = "//" , { ? any character except newline ? } ;
block_comment  = "/*" , { ? any character ? - "*/" } , "*/" ;
(* Block comments support nesting *)

whitespace     = " " | "\t" | "\r" | "\n" ;
(* Whitespace is insignificant except as a token separator.
   Exception: @label — @ and identifier fused into a single AtLabel token. *)

(* =========================================================================
   NOT IMPLEMENTED
   ========================================================================= *)

(* [NOT IMPLEMENTED] Post-condition loop (do-while):
   block_post_cond = "{" , { statement } , "}" , "~>" , expr ;
*)

(* [NOT IMPLEMENTED] $!! error propagation from lambdas — named functions only *)

(* [NOT IMPLEMENTED] Match multi-value arm:
   multi_value_arm = expr , { "," , expr } , ":" , ( expr | block ) ;
*)

(* [NOT IMPLEMENTED] Match identifier binding:
   binding_pattern = identifier , ":" , expr ;
*)

(* =========================================================================
   RETIRED OPERATORS
   ========================================================================= *)

(* [RETIRED v0.0.2] $++[i] insert at position → replaced by $+[i] *)
(* [RETIRED v0.0.2] $--[pos:count] remove by position+count → replaced by $-[pos..end] *)
```
