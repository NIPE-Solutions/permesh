// SPDX-License-Identifier: MIT
//! Reference-only configuration and explicitly resolved, zeroizing credentials.
use secrecy::{ExposeSecret, SecretString};
use std::{fmt, str::FromStr};

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid secret reference; use env://NAME or keychain://provider-id/credential-name")]
    InvalidReference,
    #[error("credential environment variable is missing, empty, or not Unicode")]
    EnvironmentUnavailable,
    #[error(
        "credential store unavailable or credential missing; check the OS credential store and auth login"
    )]
    KeychainUnavailable,
    #[error("credential mutation requires a keychain reference")]
    NotKeychain,
    #[error("credential must not be empty")]
    Empty,
}
#[derive(Clone, PartialEq, Eq)]
pub enum SecretRef {
    Env(String),
    Keychain { service: String, account: String },
}
fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
impl SecretRef {
    pub fn parse(value: &str) -> Result<Self> {
        value.parse()
    }
    fn valid(&self) -> bool {
        match self {
            Self::Env(name) => {
                name.len() <= 256
                    && name
                        .bytes()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
                    && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            }
            Self::Keychain { service, account } => valid_id(service) && valid_id(account),
        }
    }
}
impl FromStr for SecretRef {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self> {
        let reference = if let Some(name) = value.strip_prefix("env://") {
            Self::Env(name.into())
        } else if let Some(rest) = value.strip_prefix("keychain://") {
            let (service, account) = rest.split_once('/').ok_or(Error::InvalidReference)?;
            Self::Keychain {
                service: service.into(),
                account: account.into(),
            }
        } else {
            return Err(Error::InvalidReference);
        };
        if reference.valid() {
            Ok(reference)
        } else {
            Err(Error::InvalidReference)
        }
    }
}
impl fmt::Display for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.valid() {
            return f.write_str("[invalid secret reference]");
        }
        match self {
            Self::Env(name) => write!(f, "env://{name}"),
            Self::Keychain { service, account } => write!(f, "keychain://{service}/{account}"),
        }
    }
}
impl fmt::Debug for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
pub struct Secret(SecretString);
impl Secret {
    pub fn new(value: String) -> Self {
        Self(value.into())
    }
    pub fn expose(&self) -> &str {
        self.0.expose_secret()
    }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}
#[derive(Debug, Default)]
pub struct SecretResolver;
impl SecretResolver {
    pub fn resolve(&self, reference: &SecretRef) -> Result<Secret> {
        if !reference.valid() {
            return Err(Error::InvalidReference);
        }
        let value = match reference {
            SecretRef::Env(name) => {
                std::env::var(name).map_err(|_| Error::EnvironmentUnavailable)?
            }
            SecretRef::Keychain { .. } => entry(reference)?
                .get_password()
                .map_err(|_| Error::KeychainUnavailable)?,
        };
        if value.is_empty() {
            return Err(Error::Empty);
        }
        Ok(Secret::new(value))
    }
}
fn entry(reference: &SecretRef) -> Result<keyring::Entry> {
    if !reference.valid() {
        return Err(Error::InvalidReference);
    }
    let SecretRef::Keychain { service, account } = reference else {
        return Err(Error::NotKeychain);
    };
    keyring::Entry::new(&format!("permesh:{service}"), account)
        .map_err(|_| Error::KeychainUnavailable)
}
/// Explicitly store a credential in the native OS credential store.
pub fn store(reference: &SecretRef, secret: &Secret) -> Result<()> {
    if secret.expose().is_empty() {
        return Err(Error::Empty);
    }
    entry(reference)?
        .set_password(secret.expose())
        .map_err(|_| Error::KeychainUnavailable)
}
/// Explicitly delete a credential from the native OS credential store.
pub fn delete(reference: &SecretRef) -> Result<()> {
    entry(reference)?
        .delete_credential()
        .map_err(|_| Error::KeychainUnavailable)
}
