// SPDX-License-Identifier: MIT OR Apache-2.0
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
