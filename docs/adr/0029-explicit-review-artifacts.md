# ADR 0029: Explicit local review artifacts

Status: accepted for the access-review source candidate; format stability remains under review.

## Context

A transient query explains one observation. Reviewing changes or verifying a departure
requires retaining deliberate evidence from earlier collections. Serializing the core
model directly would tie stored data to implementation changes, and comparing an
empty failed scan with a successful one could falsely suggest access was removed.

## Decision

Add explicit snapshot creation and offline inspection/comparison. Keep storage
format 1 independent of provider protocols, control output and domain serialization.
Conversion into the core remains validated. Store reviewed source context, code pins,
capabilities, collection windows, limitations, identity authority/mapping context and
normalized evidence. Do not store resolved credentials or raw provider responses.

Use stable scoped IDs for comparison. Provider failures, changed contexts and
unordered collection windows make disappearance inconclusive. A pinned inventory's
content digest identifies data; its declared scope and freshness policy identify the
collection context. New contents must have a newer export time before absence can
be classified as no longer observed. Even comparable absence is not a proof of denial.

Writes are explicit, private where the OS supports it, atomic and refuse overwrite
by default. Readers treat documents as untrusted bounded data and reject unsupported
versions, duplicate fields and invalid references. Offline commands do not load the
workspace, resolve credentials or execute providers.

## Consequences

Artifacts enable future reviews without introducing a persistent database or service.
They contain sensitive security metadata and require user-managed retention and
sharing. Digests detect context differences but do not authenticate the author or
prove unchanged upstream visibility. Windows ACLs and same-user filesystem races
remain OS-level concerns. A versioned fixture protects the initial storage contract;
future semantic changes require deliberate migration or incompatibility handling.
