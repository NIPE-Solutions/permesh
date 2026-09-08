// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
//! Native synthetic test fixture; never installed or registered automatically.
use std::{
    fs,
    io::{self, BufRead, Write},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
fn emit(value: serde_json::Value) {
    let mut out = io::stdout().lock();
    serde_json::to_writer(&mut out, &value).unwrap();
    writeln!(out).unwrap();
    out.flush().unwrap();
}
fn marker(name: &str) -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join(name)
}
fn hang() -> ! {
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == "--capture-input") {
        io::copy(&mut io::stdin().lock(), &mut io::stdout().lock()).unwrap();
        return;
    }
    if args.get(1).is_some_and(|arg| arg == "--descendant") {
        let until = Instant::now() + Duration::from_secs(10);
        let mut count = 0;
        while Instant::now() < until {
            fs::write(&args[2], count.to_string()).unwrap();
            count += 1;
            thread::sleep(Duration::from_millis(10));
        }
        return;
    }
    let mut lines = io::stdin().lock().lines();
    let request: serde_json::Value = serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
    let instance = request["instance"].as_str().unwrap();
    if request["protocol"] == 5 {
        negotiated_peer(instance, request["operation"].as_str().unwrap(), &mut lines);
        return;
    }
    if request["protocol"] == 4 {
        emit(
            serde_json::json!({"protocol":4,"id":"handshake","event":"handshake","provider":"fixture","capabilities":[],"draft":true}),
        );
        let request: serde_json::Value =
            serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
        assert_eq!(
            request,
            serde_json::json!({"protocol":4,"id":"describe_auth","method":"describe_auth"})
        );
        fs::write(marker("ready"), "yes").unwrap();
        let request: serde_json::Value =
            serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
        assert_eq!(
            request,
            serde_json::json!({"protocol":4,"id":"cancel","method":"cancel"})
        );
        emit(serde_json::json!({"protocol":4,"id":"cancel","event":"cancelled"}));
        fs::write(marker("cancelled"), "yes").unwrap();
        return;
    }
    if request["protocol"] == 3 {
        describe_peer(instance, &mut lines);
        return;
    }
    if request["protocol"] == 2 {
        configured_peer(instance, &mut lines);
        return;
    }
    if instance == "handshake-hang" {
        hang();
    }
    if instance == "handshake-trickle" {
        loop {
            print!(" ");
            io::stdout().flush().unwrap();
            thread::sleep(Duration::from_millis(20));
        }
    }
    let provider = if instance == "wrong-provider" {
        "wrong"
    } else {
        "fixture"
    };
    let capabilities = if matches!(instance, "wrong-caps" | "wrong-instance") {
        serde_json::json!(["accounts"])
    } else {
        serde_json::json!([])
    };
    emit(
        serde_json::json!({"protocol":1,"id":"handshake","event":"handshake","provider":provider,"capabilities":capabilities,"draft":true}),
    );
    let Some(Ok(line)) = lines.next() else {
        return;
    };
    let request: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(request["method"], "discover");
    match instance {
        "operation-hang" => hang(),
        "wrong-instance" => {
            emit(
                serde_json::json!({"protocol":1,"id":"discover","event":"record","kind":"account","data":{"key":{"provider":"other","id":"alice"},"login":"alice","kind":"human","verified_emails":[]}}),
            );
            emit(
                serde_json::json!({"protocol":1,"id":"discover","event":"complete","count":1,"complete":true,"limitations":[]}),
            );
            assert!(lines.next().is_none());
            return;
        }
        "stderr-limit" => {
            io::stderr().write_all(&vec![b'x'; 64 * 1024]).unwrap();
        }

        "cancel" => {
            fs::write(marker("ready"), "yes").unwrap();
            let request: serde_json::Value =
                serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
            assert_eq!(request["method"], "cancel");
            emit(serde_json::json!({"protocol":1,"id":"cancel","event":"cancelled"}));
            fs::write(marker("cancelled"), "yes").unwrap();
            return;
        }
        "stdout-flood" => {
            let _ = io::stdout().write_all(&vec![b'x'; 2 * 1024 * 1024]);
            hang();
        }
        "stderr-flood" => {
            let _ = io::stderr().write_all(&vec![b'x'; 128 * 1024]);
            hang();
        }
        "malformed" => {
            println!("SENTINEL_SECRET");
            return;
        }
        "truncated" => {
            print!("{{");
            return;
        }
        "incomplete" => return,
        "environment" => {
            assert_eq!(args.len(), 1);
            assert!(std::env::vars_os().all(|(key, _)| cfg!(windows)
                && key.to_string_lossy().eq_ignore_ascii_case("SystemRoot")));
            let cwd = std::env::current_dir().unwrap();
            assert_ne!(cwd, std::env::current_exe().unwrap().parent().unwrap());
            assert!(fs::read_dir(&cwd).unwrap().next().is_none());
            fs::write(
                marker("working-directory"),
                cwd.to_string_lossy().as_bytes(),
            )
            .unwrap();
        }
        _ => {}
    }
    if instance.starts_with("descendant") {
        let path = marker("heartbeat");
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.arg("--descendant").arg(&path).stdin(Stdio::null());
        if instance == "descendant-detached-pipes" {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }
        // Deliberately leave descendants for the host supervision regression.
        #[allow(clippy::zombie_processes)]
        let _child = command.spawn().unwrap();
        let until = Instant::now() + Duration::from_secs(2);
        while !path.exists() && Instant::now() < until {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(path.exists());
        if instance == "descendant-malformed" {
            println!("bad");
            io::stdout().flush().unwrap();
            hang();
        }
        if instance == "descendant-hang" {
            hang();
        }
    }
    let complete = instance != "partial";
    let limitations = if complete {
        serde_json::json!([])
    } else {
        serde_json::json!(["visibility_limited"])
    };
    emit(
        serde_json::json!({"protocol":1,"id":"discover","event":"complete","count":0,"complete":complete,"limitations":limitations}),
    );
    if instance == "extra" {
        println!("{{}}");
    }
    if instance == "complete-hang" {
        hang();
    }
    assert!(lines.next().is_none());
    if matches!(instance, "nonzero" | "descendant-nonzero") {
        std::process::exit(7);
    }
}

fn configured_peer(instance: &str, lines: &mut impl Iterator<Item = io::Result<String>>) {
    use serde_json::json;
    let provider = if instance == "wrong-provider" {
        "wrong"
    } else {
        "fixture"
    };
    let version = if instance == "downgrade" { 1 } else { 2 };
    let capabilities = if instance == "wrong-caps" {
        json!([])
    } else {
        json!([
            "accounts",
            "identities",
            "resources",
            "groups",
            "memberships",
            "grants"
        ])
    };
    emit(
        json!({"protocol":version,"id":"handshake","event":"handshake","provider":provider,"capabilities":capabilities,"draft":true}),
    );
    let Some(Ok(line)) = lines.next() else {
        return;
    };
    if matches!(instance, "wrong-provider" | "wrong-caps" | "downgrade") {
        fs::write(marker("delivered"), "operation received").unwrap();
        return;
    }
    let request: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(request["protocol"], 2);
    assert_eq!(std::env::args_os().count(), 1);
    assert!(
        std::env::vars_os()
            .all(|(key, _)| cfg!(windows)
                && key.to_string_lossy().eq_ignore_ascii_case("SystemRoot"))
    );
    let credentials = request["credentials"].as_object().unwrap();
    if let Some(token) = credentials.get("token") {
        assert_eq!(token, "synthetic-fixture-token");
    }
    if instance == "two-first" {
        assert_eq!(credentials["client_secret"], "first-private-slot");
    }
    if instance == "two-second" {
        assert_eq!(credentials["client_secret"], "second-private-slot");
    }
    let mode = request["configuration"]["mode"]
        .as_str()
        .unwrap_or("normal");
    let method = request["method"].as_str().unwrap();
    assert_eq!(request["id"], method);
    if mode == "hang" {
        hang();
    }
    if mode == "test-hang" {
        fs::write(marker("heartbeat"), "0").unwrap();
        fs::write(marker("ready"), "yes").unwrap();
        let mut count = 0u64;
        loop {
            count += 1;
            fs::write(marker("heartbeat"), count.to_string()).unwrap();
            thread::sleep(Duration::from_millis(10));
        }
    }
    if mode == "echo-stderr" {
        eprintln!("{}", credentials["token"].as_str().unwrap());
    }
    if mode == "checkfail" {
        emit(json!({"protocol":2,"id":method,"event":"error","code":"authentication"}));
        return;
    }
    let limitations = if mode == "partial" {
        json!(["visibility_limited"])
    } else {
        json!([])
    };
    if method == "check" {
        emit(
            json!({"protocol":2,"id":"check","event":"health","status":"ok","limitations":limitations}),
        );
        assert!(lines.next().is_none());
        return;
    }
    assert_eq!(method, "discover");
    if matches!(mode, "echo" | "echo-escaped" | "echo-key") {
        let secret = credentials["token"].as_str().unwrap();
        if mode == "echo-escaped" {
            let encoded: String = secret
                .chars()
                .map(|c| format!("\\u{:04x}", c as u32))
                .collect();
            println!(
                "{{\"protocol\":2,\"id\":\"discover\",\"event\":\"record\",\"kind\":\"account\",\"data\":{{\"key\":{{\"provider\":\"{instance}\",\"id\":\"alice\"}},\"login\":\"{encoded}\",\"kind\":\"human\",\"verified_emails\":[]}}}}"
            );
            io::stdout().flush().unwrap();
        } else if mode == "echo-key" {
            emit(json!({"protocol":2,"id":"discover","event":"record",secret:"value"}));
        } else {
            emit(
                json!({"protocol":2,"id":"discover","event":"record","kind":"account","data":{"key":{"provider":instance,"id":"alice"},"login":secret,"kind":"human","verified_emails":[]}}),
            );
        }
        if mode != "echo-key" {
            emit(
                json!({"protocol":2,"id":"discover","event":"complete","count":1,"complete":true,"limitations":[]}),
            );
            assert!(lines.next().is_none());
        }
        return;
    }
    let alice = json!({"provider":instance,"id":"alice"});
    let orphan = json!({"provider":instance,"id":"orphan"});
    let group = json!({"provider":instance,"id":"backend"});
    let repository = json!({"provider":instance,"id":"repository"});
    let provenance =
        json!({"method":"synthetic trusted fixture","observed_at":"2026-01-01T00:00:00Z"});
    let records = [
        (
            "identity",
            json!({"id":"alice@example.com","kind":"human","status":"active","verified_emails":["alice@example.com"]}),
        ),
        (
            "identity",
            json!({"id":"orphan@example.com","kind":"human","status":"inactive","verified_emails":["orphan@example.com"]}),
        ),
        (
            "account",
            json!({"key":alice,"login":"alice-dev","kind":"human","verified_emails":["alice@example.com"]}),
        ),
        (
            "account",
            json!({"key":orphan,"login":"orphan-dev","kind":"human","verified_emails":["orphan@example.com"]}),
        ),
        (
            "resource",
            json!({"key":repository,"name":"Fixture repository"}),
        ),
        ("group", json!({"key":group,"name":"Backend"})),
        (
            "membership",
            json!({"member":{"kind":"account","key":alice},"group":group,"provenance":provenance}),
        ),
        (
            "grant",
            json!({"id":"owner","subject":{"kind":"group","key":group},"resource":repository,"role":"owner","privilege":"owner","certainty":"observed","provenance":provenance}),
        ),
        (
            "grant",
            json!({"id":"orphan-read","subject":{"kind":"account","key":orphan},"resource":repository,"role":"read","privilege":"standard","certainty":"observed","provenance":provenance}),
        ),
    ];
    for (kind, data) in &records {
        emit(json!({"protocol":2,"id":"discover","event":"record","kind":kind,"data":data}));
    }
    emit(
        json!({"protocol":2,"id":"discover","event":"complete","count":records.len(),"complete":mode!="partial","limitations":limitations}),
    );
    assert!(lines.next().is_none());
}

fn setup_spec() -> serde_json::Value {
    use serde_json::json;
    json!({"schema_version":1,"title":"Fixture setup","description":"Configure a synthetic permission provider.","steps":[
        {"id":"authentication","title":"Authentication","description":"Select credential references.","fields":[
            {"key":"auth_method","label":"Authentication method","help":"Choose token or service credentials.","required":true,"default":"token","input":{"type":"choice","options":[{"value":"token","label":"Token"},{"value":"service","label":"Service credentials"}]}}
        ]},
        {"id":"token_auth","title":"Token authentication","description":"Provide a token reference.","when":{"field":"auth_method","equals":"token"},"fields":[
            {"key":"token","label":"Token reference","help":"Enter an env:// or keychain:// reference, never a token value.","required":true,"input":{"type":"credential"}}
        ]},
        {"id":"service_auth","title":"Service authentication","description":"Provide service credential references.","when":{"field":"auth_method","equals":"service"},"fields":[
            {"key":"client_id","label":"Client ID","help":"Service application identifier.","required":true,"input":{"type":"text","min_length":1,"max_length":128}},
            {"key":"client_secret","label":"Client secret reference","help":"Enter an env:// or keychain:// reference, never a secret value.","required":true,"input":{"type":"credential"}}
        ]},
        {"id":"connection","title":"Connection","description":"Provider connection settings.","fields":[
            {"key":"tenant","label":"Tenant","help":"Directory tenant name.","required":true,"default":"acme","input":{"type":"text","min_length":1,"max_length":128}},
            {"key":"port","label":"Port","help":"Connection port.","required":true,"default":443,"input":{"type":"integer","minimum":1,"maximum":65535}},
            {"key":"enabled","label":"Enabled","help":"Enable this connection.","required":true,"default":true,"input":{"type":"boolean"}},
            {"key":"regions","label":"Regions","help":"List of region names.","required":false,"default":["eu-central"],"input":{"type":"string_list","min_items":0,"max_items":16}},
            {"key":"metadata","label":"Metadata","help":"Additional structured settings.","required":false,"default":{},"input":{"type":"json"}}
        ]}
    ]})
}
fn describe_peer(instance: &str, lines: &mut impl Iterator<Item = io::Result<String>>) {
    use serde_json::json;
    assert_eq!(std::env::args_os().count(), 1);
    assert!(
        std::env::vars_os()
            .all(|(key, _)| cfg!(windows)
                && key.to_string_lossy().eq_ignore_ascii_case("SystemRoot"))
    );
    let version = if instance == "describe-wrong-version" {
        2
    } else {
        3
    };
    let provider = if instance == "wrong-provider" {
        "wrong"
    } else {
        "fixture"
    };
    let capabilities = if instance == "wrong-caps" {
        json!([])
    } else {
        json!([
            "accounts",
            "identities",
            "resources",
            "groups",
            "memberships",
            "grants"
        ])
    };
    emit(
        json!({"protocol":version,"id":"handshake","event":"handshake","provider":provider,"capabilities":capabilities,"draft":true}),
    );
    let Some(Ok(line)) = lines.next() else {
        return;
    };
    let request: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(
        request,
        json!({"protocol":3,"id":"describe","method":"describe"})
    );
    if instance == "describe-timeout" {
        hang();
    }
    if instance == "describe-cancel" {
        fs::write(marker("ready"), "yes").unwrap();
        let request: serde_json::Value =
            serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
        assert_eq!(
            request,
            json!({"protocol":3,"id":"cancel","method":"cancel"})
        );
        fs::write(marker("cancelled"), "yes").unwrap();
        emit(json!({"protocol":3,"id":"cancel","event":"cancelled"}));
        return;
    }
    if instance == "describe-error" {
        emit(json!({"protocol":3,"id":"describe","event":"error","code":"unsupported_method"}));
        return;
    }
    let mut spec = setup_spec();
    if instance == "describe-malformed" {
        spec["extra"] = json!("SENTINEL");
    }
    if instance == "describe-invalid" {
        spec["steps"][1]["fields"][0]["default"] = json!("SENTINEL");
    }
    emit(json!({"protocol":3,"id":"describe","event":"setup","spec":spec}));
    if instance == "describe-no-eof" {
        hang();
    }
    if instance == "describe-extra" {
        println!("{{}}");
    }
    assert!(lines.next().is_none());
    if instance == "describe-nonzero" {
        std::process::exit(7);
    }
}

fn negotiated_peer(
    instance: &str,
    operation: &str,
    lines: &mut impl Iterator<Item = io::Result<String>>,
) {
    use serde_json::json;
    assert!(matches!(operation, "discover" | "check"));
    if instance == "handshake-hang" {
        hang();
    }
    let provider = if instance == "wrong-provider" {
        "wrong"
    } else {
        "fixture"
    };
    let version = if instance == "downgrade" { 2 } else { 5 };
    let capabilities = if instance == "wrong-caps" {
        json!([])
    } else {
        json!([
            "accounts",
            "identities",
            "resources",
            "groups",
            "memberships",
            "grants"
        ])
    };
    let operations = if instance == "wrong-operation" {
        json!([if operation == "check" {
            "discover"
        } else {
            "check"
        }])
    } else {
        json!(["discover", "check"])
    };
    emit(
        json!({"protocol":version,"id":"handshake","event":"handshake","provider":provider,"capabilities":capabilities,"operations":operations,"draft":true}),
    );
    let Some(Ok(line)) = lines.next() else {
        return;
    };
    if matches!(
        instance,
        "wrong-provider" | "wrong-caps" | "downgrade" | "wrong-operation"
    ) {
        fs::write(marker("delivered"), "operation received").unwrap();
        return;
    }
    let request: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(request["protocol"], 5);
    assert_eq!(request["method"], operation);
    assert_eq!(request["id"], operation);
    assert_eq!(request["credentials"]["token"], "synthetic-fixture-token");
    assert_eq!(std::env::args_os().count(), 1);
    assert!(
        std::env::vars_os()
            .all(|(key, _)| cfg!(windows)
                && key.to_string_lossy().eq_ignore_ascii_case("SystemRoot"))
    );
    let mode = request["configuration"]["mode"].as_str().unwrap();
    match mode {
        "environment" => {
            let cwd = std::env::current_dir().unwrap();
            assert!(fs::read_dir(&cwd).unwrap().next().is_none());
            fs::write(
                marker("working-directory"),
                cwd.to_string_lossy().as_bytes(),
            )
            .unwrap();
        }
        "cancel" => {
            fs::write(marker("ready"), "yes").unwrap();
            let request: serde_json::Value =
                serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap();
            assert_eq!(
                request,
                json!({"protocol":5,"id":"cancel","method":"cancel"})
            );
            fs::write(marker("cancelled"), "yes").unwrap();
            return;
        }
        "hang" => hang(),
        "stdout-flood" => {
            let _ = io::stdout().write_all(&vec![b'x'; 2 * 1024 * 1024]);
            hang();
        }
        "stderr-flood" => {
            let _ = io::stderr().write_all(&vec![b'x'; 128 * 1024]);
            hang();
        }
        "echo" | "echo-escaped" | "echo-key" => {
            let secret = request["credentials"]["token"].as_str().unwrap();
            let value = if mode == "echo-key" {
                json!({"protocol":5,"id":operation,"event":"error","code":"authentication",secret:"value"})
            } else if operation == "discover" {
                json!({"protocol":5,"id":"discover","event":"record","kind":"resource","data":{"key":{"provider":instance,"id":"reflected"},"name":secret,"kind":null,"parent":null}})
            } else {
                json!({"protocol":5,"id":operation,"event":"error","code":"authentication","provider_code":secret})
            };
            if mode == "echo-escaped" {
                let encoded: String = secret
                    .chars()
                    .map(|c| format!("\\u{:04x}", c as u32))
                    .collect();
                println!("{}", value.to_string().replace(secret, &encoded));
                io::stdout().flush().unwrap();
            } else {
                emit(value);
            }
            if operation == "discover" && mode != "echo-key" {
                emit(
                    json!({"protocol":5,"id":"discover","event":"complete","count":1,"complete":true,"limitations":[]}),
                );
                assert!(lines.next().is_none());
            }
            return;
        }
        "checkfail" => {
            emit(
                json!({"protocol":5,"id":operation,"event":"error","code":"authentication","provider_code":"token.invalid"}),
            );
            return;
        }
        _ => {}
    }
    let limitations = if mode == "partial" {
        json!(["visibility_limited"])
    } else {
        json!([])
    };
    if operation == "check" {
        emit(
            json!({"protocol":5,"id":"check","event":"health","status":"ok","limitations":limitations}),
        );
    } else {
        let account = json!({"provider":instance,"id":"service"});
        let root = json!({"provider":instance,"id":"root"});
        let child = json!({"provider":instance,"id":"child"});
        let group = json!({"provider":instance,"id":"team"});
        let provenance = json!({"method":"native fixture","observed_at":"2026-01-01T00:00:00Z"});
        let records = [
            (
                "identity",
                json!({"id":"service@example.com","kind":"service","affiliation":"internal","status":"inactive","verified_emails":["service@example.com"]}),
            ),
            (
                "account",
                json!({"key":account,"login":"service","kind":"service","affiliation":"external","status":"suspended","verified_emails":["service@example.com"]}),
            ),
            (
                "resource",
                json!({"key":root,"name":"Root","kind":null,"parent":null}),
            ),
            (
                "resource",
                json!({"key":child,"name":"Child","kind":"fixture.repository","parent":root}),
            ),
            ("group", json!({"key":group,"name":"Team"})),
            (
                "membership",
                json!({"member":{"kind":"account","key":account},"group":group,"provenance":provenance}),
            ),
            (
                "grant",
                json!({"id":"assignment","subject":{"kind":"group","key":group},"resource":child,"role":"read","privilege":"standard","certainty":"derived","evidence_kind":"policy_attachment","provenance":provenance}),
            ),
        ];
        for (kind, data) in &records {
            emit(json!({"protocol":5,"id":"discover","event":"record","kind":kind,"data":data}));
        }
        if mode == "error-after-record" {
            emit(
                json!({"protocol":5,"id":"discover","event":"error","code":"authentication","provider_code":"token.invalid"}),
            );
            return;
        }
        if mode == "incomplete" {
            return;
        }
        emit(
            json!({"protocol":5,"id":"discover","event":"complete","count":records.len(),"complete":mode!="partial","limitations":limitations}),
        );
    }
    if mode == "extra" {
        println!("{{}}");
    }
    if mode == "complete-hang" {
        hang();
    }
    assert!(lines.next().is_none());
}
