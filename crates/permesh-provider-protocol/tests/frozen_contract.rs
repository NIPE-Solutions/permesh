// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_provider_protocol::{
    BrowserAuthDecoder, DiscoveryDecoder, HealthDecoder, SetupDecoder,
};

const DISCOVERY: [&str; 2] = [
    include_str!("fixtures/draft1-discovery.ndjson"),
    include_str!("fixtures/draft2-discovery.ndjson"),
];

#[test]
fn wire_capabilities_are_frozen_independently_of_sdk_metadata() {
    use permesh_provider_protocol::records::Capability;
    use permesh_provider_sdk::Capability as SdkCapability;
    for (text, registered) in [
        ("accounts", SdkCapability::Accounts),
        ("identities", SdkCapability::Identities),
        ("resources", SdkCapability::Resources),
        ("groups", SdkCapability::Groups),
        ("memberships", SdkCapability::Memberships),
        ("grants", SdkCapability::Grants),
    ] {
        let encoded = format!("\"{text}\"");
        let capability: Capability = serde_json::from_str(&encoded).unwrap();
        assert_eq!(serde_json::to_string(&capability).unwrap(), encoded);
        let mut decoder = DiscoveryDecoder::new("fixture", "main", Some(&[registered])).unwrap();
        let handshake = format!(
            "{{\"protocol\":1,\"id\":\"handshake\",\"event\":\"handshake\",\"provider\":\"fixture\",\"capabilities\":[{encoded}],\"draft\":true}}\n"
        );
        decoder.push_frame(handshake.as_bytes()).unwrap();
    }
    assert!(serde_json::from_str::<Capability>("\"future_capability\"").is_err());
}

#[test]
fn provider_owned_records_emit_frozen_bytes_without_core_serialization() {
    use permesh_provider_protocol::records::Record;
    for frame in DISCOVERY[0].lines().skip(1).take(6) {
        let record_json = frame.split_once("\"kind\":").unwrap().1;
        let record_json = format!("{{\"kind\":{record_json}");
        let record: Record = serde_json::from_str(&record_json).unwrap();
        assert_eq!(serde_json::to_string(&record).unwrap(), record_json);
    }
}

#[test]
fn historical_discovery_records_survive_mapping_and_validate_at_eof() {
    for (index, transcript) in DISCOVERY.iter().enumerate() {
        let mut decoder =
            DiscoveryDecoder::new_versioned("fixture", "main", None, index as u32 + 1).unwrap();
        for frame in transcript.split_inclusive('\n') {
            decoder.push_frame(frame.as_bytes()).unwrap();
        }
        let snapshot = decoder.finish().unwrap();
        assert_eq!(snapshot.identities[0].id, "person");
        assert_eq!(
            snapshot.identities[0].kind,
            permesh_core::IdentityKind::Human
        );
        assert_eq!(
            snapshot.identities[0].status,
            permesh_core::IdentityStatus::Active
        );
        assert_eq!(
            snapshot.identities[0].verified_emails,
            ["person@example.test"]
        );
        assert_eq!(snapshot.accounts[0].key.id, "account");
        assert_eq!(snapshot.accounts[0].key.provider, "main");
        assert_eq!(snapshot.accounts[0].login, "person");
        assert_eq!(snapshot.accounts[0].kind, permesh_core::IdentityKind::Human);
        assert_eq!(
            snapshot.accounts[0].verified_emails,
            ["person@example.test"]
        );
        assert_eq!(snapshot.resources[0].key.id, "resource");
        assert_eq!(snapshot.resources[0].name, "Resource");
        assert_eq!(snapshot.groups[0].key.id, "group");
        assert_eq!(snapshot.groups[0].name, "Group");
        assert_eq!(snapshot.memberships.len(), 1);
        assert_eq!(
            snapshot.memberships[0].member,
            permesh_core::Subject::Account(permesh_core::EntityKey::new("main", "account"))
        );
        assert_eq!(snapshot.memberships[0].group.id, "group");
        assert_eq!(snapshot.memberships[0].provenance.method, "synthetic");
        assert_eq!(
            snapshot.memberships[0].provenance.observed_at,
            "2026-01-01T00:00:00Z"
        );
        assert_eq!(snapshot.grants[0].id, "grant");
        assert_eq!(
            snapshot.grants[0].subject,
            permesh_core::Subject::Group(permesh_core::EntityKey::new("main", "group"))
        );
        assert_eq!(snapshot.grants[0].resource.id, "resource");
        assert_eq!(snapshot.grants[0].role, "read");
        assert_eq!(
            snapshot.grants[0].privilege,
            permesh_core::Privilege::Standard
        );
        assert_eq!(
            snapshot.grants[0].certainty,
            permesh_core::Certainty::Observed
        );
        assert_eq!(snapshot.grants[0].provenance.method, "synthetic");
        assert_eq!(
            snapshot.grants[0].provenance.observed_at,
            "2026-01-01T00:00:00Z"
        );
        assert!(snapshot.complete);
        snapshot.validate().unwrap();
    }
}

#[test]
fn every_legacy_enum_value_keeps_its_wire_spelling_and_domain_meaning() {
    use permesh_provider_protocol::records;
    macro_rules! cases {
        ($name:ident, $field:ident, $collection:ident, $original:literal, $($text:literal => $variant:ident),+ $(,)?) => {
            $(
                let wire: records::$name = serde_json::from_str($text).unwrap();
                assert_eq!(serde_json::to_string(&wire).unwrap(), $text);
                let transcript = DISCOVERY[0].replace(
                    concat!("\"", stringify!($field), "\":\"", $original, "\""),
                    &format!("\"{}\":{}", stringify!($field), $text));
                let mut decoder = DiscoveryDecoder::new("fixture", "main", None).unwrap();
                for frame in transcript.split_inclusive('\n') {
                    decoder.push_frame(frame.as_bytes()).unwrap();
                }
                assert_eq!(decoder.finish().unwrap().$collection[0].$field, permesh_core::$name::$variant);
            )+
        };
    }
    cases!(IdentityKind, kind, identities, "human", "\"human\"" => Human, "\"external\"" => Unknown,
        "\"service\"" => Service, "\"bot\"" => Bot, "\"unknown\"" => Unknown);
    cases!(IdentityStatus, status, identities, "active", "\"active\"" => Active, "\"inactive\"" => Inactive,
        "\"external\"" => Unknown, "\"service\"" => Unknown, "\"unknown\"" => Unknown);
    cases!(Privilege, privilege, grants, "standard", "\"standard\"" => Standard, "\"elevated\"" => Elevated,
        "\"admin\"" => Admin, "\"owner\"" => Owner, "\"unknown\"" => Unknown);
    cases!(Certainty, certainty, grants, "observed", "\"observed\"" => Observed, "\"inferred\"" => Inferred,
        "\"unknown\"" => Unknown);
}

#[test]
fn historical_operation_specific_drafts_remain_accepted() {
    let mut health = HealthDecoder::new("fixture", "main", None).unwrap();
    for frame in include_str!("fixtures/draft2-health.ndjson").split_inclusive('\n') {
        health.push_frame(frame.as_bytes()).unwrap();
    }
    assert_eq!(health.finish().unwrap().limitations.len(), 1);
    let mut setup = SetupDecoder::new("fixture", "main", None).unwrap();
    for frame in include_str!("fixtures/draft3-setup.ndjson").split_inclusive('\n') {
        setup.push_frame(frame.as_bytes()).unwrap();
    }
    assert_eq!(setup.finish().unwrap().steps[0].id, "connection");
    let mut auth = BrowserAuthDecoder::new("fixture", "main", None).unwrap();
    for frame in include_str!("fixtures/draft4-auth.ndjson").split_inclusive('\n') {
        auth.push_frame(frame.as_bytes()).unwrap();
    }
    assert_eq!(auth.finish().unwrap().refresh_token_slot, "refresh_token");
}

#[test]
fn historical_records_reject_schema_capability_version_and_reference_changes() {
    for (from, to) in [
        ("\"kind\":\"human\"", "\"kind\":\"future_kind\""),
        (
            "\"login\":\"person\"",
            "\"login\":\"person\",\"extra\":true",
        ),
        ("\"id\":\"account\"", "\"id\":\"\""),
        ("\"accounts\",", ""),
        ("\"protocol\":1", "\"protocol\":2"),
        ("2026-01-01T00:00:00Z", "invalid-time"),
    ] {
        let transcript = DISCOVERY[0].replace(from, to);
        let mut decoder = DiscoveryDecoder::new("fixture", "main", None).unwrap();
        let rejected = transcript
            .split_inclusive('\n')
            .any(|frame| decoder.push_frame(frame.as_bytes()).is_err());
        assert!(
            rejected || decoder.finish().is_err(),
            "mutation accepted: {from}"
        );
    }
}
