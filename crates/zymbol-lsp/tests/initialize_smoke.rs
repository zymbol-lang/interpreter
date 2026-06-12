//! Smoke test for the LSP stdio wiring: spawns the real `zymbol-lsp` binary
//! and performs a full `initialize → initialized → shutdown → exit` handshake
//! over stdin/stdout with LSP `Content-Length` framing.
//!
//! This covers the tower-lsp layer (transport, dispatch, capability
//! negotiation) that unit tests in `zymbol-analyzer` cannot reach.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;

struct LspClient {
    child: Child,
    stdin: Option<ChildStdin>,
    reader: BufReader<ChildStdout>,
}

impl LspClient {
    fn spawn() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_zymbol-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn zymbol-lsp binary");
        let stdin = child.stdin.take();
        let reader = BufReader::new(child.stdout.take().unwrap());
        Self { child, stdin, reader }
    }

    fn send(&mut self, body: &str) {
        let stdin = self.stdin.as_mut().expect("stdin already closed");
        write!(stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
        stdin.flush().unwrap();
    }

    /// Read one framed message and return its body.
    fn recv(&mut self) -> String {
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            self.reader.read_line(&mut line).expect("read header line");
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some(v) = line.strip_prefix("Content-Length:") {
                content_length = Some(v.trim().parse().expect("Content-Length value"));
            }
        }
        let len = content_length.expect("missing Content-Length header");
        let mut buf = vec![0u8; len];
        self.reader.read_exact(&mut buf).expect("read body");
        String::from_utf8(buf).expect("body is UTF-8")
    }

    /// Read framed messages until the response with the given request id
    /// arrives. Server-initiated notifications (window/logMessage) are
    /// skipped; server-initiated requests (client/registerCapability) get a
    /// null-result reply — like a real client, and required for the server's
    /// `initialized` handler (which awaits the reply) to make progress.
    fn recv_response_for(&mut self, id: u64) -> serde_json::Value {
        for _ in 0..16 {
            let body = self.recv();
            let msg: serde_json::Value = serde_json::from_str(&body).expect("valid JSON");
            match (msg.get("method"), msg.get("id")) {
                (Some(_), Some(req_id)) => {
                    let reply = format!(r#"{{"jsonrpc":"2.0","id":{req_id},"result":null}}"#);
                    self.send(&reply);
                }
                (None, Some(resp_id)) if *resp_id == serde_json::json!(id) => return msg,
                _ => {} // notification or unrelated response — skip
            }
        }
        panic!("no response with id {id} after 16 messages");
    }

    /// Read until a server→client request with the given method arrives,
    /// reply with a null result, and return the request. Notifications are
    /// skipped along the way.
    fn recv_request(&mut self, method: &str) -> serde_json::Value {
        for _ in 0..16 {
            let body = self.recv();
            let msg: serde_json::Value = serde_json::from_str(&body).expect("valid JSON");
            if let (Some(m), Some(req_id)) = (msg.get("method"), msg.get("id")) {
                let reply = format!(r#"{{"jsonrpc":"2.0","id":{req_id},"result":null}}"#);
                self.send(&reply);
                if m == method {
                    return msg;
                }
            }
        }
        panic!("server never sent request {method}");
    }
}

#[test]
fn initialize_handshake_over_stdio() {
    let mut client = LspClient::spawn();

    client.send(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{},"processId":null,"rootUri":null}}"#,
    );
    let resp = client.recv_response_for(1);

    let result = &resp["result"];
    assert!(
        result.get("capabilities").is_some(),
        "initialize result must advertise capabilities, got: {resp}"
    );
    assert_eq!(
        result["serverInfo"]["name"], "zymbol-lsp",
        "serverInfo.name mismatch, got: {resp}"
    );
    // Core capabilities the VS Code extension relies on.
    let caps = &result["capabilities"];
    assert!(
        !caps["textDocumentSync"].is_null(),
        "textDocumentSync must be advertised, got: {caps}"
    );

    client.send(r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#);
    // The initialized handler sends client/registerCapability and awaits the
    // reply. Answer it BEFORE shutdown: tower-lsp handles requests
    // concurrently, so shutting down first would leave that handler pending
    // forever and the process would never exit.
    let watcher_reg = client.recv_request("client/registerCapability");
    assert!(
        watcher_reg.to_string().contains("didChangeWatchedFiles"),
        "expected file-watcher registration, got: {watcher_reg}"
    );
    client.send(r#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#);
    let shutdown = client.recv_response_for(2);
    assert!(
        shutdown.get("error").is_none(),
        "shutdown must succeed, got: {shutdown}"
    );

    client.send(r#"{"jsonrpc":"2.0","method":"exit"}"#);
    // Close stdin: tokio's stdin reader keeps the process alive until EOF,
    // so the pipe must be dropped for the exit notification to take effect.
    client.stdin.take();

    // The server must terminate on its own after `exit`.
    let status = wait_with_timeout(&mut client.child, Duration::from_secs(10));
    assert!(status.success(), "server exited with {status:?}");
}

fn wait_with_timeout(child: &mut Child, timeout: Duration) -> std::process::ExitStatus {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            return status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            panic!("zymbol-lsp did not exit within {timeout:?} after `exit` notification");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
