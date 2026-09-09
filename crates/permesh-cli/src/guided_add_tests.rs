// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use super::*;
use clap::Parser;
use permesh_provider_external::{catalog, packages::InstalledPackage, trust};
use std::{fs, future::ready, path::PathBuf, process::Command, sync::OnceLock};

fn native_fixture() -> PathBuf {
    static FIXTURE: OnceLock<(tempfile::TempDir, PathBuf)> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("peer.rs");
        let binary = temp.path().join(if cfg!(windows) { "peer.exe" } else { "peer" });
        fs::write(&source, r##"
use std::io::{BufRead, Write};
fn main() {
    let marker = std::env::current_exe().unwrap().parent().unwrap().join("executed");
    std::fs::write(marker, b"describe only").unwrap();
    let mut input = std::io::stdin().lock().lines();
    let handshake = input.next().unwrap().unwrap();
    assert!(handshake.contains("handshake"));
    assert!(!handshake.contains("credentials"));
    println!(r#"{{"protocol":3,"id":"handshake","event":"handshake","provider":"github","capabilities":["accounts"],"draft":true}}"#);
    std::io::stdout().flush().unwrap();
    let request = input.next().unwrap().unwrap();
    assert!(request.contains("describe"));
    assert!(!request.contains("credentials"));
    println!(r#"{{"protocol":3,"id":"describe","event":"setup","spec":{{"schema_version":1,"title":"GitHub fixture","description":"Synthetic offline setup","steps":[{{"id":"connection","title":"Connection","description":"Settings","fields":[{{"key":"organization","label":"Organization","help":"Synthetic organization","required":true,"default":"acme","input":{{"type":"text","min_length":1,"max_length":128}}}},{{"key":"token","label":"Token reference","help":"Reference only","required":true,"input":{{"type":"credential"}}}}]}}]}}}}"#);
}
"##).unwrap();
        assert!(Command::new("rustc").args(["--edition", "2024"]).arg(&source).arg("-o").arg(&binary).status().unwrap().success());
        (temp, binary.canonicalize().unwrap())
    }).1.clone()
}
struct Sandbox {
    _temp: tempfile::TempDir,
    cli: Cli,
    args: AddProvider,
    root: PathBuf,
    config: PathBuf,
    answers: PathBuf,
    package: InstalledPackage,
}
impl Sandbox {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let config = base.join("permesh.yaml");
        let answers = base.join("answers.yaml");
        fs::write(
            &config,
            "version: 1\norganization: {name: Test}\nproviders: []\n",
        )
        .unwrap();
        fs::write(
            &answers,
            "version: 1\nanswers: {token: 'env://PERMESH_GUIDED_FIXTURE_UNAVAILABLE_TOKEN'}\n",
        )
        .unwrap();
        let cli = Cli::try_parse_from([
            "permesh",
            "--config",
            config.to_str().unwrap(),
            "provider",
            "add",
            "github",
            "--answers",
            answers.to_str().unwrap(),
        ])
        .unwrap();
        let crate::args::Command::Provider {
            command: crate::args::ProviderCommand::Add(args),
        } = &cli.command
        else {
            panic!()
        };
        let args = (**args).clone();
        let executable = base.join(if cfg!(windows) {
            "package-provider.exe"
        } else {
            "package-provider"
        });
        fs::copy(native_fixture(), &executable).unwrap();
        let digest = trust::inspect(&executable).unwrap().sha256;
        let release = serde_json::from_value(serde_json::json!({"provider":"github","version":"1.0.0","target":catalog::native_target().unwrap(),"capabilities":["accounts"],"protocols":[2,3],"archive_sha256":"a".repeat(64),"executable_sha256":digest,"archive_size":128})).unwrap();
        Self {
            _temp: temp,
            cli,
            args,
            root: base.join("state/providers"),
            config,
            answers,
            package: InstalledPackage {
                release,
                executable,
            },
        }
    }
    async fn run(&self, decisions: Vec<bool>) -> Result<Outcome, AppError> {
        let mut decisions = decisions.into_iter();
        run_with(
            &self.cli,
            &self.args,
            &BlockingPool::new(),
            &Cancellation::new(),
            Environment {
                root: self.root.clone(),
                interactive: true,
            },
            ready(Ok(self.package.clone())),
            move |_| ready(Ok(decisions.next().unwrap())),
        )
        .await
    }
}
#[tokio::test]
async fn declining_binary_trust_never_registers_executes_or_changes_workspace() {
    let sandbox = Sandbox::new();
    let before = fs::read(&sandbox.config).unwrap();
    let result = sandbox.run(vec![false]).await.unwrap();
    assert_eq!(result.report.result["approved"], false);
    assert!(!sandbox.root.exists());
    assert!(
        !sandbox
            .package
            .executable
            .parent()
            .unwrap()
            .join("executed")
            .exists()
    );
    assert_eq!(fs::read(&sandbox.config).unwrap(), before);
}
#[tokio::test]
async fn existing_instance_fails_before_polling_package_fetch() {
    let sandbox = Sandbox::new();
    fs::write(
        &sandbox.config,
        "version: 1\norganization: {name: Test}\nproviders: [{id: github-main, type: demo}]\n",
    )
    .unwrap();
    let result = run_with(
        &sandbox.cli,
        &sandbox.args,
        &BlockingPool::new(),
        &Cancellation::new(),
        Environment {
            root: sandbox.root.clone(),
            interactive: true,
        },
        async { panic!("fetch polled before workspace validation") },
        |_| ready(Ok(true)),
    )
    .await;
    assert!(result.is_err_and(|e| e.message.contains("already exists")));
    assert!(!sandbox.root.exists());
}
#[tokio::test]
async fn setup_failure_preserves_trust_and_can_resume_with_corrected_answers() {
    let sandbox = Sandbox::new();
    let before = fs::read(&sandbox.config).unwrap();
    fs::write(&sandbox.answers, "version: 1\nanswers: {}\n").unwrap();
    let error = sandbox.run(vec![true]).await.err().unwrap();
    assert!(error.message.contains("provider add github"));
    assert_eq!(fs::read(&sandbox.config).unwrap(), before);
    assert!(sandbox.root.join("github/executed").exists());
    trust::Registry::new(sandbox.root.clone())
        .unwrap()
        .load("github")
        .unwrap();
    fs::write(
        &sandbox.answers,
        "version: 1\nanswers: {token: 'env://PERMESH_GUIDED_FIXTURE_UNAVAILABLE_TOKEN'}\n",
    )
    .unwrap();
    let result = sandbox.run(vec![true, true]).await.unwrap();
    assert_eq!(result.report.result["approved"], true);
}
#[tokio::test]
async fn workspace_approval_is_explicit_and_does_not_resolve_credentials() {
    let sandbox = Sandbox::new();
    let result = sandbox.run(vec![true, false]).await.unwrap();
    assert_eq!(result.report.result["approved"], false);
    let config = permesh_config::Config::load(&sandbox.config).unwrap();
    assert_eq!(config.providers.len(), 1);
    assert!(
        !sandbox
            .root
            .parent()
            .unwrap()
            .join("workspace-approvals")
            .exists()
    );
    let sandbox = Sandbox::new();
    let result = sandbox.run(vec![true, true]).await.unwrap();
    assert_eq!(result.report.result["approved"], true);
    assert_eq!(result.report.result["credentials_resolved"], false);
    assert!(
        sandbox
            .root
            .parent()
            .unwrap()
            .join("workspace-approvals")
            .exists()
    );
}

#[tokio::test]
async fn precancelled_onboarding_never_polls_fetch_or_creates_state() {
    let sandbox = Sandbox::new();
    let cancel = Cancellation::new();
    cancel.cancel();
    let error = run_with(
        &sandbox.cli,
        &sandbox.args,
        &BlockingPool::new(),
        &cancel,
        Environment {
            root: sandbox.root.clone(),
            interactive: true,
        },
        async { panic!("fetch after cancellation") },
        |_| ready(Ok(true)),
    )
    .await
    .err()
    .unwrap();
    assert_eq!(error.code, 130);
    assert!(!sandbox.root.exists());
}
#[tokio::test]
async fn workspace_changes_during_either_consent_are_preserved_without_approval() {
    for change_at in [0, 1] {
        let sandbox = Sandbox::new();
        let mut consent_count = 0;
        let error = run_with(
            &sandbox.cli,
            &sandbox.args,
            &BlockingPool::new(),
            &Cancellation::new(),
            Environment {
                root: sandbox.root.clone(),
                interactive: true,
            },
            ready(Ok(sandbox.package.clone())),
            |_| {
                if consent_count == change_at {
                    if change_at == 0 {
                        fs::write(
                            &sandbox.config,
                            "version: 1\norganization: {name: Changed}\nproviders: []\n",
                        )
                        .unwrap();
                    } else {
                        let text = fs::read_to_string(&sandbox.config).unwrap();
                        assert!(text.contains("acme"));
                        fs::write(&sandbox.config, text.replace("acme", "other-tenant")).unwrap();
                    }
                }
                consent_count += 1;
                ready(Ok(true))
            },
        )
        .await
        .err()
        .unwrap();
        assert_eq!(error.code, 2);
        let text = fs::read_to_string(&sandbox.config).unwrap();
        assert!(text.contains(if change_at == 0 {
            "Changed"
        } else {
            "other-tenant"
        }));
        assert!(
            !sandbox
                .root
                .parent()
                .unwrap()
                .join("workspace-approvals")
                .exists()
        );
    }
}
#[tokio::test]
async fn exact_package_pin_cannot_be_replaced_by_selected_registration() {
    let sandbox = Sandbox::new();
    let registry = trust::Registry::new(sandbox.root.clone()).unwrap();
    // Use a small native provider fixture: the full test executable can exceed
    // the registry's binary-size limit with CI debug information enabled.
    let source = sandbox.config.with_file_name("other.rs");
    let other = sandbox
        .config
        .with_file_name(if cfg!(windows) { "other.exe" } else { "other" });
    fs::write(&source, "fn main() {}\n").unwrap();
    assert!(
        Command::new("rustc")
            .arg(&source)
            .arg("-o")
            .arg(&other)
            .status()
            .unwrap()
            .success()
    );
    let other_hash = trust::inspect(&other).unwrap().sha256;
    registry
        .trust(
            &other,
            "github",
            &other_hash,
            &[permesh_provider_sdk::Capability::Accounts],
        )
        .unwrap();
    assert_ne!(other_hash, sandbox.package.release.executable_sha256);
    sandbox.run(vec![true, true]).await.unwrap();
    let config = permesh_config::Config::load(&sandbox.config).unwrap();
    assert_eq!(
        config.providers[0].external.as_ref().unwrap().sha256,
        sandbox.package.release.executable_sha256
    );
    assert!(registry.load_pinned("github", &other_hash).is_ok());
}
#[tokio::test]
async fn review_displays_actual_package_and_instance_credential_references() {
    let mut sandbox = Sandbox::new();
    sandbox.cli.verbose = 1;
    let mut reviews = Vec::new();
    run_with(
        &sandbox.cli,
        &sandbox.args,
        &BlockingPool::new(),
        &Cancellation::new(),
        Environment {
            root: sandbox.root.clone(),
            interactive: true,
        },
        ready(Ok(sandbox.package.clone())),
        |review| {
            reviews.push(review);
            ready(Ok(true))
        },
    )
    .await
    .unwrap();
    assert_eq!(reviews.len(), 2);
    assert!(reviews[0].contains(&sandbox.package.release.executable_sha256));
    assert!(reviews[0].contains(download::CATALOG_URL));
    assert!(reviews[0].contains("accounts"));
    assert!(reviews[1].contains("acme"));
    assert!(reviews[1].contains("env://PERMESH_GUIDED_FIXTURE_UNAVAILABLE_TOKEN"));
}

#[tokio::test]
async fn cancelling_either_consent_cannot_publish_later_trust_or_approval() {
    for cancel_at in [0, 1] {
        let sandbox = Sandbox::new();
        let cancel = Cancellation::new();
        let mut count = 0;
        let error = run_with(
            &sandbox.cli,
            &sandbox.args,
            &BlockingPool::new(),
            &cancel,
            Environment {
                root: sandbox.root.clone(),
                interactive: true,
            },
            ready(Ok(sandbox.package.clone())),
            |_| {
                if count == cancel_at {
                    cancel.cancel();
                }
                count += 1;
                ready(Ok(true))
            },
        )
        .await
        .err()
        .unwrap();
        assert_eq!(error.code, 130);
        assert!(
            !sandbox
                .root
                .parent()
                .unwrap()
                .join("workspace-approvals")
                .exists()
        );
        if cancel_at == 0 {
            assert!(!sandbox.root.exists());
            assert!(
                !sandbox
                    .package
                    .executable
                    .parent()
                    .unwrap()
                    .join("executed")
                    .exists()
            );
        } else {
            assert_eq!(
                permesh_config::Config::load(&sandbox.config)
                    .unwrap()
                    .providers
                    .len(),
                1
            );
        }
    }
}

#[tokio::test]
async fn invalid_flags_config_and_answer_envelopes_fail_before_fetch() {
    for case in ["version", "id", "flags", "config", "answers"] {
        let mut sandbox = Sandbox::new();
        match case {
            "version" => sandbox.args.version = Some("SENTINEL_INVALID_VERSION".into()),
            "id" => sandbox.args.id = Some("SENTINEL/ID".into()),
            "flags" => sandbox.args.organization.push("ignored".into()),
            "config" => fs::write(
                &sandbox.config,
                "version: 99\norganization: {name: Test}\nproviders: []\n",
            )
            .unwrap(),
            "answers" => fs::write(
                &sandbox.answers,
                "version: 1\nanswers: {token: one, token: SENTINEL}\n",
            )
            .unwrap(),
            _ => unreachable!(),
        }
        let error = run_with(
            &sandbox.cli,
            &sandbox.args,
            &BlockingPool::new(),
            &Cancellation::new(),
            Environment {
                root: sandbox.root.clone(),
                interactive: true,
            },
            async { panic!("invalid input fetched package") },
            |_| ready(Ok(true)),
        )
        .await
        .err()
        .unwrap();
        assert_eq!(error.code, 2);
        assert!(!error.message.contains("SENTINEL"));
        assert!(!sandbox.root.exists());
    }
}
#[tokio::test]
async fn wrong_exact_version_is_rejected_before_trust_or_execution() {
    let mut sandbox = Sandbox::new();
    sandbox.args.version = Some("2.0.0".into());
    let error = run_with(
        &sandbox.cli,
        &sandbox.args,
        &BlockingPool::new(),
        &Cancellation::new(),
        Environment {
            root: sandbox.root.clone(),
            interactive: true,
        },
        ready(Ok(sandbox.package.clone())),
        |_| {
            panic!("mismatched package reached consent");
            #[allow(unreachable_code)]
            ready(Ok(true))
        },
    )
    .await
    .err()
    .unwrap();
    assert_eq!(error.code, 2);
    assert!(!sandbox.root.exists());
    assert!(
        !sandbox
            .package
            .executable
            .parent()
            .unwrap()
            .join("executed")
            .exists()
    );
}

#[tokio::test]
async fn default_trust_review_explains_scope_without_manual_hash_or_path_steps() {
    let sandbox = Sandbox::new();
    let mut reviews = Vec::new();
    run_with(
        &sandbox.cli,
        &sandbox.args,
        &BlockingPool::new(),
        &Cancellation::new(),
        Environment {
            root: sandbox.root.clone(),
            interactive: true,
        },
        ready(Ok(sandbox.package.clone())),
        |review| {
            reviews.push(review);
            ready(Ok(false))
        },
    )
    .await
    .unwrap();
    let review = &reviews[0];
    assert!(review.contains("github provider 1.0.0"));
    assert!(review.contains("Reported capabilities: accounts"));
    assert!(review.contains("publisher signatures were not verified"));
    assert!(review.contains("not sandboxed"));
    assert!(!review.contains(&sandbox.package.release.executable_sha256));
    assert!(!review.contains(sandbox.package.executable.to_str().unwrap()));
}

#[tokio::test]
async fn official_setup_persists_verified_contract_despite_contradictory_selector() {
    for negotiated in [false, true] {
        let mut sandbox = Sandbox::new();
        sandbox.args.discovery_protocol = if negotiated {
            crate::args::DiscoveryProtocol::Legacy
        } else {
            crate::args::DiscoveryProtocol::NegotiatedV1
        };
        let expected = if negotiated {
            sandbox.package.release.discovery_protocol = catalog::DiscoveryProtocol::NegotiatedV1;
            sandbox.package.release.protocols = vec![3];
            permesh_config::DiscoveryProtocol::NegotiatedV1
        } else {
            permesh_config::DiscoveryProtocol::Legacy
        };
        sandbox.run(vec![true, false]).await.unwrap();
        let config = permesh_config::Config::load(&sandbox.config).unwrap();
        assert_eq!(
            config.providers[0]
                .external
                .as_ref()
                .unwrap()
                .discovery_protocol,
            expected
        );
        assert_eq!(
            fs::read_to_string(&sandbox.config)
                .unwrap()
                .contains("discovery_protocol:"),
            negotiated
        );
        let access = WorkspaceAccess {
            root: sandbox.root.clone(),
        };
        assert_missing_approval(&sandbox, &config, &access);
    }
}

#[tokio::test]
async fn official_contract_conflicts_and_provider_mismatches_fail_before_trust() {
    for mismatch in ["protocols", "provider"] {
        let mut sandbox = Sandbox::new();
        if mismatch == "protocols" {
            sandbox.package.release.discovery_protocol = catalog::DiscoveryProtocol::NegotiatedV1;
        } else {
            sandbox.args.provider_type = "google".into();
        }
        let before = fs::read(&sandbox.config).unwrap();
        assert!(sandbox.run(vec![]).await.is_err());
        assert!(!sandbox.root.exists());
        assert_eq!(fs::read(&sandbox.config).unwrap(), before);
    }
}

#[tokio::test]
async fn standalone_setup_persists_explicit_contract_and_requires_separate_approval() {
    for selector in [
        crate::args::DiscoveryProtocol::Legacy,
        crate::args::DiscoveryProtocol::NegotiatedV1,
    ] {
        let sandbox = Sandbox::new();
        let mut args = setup_args(&sandbox.args).unwrap();
        args.discovery_protocol = selector;
        let prepared = setup::prepare(&sandbox.cli, &args).unwrap();
        let mut trusted = trust_package(sandbox.root.clone(), &sandbox.package).unwrap();
        // Standalone setup takes the explicit selector after verifying local registration.
        trusted.discovery_protocol = args.discovery_protocol.into();
        setup::execute(
            &args,
            prepared,
            &BlockingPool::new(),
            &Cancellation::new(),
            trusted,
        )
        .await
        .unwrap();
        let config = permesh_config::Config::load(&sandbox.config).unwrap();
        assert_eq!(
            config.providers[0]
                .external
                .as_ref()
                .unwrap()
                .discovery_protocol,
            selector.into()
        );
        let access = WorkspaceAccess {
            root: sandbox.root.clone(),
        };
        assert_missing_approval(&sandbox, &config, &access);
    }
}

fn assert_missing_approval(
    sandbox: &Sandbox,
    config: &permesh_config::Config,
    access: &WorkspaceAccess,
) {
    let (_, _, fingerprint) = access
        .reviewed(config, &sandbox.config, &config.providers[0].id)
        .unwrap();
    let approvals = permesh_provider_external::approvals::ApprovalStore::new(
        sandbox.root.parent().unwrap().join("workspace-approvals"),
    )
    .unwrap();
    assert!(
        approvals
            .verify(&sandbox.config, &config.providers[0].id, &fingerprint)
            .is_err()
    );
}
