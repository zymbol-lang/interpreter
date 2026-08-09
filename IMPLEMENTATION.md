# Zymbol-Lang — Implementation Notes

Internal reference for contributors and tooling authors: EBNF grammar, feature coverage, and execution model details.

**Interpreter version**: v0.0.7

See also: [GUIDE.md](GUIDE.md) — language guide for users  
See also: [REFERENCE.md](REFERENCE.md) — limitations and symbol table

---

## Execution Model

Zymbol has two execution strategies that produce identical output for all supported features.

| Mode | Invocation | Description |
|------|-----------|-------------|
| Tree-walker | `zymbol run file.zy` | Walks the AST directly. Default. Supports all language features. |
| Register VM | `zymbol run --vm file.zy` | Compiles to bytecode first, then executes. ~4× faster. Module system support reached parity with the tree-walker in v0.0.8 (see REFERENCE.md L23 and `tests/scripts/vm_compare.sh`, 551/551). Argument-count mismatches are rejected by semantic analysis before either engine starts; the compiler's own call-site check is a backstop for API callers that skip that analysis (REFERENCE.md L28). Default engine for `.zyp` packages. |

All examples in GUIDE.md are verified against both modes. A feature listed as "TW only" in the coverage table below is not yet supported by the VM.

---

## Table of Contents

23. [EBNF Coverage Status](#23-ebnf-coverage-status)
A. [Normative EBNF Grammar](#appendix-a-normative-ebnf-grammar)

---

## 23. EBNF Coverage Status

The authoritative formal grammar is in [`zymbol-lang.ebnf`](zymbol-lang.ebnf) and reproduced in full in [Appendix A](#appendix-a-normative-ebnf-grammar). The table below summarizes implementation status per feature.

> **What "parity" means**: every parity test exercises features marked ✅|✅ and produces identical output in both tree-walker and VM. Tests for features marked ⚠ (VM unsupported) or `—` (VM not applicable) run against the tree-walker only and are not part of the parity count. The authoritative count comes from `bash tests/scripts/vm_compare.sh` (551/551 as of v0.0.8, including `tests/arity/`).
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
| match or-pattern `p1 \|\| p2` (alternatives) | ✅ | ✅ | v0.0.8 |
| match identifier binding | ❌ | ❌ | **Dismissed 2026-06-12** — bind before the match (idiom) |
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
| CLI args capture `><` | ✅ | ✅ | VM support added (`LoadCliArgs`) |
| Negative array indices | ✅ | ✅ | `arr[-1]` normalized in both modes (v0.0.2) |
| Destructuring assignment | ✅ | ✅ | `[a, b] = arr`, `(name: n) = t` (v0.0.2) |
| Named functions as first-class values | ✅ | ✅ | Fixed v0.0.4 audit: identifier → `MakeFunc`; fns with outer-scope captures remain TW-only |
| `$!` error check | ✅ | ✅ | Fixed v0.0.4 audit: `Value::Error` variant + real `IsError` check |
| `$!!` error propagation | ✅ | ✅ | Fixed v0.0.4 audit: `Expr::ErrorPropagate` compiled (IsError + branch + Return) |
| Times loop `@ N { }` | ✅ | ✅ | Int condition evaluated once → repeat exactly N times |
| Hot definition `x°` / `°x` | ✅ | ✅ | v0.0.5 — auto-init to neutral value, loop-scope anchoring |
| TUI primitives (`@~` `>>!` `>>?` `>>~` `<<\|` `<<\|?` `>>\|`) | ✅ | ✅ | v0.0.5; raw-mode ops require a TTY |
| String repeat `$*` | ✅ | ✅ | v0.0.5 |
| FatArrow `=>` (match arms, import alias, export rename) | ✅ | ✅ | v0.0.6 breaking change — replaced `:` / `<=` |
| Deep functional update `arr[i>j]$~ val` | ✅ | ✅ | v0.0.7 — `DeepSet` instruction; all `$~` forms (incl. positional tuples) route through it |
| Named-tuple update `nt[i]$~` / `nt["field"]$~` | ✅ | ✅ | v0.0.6 |
| `<<` input (prompt, numeric cast) in VM | ✅ | ✅ | v0.0.6 (`ReadLine` instruction) |
| Typed/validated input `<< ##.(T,D) "p" var` | ✅ | ✅ | v0.0.7 — re-prompts until valid; `##.` `###` `##"` `##'` typespecs |
| `std/math`, `std/random` | ✅ | ✅ | v0.0.6 — native stdlib via `<# std/<name> => alias` |
| `std/json`, `std/io`, `std/net` | ✅ | ✅ | v0.0.7 — soft `##Parse`/`##IO`/`##Network` errors |
| `std/db` (ODBC, vendor-neutral) | ✅ | ✅ | v0.0.7 — soft `##DB` errors; builtin ids 500–514 |
| `std/term` (terminal display metrics) | ✅ | ✅ | v0.0.8 — `width`/`pad_*`/`center`/`truncate`; columns via `unicode-width`; builtin ids 600–604 |
| Static undefined-function detection at `check` time | ✅ | — | v0.0.7 — semantic phase (`zymbol-semantic/type_check.rs`) |
| Static argument-count check, all call forms | ✅ | — | v0.0.8 — `f()`, `alias::f()` and `std::f()` alike, fatal before execution; arity table from `zymbol-semantic/call_arity.rs` (REFERENCE.md L28) |
| Auto-free (destruction at last use) | ✅ | ✅ | v0.0.8 — `zymbol-semantic/last_use.rs`; TW frees per statement, VM emits `LoadUnit` (temporaries not yet covered) |
| `.zyp` packages (`zymbol package`, `zymbol run pkg.zyp`) | ✅ | ✅ | v0.0.8 — CLI/format feature, no grammar surface; VM is the default engine for a `.zyp` |
| `do-while ~>` (post-cond loop) | ❌ | ❌ | **Dismissed 2026-06-12** — infinite loop + `@!` is the idiom; `~>` stays unoccupied |

---

## Appendix A. Normative EBNF Grammar

Source file: [`zymbol-lang.ebnf`](zymbol-lang.ebnf) — version 3.1.0, sprint v0.0.7.

The canonical grammar is maintained in `zymbol-lang.ebnf`; the copy below is reproduced
**verbatim** from that file (synced 2026-06-12). If they ever diverge, the file wins.

```ebnf
(*
  Zymbol-Lang EBNF Grammar
  Version:    3.1.0
  Generated:  2026-06-12
  Sprint:     v0.0.7
  Test counts: authoritative via the test suite, not this header.
    E2E / VM parity:  bash tests/scripts/vm_compare.sh
    Unit tests:       cargo test

  Source of truth: Rust implementation in interpreter/crates/
    zymbol-lexer   (TokenKind catalog)
    zymbol-parser  (parse rules — 17 source files)
    zymbol-ast     (Statement / Expr enums)

  Supersedes: zymbol-lang.ebnf v2.5.0 (aspirational, several inaccuracies)
  Deprecated: _staging/docs/GRAMMAR_VERIFIED.ebnf v2.1.0

  Changes in 3.1.0 (sprint v0.0.7) — all verified against the parser:
  [C01] input_stmt gains optional typed/validated typespec:
        << ##.(T,D) | ##. | ###(N) | ### | ##"(N) | ##" | ##'  before the prompt (v0.0.7)
  [C02] loop_head: while (@ cond) and times (@ N) loop specs were missing
        from 3.0.0 — added (both predate v0.0.5; grammar omission only)
  [C03] export_item: local rename  name => public_name  added (FatArrow, v0.0.6)
  [C04] collection_update generalized: arr[i]$~, named_tuple["field"]$~ and
        deep  arr[i>j>…]$~  — all forms run in BOTH engines (VM: DeepSet
        instruction). Ranges (..) are not allowed in a $~ path.
  [C05] hot °name corrected: valid as RHS expression too, not only LHS
  [C06] No new syntax for the v0.0.6/v0.0.7/v0.0.8 stdlib (std/math, std/random,
        std/json, std/io, std/net, std/db, std/term) — consumed via existing
        import_stmt (<# std/name => alias) and module calls (alias::fn)

  Key divergences vs v2.5.0:
  [D01] Module is a closed block: # name { ... }  (not a standalone declaration)
  [D02] >>? is an expression (Expr::TerminalSize), NOT a statement
  [D03] No guard patterns in match (_?, pattern?) — removed from parser
  [D04] ##. / ### / ##!  take a parse_postfix() operand, NOT primary_expr
  [D05] $* (StringRepeat) is new; absent in v2.5.0
  [D06] << #|var|  numeric-cast input is new
  [D07] >>~ has up to 5 sparse slots: (row, col, BKS, fg, bg)  not 4
  [D08] Hot notation °x / x° added (HotIdent / PreHotIdent tokens)
  [D09] Loop labels use @:name  (AtColonLabel), not @@name
  [D10] No \u{hex} escape sequence in strings (removed; not in v2.5.0 either but present in GRAMMAR_VERIFIED.ebnf)
  [D11] Range patterns: only int..int and char..char  (no identifier bounds)
  [D12] Module import uses => alias only  (<= alias is rejected by the parser)
  [D13] >< variable  is CLI args capture  (not § as referred to in docs)
  [D14] Output items use a restricted expression form (arithmetic only, not full expr)
*)

(* ============================================================
   SECTION 1 — PROGRAM STRUCTURE
   ============================================================ *)

(*
  Two distinct file types:
    executable  — starts with optional imports, then statements
    module      — starts with  #  and contains a single module block
*)

program =
    executable_file
  | module_file
  ;

executable_file =
    { import_stmt }
  , { statement }
  ;

(*
  A module file contains exactly ONE module block; nothing is allowed after it.
*)
module_file = module_block ;


(* ============================================================
   SECTION 2 — MODULE SYSTEM
   ============================================================ *)

(*
  Module block encloses everything: imports, optional export list, declarations.
  The module name may be relative (.name) or bare (name).
  The leading dot marks the canonical module path in the file system.
*)
module_block =
    "#" , module_name , "{"
  ,     { import_stmt }
  ,     [ export_block ]
  ,     { module_member }
  , "}"
  ;

module_name = [ "." ] , identifier ;

(*
  Only const declarations, variable declarations, and function declarations
  are allowed at module top-level.  Arbitrary statements are forbidden there
  (E013 diagnostic).
*)
module_member =
    const_decl
  | assignment_stmt
  | function_decl
  ;

(*
  Import statement: <# path => alias
  Only  =>  is accepted as the alias separator.
  The legacy  <=  alias form is rejected by the parser.     [D12]
*)
import_stmt = "<#" , module_path , "=>" , identifier ;

module_path =
    ( "./" | "../" | "/" | "~/" ) , { path_segment , "/" } , path_segment
  | path_segment , { "/" , path_segment }
  ;

path_segment = identifier ;

(*
  Export block: declares which functions are publicly visible.
  Placed inside the module block, before any member declarations.
*)
export_block = "#>" , "{" , export_item , { "," , export_item } , "}" ;

(*
  An export item is a bare name (export a local declaration) or a re-export that
  pulls a function or constant from an imported module, optionally renamed with =>.
*)
export_item =
    identifier , [ "=>" , identifier ]           (* export local declaration, optional rename (v0.0.6) *)
  | identifier , "::" , identifier , [ "=>" , identifier ]   (* re-export module fn *)
  | identifier , "." , identifier , [ "=>" , identifier ]    (* re-export module const *)
  ;


(* ============================================================
   SECTION 3 — STATEMENTS
   ============================================================ *)

(*
  Complete statement dispatch as implemented in parse_statement().
  Whitespace (including newlines) between statements is silently consumed.
  ¶  and  \\  are the ONLY tokens that produce explicit newline statements.
*)
statement =
    output_stmt              (* >> *)
  | tui_block_stmt           (* >>| { } *)
  | output_pos_stmt          (* >>~ (...) > items *)
  | clear_screen_stmt        (* >>! *)
  | input_stmt               (* << var  or  << #|var| *)
  | key_input_stmt           (* <<| var  or  <<|? var *)
  | cli_args_capture         (* >< var *)
  | if_stmt                  (* ? cond { } *)
  | match_stmt               (* ?? expr { } *)
  | loop_stmt                (* @ { }  or  @ item : iterable { } *)
  | break_stmt               (* @!  or  @:label! *)
  | continue_stmt            (* @>  or  @:label> *)
  | sleep_stmt               (* @~ duration *)
  | try_stmt                 (* !? { } :! { } :> { } *)
  | newline_stmt             (* ¶  or  \\ *)
  | lifetime_end             (* \variable *)
  | return_stmt              (* <~ [expr] *)
  | function_decl            (* name(params) { } *)
  | const_decl               (* name := expr *)
  | assignment_stmt          (* name = expr  and compound forms *)
  | destructure_assign       (* [a,b]=expr  or  (a,b)=expr *)
  | set_numeral_mode         (* #09#  (numeral-system switch) *)
  | bash_exec_stmt           (* <\ args \> *)
  | expr_stmt                (* function call as void statement *)
  ;

(*
  A block is a brace-delimited sequence of statements.
*)
block = "{" , { statement } , "}" ;


(* ============================================================
   SECTION 4 — OUTPUT STATEMENTS
   ============================================================ *)

(*
  Basic output: zero or more items space-separated on the same logical line.
  Items are terminated by ¶, \\, }, EOF, or any statement-starting token.
  Items use a restricted expression form: arithmetic (+,-,*,/,%,^) and all
  postfix operators, but NOT comparison or logical operators at top level.
  Use parentheses for full expressions: >> (a == b) ¶
*)
output_stmt = ">>" , { output_item } ;

output_item =
    output_item_add
  | interpolated_string      (* expanded inline: no extra parens needed *)
  ;

output_item_add =
    output_item_mul
  | output_item_add , "+" , output_item_mul
  | output_item_add , "-" , output_item_mul
  ;

output_item_mul =
    output_item_term
  | output_item_mul , "*" , output_item_term
  | output_item_mul , "/" , output_item_term
  | output_item_mul , "%" , output_item_term
  | unary_expr                (* -expr  or  !expr at item start *)
  ;

output_item_term =
    primary_expr , [ output_item_postfix ] , [ "^" , output_item_term ]
  ;

(*
  output_item_postfix mirrors the regular postfix chain but does NOT
  form a function call if the base is a literal  (bug fix BUG-06).
*)
output_item_postfix =
    "[" , expr , "]"                       (* indexing *)
  | nav_index                               (* deep / flat / structured extract *)
  | "." , identifier                        (* member access *)
  | "(" , arg_list , ")"                   (* call — only if base is not a literal *)
  | collection_postfix
  | string_postfix
  | data_postfix
  | error_postfix
  ;

(*
  TUI block: wraps arbitrary statements in a TUI rendering context.
*)
tui_block_stmt = ">>|" , block ;

(*
  Positioned output: moves the cursor before printing.
  The position is a sparse tuple of up to 5 optional slots.
  Absent slots are indicated by bare commas.  Items follow  > .
  Alternative form: >>~ variable > items  (variable must eval to a dense tuple).
*)
output_pos_stmt =
    ">>~" , ( sparse_pos_tuple | identifier ) , ">" , { output_item }
  ;

sparse_pos_tuple =
    "(" , pos_slot , "," , pos_slot , [ "," , pos_slot , [ "," , pos_slot , [ "," , pos_slot ] ] ] , ")"
  ;

pos_slot = [ expr ] ;    (* absent slot is just empty — the comma acts as placeholder *)

(*
  Clear screen.
*)
clear_screen_stmt = ">>!" ;

(*
  Terminal size expression (NOT a statement).    [D02]
  Evaluated inline wherever a primary expression is expected.
*)
terminal_size_expr = ">>?" ;


(* ============================================================
   SECTION 5 — INPUT STATEMENTS
   ============================================================ *)

(*
  Standard string input (stores raw string in variable).
  Numeric cast input  << #|var|  reads and immediately converts to int/float.  [D06]
  Typed/validated input (v0.0.7): an input_typespec BEFORE the prompt constrains
  and converts the value at read time; invalid input re-prompts until valid,
  end-of-input aborts.  [C01]
  Optional prompt string is printed before waiting for input.
*)
input_stmt =
    "<<" , [ input_typespec ] , [ prompt_string ] , identifier
  | "<<" , [ prompt_string ] , "#|" , identifier , "|"     (* legacy numeric cast *)
  ;

input_typespec =
    "##." , [ "(" , uint , "," , uint , ")" ]   (* Float; (T,D) = max total digits, max decimals *)
  | "###" , [ "(" , uint , ")" ]                (* Int; (N) = max digit count *)
  | '##"' , [ "(" , uint , ")" ]                (* String; (N) = max character count *)
  | "##'"                                       (* exactly one character -> Char *)
  ;

uint = digit , { digit } ;

prompt_string = string_literal | interpolated_string ;

(*
  Key input: captures a single keypress.
  <<|   blocking  — waits until a key is pressed.
  <<|?  non-blocking — returns immediately with "" if no key is queued.
*)
key_input_stmt =
    ( "<<|" | "<<|?" ) , identifier
  ;

(*
  CLI arguments capture: binds the command-line argument array to a variable.
*)
cli_args_capture = "><" , identifier ;

(*
  Newline statement: emits a line break.  Acts as statement terminator in
  output items and return values.
*)
newline_stmt = "¶" | "\\\\" ;

(*
  Numeral mode switch: changes the active digit block (Unicode numeral system).
  #09# resets to ASCII.  The two characters must be the 0 and 9 digits of the
  SAME Unicode digit block (e.g.  #०९#  for Devanagari, #০৯# for Bengali).
*)
set_numeral_mode = "#" , zero_digit , nine_digit , "#" ;
zero_digit = (* Unicode code-point whose digit_value in some block is 0 *) ;
nine_digit  = (* Unicode code-point whose digit_value in the same block is 9 *) ;


(* ============================================================
   SECTION 6 — CONTROL FLOW
   ============================================================ *)

(*
  IF statement.
  Condition is a full expression.  else-if chain uses _?; else branch uses _.
*)
if_stmt =
    "?" , expr , block
  , { "_?" , expr , block }
  , [ "_" , block ]
  ;

(*
  MATCH statement or expression.
  ?? is the match operator.  Cases are newline-separated (no commas).
  Each arm: pattern => value  or  pattern => { block }.
  An arm may have BOTH a value and a trailing block.
*)
match_stmt =
    "??" , expr , "{"
  , { match_arm }
  , "}"
  ;

match_expr = match_stmt ;    (* same syntax; context decides statement vs expression *)

match_arm =
    pattern , "=>" , ( match_arm_block | match_arm_value , [ block ] )
  ;

match_arm_block = block ;

(*
  match_arm_value uses a restricted expression form: logical ops (||, &&) and
  arithmetic are valid, but bare comparison operators (<, >, <=, >=, ==, <>)
  are NOT allowed at top level to avoid ambiguity with the next arm's
  comparison pattern.  Comparisons inside parentheses are fine.
*)
match_arm_value = arm_pipe_expr ;

(*
  TRY / CATCH / FINALLY.
  At least !? { } is required.  Multiple typed catch clauses are allowed.
  ##_ is a wildcard error type (catches any error).
*)
try_stmt =
    "!?" , block
  , { ":!" , [ "##" , ( identifier | "_" ) ] , block }
  , [ ":>" , block ]
  ;


(* ============================================================
   SECTION 7 — LOOPS
   ============================================================ *)

(*
  Infinite loop: @ { body }
  While loop:   @ cond { body }       (Bool expr, re-evaluated each iteration)
  Times loop:   @ n { body }          (Int expr > 0, evaluated ONCE -> n executions)
  For-each: @ item : iterable { body }
  For-range: @ i : start..end { body }  or  @ i : start..end:step { body }
  Labeled: @:label { body }   (the label token is @:identifier)

  Legacy fused labels (@label, merged with @ at lex time) are still accepted
  for backward compatibility but @:name is the canonical form.
*)
loop_stmt =
    loop_head , block
  ;

loop_head =
    ( "@" | loop_label ) , [ loop_spec ]
  ;

loop_label = "@:" , identifier ;    (* lexed as a single AtColonLabel token *)

loop_spec =
    loop_binding         (* for-each / for-range:  item : iterable *)
  | expr                 (* Bool -> while; positive Int -> times  [C02] *)
  ;

loop_binding = identifier , ":" , expr ;    (* item : iterable *)

(*
  Break: exits the nearest loop or the named ancestor.
  @!         — exits the nearest enclosing loop (no label).
  @:label!   — exits the loop whose header was  @:label  (fused token).
  The legacy space-separated form  @! label  was removed in v0.0.5.
*)
break_stmt =
    "@!"
  | "@:" , identifier , "!"    (* fused AtColonLabelBreak token *)
  ;

(*
  Continue: skips to the next iteration.
  @>         — continues the nearest enclosing loop.
  @:label>   — continues the loop named  label  (fused token).
  The legacy space-separated form  @> label  was removed in v0.0.5.
*)
continue_stmt =
    "@>"
  | "@:" , identifier , ">"   (* fused AtColonLabelContinue token *)
  ;

(*
  Sleep: pauses execution for the given duration.
  Duration is a full expression (evaluated to milliseconds or seconds).
*)
sleep_stmt = "@~" , expr ;


(* ============================================================
   SECTION 8 — FUNCTIONS AND LAMBDAS
   ============================================================ *)

(*
  Function declaration: name(params) { body }
  Detected by: Ident  LParen  ...  RParen  LBrace
  (parser uses 1-token look-ahead and a bracket scan to distinguish from
   function-call statements)
*)
function_decl = identifier , "(" , [ param_list ] , ")" , block ;

param_list = parameter , { "," , parameter } ;

parameter = identifier , [ param_modifier ] ;

(*
  Parameter modifiers:
    name~   — mutable: caller's copy may be modified inside the function
    name<~  — output: written back to the caller's variable on return
  Plain  name  means normal (immutable pass-by-value).
*)
param_modifier = "~" | "<~" ;

(*
  Return statement: <~ [expr]
  The return value supports juxtaposition concatenation on the same line.
  An empty <~ (followed by delimiter) returns Unit.
*)
return_stmt = "<~" , [ juxtapose_chain ] ;

(*
  Lambda expression forms:
    x -> expr                    single parameter, expression body
    (x -> expr)                  single parameter in parens (closures)
    (a, b, c) -> expr            multi-parameter, expression body
    x -> { block }               single parameter, block body
    (a, b) -> { block }          multi-parameter, block body

  When passed to HOF operators ($>, $|, $<, $^), a bare identifier is also
  accepted as a function reference (not a lambda but resolves to Expr::Identifier).
*)
lambda_expr =
    lambda_params , "->" , lambda_body
  ;

lambda_params =
    identifier                                 (* single param *)
  | "(" , identifier , { "," , identifier } , ")"  (* multi-param or single-in-parens *)
  ;

lambda_body = expr | block ;

(*
  Function call expression: callable(args...)
  callable may be any expression that evaluates to a function.
*)
function_call_expr = expr , "(" , [ call_arg_list ] , ")" ;

call_arg_list = expr , { "," , expr } ;


(* ============================================================
   SECTION 9 — VARIABLES
   ============================================================ *)

(*
  Assignment statement: name = expr  (and compound / indexed forms).
  Hot identifiers (°name  and  name°) are valid LHS for assignments.
  The RHS uses juxtaposition concatenation: s = "hello" ' ' name " world"
*)
assignment_stmt =
    lhs , "=" , juxtapose_chain        (* regular *)
  | lhs , "+=" , expr
  | lhs , "-=" , expr
  | lhs , "*=" , expr
  | lhs , "/=" , expr
  | lhs , "%=" , expr
  | lhs , "^=" , expr
  | identifier , "++"                  (* expands to  name = name + 1 *)
  | identifier , "--"                  (* expands to  name = name - 1 *)
  | identifier , "[" , expr , "]" , compound_assign_op , expr  (* indexed assignment *)
  ;

lhs = identifier | hot_identifier ;

compound_assign_op = "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "^=" ;

(*
  Constant declaration: name := expr
  Immutable after binding.  Supports juxtaposition RHS.
*)
const_decl = identifier , ":=" , juxtapose_chain ;

(*
  Destructure assignment.
  Array form: [a, b, *rest, _] = expr
  Positional tuple form: (a, b, *rest) = expr
  Named tuple form: (field1: var1, field2: var2) = expr
*)
destructure_assign =
    array_destructure_pattern , "=" , expr
  | positional_tuple_destructure , "=" , expr
  | named_tuple_destructure , "=" , expr
  ;

array_destructure_pattern =
    "[" , destructure_item , { "," , destructure_item } , "]"
  ;

positional_tuple_destructure =
    "(" , positional_destructure_item , { "," , positional_destructure_item } , ")"
  ;

named_tuple_destructure =
    "(" , identifier , ":" , identifier , { "," , identifier , ":" , identifier } , ")"
  ;

destructure_item =
    identifier                (* bind to name *)
  | "_"                       (* ignore *)
  | "*" , identifier          (* rest / spread *)
  ;

positional_destructure_item =
    identifier
  | "*" , identifier
  ;

(*
  Lifetime end: explicitly destroys a variable and frees its resources.
*)
lifetime_end = "\\" , identifier ;

(*
  Juxtaposition concatenation: same-line adjacent primary values are implicitly
  concatenated (BinaryOp::Concat).  Works for strings, chars, numbers, identifiers.
  Used by assignments, const declarations, and return statements, and — since
  v0.0.8 — inside delimited positions: call arguments, array elements, tuple
  elements and grouped expressions.

  One difference between the two contexts: a following `(` may continue the
  chain at statement level (when the accumulator is a literal or a binary
  expression), but never inside a delimited position, where a parenthesis is
  ambiguous with a lambda, a tuple and a grouped expression.
*)
juxtapose_chain = expr , { juxtapose_token } ;

(*
  Tokens that may start a juxtaposed continuation (same source line only):
  string_literal, interpolated_string, char_literal, integer, float,
  bool_literal, identifier, hot_identifier.
*)
juxtapose_token =
    string_literal
  | interpolated_string
  | char_literal
  | integer_literal
  | float_literal
  | bool_literal
  | identifier
  | hot_identifier
  ;


(* ============================================================
   SECTION 10 — EXPRESSIONS
   ============================================================ *)

(*
  Full expression hierarchy, lowest to highest precedence:
    1.  Pipe           |>
    2.  Logical OR     ||
    3.  Logical AND    &&
    4.  Comparison     ==  <>  <  >  <=  >=
    5.  Addition       +  -
    6.  Multiplication *  /  %
    7.  Power          ^   (right-associative)
    8.  Range          ..  (and ..  :step)
    9.  Postfix        []  .  ()  $ops  #ops  ::
    10. Unary          !  -  +
    11. Primary        literals  identifiers  (...)  [...]  calls  lambdas  ...
*)
expr = pipe_expr ;

(*  Pipe operator: value |> func(_)  or  value |> func  (implicit placeholder)  *)
pipe_expr =
    logic_or_expr
  | pipe_expr , "|>" , postfix_expr , [ "(" , pipe_arg_list , ")" ]
  ;

pipe_arg_list = pipe_arg , { "," , pipe_arg } ;
pipe_arg = expr | "_" ;    (* _ is a positional placeholder for the piped value *)

logic_or_expr =
    logic_and_expr
  | logic_or_expr , "||" , logic_and_expr
  ;

logic_and_expr =
    comparison_expr
  | logic_and_expr , "&&" , comparison_expr
  ;

comparison_expr =
    addition_expr
  | comparison_expr , ( "==" | "<>" | "<" | ">" | "<=" | ">=" ) , addition_expr
  ;

addition_expr =
    mul_expr
  | addition_expr , "+" , mul_expr
  | addition_expr , "-" , mul_expr
  ;

mul_expr =
    power_expr
  | mul_expr , "*" , power_expr
  | mul_expr , "/" , power_expr
  | mul_expr , "%" , power_expr
  ;

(*  Power is right-associative: 2^3^4 = 2^(3^4)  *)
power_expr =
    range_expr
  | range_expr , "^" , power_expr
  ;

(*
  Range expression: start..end  or  start..end:step
  Both endpoints are parsed at postfix level.
  Valid wherever a full expression is valid.
*)
range_expr =
    postfix_expr
  | postfix_expr , ".." , postfix_expr , [ ":" , postfix_expr ]
  ;

unary_expr =
    primary_expr
  | "!" , unary_expr
  | "-" , unary_expr
  | "+" , unary_expr
  ;

(*
  Postfix chain (applied left-to-right on a primary).
*)
postfix_expr = unary_expr , { postfix_op } ;

postfix_op =
    "[" , expr , "]"                (* regular indexing *)
  | nav_index                        (* deep / flat / structured navigation *)
  | "." , identifier                 (* member access  obj.field *)
  | "::" , identifier , [ "(" , call_arg_list , ")" ]  (* module call  alias::fn(args) *)
  | "(" , [ call_arg_list ] , ")"   (* function call *)
  | collection_postfix
  | string_postfix
  | data_postfix
  | error_postfix
  ;


(* ============================================================
   SECTION 11 — COLLECTION OPERATORS ($ postfix)
   ============================================================ *)

(*
  All collection and string $ operators are postfix, left-binding.
  They are available in both regular expressions and output items.
*)
collection_postfix =
    "$#"                                    (* length:  arr$# *)
  | "$+" , expr                            (* append:  arr$+ val *)
  | "$+[" , expr , "]" , expr              (* insert at index:  arr$+[i] val *)
  | "$-" , expr                            (* remove first occurrence:  arr$- val *)
  | collection_remove_positional           (* remove at position/range:  arr$-[...] *)
  | "$--" , expr                           (* remove all occurrences:  arr$-- val *)
  | "$?" , expr                            (* contains:  arr$? val *)
  | collection_update                      (* update element:  arr[i]$~ val *)
  | collection_slice                       (* slice:  arr$[...] *)
  | "$>" , hof_callable                    (* map:  arr$> (x -> x*2) *)
  | "$|" , hof_callable                    (* filter:  arr$| (x -> x>0) *)
  | "$<" , hof_callable                    (* reduce:  arr$< (acc x -> acc+x) *)
  | "$^+"                                  (* sort ascending  *)
  | "$^-"                                  (* sort descending *)
  | "$^" , hof_callable                   (* sort custom:  arr$^ (a b -> a > b) *)
  ;

(*
  HOF callable: lambda expression  OR  bare identifier (function reference).
*)
hof_callable = lambda_expr | identifier ;

(*
  collection_remove_positional — arr$-[index]  or  arr$-[start..end]  or  arr$-[start:count]
  All four boundary variants are supported; start/end may be negative.
*)
collection_remove_positional =
    "$-[" , expr , "]"                         (* single index:    arr$-[i] *)
  | "$-[" , expr , ".." , expr , "]"           (* explicit range:  arr$-[start..end] *)
  | "$-[" , ".." , expr , "]"                  (* open start:      arr$-[..end] *)
  | "$-[" , expr , ".." , "]"                  (* open end:        arr$-[start..] *)
  | "$-[" , ".." , "]"                         (* remove all:      arr$-[..] *)
  | "$-[" , expr , ":" , expr , "]"            (* count-based:     arr$-[start:count] *)
  ;

(*
  collection_update — requires an already-indexed LHS; returns a NEW collection.  [C04]
  The  $~  token is only valid when the left-hand expression is an index expr:
    arr[i]$~ val           — array element (1-based, negative allowed)
    tuple[i]$~ val         — positional/named tuple by index
    nt["field"]$~ val      — named tuple by field-name string (v0.0.6)
    arr[i>j>…]$~ val       — deep update through a nav path (scalar steps only;
                             ranges (..) are not allowed in a $~ path)
*)
collection_update = ( index_expr | nav_index_expr ) , "$~" , expr ;
index_expr     = postfix_expr , "[" , expr , "]" ;
nav_index_expr = postfix_expr , nav_index ;

(*
  collection_slice — arr$[start..end]  or  arr$[start:count]
  start/end may be negative (counts from end).
*)
collection_slice =
    "$[" , expr , ".." , expr , "]"            (* explicit range:  arr$[start..end] *)
  | "$[" , ".." , expr , "]"                   (* open start:      arr$[..end] *)
  | "$[" , expr , ".." , "]"                   (* open end:        arr$[start..] *)
  | "$[" , expr , ":" , expr , "]"             (* count-based:     arr$[start:count] *)
  ;


(* ============================================================
   SECTION 12 — STRING OPERATORS ($ postfix on strings)
   ============================================================ *)

string_postfix =
    "$??" , postfix_expr                        (* find all positions:  str$?? sub *)
  | "$++" , postfix_expr , ":" , postfix_expr   (* insert at position:  str$++ pos : sub *)
  | string_replace                              (* replace:  str$~~[from:to]  or  str$~~[from:to:n] *)
  | "$/" , postfix_expr                         (* split:  str$/ delim *)
  | "$*" , postfix_expr                         (* repeat:  str$* n  [D05] *)
  | string_concat_build                         (* $++ same-line juxtaposition form *)
  ;

(*
  string_replace — str$~~[pattern:replacement]  or  str$~~[pattern:replacement:count]
  count limits how many occurrences are replaced (omit to replace all).
*)
string_replace =
    "$~~[" , expr , ":" , expr , "]"            (* replace all:     str$~~[pat:rep] *)
  | "$~~[" , expr , ":" , expr , ":" , expr , "]"  (* replace first N: str$~~[pat:rep:n] *)
  ;

(*
  $++ concat-build: builds a new string by appending same-line juxtaposed items.
  No  pos : sub  argument in this form; items follow directly.
*)
string_concat_build = "$++" , postfix_expr , { postfix_expr } ;


(* ============================================================
   SECTION 13 — DATA / TYPE OPERATORS
   ============================================================ *)

(*
  Numeric cast operators (prefix, before the operand).
  The operand is parsed at postfix level  (#.  ###  ##!).         [D04]
*)
data_prefix =
    "##." , postfix_expr                   (* cast to Float *)
  | "###" , postfix_expr                   (* cast to Int  (round) *)
  | "##!" , postfix_expr                   (* cast to Int  (truncate) *)
  ;

(*
  Pipe-style operators that wrap an expression in |…|.
  The inner expression is parsed at full  expr  level.
*)
data_postfix =
    "#?"                                   (* type metadata:  expr#? *)
  ;

data_wrapped_expr =
    "#|" , expr , "|"                     (* numeric eval / cast:  #|expr| *)
  | "#,|" , expr , "|"                   (* element count *)
  | "#^|" , expr , "|"                   (* maximum value *)
  | "#." , integer_literal , "|" , expr , "|"  (* round to N decimals: #.N|expr| *)
  | "#!" , integer_literal , "|" , expr , "|"  (* format to N decimal places: #!N|expr| *)
  ;

(*
  Base conversion: integer literals with explicit radix prefix.
  These are lexed as Integer tokens with the appropriate value.
*)
base_literal =
    "0x" , hex_digits      (* hexadecimal *)
  | "0b" , bin_digits      (* binary *)
  | "0o" , oct_digits      (* octal *)
  ;

(*
  Error check / propagate postfix.
*)
error_postfix =
    "$!"                   (* error check:  expr$!  (returns bool) *)
  | "$!!"                  (* error propagate:  expr$!!  (unwrap or re-raise) *)
  ;

(*
  Error propagate as statement: name$!!
  Dispatched from parse_assignment when $!! follows a plain identifier.
*)
error_propagate_stmt = identifier , "$!!" ;


(* ============================================================
   SECTION 14 — NAVIGATION INDEXING
   ============================================================ *)

(*
  Navigation indexing replaces  arr[i]  for multi-dimensional and ranged access.
  The parser detects which form to use by 2-token lookahead after  [ .

  arr[i>j>k]         — deep scalar: access nested dimension by dimension
  arr[i>j..k]        — flat extract: dimension i, then slice j..k at next level
  arr[p ; q ; r]     — flat extract: multiple independent paths → flat array
  arr[[p,q]]         — flat extract in double-bracket form
  arr[[g1];[g2]]     — structured extract: each group → one inner array
*)
nav_index =
    "[" , nav_path , "]"                  (* single path: Index, DeepIndex, or FlatExtract *)
  | "[" , nav_path , { ";" , nav_path } , "]"  (* multi-path FlatExtract *)
  | "[" , "[" , extract_group , "]" , "]"           (* FlatExtract double-bracket *)
  | "[" , "[" , extract_group , "]" , { ";" , "[" , extract_group , "]" } , "]"  (* StructuredExtract *)
  ;

nav_path = nav_step , { ">" , nav_step } ;

nav_step = nav_atom , [ ".." , nav_atom ] ;  (* range step within a dimension *)

nav_atom =
    integer_literal
  | "-" , integer_literal     (* negative index *)
  | identifier
  | "(" , expr , ")"         (* computed index *)
  ;

extract_group = "[" , nav_path , { "," , nav_path } , "]" ;


(* ============================================================
   SECTION 15 — MATCH PATTERNS
   ============================================================ *)

(*
  Patterns are used only inside  ??  match expressions.
  There are NO guard patterns (x? / _?) in the current implementation.  [D03]
  Range patterns accept only integer literals or char literals as bounds.  [D11]
*)
(*
  A pattern is one or more primary patterns joined by "||".  Alternatives are
  tested left to right and the first match wins.  "||" is recognised only at
  the top level of an arm — list elements are primary patterns, so
  [1, 2] stays unambiguous.
*)
pattern =
    pattern_primary , { "||" , pattern_primary }
  ;

pattern_primary =
    "_"                                    (* wildcard *)
  | string_literal                         (* exact string match *)
  | integer_literal                        (* exact integer *)
  | integer_literal , ".." , integer_literal  (* integer range (inclusive) *)
  | char_literal                           (* exact char *)
  | char_literal , ".." , char_literal    (* char range (inclusive) *)
  | float_literal                          (* exact float *)
  | bool_literal                           (* exact boolean *)
  | "[" , pattern , { "," , pattern } , "]"  (* list pattern (exact length) *)
  | "[" , "]"                             (* empty list pattern *)
  | "<" , addition_expr                   (* less-than comparison pattern *)
  | ">" , addition_expr                   (* greater-than comparison pattern *)
  | "<=" , addition_expr                  (* less-than-or-equal pattern *)
  | ">=" , addition_expr                  (* greater-than-or-equal pattern *)
  | "==" , addition_expr                  (* equality comparison pattern *)
  | "<>" , addition_expr                  (* not-equal comparison pattern *)
  | identifier                             (* variable / containment pattern *)
  ;


(* ============================================================
   SECTION 16 — PRIMARY EXPRESSIONS
   ============================================================ *)

primary_expr =
    literal
  | identifier
  | hot_identifier                         (* name° or °name *)
  | "(" , expr , ")"                      (* grouped expression *)
  | tuple_or_grouped
  | array_literal
  | match_expr                             (* ?? expr { cases } *)
  | lambda_expr                            (* x -> expr  or  (a,b) -> expr *)
  | terminal_size_expr                     (* >>? — returns (cols, rows) *)
  | data_prefix                            (* ##.  ###  ##! *)
  | data_wrapped_expr                      (* #|..| #,|..| etc. *)
  | base_literal                           (* 0x  0b  0o *)
  | execute_expr                           (* </ path /> *)
  | bash_exec_expr                         (* <\ args \> *)
  ;

(*
  Grouped expression and tuple share the same leading  ( .
  Named tuple is detected when the first token is  Ident  followed by  : .
*)
tuple_or_grouped =
    "(" , expr , ")"                       (* grouped (single expr) *)
  | "(" , expr , "," , expr , { "," , expr } , ")"  (* positional tuple *)
  | "(" , identifier , ":" , expr , { "," , identifier , ":" , expr } , ")"  (* named tuple *)
  ;

array_literal = "[" , [ expr , { "," , expr } , [ "," ] ] , "]" ;

(*
  Execute a Zymbol script file inline.
*)
execute_expr = "</" , path_string , "/>" ;
path_string = (* raw file path between </ and />, whitespace-trimmed *) ;

(*
  Shell execution: evaluates expressions and concatenates them as a command string.
*)
bash_exec_expr = "<\\" , { expr } , "\\>" ;
bash_exec_stmt  = bash_exec_expr ;    (* when used as a void statement *)

(*
  Expression statement: a function call (or module call) used for its side
  effects only — the return value is silently discarded.
  Also includes error-propagate as a statement: name$!!
*)
expr_stmt =
    function_call_expr
  | identifier , "::" , identifier , "(" , [ call_arg_list ] , ")"  (* module void call *)
  | error_propagate_stmt
  ;


(* ============================================================
   SECTION 17 — LITERALS
   ============================================================ *)

literal =
    string_literal
  | interpolated_string
  | char_literal
  | integer_literal
  | float_literal
  | bool_literal
  ;

(*
  String literal: delimited by double quotes.
  Escape sequences: \n  \t  \r  \"  \\  \{  \}
  There is NO \u{hex} escape sequence.                           [D10]
  \{  and  \}  are replaced by sentinel bytes to escape interpolation braces.
*)
string_literal = '"' , { string_char } , '"' ;
string_char =
    (* any Unicode character except  "  and  \ *)
  | "\\n"
  | "\\t"
  | "\\r"
  | '\\"'
  | "\\\\"
  | "\\{"
  | "\\}"
  ;

(*
  Interpolated string: a string with  {variable}  substitutions.
  Uses the same escape rules as plain strings.
*)
interpolated_string = '"' , { interpolated_part } , '"' ;
interpolated_part =
    (* literal text characters *)
  | "{" , identifier , "}"    (* variable interpolation *)
  ;

(*
  Char literal: delimited by single quotes, exactly one grapheme cluster.
  Escape sequences: \n  \t  \r  \'  \\  \0
  There is no \u{hex} escape for chars either.
*)
char_literal = "'" , ( char_char | char_escape ) , "'" ;
char_char    = (* any Unicode grapheme cluster except  '  and  \ *) ;
char_escape  = "\\n" | "\\t" | "\\r" | "\\'" | "\\\\" | "\\0" ;

(*
  Integer literal: decimal digits.  Underscores are allowed as separators.
  Also includes Unicode digit blocks (Devanagari, Bengali, …) when the
  active numeral mode is set.
*)
integer_literal = digit , { digit | "_" } ;

(*
  Float literal: integer part, dot, fractional part.
*)
float_literal = digit , { digit } , "." , digit , { digit } ;

(*
  Boolean literal: #1  (true)  or  #0  (false).
  Unicode digit equivalents are also accepted when a numeral mode is active.
*)
bool_literal = "#1" | "#0" ;

digit = "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" ;
hex_digits = hex_digit , { hex_digit } ;
hex_digit  = digit | "a" | "b" | "c" | "d" | "e" | "f"
                   | "A" | "B" | "C" | "D" | "E" | "F" ;
bin_digits = ( "0" | "1" ) , { "0" | "1" } ;
oct_digits = ( "0".."7" ) , { "0".."7" } ;


(* ============================================================
   SECTION 18 — IDENTIFIERS AND HOT NOTATION
   ============================================================ *)

(*
  Plain identifier: starts with a letter or Unicode symbol (not a digit,
  whitespace, or known operator character), continues with letters, digits,
  underscores, or further Unicode symbols.
  An underscore alone  _  is the wildcard token, not an identifier.
*)
identifier = id_start , { id_continue } ;

id_start    = letter | unicode_symbol_char ;
id_continue = letter | digit | "_" | unicode_symbol_char ;

(*
  Hot identifiers carry the ° (DEGREE SIGN, U+00B0) prefix or suffix.
  They are used to anchor accumulator variables above the nearest loop scope.

    name°   — postfix hot  (HotIdent token):  lexed as a single token
    °name   — prefix hot   (PreHotIdent token):  also a single token

  Both forms are valid as an expression (RHS) or as an assignment LHS.  [C05]
    name°  anchors to the nearest enclosing @ scope (dies with the loop).
    °name  anchors to the scope ABOVE the nearest @ (survives the loop) —
           e.g.  total = °total + item  inside a loop.
*)
hot_identifier = identifier , "°" | "°" , identifier ;


(* ============================================================
   SECTION 19 — COMMENTS
   ============================================================ *)

(*
  Line comment: everything from  //  to end of line.
  Block comment: everything between  /*  and  */.  Nestable: inner  /* */  pairs
  are tracked and must be balanced before the outer comment closes.
  Comments are stripped during lexing and do not produce tokens.
*)
line_comment  = "//" , { (* any character except newline *) } ;
block_comment = "/*" , { block_comment | (* any character *) } , "*/" ;


(* ============================================================
   SECTION 20 — OPERATOR SUMMARY (REFERENCE)
   ============================================================ *)

(*
  Symbolic operator reference — all recognized multi-character tokens.

  OUTPUT
    >>          basic output (0 or more items)
    >>|         TUI block
    >>~         positioned output
    >>!         clear screen
    >>?         terminal size (expression, not statement)  [D02]

  INPUT
    <<          string input
    << ##.      typed input: Float        (##.(T,D) decimal-validated)   (v0.0.7)
    << ###      typed input: Int          (###(N) max digits)            (v0.0.7)
    << ##"      typed input: String       (##"(N) max chars)             (v0.0.7)
    << ##'      typed input: Char (exactly one character)                (v0.0.7)
    <<|         blocking key input
    <<|?        non-blocking key input

  CLI
    ><          CLI args capture

  ARITHMETIC
    +  -  *  /  %  ^     binary arithmetic (^ right-associative)
    +  -  !               unary operators

  COMPARISON
    ==  <>  <  >  <=  >=

  LOGICAL
    &&  ||

  ASSIGNMENT
    =   :=   +=  -=  *=  /=  %=  ^=   ++  --

  RANGE
    ..            exclusive-start, inclusive-end range
    ..:step       range with step

  COLLECTION $-operators
    $#   $+   $+[  $-   $-[  $--   $?   $??  $~~  $/   $*   $~
    $[   $>   $|   $<   $^+  $^-   $^   $++

  ERROR
    $!   $!!

  TYPE / DATA
    ##.  ###  ##!   #|  #,|  #^|  #.N|  #!N|   #?

  MODULE
    <#   #>   ::   =>

  CONTROL
    ?    ??    @    @:label   @!   @>   @~
    !?   :!    :>

  STRING
    $~~  $/  $*  $++  $??

  PIPE
    |>

  LAMBDA
    ->

  MATCH
    =>

  NAVIGATION INDEX
    >   (dimension separator inside [...])
    ;   (path separator inside [...])

  MISC
    ¶   \\   \   <~   ~   °
    </ />   <\ \>

  NUMERAL MODE
    #09#  (any Unicode digit-block 0 and 9 in the same block)
*)

(* ============================================================
   SECTION 21 — NOT IMPLEMENTED (reference)
   ============================================================ *)

(*
  The following constructs appear in earlier EBNF drafts or ROADMAP but are
  NOT present in the current parser:

  [NI01] Post-condition loop  ~>  — DISMISSED 2026-06-12: will not be
         implemented; the idiom is an infinite loop with a trailing break
         ( @ { body  ? !cond { @! } } ). The  ~>  symbol remains unoccupied.
  [NI02] Match multi-value arms  pattern => (a, b)  — single value only
         (list containment  [a, b] => v  covers the use case since v0.0.4)
  [NI03] Match binding  pattern as name  — DISMISSED 2026-06-12: will not be
         implemented; bind the value before the match instead.
  [NI04] Lambda $!!  as a standalone HOF  — $!! is postfix on an expr, not HOF
  [NI05] Dict / map literals  #{ key: val }  — not in current lexer or parser
  [NI06] First-class function composition  :.  — not implemented
  [NI07] Sum types / variant constructors — not implemented
*)

(* ============================================================
   SECTION 22 — RETIRED OPERATORS (reference)
   ============================================================ *)

(*
  The following operators existed in earlier versions but are no longer
  accepted by the current parser:

  [R01] $++[i]  bracket form of string insert — replaced by  $++ pos : sub
  [R02] $--[pos:count]  positional remove — now  $-[pos:count]
  [R03] <=  as import/export alias separator — replaced by  =>  only   [D12]
  [R04] @@label  loop label — replaced by  @:label               [D09]
  [R05] _?  guard pattern in match — removed                      [D03]
*)
```
