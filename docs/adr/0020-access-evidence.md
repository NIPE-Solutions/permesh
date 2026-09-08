# ADR 0020: Preserve access evidence without claiming effective authorization

Status: Proposed for the domain migration slice

## Decision

Retain the native role and broad normalized privilege. Extend normalized access
records with evidence kind: permission observation, assignment, policy attachment
or unknown. Certainty describes the observation (observed, derived, inferred,
unknown), independently of privilege. A known policy attachment can be observed
with unknown privilege and unknown effective authorization.

Retain the existing Grant name internally until a rename provides more value
than migration churn. Document that it represents access evidence, not a proof
that every action is allowed. Explicit deny/resource policies must not be forced
into a positive grant; add a separate supported evidence contract when a concrete
adapter can observe them reliably.

## Consequences

AWS IAM attachments remain attachments; do not classify AdministratorAccess by
name alone. Identity Center permission-set/account assignments should use the
assignment model. GitHub team access paths derive from observed membership and
permission records; branch protection and other policy effects remain unknown.
Resource containment never synthesizes access. Old-wire observations default to
unknown evidence kind, preserving the original certainty and role without
claiming semantics the sender could not express.
