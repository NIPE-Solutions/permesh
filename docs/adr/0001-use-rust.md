# 1. Use stable Rust

Status: accepted

## Decision and consequences

A single native binary with strong domain types and explicit error handling fits the read-only local tool. Rust workspace boundaries enforce provider/core separation. Cost: compile time and platform credential integration. Internal APIs may evolve before 1.0.
