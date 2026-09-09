# Explicit snapshots and offline comparison

These commands are available for evaluation in published CLI `0.1.0-alpha.3`.
Alpha.2 remains unchanged and does not contain them.

A snapshot records observations, not a complete authorization decision. It is an
explicit export, never a background cache or database. Files contain sensitive
identity and access metadata and should not be committed to Git by default.

## Commands

```sh
permesh snapshot create --output review.snapshot.json
permesh snapshot inspect review.snapshot.json
permesh diff previous.snapshot.json review.snapshot.json
```

Creation uses configured providers and their existing trust, approval and credential
boundaries. Inspection and comparison are offline: they load no workspace, resolve
no credentials and launch no provider. `--json` emits versioned result envelopes.

Creation refuses existing output unless `--overwrite` is explicit. Writes use a
private temporary file in the destination directory and an atomic rename. Unix
files are owner-readable/writable only; Windows inherits directory ACLs, so choose
a directory restricted to intended readers. Symlink destinations are rejected.

## Storage contract

`format: permesh_snapshot`, `format_version: 1` owns its record DTOs separately
from both provider wire models and internal Rust types. The artifact includes the
producing version, capture window, explicit identity aliases/authorities and each
requested provider's instance, type, executable pin where available, capabilities,
configuration-context digest, collection outcome and validated normalized data.

Provider versions that cannot be established are left unknown, not guessed from
an executable name. The executable digest is the exact code identity where an
external pin is available. Collection windows bound the operation; they do not
claim simultaneous or transactional observations across APIs.

Configuration values and credential references are not exported. A context digest
binds reviewed configuration, including the declared tenant/filter context and
credential references, for comparison. It does not prove that the upstream token's
permissions or tenant visibility stayed unchanged. Native resource and account IDs
retain provider scope; unknown provider-specific scope stays a documented limitation.

For a pinned file inventory, the content digest identifies the export, not the
collection scope. Comparisons bind the configured path, freshness policy and
inventory's declared scope. A different export must have a strictly newer export
time; copying older data into a later scan cannot establish disappearance. Inventory
scope, export time and content digest remain explicit source evidence.

Loads reject unsupported schemas, duplicate/unknown fields, invalid graph references,
invalid timestamps, excess nesting and files beyond the documented size limit.
Limits are 64 MiB per file, 16 KiB per string, 128 JSON nesting levels and one
million records per array (additional graph bounds also apply). A diff is capped at
100,000 changes and a 64 MiB change payload; exceeding either fails explicitly.
All reports are sensitive even when credentials are absent.

## Comparison meaning

Entities match on provider instance and immutable native ID. A login or resource
rename is a label change; it is not a new person or permission. Grant and membership
changes retain native semantics and provenance. Observation timestamps alone do
not become changes. Identity-mapping changes are reported separately.

When provider context, code, capabilities, limitations or collection completeness
prevent comparison, absent records are **inconclusive**. In a comparable successful
collection, absence means **no longer observed in that scope**, never revoked or
proof of universally denied access. A provider outage cannot become mass removal.
A later collection must start after the earlier one completed before missing
records can be classified as no longer observed. Reordering records and changing
observation timestamps alone do not produce access changes.
Both snapshots may still share an unobservable upstream visibility restriction.

Diff exit 0 means comparison completed within its reported scope, even when changes
exist. Exit 4 means part of the comparison is inconclusive. Invalid input returns 2;
local storage/output failures return 5. Existing inspection exit semantics are unchanged.
