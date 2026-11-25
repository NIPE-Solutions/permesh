// SPDX-License-Identifier: MIT
//! Official providers with recorded live tenant acceptance. Evidence for each
//! entry is in `docs/live-acceptance.md`; add a provider only after its run passes.
const LIVE_TESTED: &[&str] = &["github"];

pub fn live_tested(provider_id: &str) -> bool {
    LIVE_TESTED.contains(&provider_id)
}
