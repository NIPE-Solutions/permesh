# Control-command schema 1 fixtures

`reports.json` contains 13 complete JSON command reports captured from the
`10ff45b` CLI before control DTO extraction. The capture used the synthetic demo
and an inert native-magic byte file that was inspected and registered but never
executed. No provider API or credential lookup was needed.

Only command execution timestamps, the temporary root path and the path-bound
approval fingerprint are normalized. Tests validate timestamp and digest shape
before normalization. Observation timestamps and other data must remain exact.
Both unapproved and approved review reports, all capability strings, empty and
populated registration lists, full identity configuration and configuration nulls
are preserved.

`distribution.json` freezes the complete selected-release and installed-package
objects, including null for an absent package. It passed against the original
storage serializers before switching the test to CLI DTOs. This tests output
mapping without downloading a catalog or installing a provider.

The outer result keys already belong to the CLI. `schema1_control` independently
owns metadata, capability spelling, identity source/configuration summaries,
inspection, registration, approval, release and package serialization. All
conversions are explicit field mappings. Public SDK setup and browser-auth specs
are intentionally reused because they already own separately versioned boundary
schemas; duplicating them here would introduce competing versions of the same
public contract. Existing protocol setup/browser fixtures cover those schemas.

Do not regenerate expected files from the new serializers during tests. Changes
to internal domain, configuration or storage structs must not add fields to these
schema 1 outputs implicitly.
