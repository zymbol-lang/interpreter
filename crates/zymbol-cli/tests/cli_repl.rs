use assert_cmd::Command;
use predicates::str::contains;

fn zymbol() -> Command {
    Command::cargo_bin("zymbol").expect("zymbol binary not found — run `cargo build` first")
}

// ── Basic output via piped stdin ───────────────────────────────────────────

#[test]
fn cli_repl_hello_world() {
    zymbol()
        .arg("repl")
        .write_stdin(">> \"hello\"¶\n")
        .assert()
        .success()
        .stdout(contains("hello"));
}

#[test]
fn cli_repl_arithmetic() {
    zymbol()
        .arg("repl")
        .write_stdin(">> 2 + 3¶\n")
        .assert()
        .success()
        .stdout(contains("5"));
}

// ── pIqaD digits through CLI ──────────────────────────────────────────────

#[test]
fn cli_repl_piqad_digits_roundtrip() {
    let digits: String = (0xF8F0u32..=0xF8F9).map(|c| char::from_u32(c).unwrap()).collect();
    let cmd = format!(">> \"{}\"¶\n", digits);

    zymbol()
        .arg("repl")
        .write_stdin(cmd)
        .assert()
        .success()
        .stdout(contains(digits));
}

// ── Variable assignment survives within session ────────────────────────────

#[test]
fn cli_repl_variable_persists_in_session() {
    zymbol()
        .arg("repl")
        .write_stdin("x = 99\n>> x¶\n")
        .assert()
        .success()
        .stdout(contains("99"));
}

// ── Positioned output >>~ ─────────────────────────────────────────────────

#[test]
fn cli_repl_output_pos_emits_ansi_escape() {
    // >>~ (row, col) > "text" must emit ESC[row;colH before the text
    zymbol()
        .arg("repl")
        .write_stdin(">>~ (1, 1) > \"A\"\n")
        .assert()
        .success()
        .stdout(contains("\x1b[1;1H"));
}

#[test]
fn cli_repl_output_pos_text_follows_escape() {
    // The actual character must appear right after the cursor-move escape
    let out = zymbol()
        .arg("repl")
        .write_stdin(">>~ (2, 3) > \"Z\"\n")
        .output()
        .expect("zymbol failed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("\x1b[2;3H"), "missing cursor-move escape");
    assert!(stdout.contains('Z'), "missing character after escape");
}

#[test]
fn cli_repl_output_pos_sparse_syntax() {
    // >>~ (r, c) > "x" with sparse tuple (only position, no color/style)
    zymbol()
        .arg("repl")
        .write_stdin(">>~ (5, 10) > \"*\"\n")
        .assert()
        .success()
        .stdout(contains("\x1b[5;10H"));
}

// ── Clear screen >>! ──────────────────────────────────────────────────────

#[test]
fn cli_repl_clear_screen_emits_ansi() {
    // >>! must emit the ANSI erase-display sequence ESC[2J
    zymbol()
        .arg("repl")
        .write_stdin(">>!\n>> \"after\"¶\n")
        .assert()
        .success()
        .stdout(contains("\x1b[2J"))
        .stdout(contains("after"));
}

#[test]
fn cli_repl_clear_screen_moves_cursor_home() {
    // After clearing, cursor moves to (1,1): ESC[1;1H
    zymbol()
        .arg("repl")
        .write_stdin(">>!\n")
        .assert()
        .success()
        .stdout(contains("\x1b[1;1H"));
}

// ── Terminal size >>? ─────────────────────────────────────────────────────

#[test]
fn cli_repl_terminal_size_positive() {
    // `>>?` must give two usable dimensions — that is all a program can rely on.
    //
    // This used to assert the exact 24×80 that crossterm falls back to with no
    // terminal, which is only what happens on Unix: there, a piped stdout means no
    // tty and the query fails. Windows answers it anyway, because crossterm asks
    // the console through `CONOUT$` rather than through the redirected handle — so
    // the test read a real 30×120 and failed on a correct answer.
    let output = zymbol()
        .arg("repl")
        .write_stdin("[H, W] = >>?\n>> H ¶\n>> W ¶\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("stdout is UTF-8");

    let dims: Vec<u32> = stdout
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect();

    assert_eq!(dims.len(), 2, "expected two dimensions, got: {stdout:?}");
    assert!(
        dims.iter().all(|&d| d > 0),
        "dimensions must be positive, got: {dims:?}"
    );
}

#[test]
fn cli_repl_terminal_size_usable_in_condition() {
    // >>? result can be destructured and compared; #1 = true in Zymbol
    zymbol()
        .arg("repl")
        .write_stdin("[H, W] = >>?\nok = H > 0\n>> ok¶\n")
        .assert()
        .success()
        .stdout(contains("#1"));
}
