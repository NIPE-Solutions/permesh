// SPDX-License-Identifier: MIT OR Apache-2.0
//! Explicit native-store checks; ordinary cargo test never touches credentials.
#![allow(clippy::unwrap_used)]
use permesh_secrets::{Secret, SecretRef, SecretResolver, delete, store};

struct EntryGuard(SecretRef, bool);
impl Drop for EntryGuard {
    fn drop(&mut self) {
        if self.1 && delete(&self.0).is_err() && !std::thread::panicking() {
            eprintln!("Temporary native credential cleanup failed");
        }
    }
}

#[test]
#[ignore = "creates and removes a synthetic native credential; requires an unlocked store"]
fn native_round_trip() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let reference = SecretRef::parse(&format!(
        "keychain://test-{}-{nonce}/token",
        std::process::id()
    ))
    .unwrap();
    assert!(SecretResolver.resolve(&reference).is_err());
    let mut guard = EntryGuard(reference, true);
    let first = Secret::new(format!("synthetic-first-{nonce}"));
    store(&guard.0, &first).unwrap();
    assert!(SecretResolver.resolve(&guard.0).unwrap().expose() == first.expose());
    let second = Secret::new(format!("synthetic-replacement-{nonce}"));
    store(&guard.0, &second).unwrap();
    assert!(SecretResolver.resolve(&guard.0).unwrap().expose() == second.expose());
    delete(&guard.0).unwrap();
    assert!(SecretResolver.resolve(&guard.0).is_err());
    // Cleanup already verified; the guard is only needed on earlier failure.
    guard.1 = false;
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires an intentionally unavailable D-Bus session"]
fn unavailable_service_is_redacted() {
    assert!(
        std::env::var("DBUS_SESSION_BUS_ADDRESS")
            .unwrap()
            .contains("permesh-no-bus")
    );
    let reference = SecretRef::parse("keychain://test-no-service/token").unwrap();
    let value = "synthetic-unavailable-credential";
    let error = store(&reference, &Secret::new(value.into())).unwrap_err();
    let message = format!("{error} {error:?}");
    assert!(!message.contains(value));
    assert!(!message.contains("permesh-no-bus"));
    assert!(SecretResolver.resolve(&reference).is_err());
}
