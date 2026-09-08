// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::{args::Cli, error::AppError, report::Outcome};
use permesh_config::{Config, ExternalConfig, IdentitySource, ProviderConfig, ProviderKind};
use permesh_provider_external::trust::Registration;
use permesh_provider_sdk::setup::ResolvedSetup;
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub struct Draft {
    path: PathBuf,
    original: Vec<u8>,
    config: Config,
}
fn source(path: &Path) -> Result<Vec<u8>, AppError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| AppError::input("Cannot inspect workspace configuration"))?;
    if !metadata.is_file() {
        return Err(AppError::input(
            "Setup requires a regular configuration file; symlink replacement is not supported",
        ));
    }
    let file =
        File::open(path).map_err(|_| AppError::input("Cannot read workspace configuration"))?;
    if !file
        .metadata()
        .map_err(|_| AppError::input("Cannot inspect workspace configuration"))?
        .is_file()
    {
        return Err(AppError::input(
            "Setup requires a regular configuration file",
        ));
    }
    let mut bytes = Vec::new();
    file.take(permesh_config::MAX_CONFIG_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AppError::input("Cannot read workspace configuration"))?;
    if bytes.len() > permesh_config::MAX_CONFIG_BYTES {
        return Err(AppError::input("Workspace configuration exceeds 1 MiB"));
    }
    Ok(bytes)
}
impl Draft {
    pub fn load(cli: &Cli, id: &str) -> Result<Self, AppError> {
        let path = crate::workspace::path(cli)?;
        let original = source(&path)?;
        let path = path
            .canonicalize()
            .map_err(|_| AppError::input("Cannot locate workspace configuration"))?;
        let config = Config::from_bytes(&original)?;
        if config.providers.iter().any(|p| p.id == id) {
            return Err(AppError::input(
                "Instance already exists; choose another --id or edit its configuration explicitly",
            ));
        }
        Ok(Self {
            path,
            original,
            config,
        })
    }
    pub fn commit(
        mut self,
        id: &str,
        registration: &Registration,
        values: ResolvedSetup,
        authoritative: bool,
    ) -> Result<Outcome, AppError> {
        self.config.providers.push(ProviderConfig {
            id: id.into(),
            kind: ProviderKind::External,
            organizations: vec![],
            customer_id: None,
            auth: None,
            external: Some(ExternalConfig {
                provider: registration.id.clone(),
                sha256: registration.sha256.clone(),
                configuration: values.configuration,
                credentials: values.credentials,
            }),
        });
        if authoritative {
            self.config.identity.sources.push(IdentitySource {
                provider: id.into(),
                authoritative: true,
            });
        }
        let yaml = permesh_config::to_yaml(&self.config)?;
        Config::from_bytes(yaml.as_bytes())?;
        let parent = self
            .path
            .parent()
            .ok_or_else(|| AppError::input("Cannot locate workspace directory"))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|_| AppError::input("Cannot prepare workspace update"))?;
        temporary
            .write_all(yaml.as_bytes())
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|_| AppError::new(5, "Cannot write workspace update"))?;
        if source(&self.path)? != self.original {
            return Err(AppError::input(
                "Workspace changed during setup; no configuration was replaced. Run setup again using the current configuration.",
            ));
        }
        temporary
            .persist(&self.path)
            .map_err(|_| AppError::new(5, "Cannot replace workspace configuration"))?;
        Outcome::new(
            "provider_setup",
            serde_json::json!({"id":id,"provider":registration.id,"sha256":registration.sha256,"file":self.path,"message":"Created provider instance. Review the configuration diff and execution approval before querying; credentials were not resolved or stored.","next":format!("permesh provider external review {id}")}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    #[test]
    fn changed_workspace_and_invalid_references_never_replace_configuration() -> TestResult {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("permesh.yaml");
        let cli = Cli::try_parse_from([
            "permesh",
            "--config",
            path.to_str().ok_or("UTF8 path")?,
            "init",
        ])?;
        std::fs::write(
            &path,
            "version: 1\norganization: {name: Original}\nproviders: []\n",
        )?;
        let draft = Draft::load(&cli, "example-main").map_err(|e| e.message)?;
        let changed = "version: 1\norganization: {name: Changed}\nproviders: []\n";
        std::fs::write(&path, changed)?;
        let registration = Registration {
            schema: 1,
            id: "example".into(),
            sha256: "a".repeat(64),
            capabilities: vec![],
        };
        assert!(
            draft
                .commit(
                    "example-main",
                    &registration,
                    ResolvedSetup::default(),
                    false
                )
                .is_err_and(|e| e.message.contains("changed"))
        );
        assert_eq!(std::fs::read_to_string(&path)?, changed);
        let draft = Draft::load(&cli, "example-main").map_err(|e| e.message)?;
        let mut values = ResolvedSetup::default();
        values
            .credentials
            .insert("token".into(), "SENTINEL_PRIVATE".into());
        assert!(
            draft
                .commit("example-main", &registration, values, false)
                .is_err_and(|e| !e.message.contains("SENTINEL_PRIVATE"))
        );
        assert_eq!(std::fs::read_to_string(&path)?, changed);
        Ok(())
    }
    #[test]
    fn setup_refuses_to_write_a_workspace_that_cannot_be_loaded() -> TestResult {
        let temporary = tempfile::tempdir()?;
        let path = temporary.path().join("permesh.yaml");
        let cli = Cli::try_parse_from([
            "permesh",
            "--config",
            path.to_str().ok_or("UTF8 path")?,
            "init",
        ])?;
        let providers: Vec<_> = (0..16).map(|n| serde_json::json!({"id":format!("instance{n}"),"type":"external","external":{"provider":"example","sha256":"a".repeat(64),"configuration":{"padding":"x".repeat(62000)}}})).collect();
        let original = serde_json::to_vec(
            &serde_json::json!({"version":1,"organization":{"name":"Example"},"providers":providers}),
        )?;
        Config::from_bytes(&original)?;
        std::fs::write(&path, &original)?;
        let draft = Draft::load(&cli, "new-main").map_err(|e| e.message)?;
        let registration = Registration {
            schema: 1,
            id: "example".into(),
            sha256: "a".repeat(64),
            capabilities: vec![],
        };
        let mut values = ResolvedSetup::default();
        values.configuration.insert(
            "padding".into(),
            serde_json::Value::String("x".repeat(64000)),
        );
        assert!(
            draft
                .commit("new-main", &registration, values, false)
                .is_err()
        );
        assert_eq!(std::fs::read(&path)?, original);
        Ok(())
    }
}
