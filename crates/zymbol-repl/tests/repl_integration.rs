mod common;
use common::ReplTestHarness;

// ── pIqaD digits U+F8F0–U+F8F9 ────────────────────────────────────────────

#[test]
fn test_piqad_digits_output() {
    let digits: String = (0xF8F0u32..=0xF8F9).map(|c| char::from_u32(c).unwrap()).collect();
    let code = format!(">> \"{}\"¶", digits);

    let mut h = ReplTestHarness::new();
    let result = h.run_line(&code);

    assert!(result.error.is_none(), "unexpected error: {:?}", result.error);
    assert_eq!(result.output.trim_end_matches('\n'), digits,
        "pIqaD digits output mismatch");
}

#[test]
fn test_piqad_digits_codepoints_intact() {
    let digits: String = (0xF8F0u32..=0xF8F9).map(|c| char::from_u32(c).unwrap()).collect();
    let code = format!(">> \"{}\"¶", digits);

    let mut h = ReplTestHarness::new();
    let out = h.output(&code);

    let got: Vec<u32> = out.trim_end_matches('\n').chars().map(|c| c as u32).collect();
    let expected: Vec<u32> = (0xF8F0..=0xF8F9).collect();
    assert_eq!(got, expected, "codepoint mismatch in pIqaD digit output");
}

// ── pIqaD letters U+F8D0–U+F8E9 ───────────────────────────────────────────

#[test]
fn test_piqad_letters_output() {
    // tlhIngan: tlh(F8E4) I(F8D7) ng(F8DC) a(F8D0) n(F8DB)
    let word: String = [0xF8E4u32, 0xF8D7, 0xF8DC, 0xF8D0, 0xF8DB]
        .iter()
        .map(|&c| char::from_u32(c).unwrap())
        .collect();
    let code = format!(">> \"{}\"¶", word);

    let mut h = ReplTestHarness::new();
    let out = h.output(&code);

    assert_eq!(out.trim_end_matches('\n'), word,
        "pIqaD letters (tlhIngan) output mismatch");
}

#[test]
fn test_piqad_full_alphabet_roundtrip() {
    // All 26 pIqaD letter codepoints U+F8D0–U+F8E9
    let letters: String = (0xF8D0u32..=0xF8E9).map(|c| char::from_u32(c).unwrap()).collect();
    let code = format!(">> \"{}\"¶", letters);

    let mut h = ReplTestHarness::new();
    let out = h.output(&code);

    let got: Vec<u32> = out.trim_end_matches('\n').chars().map(|c| c as u32).collect();
    let expected: Vec<u32> = (0xF8D0..=0xF8E9).collect();
    assert_eq!(got, expected, "pIqaD full alphabet roundtrip failed");
}

// ── Variable listing (VARS equivalent) ────────────────────────────────────

#[test]
fn test_vars_shows_defined_variables() {
    let mut h = ReplTestHarness::new();
    h.output("x = 42");
    h.output("name = \"zymbol\"");

    let vars = h.variables();
    let names: Vec<&str> = vars.iter().map(|(n, _)| n.as_str()).collect();

    assert!(names.contains(&"x"), "x should be in variables");
    assert!(names.contains(&"name"), "name should be in variables");
}

#[test]
fn test_vars_empty_initially() {
    let h = ReplTestHarness::new();
    assert!(h.variables().is_empty(), "fresh harness should have no variables");
}

// ── History tracking ───────────────────────────────────────────────────────

#[test]
fn test_history_grows_in_order() {
    let mut h = ReplTestHarness::new();
    h.output("x = 1");
    h.output("y = 2");
    h.output(">> x + y¶");

    let hist = h.history();
    assert_eq!(hist.len(), 3);
    assert_eq!(hist[0], "x = 1");
    assert_eq!(hist[1], "y = 2");
    assert_eq!(hist[2], ">> x + y¶");
}

#[test]
fn test_history_skips_empty_lines() {
    let mut h = ReplTestHarness::new();
    h.output("");
    h.output("   ");
    h.output("x = 99");

    assert_eq!(h.history().len(), 1);
}

// ── Regular input (<<) ────────────────────────────────────────────────────

#[test]
fn test_input_mock_single_line() {
    let mut h = ReplTestHarness::new();
    h.queue_input(&["hello from mock\n"]);
    h.output("<< \"enter: \" x");

    let vars = h.variables();
    let x_val = vars.iter().find(|(n, _)| n == "x").map(|(_, v)| v.as_str());
    assert_eq!(x_val, Some("hello from mock"), "mocked input should be stored in x");
}

#[test]
fn test_input_mock_multiple_lines() {
    let mut h = ReplTestHarness::new();
    h.queue_input(&["first\n", "second\n"]);
    h.output("<< \"1: \" a");
    h.output("<< \"2: \" b");

    let vars = h.variables();
    let get = |n: &str| vars.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone());
    assert_eq!(get("a").as_deref(), Some("first"));
    assert_eq!(get("b").as_deref(), Some("second"));
}

#[test]
fn test_input_no_prompt() {
    // << x without a prompt — reads silently, stores in x
    let mut h = ReplTestHarness::new();
    h.queue_input(&["silent\n"]);
    let result = h.run_line("<< x");
    assert!(result.error.is_none(), "no-prompt input should not error");

    let vars = h.variables();
    let x_val = vars.iter().find(|(n, _)| n == "x").map(|(_, v)| v.as_str());
    assert_eq!(x_val, Some("silent"), "<< x should store the read line");
}

#[test]
fn test_input_trims_whitespace() {
    // The interpreter trims leading/trailing whitespace before storing
    let mut h = ReplTestHarness::new();
    h.queue_input(&["  padded  \n"]);
    h.output("<< x");

    let vars = h.variables();
    let x_val = vars.iter().find(|(n, _)| n == "x").map(|(_, v)| v.as_str());
    assert_eq!(x_val, Some("padded"), "<< should trim surrounding whitespace");
}

#[test]
fn test_input_empty_gives_empty_string() {
    let mut h = ReplTestHarness::new();
    h.queue_input(&["\n"]);
    h.output("<< x");

    let vars = h.variables();
    let x_val = vars.iter().find(|(n, _)| n == "x").map(|(_, v)| v.as_str());
    assert_eq!(x_val, Some(""), "empty input should yield empty string");
}

#[test]
fn test_input_numeric_cast_integer() {
    // << #|x| reads a line and converts to Int
    let mut h = ReplTestHarness::new();
    h.queue_input(&["42\n"]);
    let result = h.run_line("<< #|x|");
    assert!(result.error.is_none(), "numeric input should not error");

    let vars = h.variables();
    let x_val = vars.iter().find(|(n, _)| n == "x").map(|(_, v)| v.as_str());
    assert_eq!(x_val, Some("42"), "integer input should be stored as 42");
}

#[test]
fn test_input_numeric_cast_float() {
    let mut h = ReplTestHarness::new();
    h.queue_input(&["3.14\n"]);
    h.output("<< #|x|");

    let vars = h.variables();
    let x_val = vars.iter().find(|(n, _)| n == "x").map(|(_, v)| v.as_str());
    assert_eq!(x_val, Some("3.14"), "float input should be stored as 3.14");
}

#[test]
fn test_input_numeric_with_prompt() {
    let mut h = ReplTestHarness::new();
    h.queue_input(&["7\n"]);
    let result = h.run_line("<< \"num: \" #|n|");
    assert!(result.error.is_none());

    let vars = h.variables();
    let n_val = vars.iter().find(|(k, _)| k == "n").map(|(_, v)| v.as_str());
    assert_eq!(n_val, Some("7"));
}

#[test]
fn test_input_value_usable_in_expression() {
    // Variable read from << participates in arithmetic
    let mut h = ReplTestHarness::new();
    h.queue_input(&["10\n"]);
    h.output("<< #|x|");
    let out = h.output(">> x + 5¶");
    assert_eq!(out.trim_end_matches('\n'), "15");
}

#[test]
fn test_input_sequential_three() {
    let mut h = ReplTestHarness::new();
    h.queue_input(&["alpha\n", "beta\n", "gamma\n"]);
    h.output("<< a");
    h.output("<< b");
    h.output("<< c");

    let vars = h.variables();
    let get = |n: &str| vars.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone());
    assert_eq!(get("a").as_deref(), Some("alpha"));
    assert_eq!(get("b").as_deref(), Some("beta"));
    assert_eq!(get("c").as_deref(), Some("gamma"));
}

// ── Unicode width (cursor positioning) ────────────────────────────────────

#[test]
fn test_cursor_width_piqad_is_one_col() {
    // Verify that pIqaD codepoints do not produce double-width output.
    // Each pIqaD character should occupy exactly 1 terminal column,
    // so 10 digits should produce 10 bytes of meaningful content (3 UTF-8 bytes each).
    let digits: String = (0xF8F0u32..=0xF8F9).map(|c| char::from_u32(c).unwrap()).collect();
    assert_eq!(digits.chars().count(), 10);

    // unicode-width should treat PUA as width 1 per character
    use unicode_width::UnicodeWidthStr;
    assert_eq!(digits.width(), 10, "each pIqaD digit must be 1 display column");
}

#[test]
fn test_cursor_width_wide_chars() {
    use unicode_width::UnicodeWidthStr;
    // CJK characters are 2 columns wide
    assert_eq!("日本語".width(), 6);
    // ASCII is 1 column wide
    assert_eq!("hello".width(), 5);
    // Emoji are 2 columns wide
    assert_eq!("😀".width(), 2);
}

// ── Error handling ─────────────────────────────────────────────────────────

#[test]
fn test_error_on_invalid_syntax() {
    let mut h = ReplTestHarness::new();
    let err = h.error(">>> invalid syntax %%%");
    assert!(err.is_some(), "invalid syntax should produce an error");
}

#[test]
fn test_error_on_undefined_variable() {
    let mut h = ReplTestHarness::new();
    let err = h.error(">> undefined_var¶");
    assert!(err.is_some(), "undefined variable should produce an error");
}

// ── RESET command ─────────────────────────────────────────────────────────

#[test]
fn test_reset_clears_variables() {
    let mut h = ReplTestHarness::new();
    h.output("x = 42");
    h.output("y = \"zymbol\"");
    assert!(!h.variables().is_empty());

    h.interpreter.reset_scope();
    assert!(h.variables().is_empty(), "reset_scope should clear all variables");
}

#[test]
fn test_reset_allows_redefine() {
    let mut h = ReplTestHarness::new();
    h.output("x = 1");
    h.interpreter.reset_scope();
    // After reset, x = 2 should work without conflict
    assert!(h.error("x = 2").is_none(), "redefining x after reset should not error");
    let vars = h.variables();
    let x_val = vars.iter().find(|(n, _)| n == "x").map(|(_, v)| v.as_str());
    assert_eq!(x_val, Some("2"));
}

// ── Non-blocking key input (<<|?) ────────────────────────────────────────

#[test]
fn test_key_input_nonblocking_returns_null_in_headless() {
    // <<|? is non-blocking: no key available → stores '\0', must not error
    let mut h = ReplTestHarness::new();
    let result = h.run_line("<<|? k");
    assert!(result.error.is_none(), "<<|? should not error in headless: {:?}", result.error);

    let vars = h.variables();
    let k_val = vars.iter().find(|(n, _)| n == "k").map(|(_, v)| v.clone());
    // '\0' is formatted by the interpreter; just verify variable exists without error
    assert!(k_val.is_some(), "<<|? should define the variable k");
}

// ── TUI key input <<| (requires a terminal) ──────────────────────────────

/// Can crossterm reach a terminal from this process?
///
/// The question the TUI tests below need answered is "will `<<|` block waiting for
/// a key, or fail because there is nothing to read from" — and that is decided by
/// whether crossterm can open the terminal, not by whether *stdout* is a tty.
///
/// Those are the same question on Unix and different ones on Windows, where
/// crossterm talks to `CONIN$`/`CONOUT$` rather than to the standard handles. Under
/// `cargo test` stdout is captured, so `is_tty()` said "headless" while the process
/// still had a console attached — and `<<|` sat waiting for a keypress that was
/// never coming, hanging the whole suite until the process was killed by hand.
///
/// `terminal::size()` asks the same layer `<<|` reads from, so it answers for both
/// platforms.
fn terminal_is_reachable() -> bool {
    crossterm::terminal::size().is_ok()
}

/// Can this process actually enter raw mode?
///
/// A narrower question than [`terminal_is_reachable`], and the two do come apart.
/// On a Linux box with no controlling terminal — `cargo test` under a CI runner, or
/// any non-interactive session — `terminal::size()` still answers, while
/// `enable_raw_mode()` fails with `ENXIO` because there is no `/dev/tty` to open.
/// A test that asked the first question and then asserted the second failed on
/// Linux while being right about Windows.
///
/// Asking by doing is the only answer that cannot drift: raw mode is enabled and
/// immediately undone, which is exactly what the statement under test would do.
fn raw_mode_is_available() -> bool {
    if crossterm::terminal::enable_raw_mode().is_ok() {
        let _ = crossterm::terminal::disable_raw_mode();
        return true;
    }
    false
}

#[test]
fn test_key_input_headless_graceful() {
    // <<| reads one keypress. In headless mode crossterm cannot access the
    // terminal device and must produce a runtime error.
    // On a real terminal the statement blocks for input — we skip that path here.
    if terminal_is_reachable() {
        return; // would block waiting for a keypress; skip
    }
    let mut h = ReplTestHarness::new();
    let result = h.run_line("<<| key");
    assert!(result.error.is_some(), "<<| should error in headless mode");
    let err = result.error.unwrap();
    assert!(
        err.contains("input") || err.contains("device") || err.contains("initialize") || err.contains("os error"),
        "unexpected <<| error message: {}", err
    );
}

// ── TUI block (requires TTY — skipped in headless CI) ─────────────────────

#[test]
fn test_tui_block_headless_graceful() {
    // In a headless environment (CI, pipes) the TUI block fails because crossterm
    // cannot enable raw mode without a real TTY. The test detects this at runtime
    // and passes by verifying the error message is the expected OS-level rejection.
    // On a real terminal the block should execute without error.
    let mut h = ReplTestHarness::new();
    let result = h.run_line(">>| { >> \"in tui\"¶ }");

    if raw_mode_is_available() {
        // Real terminal: block should execute cleanly.
        assert!(result.error.is_none(), "TUI block should work on a real TTY: {:?}", result.error);
    } else {
        // Headless: error is expected and must mention raw mode or device.
        if let Some(ref err) = result.error {
            assert!(
                err.contains("raw mode") || err.contains("device") || err.contains("os error"),
                "unexpected TUI error in headless mode: {}", err
            );
        }
        // No assertion if error is None — some platforms handle this gracefully.
    }
}
