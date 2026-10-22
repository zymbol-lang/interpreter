//! `zymbol run --deny` and `zymbol check --effects` (G1, decided 2026-10-21).
//!
//! What these assert is the consequence, not the message: a refused program must not
//! have run AT ALL. Each program writes a marker file on its first line, before it
//! reaches the denied gate, and the marker must not exist after a refusal — a control
//! that stops a program half way, after it has written a file, is what `--deny` exists
//! not to be.

use assert_cmd::Command;
use predicates::str::contains;
use std::fs;
use std::path::Path;

fn zymbol() -> Command {
    Command::cargo_bin("zymbol").expect("zymbol binary not found — run `cargo build` first")
}

fn write(dir: &Path, rel: &str, src: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, src).unwrap();
}

/// A program whose only network use is inside an imported module, and whose only
/// shell use is inside a sub-script. Its first line writes `MARCA`.
fn app() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    write(
        d.path(),
        "main.zy",
        "<# std/io => io\n<# ./lib/clima => clima\n_w = io::write(\"MARCA\", \"ran\")\n>> clima::hoy() ¶\n_r = </ ./tarea.zy />\n",
    );
    write(
        d.path(),
        "lib/clima.zy",
        "# clima {\n    <# std/net => net\n    #> { hoy }\n    hoy() { <~ \"clima\" }\n}\n",
    );
    write(d.path(), "tarea.zy", "x = <\\ \"echo\" \\>\n>> x ¶\n");
    d
}

fn run_denied(d: &Path, deny: &str, tw: bool) -> assert_cmd::assert::Assert {
    let mut cmd = zymbol();
    cmd.current_dir(d).arg("run");
    if tw {
        cmd.arg("--tw");
    }
    cmd.arg("--deny").arg(deny).arg("main.zy").assert()
}

#[test]
fn a_denied_effect_in_an_imported_module_refuses_before_anything_runs() {
    for tw in [false, true] {
        let d = app();
        run_denied(d.path(), "net", tw)
            .code(1)
            .stderr(contains("--deny net refuses this program — nothing ran"))
            .stderr(contains("lib/clima.zy:2"));
        assert!(!d.path().join("MARCA").exists(), "tw={tw}: the program ran before it was refused");
    }
}

#[test]
fn a_denied_effect_in_a_sub_script_refuses_before_anything_runs() {
    let d = app();
    run_denied(d.path(), "shell", false).code(1).stderr(contains("tarea.zy:1"));
    assert!(!d.path().join("MARCA").exists());
}

#[test]
fn an_effect_the_program_does_not_use_does_not_stop_it() {
    let d = app();
    run_denied(d.path(), "random,db,stdin,argv", false).success().stdout(contains("clima"));
    assert!(d.path().join("MARCA").exists());
}

#[test]
fn an_unknown_effect_is_a_usage_error_that_names_the_known_ones() {
    let d = app();
    run_denied(d.path(), "nett", false)
        .code(2)
        .stderr(contains("unknown effect 'nett'"))
        .stderr(contains("io, net, db, time, random, shell, sub, stdin, argv"));
    assert!(!d.path().join("MARCA").exists());
}

#[test]
fn a_file_that_cannot_be_read_is_refused_not_trusted() {
    let d = tempfile::tempdir().unwrap();
    write(d.path(), "main.zy", "<# std/io => io\n<# ./roto => r\n_w = io::write(\"MARCA\", \"ran\")\n");
    write(d.path(), "roto.zy", "# r {\n    #> { f }\n    f( {\n}\n");
    run_denied(d.path(), "time", false)
        .code(1)
        .stderr(contains("cannot verify this program — nothing ran"))
        .stderr(contains("roto.zy"));
    assert!(!d.path().join("MARCA").exists());
}

#[test]
fn check_effects_lists_the_surface_with_file_and_line() {
    let d = app();
    zymbol()
        .current_dir(d.path())
        .args(["check", "--effects", "main.zy"])
        .assert()
        .success()
        .stdout(contains("effects of main.zy:"))
        .stdout(contains("net     lib/clima.zy:2"))
        .stdout(contains("io      main.zy:1"))
        .stdout(contains("sub     main.zy:5"))
        .stdout(contains("shell   tarea.zy:1"));
    assert!(!d.path().join("MARCA").exists(), "check must not run the program");
}

#[test]
fn without_deny_nothing_changes() {
    let d = app();
    zymbol().current_dir(d.path()).args(["run", "main.zy"]).assert().success();
    assert!(d.path().join("MARCA").exists());
}
