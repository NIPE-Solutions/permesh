// SPDX-License-Identifier: MIT
use super::{
    Operation,
    records::Record,
    wire::{Envelope, Event},
};
use crate::{MAX_RECORDS, Progress, ProtocolError, framing::read_json_frame, wire::valid_name};
use permesh_core::Snapshot;
use permesh_provider_sdk::{Capability, Health};

// Shared framing, version pinning, handshake negotiation and irreversible errors.
// Operation-specific decoders never expose their accumulated result before EOF.
struct Session {
    provider: String,
    expected: Option<Vec<Capability>>,
    capabilities: Option<Vec<Capability>>,
    operation: Operation,
    total: usize,
    completed: bool,
    error: Option<ProtocolError>,
}
impl Session {
    fn new(
        provider: &str,
        instance: &str,
        capabilities: Option<&[Capability]>,
        operation: Operation,
    ) -> Result<Self, ProtocolError> {
        if !valid_name(provider) || !valid_name(instance) {
            return Err(ProtocolError::Provider);
        }
        if capabilities.is_some_and(has_duplicates) {
            return Err(ProtocolError::Capability);
        }
        Ok(Self {
            provider: provider.to_owned(),
            expected: capabilities.map(<[Capability]>::to_vec),
            capabilities: None,
            operation,
            total: 0,
            completed: false,
            error: None,
        })
    }
    fn frame(&mut self, frame: &[u8]) -> Result<serde_json::Value, ProtocolError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let mut reader = frame;
        let value =
            read_json_frame(&mut reader, &mut self.total)?.ok_or(ProtocolError::Truncated)?;
        if !reader.is_empty() {
            return Err(ProtocolError::Sequence);
        }
        Ok(value)
    }
    // None is a successfully negotiated handshake; Some is an operation event.
    fn accept(&mut self, value: serde_json::Value) -> Result<Option<Event>, ProtocolError> {
        if self.completed {
            return Err(ProtocolError::Sequence);
        }
        let envelope = Envelope::parse(value)?;
        if self.capabilities.is_none() {
            if envelope.id != "handshake" {
                return Err(ProtocolError::Sequence);
            }
            match envelope.event {
                Event::Handshake {
                    provider,
                    capabilities,
                    draft,
                    operations,
                } => {
                    if provider != self.provider {
                        return Err(ProtocolError::Provider);
                    }
                    if !draft {
                        return Err(ProtocolError::Version);
                    }
                    let capabilities: Vec<_> = capabilities
                        .into_iter()
                        .map(super::mapping::capability)
                        .collect();
                    if has_duplicates(&capabilities)
                        || self.expected.as_ref().is_some_and(|expected| {
                            expected.len() != capabilities.len()
                                || expected.iter().any(|item| !capabilities.contains(item))
                        })
                    {
                        return Err(ProtocolError::Capability);
                    }
                    if operations.len() > super::MAX_OPERATIONS
                        || !operations
                            .iter()
                            .any(|item| item == self.operation.as_str())
                        || operations.iter().enumerate().any(|(i, item)| {
                            (item.len() > super::MAX_OPERATION_NAME_BYTES || !valid_name(item))
                                || operations[..i].contains(item)
                        })
                    {
                        return Err(ProtocolError::Capability);
                    }
                    self.capabilities = Some(capabilities);
                    Ok(None)
                }
                Event::Error { .. } => Err(ProtocolError::ProviderFailed),
                _ => Err(ProtocolError::Sequence),
            }
        } else {
            if envelope.id != self.operation.as_str() {
                return Err(ProtocolError::Sequence);
            }
            Ok(Some(envelope.event))
        }
    }
    fn remember<T>(&mut self, result: Result<T, ProtocolError>) -> Result<T, ProtocolError> {
        if let Err(error) = result {
            self.error = Some(error);
        }
        result
    }
    fn finish(&self) -> Result<(), ProtocolError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if !self.completed {
            return Err(ProtocolError::Incomplete);
        }
        Ok(())
    }
}

/// Incremental validator for one handshake and discovery exchange.
/// The caller supplies bounded LF-terminated frames and must observe EOF before
/// calling `finish`. Any error permanently invalidates this decoder.
pub struct DiscoveryDecoder {
    session: Session,
    snapshot: Snapshot,
    count: usize,
}
impl DiscoveryDecoder {
    /// Require discovery support. `Some(capabilities)` pins the exact registered
    /// capability set and must be used by process hosts. `None` supports offline
    /// validation without registration metadata.
    pub fn new(
        provider: &str,
        instance: &str,
        capabilities: Option<&[Capability]>,
    ) -> Result<Self, ProtocolError> {
        Ok(Self {
            session: Session::new(provider, instance, capabilities, Operation::Discover)?,
            snapshot: Snapshot::new(instance),
            count: 0,
        })
    }
    /// Accept exactly one LF-terminated frame, including its delimiter in all
    /// byte budgets. Additional lines and trailing bytes are rejected.
    pub fn push_frame(&mut self, frame: &[u8]) -> Result<Progress, ProtocolError> {
        let result = (|| {
            let value = self.session.frame(frame)?;
            self.accept(value)
        })();
        self.session.remember(result)
    }
    fn accept(&mut self, value: serde_json::Value) -> Result<Progress, ProtocolError> {
        let Some(event) = self.session.accept(value)? else {
            return Ok(Progress::Handshake);
        };
        match event {
            Event::Record { record } => {
                if self.count == MAX_RECORDS {
                    return Err(ProtocolError::RecordLimit);
                }
                if !self.session.capabilities.as_ref().is_some_and(|items| {
                    items.contains(&super::mapping::capability(record.capability()))
                }) {
                    return Err(ProtocolError::Capability);
                }
                match record {
                    Record::Identity(value) => self
                        .snapshot
                        .identities
                        .push(super::mapping::identity(value)),
                    Record::Account(value) => {
                        self.snapshot.accounts.push(super::mapping::account(value))
                    }
                    Record::Resource(value) => self
                        .snapshot
                        .resources
                        .push(super::mapping::resource(value)),
                    Record::Group(value) => self.snapshot.groups.push(super::mapping::group(value)),
                    Record::Membership(value) => self
                        .snapshot
                        .memberships
                        .push(super::mapping::membership(value)),
                    Record::Grant(value) => self.snapshot.grants.push(super::mapping::grant(value)),
                }
                self.count += 1;
                Ok(Progress::Record)
            }
            Event::Complete {
                count,
                complete,
                limitations,
            } => {
                if count != self.count {
                    return Err(ProtocolError::Count);
                }
                if !complete && limitations.is_empty() {
                    return Err(ProtocolError::Schema);
                }
                self.snapshot.complete = complete;
                self.snapshot.limitations = limitations
                    .iter()
                    .map(|item| item.message().to_owned())
                    .collect();
                self.session.completed = true;
                Ok(Progress::Complete)
            }
            Event::Error { .. } => Err(ProtocolError::ProviderFailed),
            _ => Err(ProtocolError::Sequence),
        }
    }
    /// After the caller observes EOF, validate references and return a sorted
    /// snapshot. Completion alone never exposes unvalidated records.
    pub fn finish(mut self) -> Result<Snapshot, ProtocolError> {
        self.session.finish()?;
        self.snapshot
            .validate()
            .map_err(|_| ProtocolError::Snapshot)?;
        self.snapshot.sort();
        Ok(self.snapshot)
    }
}

/// Incremental negotiated v1 health exchange. The caller must observe EOF
/// before calling `finish`; any additional frame permanently rejects the result.
pub struct HealthDecoder {
    session: Session,
    health: Option<Health>,
}
impl HealthDecoder {
    pub fn new(
        provider: &str,
        instance: &str,
        capabilities: Option<&[Capability]>,
    ) -> Result<Self, ProtocolError> {
        Ok(Self {
            session: Session::new(provider, instance, capabilities, Operation::Check)?,
            health: None,
        })
    }
    pub fn push_frame(&mut self, frame: &[u8]) -> Result<Progress, ProtocolError> {
        let result = (|| {
            let value = self.session.frame(frame)?;
            let Some(event) = self.session.accept(value)? else {
                return Ok(Progress::Handshake);
            };
            match event {
                Event::Health {
                    limitations: Some(limitations),
                    ..
                } => {
                    self.health = Some(Health {
                        message: "Trusted external provider health check completed".to_owned(),
                        limitations: limitations
                            .iter()
                            .map(|item| item.message().to_owned())
                            .collect(),
                    });
                    self.session.completed = true;
                    Ok(Progress::Complete)
                }
                Event::Health {
                    limitations: None, ..
                } => Err(ProtocolError::Schema),
                Event::Error { .. } => Err(ProtocolError::ProviderFailed),
                _ => Err(ProtocolError::Sequence),
            }
        })();
        self.session.remember(result)
    }
    pub fn finish(self) -> Result<Health, ProtocolError> {
        self.session.finish()?;
        self.health.ok_or(ProtocolError::Incomplete)
    }
}

fn has_duplicates(capabilities: &[Capability]) -> bool {
    capabilities
        .iter()
        .enumerate()
        .any(|(i, capability)| capabilities[..i].contains(capability))
}
