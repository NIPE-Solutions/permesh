// SPDX-License-Identifier: MIT
//! Bound cumulative record copies during path expansion, not just path counts.
//! This is a conservative work budget, not a measurement of allocator/RSS usage.
//! Input, indexes, relevance lookups and output DTOs are outside this estimate.
//! Update these estimators when dynamic fields are added to domain records.
use crate::{DomainError, EntityKey, Grant, Group, Membership, Provenance, Resource, Subject};

const MAX_COPIED_BYTES: usize = 64 * 1024 * 1024;

pub(crate) struct CopyBudget(usize);
impl CopyBudget {
    pub(crate) fn new() -> Self {
        Self(MAX_COPIED_BYTES)
    }
    pub(crate) fn charge(&mut self, bytes: usize) -> Result<(), DomainError> {
        self.0 = self.0.checked_sub(bytes).ok_or(DomainError::PathLimit)?;
        Ok(())
    }
}
fn total(parts: &[usize]) -> usize {
    parts.iter().copied().fold(0, usize::saturating_add)
}
pub(crate) fn key_heap(key: &EntityKey) -> usize {
    key.provider.len().saturating_add(key.id.len())
}
pub(crate) fn subject_heap(subject: &Subject) -> usize {
    match subject {
        Subject::Account(key) | Subject::Group(key) => key_heap(key),
    }
}
fn provenance_heap(value: &Provenance) -> usize {
    value.method.len().saturating_add(value.observed_at.len())
}
pub(crate) fn group(value: &Group) -> usize {
    total(&[size_of::<Group>(), key_heap(&value.key), value.name.len()])
}
pub(crate) fn membership(value: &Membership) -> usize {
    total(&[
        size_of::<Membership>(),
        subject_heap(&value.member),
        key_heap(&value.group),
        provenance_heap(&value.provenance),
    ])
}
pub(crate) fn resource(value: &Resource) -> usize {
    total(&[
        size_of::<Resource>(),
        value.kind.as_ref().map_or(0, String::len),
        value.parent.as_ref().map_or(0, key_heap),
        key_heap(&value.key),
        value.name.len(),
    ])
}
pub(crate) fn grant(value: &Grant) -> usize {
    total(&[
        size_of::<Grant>(),
        value.id.len(),
        subject_heap(&value.subject),
        key_heap(&value.resource),
        value.role.len(),
        provenance_heap(&value.provenance),
    ])
}
