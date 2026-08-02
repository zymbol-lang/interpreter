//! `zymbol check` must catch misuse of a `std/` module.
//!
//! These calls used to reach run time untouched: nothing on disk describes a
//! native module, so the alias was a blind spot for the whole static pipeline.

use assert_cmd::Command;
use predicates::str::contains;
use std::io::Write;

fn zymbol() -> Command {
    Command::cargo_bin("zymbol").expect("zymbol binary not found — run `cargo build` first")
}

/// Write `source` to a temp file and return the handle (kept alive by caller).
fn source_file(source: &str) -> tempfile::NamedTempFile {
    let mut file = tempfile::Builder::new()
        .suffix(".zy")
        .tempfile()
        .expect("temp file");
    file.write_all(source.as_bytes()).expect("write source");
    file.flush().expect("flush");
    file
}

#[test]
fn check_accepts_real_stdlib_calls() {
    let file = source_file("<# std/math => math\n\n>> math::sin(2.0) ¶\n>> math.PI ¶\n");
    zymbol()
        .arg("check")
        .arg(file.path())
        .assert()
        .success()
        .stdout(contains("No errors"));
}

#[test]
fn check_rejects_a_function_the_stdlib_does_not_have() {
    let file = source_file("<# std/math => math\n\n>> math::inventada(2.0) ¶\n");
    zymbol()
        .arg("check")
        .arg(file.path())
        .assert()
        .failure()
        .stderr(contains("std/math does not export a function 'inventada'"));
}

#[test]
fn check_suggests_the_intended_name() {
    let file = source_file("<# std/math => m\n\n>> m::sqr(4.0) ¶\n");
    zymbol()
        .arg("check")
        .arg(file.path())
        .assert()
        .failure()
        .stderr(contains("did you mean 'm::sqrt'?"));
}

#[test]
fn check_names_the_wrong_access_operator() {
    let file = source_file("<# std/math => m\n\n>> m.sin ¶\n");
    zymbol()
        .arg("check")
        .arg(file.path())
        .assert()
        .failure()
        .stderr(contains("is a function of std/math, not a constant"));
}
