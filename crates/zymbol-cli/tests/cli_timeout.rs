//! `zymbol run --timeout` (G3, decided by the author on 2026-10-22).
//!
//! What a program that does not terminate does to whoever runs it is the
//! question, so the tests ask the consequence: the process ends, in about the
//! time asked, with status 124 and nothing more on stdout — and a shell command
//! it started ends with it. A program that finishes in time is not touched.

use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

fn zymbol() -> Command {
    Command::cargo_bin("zymbol").expect("zymbol binary not found — run `cargo build` first")
}

fn program(dir: &Path, src: &str) -> std::path::PathBuf {
    let p = dir.join("p.zy");
    fs::write(&p, src).unwrap();
    p
}

/// Runs `src` with `--timeout 0.5` on `engine` and asserts it was stopped by it.
fn stopped(src: &str, engine: &str) {
    let d = tempfile::tempdir().unwrap();
    let p = program(d.path(), src);
    let start = Instant::now();
    zymbol()
        .current_dir(d.path())
        .args(["run", engine, "--timeout", "0.5"])
        .arg(&p)
        .timeout(Duration::from_secs(20))
        .assert()
        .code(124)
        .stdout("")
        .stderr(contains("stopped after 0.5 s (--timeout)"));
    let took = start.elapsed();
    assert!(took < Duration::from_secs(5), "{engine}: took {took:?}");
}

#[test]
fn an_infinite_loop_is_stopped() {
    for engine in ["--vm", "--tw"] {
        stopped("n = 0\n@ { n = n + 1 }\n", engine);
    }
}

#[test]
fn a_tail_recursion_is_stopped() {
    for engine in ["--vm", "--tw"] {
        stopped("f(n) { <~ f(n + 1) }\n>> f(1) ¶\n", engine);
    }
}

#[test]
fn a_wait_is_stopped() {
    stopped("@~ 60000\n>> \"fin\" ¶\n", "--vm");
}

#[test]
fn the_program_cannot_catch_it() {
    stopped("!? {\n    @ { }\n} :! {\n    >> \"capturado\" ¶\n}\n>> \"siguió\" ¶\n", "--vm");
}

#[test]
fn a_running_shell_command_ends_with_the_program() {
    for engine in ["--vm", "--tw"] {
        let d = tempfile::tempdir().unwrap();
        stopped_in(d.path(), "r = <\\ \"sleep 2; touch MARCA\" \\>\n>> r ¶\n", engine);
        std::thread::sleep(Duration::from_secs(3));
        assert!(!d.path().join("MARCA").exists(), "{engine}: the command outlived the program");
    }
}

fn stopped_in(dir: &Path, src: &str, engine: &str) {
    let p = program(dir, src);
    zymbol()
        .current_dir(dir)
        .args(["run", engine, "--timeout", "0.5"])
        .arg(&p)
        .timeout(Duration::from_secs(20))
        .assert()
        .code(124)
        .stdout("");
}

#[test]
fn a_program_that_finishes_in_time_is_not_touched() {
    let d = tempfile::tempdir().unwrap();
    let p = program(d.path(), ">> \"hola\" ¶\n");
    zymbol().args(["run", "--timeout", "10"]).arg(&p).assert().success().stdout("hola\n");
}

#[test]
fn a_timeout_that_is_not_a_positive_number_is_a_usage_error() {
    let d = tempfile::tempdir().unwrap();
    let p = program(d.path(), ">> \"hola\" ¶\n");
    zymbol()
        .args(["run", "--timeout", "0"])
        .arg(&p)
        .assert()
        .code(2)
        .stderr(contains("--timeout: 0 is not a number of seconds greater than 0"));
}
