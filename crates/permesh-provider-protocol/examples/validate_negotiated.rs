// SPDX-License-Identifier: MIT
//! Developer-only offline protocol-1 validator. Does not execute or trust providers.
use permesh_provider_protocol::{
    MAX_FRAME_BYTES, ProtocolError,
    negotiated::{DiscoveryDecoder, HealthDecoder, PROTOCOL_VERSION},
};
use std::{
    io::{self, BufRead, Read, Write},
    process::ExitCode,
};

fn validate(
    provider: &str,
    instance: &str,
    operation: &str,
) -> Result<serde_json::Value, ProtocolError> {
    // No registry is present in this offline tool. The process host always supplies
    // Some(exact registered capabilities); offline success does not grant trust.
    let mut discovery = DiscoveryDecoder::new(provider, instance, None)?;
    let mut health = HealthDecoder::new(provider, instance, None)?;
    if !matches!(operation, "discover" | "check") {
        return Err(ProtocolError::Schema);
    }
    let mut input = io::stdin().lock();
    loop {
        let mut frame = Vec::new();
        input
            .by_ref()
            .take((MAX_FRAME_BYTES + 1) as u64)
            .read_until(b'\n', &mut frame)
            .map_err(|_| ProtocolError::Io)?;
        if frame.is_empty() {
            break;
        }
        if operation == "discover" {
            discovery.push_frame(&frame)?;
        } else {
            health.push_frame(&frame)?;
        }
    }
    let mut summary =
        serde_json::json!({"protocol_version":PROTOCOL_VERSION,"operation":operation,"valid":true});
    if operation == "discover" {
        let snapshot = discovery.finish()?;
        summary["complete"] = serde_json::json!(snapshot.complete);
        summary["counts"] = serde_json::json!({
            "identities":snapshot.identities.len(),"accounts":snapshot.accounts.len(),
            "resources":snapshot.resources.len(),"groups":snapshot.groups.len(),
            "memberships":snapshot.memberships.len(),"grants":snapshot.grants.len()
        });
    } else {
        summary["limitations"] = serde_json::json!(health.finish()?.limitations.len());
    }
    Ok(summary)
}
fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [provider, instance, operation] = args.as_slice() else {
        return error(
            "Usage: validate_negotiated <provider> <instance> <discover|check>",
            2,
        );
    };
    let (Some(provider), Some(instance), Some(operation)) =
        (provider.to_str(), instance.to_str(), operation.to_str())
    else {
        return error("Arguments must be ASCII identifiers", 2);
    };
    let summary = match validate(provider, instance, operation) {
        Ok(summary) => summary,
        Err(failure) => return error(&failure.to_string(), 2),
    };
    let mut bytes = match serde_json::to_vec(&summary) {
        Ok(bytes) => bytes,
        Err(_) => return error("Cannot encode validation summary", 5),
    };
    bytes.push(b'\n');
    let mut output = io::stdout().lock();
    match output.write_all(&bytes).and_then(|()| output.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(_) => error("Cannot write validation summary", 5),
    }
}
fn error(message: &str, code: u8) -> ExitCode {
    if writeln!(io::stderr().lock(), "{message}").is_err() {
        return ExitCode::from(5);
    }
    ExitCode::from(code)
}
