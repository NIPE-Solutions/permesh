// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(clippy::unwrap_used)]
use permesh_provider_demo::DemoProvider;
use permesh_provider_sdk::{Provider, validate_snapshot};
#[tokio::test]
async fn demo_obeys_contract_and_explains_alice() {
    let provider = DemoProvider::new("demo");
    let s = provider.discover().await.unwrap();
    validate_snapshot(&s).unwrap();
    let result = permesh_core::query_user(&[s], &Default::default(), "alice@example.com").unwrap();
    assert_eq!(result.accounts.len(), 1);
    assert_eq!(result.access.len(), 2);
    assert!(
        result
            .access
            .iter()
            .any(|p| p.groups.iter().any(|g| g.name == "team/backend"))
    );
    assert!(provider.check().await.is_ok());
}
