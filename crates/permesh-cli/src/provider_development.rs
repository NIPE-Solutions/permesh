// SPDX-License-Identifier: MIT
//! Explicit source generation and offline transcript validation; never executes a provider.
use crate::{
    args::DiscoveryProtocol, blocking::BlockingPool, cancellation::Cancellation, error::AppError,
    report::Outcome,
};
use clap::{Subcommand, ValueEnum};
use permesh_provider_protocol as protocol;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
#[path = "provider_development_scaffold.rs"]
mod template;

#[derive(Clone, Copy, ValueEnum)]
pub enum Operation {
    Discover,
    Check,
    Describe,
}
#[derive(Clone, Subcommand)]
pub enum DevelopmentCommand {
    /// Generate a native crate in an explicitly selected official-provider workspace; never build or execute it.
    Scaffold {
        providers_root: PathBuf,
        #[arg(long)]
        id: String,
    },
    /// Validate a saved response transcript offline; this does not qualify or trust an executable.
    Validate {
        transcript: PathBuf,
        #[arg(long)]
        provider: String,
        #[arg(long)]
        instance: String,
        #[arg(long, value_enum)]
        operation: Operation,
        #[arg(long, value_enum, default_value_t=DiscoveryProtocol::NegotiatedV1)]
        discovery_protocol: DiscoveryProtocol,
    },
}
fn cancelled(cancel: &Cancellation) -> Result<(), AppError> {
    if cancel.is_cancelled() {
        Err(AppError::new(130, "Cancelled"))
    } else {
        Ok(())
    }
}
pub async fn run(
    command: &DevelopmentCommand,
    blocking: &BlockingPool,
    cancellation: &Cancellation,
) -> Result<Outcome, AppError> {
    cancelled(cancellation)?;
    let command = command.clone();
    let cancel = cancellation.clone();
    let value = blocking
        .run(move || match command {
            DevelopmentCommand::Scaffold { providers_root, id } => {
                scaffold(&providers_root, &id, &cancel)
            }
            DevelopmentCommand::Validate {
                transcript,
                provider,
                instance,
                operation,
                discovery_protocol,
            } => {
                cancelled(&cancel)?;
                let metadata = fs::symlink_metadata(&transcript)
                    .map_err(|_| AppError::input("Cannot read transcript file"))?;
                if !metadata.is_file()
                    || redirected(&metadata)
                    || metadata.len() > protocol::MAX_TRANSCRIPT_BYTES as u64
                {
                    return Err(AppError::input(
                        "Transcript must be a regular file within the protocol byte limit",
                    ));
                }
                let mut bytes = Vec::new();
                let file = fs::File::open(transcript)
                    .map_err(|_| AppError::input("Cannot read transcript file"))?;
                let opened = file
                    .metadata()
                    .map_err(|_| AppError::input("Cannot inspect opened transcript"))?;
                same_file(&metadata, &opened)?;
                file.take((protocol::MAX_TRANSCRIPT_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)
                    .map_err(|_| AppError::input("Cannot read transcript file"))?;
                validate_bytes(
                    &bytes,
                    &provider,
                    &instance,
                    operation,
                    discovery_protocol,
                    &cancel,
                )
            }
        })
        .await??;
    Outcome::new("provider_development", value)
}
fn same_file(before: &fs::Metadata, opened: &fs::Metadata) -> Result<(), AppError> {
    if !opened.is_file()
        || redirected(opened)
        || opened.len() > protocol::MAX_TRANSCRIPT_BYTES as u64
        || before.len() != opened.len()
    {
        return Err(AppError::input(
            "Transcript changed or exceeds the file limit",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() != opened.dev() || before.ino() != opened.ino() {
            return Err(AppError::input("Transcript changed while opening"));
        }
    }
    Ok(())
}
fn protocol_failure(error: protocol::ProtocolError) -> AppError {
    AppError::input(error.to_string())
}
fn validate_bytes(
    bytes: &[u8],
    provider: &str,
    instance: &str,
    operation: Operation,
    contract: DiscoveryProtocol,
    cancel: &Cancellation,
) -> Result<Value, AppError> {
    cancelled(cancel)?;
    if bytes.len() > protocol::MAX_TRANSCRIPT_BYTES {
        return Err(protocol_failure(protocol::ProtocolError::TranscriptLimit));
    }
    let mut summary = json!({"development_version":1,"valid":true,"executed":false,"qualification":"offline_transcript_only","message":"Transcript schema/order validated offline. This does not establish executable trust, credential safety, API correctness or live tenant qualification."});
    macro_rules! decode {
        ($decoder:expr) => {{
            let mut decoder = $decoder.map_err(protocol_failure)?;
            for frame in bytes.split_inclusive(|b| *b == b'\n') {
                cancelled(cancel)?;
                decoder.push_frame(frame).map_err(protocol_failure)?;
            }
            decoder.finish().map_err(protocol_failure)?
        }};
    }
    match operation {
        Operation::Discover => {
            let snapshot = match contract {
                DiscoveryProtocol::Legacy => decode!(protocol::DiscoveryDecoder::new_versioned(
                    provider, instance, None, 2
                )),
                DiscoveryProtocol::NegotiatedV1 => decode!(
                    protocol::negotiated::DiscoveryDecoder::new(provider, instance, None)
                ),
            };
            summary["operation"] = json!("discover");
            summary["complete"] = json!(snapshot.complete);
            summary["counts"] = json!({"accounts":snapshot.accounts.len(),"identities":snapshot.identities.len(),"groups":snapshot.groups.len(),"resources":snapshot.resources.len(),"memberships":snapshot.memberships.len(),"grants":snapshot.grants.len()});
            summary["limitations"] = json!(snapshot.limitations.len());
        }
        Operation::Check => {
            let health = match contract {
                DiscoveryProtocol::Legacy => {
                    decode!(protocol::HealthDecoder::new(provider, instance, None))
                }
                DiscoveryProtocol::NegotiatedV1 => decode!(
                    protocol::negotiated::HealthDecoder::new(provider, instance, None)
                ),
            };
            summary["operation"] = json!("check");
            summary["limitations"] = json!(health.limitations.len());
        }
        Operation::Describe => {
            if contract != DiscoveryProtocol::Legacy {
                return Err(AppError::input(
                    "Setup descriptions use the legacy contract; select --discovery-protocol legacy",
                ));
            }
            let spec = decode!(protocol::SetupDecoder::new(provider, instance, None));
            summary["operation"] = json!("describe");
            summary["setup_schema_version"] = json!(spec.schema_version);
            summary["steps"] = json!(spec.steps.len());
        }
    }
    summary["discovery_protocol"] = json!(contract.to_string());
    Ok(summary)
}
fn redirected(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}
fn scaffold(root: &Path, id: &str, cancel: &Cancellation) -> Result<Value, AppError> {
    cancelled(cancel)?;
    if id.is_empty()
        || id.len() > 48
        || !id.as_bytes()[0].is_ascii_lowercase()
        || !id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(AppError::input(
            "Provider ID must start with a lowercase letter and contain at most 48 lowercase letters, digits or hyphens",
        ));
    }
    let root = root
        .canonicalize()
        .map_err(|_| AppError::input("Select an existing official-provider workspace root"))?;
    let providers = root.join("providers");
    for (path, directory) in [
        (root.join("Cargo.toml"), false),
        (root.join("crates"), true),
        (root.join("crates/native-runtime"), true),
        (root.join("crates/native-runtime/Cargo.toml"), false),
        (providers.clone(), true),
    ] {
        let metadata=fs::symlink_metadata(path).map_err(|_|AppError::input("Scaffold requires an existing provider workspace with providers/ and crates/native-runtime/Cargo.toml"))?;
        if redirected(&metadata)
            || (directory && !metadata.is_dir())
            || (!directory && !metadata.is_file())
        {
            return Err(AppError::input(
                "Scaffold workspace paths must be regular directories/files, not symlinks",
            ));
        }
    }
    let destination = providers.join(id);
    cancelled(cancel)?;
    fs::create_dir(&destination).map_err(|_| {
        AppError::input("Cannot create provider directory; destination must not exist")
    })?;
    let result = (|| {
        fs::create_dir(destination.join("src"))
            .map_err(|_| AppError::input("Cannot create scaffold source directory"))?;
        for (name, contents) in template::files(id) {
            cancelled(cancel)?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination.join(name))
                .map_err(|_| AppError::input("Cannot create scaffold file"))?;
            file.write_all(contents.as_bytes())
                .and_then(|()| file.sync_all())
                .map_err(|_| AppError::input("Cannot write scaffold file"))?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        if fs::remove_dir_all(&destination).is_err() {
            return Err(AppError::new(
                5,
                "Scaffold generation failed and cleanup could not remove new files; inspect the destination before retrying",
            ));
        }
        return Err(error);
    }
    Ok(
        json!({"development_version":1,"operation":"scaffold","created":destination,"provider":id,"executed":false,"files":["Cargo.toml","README.md","src/main.rs","src/tests.rs"],"message":"Synthetic native provider source created. Workspace membership, build, executable trust and provider qualification remain explicit steps; no code was executed.","next":format!("Review providers/{id}/README.md and add providers/{id} to the selected workspace members before building.")}),
    )
}
#[cfg(test)]
#[path = "provider_development_tests.rs"]
mod tests;

pub fn write(out: &mut impl std::io::Write, result: &Value) -> std::io::Result<()> {
    use crate::output::safe;
    if let Some(path) = result["created"].as_str() {
        writeln!(out, "Created native scaffold: {}", safe(path))?;
    } else {
        writeln!(
            out,
            "Offline {} transcript: valid",
            safe(result["operation"].as_str().unwrap_or_default())
        )?;
        if let Some(complete) = result["complete"].as_bool() {
            writeln!(out, "Reported discovery complete: {complete}")?;
        }
        if let Some(counts) = result["counts"].as_object() {
            for (name, count) in counts {
                writeln!(
                    out,
                    "{}: {}",
                    safe(name),
                    count.as_u64().unwrap_or_default()
                )?;
            }
        }
        if let Some(limitations) = result["limitations"].as_u64() {
            writeln!(out, "Limitations: {limitations}")?;
        }
    }
    Ok(())
}
