// SPDX-License-Identifier: MIT
//! Official provider onboarding composes existing verified distribution and trust boundaries.
use crate::{
    args::{AddProvider, Cli},
    blocking::BlockingPool,
    cancellation::Cancellation,
    error::AppError,
    external::{failure, storage_root},
    external_workspace::WorkspaceAccess,
    report::Outcome,
    setup::{self, SetupArgs, SetupOutcome, TrustedSetup},
};
use permesh_provider_external::{catalog, download, packages::InstalledPackage, trust::Registry};
use std::{future::Future, io::IsTerminal, path::PathBuf};

struct Environment {
    root: PathBuf,
    interactive: bool,
}
fn cancelled(cancellation: &Cancellation) -> Result<(), AppError> {
    if cancellation.is_cancelled() {
        Err(AppError::new(130, "Cancelled"))
    } else {
        Ok(())
    }
}
fn setup_args(args: &AddProvider) -> Result<SetupArgs, AppError> {
    if !matches!(
        args.provider_type.as_str(),
        "github" | "google" | "cloudflare" | "aws"
    ) || !args.organization.is_empty()
        || args.customer_id.is_some()
        || args.token_ref.is_some()
        || args.provider.is_some()
        || args.sha256.is_some()
        || !args.setting.is_empty()
        || !args.credential.is_empty()
    {
        return Err(AppError::input(
            "Official provider add handles official install, trust and setup. Use its declarative form or --answers FILE instead of legacy/external configuration flags.",
        ));
    }
    catalog::validate_request(&args.provider_type, args.version.as_deref())
        .map_err(|_| AppError::input("--version requires an exact stable provider version"))?;
    Ok(SetupArgs {
        provider: args.provider_type.clone(),
        discovery_protocol: crate::args::DiscoveryProtocol::Legacy,
        id: args.id.clone(),
        answers: args.answers.clone(),
        describe: false,
        authoritative: args.authoritative,
    })
}
fn continuation(id: &str, package: &InstalledPackage) -> String {
    format!(
        "permesh provider add {} --id {id} --version {}",
        package.release.provider, package.release.version
    )
}
fn annotate(mut error: AppError, next: &str) -> AppError {
    if error.code != 130 {
        error.message.push_str(&format!(" Completed package/trust steps remain local. Resume with {next}; for noninteractive use supply corrected --answers FILE and --accept-risk."));
    }
    error
}
pub async fn run(
    cli: &Cli,
    args: &AddProvider,
    pool: &BlockingPool,
    cancellation: &Cancellation,
) -> Result<Outcome, AppError> {
    let interactive = !cli.json && std::io::stdin().is_terminal();
    let env = Environment {
        root: storage_root()?,
        interactive,
    };
    let fetch = async {
        crate::distribution::acquire(
            &args.provider_type,
            args.version.as_deref(),
            false,
            false,
            true,
            pool,
            cancellation,
        )
        .await?
        .package
        .ok_or_else(|| AppError::new(5, "Provider package was not installed"))
    };
    let consent_pool = pool.clone();
    let consent_cancel = cancellation.clone();
    let accept = args.accept_risk;
    run_with(cli, args, pool, cancellation, env, fetch, move |review| {
        let pool = consent_pool.clone();
        let cancel = consent_cancel.clone();
        async move {
            if accept { return Ok(true); }
            tokio::select! {
                biased;
                () = cancel.cancelled() => Err(AppError::new(130,"Cancelled")),
                result = pool.run(move || crate::guided_prompt::confirm(&review, &mut std::io::stdin().lock(), &mut std::io::stderr())) => result?,
            }
        }
    }).await
}

async fn run_with<F, C, R>(
    cli: &Cli,
    args: &AddProvider,
    pool: &BlockingPool,
    cancellation: &Cancellation,
    env: Environment,
    fetch: F,
    mut consent: C,
) -> Result<Outcome, AppError>
where
    F: Future<Output = Result<InstalledPackage, AppError>>,
    C: FnMut(String) -> R,
    R: Future<Output = Result<bool, AppError>>,
{
    cancelled(cancellation)?;
    let setup_args = setup_args(args)?;
    if !env.interactive && (args.answers.is_none() || !args.accept_risk) {
        return Err(AppError::input(
            "Noninteractive official provider add requires --answers FILE and --accept-risk. The flag authorizes both native binary trust and the resulting instance's credential delivery; authentication remains separate.",
        ));
    }
    // Retain the captured workspace revision and parsed answers across download and consent.
    let prepared = setup::prepare(cli, &setup_args)?;
    let id = prepared.id.clone();
    let package = tokio::select! {
        biased;
        ()=cancellation.cancelled()=>return Err(AppError::new(130,"Cancelled")),
        result=fetch=>result?,
    };
    cancelled(cancellation)?;
    let release = &package.release;
    release
        .validate()
        .map_err(|_| AppError::new(3, "Invalid official package metadata"))?;
    if release.provider != args.provider_type
        || Some(release.target.as_str()) != catalog::native_target()
        || !release.protocols.contains(&3)
        || args
            .version
            .as_ref()
            .is_some_and(|v| *v != release.version.to_string())
    {
        return Err(AppError::input(
            "Selected official package does not match the requested version/platform or does not support declarative setup",
        ));
    }
    let next = continuation(&id, &package);
    let capabilities = release
        .capabilities
        .iter()
        .map(|capability| match capability {
            permesh_provider_sdk::Capability::Accounts => "accounts",
            permesh_provider_sdk::Capability::Identities => "identities",
            permesh_provider_sdk::Capability::Resources => "resources",
            permesh_provider_sdk::Capability::Groups => "groups",
            permesh_provider_sdk::Capability::Memberships => "group memberships",
            permesh_provider_sdk::Capability::Grants => "access grants",
        })
        .collect::<Vec<_>>()
        .join(", ");
    let mut review = format!(
        "{} provider {}
Source: {}
Reported capabilities: {capabilities}

Package checksums match the public catalog; publisher signatures were not verified.
This code runs as your user with access to files and network; it is not sandboxed.
Trust this provider and run its setup form?",
        release.provider,
        release.version,
        download::CATALOG_URL
    );
    if cli.verbose > 0 {
        review.push_str(&format!(
            "
Platform: {}
Executable: {}
SHA-256: {}",
            release.target,
            crate::output::safe(&package.executable.display().to_string()),
            release.executable_sha256
        ));
    }
    if !consent(review).await? {
        return Outcome::new(
            "provider_add",
            serde_json::json!({"id":id,"approved":false,"configured":false,"credentials_resolved":false,"message":"Package is available locally. Binary trust declined; no provider executed or workspace changed.","next":next}),
        );
    }
    cancelled(cancellation)?;
    // Bounded synchronous publication: no detached task can register code after cancellation returns.
    let trusted = trust_package(env.root.clone(), &package).map_err(|e| annotate(e, &next))?;
    cancelled(cancellation)?;
    let result = setup::execute(&setup_args, prepared, pool, cancellation, trusted)
        .await
        .map_err(|e| annotate(e, &next))?;
    let SetupOutcome::Created(created) = result else {
        return Err(AppError::new(5, "Expected a configured provider instance"));
    };
    let result = finish(created, &package, env.root, cancellation, &mut consent).await;
    result.map_err(|mut error:AppError| {
        if error.code!=130 {error.message.push_str(&format!(" Instance remains configured. Continue with permesh provider external review {id}, then approve the reviewed fingerprint with provider external approve {id} --fingerprint REVIEWED_FINGERPRINT --accept-risk."));}
        error
    })
}

fn trust_package(root: PathBuf, package: &InstalledPackage) -> Result<TrustedSetup, AppError> {
    let release = &package.release;
    let registry = Registry::new(root.clone()).map_err(failure)?;
    let registration = match registry.load_pinned(&release.provider, &release.executable_sha256) {
        Ok(registration) => {
            if registration.capabilities.len() != release.capabilities.len()
                || registration
                    .capabilities
                    .iter()
                    .any(|capability| !release.capabilities.contains(capability))
            {
                return Err(AppError::input(
                    "Existing trusted digest has different capabilities; review its registration explicitly",
                ));
            }
            registry.verify(&registration).map_err(failure)?;
            registration
        }
        Err(_) => registry
            .trust(
                &package.executable,
                &release.provider,
                &release.executable_sha256,
                &release.capabilities,
            )
            .map_err(failure)?,
    };
    let executable = registry.verify(&registration).map_err(failure)?;
    Ok(TrustedSetup {
        root: root.clone(),
        registration,
        executable,
        discovery_protocol: match release.discovery_protocol {
            catalog::DiscoveryProtocol::Legacy => permesh_config::DiscoveryProtocol::Legacy,
            catalog::DiscoveryProtocol::NegotiatedV1 => {
                permesh_config::DiscoveryProtocol::NegotiatedV1
            }
        },
    })
}

async fn finish<C, R>(
    created: crate::setup_workspace::CreatedInstance,
    package: &InstalledPackage,
    root: PathBuf,
    cancellation: &Cancellation,
    consent: &mut C,
) -> Result<Outcome, AppError>
where
    C: FnMut(String) -> R,
    R: Future<Output = Result<bool, AppError>>,
{
    let id = &created.id;
    let release = &package.release;
    let access = WorkspaceAccess { root };
    let (_, _, fingerprint) = access.reviewed(&created.config, &created.path, id)?;
    let instance = created
        .config
        .providers
        .iter()
        .find(|p| p.id == *id)
        .and_then(|p| p.external.as_ref())
        .ok_or_else(|| AppError::new(5, "Created provider instance is missing"))?;
    let review = format!(
        "{} instance {id}\nWorkspace: {}\nSettings: {}\nCredential references: {}\nDiscovery protocol: {}\n\nApprove this instance's settings and named credential delivery to the trusted binary? Credentials have not been resolved or stored. Authentication remains a separate auth login command.",
        release.provider,
        crate::output::safe(&created.path.display().to_string()),
        serde_json::to_string_pretty(&instance.configuration)
            .map_err(|_| AppError::new(5, "Cannot format settings"))?,
        serde_json::to_string_pretty(&instance.credentials)
            .map_err(|_| AppError::new(5, "Cannot format credential references"))?,
        match instance.discovery_protocol {
            permesh_config::DiscoveryProtocol::Legacy => "legacy",
            permesh_config::DiscoveryProtocol::NegotiatedV1 => "negotiated-v1",
        }
    );
    cancelled(cancellation)?;
    let approve = consent(review).await?;
    cancelled(cancellation)?;
    if approve {
        // Consent binds the exact displayed provider context; reject intervening edits.
        let current = permesh_config::Config::load(&created.path)?;
        access.approve(&current, &created.path, id, &fingerprint, true)?;
    }
    Outcome::new(
        "provider_add",
        serde_json::json!({"id":id,"provider":release.provider,"version":release.version.to_string(),"sha256":release.executable_sha256,"file":created.path,"configured":true,"approved":approve,"credentials_resolved":false,"configuration":instance.configuration,"credential_references":instance.credentials,"message":if approve {"Provider instance configured and approved. Authentication remains separate; no credentials were resolved or stored."}else{"Provider instance configured. Workspace execution approval declined; credentials remain unavailable to the provider until explicit approval."},"next":if approve {format!("permesh auth login {id}; permesh doctor")}else{format!("permesh provider external review {id}")}}),
    )
}

#[cfg(test)]
#[path = "guided_add_tests.rs"]
mod tests;
