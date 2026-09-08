// SPDX-License-Identifier: MIT
//! Developer-only offline validator. Input is consumed through EOF; no process supervision.
use permesh_provider_protocol::{PROTOCOL_VERSION, validate_discovery};
use std::{
    io::{self, Write},
    process::ExitCode,
};

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [provider, instance] = args.as_slice() else {
        return error(
            "Usage: validate <provider-type> <instance-id> < transcript.ndjson",
            2,
        );
    };
    let (Some(provider), Some(instance)) = (provider.to_str(), instance.to_str()) else {
        return error("Provider type and instance ID must be ASCII identifiers", 2);
    };
    let snapshot = match validate_discovery(io::stdin().lock(), provider, instance) {
        Ok(snapshot) => snapshot,
        Err(failure) => return error(&failure.to_string(), 2),
    };
    // Report counts only. Do not echo record contents or persist the snapshot.
    let summary = serde_json::json!({"protocol_version":PROTOCOL_VERSION,"valid":true,"complete":snapshot.complete,"counts":{
        "identities":snapshot.identities.len(),"accounts":snapshot.accounts.len(),
        "resources":snapshot.resources.len(),"groups":snapshot.groups.len(),
        "memberships":snapshot.memberships.len(),"grants":snapshot.grants.len()
    }});
    let mut output = match serde_json::to_vec(&summary) {
        Ok(output) => output,
        Err(_) => return error("Cannot encode validation summary", 5),
    };
    output.push(b'\n');
    let mut stdout = io::stdout().lock();
    match stdout.write_all(&output).and_then(|()| stdout.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) if failure.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(_) => error("Cannot write validation summary", 5),
    }
}
fn error(message: &str, code: u8) -> ExitCode {
    if writeln!(io::stderr().lock(), "{message}").is_err() {
        return ExitCode::from(5);
    }
    ExitCode::from(code)
}
