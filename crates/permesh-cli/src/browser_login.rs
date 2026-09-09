// SPDX-License-Identifier: MIT
//! Explicit browser login is the only caller permitted to persist OAuth results.
use crate::{
    blocking::BlockingPool,
    browser_flow::{FlowError, Pending},
    cancellation::Cancellation,
    error::AppError,
    external_workspace,
    report::Outcome,
};
use permesh_config::Config;
use permesh_provider_sdk::browser_auth::BrowserAuthSpec;
use permesh_secrets::{Secret, SecretRef, SecretResolver};
use std::{path::Path, time::Duration};
fn failure(error: FlowError) -> AppError {
    AppError::new(
        if error == FlowError::Cancelled {
            130
        } else {
            3
        },
        error.message(),
    )
}
struct Settings {
    client_id: String,
    client_secret: Option<SecretRef>,
    output: SecretRef,
}
fn settings(config: &Config, id: &str, spec: &BrowserAuthSpec) -> Result<Settings, AppError> {
    spec.validate()
        .map_err(|_| failure(FlowError::Configuration))?;
    let external = config
        .providers
        .iter()
        .find(|p| p.id == id)
        .and_then(|p| p.external.as_ref())
        .ok_or_else(|| failure(FlowError::Configuration))?;
    if spec
        .when
        .as_ref()
        .is_some_and(|when| external.configuration.get(&when.field) != Some(&when.equals))
    {
        return Err(AppError::input(
            "Browser login is unavailable for this authentication mode; configure the provider's browser-compatible mode and approve the updated workspace.",
        ));
    }
    let client_id=external.configuration.get(&spec.client_id_field).and_then(|v|v.as_str()).ok_or_else(||AppError::input("Configure the OAuth client ID required by this provider, then approve the updated workspace."))?.to_string();
    let output = external
        .credentials
        .get(&spec.refresh_token_slot)
        .and_then(|r| SecretRef::parse(r).ok())
        .ok_or_else(|| {
            AppError::input(
                "Configure the browser refresh credential as a same-instance keychain reference.",
            )
        })?;
    if !matches!(&output,SecretRef::Keychain{service,account} if service==id && account==&spec.refresh_token_slot)
    {
        return Err(AppError::input(
            "Browser login requires keychain://INSTANCE/SLOT matching the declared refresh credential slot.",
        ));
    }
    let client_secret = spec
        .client_secret_slot
        .as_ref()
        .map(|slot| {
            external
                .credentials
                .get(slot)
                .and_then(|r| SecretRef::parse(r).ok())
                .ok_or_else(|| {
                    AppError::input(
                        "Configure the OAuth client secret reference required by this provider.",
                    )
                })
        })
        .transpose()?;
    if client_secret.as_ref() == Some(&output) {
        return Err(AppError::input(
            "OAuth client secret and refresh token must use distinct credential references.",
        ));
    }
    Ok(Settings {
        client_id,
        client_secret,
        output,
    })
}
/// This synchronous commit is the cancellation linearization point. Never queue
/// credential writes on workers which can outlive a cancelled login.
fn commit(
    reference: &SecretRef,
    secret: &Secret,
    cancel: &Cancellation,
    write: impl FnOnce(&SecretRef, &Secret) -> Result<(), permesh_secrets::Error>,
) -> Result<(), AppError> {
    if cancel.is_cancelled() {
        return Err(failure(FlowError::Cancelled));
    }
    write(reference,secret).map_err(|_|AppError::new(3,"Cannot store browser credential in the native keychain. Check OS credential storage availability; no token details are available."))
}
pub(crate) async fn run(
    config: &Config,
    path: &Path,
    id: &str,
    no_open: bool,
    blocking: &BlockingPool,
    cancel: &Cancellation,
) -> Result<Outcome, AppError> {
    if config
        .providers
        .iter()
        .find(|provider| provider.id == id)
        .and_then(|provider| provider.external.as_ref())
        .is_some_and(|external| external.network.is_some())
    {
        return Err(AppError::input(
            "Browser login does not support explicit network settings; use an access-token or refresh-token authentication mode with an independently obtained credential",
        ));
    }
    let owned = config.clone();
    let file = path.to_owned();
    let instance = id.to_owned();
    let authorized = blocking.run(move || external_workspace::authorize(&owned, &file, &instance));
    let (executable, registration) = tokio::select! {biased;()=cancel.cancelled()=>return Err(failure(FlowError::Cancelled)),result=authorized=>result??};
    let spec=permesh_provider_external::host::describe_auth(&executable,&registration.id,id,&registration.capabilities,cancel.cancelled()).await.map_err(|_|if cancel.is_cancelled(){failure(FlowError::Cancelled)}else{AppError::input("This approved provider did not return a valid browser authentication declaration. Check provider browser-login support.")})?;
    let settings = settings(config, id, &spec)?;
    let secret = if let Some(reference) = settings.client_secret {
        let resolve = blocking.run(move || SecretResolver.resolve(&reference));
        Some(
            tokio::select! {biased;()=cancel.cancelled()=>return Err(failure(FlowError::Cancelled)),result=tokio::time::timeout(Duration::from_secs(30),resolve)=>result.map_err(|_|failure(FlowError::Timeout))??.map_err(|_|AppError::new(3,"OAuth client secret unavailable; set its configured credential reference before browser login."))?},
        )
    } else {
        None
    };
    let pending = Pending::begin(spec, settings.client_id)
        .await
        .map_err(failure)?;
    pending.validate_secret(secret.as_ref()).map_err(failure)?;
    if cancel.is_cancelled() {
        return Err(failure(FlowError::Cancelled));
    }
    if no_open {
        eprintln!(
            "Open this authorization URL in your browser:\n{}",
            pending.url()
        );
    } else {
        crate::browser_flow::open(pending.url(), cancel)
            .await
            .map_err(failure)?;
    }
    let refresh = pending
        .finish(secret.as_ref(), cancel)
        .await
        .map_err(failure)?;
    let expected = serde_json::to_value(config).map_err(|_| failure(FlowError::Configuration))?;
    let file = path.to_owned();
    let instance = id.to_owned();
    let verify = blocking.run(move || {
        let current = Config::load(&file).map_err(|_| {
            AppError::input("Workspace changed during browser login; no credential was stored.")
        })?;
        if serde_json::to_value(&current).map_err(|_| failure(FlowError::Configuration))?
            != expected
        {
            return Err(AppError::input(
                "Workspace changed during browser login; no credential was stored.",
            ));
        }
        external_workspace::authorize(&current, &file, &instance).map(|_| ())
    });
    tokio::select! {biased;()=cancel.cancelled()=>return Err(failure(FlowError::Cancelled)),result=verify=>result??};
    commit(&settings.output, &refresh, cancel, permesh_secrets::store)?;
    Outcome::new(
        "auth_login",
        serde_json::json!({"message":"Stored browser refresh credential in the native keychain. Run permesh doctor to verify provider authentication and visibility."}),
    )
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn explicit_network_rejects_browser_login_before_provider_or_secret_access() {
        let config: Config = serde_json::from_value(serde_json::json!({"version":1,"organization":{"name":"test"},"providers":[{"id":"instance","type":"external","external":{"provider":"fixture","sha256":"a".repeat(64),"discovery_protocol":"negotiated_v1","network":{"https_proxy":"http://proxy.example:8080"},"credentials":{"client_secret":"env://PERMESH_BROWSER_NETWORK_UNAVAILABLE"}}}]})).unwrap();
        let error = run(
            &config,
            Path::new("missing-workspace"),
            "instance",
            true,
            &BlockingPool::new(),
            &Cancellation::new(),
        )
        .await
        .err()
        .unwrap();
        assert!(
            error
                .message
                .contains("Browser login does not support explicit network")
        );
        assert!(error.message.contains("access-token or refresh-token"));
    }
    #[test]
    fn output_destination_and_auth_mode_are_bound_to_instance_and_declaration() {
        let mut config:Config=serde_json::from_value(serde_json::json!({"version":1,"organization":{"name":"test"},"providers":[{"id":"instance","type":"external","external":{"provider":"fixture","sha256":"a".repeat(64),"configuration":{"client_id":"client","auth_mode":"refresh_token"},"credentials":{"refresh_token":"keychain://instance/refresh_token"}}}]})).unwrap();
        let mut spec:BrowserAuthSpec=serde_json::from_value(serde_json::json!({"schema_version":1,"authorization_endpoint":"https://id.example.test/authorize","token_endpoint":"https://id.example.test/token","scopes":["read"],"client_id_field":"client_id","refresh_token_slot":"refresh_token","when":{"field":"auth_mode","equals":"refresh_token"},"authorization_parameters":{}})).unwrap();
        assert!(settings(&config, "instance", &spec).is_ok());
        spec.client_secret_slot = Some("client_secret".into());
        config.providers[0]
            .external
            .as_mut()
            .unwrap()
            .credentials
            .insert(
                "client_secret".into(),
                "keychain://instance/refresh_token".into(),
            );
        assert!(settings(&config, "instance", &spec).is_err());
        spec.client_secret_slot = None;

        for reference in [
            "keychain://other/refresh_token",
            "keychain://instance/token",
            "env://REFRESH",
        ] {
            config.providers[0]
                .external
                .as_mut()
                .unwrap()
                .credentials
                .insert("refresh_token".into(), reference.into());
            assert!(settings(&config, "instance", &spec).is_err());
        }
        config.providers[0]
            .external
            .as_mut()
            .unwrap()
            .credentials
            .insert(
                "refresh_token".into(),
                "keychain://instance/refresh_token".into(),
            );
        config.providers[0]
            .external
            .as_mut()
            .unwrap()
            .configuration
            .insert("auth_mode".into(), serde_json::json!("access_token"));
        assert!(settings(&config, "instance", &spec).is_err());
    }
    #[test]
    fn cancelled_commit_does_not_write_and_write_errors_are_redacted() {
        let reference = SecretRef::parse("keychain://instance/refresh_token").unwrap();
        let secret = Secret::new("NEVER_ECHO_TOKEN".into());
        let cancel = Cancellation::new();
        cancel.cancel();
        let result = commit(&reference, &secret, &cancel, |_, _| {
            panic!("cancelled write")
        });
        assert_eq!(result.unwrap_err().code, 130);
        let result = commit(&reference, &secret, &Cancellation::new(), |_, _| {
            Err(permesh_secrets::Error::KeychainUnavailable)
        });
        assert!(!result.unwrap_err().message.contains(secret.expose()));
    }
}
