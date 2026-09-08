// SPDX-License-Identifier: MIT OR Apache-2.0
use super::*;
use permesh_provider_sdk::Capability;
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Write},
    path::Path,
};
use zip::{ZipWriter, write::SimpleFileOptions};
type TestResult = Result<(), Box<dyn std::error::Error>>;
const fn native_fixture() -> [u8; 256] {
    let mut bytes = [0; 256];
    if cfg!(windows) {
        bytes[0] = b'M';
        bytes[1] = b'Z';
        bytes[60] = 64;
        bytes[64] = b'P';
        bytes[65] = b'E';
        bytes[68] = 0x64;
        bytes[69] = 0x86;
        bytes[84] = 0xf0;
        bytes[86] = 2;
        bytes[88] = 0x0b;
        bytes[89] = 2;
    } else {
        bytes[0] = 0x7f;
        bytes[1] = b'E';
        bytes[2] = b'L';
        bytes[3] = b'F';
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[6] = 1;
        bytes[16] = 2;
        bytes[18] = 62;
        bytes[20] = 1;
        bytes[52] = 64;
    }
    bytes
}
const NATIVE: &[u8] = &native_fixture();
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn target() -> &'static str {
    if cfg!(windows) {
        "x86_64-pc-windows-msvc"
    } else {
        "x86_64-unknown-linux-gnu"
    }
}
fn binary() -> &'static str {
    if cfg!(windows) {
        "provider.exe"
    } else {
        "provider"
    }
}
fn archive(entries: &[(&str, &[u8])]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer.start_file(
            *name,
            SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .unix_permissions(0o755),
        )?;
        writer.write_all(bytes)?;
    }
    Ok(writer.finish()?.into_inner())
}
fn release(bytes: &[u8]) -> Release {
    Release {
        provider: "example".into(),
        version: semver::Version::new(1, 0, 0),
        target: target().into(),
        capabilities: vec![Capability::Accounts],
        protocols: vec![2, 3],
        archive_sha256: digest(bytes),
        executable_sha256: digest(NATIVE),
        archive_size: bytes.len() as u64,
    }
}
fn store(root: &Path) -> Result<PackageStore, DistributionError> {
    PackageStore::new(root.join("packages"))
}
#[test]
fn missing_inventory_is_read_only_and_relative_roots_rejected() -> TestResult {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    assert!(store(&root)?.list_provider("example")?.is_empty());
    assert!(!root.join("packages").exists());
    assert!(PackageStore::new(PathBuf::from("relative")).is_err());
    Ok(())
}
#[test]
fn installs_private_native_copy_and_preserves_prior_versions() -> TestResult {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let store = store(&root)?;
    let bytes = archive(&[(binary(), NATIVE), ("LICENSE", b"Example license")])?;
    let mut rel = release(&bytes);
    let first = store.install(&rel, &bytes)?;
    assert_eq!(std::fs::read(&first.executable)?, NATIVE);
    assert!(first.executable.is_absolute());
    rel.version = semver::Version::new(2, 0, 0);
    let second = store.install(&rel, &bytes)?;
    assert_ne!(first.executable, second.executable);
    assert_eq!(store.list_provider("example")?.len(), 2);
    assert_eq!(store.install(&rel, &bytes)?.executable, second.executable);
    assert_eq!(std::fs::read(first.executable)?, NATIVE);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(second.executable)?.permissions().mode() & 0o777,
            0o700
        );
    }
    Ok(())
}
#[test]
fn digest_changes_bad_archives_and_tampering_do_not_replace_installations() -> TestResult {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let store = store(&root)?;
    let bytes = archive(&[(binary(), NATIVE), ("LICENSE", b"License")])?;
    let rel = release(&bytes);
    let installed = store.install(&rel, &bytes)?;
    let replacement = archive(&[(binary(), NATIVE), ("LICENSE", b"Different license")])?;
    assert!(store.install(&release(&replacement), &replacement).is_err());
    assert_eq!(std::fs::read(&installed.executable)?, NATIVE);
    let mut invalid = rel.clone();
    invalid.version = semver::Version::new(2, 0, 0);
    invalid.executable_sha256 = "0".repeat(64);
    assert!(store.install(&invalid, &bytes).is_err());
    assert_eq!(store.list_provider("example")?.len(), 1);
    std::fs::write(&installed.executable, b"\x7fELFtampered")?;
    assert!(store.list_provider("example").is_err());
    assert!(store.install(&rel, &bytes).is_err());
    Ok(())
}
#[test]
fn unapproved_zip_names_and_extra_or_missing_entries_are_rejected() -> TestResult {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let store = store(&root)?;
    for name in [
        "../provider",
        "/provider",
        "dir/provider",
        "dir\\provider",
        "provider:stream",
        "PROVIDER",
        "provider/",
    ] {
        let bytes = archive(&[(name, NATIVE), ("LICENSE", b"License")])?;
        assert!(
            store.install(&release(&bytes), &bytes).is_err(),
            "accepted {name}"
        );
    }
    for entries in [
        vec![(binary(), NATIVE)],
        vec![
            (binary(), NATIVE),
            ("LICENSE", b"License"),
            ("extra", b"extra"),
        ],
    ] {
        let bytes = archive(&entries)?;
        assert!(store.install(&release(&bytes), &bytes).is_err());
    }
    assert!(store.list_provider("example")?.is_empty());
    Ok(())
}
#[test]
fn script_payloads_and_oversized_license_are_rejected() -> TestResult {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let store = store(&root)?;
    let script = b"#!/bin/sh\nexit 0\n";
    let bytes = archive(&[(binary(), script), ("LICENSE", b"License")])?;
    let mut rel = release(&bytes);
    rel.executable_sha256 = digest(script);
    assert!(store.install(&rel, &bytes).is_err());
    let large = vec![b'x'; 1024 * 1024 + 1];
    let bytes = archive(&[(binary(), NATIVE), ("LICENSE", &large)])?;
    assert!(store.install(&release(&bytes), &bytes).is_err());
    assert!(store.list_provider("example")?.is_empty());
    Ok(())
}
#[test]
fn concurrent_installations_never_overwrite_or_remove_another_reservation() -> TestResult {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let store = store(&root)?;
    let bytes = archive(&[(binary(), NATIVE), ("LICENSE", b"License")])?;
    let rel = release(&bytes);
    let successes = std::thread::scope(|scope| {
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let barrier = barrier.clone();
                let store = &store;
                let rel = &rel;
                let bytes = &bytes;
                scope.spawn(move || {
                    barrier.wait();
                    store.install(rel, bytes).is_ok()
                })
            })
            .collect();
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap_or(false))
            .filter(|ok| *ok)
            .count()
    });
    assert!(successes >= 1);
    let inventory = store.list_provider("example")?;
    assert_eq!(inventory.len(), 1);
    assert_eq!(std::fs::read(&inventory[0].executable)?, NATIVE);
    assert!(store.install(&rel, &bytes).is_ok());
    Ok(())
}
#[cfg(unix)]
#[test]
fn symlinked_package_or_storage_ancestry_is_rejected() -> TestResult {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let bytes = archive(&[(binary(), NATIVE), ("LICENSE", b"License")])?;
    let managed = store(&root)?;
    let installed = managed.install(&release(&bytes), &bytes)?;
    let outside = root.join("outside");
    std::fs::write(&outside, NATIVE)?;
    std::fs::remove_file(&installed.executable)?;
    symlink(&outside, &installed.executable)?;
    assert!(managed.list_provider("example").is_err());
    symlink(root.join("packages"), root.join("alias"))?;
    assert!(PackageStore::new(root.join("alias")).is_err());
    assert_eq!(std::fs::read(outside)?, NATIVE);
    Ok(())
}
fn central(bytes: &[u8]) -> Result<usize, Box<dyn std::error::Error>> {
    bytes
        .windows(4)
        .position(|v| v == b"PK\x01\x02")
        .ok_or_else(|| "missing central header".into())
}
#[test]
fn encrypted_special_modes_duplicate_names_and_forged_sizes_fail_closed() -> TestResult {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let store = store(&root)?;
    let original = archive(&[(binary(), NATIVE), ("LICENSE", b"License")])?;
    let first = central(&original)?;
    let second = first + 46 + binary().len();
    // Real compressed ZIP files with altered security-sensitive central fields.
    for mode in [0o120777u32, 0o020600, 0o104755, 0o102755, 0o101755] {
        let mut bytes = original.clone();
        bytes[first + 5] = 3;
        bytes[first + 38..first + 42].copy_from_slice(&(mode << 16).to_le_bytes());
        assert!(store.install(&release(&bytes), &bytes).is_err());
    }
    let mut encrypted = original.clone();
    encrypted[first + 8] |= 1;
    encrypted[6] |= 1;
    assert!(store.install(&release(&encrypted), &encrypted).is_err());
    let mut oversized = original.clone();
    oversized[first + 24..first + 28].copy_from_slice(&(128u32 * 1024 * 1024 + 1).to_le_bytes());
    assert!(store.install(&release(&oversized), &oversized).is_err());
    // Rename LICENSE to the binary in both headers, adjusting lengths via a fresh
    // archive whose two distinct names have the same length as the expected file.
    let other = "x".repeat(binary().len());
    let mut duplicate = archive(&[(binary(), NATIVE), (&other, b"License")])?;
    let first_duplicate = central(&duplicate)?;
    let second_duplicate = first_duplicate + 46 + binary().len();
    let local =
        u32::from_le_bytes(duplicate[second_duplicate + 42..second_duplicate + 46].try_into()?)
            as usize;
    duplicate[second_duplicate + 46..second_duplicate + 46 + binary().len()]
        .copy_from_slice(binary().as_bytes());
    duplicate[local + 30..local + 30 + binary().len()].copy_from_slice(binary().as_bytes());
    assert!(store.install(&release(&duplicate), &duplicate).is_err());
    let mut missing = original.clone();
    missing[second + 46..second + 53].copy_from_slice(b"licence");
    assert!(store.install(&release(&missing), &missing).is_err());
    assert!(store.list_provider("example")?.is_empty());
    Ok(())
}
#[test]
fn license_and_manifest_changes_are_detected_on_inventory() -> TestResult {
    for name in ["LICENSE", "manifest.json"] {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        let store = store(&root)?;
        let bytes = archive(&[(binary(), NATIVE), ("LICENSE", b"License")])?;
        let installed = store.install(&release(&bytes), &bytes)?;
        let dir = installed.executable.parent().ok_or("no parent")?;
        std::fs::write(dir.join(name), b"tampered")?;
        assert!(store.list_provider("example").is_err());
    }
    Ok(())
}
#[test]
fn unknown_files_never_get_recursively_deleted() -> TestResult {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let store = store(&root)?;
    let bytes = archive(&[(binary(), NATIVE), ("LICENSE", b"License")])?;
    let rel = release(&bytes);
    let installed = store.install(&rel, &bytes)?;
    let dir = installed.executable.parent().ok_or("no parent")?;
    std::fs::write(dir.join("keep-me"), b"not a managed file")?;
    assert!(store.install(&rel, &bytes).is_err());
    assert_eq!(std::fs::read(dir.join("keep-me"))?, b"not a managed file");
    Ok(())
}

#[test]
fn declared_target_must_match_native_format_and_architecture() -> TestResult {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let store = store(&root)?;
    let bytes = archive(&[(binary(), NATIVE), ("LICENSE", b"License")])?;
    let mut rel = release(&bytes);
    rel.target = "aarch64-apple-darwin".into();
    assert!(store.install(&rel, &bytes).is_err());
    if !cfg!(windows) {
        rel.target = "aarch64-unknown-linux-gnu".into();
        assert!(store.install(&rel, &bytes).is_err());
    }
    Ok(())
}
#[test]
fn all_supported_native_headers_are_checked_without_execution() -> TestResult {
    for target in crate::catalog::TARGETS {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        let store = store(&root)?;
        let mut native = vec![0u8; 512];
        if target.ends_with("unknown-linux-gnu") {
            native[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
            native[16] = 2;
            native[18] = if target.starts_with("aarch64-") {
                183
            } else {
                62
            };
            native[20] = 1;
            native[52] = 64;
        } else if target.ends_with("apple-darwin") {
            native[..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
            native[4..8].copy_from_slice(
                &(if target.starts_with("aarch64-") {
                    0x0100000cu32
                } else {
                    0x01000007
                })
                .to_le_bytes(),
            );
            native[12] = 2;
        } else {
            native[..2].copy_from_slice(b"MZ");
            native[60] = 64;
            native[64..68].copy_from_slice(b"PE\0\0");
            native[68..70].copy_from_slice(&0x8664u16.to_le_bytes());
            native[84] = 240;
            native[86] = 2;
            native[88] = 11;
            native[89] = 2;
        }
        let name = if target.ends_with("windows-msvc") {
            "provider.exe"
        } else {
            "provider"
        };
        let bytes = archive(&[(name, &native), ("LICENSE", b"License")])?;
        let mut rel = release(&bytes);
        rel.target = target.into();
        rel.executable_sha256 = digest(&native);
        let installed = store.install(&rel, &bytes)?;
        assert_eq!(std::fs::read(installed.executable)?, native);
    }
    Ok(())
}
#[test]
fn manifest_binding_unknown_fields_and_size_limits_fail_closed() -> TestResult {
    for change in [
        "provider",
        "version",
        "target",
        "unknown",
        "duplicate",
        "oversized",
    ] {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        let store = store(&root)?;
        let bytes = archive(&[(binary(), NATIVE), ("LICENSE", b"License")])?;
        let rel = release(&bytes);
        let installed = store.install(&rel, &bytes)?;
        let path = installed
            .executable
            .parent()
            .ok_or("no parent")?
            .join("manifest.json");
        let mut manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        let encoded = match change {
            "provider" => {
                manifest["release"]["provider"] = "other".into();
                serde_json::to_vec(&manifest)?
            }
            "version" => {
                manifest["release"]["version"] = "9.0.0".into();
                serde_json::to_vec(&manifest)?
            }
            "target" => {
                manifest["release"]["target"] = "aarch64-apple-darwin".into();
                serde_json::to_vec(&manifest)?
            }
            "unknown" => {
                manifest["path"] = "/tmp/untrusted".into();
                serde_json::to_vec(&manifest)?
            }
            "duplicate" => {
                let original = serde_json::to_string(&manifest)?;
                format!("{{\"schema\":1,{}", &original[1..]).into_bytes()
            }
            _ => vec![b'x'; 16 * 1024 + 1],
        };
        std::fs::write(&path, encoded)?;
        assert!(
            store.list_provider("example").is_err(),
            "accepted changed {change}"
        );
        assert!(store.install(&rel, &bytes).is_err());
    }
    Ok(())
}
#[test]
fn central_directory_count_mismatch_and_local_name_mismatch_are_rejected() -> TestResult {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let store = store(&root)?;
    let bytes = archive(&[(binary(), NATIVE), ("LICENSE", b"License")])?;
    for count in [0u16, 1, 3, u16::MAX] {
        let mut mutated = bytes.clone();
        let end = mutated.len() - 22;
        mutated[end + 8..end + 10].copy_from_slice(&count.to_le_bytes());
        mutated[end + 10..end + 12].copy_from_slice(&count.to_le_bytes());
        assert!(store.install(&release(&mutated), &mutated).is_err());
    }
    let mut local = bytes.clone();
    local[30] = b'x';
    assert!(store.install(&release(&local), &local).is_err());
    let mut appended = bytes.clone();
    appended.extend_from_slice(b"trailer");
    assert!(store.install(&release(&appended), &appended).is_err());
    let mut corrupt = bytes;
    corrupt[30 + binary().len()] ^= 0xff;
    assert!(store.install(&release(&corrupt), &corrupt).is_err());
    Ok(())
}
#[test]
fn zip64_extra_cannot_override_preflight_sizes_or_local_offsets() -> TestResult {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let store = store(&root)?;
    let mut bytes = archive(&[(binary(), NATIVE), ("LICENSE", b"License")])?;
    let central = central(&bytes)?;
    let compressed = u32::from_le_bytes(bytes[central + 20..central + 24].try_into()?) as u64;
    let size = u32::from_le_bytes(bytes[central + 24..central + 28].try_into()?) as u64;
    let offset = u32::from_le_bytes(bytes[central + 42..central + 46].try_into()?) as u64;
    let mut extra = vec![1, 0, 24, 0];
    extra.extend_from_slice(&size.to_le_bytes());
    extra.extend_from_slice(&compressed.to_le_bytes());
    extra.extend_from_slice(&offset.to_le_bytes());
    bytes[central + 30..central + 32].copy_from_slice(&28u16.to_le_bytes());
    bytes.splice(
        central + 46 + binary().len()..central + 46 + binary().len(),
        extra,
    );
    let end = bytes.len() - 22;
    let old_size = u32::from_le_bytes(bytes[end + 12..end + 16].try_into()?);
    bytes[end + 12..end + 16].copy_from_slice(&(old_size + 28).to_le_bytes());
    assert!(store.install(&release(&bytes), &bytes).is_err());
    assert!(!root.join("packages").exists());
    Ok(())
}
#[test]
fn existing_staging_directory_is_not_owned_or_cleaned_by_a_later_install() -> TestResult {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let store = store(&root)?;
    let bytes = archive(&[(binary(), NATIVE), ("LICENSE", b"License")])?;
    let mut rel = release(&bytes);
    let first = store.install(&rel, &bytes)?;
    rel.version = semver::Version::new(2, 0, 0);
    let version = root.join("packages/example/2.0.0");
    crate::trust::ensure_root(&version)?;
    let stage = version.join(format!(".pending-{}", rel.target));
    crate::trust::make_dir(&stage)?;
    crate::trust::write_new(&stage.join("keep-me"), b"another reservation", false)?;
    assert!(store.install(&rel, &bytes).is_err());
    assert_eq!(
        std::fs::read(stage.join("keep-me"))?,
        b"another reservation"
    );
    assert!(!version.join(format!(".install-{}", rel.target)).exists());
    assert_eq!(std::fs::read(first.executable)?, NATIVE);
    assert_eq!(store.list_provider("example")?.len(), 1);
    Ok(())
}
#[test]
fn archive_digest_and_declared_size_are_checked_before_state_creation() -> TestResult {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let store = store(&root)?;
    let bytes = archive(&[(binary(), NATIVE), ("LICENSE", b"License")])?;
    let mut rel = release(&bytes);
    rel.archive_sha256 = "0".repeat(64);
    assert!(store.install(&rel, &bytes).is_err());
    rel = release(&bytes);
    rel.archive_size += 1;
    assert!(store.install(&rel, &bytes).is_err());
    assert!(!root.join("packages").exists());
    Ok(())
}
