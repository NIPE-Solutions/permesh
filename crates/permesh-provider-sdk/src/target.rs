// SPDX-License-Identifier: MIT
//! Shared supported native targets for official packages and portable workspace pins.
pub const TARGETS: [&str; 5] = [
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
];

/// Classify the running CLI/provider host build, never environment-variable overrides.
pub fn native_target() -> Option<&'static str> {
    let abi = if cfg!(target_env = "gnu") {
        "gnu"
    } else if cfg!(target_env = "msvc") {
        "msvc"
    } else {
        ""
    };
    classify(std::env::consts::ARCH, std::env::consts::OS, abi)
}
fn classify(arch: &str, os: &str, abi: &str) -> Option<&'static str> {
    match (arch, os, abi) {
        ("x86_64", "linux", "gnu") => Some("x86_64-unknown-linux-gnu"),
        ("aarch64", "linux", "gnu") => Some("aarch64-unknown-linux-gnu"),
        ("x86_64", "macos", _) => Some("x86_64-apple-darwin"),
        ("aarch64", "macos", _) => Some("aarch64-apple-darwin"),
        ("x86_64", "windows", "msvc") => Some("x86_64-pc-windows-msvc"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_mapping_covers_supported_builds_and_rejects_other_abis() {
        for (arch, os, abi, expected) in [
            ("x86_64", "linux", "gnu", Some("x86_64-unknown-linux-gnu")),
            ("aarch64", "linux", "gnu", Some("aarch64-unknown-linux-gnu")),
            ("x86_64", "macos", "", Some("x86_64-apple-darwin")),
            ("aarch64", "macos", "", Some("aarch64-apple-darwin")),
            ("x86_64", "windows", "msvc", Some("x86_64-pc-windows-msvc")),
            ("x86_64", "linux", "musl", None),
            ("aarch64", "linux", "musl", None),
            ("x86_64", "windows", "gnu", None),
            ("aarch64", "windows", "msvc", None),
            ("x86_64", "freebsd", "", None),
            ("riscv64", "linux", "gnu", None),
        ] {
            assert_eq!(classify(arch, os, abi), expected);
        }
        if let Some(target) = native_target() {
            assert!(TARGETS.contains(&target));
        }
    }
}
