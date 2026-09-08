// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::{
    MAX_RECORDS, ProtocolError,
    framing::read_json_frame,
    wire::{Envelope, Event, Record, valid_name},
};
use permesh_core::Snapshot;
use permesh_provider_sdk::Capability;
use std::io::BufRead;

/// A validated event; the snapshot is available only after EOF via `finish`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Handshake,
    Record,
    Complete,
}

/// Incremental validator for one handshake and discovery exchange.
/// The caller supplies bounded LF-terminated frames and must observe EOF before
/// calling `finish`. Any error permanently invalidates this decoder.
pub struct DiscoveryDecoder {
    provider: String,
    expected: Option<Vec<Capability>>,
    capabilities: Option<Vec<Capability>>,
    snapshot: Snapshot,
    total: usize,
    count: usize,
    completed: bool,
    error: Option<ProtocolError>,
}

impl DiscoveryDecoder {
    pub fn new(
        provider: &str,
        instance: &str,
        capabilities: Option<&[Capability]>,
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
            snapshot: Snapshot::new(instance),
            total: 0,
            count: 0,
            completed: false,
            error: None,
        })
    }

    /// Accept exactly one LF-terminated frame, including its delimiter in all
    /// byte budgets. Additional lines and trailing bytes are rejected.
    pub fn push_frame(&mut self, frame: &[u8]) -> Result<Progress, ProtocolError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let result = (|| {
            let mut reader = frame;
            let value =
                read_json_frame(&mut reader, &mut self.total)?.ok_or(ProtocolError::Truncated)?;
            if !reader.is_empty() {
                return Err(ProtocolError::Sequence);
            }
            self.accept(value)
        })();
        if let Err(error) = result {
            self.error = Some(error);
        }
        result
    }

    fn accept(&mut self, value: serde_json::Value) -> Result<Progress, ProtocolError> {
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
                } => {
                    if provider != self.provider {
                        return Err(ProtocolError::Provider);
                    }
                    if !draft {
                        return Err(ProtocolError::Version);
                    }
                    if has_duplicates(&capabilities)
                        || self.expected.as_ref().is_some_and(|expected| {
                            expected.len() != capabilities.len()
                                || expected.iter().any(|item| !capabilities.contains(item))
                        })
                    {
                        return Err(ProtocolError::Capability);
                    }
                    self.capabilities = Some(capabilities);
                    return Ok(Progress::Handshake);
                }
                Event::Error { .. } => return Err(ProtocolError::ProviderFailed),
                _ => return Err(ProtocolError::Sequence),
            }
        }
        if envelope.id != "discover" {
            return Err(ProtocolError::Sequence);
        }
        match envelope.event {
            Event::Record { record } => {
                if self.count == MAX_RECORDS {
                    return Err(ProtocolError::RecordLimit);
                }
                if !self
                    .capabilities
                    .as_ref()
                    .is_some_and(|items| items.contains(&record.capability()))
                {
                    return Err(ProtocolError::Capability);
                }
                match record {
                    Record::Identity(value) => self.snapshot.identities.push(value),
                    Record::Account(value) => self.snapshot.accounts.push(value),
                    Record::Resource(value) => self.snapshot.resources.push(value),
                    Record::Group(value) => self.snapshot.groups.push(value),
                    Record::Membership(value) => self.snapshot.memberships.push(value),
                    Record::Grant(value) => self.snapshot.grants.push(value),
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
                self.completed = true;
                Ok(Progress::Complete)
            }
            Event::Error { .. } => Err(ProtocolError::ProviderFailed),
            _ => Err(ProtocolError::Sequence),
        }
    }

    /// After the caller observes EOF, validate references and return a sorted
    /// snapshot. Completion alone never exposes unvalidated records.
    pub fn finish(mut self) -> Result<Snapshot, ProtocolError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if !self.completed {
            return Err(ProtocolError::Incomplete);
        }
        self.snapshot
            .validate()
            .map_err(|_| ProtocolError::Snapshot)?;
        self.snapshot.sort();
        Ok(self.snapshot)
    }
}

fn has_duplicates(capabilities: &[Capability]) -> bool {
    capabilities
        .iter()
        .enumerate()
        .any(|(i, capability)| capabilities[..i].contains(capability))
}

/// Validate a handshake plus one discovery response through EOF. No process is
/// launched. A malformed exchange never returns a partially validated snapshot.
pub fn validate_discovery(
    mut reader: impl BufRead,
    provider: &str,
    instance: &str,
) -> Result<Snapshot, ProtocolError> {
    let mut decoder = DiscoveryDecoder::new(provider, instance, None)?;
    while let Some(value) = read_json_frame(&mut reader, &mut decoder.total)? {
        decoder.accept(value)?;
    }
    decoder.finish()
}
