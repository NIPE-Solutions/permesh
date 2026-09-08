// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::{
    MAX_RECORDS, ProtocolError,
    framing::read_json_frame,
    wire::{Envelope, Event, Record, valid_name},
};
use permesh_core::Snapshot;
use std::io::BufRead;

/// Validate a handshake plus one discovery response through EOF. No process is
/// launched. Blocking input, deadlines and cancellation belong to a future host.
/// A malformed exchange never returns a partially validated snapshot.
pub fn validate_discovery(
    mut reader: impl BufRead,
    provider: &str,
    instance: &str,
) -> Result<Snapshot, ProtocolError> {
    if !valid_name(provider) || !valid_name(instance) {
        return Err(ProtocolError::Provider);
    }
    let mut total = 0;
    let first = read_json_frame(&mut reader, &mut total)?.ok_or(ProtocolError::Incomplete)?;
    let handshake = Envelope::parse(first)?;
    if handshake.id != "handshake" {
        return Err(ProtocolError::Sequence);
    }
    let capabilities = match handshake.event {
        Event::Handshake {
            provider: observed,
            capabilities,
            draft,
        } => {
            if observed != provider {
                return Err(ProtocolError::Provider);
            }
            if !draft {
                return Err(ProtocolError::Version);
            }
            if capabilities
                .iter()
                .enumerate()
                .any(|(i, capability)| capabilities[..i].contains(capability))
            {
                return Err(ProtocolError::Capability);
            }
            capabilities
        }
        Event::Error { .. } => return Err(ProtocolError::ProviderFailed),
        _ => return Err(ProtocolError::Sequence),
    };
    let mut snapshot = Snapshot::new(instance);
    let mut count = 0;
    let mut completed = false;
    while let Some(value) = read_json_frame(&mut reader, &mut total)? {
        if completed {
            return Err(ProtocolError::Sequence);
        }
        let envelope = Envelope::parse(value)?;
        if envelope.id != "discover" {
            return Err(ProtocolError::Sequence);
        }
        match envelope.event {
            Event::Record { record } => {
                if count == MAX_RECORDS {
                    return Err(ProtocolError::RecordLimit);
                }
                if !capabilities.contains(&record.capability()) {
                    return Err(ProtocolError::Capability);
                }
                match record {
                    Record::Identity(value) => snapshot.identities.push(value),
                    Record::Account(value) => snapshot.accounts.push(value),
                    Record::Resource(value) => snapshot.resources.push(value),
                    Record::Group(value) => snapshot.groups.push(value),
                    Record::Membership(value) => snapshot.memberships.push(value),
                    Record::Grant(value) => snapshot.grants.push(value),
                }
                count += 1;
            }
            Event::Complete {
                count: claimed,
                complete,
                limitations,
            } => {
                if claimed != count {
                    return Err(ProtocolError::Count);
                }
                if !complete && limitations.is_empty() {
                    return Err(ProtocolError::Schema);
                }
                snapshot.complete = complete;
                snapshot.limitations = limitations
                    .iter()
                    .map(|item| item.message().to_owned())
                    .collect();
                completed = true;
            }
            Event::Error { .. } => return Err(ProtocolError::ProviderFailed),
            _ => return Err(ProtocolError::Sequence),
        }
    }
    if !completed {
        return Err(ProtocolError::Incomplete);
    }
    snapshot.validate().map_err(|_| ProtocolError::Snapshot)?;
    snapshot.sort();
    Ok(snapshot)
}
