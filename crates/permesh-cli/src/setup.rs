// SPDX-License-Identifier: MIT
use crate::{
    args::Cli,
    blocking::BlockingPool,
    cancellation::Cancellation,
    error::AppError,
    external::{failure, storage_root},
    report::Outcome,
    setup_workspace::Draft,
};
use clap::Args;
use permesh_provider_external::{host, trust::Registry};
use permesh_provider_sdk::{
    Capability,
    setup::{Input, SetupSpec},
};
use permesh_secrets::SecretRef;
use serde_json::Value;
use std::{collections::BTreeMap, io::IsTerminal, path::PathBuf, time::Duration};
const LOCAL_DEADLINE: Duration = Duration::from_secs(120);
#[derive(Clone, Args)]
pub struct SetupArgs {
    /// Locally trusted provider registration ID.
    pub provider: String,
    #[arg(long, help = "New instance ID; defaults to PROVIDER-main")]
    pub id: Option<String>,
    #[arg(
        long,
        value_name = "FILE",
        conflicts_with = "describe",
        help = "Versioned local YAML/JSON answers; settings and credential references only"
    )]
    pub answers: Option<PathBuf>,
    #[arg(
        long,
        conflicts_with = "authoritative",
        help = "Describe the setup form without reading or changing a workspace"
    )]
    pub describe: bool,
    #[arg(
        long,
        help = "Explicitly use this instance as an authoritative identity source"
    )]
    pub authoritative: bool,
}
fn valid_id(id: &str) -> bool {
    id.len() <= 64
        && id.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn cancelled() -> AppError {
    AppError::new(130, "Cancelled")
}
pub(crate) fn validate_references(
    spec: &SetupSpec,
    answers: &BTreeMap<String, Value>,
    id: &str,
) -> Result<(), AppError> {
    for field in spec
        .steps
        .iter()
        .flat_map(|s| &s.fields)
        .filter(|f| matches!(f.input, Input::Credential))
    {
        if let Some(value) = answers.get(&field.key).filter(|v| !v.is_null()) {
            let reference = value.as_str().and_then(|v| SecretRef::parse(v).ok()).ok_or_else(|| AppError::input("Credential fields accept env://NAME or keychain://INSTANCE/SLOT references only; never enter secret values in setup"))?;
            if let SecretRef::Keychain { service, account } = reference
                && (service != id || account != field.key)
            {
                return Err(AppError::input(
                    "Keychain references must match the new instance ID and credential field name",
                ));
            }
        }
    }
    Ok(())
}
pub async fn run(
    cli: &Cli,
    args: &SetupArgs,
    pool: &BlockingPool,
    cancellation: &Cancellation,
) -> Result<Outcome, AppError> {
    if args.describe && (args.answers.is_some() || args.authoritative) {
        return Err(AppError::input(
            "--describe cannot use --answers or --authoritative",
        ));
    }
    if !args.describe && args.answers.is_none() && (cli.json || !std::io::stdin().is_terminal()) {
        return Err(AppError::input(
            "Noninteractive setup requires --answers FILE. Use --describe --json to inspect the form first.",
        ));
    }
    let id = args
        .id
        .clone()
        .unwrap_or_else(|| format!("{}-main", args.provider));
    if !valid_id(&id) || !valid_id(&args.provider) {
        return Err(AppError::input(
            "Provider and instance IDs require an ASCII letter followed by letters, digits, hyphen or underscore, up to 64 bytes; use --id for a shorter instance ID",
        ));
    }
    let mut answers = if let Some(path) = &args.answers {
        permesh_config::load_setup_answers(path).map_err(|error| {
            let message = match error {
                permesh_config::Error::ParseAt { line, column } => format!("Invalid setup answers at line {line}, column {column}; check duplicate keys, field names and nesting limits in --answers FILE"),
                permesh_config::Error::Validation(message) => format!("Invalid setup answers: {message}"),
                _ => "Cannot read or parse --answers FILE; use version: 1 and an answers mapping of nonsecret values and credential references".into(),
            };
            AppError::input(message)
        })?
    } else {
        BTreeMap::new()
    };
    let draft = if args.describe {
        None
    } else {
        Some(Draft::load(cli, &id)?)
    };
    let registry = Registry::new(storage_root()?).map_err(failure)?;
    let provider = args.provider.clone();
    let (registration, executable) = tokio::select! {
        biased;
        ()=cancellation.cancelled()=>return Err(cancelled()),
        result=tokio::time::timeout(LOCAL_DEADLINE,pool.run(move|| {let registration=registry.load(&provider)?; let path=registry.verify(&registration)?; Ok::<_,permesh_provider_external::ExternalError>((registration,path))}))=>result.map_err(|_|AppError::new(3,"Provider registration verification timed out"))??.map_err(failure)?,
    };
    if args.authoritative && !registration.capabilities.contains(&Capability::Identities) {
        return Err(AppError::input(
            "Authoritative identity sources require the registered identities capability",
        ));
    }
    // Await the supervisor even on cancellation; never drop its cleanup future.
    let spec = host::describe(
        &executable,
        &registration.id,
        &id,
        &registration.capabilities,
        cancellation.cancelled(),
    )
    .await
    .map_err(|e| {
        if matches!(e, permesh_provider_external::ExternalError::Cleanup) {
            AppError::new(5, "External setup process cleanup failed")
        } else {
            failure(e)
        }
    })?;
    if args.describe {
        return Outcome::new(
            "provider_setup_describe",
            serde_json::json!({"provider":registration.id,"id":id,"sha256":registration.sha256,"spec":spec,"message":"Setup form from a trusted native provider. No workspace settings, answers or credentials were sent. No configuration was changed."}),
        );
    }
    spec.questions(&answers)
        .map_err(|e| AppError::input(e.to_string()))?;
    validate_references(&spec, &answers, &id)?;
    if args.answers.is_none() {
        let spec_owned = spec.clone();
        let id_owned = id.clone();
        let cancel = cancellation.clone();
        // Do not hold stderr locked while waiting for input: cancellation must
        // still be able to print its result from the main thread.
        answers = tokio::select! {
            biased;
            ()=cancellation.cancelled()=>return Err(cancelled()),
            result=pool.run(move||crate::setup_prompt::collect(&spec_owned,answers,&id_owned,&cancel,&mut std::io::stdin().lock(),&mut std::io::stderr()))=>result??,
        };
    }
    let values = spec
        .resolve(&answers)
        .map_err(|e| AppError::input(e.to_string()))?;
    validate_references(&spec, &answers, &id)?;
    // Reject a changed registration after a potentially long interactive form.
    let registry = Registry::new(storage_root()?).map_err(failure)?;
    let expected = registration.clone();
    tokio::select! {
        biased;
        ()=cancellation.cancelled()=>return Err(cancelled()),
        result=tokio::time::timeout(LOCAL_DEADLINE,pool.run(move||registry.verify(&expected)))=>{ result.map_err(|_|AppError::new(3,"Provider registration verification timed out"))??.map_err(failure)?; },
    }
    if cancellation.is_cancelled() {
        return Err(cancelled());
    }
    // The final bounded filesystem commit has no asynchronous suspension point.
    // No background prompt/credential worker can write the configuration later.
    draft
        .ok_or_else(|| AppError::new(5, "Setup workspace is missing"))?
        .commit(&id, &registration, values, args.authoritative)
}
