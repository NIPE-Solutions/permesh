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

const TOOLS: &[&str] = &[
    "doctor",
    "user_access",
    "resource_access",
    "admins",
    "orphaned",
    "profile_check",
    "plan_join",
    "plan_move",
    "plan_leave",
    "verify_plan",
];
const VERSION: &str = "2026-07-28";
const INSTRUCTIONS: &str = "Permesh answers who has access to what, read-only. Successful tool results have `report.complete`: when it is false, some providers could not be fully read and missing access is not proof of absence, so say you cannot tell. Plan tools write a new plan file for a person to review and carry out; Permesh never changes access.";

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
    assert_eq!(client.tool_names(), TOOLS);
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

#[cfg(unix)]
#[test]
fn closed_output_is_not_blamed_on_protocol() {
    let dir = demo();
    let mut child = command(dir.path())
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Nobody reads the answer: replying to the client fails.
    drop(child.stdout.take());
    let mut stdin = child.stdin.take().unwrap();
    writeln!(
        stdin,
        "{}",
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "2025-11-25", "capabilities": {},
            "clientInfo": {"name": "old", "version": "0"}}})
    )
    .unwrap();
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(5), "{stderr}");
    assert!(stderr.contains("could not reach the assistant"), "{stderr}");
    assert!(!stderr.contains("2026-07-28"), "{stderr}");
}

#[test]
fn missing_meta_is_invalid_params() {
    let dir = demo();
    let mut client = Client::start(dir.path());
    let id = client.request_raw("tools/list", json!({}));
    assert_eq!(client.response(id)["error"]["code"], -32602);
    assert_eq!(client.tool_names(), TOOLS);
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
    assert_eq!(client.tool_names(), TOOLS);
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

const BACKEND: &str = "profile: backend\ngrants:\n  - {instance: demo, group: team-10}\n  - {instance: demo, role: Read, resource: repo-21}\n";

fn json_run(dir: &Path, args: &[&str]) -> Value {
    let output = command(dir).args(args).arg("--json").output().unwrap();
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| panic!("{args:?}: {output:?}"))
}
fn write(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}
fn structured(result: &Value) -> &Value {
    &result["structuredContent"]
}

#[test]
fn user_access_matches_cli() {
    let dir = demo();
    let cli = json_run(dir.path(), &["user", "alice@example.com"]);
    let mut client = Client::start(dir.path());
    let result = client.call("user_access", json!({"identity": "alice@example.com"}));
    assert_eq!(result["isError"], false, "{result}");
    let report = &structured(&result)["report"];
    assert_eq!(report["result"], cli["result"]);
    assert_eq!(report["schema_version"], 2);
    let access = report["result"]["access"].as_array().unwrap();
    assert!(!access.is_empty());
    assert!(
        access.iter().all(|path| path["certainty"].is_string()),
        "{report}"
    );
}

#[test]
fn user_access_not_found_is_error_exit_1() {
    let dir = demo();
    let mut client = Client::start(dir.path());
    let result = client.call("user_access", json!({"identity": "missing@example.com"}));
    assert_eq!(result["isError"], true, "{result}");
    assert_eq!(structured(&result)["exit_code"], 1);
}

#[test]
fn resource_access_by_query_and_by_id() {
    let dir = demo();
    let mut client = Client::start(dir.path());
    let by_id = client.call(
        "resource_access",
        json!({"instance": "demo", "id": "repo-20"}),
    );
    assert_eq!(by_id["isError"], false, "{by_id}");
    let resource = &structured(&by_id)["report"]["result"]["resource"];
    assert_eq!(resource["key"]["id"], "repo-20");
    let name = resource["name"].as_str().unwrap();
    let by_query = client.call("resource_access", json!({"query": name}));
    assert_eq!(
        structured(&by_query)["report"]["result"]["resource"]["key"]["id"],
        "repo-20",
        "{by_query}"
    );
    let cli = json_run(
        dir.path(),
        &["resource", "--instance", "demo", "--id", "repo-20"],
    );
    let timeless = |value: &Value| {
        let mut value = value.clone();
        for key in ["observation_started_at", "observation_completed_at"] {
            value.as_object_mut().unwrap().remove(key);
        }
        value
    };
    assert!(
        timeless(&structured(&by_id)["report"]["result"]) == timeless(&cli["result"]),
        "resource result differs from the CLI"
    );
}

#[test]
fn resource_access_rejects_both_forms() {
    let dir = demo();
    let mut client = Client::start(dir.path());
    for arguments in [
        json!({"query": "x", "instance": "demo", "id": "repo-20"}),
        json!({}),
        json!({"instance": "demo"}),
    ] {
        let result = client.call("resource_access", arguments);
        assert_eq!(result["isError"], true, "{result}");
        assert_eq!(structured(&result)["exit_code"], 2);
        assert_eq!(
            structured(&result)["error"]["message"],
            "Give query, or instance and id"
        );
    }
}

#[test]
fn admins_and_orphaned_match_cli() {
    let dir = demo();
    let mut client = Client::start(dir.path());
    for (tool, command) in [("admins", "admins"), ("orphaned", "orphaned")] {
        let cli = json_run(dir.path(), &[command]);
        let result = client.call(tool, json!({}));
        if cli["error"].is_object() {
            assert_eq!(result["isError"], true, "{result}");
            assert_eq!(
                structured(&result)["error"]["message"],
                cli["error"]["message"]
            );
            assert_eq!(structured(&result)["exit_code"], cli["error"]["code"]);
        } else {
            assert_eq!(result["isError"], false, "{result}");
            assert_eq!(structured(&result)["report"]["result"], cli["result"]);
        }
    }
}

#[test]
fn profile_check_reports_unexplained() {
    let dir = demo();
    write(dir.path(), "profiles/backend.yaml", BACKEND);
    write(
        dir.path(),
        "people/bob.yaml",
        "identity: bob@example.com\nprofiles: [backend]\n",
    );
    let cli = json_run(dir.path(), &["profile", "check"]);
    let mut client = Client::start(dir.path());
    let result = client.call("profile_check", json!({}));
    assert_eq!(result["isError"], false, "{result}");
    assert_eq!(structured(&result)["report"]["result"], cli["result"]);
    assert_eq!(structured(&result)["report"]["command"], "profile_check");
}

#[test]
fn incomplete_capture_carries_note() {
    let dir = demo();
    write(dir.path(), "profiles/backend.yaml", BACKEND);
    write(
        dir.path(),
        "people/bob.yaml",
        "identity: bob@example.com\nprofiles: [backend]\n",
    );
    assert!(
        run(dir.path(), &["snapshot", "create", "--output", "s.json"])
            .status
            .success()
    );
    let mut snapshot: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("s.json")).unwrap()).unwrap();
    let limitations = json!(["Fixture visibility incomplete"]);
    snapshot["providers"][0]["state"] = "partial".into();
    snapshot["providers"][0]["limitations"] = limitations.clone();
    snapshot["providers"][0]["data"]["complete"] = false.into();
    snapshot["providers"][0]["data"]["limitations"] = limitations;
    std::fs::write(dir.path().join("partial.json"), snapshot.to_string()).unwrap();
    let mut client = Client::start(dir.path());
    let result = client.call("profile_check", json!({"snapshot": "partial.json"}));
    assert_eq!(result["isError"], false, "{result}");
    assert_eq!(structured(&result)["exit_code"], 4);
    assert_eq!(structured(&result)["report"]["complete"], false);
    assert_eq!(
        structured(&result)["note"],
        "Some providers could not be fully read. Missing access here is not proof of absence."
    );
}

#[test]
fn snapshot_outside_workspace_is_input_error() {
    let dir = demo();
    let mut client = Client::start(dir.path());
    let result = client.call("profile_check", json!({"snapshot": "../s.json"}));
    assert_eq!(result["isError"], true, "{result}");
    assert_eq!(structured(&result)["exit_code"], 2);
    assert_eq!(
        structured(&result)["error"]["message"],
        "Path is outside the workspace"
    );
}

fn step_ids(plan: &Value) -> std::collections::BTreeSet<String> {
    plan["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step["id"].as_str().unwrap().to_owned())
        .collect()
}
fn planned() -> tempfile::TempDir {
    let dir = demo();
    write(dir.path(), "profiles/backend.yaml", BACKEND);
    write(
        dir.path(),
        "people/bob.yaml",
        "identity: bob@example.com\nprofiles: [backend]\n",
    );
    std::fs::create_dir(dir.path().join("plans")).unwrap();
    dir
}

#[test]
fn plan_join_writes_plan_file() {
    let dir = planned();
    let mut client = Client::start(dir.path());
    let result = client.call(
        "plan_join",
        json!({"person": "carol@example.com", "profiles": ["backend"], "output": "plans/carol.json"}),
    );
    assert_eq!(result["isError"], false, "{result}");
    assert_eq!(structured(&result)["exit_code"], 0);
    let report = &structured(&result)["report"];
    assert_eq!(report["command"], "access_plan");
    let file: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("plans/carol.json")).unwrap())
            .unwrap();
    assert_eq!(file, report["result"]["access_plan"]);
    assert_eq!(file["kind"], "join");
    let cli = command(dir.path())
        .args(["access", "verify", "plans/carol.json", "--json"])
        .output()
        .unwrap();
    assert_ne!(cli.status.code(), Some(2), "{cli:?}");
}

#[test]
fn plan_tool_never_overwrites() {
    let dir = planned();
    let mut client = Client::start(dir.path());
    let arguments = json!({"person": "carol@example.com", "profiles": ["backend"], "output": "plans/carol.json"});
    client.call("plan_join", arguments.clone());
    let before = std::fs::read(dir.path().join("plans/carol.json")).unwrap();
    let result = client.call("plan_join", arguments);
    assert_eq!(result["isError"], true, "{result}");
    assert_eq!(structured(&result)["exit_code"], 2);
    assert_eq!(
        structured(&result)["error"]["message"],
        "Output file already exists"
    );
    assert_eq!(
        std::fs::read(dir.path().join("plans/carol.json")).unwrap(),
        before
    );
}

#[test]
fn plan_move_and_leave_match_cli() {
    let dir = planned();
    let mut client = Client::start(dir.path());
    let moved = client.call(
        "plan_move",
        json!({"person": "alice@example.com", "remove": ["backend"], "output": "plans/move.json"}),
    );
    assert_eq!(moved["isError"], false, "{moved}");
    let left = client.call(
        "plan_leave",
        json!({"person": "alice@example.com", "output": "plans/leave.json", "expires_in_hours": 48}),
    );
    assert_eq!(left["isError"], false, "{left}");
    let cli_move = json_run(
        dir.path(),
        &[
            "access",
            "move",
            "alice@example.com",
            "--remove",
            "backend",
            "--output",
            "cli-move.json",
        ],
    );
    let cli_leave = json_run(
        dir.path(),
        &[
            "access",
            "leave",
            "alice@example.com",
            "--output",
            "cli-leave.json",
        ],
    );
    let ids = |value: &Value| step_ids(&value["result"]["access_plan"]);
    assert!(!ids(&cli_move).is_empty());
    assert_eq!(ids(&structured(&moved)["report"]), ids(&cli_move));
    assert_eq!(ids(&structured(&left)["report"]), ids(&cli_leave));
    assert_eq!(
        structured(&left)["report"]["result"]["access_plan"]["kind"],
        "leave"
    );
}

#[test]
fn verify_plan_reports_steps() {
    let dir = planned();
    let mut client = Client::start(dir.path());
    client.call(
        "plan_join",
        json!({"person": "bob@example.com", "profiles": ["backend"], "output": "plans/bob.json"}),
    );
    let result = client.call("verify_plan", json!({"plan": "plans/bob.json"}));
    let cli = command(dir.path())
        .args(["access", "verify", "plans/bob.json", "--json"])
        .output()
        .unwrap();
    let cli_report: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(result["isError"], false, "{result}");
    assert_eq!(
        structured(&result)["exit_code"],
        json!(cli.status.code().unwrap())
    );
    assert_eq!(structured(&result)["exit_code"], 6);
    assert_eq!(
        structured(&result)["report"]["result"]["checks"],
        cli_report["result"]["checks"]
    );
}

#[test]
fn plan_output_outside_workspace_is_refused() {
    let dir = planned();
    let mut client = Client::start(dir.path());
    let result = client.call(
        "plan_join",
        json!({"person": "carol@example.com", "profiles": ["backend"], "output": "../x.json"}),
    );
    assert_eq!(result["isError"], true, "{result}");
    assert_eq!(structured(&result)["exit_code"], 2);
    assert_eq!(
        structured(&result)["error"]["message"],
        "Path is outside the workspace"
    );
    assert!(!dir.path().parent().unwrap().join("x.json").exists());
}

/// A workspace whose only provider hangs in discovery after writing `ready.pid`.
#[cfg(unix)]
fn slow_workspace() -> (tempfile::TempDir, std::path::PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let digest = fixture(&root);
    let config = json!({"version":1,"organization":{"name":"Cancellation fixture"},
        "providers":[{"id":"instance","type":"external","external":{"provider":"fixture","sha256":digest,"configuration":{},"credentials":{}}}]});
    std::fs::write(
        root.join("permesh.yaml"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    let review = json_run(&root, &["provider", "external", "review", "instance"]);
    let fingerprint = review["result"]["fingerprint"].as_str().unwrap();
    json_run(
        &root,
        &[
            "provider",
            "external",
            "approve",
            "instance",
            "--fingerprint",
            fingerprint,
            "--accept-risk",
        ],
    );
    (temporary, root)
}
#[cfg(unix)]
fn wait_for_provider(root: &Path) -> String {
    let marker = root.join("ready.pid");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !marker.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    std::fs::read_to_string(&marker)
        .expect("discovery never started")
        .trim()
        .to_owned()
}
#[cfg(unix)]
fn alive(pid: &str) -> bool {
    Command::new("kill")
        .args(["-0", pid])
        .output()
        .unwrap()
        .status
        .success()
}

#[cfg(unix)]
#[test]
fn sigterm_during_call_stops_providers() {
    let (_temporary, root) = slow_workspace();
    let mut client = Client::start(&root);
    client.start_request(
        "tools/call",
        json!({"name": "user_access", "arguments": {"identity": "missing"}}),
    );
    let provider = wait_for_provider(&root);
    let server = client.child.id().to_string();
    assert!(
        Command::new("kill")
            .args(["-TERM", &server])
            .status()
            .unwrap()
            .success()
    );
    let (code, lines) = client.finish();
    let survived = alive(&provider);
    if survived {
        let _ = Command::new("kill").args(["-KILL", &provider]).status();
    }
    assert!(!survived, "provider outlived the stopped MCP server");
    assert_eq!(code, Some(130));
    for line in lines {
        assert!(serde_json::from_str::<Value>(&line).is_ok(), "{line}");
    }
}

#[cfg(unix)]
#[test]
fn stdin_eof_during_call_stops_providers() {
    let (_temporary, root) = slow_workspace();
    let mut client = Client::start(&root);
    client.start_request(
        "tools/call",
        json!({"name": "user_access", "arguments": {"identity": "missing"}}),
    );
    let provider = wait_for_provider(&root);
    let (_, _) = client.finish();
    let survived = alive(&provider);
    if survived {
        let _ = Command::new("kill").args(["-KILL", &provider]).status();
    }
    assert!(!survived, "provider outlived the closed MCP session");
}

#[cfg(unix)]
#[test]
fn cancelled_call_drains_and_server_continues() {
    use std::time::Instant;
    let (_temporary, root) = slow_workspace();
    let mut client = Client::start(&root);
    let id = client.start_request(
        "tools/call",
        json!({"name": "user_access", "arguments": {"identity": "missing"}}),
    );
    let marker = root.join("ready.pid");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !marker.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let provider_pid = std::fs::read_to_string(&marker).expect("discovery never started");
    client.send(
        &json!({"jsonrpc": "2.0", "method": "notifications/cancelled",
        "params": {"requestId": id, "reason": "test", "_meta": meta()}}),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    let alive = || {
        Command::new("kill")
            .args(["-0", provider_pid.trim()])
            .output()
            .unwrap()
            .status
            .success()
    };
    while alive() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let still_alive = alive();
    if still_alive {
        let _ = Command::new("kill")
            .args(["-KILL", provider_pid.trim()])
            .status();
    }
    assert!(
        !still_alive,
        "cancelled provider was not terminated and reaped"
    );
    assert_eq!(client.tool_names().len(), TOOLS.len());
    let (_, lines) = client.finish();
    for line in lines {
        let value: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["jsonrpc"], "2.0", "{line}");
        assert!(!line.contains("CANCELLATION_PRIVATE"), "{line}");
    }
}

#[cfg(unix)]
#[test]
fn cancelled_queued_call_never_starts() {
    let (_temporary, root) = slow_workspace();
    let mut client = Client::start(&root);
    let running = client.start_request(
        "tools/call",
        json!({"name": "user_access", "arguments": {"identity": "missing"}}),
    );
    let provider = wait_for_provider(&root);
    let queued = client.start_request(
        "tools/call",
        json!({"name": "user_access", "arguments": {"identity": "missing"}}),
    );
    for id in [queued, running] {
        client.send(
            &json!({"jsonrpc": "2.0", "method": "notifications/cancelled",
            "params": {"requestId": id, "reason": "test", "_meta": meta()}}),
        );
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while alive(&provider) && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive(&provider), "cancelled provider was not reaped");
    // The server stays usable, and the cancelled queued call started no provider.
    assert_eq!(client.tool_names().len(), TOOLS.len());
    std::thread::sleep(Duration::from_millis(500));
    let starts = std::fs::read_to_string(root.join("ready.starts")).unwrap();
    assert_eq!(starts.lines().count(), 1, "{starts}");
    let _ = client.finish();
}

/// A real native provider speaks either legacy protocol, exposes a private
/// observation, then hangs after marking its PID. This synchronizes SIGINT with
/// active discovery rather than relying on startup timing.
#[cfg(unix)]
fn fixture(root: &Path) -> String {
    let source = root.join("peer.rs");
    let binary = root.join("peer");
    let code = r##"
use std::io::{BufRead, Write};
fn main() {
    let mut lines = std::io::stdin().lock().lines();
    let handshake = lines.next().unwrap().unwrap();
    let version = if handshake.contains("\"protocol\":2") { 2 } else { 1 };
    println!("{}", r#"{"protocol":VERSION,"id":"handshake","event":"handshake","provider":"fixture","capabilities":["accounts"],"draft":true}"#.replace("VERSION", &version.to_string()));
    std::io::stdout().flush().unwrap();
    let discover = lines.next().unwrap().unwrap();
    assert!(discover.contains("\"method\":\"discover\""));
    println!("{}", r#"{"protocol":VERSION,"id":"discover","event":"record","kind":"account","data":{"key":{"provider":"instance","id":"guest"},"login":"CANCELLATION_PRIVATE_OBSERVATION","kind":"human","verified_emails":[]}}"#.replace("VERSION", &version.to_string()));
    std::io::stdout().flush().unwrap();
    eprintln!("CANCELLATION_PRIVATE_STDERR");
    let marker = std::path::Path::new(MARKER);
    let mut starts = std::fs::OpenOptions::new().create(true).append(true).open(marker.with_extension("starts")).unwrap();
    writeln!(starts, "{}", std::process::id()).unwrap();
    let temporary = marker.with_extension("tmp");
    std::fs::write(&temporary, std::process::id().to_string()).unwrap();
    std::fs::rename(temporary, marker).unwrap();
    // Deliberately ignore cancellation; the host must terminate and reap us.
    loop { std::thread::sleep(std::time::Duration::from_secs(60)); }
}
"##
    .replace("MARKER", &format!("{:?}", root.join("ready.pid")));
    std::fs::write(&source, code).unwrap();
    assert!(
        Command::new("rustc")
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .status()
            .unwrap()
            .success()
    );
    let inspect = json_run(
        root,
        &["provider", "external", "inspect", binary.to_str().unwrap()],
    );
    let digest = inspect["result"]["inspection"]["sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    json_run(
        root,
        &[
            "provider",
            "external",
            "trust",
            binary.to_str().unwrap(),
            "--id",
            "fixture",
            "--sha256",
            &digest,
            "--capability",
            "accounts",
            "--accept-risk",
        ],
    );
    digest
}
