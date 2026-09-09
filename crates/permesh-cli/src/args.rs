// SPDX-License-Identifier: MIT
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
#[derive(Parser, Clone)]
#[command(
    name = "permesh",
    version,
    about = "Know who has access to what.",
    long_about = "Permesh discovers and correlates access locally. No backend. No telemetry. Read-only provider access."
)]
pub struct Cli {
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        help = "Emit JSON (access schema 2; control schema 1)"
    )]
    pub json: bool,
    #[arg(long, global = true, value_enum, default_value = "auto")]
    pub color: Color,
    #[arg(short='v',global=true,action=clap::ArgAction::Count,help="Include curated diagnostic context (never raw API data)")]
    pub verbose: u8,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Clone, Copy, ValueEnum)]
pub enum Color {
    Auto,
    Always,
    Never,
}
/// Explicit discovery contract for separately trusted native provider binaries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum DiscoveryProtocol {
    #[default]
    Legacy,
    NegotiatedV1,
}
impl From<DiscoveryProtocol> for permesh_config::DiscoveryProtocol {
    fn from(value: DiscoveryProtocol) -> Self {
        match value {
            DiscoveryProtocol::Legacy => Self::Legacy,
            DiscoveryProtocol::NegotiatedV1 => Self::NegotiatedV1,
        }
    }
}
impl std::fmt::Display for DiscoveryProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Legacy => "legacy",
            Self::NegotiatedV1 => "negotiated-v1",
        })
    }
}
#[derive(Subcommand, Clone)]
pub enum Command {
    /// Create a workspace without overwriting existing files.
    Init {
        #[arg(long)]
        demo: bool,
        #[arg(long)]
        organization: Option<String>,
    },
    /// Inspect configured provider instances or add a provider.
    Provider {
        #[command(subcommand)]
        command: ProviderCommand,
    },
    /// Manage local credentials for one provider instance.
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Diagnose configuration, credentials and provider connectivity.
    Doctor {
        #[arg(
            long,
            help = "Include curated stage codes and remediation from the same health checks"
        )]
        details: bool,
    },
    /// Show observed access and the paths that explain it.
    User { identity: String },
    /// Show privileged access, uncertain roles, and unresolved group grants.
    Admins,
    /// Review accounts not confidently associated with an active authoritative identity.
    Orphaned,
    /// Inspect identity evidence and explicitly manage stable account mappings.
    Identity {
        #[command(subcommand)]
        command: crate::identity_command::IdentityCommand,
    },
    /// Inspect who has observed access to a selected resource.
    Resource(crate::resource_command::ResourceArgs),
    /// Evaluate focused local access review rules without changing infrastructure.
    Policy {
        #[command(subcommand)]
        command: crate::policy_command::PolicyCommand,
    },
    /// Assess, plan and verify a departure review without changing provider access.
    Offboard {
        #[command(subcommand)]
        command: crate::offboard::OffboardCommand,
    },
    /// Save or inspect explicit local access snapshots.
    Snapshot {
        #[command(subcommand)]
        command: crate::snapshot_command::SnapshotCommand,
    },
    /// Compare two saved snapshots offline; absence is never proof of revocation.
    Diff { before: PathBuf, after: PathBuf },
    /// Print a static shell completion script; no workspace or provider access.
    Completion {
        #[arg(value_enum, conflicts_with = "json")]
        shell: clap_complete::Shell,
    },
    /// Show version and privacy defaults.
    Version,
}
#[derive(Subcommand, Clone)]
pub enum ProviderCommand {
    /// Create a provider scaffold or validate a recorded protocol exchange offline.
    Dev {
        #[command(subcommand)]
        command: crate::provider_development::DevelopmentCommand,
    },
    /// Migrate one GitHub or Google instance to an explicitly pinned trusted external provider.
    Migrate(crate::provider_migration::MigrationArgs),
    /// Download an exact official provider package without executing or trusting it.
    Install(crate::distribution::InstallArgs),
    /// Explicitly check or download official provider updates; workspace pins remain unchanged.
    Update(crate::distribution::UpdateArgs),
    List,
    Status {
        id: Option<String>,
    },
    Capabilities {
        id: String,
    },
    Add(Box<AddProvider>),
    /// Configure a new instance using a trusted provider's declarative setup form.
    Setup(crate::setup::SetupArgs),
    /// Explicitly manage and run trusted native external providers.
    External {
        #[command(subcommand)]
        command: crate::external::ExternalCommand,
    },
}
#[derive(Args, Clone)]
pub struct AddProvider {
    #[arg(value_parser=["github", "google", "cloudflare", "aws", "external"], help="Provider type; official providers guide package trust and setup")]
    pub provider_type: String,
    #[arg(long, value_enum, default_value_t = DiscoveryProtocol::Legacy, help = "Discovery contract for advanced external configuration; official packages use catalog metadata")]
    pub discovery_protocol: DiscoveryProtocol,
    #[arg(
        long,
        help = "Exact official provider version; defaults to the newest compatible release"
    )]
    pub version: Option<String>,
    #[arg(
        long,
        help = "Pin all available platforms of one official release for a shared workspace"
    )]
    pub portable: bool,
    #[arg(
        long,
        value_name = "FILE",
        help = "Provider declarative setup answers; nonsecret settings and credential references only"
    )]
    pub answers: Option<PathBuf>,
    #[arg(
        long,
        help = "Explicitly trust the selected official provider binary and approve the resulting instance's settings and credential delivery"
    )]
    pub accept_risk: bool,
    #[arg(long, help = "Stable instance ID; defaults to TYPE-main")]
    pub id: Option<String>,
    #[arg(long, help = "GitHub organization (repeat for multiple organizations)")]
    pub organization: Vec<String>,
    #[arg(long, help = "Explicit Google Workspace customer ID")]
    pub customer_id: Option<String>,
    #[arg(long, help = "Use this provider as an authoritative identity source")]
    pub authoritative: bool,
    #[arg(
        long,
        help = "Secret reference; defaults to the instance's native keychain entry"
    )]
    pub token_ref: Option<String>,
    #[arg(long, help = "Registered external provider ID")]
    pub provider: Option<String>,
    #[arg(
        long,
        conflicts_with = "target_sha256",
        help = "Pin the registered external executable SHA-256"
    )]
    pub sha256: Option<String>,
    #[arg(
        long,
        value_name = "TARGET=DIGEST",
        help = "Reviewed external executable digest for a platform; repeat for team platforms"
    )]
    pub target_sha256: Vec<String>,
    #[arg(
        long,
        value_name = "KEY=VALUE",
        help = "External nonsecret string setting; repeat as needed"
    )]
    pub setting: Vec<String>,
    #[arg(
        long,
        value_name = "NAME=REFERENCE",
        help = "External credential reference; repeat as needed"
    )]
    pub credential: Vec<String>,
}
#[derive(Clone, Subcommand)]
pub enum AuthCommand {
    /// Store a token in the configured native keychain entry.
    Login {
        id: String,
        #[arg(long, help = "Read token from stdin; no token arguments are accepted")]
        token_stdin: bool,
        #[arg(long, help = "Named external credential slot")]
        credential: Option<String>,
        #[arg(long, conflicts_with_all = ["token_stdin", "credential"], help = "Authorize in a browser using the provider's declared OAuth flow")]
        browser: bool,
        #[arg(
            long,
            requires = "browser",
            help = "Print the authorization URL instead of opening a browser"
        )]
        no_open: bool,
    },
    /// Check whether credentials can be resolved, without contacting providers.
    Status { id: Option<String> },
    /// Delete this instance's local keychain credential; revoke the token separately at its provider.
    Logout {
        id: String,
        #[arg(long)]
        credential: Option<String>,
    },
}
