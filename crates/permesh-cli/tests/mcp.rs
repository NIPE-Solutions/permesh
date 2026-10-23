// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used, clippy::expect_used)]
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Output, Stdio},
    sync::mpsc::{Receiver, channel},
    time::Duration,
};

const VERSION: &str = "2026-07-28";
const INSTRUCTIONS: &str = "Permesh answers who has access to what, read-only. Every tool result has `complete`: when it is false, some providers could not be fully read and missing access is not proof of absence, so say you cannot tell. Plan tools write a new plan file for a person to review and carry out; Permesh never changes access.";

fn command(dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_permesh"));
    command
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .env("PERMESH_DATA_DIR", dir.join("state"));
    command
}
fn run(dir: &Path, args: &[&str]) -> Output {
    command(dir).args(args).output().unwrap()
}
fn demo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    dir
}
fn meta() -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": VERSION,
        "io.modelcontextprotocol/clientCapabilities": {},
        "io.modelcontextprotocol/clientInfo": {"name": "test", "version": "0"}
    })
}

/// A newline-delimited JSON-RPC client for `permesh mcp`; every stdout line is kept.
struct Client {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    seen: Vec<String>,
    next: u64,
}
impl Client {
    fn start(dir: &Path) -> Self {
        let mut child = command(dir)
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (tx, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            stdin,
            lines,
            seen: vec![],
            next: 1,
        }
    }
    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }
    /// Sends a request with modern `_meta` merged into params; returns its id.
    fn request_raw(&mut self, method: &str, params: Value) -> u64 {
        let id = self.next;
        self.next += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        id
    }
    fn start_request(&mut self, method: &str, mut params: Value) -> u64 {
        params["_meta"] = meta();
        self.request_raw(method, params)
    }
    fn response(&mut self, id: u64) -> Value {
        loop {
            let line = self
                .lines
                .recv_timeout(Duration::from_secs(60))
                .expect("no response from permesh mcp");
            self.seen.push(line.clone());
            let value: Value = serde_json::from_str(&line).unwrap();
            if value["id"] == json!(id) {
                return value;
            }
        }
    }
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.start_request(method, params);
        self.response(id)
    }
    fn call(&mut self, name: &str, arguments: Value) -> Value {
        let response = self.request("tools/call", json!({"name": name, "arguments": arguments}));
        assert!(response["error"].is_null(), "{response}");
        response["result"].clone()
    }
    fn tool_names(&mut self) -> Vec<String> {
        let response = self.request("tools/list", json!({}));
        response["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect()
    }
    /// Closes stdin and returns the exit code and every stdout line.
    fn finish(mut self) -> (Option<i32>, Vec<String>) {
        drop(self.stdin.take());
        let status = self.child.wait().unwrap();
        while let Ok(line) = self.lines.recv_timeout(Duration::from_secs(5)) {
            self.seen.push(line);
        }
        (status.code(), self.seen)
    }
}

#[test]
fn server_discover_describes_server() {
    let dir = demo();
    let mut client = Client::start(dir.path());
    let result = client.request("server/discover", json!({}))["result"].clone();
    assert_eq!(result["supportedVersions"], json!([VERSION]), "{result}");
    assert_eq!(
        result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "permesh"
    );
    assert_eq!(result["instructions"], INSTRUCTIONS);
    assert!(result["capabilities"]["tools"].is_object(), "{result}");
}

#[test]
fn tools_list_without_handshake() {
    let dir = demo();
    let mut client = Client::start(dir.path());
    assert_eq!(client.tool_names(), ["doctor"]);
}

#[test]
fn legacy_initialize_is_rejected() {
    let dir = demo();
    let mut client = Client::start(dir.path());
    let id = client.request_raw(
        "initialize",
        json!({"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "old", "version": "0"}}),
    );
    let response = client.response(id);
    assert_eq!(response["error"]["code"], -32022, "{response}");
    assert_eq!(response["error"]["data"]["supported"], json!([VERSION]));
    let (code, lines) = client.finish();
    assert_eq!(code, Some(2));
    assert!(
        lines
            .iter()
            .all(|l| serde_json::from_str::<Value>(l).is_ok())
    );
}

#[test]
fn missing_meta_is_invalid_params() {
    let dir = demo();
    let mut client = Client::start(dir.path());
    let id = client.request_raw("tools/list", json!({}));
    assert_eq!(client.response(id)["error"]["code"], -32602);
    assert_eq!(client.tool_names(), ["doctor"]);
}

#[test]
fn doctor_tool_returns_report() {
    let dir = demo();
    let mut client = Client::start(dir.path());
    let result = client.call("doctor", json!({}));
    assert_eq!(result["isError"], false, "{result}");
    let structured = &result["structuredContent"];
    assert_eq!(structured["exit_code"], 0, "{result}");
    assert_eq!(structured["report"]["command"], "doctor");
    let text: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(&text, structured);
}

#[test]
fn unknown_tool_and_argument_are_errors() {
    let dir = demo();
    let mut client = Client::start(dir.path());
    for (name, arguments) in [("nope", json!({})), ("doctor", json!({"detail": true}))] {
        let response = client.request("tools/call", json!({"name": name, "arguments": arguments}));
        let tool_error = response["result"]["isError"] == true;
        let rpc_error = response["error"]["code"] == -32602;
        assert!(tool_error || rpc_error, "{response}");
    }
    assert_eq!(client.tool_names(), ["doctor"]);
}

#[test]
fn stdout_is_only_json_rpc() {
    let dir = demo();
    let mut client = Client::start(dir.path());
    client.request("server/discover", json!({}));
    client.tool_names();
    client.call("doctor", json!({"details": true}));
    let (code, lines) = client.finish();
    assert_eq!(code, Some(0));
    assert!(!lines.is_empty());
    for line in lines {
        let value: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["jsonrpc"], "2.0", "{line}");
    }
}

#[test]
fn outside_workspace_exits_input() {
    let dir = tempfile::tempdir().unwrap();
    let output = command(dir.path())
        .arg("mcp")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}
