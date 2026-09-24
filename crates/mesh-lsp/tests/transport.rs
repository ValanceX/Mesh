//! The real binary over stdio: how it ends when the client exits, or when
//! the transport stops under it (outline D7). It must never panic.

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

fn spawn() -> Child {
    Command::new(env!("CARGO_BIN_EXE_mesh-lsp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("mesh-lsp starts")
}

fn frame(message: &Value) -> Vec<u8> {
    let body = message.to_string();
    format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
}

fn send(stdin: &mut ChildStdin, message: Value) {
    // The server may already have stopped; the test then checks how.
    let _ = stdin.write_all(&frame(&message));
    let _ = stdin.flush();
}

fn initialize() -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "processId": null, "rootUri": null, "capabilities": {} } })
}

fn notification(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "method": method, "params": params })
}

/// Waits for `child` to exit, draining its stdout so it never blocks, and
/// returns its exit code and stderr.
fn finish(mut child: Child) -> (Option<i32>, String) {
    if let Some(mut stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            let _ = std::io::copy(&mut stdout, &mut std::io::sink());
        });
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("mesh-lsp can be waited on") {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("mesh-lsp didn't exit within 20 s");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_string(&mut stderr);
    }
    (status.code(), stderr)
}

#[test]
fn shutdown_then_exit_is_status_0() {
    let mut child = spawn();
    let mut stdin = child.stdin.take().expect("stdin");
    send(&mut stdin, initialize());
    send(&mut stdin, notification("initialized", json!({})));
    send(
        &mut stdin,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "shutdown" }),
    );
    send(&mut stdin, notification("exit", Value::Null));
    let (code, stderr) = finish(child);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn exit_without_shutdown_is_status_1() {
    let mut child = spawn();
    let mut stdin = child.stdin.take().expect("stdin");
    send(&mut stdin, initialize());
    send(&mut stdin, notification("exit", Value::Null));
    let (code, stderr) = finish(child);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn closing_stdin_stops_the_server() {
    let mut child = spawn();
    let mut stdin = child.stdin.take().expect("stdin");
    send(&mut stdin, initialize());
    drop(stdin);
    let (code, stderr) = finish(child);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn closing_stdout_stops_the_server() {
    let mut child = spawn();
    drop(child.stdout.take());
    let mut stdin = child.stdin.take().expect("stdin");
    send(&mut stdin, initialize());
    send(&mut stdin, notification("initialized", json!({})));
    // Each of these makes the server write, which now fails.
    for version in 1..=3 {
        send(
            &mut stdin,
            notification(
                "textDocument/didOpen",
                json!({ "textDocument": {
                    "uri": format!("file:///a{version}.mprx"), "languageId": "mprx",
                    "version": version, "text": "<a"
                } }),
            ),
        );
    }
    // stdin stays open: the server must stop because of stdout alone.
    let (code, stderr) = finish(child);
    drop(stdin);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn a_message_that_isnt_json_stops_the_server_cleanly() {
    let mut child = spawn();
    let mut stdin = child.stdin.take().expect("stdin");
    send(&mut stdin, initialize());
    let _ = stdin.write_all(b"Content-Length: 5\r\n\r\n{nope");
    let _ = stdin.flush();
    let (code, stderr) = finish(child);
    drop(stdin);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}
