# 3. Strict single-file YAML

Status: accepted

## Decision and consequences

Schema-versioned permesh.yaml supports one-file workspaces and ancestor discovery. Use maintained serde-saphyr with optional includes/properties disabled and resource budgets. Reject unknown fields and duplicate IDs. Imports and overrides are deferred until deterministic semantics are implemented.
