// SPDX-License-Identifier: MIT
use crate::{
    args::{AddProvider, Cli},
    error::AppError,
    report::Outcome,
};
use permesh_config::{
    AuthConfig, Config, IdentityConfig, IdentitySource, Organization, ProviderConfig, ProviderKind,
    find_workspace, to_yaml,
};
use std::{
    fs::OpenOptions,
    io::{self, BufRead, IsTerminal, Write},
    path::{Path, PathBuf},
};
pub fn path(cli: &Cli) -> Result<PathBuf, AppError> {
    if let Some(path) = &cli.config {
        Ok(path.clone())
    } else {
        Ok(find_workspace(&std::env::current_dir().map_err(|_| {
            AppError::input("Cannot read current directory")
        })?)?)
    }
}
pub fn init(cli: &Cli, demo: bool, organization: &Option<String>) -> Result<Outcome, AppError> {
    let name = if let Some(name) = organization {
        name.clone()
    } else if io::stdin().is_terminal() && !cli.json {
        write!(
            io::stderr().lock(),
            "Permesh\n\nOrganization name [My organization]: "
        )
        .map_err(|_| AppError::new(5, "Cannot write prompt"))?;
        io::stderr()
            .flush()
            .map_err(|_| AppError::new(5, "Cannot write prompt"))?;
        let mut input = String::new();
        std::io::Read::take(io::stdin().lock(), 258)
            .read_line(&mut input)
            .map_err(|_| AppError::input("Cannot read organization name"))?;
        if input.trim().is_empty() {
            "My organization".into()
        } else {
            input.trim().into()
        }
    } else {
        "My organization".into()
    };
    let providers = if demo {
        vec![ProviderConfig {
            id: "demo".into(),
            kind: ProviderKind::Demo,
            customer_id: None,
            organizations: vec![],
            auth: None,
            external: None,
        }]
    } else {
        vec![]
    };
    let sources = if demo {
        vec![IdentitySource {
            provider: "demo".into(),
            authoritative: true,
        }]
    } else {
        vec![]
    };
    let config = Config {
        version: 1,
        organization: Organization { name },
        providers,
        identity: IdentityConfig {
            sources,
            ..Default::default()
        },
    };
    let yaml = to_yaml(&config)?;
    let path = cli
        .config
        .clone()
        .unwrap_or_else(|| PathBuf::from("permesh.yaml"));
    create_file(&path, yaml.as_bytes())?;
    Outcome::new(
        "init",
        serde_json::json!({"message":"Created workspace","file":path,"next":if demo {"permesh doctor; permesh user alice@example.com"}else{"permesh provider install github --version VERSION"}}),
    )
}
fn create_file(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file=options.open(path).map_err(|_|AppError::input("Cannot create workspace file. Choose a writable directory and a path that does not already exist."))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| AppError::new(5, "Cannot finish writing workspace file"))
}
pub fn add(cli: &Cli, args: &AddProvider) -> Result<Outcome, AppError> {
    if args.provider_type == "github" {
        return Err(AppError::input(
            "GitHub uses an external provider: install an official package, explicitly trust its binary, then run permesh provider setup github --id INSTANCE. Existing legacy instances use provider migrate",
        ));
    }
    let path = path(cli)?;
    if std::fs::symlink_metadata(&path)
        .map_err(|_| AppError::input("Cannot inspect configuration file"))?
        .file_type()
        .is_symlink()
    {
        return Err(AppError::input(
            "Provider add refuses to replace a symlink. Edit its target explicitly.",
        ));
    }
    let mut config = Config::load(&path)?;
    let kind = match args.provider_type.as_str() {
        "google" => ProviderKind::Google,
        "external" => ProviderKind::External,
        _ => return Err(AppError::input("Unsupported provider type")),
    };
    if args.authoritative && !matches!(kind, ProviderKind::Google | ProviderKind::External) {
        return Err(AppError::input(
            "Only identity-source providers support --authoritative",
        ));
    }
    let id = args
        .id
        .clone()
        .unwrap_or_else(|| format!("{}-main", args.provider_type));
    let external = if kind == ProviderKind::External {
        if args.token_ref.is_some() {
            return Err(AppError::input(
                "Use --credential NAME=REFERENCE for external credentials",
            ));
        }
        Some(permesh_config::ExternalConfig {
            provider: args
                .provider
                .clone()
                .ok_or_else(|| AppError::input("External providers require --provider"))?,
            sha256: args
                .sha256
                .clone()
                .ok_or_else(|| AppError::input("External providers require --sha256"))?,
            configuration: pairs(&args.setting)?
                .into_iter()
                .map(|(key, value)| (key, serde_json::Value::String(value)))
                .collect(),
            credentials: pairs(&args.credential)?,
        })
    } else {
        if args.provider.is_some()
            || args.sha256.is_some()
            || !args.setting.is_empty()
            || !args.credential.is_empty()
        {
            return Err(AppError::input(
                "--provider, --sha256, --setting and --credential require type external",
            ));
        }
        None
    };
    config.providers.push(ProviderConfig {
        id: id.clone(),
        kind,
        customer_id: args.customer_id.clone(),
        organizations: args.organization.clone(),
        auth: (kind != ProviderKind::External).then(|| AuthConfig {
            token: args
                .token_ref
                .clone()
                .unwrap_or_else(|| format!("keychain://{id}/token")),
        }),
        external,
    });
    if args.authoritative {
        config.identity.sources.push(IdentitySource {
            provider: id.clone(),
            authoritative: true,
        });
    }
    let yaml = to_yaml(&config)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|_| AppError::input("Cannot create temporary configuration file"))?;
    temporary
        .write_all(yaml.as_bytes())
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|_| AppError::new(5, "Cannot write configuration"))?;
    temporary
        .persist(&path)
        .map_err(|_| AppError::new(5, "Cannot replace configuration"))?;
    Outcome::new(
        "provider_add",
        serde_json::json!({"message":"Added provider. Configuration formatting was normalized; review the Git diff.","next":if kind == ProviderKind::External {format!("permesh provider external review {id}")}else{format!("permesh auth login {id}")}}),
    )
}

fn pairs(values: &[String]) -> Result<std::collections::BTreeMap<String, String>, AppError> {
    let mut result = std::collections::BTreeMap::new();
    for value in values {
        let (key, value) = value
            .split_once('=')
            .ok_or_else(|| AppError::input("Expected NAME=VALUE"))?;
        if key.is_empty() || result.insert(key.to_owned(), value.to_owned()).is_some() {
            return Err(AppError::input(
                "Setting and credential names must be nonempty and unique",
            ));
        }
    }
    Ok(result)
}
