//! A time limit on a `std/net` request (ZethyCLI G1, decided by the author on
//! 2026-10-24): `net::get(url, headers, seconds)`, the same after the headers of
//! `post` and `post_json`, and `net::head(url, seconds)` — and, for every
//! request that gives none, the process's own, `net::timeout(seconds)`, which
//! returns the one it replaces. `0` is no limit, in a call and in `timeout`.
//!
//! What these assert is the consequence: against a server that accepts and never
//! answers, the program comes back — within the limit, not when something outside
//! kills it — with a soft `##Network` it can test with `$!`, and goes on to its
//! next line. Before, both engines waited past two minutes. A server that answers
//! inside the limit still gives its body. ZyDDT cannot ask this: a cell has no
//! server to talk to.

use assert_cmd::Command;
use predicates::str::contains;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

fn zymbol() -> Command {
    Command::cargo_bin("zymbol").expect("zymbol binary not found — run `cargo build` first")
}

/// A server that accepts every connection and never answers. Returns its port.
fn mute_server() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for c in l.incoming().flatten() {
            held.push(c);
        }
    });
    port
}

/// A server that answers `hola` at once.
fn quick_server() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for mut c in l.incoming().flatten() {
            let mut buf = [0u8; 4096];
            let _ = c.read(&mut buf);
            let _ = c.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nhola");
        }
    });
    port
}

fn run(src: &str, tw: bool) -> (assert_cmd::assert::Assert, Duration) {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("main.zy"), src).unwrap();
    let mut cmd = zymbol();
    cmd.current_dir(d.path()).arg("run");
    if tw {
        cmd.arg("--tw");
    }
    // A safety net for the test itself, far above every limit below.
    cmd.timeout(Duration::from_secs(30));
    let started = Instant::now();
    let a = cmd.arg("main.zy").assert();
    (a, started.elapsed())
}

#[test]
fn a_mute_server_gives_a_soft_network_error_within_the_limit() {
    let port = mute_server();
    let src = format!(
        "<# std/net => net\n\
         r = net::get(\"http://127.0.0.1:{port}/\", [], 1)\n\
         >> r$! \" \" r ¶\n\
         p = net::post_json(\"http://127.0.0.1:{port}/\", \"[]\", [(\"x-a\", \"b\")], 0.5)\n\
         >> p$! \" \" p ¶\n\
         >> net::head(\"http://127.0.0.1:{port}/\", 1) ¶\n\
         >> \"sigue\" ¶\n"
    );
    for tw in [false, true] {
        let (a, took) = run(&src, tw);
        a.success()
            .stdout(contains("#1 ##Network(timeout: no answer within 1 s)"))
            .stdout(contains("#1 ##Network(timeout: no answer within 0.5 s)"))
            .stdout(contains("#0\nsigue"));
        // 1 + 0.5 + 1 seconds of limits, and a margin for a loaded machine.
        assert!(took < Duration::from_secs(12), "tw={tw}: took {took:?}, the limits were not applied");
    }
}

#[test]
fn the_process_limit_reaches_every_request_and_a_call_overrides_it() {
    let mute = mute_server();
    let quick = quick_server();
    // A module sets the limit; the importer's request takes it. `0` in a call
    // is no limit even then, and `timeout` hands back what it replaced.
    let src = format!(
        "<# std/net => net\n\
         <# ./lib/red => red\n\
         >> red::preparar() ¶\n\
         r = net::get(\"http://127.0.0.1:{mute}/\")\n\
         >> r ¶\n\
         >> net::get(\"http://127.0.0.1:{quick}/\", [], 0) ¶\n\
         >> net::head(\"http://127.0.0.1:{mute}/\") ¶\n\
         >> net::timeout(0) \" \" net::timeout(0) ¶\n"
    );
    for tw in [false, true] {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("main.zy"), &src).unwrap();
        std::fs::create_dir_all(d.path().join("lib")).unwrap();
        std::fs::write(
            d.path().join("lib/red.zy"),
            "# red {\n    <# std/net => net\n    #> { preparar }\n    preparar() { <~ net::timeout(1) }\n}\n",
        )
        .unwrap();
        let mut cmd = zymbol();
        cmd.current_dir(d.path()).arg("run");
        if tw {
            cmd.arg("--tw");
        }
        let started = Instant::now();
        cmd.timeout(Duration::from_secs(30))
            .arg("main.zy")
            .assert()
            .success()
            .stdout(contains("0\n##Network(timeout: no answer within 1 s)\nhola\n#0\n1 0"));
        assert!(started.elapsed() < Duration::from_secs(10), "tw={tw}: the process limit was not applied");
    }
}

#[test]
fn a_server_inside_the_limit_still_answers() {
    let port = quick_server();
    let src = format!(
        "<# std/net => net\n\
         >> net::get(\"http://127.0.0.1:{port}/\", [], 5) ¶\n\
         >> net::head(\"http://127.0.0.1:{port}/\", 5) ¶\n\
         >> net::get(\"http://127.0.0.1:{port}/\") ¶\n"
    );
    for tw in [false, true] {
        run(&src, tw).0.success().stdout(contains("hola\n#1\nhola"));
    }
}
