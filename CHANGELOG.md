# Changelog

All notable changes to Zymbol-Lang are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)  
Versioning: [Semantic Versioning](https://semver.org/) (pre-1.0 series)

---

## [0.0.8] — 2026-08-02

Memory-model debt release: every divergence found by the design-vs-implementation
audit in [MEMORY_MODEL.md](MEMORY_MODEL.md) is resolved — findings MM-1 … MM-9 plus
the two VM parity bugs (MM-10, MM-11) discovered while verifying the fixes. Three
validation projects written in Zymbol (zy-GO, zy-Serpiente, zyKlingonGalaxy) contributed
the rest: HLZ-001 … HLZ-011, HLZ-SRP-001 and HLZ-KL-001 are findings from writing real
applications, not from unit tests.

Measured on the branch: **936 unit tests**, **536/536 TW/VM parity**, **523/525 golden**,
formatter property suite **600 PASS / 0 FAIL** with no regressions against the baseline.
The two golden failures are stale hand-written `.expected` fixtures, not interpreter
regressions — see [IMPL_V008.md](IMPL_V008.md) § E.1. The one remaining piece of known
debt is § E.3: the browser interpreter is behind the Rust engines on **seven** cases — six
v0.0.8 fixes with no counterpart there yet, plus one float-literal precision bug that
predates this release and was only found once the playground's examples became real files
on disk.

### Added

**Automatic destruction at last use (auto-free) — both engines, always on**
- A variable's memory is released right after the statement containing its
  last use, instead of at scope end. Invisible by design: it never changes a
  correct program's behavior — it only lowers peak memory (measured: a script
  holding two sequential 30 MB strings peaks at ~64 MB instead of ~94 MB in
  the tree-walker).
- New purely lexical, conservative last-use analysis
  (`zymbol_semantic::last_use`): a region is a flat statement sequence
  (top-level program or a named function body); mentions are collected from
  the whole statement subtree — nested blocks, loop bodies, lambda bodies
  (capture happens at the statement containing the lambda), `{var}` string
  interpolations (verbatim, mirroring the runtime resolver), and input
  prompts. The `Expr` walker is exhaustive (no `_` arm), so future syntax
  additions fail compilation until their mention rules are reviewed.
- Never auto-freed (conservative exclusions): constants, hot names
  (`x°`/`°x`), `_`-prefixed names, module-level bindings, output/mutable
  parameters, and free variables of named functions used as first-class
  values. Normal parameters and region-level locals are candidates.
- Tree-walker: per-body schedules stored in `FunctionDef::Zymbol::auto_free`
  and applied by `execute_body_scheduled`; destruction is skipped while
  control flow is pending (frame/loop teardown owns those paths).
  Auto-destroyed names live in a frame-local `auto_dead_variables` set: using
  one (impossible in a correct program) raises a distinctive
  `internal: use after auto-destruction` error — including from string
  interpolation, which otherwise silently prints `{var}`.
- VM: the compiler emits `LoadUnit` on the variable's register after its
  last-use statement (same analysis, per module context). Known limitation:
  expression temporaries may retain a value until their register is
  overwritten, so the VM's peak-memory win is currently smaller than the
  tree-walker's.
- The previously dead wiring (`set_destruction_schedule`, `statement_index`)
  was removed; `zymbol check`'s ambiguous-lifetime warnings (old def-use
  analyzer) are unchanged.
- Verified when it landed: 847 unit tests (12 new analyzer + 2 interpreter),
  519/519 TW/VM parity, 503/503 golden, 89/89 GUIDE examples, benchmark gate
  14/14 with no regressions. (Counts at that commit — the release totals are in
  the header above.)

**`std/term` — terminal display metrics (both engines)**

- New stdlib module with five functions: `width`, `pad_left`, `pad_right`,
  `center`, `truncate`. Width is measured in **terminal columns**, not grapheme
  count — CJK ideographs, kana, hangul and most emoji occupy two columns each,
  so `"手番"$#` is `2` but `term::width("手番")` is `4`.
- The boundary is deliberate: `std/term` answers a question about the *screen*.
  Everything that operates on a string's *content* — split (`$/`), slice
  (`$[..]`), replace (`$~~`), repeat (`$*`), join, trim — is (or will be) a
  language symbol and never enters this module. Naming it `term` rather than
  `text` keeps that line visible.
- Column widths come from the `unicode-width` tables over grapheme clusters
  (`unicode-segmentation`), so a multi-code-point cluster is measured as one
  unit and `truncate` never splits a wide glyph. `pad_*`/`center` pad with
  spaces to an exact column count and leave already-wide strings untouched;
  `center` gives a spare column to the right. Motivated by zy-GO, which carried
  a hand-maintained ~40-range East Asian width table inside the game
  (`表示/文字.zy`) — now replaced by this module.

**`##!` on a `Char` yields its Unicode code point**

- `##!'A'` is `65`, `##!'あ'` is `12354`. This is the only direct Char→Int
  route (the previous workaround was inverting a base literal, `0d|c|`, and
  stripping the `0d` prefix), and it makes characters classifiable by range —
  `Char` is otherwise neither comparable nor castable. `###` is unchanged;
  a Char has no fractional part, so only the truncating cast was extended.
- Regression: `tests/casts/06_char_to_int.zy` and `tests/stdlib/stdlib_term.zy`
  (both TW == VM), plus four unit tests over the pure width/pad/center/truncate
  helpers.

**Match or-patterns — `p1 || p2` alternatives in a `??` arm (both engines)**

- An arm's pattern can now be a `||`-separated chain: `'p' || 'P' => { ... }`
  matches if any alternative matches. Alternatives are tested left to right
  and the first one that matches wins.
- Alternatives combine any pattern kind, not just literals — range, comparison,
  ident and list patterns all mix freely in one arm: `1..10 || 20..30`,
  `< 0 || > 100`, `1 || expected || 9`, `["run", _] || ["build", _]`.
- New `Pattern::Or(Vec<Pattern>, Span)` variant
  (`crates/zymbol-ast/src/match_stmt.rs`). `||` is recognised only at the top
  level of an arm — list elements stay primary patterns, so `[1, 2]` is never
  ambiguous with two alternatives.
- Requested directly against `GO/対局.zy`, whose key-handling arms only
  accepted the lowercase letter (`'p'`) and silently ignored the uppercase
  one; `['p', 'P']` (list containment) already covered that one case, but had
  no equivalent for non-literal patterns.
- VM: `compile_match_expr` was refactored into a recursive
  `emit_pattern_test` that returns `(skip_patches, to_body_patches)` instead
  of emitting the arm body inline per pattern kind — the `Or` case chains
  this helper across alternatives, patching the last one's failure to skip
  the whole arm and every earlier one's success to jump to the body.
- Mirrored in the browser interpreter (`web/src/zymbol/zymbol.js`):
  `parseMatchPattern` wraps `parseMatchPatternPrimary` with the same
  top-level-only `||` chaining, and `matchPattern`'s new `'or'` case tests
  alternatives left to right.
- Regression: `tests/match/16_or_pattern_basic.zy`,
  `tests/match/17_or_pattern_mixed.zy`, `tests/match/18_or_pattern_block.zy`
  (all TW == VM). Verified byte-identical against the JS mirror as well.

**Zymbol Packages (`.zyp`) — one file for a multi-file program**

- New `zymbol package` and `.zyp` support in `zymbol run`, backed by a new
  `zymbol-package` crate. A `.zyp` is a ZIP archive of **source**, not a
  compiled binary: `zyp.toml` (manifest), `zyp.json` (the same manifest
  pre-serialized so the web playground never parses TOML), and the packaged
  `.zy` tree under `src/`. Unrelated to `zymbol build`/`zymbol-standalone`,
  which produces a native executable — neither feature depends on the other.

  ```bash
  zymbol package DIR --script main.zy -o out.zyp   # build the archive
  zymbol package DIR --script main.zy --dry-run    # closure + warnings, writes nothing
  zymbol run out.zyp                               # run it
  zymbol run out.zyp --script 囲碁 --tw            # pick an entry, force an engine
  ```

- The manifest declares one or more `[[script]]` entries (`name`, `path`,
  `default`, `desc`) and `package.engine`, a semver *requirement*. Always write
  `engine = ">=0.0.8"`: a bare `"0.0.8"` is a caret requirement, which pre-1.0
  matches only that exact version and would break on every patch release.
  `zymbol package` always synthesizes the `>=` form.
- **Closure computation is strict**: `compute_closure` walks module imports and
  `</ file.zy />` targets from the declared scripts; a `.zy` file that is
  neither listed nor reachable is never packaged. **Packaging is permissive**:
  anything unresolvable statically — an absolute import, a `<\ shell \>`, a
  parse error — becomes a warning (`W001`–`W011`) rather than a hard failure,
  so `--dry-run` always produces something the author can inspect. The one hard
  error is a `[[script]]` that turns out to be a module file: a package whose
  entry point can't run isn't permissive, it's broken.
- **`zymbol run pkg.zyp` extracts to an ephemeral temp dir and never `chdir`s.**
  Code is read from the temp dir, but a script's own `std/io` writes (relative
  paths) still land in the user's real working directory, because they resolve
  against the process's actual cwd. That asymmetry is the point of ephemeral
  extraction: the code is disposable, the data the script writes is not.
  `--keep-temp` retains the directory and prints its path for debugging.
- Default engine for a `.zyp` is the **VM**; loose `.zy` files still default to
  the tree-walker, so nothing changes for existing scripts. Precedence:
  `--tw` > `--vm` > manifest `mode` > VM.
- Security: ZIP entry names and `[[script]].path` go through one lexical rule
  (`path_safety`) — no `..`, no absolute prefix, no backslash, no NUL, no drive
  letter — checked at manifest parse time, again at extraction, and once more at
  write time. A `[[script]].path` of `../../elsewhere.zy` previously escaped the
  extraction directory and got arbitrary source on the user's disk read and run.
  Per-entry and total decompressed size are capped at 100 MiB against zip bombs.
- The writer is deterministic: fixed 1980-01-01 timestamps and fixed entry
  order, so the same source tree always produces a byte-identical archive and a
  `.zyp` can be verified by hash.
- Web: `web/src/zymbol/zyp.js` reads the ZIP by hand (central directory +
  `DecompressionStream('deflate-raw')`) — no bundler, no CDN, consistent with
  `web/`'s no-build-step policy. Loading a `.zyp` in the playground **mounts**
  the whole source tree (visible in the sidebar and to the module resolver,
  named by full relative path, e.g. `packages/go/核/盤.zy`) but **opens only
  one tab**, the default `[[script]]`; the manifest's scripts populate the
  picker next to ▶ Run.
- Tests: `crates/zymbol-package/tests/roundtrip.rs` plus unit tests over
  manifest validation, the pre-1.0 semver trap, path-traversal rejection, and
  closure/warning behavior. `web/tests/test_zyp.mjs` covers the browser reader
  and module resolver against fixtures it builds itself.

### Changed

**One shared module-path resolution rule — `ModulePath::resolve_from`**

- The tree-walker, the semantic analyzer and the VM compiler each carried their
  own copy of "given this import and this importing file, which file is it?".
  They agreed on the common case and diverged on the rest: `compile_import`
  ignored `is_absolute` and `home_relative`, so `<# /abs/path => x` and
  `<# ~/lib/x => x` resolved to a *different file* under `--vm` than under the
  tree-walker — silently, because both paths existed in a normal checkout.
- `ModulePath::resolve_from(&self, importer: &Path) -> PathBuf` is now the
  single rule, and all three call it. Adding a path form (or changing how one
  resolves) is one edit instead of three that can drift.
- Found while adding `.zyp` packaging, which needed a fourth consumer of the
  same rule for closure computation and would have inherited whichever copy it
  was written against.
- Regression: `tests/modules_scope/alias_shadowed_by_variable.zy` (TW == VM).

**`zymbol check` now checks the whole program, not just the named file**

- It followed no imports: a module that failed to parse or type-check was
  invisible until run time, so `check` returning clean meant nothing for any
  project organised in modules. It now walks imports transitively (stdlib
  excluded, cycles cut) and reports each module's errors at the module's own
  line, followed by `note: reached from <importer>`.
- Style warnings (unused variables, ambiguous lifetimes) deliberately stay with
  the file named on the command line — they are about the code you are editing,
  not about its dependencies.
- The LSP gets the same coverage through `ModuleIndex::set_module_errors`, plus
  a new `module-has-errors` diagnostic on the import line, so a broken
  dependency is visible in the editor at the place that pulls it in.

### Fixed

**`>>?` aborted in one engine and returned a size in the other when there was no terminal (found by the new package gate)**
- With output redirected, inside a container, or in CI, the tree-walker
  propagated the OS error from `crossterm::terminal::size()` while the VM
  already fell back to 80x24. Identical in a real terminal, which is why the
  parity suite reported a clean sweep for as long as it only ever ran in one.
- Both engines now fall back to the conventional `[24, 80]`, so a TUI program
  stays runnable when piped — it simply lays itself out for 80 columns. The
  behaviour was undocumented; [GUIDE.md](GUIDE.md) now states it.
- `std/term::width` is unaffected: it measures the display width of a string,
  not the terminal.

**Linux packages declared a glibc floor they did not honour (found by the new package gate)**
- `control.in` hard-coded `Depends: libc6 (>= 2.17)` and `zymbol.spec.in`
  `Requires: glibc >= 2.17`, while the binary needs whatever glibc built it — a
  binary runs on its build machine's glibc or newer, never older. `dpkg` would
  install the package on a system too old to run it, satisfying the declared
  dependency and then failing at exec with `version 'GLIBC_2.xx' not found`.
- `build-packages.sh` now derives the floor from the binary's own versioned
  symbols (`glibc_min_for`) for both formats. The Arch `PKGBUILD` keeps an
  unversioned `glibc`, correct for a rolling release.
- `verify-deb.sh` compares the declared floor against the binary's requirement
  **statically**, so it fails even when the verification container's glibc is new
  enough to mask the problem — which is the case in CI (`ubuntu-22.04` builds at
  2.35, `debian:12` verifies at 2.36).
- Supported floor is unchanged in practice: glibc 2.35, from the release builder.
  Older systems are served by the static musl binary.
- New `--binary PATH` in `build-packages.sh`, to package a binary that lives
  outside `target/` (a container build, for instance).

**A stdlib module the build does not include reported two different names (found by the new package gate)**
- `<# std/db` in a binary built `--no-default-features` — which is precisely what
  the Linux packages ship — produced `module not found: std/db` in the
  tree-walker and `module not found: std/db.zy` in the VM.
- Cause: `compile_import` handled stdlib paths it had entries for and let every
  other stdlib path fall through to file resolution, whose error formats
  `{}.zy`. A stdlib path has no file to resolve to — `ModulePath::resolve_from`
  returns `None` for it by contract — so the fallthrough could only ever produce
  a misspelt name. The tree-walker returns inside its stdlib branch
  (`load_stdlib_module`) and never reaches a file path; the compiler now does the
  same. Also affects a typo'd stdlib import (`<# std/mth`).
- Invisible to `vm_compare.sh` as normally run: the suite is measured with a
  full-featured binary, where `std/db` exists and the fallthrough is never taken.
  It surfaced only when the new release gate ran the suite against the installed
  `.deb`. See [packaging/verify/README.md](packaging/verify/README.md).
- Parity with the packaged binary is now 536/536, same as the development build.

**Findings from the zy-Serpiente and zyKlingonGalaxy i18n rework (HLZ-SRP-001, HLZ-KL-001)**

Rewriting the internationalization of the two older TUI games against
[USERAPPI18N.md](USERAPPI18N.md) surfaced two divergences. Both were found by
writing ordinary application code, and both were silent in one engine and
correct in the other or correct nowhere.

**HLZ-SRP-001 — a module function that wrote state and returned a value lost the write**
- In the tree-walker, `f() { v = "en"  <~ v }` returned `"en"` and left the
  module's `v` at its previous value. No error, no warning. The register VM was
  correct, which made it easy to miss: a program tested under `--vm` behaved,
  and the default engine did not.
- Cause: the MoveOrClone optimisation in `Statement::Return` moves a returned
  bare identifier out of scope instead of cloning it — O(1) for strings and
  arrays. The module-state write-back then looked the key up, found nothing, and
  read that as "this frame never touched it", so the mutation was dropped. Output
  parameters were already excluded from the move for the same reason (QW13); module
  state was not.
- Fix: `current_output_params` becomes `move_guard_names` and now holds both
  output parameters and the module variables injected into the frame. Both are
  read again after the return, so both are cloned rather than moved.
- The shape that triggered it is the natural one for a stateful API — "change
  this and tell me how it came out": a counter, a cursor, a locale rotator.
- Regression: `tests/modules_scope/mod_state_return.zy`, which writes state and
  returns it as a literal, an indexed value, a local, an unrelated value, and
  alongside an output parameter. It fails on the previous binary.

**HLZ-KL-001 — string interpolation rejected identifiers the lexer accepts**
- `"{x}"` validated the name with `is_alphanumeric()`, which is narrower than
  the identifier rule used everywhere else. Kanji are Unicode category `Lo` and
  passed; Private Use Area glyphs — pIqaD, the Klingon script zyKlingonGalaxy is
  written in — are category `Co` and did not. A program whose identifiers were
  valid in every other position could not interpolate them.
- The same narrower rule had been copied into two more places, with two more
  symptoms: the unused-variable analysis did not count an interpolation as a use
  of such a name, so it warned about variables the program does read; and the LSP's
  word-at-cursor and identifier-validity helpers did not recognise them, so hover
  and completion did not work in such a program.
- Fix: `Lexer::is_ident_start` and `Lexer::is_ident_continue` are now public and
  are the single definition. The lexer's interpolation loop, the semantic
  analyser's interpolation scan and the analyzer's three helpers all defer to
  them.
- Regression: `tests/i18n/interp_identificadores.zy` interpolates names in Latin,
  kanji, Hangul, Cyrillic, Greek, Devanagari, pIqaD (including the Klingon
  apostrophe) and an emoji, at top level and inside a module function.

**Numeral mode (`#d0d9#`) did not reach interpolation, juxtaposition, `$++` or `>>~`**
- `#०९#\n>> n ¶` printed Devanagari digits, but `#०९#\ny = "{n}"\n>> y ¶` still
  printed ASCII — the same value, through a different route to the screen,
  silently reverted to `0`–`9`. `Value::to_display_string()` (tree-walker) and
  `to_string_repr()`/`Display` (VM) are the generic value-to-text conversions
  used by string interpolation, juxtaposition (`"a" b`), `$++` and `>>~`; none
  of them has a numeral-mode field to read, since numeral-mode awareness had
  only ever been wired into `>>`'s own per-item formatting.
- Surfaced rebuilding zyKlingonGalaxy's HUD renderer: every score/delay/wave
  count is composed into a label string (`"H:" $+ mI'(n)`) that a generic
  centered-list helper then draws with `>>~` — a hand-written digit-by-digit
  pIqaD conversion existed specifically because no runtime path from a live
  `Int` to displayed text respected the active script except bare `>>`.
- Fix: every call site with `&self`/`&mut self` access to the active mode now
  routes Int/Float/Bool through the same numeral-aware conversion `>>` uses
  instead of the bare, context-free one — `value_to_concat_str` (juxtaposition
  and `$++`, tree-walker), `interpolate_string`, both `execute_output_pos`
  branches, and, in the VM, both copies each of `ConcatStr`, `ConcatBuild` and
  `BuildStr`, plus `PrintAt`. `Value::to_display_string()`/`to_string_repr()`
  themselves are unchanged (no interpreter context to read the mode from) —
  the fix is at every place that had that context and wasn't using it.
- Regression: `test_i18n_mode_affects_interpolation`,
  `test_i18n_mode_affects_juxtaposition`, `test_i18n_mode_affects_concat_build`
  and their `_vm` parity counterparts in `zymbol-interpreter/tests/e2e.rs`.

**Numeral mode, audited: collections, the round trip, and the cost of the fix**

An audit of the change above found three things it left undone. A fourth
finding — that the mode also reaches text the program uses as *data* (file
names, shell commands built by interpolation) — is **intended behaviour**, not a
defect: `#d0d9#` states how this program writes numbers, and validating that is
the developer's responsibility. It is now documented as such in GUIDE.md
("Intent and Responsibility"), alongside the one exception: `json::encode` keeps
emitting ASCII, because a serialization format has a grammar of its own.

- **Collections still reverted to ASCII.** `>> [1, 2, 3] ¶` printed
  `[1, 2, 3]` under an active mode while each element printed on its own came
  out in the active script — the conversion applied the mode at the top level
  only, so a number stopped following it by the mere fact of sitting in a list.
  Fixed with `Value::to_display_string_in`/`to_repr_string_in` (tree-walker) and
  `Value::to_display_in` (VM), recursive over arrays, tuples and named tuples,
  used by `format_value`, `numeral_repr` and both `Print` branches. Brackets,
  commas and separators stay ASCII.
- **The digits did not come back.** `#|…|` normalized Unicode digits, but
  `#.N|…|`, `#!N|…|`, `<<###`, `<<#.` and `<<#(n,d)` did not — a program could
  render `१२०` and then refuse to read it, and a user could not type what the
  program had just shown them. All numeric casts now normalize through
  `ascii_digits` (a shared helper in each engine) before parsing. Non-numeric
  strings are still rejected.
- **The VM answered 0 where the tree-walker raised.** `#.1|"४२"|` was a runtime
  error in the tree-walker and `0` in the VM; `c|…|`/`e|…|` on a non-number was
  an error in one engine and `0.0` in the other. Both now fail, with the
  tree-walker's message. (Found by the audit, present since before the numeral
  work.)
- **Performance regression from the original fix, removed.** Routing every
  concatenation through `numeral_int` allocated an intermediate `String` even in
  ASCII mode, costing ~8% on `"label" i` in a loop (3M iterations: VM 0.34 s →
  0.37 s, tree-walker 0.750 s → 0.793 s). `map_ascii_digits` now takes its
  buffer by value and the VM's hot concatenation paths write straight into the
  destination when the mode is ASCII: back to 0.32–0.34 s / 0.75 s.
- Also routed: the VM's `ReadLine` prompt, the last `print!` of a value that
  still used `Display`.
- Regression: `test_i18n_mode_reaches_array_elements`,
  `…_tuple_elements`, `…_interpolated_array`, `…_array_elements_vm`,
  `test_round_accepts_unicode_digits`, `test_trunc_accepts_unicode_digits`,
  `test_round_accepts_unicode_digits_vm` and
  `test_round_rejects_non_numeric_string_in_both_engines`; the web example
  `examples/numerals/composed.zy` covers collections and the round trip in the
  CLI↔JS parity run.

**Ordering comparisons: one rule, and no second-class digit script**

`? "5" > 5` coerced the string and answered `#0`; `? "४२" > 5` raised *cannot
compare string '४२' with integer 5*. Same operator, same shape of operands, and
the only difference was which script wrote the digits — either both are strings
or both are numbers, anything else makes every script but ASCII a second-class
citizen of its own language.

- The rule, now the same in the tree-walker, the VM and the JS engine:
  **numeric** when both sides are numbers, where a string counts as a number if
  `#|…|` would convert it (digits from any of the 69 scripts);
  **lexicographic** when both sides are non-numeric text; **an error** when a
  number meets text that is not a number. Equality is deliberately excluded —
  `==` still never coerces, so `"5" == 5` and `"५" == 5` are both `#0`.
- The three engines had three implementations of ordering and disagreed *even in
  ASCII*: `"5" > 5` was `#0` in the tree-walker and `#1` in the VM (whose
  `cmp_direct` returned "greater" for every pair outside its table); `"10" > "9"`
  was `#1` in the tree-walker and `#0` in the VM; and the VM's call-frame loop
  had a third variant that answered `false` for everything but `Int`/`Int`.
  `"४२" > "९"` was `#0` in both engines where `"10" > "9"` was `#1`.
- Replaced by `cmp_order`/`cmp_order_error` (VM, used by both interpreter loops)
  and the rewritten string arms of `compare_values` (tree-walker), with
  `orderValues` mirroring them in `web/src/zymbol/zymbol.js`. Comparison errors
  now carry the same text in every engine (the JS engine also stops calling the
  operators `Lte`/`Gte`).
- Also aligned: `'a' < 'b'` and `#0 < #1` were a VM feature and a tree-walker
  error; both engines compare them now.
- Regression: `test_order_numeric_string_any_script`,
  `test_order_non_numeric_strings_stay_lexicographic`,
  `test_order_number_against_text_is_an_error` and
  `test_order_rule_is_identical_in_both_engines`.

**Static-tooling audit: what `check` and the LSP could not see**

Running the analyzer over the workspace's ~918 `.zy` files and diffing its
diagnostics against `zymbol check` and against what actually happens at run time
surfaced four divergences. The recursive `check` is filed under **Changed**
above (it is new coverage, not a repaired behavior); the other three are bugs.
The audit is repeatable: `crates/zymbol-analyzer/examples/lsp_scan.rs` prints
the analyzer's diagnostics for a list of files.

- **A re-export was dropped on the indexer's first pass.**
  `index_background_module` registered a file's imports *after* reading its
  export block — but a re-export (`alias::item => name`) resolves its source
  through that same file's alias map, which did not exist yet. Every i18n layer
  module therefore looked like it exported nothing: **33 false
  `export-not-found` diagnostics** across serpiente, klingon_galaxy,
  aprende_zymbol and api_demo, on code that runs correctly. Imports are now
  registered first. Count after the fix: 0.
- **`std/` modules were a blind spot for every static tool.** They have no file
  on disk, so an alias bound to one was never validated: `math::inventada()`,
  `m::PI()` (calling a constant), `m.sin` (reading a function as a value), and a
  typo in a stdlib re-export (`t::widht => ancho`, which silently breaks every
  caller of an i18n layer) all passed `zymbol check` in silence and showed
  nothing in the editor. `zymbol_common::stdlib` is now the shared export table
  — names plus arity — and both `check` and the LSP report through the single
  `zymbol_semantic::check_stdlib_access`, with a "did you mean" for near
  misses. A named-tuple field may legitimately share an alias's name
  (`resp.json.user`), so only a name that does not itself follow `.` or `::` is
  read as a module access.
  `crates/zymbol-cli/tests/stdlib_parity.rs` fails if the table ever drifts from
  what the tree-walker and the VM compiler actually implement, so the fix cannot
  rot the way a hand-maintained list would. See REFERENCE.md L27.
- **The formatter could not format a file with an escaped literal in a match
  pattern.** `format_pattern`'s `Pattern::Literal` arm wrote `lit.to_string()`
  — `Display`, which does not escape — while the *expression* path correctly
  used `escape_char`/`escape_string`. So `'\n'` in an expression formatted fine
  and `'\n'` in a `??` arm came back as a real newline between two quotes, which
  no longer lexes. The fail-closed safety gate caught it every time and refused
  to write (no file was ever corrupted), but that made `zymbol fmt` unusable on
  any file with `'\n'`, `'\t'`, `'\r'`, `'\0'` or `'\\'` in a match arm — which
  is exactly what key handling in a TUI program looks like. String patterns took
  the same path. Both now route through the expression escaper.
- Regression: `crates/zymbol-semantic/src/stdlib_access.rs` (8 cases),
  `crates/zymbol-cli/tests/cli_check_stdlib.rs` (4 cases),
  `crates/zymbol-cli/tests/stdlib_parity.rs`, and
  `tests/bugs/bug_char_escape_lexing.zy`, which is in the formatter property
  corpus — `fmt_property.sh` reported it as a P1 failure before the fix and
  reports 0 failures after.

**Findings from the zy-GO validation project (HLZ-001 … HLZ-009)**

Building [zy-GO](https://github.com/zymbol-lang/zy-GO) — a Go/囲碁 game whose
engine is 13 modules across four subdirectories — surfaced nine divergences.
Six were bugs, two were diagnostics that named neither cause nor fix, and one
was a documentation table that sent readers to write a TUI that could not
respond to the cursor keys.

**HLZ-001 — a module constant could not carry a sign**
- `NAME := -1` inside a module body was rejected as E013 "initializer must be a
  literal": a signed number parses as unary minus applied to a literal, which is
  an expression node, but `-1` is a constant, not a computation.
- Both the parser gate and the semantic defence-in-depth gate now accept a
  signed literal, and they share the same rule so they cannot drift apart.
- Regression: `tests/modules_scope/out_param_module.zy`, plus unit tests in
  `zymbol-semantic` for the accepted and still-rejected forms.

**HLZ-002 — an index computed from parameters was rejected as Float**
- `arr[(r - 1) * n + c]` inside a function failed analysis with "array index
  must be Int, got Float", though every operand is an integer at runtime.
- Cause: a parameter used in arithmetic was constrained to `Numeric`, and
  `Numeric` resolved to `Float` — asserting more than was known.
- New `ZymbolType::Number` means "Int or Float, undetermined": accepted as an
  array index, compatible with Int and Float and with nothing else. The static
  error for passing a String to a function that adds to its parameter is
  preserved, and now reads "expects Number" rather than the inaccurate
  "expects Float".
- Regression: `tests/modules_scope/subfolder_dot_convention.zy`.

**HLZ-003 — `==` was the only comparison that did not promote Int and Float**
- `##.0 == 0` was `#0` while `##.0 >= 0` and `##.0 <= 0` were both `#1`. Since
  the two values print identically, the contradiction was invisible: a failing
  assertion read "expected 0, got 0".
- `values_equal` in the tree-walker now promotes, as `compare_values` already
  did. The VM's `cmp_direct` already promoted, so `==` was also a silent engine
  divergence; `Value::equals` and the `CmpEqImm`/`CmpNeImm` fast and slow paths
  are aligned too — a Float subject in `?? 3.0 { 3 => … }` used to raise a type
  error in one and leave the destination register unwritten in the other.
- Regression: `tests/arithmetic/int_float_equality.zy` (TW == VM), plus unit
  tests in `zymbol-interpreter`.

**HLZ-004 — the documented dot convention was rejected by `check` and the LSP**
- `# .folder_file` in `folder/file.zy` is the convention in
  `tests/i18n/DOT_CONVENTION.md`. Validation stripped the leading dot and then
  compared the whole remaining name against the file stem, so `.core_board` was
  measured against `board` and every subdirectory module failed E001.
- The dotted form now compares against `<parent>_<stem>`. `zymbol run` had
  always accepted these modules; `check` and the LSP rejected them, which made
  `zymbol check` unusable on any project organised in folders — 11 of zy-GO's
  13 modules.
- Regression: `tests/modules_scope/subfolder_dot_convention.zy` and four unit
  tests, including a multi-byte folder name.

**HLZ-005 — `./../x` failed with a diagnostic that explained nothing**
- The old message was "expected module path" followed by "unexpected token:
  Slash". The spelling stays rejected on purpose: the AST records only
  `parent_levels`, so `./../x` and `../x` are indistinguishable once parsed and
  the formatter's token-stream safety gate would refuse any file using it. What
  was wrong was the diagnostic, which now names the fix.
- Regression: `tests/errors/parser/parent_path_alias.zy`.

**HLZ-006 — GUIDE.md documented the wrong arrow keys**
- §3b said `<<|` returns `'U'`, `'D'`, `'L'`, `'R'`. It returns the arrow
  glyphs `'↑'`, `'↓'`, `'←'`, `'→'`. Following the guide produced a TUI that
  drew correctly and ignored the cursor keys, with no error to read, because
  the key fell through to the default match arm. `serpiente/logica.zy` has
  matched on the glyphs since v0.0.5.
- Corrected, with a note that this leaves every ASCII letter free for commands.

**HLZ-008 — the VM silently dropped output parameters of module functions**
- `alias::f(x<~)` compiled without a `SetupOutputWriteback`: the caller's
  variable was never updated. No error, no warning, just the original value.
- `compile_import` reserved chunk slots for module functions but never
  registered their output-param flags, which only happened for the main
  program's own functions.
- Consequence downstream was an out-of-bounds crash far from the cause, because
  a board passed as `局面<~` silently never changed. It is why zy-GO shipped
  tree-walker only.
- Regression: `tests/modules_scope/out_param_module.zy` (TW == VM).

**HLZ-010 — the VM turned an interpolated constant into literal text**

- `"{DIR}/f.txt"` inside a function body compiled to the eight characters
  `{DIR}/f.txt` under `--vm`. `compile_interpolated_string` looked the name up
  in the local registers and, on failure, fell straight through to its
  literal-text branch — never consulting `global_consts`, where every top-level
  constant lives, nor `global_var_map`.
- Silent, and scoped in a way that hid it: the same string at the top level
  worked, and a direct `>> K` inside the same function worked. Only
  interpolation, only inside a function, only under the VM.
- Found by a benchmark harness that played sixty-four games and wrote zero
  records, because its output path was built this way. `io::write` was handed
  `{記録場所}/9路_0001.kifu`, failed softly, and said nothing.
- Regression: `tests/modules_scope/interp_global_const.zy`, covering all five
  constant types, module-level mutable state, and the genuinely-unknown name
  that must stay literal in both engines.

**HLZ-009 — the VM could not slice a String inside a module function**
- `s$[3..]` raised "expected Array, Tuple, or NamedTuple, got String". The
  `ArraySlice` instruction handled the three collection types but not String,
  and it was only reached when the subject was a runtime value rather than a
  literal the compiler could fold — so the gap appeared inside module functions
  and nowhere else.
- Regression: `tests/modules_scope/out_param_module.zy`.

With HLZ-008 and HLZ-009 fixed, all six zy-GO test suites pass under `--vm` as
well as the tree-walker.

**HLZ-007 — juxtaposition stopped at the first delimiter**

- Implicit concatenation only existed at statement level. `f(a " " b)`,
  `[a " " b]` and `(a " " b)` were all parse errors, so any composed string
  handed to a function needed an intermediate variable first.
- The finding was originally filed against string interpolation (`"{t.field}"`
  is rejected), but measuring zy-GO's side panel showed the interpolation limit
  was not what cost anything: nearly every intermediate variable there holds a
  *call*, not a field access, and no interpolation syntax would remove them.
  Two walls stood next to each other — interpolation admits only identifiers,
  and juxtaposition did not reach inside an argument list — and only the second
  one was load-bearing.
- Juxtaposition now works in call arguments, array elements, tuple elements and
  grouped expressions, with the same same-line rule as at statement level. A
  comma still separates arguments; a following `(` never continues the chain in
  these positions, because there it is ambiguous with a lambda, a tuple and a
  grouped expression.
- Parser-only change: the AST node (`BinaryOp::Concat`) already existed, so the
  tree-walker, the compiler, the VM and the formatter needed no changes.
- Cost of the trade: `f(a b)` with a forgotten comma now concatenates instead of
  raising a parse error.
- Regression: `tests/strings/30_juxtaposition_delimited.zy` (TW == VM),
  covering arguments, nesting, commas, arrays, tuples, groups, `$*` composition
  and a lambda argument that must stay a lambda.

**HLZ-011 — a variable used only as a range bound was reported as unused**

- `total = xs$#` followed by `@ i:1..total { }` warned "unused variable
  'total'" even though the loop reads it. `analyze_expr`'s `Expr::Range` arm was
  a no-op, with a comment claiming the bounds were "literals/identifiers only" —
  but `start`, `end` and `step` are full `Box<Expr>`, so any variable used only
  as a bound never counted as a use.
- The warning surfaced non-deterministically (it depended on how many other
  variables shared the scope), which is why it hid: zy-GO's `設定描画` tripped it
  while the structurally identical `助言描画` did not.
- The arm now visits `start`, `end` and `step`. A genuinely unused variable
  still warns.
- Regression: `crates/zymbol-semantic/tests/underscore_semantics.rs`
  (`test_variable_used_only_as_range_bound`,
  `test_variable_used_as_range_start_and_step`,
  `test_genuinely_unused_still_warns`).

**MM-1 — `x°`/`°x` inside a function called from a `@` loop panicked the tree-walker**
- `loop_scope_depths` (the anchor indices for hot definitions) is now saved and
  restored across call boundaries in `SavedCallState`. Inside a function with no
  loops of its own, hot definitions anchor to the function scope, as documented.
- Regression test: `tests/bugs/bug_mm1_hot_def_fn_scope.zy` (TW == VM).

**MM-2 — module-state mutations made by intra-module calls were lost (TW)**
- Write-back now runs for every module frame (both `alias::` calls and bare-name
  intra-module calls) and is **diff-based**: only keys whose value changed
  relative to the injected snapshot are persisted, so an outer frame can no
  longer clobber a nested call's write-back with its stale copy.
- Same-module nested calls inject the caller's live values (not the stale store)
  and refresh the caller's copies on return, so sequential reads/writes across
  intra-module call chains are consistent.
- Parameters named like module variables shadow them and are excluded from
  write-back.
- Regression test: `tests/bugs/bug_mm2_module_state_helper.zy` (TW == VM).

**MM-3 — `\ x` inside a function poisoned the caller's same-named variable (TW)**
- `dead_variables` is now frame-local (saved/restored in `SavedCallState`);
  destroying a name inside a callee no longer raises a false
  `use after destruction` on the caller's variable.
- Regression test: `tests/bugs/bug_mm3_destroy_frame_local.zy` (TW == VM).

**MM-4 — modules loaded at runtime skipped semantic analysis**
- `zymbol run` now applies the same semantic gate as the entry file when
  importing a module — in **both** engines (tree-walker `load_module` and VM
  `compile_import`). A module whose function reassigns a `:=` constant fails at
  import time with a semantic error instead of executing with split-brain state.
- Defense in depth: module constants are re-marked `const` inside module
  function frames, so reassignment is a runtime error even if static analysis
  is bypassed.
- Regression test: `tests/bugs/bug_mm4_module_const_guard.zy` (TW == VM,
  identical error text).

**MM-10 / L23 — VM: each import alias got its own module state copy**
- The VM compiler recompiled a module on every import, allocating fresh
  global-variable slots per alias — two aliases to the same file (or a diamond
  dependency) held divergent state. Compiled modules are now cached by
  canonical file path: any later import binds its alias to the same chunks and
  global slots, matching the tree-walker's per-path state identity.
- Regression test: `tests/bugs/bug_mm10_alias_shared_state.zy` (two aliases +
  diamond, TW == VM).

**MM-11 / L24 — VM: leftover loop-iterator value diverged from the tree-walker**
- VM range loops used the named iterator's register as the loop counter, so
  after the loop it held the first out-of-range value (TW: last executed
  value), and body writes to the iterator could alter the iteration. The VM
  now advances a hidden counter and publishes it to the named variable at the
  top of each iteration — leftover and body-write semantics match the
  tree-walker in all range variants (step, reverse, break).
- Regression test: `tests/bugs/bug_mm11_iterator_leftover.zy` (TW == VM).

**MM-9 — root-scope constants vanished at call depth ≥ 2 (TW)**
- Constants declared with `:=` at the top level of a script are now recorded in
  a global constant table that is not swapped by call frames: they resolve at
  any call depth, through recursion and lambda frames, and stay immutable
  everywhere (`is_const` consults the table). Module frames do not see script
  constants (module isolation preserved). Constants declared inside blocks or
  function bodies remain lexically scoped; block-local constants are still
  forwarded (and now re-marked) one frame at a time.
- A parameter may shadow a forwarded constant — it stays assignable.
- Regression test: `tests/bugs/bug_mm9_const_call_depth.zy` (TW == VM).

- **Legacy export separators now name themselves.** `<=` or `:` inside a `#> {}`
  block reported `expected identifier in export item`, pointing at the separator
  while asking for an identifier the author had already written. Both now produce
  `legacy export rename separator` with a help line giving the `=>` form; the
  generic case gained a help line too (`crates/zymbol-parser/src/modules.rs`).

### Documented

- **MM-5**: constants pierce function isolation by design (GUIDE §9 note).
- **MM-6**: the `@ var:` iterator reuses a pre-existing outer variable of the
  same name; the leftover value is the last executed iteration value in both
  engines (GUIDE §8 note).
- **MM-7**: `x°`/`°x` run in both engines — the stale "tree-walker only /
  `@vm-skip`" note was removed from the GUIDE.
- **MM-8**: module state identity is per file path — several aliases to the
  same module share one state in both engines (GUIDE §17 note).

**I18N.md rewritten.** The previous document was written against the pre-0.0.6
`<=` alias syntax and none of its 35 examples parsed: the breaking change that
introduced `=>` ([0.0.6], `feat(syntax)!`) updated the other reference documents
but touched only one line of this one. It is preserved as
[I18N_DEPRECATED.md](I18N_DEPRECATED.md) with a banner naming the cause; the new
[I18N.md](I18N.md) covers both internationalization mechanisms — re-export layers
for code (now including how to wrap `std/*` modules) and dispatcher modules for
runtime text, which the old document never documented at all. Every code block in
it is extracted to a clean tree and executed in both engines before publication.

- **Keys belong in the base language.** The runtime-text section documents the
  convention validated in zy-GO: i18n keys are concepts in the language the
  program is written in, each carrying a domain prefix (`区画.アゲハマ`, never
  plain `アゲハマ`). The prefix is what stops a key from ever equalling its own
  translation, which is what keeps completeness decidable in the base language.
- **GUIDE §Re-export**: the prose still said the rename separator was `:`, and a
  note still warned about L3 (`alias.CONST`) after it had been fixed.
- `tests/i18n/matematicas/` — the four module files declared `# .ελληνικά`
  instead of `# .matematicas_ελληνικά`, so they failed `zymbol check` with E001
  even though `zymbol run` executed them (the check does not reach modules
  arrived at through an import).
- The Mandarin translation layer both I18N documents describe never existed:
  `matematicas/中文.zy` and `中文_应用.zy` are now present with their golden
  file, so the suite covers the four languages the pattern claims.
- `tests/i18n/test_all_i18n.sh` invoked `app_coreano.zy`, `app_griego.zy` and
  `app_hebreo.zy`, none of which exist under those names — the script had been
  dead since the consumers were renamed. Rewritten around what
  `expected_compare.sh` does *not* do: it runs `zymbol check` on every layer
  (the gap that let E001 hide) and diffs the tree-walker against `--vm` for
  every consumer.

---

## [0.0.7] — 2026-07-02

### Added

**Native stdlib expansion — `std/json`, `std/io`, `std/net`**
- `std/json`: `decode(text)` / `encode(value)` — JSON object ↔ `NamedTuple`
  (key order preserved), array ↔ `Array`, `null` ↔ `Unit`. Soft `##Parse(...)`
  on malformed data.
- `std/json::decode_map(text, map)` — decodes **and** recursively renames
  object keys per a `NamedTuple` map (field name = source key, String value =
  new name), enabling **data-level i18n**: JSON keys from external APIs can be
  read in the consumer's language. Keys absent from the map are kept verbatim;
  an empty `()` map behaves like `decode`. Full VM parity (builtin id 202).
  Test: `tests/stdlib/stdlib_json_decode_map.zy`.
- `std/io`: `read`, `write`, `append`, `exists`, `delete`, `list`, `mkdir` —
  soft `##IO(...)` on filesystem failure.
- `std/net`: `get`, `post`, `post_json`, `head` (blocking HTTP, rustls TLS) —
  soft `##Network(...)` on failure. `get`/`post`/`post_json` accept an optional
  trailing `headers` argument: an array of `(name, value)` tuples.
- All modules ship with full VM parity and Spanish i18n adapters; wrong
  argument types are hard errors (see the REFERENCE.md error taxonomy).
- Example projects under `examples/`: `api_demo/` (public REST APIs) and
  `zethy_cli/` (AI assistant over `std/net` + `std/json` + `std/io`).

**Static undefined-function detection at `check` time**
- A bare-identifier call that is neither a known function, a variable, nor a
  module alias is now a semantic error instead of failing at runtime.

**`std/db` — vendor-neutral database access via ODBC**
- `connect/disconnect`, `exec`, `query/query_one/query_value`, `tx`,
  `begin/commit/rollback`, savepoints, `exec_script`, `table_exists`;
  Spanish i18n adapter (`db_es`). Full VM parity (builtin ids 500–514).
- Zymbol bundles no engine: the OS supplies the per-engine ODBC driver
  (validated live against SQLite and PostgreSQL with the same program).
- Availability: included in Windows prebuilt binaries (ODBC ships with the
  OS) and in source builds (default `db` cargo feature). The prebuilt
  Linux/macOS binaries are compiled without it — the ODBC driver manager
  needs `dlopen`, impossible in a fully static binary (REFERENCE.md L17).

**Formatter property harness** (`tests/scripts/fmt_property.sh`)
- Verifies P1 reparse, P2 idempotence, P3 runtime-output equality and
  P4 comment preservation over every `.zy` in `tests/` + `examples/`;
  `--baseline` mode gates CI on regressions.

**Typed/validated input via cast typespecs** (`<< ##.(5,2) "prompt" var`)
- Re-prompts until input matches the typespec (TW + VM parity).

**Deep functional update in the VM** (`arr[i>j>…]$~ val`)
- New `DeepSet` bytecode instruction: walks an index path (Int steps; String for
  a named-tuple field) and returns a new collection with the addressed element
  replaced. Previously a VM compile error ("collection update on non-index expr").
- All `$~` forms now route through `DeepSet`, which also fixes positional-tuple
  update in the VM (`t[2]$~ 999` failed with "expected Array, got Tuple" while
  the mutating `t[i] = val` correctly keeps rejecting tuples).
- New parity test: `tests/collections/deep_update.zy` (2/3-level paths, negative
  and computed indices, tuples, named-tuple nesting, `$~` inside a HOF lambda).

### Changed

**Nested `Unit` now displays as `()` in the tree-walker (engine display unification)**
- A `Unit` inside an array, tuple, or named tuple rendered as an empty hole in
  the tree-walker (`[1, , 3]`) but as `()` in the VM. Both engines now print
  `[1, (), 3]` — the last known display divergence between engines. Standalone
  `Unit` still prints nothing (both engines, unchanged). Affects e.g.
  `json::decode` of JSON `null` inside arrays/objects.
  Parity test: `tests/collections/unit_display_nested.zy`.

**Dismissed (validated with the language author, 2026-06-12)**
- `do-while ~>` (NI01) and match identifier binding (NI03) will NOT be
  implemented: their workarounds are the idiomatic forms, and coining new
  syntax for them violates the symbolic-minimalism rule. Marked as dismissed
  in ROADMAP.md, REFERENCE.md (L12), IMPLEMENTATION.md, and the EBNF.

**Formatter redesign — fail-closed and faithful**
- A safety gate (token equivalence, reparse, statement shape, comment
  count) refuses to emit non-equivalent output: `zymbol fmt` can no
  longer corrupt a file.
- The parser now preserves user parentheses (`Expr::Group`), assignment
  sugar (`+=`, `++`, `--`, indexed forms), `¶` vs `\\`, input typespecs,
  export-block commas and bracket forms, so the formatter reprints
  exactly what was written.
- Comments are re-emitted by source position (span interleaving); the
  old line-matching merge pass — the source of code-duplication bugs —
  was removed entirely.
- `FORMATTER_RULES.md` rewritten to match the enforced contract.

### Fixed

- `zymbol fmt` corrupted hot-def assignments (`total° += 10` became
  `total = total + 10`), dropped typed-input casts, stripped user
  parentheses (often breaking the program), moved the mutable-param
  marker (`num~` → `~num`) and could duplicate code while re-merging
  comments. All of these now format faithfully or fail closed.
- **L16 — `!?` corrupted the caller's scope when a called function failed.**
  Tree-walker: every error exit of a function/lambda call now restores the
  caller's state (`restore_call_state`) before propagating — previously the
  early `?` return left the function's isolated scope in place, making every
  outer variable undefined after the catch. VM: `raise!` now unwinds the
  frame stack to the nearest ancestor frame with an active catch (popping
  callee frames and registers) instead of only checking the top frame, so
  `:!` fires for errors raised at any call depth. Regression test:
  `tests/bugs/bug_l16_try_scope_restore.zy` (TW == VM).
- **L14 — destructuring silently overwrote `:=` constants.** Now a semantic
  error at `check` time ("cannot reassign constant"), consistent with direct
  reassignment. Test: `tests/errors/semantic/const_destructure_overwrite.zy`.
- Postfix operators in `>>` juxtaposition (former L1) covered by a new
  regression test: `tests/bugs/regression_postfix_output.zy`.
- **L9 and L13 verified already fixed; docs were stale.** L9: no more
  false-positive "unused variable" warnings for variables used only in string
  interpolation or BashExec (test: `tests/errors/semantic/no_false_positive_unused.zy`).
  L13: `$!!` inside lambdas propagates the error as an early return to the
  lambda's caller — identical to named functions, both engines (test:
  `tests/lambdas/error_propagate_lambda.zy`).
- **`zymbol check` printed every module error twice.** `ModuleAnalyzer::analyze`
  returned its findings in the `Err` AND retained them in `diagnostics()`; both
  channels were printed. `analyze` now drains its findings into the `Err`, so
  `diagnostics()` only carries `validate_exports` results. E00x semantic
  goldens regenerated (single occurrence per finding).
- **LSP diagnostic parity with `zymbol check`** (audit over the full 521-file
  corpus). The LSP pipeline now runs module analysis — E001 (name mismatch),
  E002 (module not found), E009 (duplicate export) and export validation were
  invisible in the editor. Ambiguous-lifetime severity raised from HINT to
  WARNING to match `zymbol check`. Remaining intentional difference: on files
  with parse errors the editor analyzes the recovered AST while `check` stops.
  New audit tool: `cargo run -p zymbol-analyzer --example dump_diagnostics`.
  Tests: `test_module_errors_in_pipeline`, `test_ambiguous_lifetime_is_warning`.
- Workspace metadata: real repository URL, ghost crate entries removed.
- Security: `bytes` 1.11.0 → 1.11.1 (RUSTSEC-2026-0007).

---

## [0.0.6] — 2026-06-07

### Changed (Breaking)

**`FatArrow` operator `=>` for match arms, import aliases, and export renames**
- Match arm separator: `pattern => result` (was `pattern : result`).
- Import alias separator: `<# path => alias` (was `<# path <= alias`).
- Export rename separator: `#> { fn => pub }` (was `fn <= pub_name`).
- Rationale: `=>` reads as "maps to" / "becomes" — unambiguous across all contexts.
- Full design history: `IMPL_V005.md §Feature-7`.

### Improved

**Standalone binaries now embed bytecode instead of source (~60% smaller)**
- `zymbol build` previously embedded the raw `.zy` source and re-ran the full
  pipeline (lex → parse → compile) on every execution, shipping lexer, parser,
  AST, compiler, and interpreter as dead weight in the standalone binary.
- New approach: compile to bytecode **at build time** inside `zymbol build`,
  serialize via `bincode`, and embed the bytes. The generated binary links only
  `zymbol-bytecode` + `zymbol-vm` (2 crates instead of 7).
- `zymbol-bytecode`: all types (`CompiledProgram`, `Instruction`, `Chunk`,
  `GlobalInit`, `BuildPart`, `HotNeutral`) now derive `Serialize`/`Deserialize`.
- `zymbol-standalone`: `write_source()` replaced by `write_bytecode()`;
  `new_from_source` accepts `base_dir` for module resolution via
  `Compiler::compile_with_dir`.
- Template `main.rs`: 16 lines — `bincode::deserialize(BYTECODE)` + `vm.run()`.
- **Result: serpiente standalone 2.2 MB → 901 KB (~2.4× smaller). VM execution
  replaces tree-walker, startup has zero lex/parse/compile overhead.**
- This is also the foundation for the upcoming `.zyb` bytecode file format
  (see ROADMAP — "Bytecode File Format").
- Reported by **[@wux4an](https://github.com/wux4an)** in
  [interpreter#1](https://github.com/zymbol-lang/interpreter/issues/1) —
  whose honest critique of the `zymbol build` limitations on release binaries
  directly motivated this redesign.

### Added

**Typed wildcards in test golden files (`***time***`, `***float***`, etc.)**
- `.expected` files now support typed regex wildcards alongside the existing `****` glob:
  - `***int***` — any integer (`-?[0-9]+`)
  - `***float***` — any float, including scientific notation
  - `***num***` — any number (int or float)
  - `***time***` — execution timing values such as `0.167s` or `12ms`
  - `***date***` — ISO 8601 dates such as `2026-05-26`
  - `***path***` — any non-whitespace path
- Matching uses `re.fullmatch` via Python 3 (always available); falls back to
  the existing `****` glob when Python 3 is absent.
- New `--regen --smart` flag in `expected_compare.sh`: automatically detects
  timing and date patterns in the output and replaces them with the corresponding
  wildcard. Fixes `stress_v2/bench_*.zy` tests that failed due to timing variance.
- Same typed wildcards available in `semantic_compare.sh`.
- `vm_compare.sh`: restored `tests/manual/` files (466 total; 463 PASS + 3 `@vm-skip`).

**REPL test harness — headless integration and CLI tests (`zymbol-repl`)**
- New `tests/common/mod.rs` — `ReplTestHarness`: wraps `Interpreter<Vec<u8>>` for TTY-free testing.
  Supports mocked `<<` input via `set_input_fn` + `Rc<RefCell<VecDeque<String>>>`.
  Methods: `run_line`, `output`, `value`, `error`, `history`, `variables`.
- `tests/repl_integration.rs` — 27 integration tests covering: pIqaD digit/letter/alphabet roundtrip,
  variable listing, history order and empty-line skipping, `<<` input (no-prompt, whitespace trim,
  empty string, numeric cast int/float, with prompt, sequential, usable in expression),
  `<<|?` non-blocking (headless → `'\0'`, no error), `<<|` blocking (headless → error),
  `>>|` TUI block (headless → raw-mode error), RESET scope, undefined-variable and syntax errors.
- `tests/cli_repl.rs` — 11 CLI subprocess tests using `assert_cmd`: basic output, arithmetic,
  pIqaD roundtrip, variable persistence, `>>~` ANSI escape sequences (single/multiple/multichar),
  `>>!` clear-screen ANSI codes, `>>?` terminal size and condition.
- `line_editor.rs` — 4 new unit tests: `cursor_word_left/right`, `delete_word_before/after`.

**REPL improvements (zymbol-repl)**
- `count_display_width`: `s.chars().count()` → `s.width()` (unicode-width crate).
  Fixes cursor misalignment with CJK (2-col), emoji (2-col), pIqaD PUA (1-col, unchanged).
- Word navigation: `Ctrl+Left/Right` (word jump), `Ctrl+W` (delete word before), `Alt+D` (delete word after).
- Persistent history: loaded from `~/.zymbol_history` on startup, saved on exit.
- Batch mode (`start_batch`): detects piped stdin via `IsTty`; runs without raw mode — enables
  `assert_cmd` CLI tests and `zymbol repl < file.zy` scripted usage.
- `RESET` command: clears all variables and function definitions from the interpreter scope.
- New dependencies: `unicode-width = "0.2"`, `dirs = "5"`.

**VM fix — `<<|` blocking key input propagates error in headless mode**
- `Instruction::ReadKey` (blocking): `Err(_) => break '\0'` replaced with
  `Err(e) => return Err(VmError::Generic(e.to_string()))`.
  Tree-walker and VM now produce identical output (`Runtime error: Failed to initialize input reader`)
  when `<<|` runs without a TTY, enabling automated TW/VM parity testing.

**`@vm-skip` removed from all three `tests/manual/tui/` files**
- `05_key_input.zy`: VM now errors identically to TW on headless `<<|`.
- `06_tui_block.zy`: TW and VM always produced identical headless error; tag was unnecessary.
- `07_output_pos_sparse.zy`: TW and VM always matched; tag was unnecessary.
- `vm_compare.sh` result: **478/478 PASS, 0 FAIL, 0 SKIP** (was 466 PASS + 3 `@vm-skip`).

**New E2E test category: `tests/input/`**
- 8 golden-file tests for the `<<` input statement, each with a `.input` companion file:
  `01_basic`, `02_numeric` (int), `03_multiple` (3 sequential), `04_trimming` (whitespace stripped),
  `05_prompt_displayed`, `06_in_condition` (drives branch), `07_in_loop` (accumulates sum), `08_numeric_float`.
- `.input` companion files: `expected_compare.sh` and `vm_compare.sh` now auto-detect a `.input`
  file alongside any `.zy` test and pipe it as stdin — no script arguments required.

**New TUI E2E tests in `tests/tui/`**
- 4 new golden files: `05_output_pos_multiple` (3 consecutive `>>~`), `06_clear_then_pos` (`>>!` + `>>~`),
  `07_terminal_size_ops` (`>>?` decomposed + arithmetic), `08_output_pos_multichar` (multi-char `>>~` text).

**GAP-Z009 regression test — named functions retain module aliases as HOF values**
- New test `tests/bugs/bug_named_fn_module_alias_hof.zy` covers the case where a named
  function referencing a module alias (e.g. `mat::sqrt`) is passed as a first-class value
  to a higher-order function and invoked from inside it. Previously failed with
  `"undefined module alias: 'mat'"`.

**`<<` input support in VM (IMPL-V005-INPUT)**
- `ReadLine(dst, Option<Reg>, bool)` — new bytecode instruction. Reads a line from stdin,
  optionally printing a prompt register first; `bool` flag enables numeric cast (Int/Float/String).
- Compiler: `Statement::Input` now compiles to bytecode. Simple prompts → `LoadStr`; interpolated
  prompts → `BuildStr` from `Vec<StringPart>`. `InputCast::Numeric` passes `true` to `ReadLine`.
- VM: handler mirrors interpreter behavior — inside TUI block (`tui_stack` non-empty) temporarily
  disables raw mode and shows cursor for input, then restores; numeric cast uses `normalize_unicode_digits`.
- Def-use analysis updated for `ReadLine`.

### Fixed

**BUG-007 — Semantic checker incorrectly rejected recursive integer functions after GAP-Z008**
- Functions like `gcd(a, b) { <~ gcd(b, a % b) }` were rejected with
  `"argument 2 has type Float, but function expects Int"` when the parameter `a`
  had only a `Numeric` constraint (no direct Int evidence). `Numeric.to_type()`
  returns `Float` (GAP-Z008), making `a % b` resolve to `Float`, which then
  failed against `b`'s `Exact(Int)` type in the recursive call.
- Root cause: `types_compatible_static()` in `zymbol-semantic/src/type_check.rs`
  had no `(Float, Int)` arm — fell through to `_ => false`.
- Fix: added `(ZymbolType::Float, ZymbolType::Int) => true` — bidirectional
  numeric compatibility, consistent with the runtime's dynamic dispatch (BUG-Z001).
- New test: `tests/bugs/bug_semantic_numeric_recursive.zy`.

**`<<` inside `>>|` TUI block freezes terminal**
- `execute_input` (interpreter) and VM `ReadLine`: when `tui_depth > 0` / `tui_stack` non-empty,
  now calls `disable_raw_mode()` + `cursor::Show` before reading and restores after.
  Previously `read_line()` blocked indefinitely because raw mode discards `\n`.

**`>>|` cursor not at (1,1) on entry**
- `execute_tui_block` and VM `EnterTui`: added `cursor::MoveTo(0, 0)` immediately after
  `EnterAlternateScreen`. Some terminals inherit the main-screen cursor position, causing the
  first `<<` prompt or `>>~` output to appear at arbitrary rows.

### Test suite — [0.0.6]

| Suite | Result |
|-------|--------|
| `cargo test` (all crates) | **820 / 820 pass** |
| `expected_compare.sh` (all) | **464 / 464 pass** |
| `expected_compare.sh tui` | **8 / 8 pass** (+4 new: multi-pos, clear+pos, size-ops, multichar) |
| `expected_compare.sh input` | **8 / 8 pass** (new category: `<<` with `.input` companion files) |
| `expected_compare.sh gaps` | **33 / 33 pass** |
| `expected_compare.sh bugs` | **19 / 19 pass** |
| `vm_compare.sh` | **478 / 478 PASS, 0 SKIP** (was 466 PASS + 3 `@vm-skip`) |
| `zymbol-repl` unit + integration + CLI | **48 / 48 pass** (10 unit · 27 integration · 11 CLI) |

---

## [0.0.5] — 2026-04-29

### Added

**Hot Definition operator `°` (U+00B0) — two-form scope anchoring**
- Two LHS forms with distinct scope lifetimes:
  - `x° op= n` (postfix) — anchors to the nearest enclosing `@` scope; variable dies when the loop ends.
  - `°x op= n` (prefix) — anchors to the scope **above** the nearest `@`; variable survives the loop.
  - Outside any loop both forms anchor to global/function scope (no difference).
- RHS hot read `p = p° + c` — returns neutral if undefined, does not anchor to any scope.
- Neutral values: `+=`/`-=` → `0`/`0.0`; `*=`/`/=` → `1`; array `$+` → `[]`; string juxtaposition → `""`.
- Warning emitted for semantically vacuous hot-def: `x° ^= 2` → always 0.
- Undefined variable error now includes hint: `'x' is undefined — did you mean 'x°' (hot definition)?`
- Implemented across: lexer (`HotIdent`, `PreHotIdent` tokens), parser (`hot`/`pre_hot` fields on `Assignment`),
  interpreter (loop scope stack with `push_loop_scope`/`set_at_nearest_loop`/`set_above_nearest_loop`),
  semantic type-checker with recursive pre_hot scan for nested `?`/`@` blocks.

**TUI / Terminal primitives (IMPL-V005)**
- `@~ N` — sleep N milliseconds. Implemented via `std::thread::sleep`; emits `Sleep` bytecode in VM.
- `>>!` — clear terminal screen (ANSI `\x1b[2J\x1b[H`). New `ClearScreen` instruction in VM.
- `>>?` — query terminal size; returns `(rows, cols)` positional tuple via crossterm. New `QueryTermSize` instruction.
- `>>~ (row, col, BKS, fg, bg) > items` — positioned output with optional style. Sparse syntax:
  any slot may be omitted (`>>~ (,,, 196) > "red"` sets fg only; `>>~ (3, 1) > "text"` positions only).
  BKS bitmask: `1`=Bold, `2`=Italic, `4`=Underline. ANSI 256-color palette (0=terminal default).
  Variable-based: `pos = (3, 1)` then `>>~ pos > "text"`.
- `<<| var` — blocking keypress read. Arrow keys → `'↑'`/`'↓'`/`'←'`/`'→'`; Enter → `'\n'`; Escape → `'\x1B'`.
- `<<|? var` — non-blocking keypress poll; returns `'\0'` if no key pending.
- `>>| { }` — TUI block: enters alternate screen + raw mode via crossterm; restores terminal on exit.
  New `EnterTui`/`ExitTui` instructions in VM with proper error propagation.
- Type-checker updated: `<<|` / `<<|?` now resolve without `undefined variable` error (GAP-S3 fix).
- New test directory `tests/manual/tui/` — manual cases covering all 6 TUI primitives.
- New VS Code snippets: `outp`, `outps`, `outpc`, `key`, `keynb`, `tui`, `sleep`, `cls`, `termsize`.

**String repeat operator `$*`**
- `"string" $* N` repeats a string N times. Implemented in tree-walker (`strings.rs`) and VM
  (`StrRepeat` instruction).
- New test: `tests/gaps/gap_serpiente_string_repeat.zy` (GAP-S1).
- VS Code grammar: `$*` added to `collection-operators` character class.
- New VS Code snippet: `repeat`.

### Fixed

**BUG-005 — VM tuple `==` and `<>` always returned `#0`**
- Tuple equality (`==`) and inequality (`<>`) in `--vm` mode always evaluated to `#0` regardless
  of actual content.
- Root cause: `cmp_direct()` and `Value::equals()` in `crates/zymbol-vm/src/lib.rs` had no `Tuple`
  arm — fell through to the `_ => 1` (not-equal) and `_ => false` defaults.
- Fix: recursive element-wise comparison added to both functions.
  `cmp_direct`: compares element by element, returns first non-zero or 0 if all equal.
  `Value::equals`: `a.len() == b.len() && zip.all(|(x, y)| x.equals(y))`.
- Additionally: `vm_extract_pos()` now unwraps a single-element outer tuple produced by the
  compiler when a variable-based `>>~ pos > ...` is used (was silently failing to move cursor).
- Root cause of Serpiente v0.0.5 bug: food detection `cab == (fr_com, fc_com + 1)` always false
  in VM mode → `comio` never set → score stayed 0 → second fruit never spawned.
- New test: `tests/bugs/bug_vm_tuple_equality.zy` (7 cases: literal, variable, arithmetic slot,
  `<>` inequality, Serpiente food-collision pattern, conditional, nested tuples).

**BUG-001 — Re-exported functions lose origin module scope**
- Functions accessed through an i18n re-export adapter (`alias::fn : newname`) raised
  `undefined variable` for any module-level variable the function read.
- Root cause: `eval_traditional_function_call` loaded context from the adapter module path,
  which carries no variables.
- Fix: `FunctionDef` now carries `origin_module_path: Option<PathBuf>`. The call site
  derives `effective_path` from that field, falling back to the caller's module only when
  the function has no recorded origin.
- New test: `tests/bugs/bug001_scope_reexport.zy` (3-file i18n fixture).

**BUG-002 — `><` CLI args capture not registered in semantic scope**
- `zymbol check` and the LSP reported `undefined variable` for any use of the captured
  identifier inside blocks (`? {}`, `@ {}`, etc.) after `>< args`.
- Root cause: `Statement::CliArgsCapture` had no handler in `type_check.rs`.
- Fix: added handler that calls `env.define_var(name, Array(String))`.
- New test: `tests/bugs/bug002_cli_args_scope.zy`.

**BUG-003 — LSP percent-decodes Unicode directory names in file URIs**
- VS Code sends `file:///home/user/%E6%BA%90%E7%A0%81/mod.zy` for paths inside directories
  with Unicode names (e.g. `源码/`). The LSP resolver built a path with the literal
  percent-encoded segment, which does not exist on the filesystem → `module-not-found` for
  every import inside those directories. CLI was unaffected.
- Fix: `uri_to_path` in `workspace.rs` now calls `percent_decode` before constructing the
  `PathBuf`. Multi-byte UTF-8 sequences (e.g. `源` = 3 bytes) are collected as raw bytes
  before UTF-8 reconstruction. No new dependencies.
- Four new unit tests in `workspace.rs`: encoded Unicode, plain Unicode, `%2F`, no-op.

**GAP-001 — Arithmetic expressions as slice bounds `$[start..end]`**
- `$[pos-1..end]` or `$[start..pos+1]` caused a parse error; only literals and plain
  identifiers were accepted as bounds.
- Root cause: `parse_collection_slice` called `parse_postfix` for bounds, which stops
  before `+`/`-` and cannot consume `..` without ambiguity.
- Fix: new `parse_slice_bound()` method in `collection_ops.rs` wraps `parse_postfix` with
  a `+`/`-` loop, stopping before `..`. Replaces all three bound call-sites in
  `parse_collection_slice`.
- New test: `tests/gaps/gap001_slice_arith_bounds.zy`.

**GAP-002 — Parenthesized expressions not accepted as `$++` items**
- `"prefix" $++ (expr)` failed with a parse error; `>>` accepted the same form correctly.
- Root cause: `parse_string_insert` gated item collection with `can_juxtapose()`, which
  intentionally excludes `LParen` to avoid lambda-comparator ambiguity in `$^+`.
- Fix: local `can_start` flag in `parse_string_insert` adds `TokenKind::LParen` without
  modifying `can_juxtapose` globally. `$^+` and juxtaposition chains are unaffected.
- New test: `tests/gaps/gap002_concat_paren_items.zy`.

**GAP-003 — `ambiguous lifetime` warning on every loop iterator variable**
- `@ elem:arr { }` always emitted `warning: ambiguous lifetime for 'elem'` regardless of
  whether the programmer had signalled intent.
- Fix in `def_use.rs` — two suppression rules, no new syntax:
  1. `_` prefix (`@ _elem:arr`): existing "intentionally ignored" convention now also
     suppresses the lifetime warning, consistent with unused-variable suppression.
  2. Pre-defined variable (`x = 0` then `@ x:arr`): if the variable already has a
     definition before the loop, the reuse is deliberate and no warning is emitted.
  Normal unnamed iterator variables still warn as before.
- New test: `tests/gaps/gap003_loop_iter_lifetime_warning.zy`.

**TUI-FIX-01 — `>>` inside `>>| {}` invisible before `<<|` key read**
- `execute_output()` writes to Rust's `Stdout` which is line-buffered; text without `\n`
  stays in the internal buffer until the next explicit flush.
  Inside a TUI block, the next flush was triggered by the following `>>~` call — always
  after the `<<|` read — so `>> "text"` before a key read was never visible to the user.
- Fix: added `tui_depth: u8` counter to `Interpreter`. `execute_tui_block()` increments it
  before executing the body and decrements it on exit. `execute_output()` calls
  `self.output.flush()` when `tui_depth > 0`.

**TUI-FIX-02 — `¶` / `\\` inside `>>| {}` broke column alignment**
- In raw mode, `\n` (LF) moves the cursor down but does NOT return to column 1.
  `execute_newline()` used `writeln!()` which emits only `\n`, causing subsequent text
  to appear offset from the left edge.
- Fix: `execute_newline()` emits `"\r\n"` (CRLF) when `tui_depth > 0`, `"\n"` otherwise.

**TUI-FIX-03 — TUI tokens not recognized as statement starters inside `>>`**
- Six v0.0.5 tokens (`KeyBlock`, `KeyNonBlock`, `OutputPos`, `OutputClear`, `OutputGate`,
  `AtTilde`) were absent from the statement-break list in `parse_output()`.
  When any of them appeared on the line after a `>>` statement, the parser attempted to
  consume them as output expressions and failed with `"expected expression, found KeyBlock"`.
- Fix: all six tokens added to the break-pattern in `crates/zymbol-parser/src/io.rs`.

### VS Code extension — v0.1.2

**Syntax highlighting:**
- `$*` added to `collection-operators` character class in `zymbol.tmGrammar.json`.
  `$*` was missing; strings and numbers using it were unhighlighted.

**Snippets (`zymbol.json`):**
- `outps` — `>>~ (row, col, BKS, fg, bg) > value` — positioned print with full style
- `outpc` — `>>~ (,,,fg) > value` — set foreground color without moving cursor
- `repeat` — `"str" $* n` — string repeat
- `hotacc` — `total° += value` — hot definition accumulator
- (Existing snippets `outp`, `key`, `keynb`, `tui`, `sleep`, `cls`, `termsize` shipped in this release.)

Built: `zymbol-lang-0.1.2-2026-05-04.vsix`

### Test suite — v0.0.5

| Suite | Result |
|-------|--------|
| `cargo test` (all crates) | all pass |
| `expected_compare.sh` (all) | **424 / 424 pass** |
| `expected_compare.sh gaps` | **20 / 20 pass** (+ gap001–003, gap-S1–S4) |
| `expected_compare.sh bugs` | **16 / 16 pass** (+ bug-S01, bug-S02, BUG-005) |
| `expected_compare.sh tui` | **5 / 5 pass** (2 vm-skip: `<<\|`, `>>|`) |

---

## [0.0.4] — 2026-04-16

### Breaking Changes

- **1-based indexing across all collections** — `arr[1]` is the first element.
  Index `0` now raises a runtime error instead of silently returning a value.
  Affects: arrays, tuples, named tuples, and strings.
  Negative indices are preserved: `arr[-1]` still means last element.

### Added

**Multi-dimensional indexing** (`arr[i>j>k]`)
- Scalar deep access: `arr[i>j>k]` — navigate nested arrays to a single value.
- Flat extraction: `arr[p ; q]` or `arr[[i>j]]` — returns a flat `Array`.
- Structured extraction: `arr[[g] ; [g]]` — returns an `Array` of `Arrays`.
- Range steps: `arr[i..j > k]` — range over one navigation dimension.
- Nested ranges: `arr[[i..j] ; [k..l]]` — double fan-out.
- New MANUAL.md section §11c. New test directory `tests/index_nav/` (15 cases).
- Deprecated: chained `arr[i][j]` syntax (still works, no longer recommended).

**Type conversion casts**
- `##.expr` — convert to `Float`.
- `###expr` — convert to `Int` (round).
- `##!expr` — convert to `Int` (truncate).
- New tokens: `HashHashDot`, `HashHashHash`, `HashHashBang`.
- New test directory `tests/casts/` (6 cases).

**String operations**
- `string$/ delim` — split string by delimiter, returns `Array(String)`.
- `base$++ a b c` — ConcatBuild: concatenate/append multiple items in one expression.

**Interpolated string literal**
- New `Literal::InterpolatedString` variant — strings with `{var}` are distinguished
  at compile time. Literal braces are escaped with `\{` and `\}`.

**Module system**
- Circular import detection: raises a clear `RuntimeError::CircularImport` instead
  of a stack overflow. The detection set propagates transitively to sub-modules.
- Private functions in modules can now call each other (intra-module calls, BUG-01 fix).
- Re-export from another module via `ExportItem::ReExport` (used by i18n nested modules).
- **Closed block syntax** (`# name { ... }`): module body is now explicitly delimited by
  braces. Flat/open syntax is no longer valid. Any token after the closing `}` is a parse
  error. `<#` imports, `#>` export block, literal constants, literal variables, and function
  definitions are the only elements permitted inside the block.
- **E013 — ExecutableStatementInModule**: new semantic error raised when an executable
  statement (`>>`, `<<`, function call, `?`, `@`, `!?`, `<~`, `<\ \>`, etc.) appears at
  module top-level. Variable and constant initializers must use a literal RHS; non-literal
  initializers also trigger E013.
- All existing module files migrated to block syntax (modules_scope, gaps, bugs, i18n).
- New tests `11_block_syntax_basic` and `12_private_state_block` covering block syntax
  end-to-end and private mutable state persistence inside blocks.
- MANUAL.md §17 rewritten: required block syntax, allowed/forbidden element table,
  E013 reference, all code examples updated.
- **E001 enforcement**: `# name { }` declaration must exactly match the filename stem.
  Dot-prefix convention (`# .name`) supported for subdirectory modules.
  E001 was previously defined but not triggered; it now fires on every `zymbol check`.
- **Module-file guard**: `zymbol run module.zy` detects a module declaration and exits
  with a clear error instead of silently doing nothing. Exit code 1.

**VM — full parity (320/320 tests)**
- Module private mutable state: new instructions `LoadGlobal(Reg, u16)` and
  `StoreGlobal(u16, Reg)`, `global_vars: Vec<Value>` field in the VM, `GlobalInit`
  in `CompiledProgram`.
- Float type propagation: Sub/Mul/Div/Pow now set `StaticType::Float` so downstream
  operations select the correct Float instruction variant.
- Lambda support in HOF: ~40 missing instruction arms added to `call_function`
  hot-loop; `$>` map / `$|` filter / `$<` reduce with lambdas now work in `--vm` mode.
- List pattern compilation: `??` match with `[a, b, *rest]` patterns now compiles
  to bytecode.
- Unicode numeric eval: `normalize_unicode_digits` converts any of the 69 supported
  Unicode digit scripts to ASCII before `#|expr|` evaluation.

**Test suite**
- `tests/errors/runtime/` — 10 regression cases: one per catchable/runtime error type
  (div-zero, index-zero, index-oob, type-cast, undefined-var, module-not-found, E004,
  E008, E010, E012). Verified with `expected_compare.sh errors/runtime`.
- `tests/errors/catchable/` — 5 catch-block cases: `##Div`, `##Index`, `##Type`,
  generic `:!`, and a combined all-types sequence. Verified with `expected_compare.sh errors/catchable`.
- `tests/errors/semantic/` — 18 semantic regression cases (E001–E013 + support modules).
  Verified with the new `tests/scripts/semantic_compare.sh` (uses `zymbol check`).
- `tests/scripts/semantic_compare.sh` — new script: runs `zymbol check`, strips ANSI
  codes, supports `****` wildcards and `--regen`. Mirrors `expected_compare.sh`.
- `tests/index_nav/` — 15 cases covering all navigation forms and error bounds.
- `tests/casts/` — 6 cases: to_float, to_int_round, to_int_trunc, expressions, errors.
- `tests/gaps/` — 8 cases: module const access, private state, export block position,
  BashExec edge cases.
- `tests/test_catch01–10` — 10 error-handling cases: basic, typed, finally, nested,
  loop, function, check, multiple, scope.
- `tests/scope01–05` — 5 scope cases: if block, nested blocks, loop block, match block,
  shadowing.
- 320 `.expected` files generated for the full VM parity suite.

**EBNF grammar** (`zymbol-lang.ebnf`, +226 lines)
- Formal rules: `nav_index`, `nav_path`, `nav_step`, `nav_atom`, `struct_group`.
- `numeric_cast_expr` rule for `##.`, `###`, `##!`.
- `index_suffix` updated: 1-based, negative indices supported.
- Comma-concat (`"a", b, "c"`) documented as removed; juxtaposition is canonical.

**Documentation** (MANUAL.md, +680 lines)
- New §11c Multi-dimensional Indexing.
- §4 Variables: subsections Variable Scope, Underscore Variables (`_name`),
  Explicit Lifetime End.
- §7 Match: List Patterns subsection.
- §11 Arrays: Negative Indices and Symmetric Slices subsection.
- §18 Data Operators: Type Conversion Casts subsection.
- §20 Known Limitations: L3 (module alias.CONST) and L4 (export block position)
  marked as Fixed.

### Changed

- All existing test cases in `tests/collections/`, `tests/lambdas/`, `tests/strings/`,
  and benchmarks migrated from 0-based to 1-based indexing.
- `packaging/publish-release.sh` and `packaging/templates/zymbol.wxs.in` updated.

### Fixed

- VM: arithmetic operations now propagate `StaticType::Float` correctly (was silently
  treating float results as Int in some compound expressions).
- Module constants: `take_variable` no longer corrupts module constants on write-back
  (was using a Unit sentinel; fix: `scope.remove(name)`).
- Limitation L3: `alias.CONST` now resolves correctly in all contexts.
- Limitation L4: `#>` export block can now appear after function definitions.
- False positive "unused variable" warnings for constants and variables listed in `#>`:
  `VariableAnalyzer` now marks exported items as used before emitting diagnostics.

### VM performance — Sprint 6A: Fused split intrinsics (2026-04-17)

New crate `zymbol-intrinsics` — pure Rust functions operating on `&str` / primitives,
zero VM types, zero boxing. Architecture mirrors CPython `Objects/unicodeobject.c`:
VM → adapter (unbox `ZyStr` → `&str`) → intrinsic fn → primitive → adapter (box → `Value`).
Circular dependencies avoided: `zymbol-intrinsics` has zero workspace dependencies.

**New crate `crates/zymbol-intrinsics/`:**
- `split.rs` — `count`, `count_str`, `first`, `last`, `join`, `join_str`, `count_where`, `parts`, `parts_str`.
- `search.rs` — `count_char`, `count_str`, `find_positions_char`, `find_positions_str`.
- `transform.rs` — `replace_char`, `replace_str`, `replace_n_char`, `replace_n_str`, `repeat`, `trim`.

**4 new fused bytecode instructions in `zymbol-bytecode`:**
- `StrSplitCount(dst, str, sep)` — fused `(s $/ sep)$#`; calls `intrinsics::split::count`, zero `Vec<Value>`.
- `StrSplitMap(dst, str, sep, fn)` — fused `(s $/ sep) $> fn`; iterates parts directly.
- `StrSplitFilter(dst, str, sep, fn)` — fused `(s $/ sep) $| fn`; no intermediate array.
- `StrSplitReduce(dst, str, sep, init, fn)` — fused `(s $/ sep) $< (init, fn)`; streaming fold.

**Compiler pattern detection (`zymbol-compiler`):**
- `compile_collection_length` detects `(s $/ sep)$#` → emits `StrSplitCount`.
- `compile_collection_map` detects `(s $/ sep) $> fn` → emits `StrSplitMap`.
- `compile_collection_filter` detects `(s $/ sep) $| fn` → emits `StrSplitFilter`.
- `compile_collection_reduce` detects `(s $/ sep) $< (init, fn)` → emits `StrSplitReduce`.
- `max_reg_used` updated with all 4 new instruction arms.

**VM dispatch (both sites) updated in `zymbol-vm`:**
- Both dispatch sites handle all 4 new instructions; `Char` and `String` separator variants dispatched.

**Benchmark (release, split-count inline vs 2-statement, 100 000 iterations):**

| Pattern | Time |
|---------|------|
| `(csv $/ ',')$#` (fused `StrSplitCount`) | 5 ms |
| `parts = csv $/ ','` ; `parts$#` (unfused) | 10 ms |

*50% reduction for the inline form. The 2-statement form cannot be fused without
dataflow analysis and still uses `StrSplit` + `ArrayLen`.*

---

### VM performance — Sprint 5G: Small String Optimization (2026-04-17)

`Value::String` payload changed from `Rc<String>` (always heap) to `ZyStr` — an 8-byte
tagged-pointer type that stores strings ≤ 7 bytes inline (no heap allocation, no atomic ops)
and falls back to a raw `Rc<String>` pointer for longer strings.

**`ZyStr` encoding (little-endian, 8 bytes):**
```
Inline (byte[7] bit7 == 1): bytes[0..len] = UTF-8 data, byte[7] = 0x80 | len
Heap   (byte[7] bit7 == 0): bytes[0..8] as u64 (LE) = raw *const String from Rc::into_raw()
```
Valid on x86-64 / arm64 where user-space pointers have bit 63 == 0.

**Changes in `crates/zymbol-vm/src/zy_str.rs` (new file):**
- `ZyStr::new(String)`: wraps the `String` directly in `Rc` (1 allocation for heap strings).
- `ZyStr::from_str_ref(&str)`: inline if ≤ 7 bytes, otherwise `Rc::new(s.to_string())`.
- `ZyStr::clone` (heap): `Rc::increment_strong_count` — single atomic op, no intermediate Rc value.
- `ZyStr::drop` (heap): `drop(Rc::from_raw(ptr))` — decrements and frees when last owner.
- `Deref<Target = str>`: all `&str` methods available on `&ZyStr` without `.as_str()` calls.
- 11 unit tests: size_is_8_bytes, inline/heap boundary, clone safety, Deref, Unicode.

**Additional micro-optimizations applied in the same sprint:**
- `StrSplit`: changed `ZyStr::new(p.to_string())` → `ZyStr::from_str_ref(p)`. Short split
  parts (≤ 7 bytes) now go inline with zero allocation.
- `ArrayRemove` (Array arm): replaced `rc_arr.as_ref().clone()` + `Rc::new(arr)` with
  `std::mem::replace` + `Rc::make_mut` — mutates the Vec in-place when refcount == 1,
  clones only when shared.
- `BuildStr` (both dispatch sites): added `String::with_capacity(sum_of_lit_lens + 4×reg_parts)`
  pre-pass to avoid reallocation during string interpolation.

**Benchmark results (VM, 5-run min, release binary):**

| Benchmark | Sprint 5F | Sprint 5G | Delta |
|-----------|-----------|-----------|-------|
| Stress core | 80 ms | 69 ms | −11 ms |
| Pattern Match | 74 ms | 43 ms | −31 ms |
| Recursion | 261 ms | 279 ms | +18 ms |
| Collections | 38 ms | 36 ms | −2 ms |
| Strings | 25 ms | 33 ms | +8 ms |
| Strings Stress | 42 ms | 56 ms | +14 ms |
| Strings Modify | 49 ms | 57 ms | +8 ms |

*Sprint 5F numbers from single-run baseline; Sprint 5G numbers from 5-run min. Net: CPU-bound
benchmarks (Stress, Pattern Match, Collections) improve; string-heavy benchmarks are neutral
to slightly worse because the benchmark strings are mostly > 7 bytes (bypass inline SSO path).*

---

### VM performance — Sprint 5F (2026-04-16)

Targeted micro-optimizations to the register VM hot paths.

**`StrReplace` char pattern — heap alloc eliminated**
- `zymbol-vm/src/lib.rs` `StrReplace`: char pattern previously built a temporary
  `String::with_capacity(4)` before calling `str::replace`. Changed to pass `char`
  directly as a Rust `Pattern`, eliminating one heap allocation per call.
- `StrReplaceN`: same problem; refactored to use a local `enum Pat { Ch(char), Str(&str) }`
  avoiding `c.to_string()` for both the `max == 0` and the bounded-replace paths.

**`Call` instruction — resize strategy confirmed optimal**
- Investigated replacing `value_stack.resize(n, Value::Unit)` + unsafe indexed overwrite
  with individual `push` calls per argument. Benchmarks showed `push` × n is slower than
  `resize` + `get_unchecked_mut` because `resize` produces a single vectorizable fill loop
  and the unsafe writes have no per-element branch overhead. Reverted; comment updated to
  document the trade-off for future reference.

**Benchmark delta (VM, 3-run avg, release binary):**

| Benchmark | Before | After | Delta |
|-----------|--------|-------|-------|
| Strings Modify | 51 ms | 49 ms | −2 ms |
| Recursion | 271 ms | 261 ms | −10 ms |
| Pattern match | 49 ms | 44 ms | −5 ms |
| Others | — | — | ±noise |

**Remaining structural gap vs Python (strings):**
`StrSplit`, `StrReplace`, and `StrReplaceN` each wrap their result in `Rc::new(String)` —
one unavoidable heap allocation per call with the current `Value` representation. Python
delegates these to C extensions with SIMD internals and no boxing. Eliminating the gap
requires Small String Optimization (SSO) in the `Value` enum — tracked for Sprint 5G.

---

### Post-release fixes (2026-04-16 review)

Six bugs and gaps identified during the v0.0.4 review session, all resolved same day.
Full record: `tests/BUG_v0.0.4.md`.

**BUG-NEW-01 — `<\` inside `#|...|` breaks NumericEval** (regression, v0.0.4)
- Introducing `BashOpen` (`<\`) caused the lexer to tokenize `<\` even inside
  NumericEval context (`#|...|`), breaking `#|<\ date +%s%N \>| / 1000000`.
- Fix: shell commands containing non-Zymbol tokens must be quoted:
  `<\ "date +%s%N" \>`. All 7 benchmark scripts updated.
- `lib_time.zy` and all benchmark string output corrected to juxtaposition (not `+`).
- All 7 Python comparison benchmarks restored to full operation.

**BUG-NEW-02 — Bool as array index not catchable by `!?`** (regression, v0.0.4)
- `arr[bool]` terminated the process with exit code 1, bypassing the `!?`/`:!`
  try/catch machinery.
- Fix (`zymbol-semantic`): `Bool` added to allowed index types so static analysis
  passes and the error reaches the runtime.
- Fix (`zymbol-vm`): `ArrayGet` changed from `as_int()?` to `raise!(...)` so the
  error is catchable in VM mode.

**BUG-NEW-03 — Cast error messages differed between WT and VM** (regression, v0.0.4)
- `##.`, `###`, `##!` on non-numeric values produced different error text in each
  execution path.
- Fix (`zymbol-interpreter/data_ops.rs`): replaced `{:?}` with a `value_type()`
  helper that yields the type name only (no value content).
- Fix (`zymbol-vm`): added `VmError::CastError { op, got }` variant; cast
  instructions now raise it instead of the generic `TypeError`.
- Both paths now emit: `"##. requires a numeric value, got String"`.

**GAP-01 — `\ var` (Explicit Lifetime End) was a no-op** (unimplemented)
- `Statement::LifetimeEnd` handler was a placeholder that did nothing; MANUAL §4
  documented it as functional.
- Fix (`zymbol-interpreter`): handler now calls `destroy_variable()`.
- Fix (`zymbol-compiler`): emits `LoadUnit(r)` and removes variable from
  `register_map`, preventing post-destroy use at compile time.

**BUG-PRE-01 — Two `cargo test` failures in `zymbol-formatter`** (pre-existing)
- `test_format_loop` and `test_format_foreach_loop` used inputs without the required
  space after `@` (`@x<10{...}` instead of `@ x<10{...}`).
- Fix: test inputs corrected; `cargo test -p zymbol-formatter` now passes 52/52.

**BUG-PRE-02 — `test_string_literal_braces` asserted wrong layer output** (pre-existing)
- The lexer stores `\{` as the `\x01` sentinel (ASCII SOH) to prevent it from being
  consumed as a string-interpolation delimiter. The test expected the post-runtime
  resolved form (`{`) from the raw lexer token.
- Fix: assertion updated to `"Use \x01curly} braces literally"` with a comment
  explaining the two-phase design.

### Test suite — v0.0.4 final state

| Suite | Result |
|-------|--------|
| `cargo test` (all crates) | **717 / 717 pass** |
| `vm_compare.sh` (WT vs VM parity) | **350 / 350 pass** |
| `run_all.sh` (7 benchmark suites) | **7 / 7 pass** |

**Benchmark results** (`run_all.sh --runs 3`, release binary):

| Benchmark | Zymbol tree-walker | Zymbol VM |
|-----------|-------------------|-----------|
| Stress core | 224 ms | — |
| Pattern match | 177 ms | — |
| Recursion (`fib(30)` + `ackermann(3,6)`) | 1 760 ms | — |
| Collections | 61 ms | 33 ms |
| Strings | 45 ms | 36 ms |
| Strings Stress | 123 ms | — |
| Strings Modify | 62 ms | — |

The recursion benchmark is dominated by `fib_rec(30)` (2.7 M recursive calls in the
tree-walker); iterative and VM paths are significantly faster.

---

## [0.0.3] — 2026-04-09

### Added

**Numeral Modes** (`#d0d9#` syntax)

Zymbol can display numbers in any of **69 Unicode digit scripts** at runtime.
The mode-switch token `#d0d9#` takes the zero-digit and nine-digit of the target
script enclosed in `#…#`. It persists until the next mode-switch in the same file.
Mode is file-local — modules never inherit or alter the caller's active script.

```zymbol
#०९#   // activate Devanagari
>> 42  ¶    // → ४२
>> 3.14 ¶   // → ३.१४

#٠٩#   // activate Arabic-Indic
>> 42 ¶     // → ٤٢

#09#   // restore ASCII
>> 42 ¶     // → 42
```

**What is affected:**
- `>>` output of `Int`, `Float`, and `Bool` values — all digits are rewritten to
  the active script.
- Boolean output: `#` prefix stays ASCII; the `0`/`1` digit adapts to the active
  script (`#१` = true in Devanagari, `#٠` = false in Arabic-Indic).
  This keeps `#0` (bool false) visually distinct from `0` (integer zero) in every
  script.

**What is NOT affected:** string content, char literals, array brackets `[]`,
tuple parentheses `()`, float decimal point (always ASCII `.`).

**Native digit literals in source code:**

Any of the 69 supported scripts can be used directly as integer literals — in
assignments, loop ranges, comparisons, and modulo operands. The lexer normalises
all scripts to the same internal integer value:

```zymbol
#०९#

n = ४२        // same as n = 42
@ i:१..१५ {  // range 1..15 in Devanagari
    ? i % १५ == ० { >> "FizzBuzz" ¶ }
    _? i % ३  == ० { >> "Fizz" ¶ }
    _? i % ५  == ० { >> "Buzz" ¶ }
    _ { >> i ¶ }
}
```

**Boolean literals in any script:**

`#` followed by the native `0` or `1` digit of any supported script lexes as a
boolean identical to ASCII `#0`/`#1`. The # prefix is always ASCII:

| Script | False | True | Mode token |
| ------ | ----- | ---- | ---------- |
| ASCII (default) | `#0` | `#1` | `#09#` |
| Devanagari | `#०` | `#१` | `#०९#` |
| Arabic-Indic | `#٠` | `#١` | `#٠٩#` |
| Ext. Arabic-Indic | `#۰` | `#۱` | `#۰۹#` |
| Bengali | `#০` | `#১` | `#০৯#` |
| Gurmukhi | `#੦` | `#੧` | `#੦੯#` |
| Gujarati | `#૦` | `#૧` | `#૦૯#` |
| Tamil | `#௦` | `#௧` | `#௦௯#` |
| Telugu | `#౦` | `#౧` | `#౦౯#` |
| Kannada | `#೦` | `#೧` | `#೦೯#` |
| Thai | `#๐` | `#๑` | `#๐๙#` |
| Myanmar | `#၀` | `#၁` | `#၀၉#` |
| Mathematical Bold | `#𝟎` | `#𝟏` | `#𝟎𝟗#` |
| Klingon pIqaD ¹ | `#` | `#` | `##` |

> ¹ Klingon pIqaD digits live in the ConScript Unicode Registry (CSUR) Private
> Use Area (U+F8F0–U+F8F9). They render only with a pIqaD-capable font such as
> _pIqaD-qolqoS_. Internally treated as a valid digit block; no special-casing
> in the interpreter.

**Selected supported scripts (25 of 69 shown):**

| Script | Range | Sample digits |
| ------ | ----- | ------------- |
| ASCII | U+0030–U+0039 | `0123456789` |
| Arabic-Indic | U+0660–U+0669 | `٠١٢٣٤٥٦٧٨٩` |
| Ext. Arabic-Indic | U+06F0–U+06F9 | `۰۱۲۳۴۵۶۷۸۹` |
| Devanagari | U+0966–U+096F | `०१२३४५६७८९` |
| Bengali | U+09E6–U+09EF | `০১২৩৪৫৬৭৮৯` |
| Gujarati | U+0AE6–U+0AEF | `૦૧૨૩૪૫૬૭૮૯` |
| Tamil | U+0BE6–U+0BEF | `௦௧௨௩௪௫௬௭௮௯` |
| Telugu | U+0C66–U+0C6F | `౦౧౨౩౪౫౬౭౮౯` |
| Thai | U+0E50–U+0E59 | `๐๑๒๓๔๕๖๗๘๙` |
| Tibetan | U+0F20–U+0F29 | `༠༡༢༣༤༥༦༧༨༩` |
| Myanmar | U+1040–U+1049 | `၀၁၂၃၄၅၆၇၈၉` |
| Khmer | U+17E0–U+17E9 | `០១២៣៤៥៦៧៨៩` |
| Mongolian | U+1810–U+1819 | `᠐᠑᠒᠓᠔᠕᠖᠗᠘᠙` |
| Mathematical Bold | U+1D7CE–U+1D7D7 | `𝟎𝟏𝟐𝟑𝟒𝟓𝟔𝟕𝟖𝟗` |
| Mathematical Double-struck | U+1D7D8–U+1D7E1 | `𝟘𝟙𝟚𝟛𝟜𝟝𝟞𝟟𝟠𝟡` |
| Mathematical Monospace | U+1D7F6–U+1D7FF | `𝟶𝟷𝟸𝟹𝟺𝟻𝟼𝟽𝟾𝟿` |
| Segmented/LCD | U+1FBF0–U+1FBF9 | `🯰🯱🯲🯳🯴🯵🯶🯷🯸🯹` |
| Klingon pIqaD ¹ | U+F8F0–U+F8F9 | `` _(CSUR PUA — requires pIqaD font)_ |
| _(+51 additional BMP/SMP scripts)_ | | _(see `crates/zymbol-lexer/src/digit_blocks.rs`)_ |

New crate `digit_blocks` (inside `zymbol-lexer`) maps the base codepoint for each
of the 69 registered blocks and provides `digit_value(char)` and
`digit_block_base(char)` used by both the lexer (literal normalisation) and the
interpreter (output formatting).

**Command execution operators**
- `</ path.zy />` — execute a `.zy` sub-script and capture its output.
- `<\ cmd \>` — execute a shell (bash) command and capture stdout + stderr.

**Tests**
- 71 i18n/numerals test cases covering every supported numeral system, including
  all boolean-literal and comparison-result forms across scripts.

**Tooling**
- LSP refactor: library logic extracted into `lib.rs`, `main.rs` simplified.
- MANUAL.md §18b and EBNF grammar updated to document all numeral-mode constructs.

### Changed

- Workspace version bumped to `0.0.3`.

---

## [0.0.2] — 2026-03-24

### Added

**Collection API** (arrays, tuples, strings — unified operators)
- `$+[i]` — insert at position.
- `$-` — remove first occurrence by value.
- `$--` — remove all occurrences by value.
- `$-[i]` / `$-[i..j]` / `$-[i:n]` — remove at index or range.
- `$??` — find all indices of a value.
- `$[s:n]` — count-based slice alias.
- `$^+` / `$^-` — sort ascending/descending, natural or custom comparator.

**Destructuring assignment**
- Array destructuring: `[a, b, *rest] = arr`.
- Named-tuple destructuring: `(name: n) = t`.
- Negative indices `arr[-1]` normalized in both tree-walker and VM.

**Tests**
- 20 new E2E test cases (`tests/collections/13–32`).
- 159/159 VM parity tests passing.

**Documentation**
- EBNF v2.1.0: `destructure_assign` grammar, fixed equality (`== | <>`),
  removed unimplemented `^=`, interpolation and negative-index notes.
- MANUAL.md: §11b Destructuring, negative indices, `!=` → `<>`, sort and
  destructuring in symbol reference and coverage table.
- ROADMAP.md: v0.0.2 header, 159/159 test count, version history entry.

### Changed

- Number formatting operators renamed: `c|..|` → `#,|..|`, `e|..|` → `#^|..|`.
- Export alias syntax formalized.

---

## [0.0.1] — 2026-03-22

Initial release — Zymbol-Lang interpreter v5I.

### Added

**Core language**
- Variables (`=`) and constants (`:=`), all primitive types: `Int`, `Float`,
  `String`, `Char`, `Bool`, `Array`, `Tuple`.
- Arithmetic, comparison, logical operators; compound assignment
  (`+=`, `-=`, `*=`, `/=`, `%=`, `^=`, `++`, `--`).
- String interpolation; output `>>` (multi-item juxtaposition); input `<<`;
  CLI args capture `><`.
- Control flow: `?` / `_?` / `_` (if / else-if / else).
- Match `??` with literal, range, guard `_?`, and wildcard arms.
- All loop forms: infinite, while, for-each, range with step, reverse range.
- Labeled loops with `@!` (break) and `@>` (continue).
- Functions with isolated scope; output parameters `<~` (pass by reference).
- Lambdas with implicit and explicit return; closures (outer scope capture).
- Higher-order functions: `$>` map, `$|` filter, `$<` reduce.
- Pipe operator `|>` with placeholder `_`.
- Error handling: `!?` / `:!` / `:>` try/catch/finally with typed catch.
- Module system: `#` / `#>` / `<#` with aliases and re-exports.
- Data operators: `#|x|`, `x#?`, `#.N|x|`, `#!N|x|`.
- Base literals and conversions: `0x`, `0b`, `0o`, `0d`.
- Explicit variable lifetime: `\ var`.

**Execution**
- Tree-walker interpreter (default): scope pool recycling, zero allocation per
  scope push/pop, tail-call optimization (TCO).
- Register VM (`--vm`): flat register stack, 4.4× faster than tree-walker on
  `fib(35)`, 16-byte `Value` via `Rc<T>` heap payloads.

**Tooling** (17-crate Rust workspace)
- `zymbol-cli` — `run`, `build`, `check`, `fmt`, `repl` subcommands.
- `zymbol-lsp` — Language Server Protocol via tower-lsp + tokio.
- `zymbol-formatter` — configurable indentation.
- `zymbol-repl` — interactive REPL with history.
- `zymbol-standalone` — embeds `.zy` files into Rust project templates.
- `zymbol-analyzer` — LSP analysis engine, document cache, symbol index.

**Tests**
- 88/88 E2E tests passing.
- 99/99 VM parity tests passing.
- RosettaStone i18n suite: 105 languages.
- 19 verified examples in `examples/`.

---

[0.0.8]: https://github.com/zymbol-lang/interpreter/compare/v0.0.7...v0.0.8
[0.0.7]: https://github.com/zymbol-lang/interpreter/compare/v0.0.6...v0.0.7
[0.0.6]: https://github.com/zymbol-lang/interpreter/compare/v0.0.5...v0.0.6
[0.0.5]: https://github.com/zymbol-lang/interpreter/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.com/zymbol-lang/interpreter/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/zymbol-lang/interpreter/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/zymbol-lang/interpreter/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/zymbol-lang/interpreter/releases/tag/v0.0.1
