use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

fn host(response: &'static str) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut input = BufReader::new(stream);
        let mut headers = String::new();
        let mut size = 0;
        loop {
            let mut line = String::new();
            input.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some(v) = line.to_lowercase().strip_prefix("content-length:") {
                size = v.trim().parse().unwrap();
            }
            headers.push_str(&line);
        }
        let mut body = vec![0; size];
        input.read_exact(&mut body).unwrap();
        tx.send(format!("{headers}\n{}", String::from_utf8(body).unwrap()))
            .unwrap();
        input.get_mut().write_all(response.as_bytes()).unwrap();
    });
    (origin, rx)
}

#[test]
fn stdio_forwards_protocol_and_reloads_descriptor_without_gui_or_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let descriptor = temp.path().join("runtime.json");
    let (origin,request)=host("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n");
    std::fs::write(
        &descriptor,
        json!({"origin":origin,"registrations":{"codex":{"token":"first-fixture-token"}}})
            .to_string(),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_studio-teams-mcp"))
        .args([
            "--runtime-file",
            descriptor.to_str().unwrap(),
            "--backend",
            "codex",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}})
    )
    .unwrap();
    let request = request.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(request.contains("POST /teams/mcp/codex HTTP/1.1"));
    assert!(request
        .to_lowercase()
        .contains("authorization: bearer first-fixture-token"));
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    let reply: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(reply["id"], 1);
    assert!(reply.get("result").is_some(), "{reply}");
    let (origin, request) = host("HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n");
    std::fs::write(
        &descriptor,
        json!({"origin":origin,"registrations":{"codex":{"token":"second-fixture-token"}}})
            .to_string(),
    )
    .unwrap();
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    let request = request.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(request
        .to_lowercase()
        .contains("authorization: bearer second-fixture-token"));
    drop(input);
    line.clear();
    output.read_to_string(&mut line).unwrap();
    assert!(line.is_empty());
    assert!(child.wait().unwrap().success());
}

#[test]
fn redirects_are_not_followed_and_errors_stay_in_jsonrpc() {
    let temp = tempfile::tempdir().unwrap();
    let descriptor = temp.path().join("runtime.json");
    let (origin,request)=host("HTTP/1.1 302 Found\r\nLocation: http://example.invalid/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    std::fs::write(
        &descriptor,
        json!({"origin":origin,"registrations":{"codex":{"token":"fixture"}}}).to_string(),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_studio-teams-mcp"))
        .args([
            "--runtime-file",
            descriptor.to_str().unwrap(),
            "--backend",
            "codex",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(
        child.stdin.take().unwrap(),
        "{}",
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})
    )
    .unwrap();
    request.recv_timeout(Duration::from_secs(5)).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let reply: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(reply["id"], 2);
    assert!(reply["error"]["message"].as_str().unwrap().contains("302"));
}
