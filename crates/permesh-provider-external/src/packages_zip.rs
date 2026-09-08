// SPDX-License-Identifier: MIT
//! Deliberately narrow ZIP profile: two ordinary entries, stored or deflated,
//! no extra fields, archive comment, spanning, ZIP64, encryption, prefixes or trailing records.
use super::{DistributionError, MAX_LICENSE, Release, filename, hash};
use std::io::{Cursor, Read};
const MAX_BINARY: u64 = 128 * 1024 * 1024;
pub(super) struct Contents {
    pub executable: Vec<u8>,
    pub license: Vec<u8>,
}
fn invalid<T>() -> Result<T, DistributionError> {
    Err(DistributionError::Integrity)
}
fn slice(bytes: &[u8], start: usize, len: usize) -> Result<&[u8], DistributionError> {
    bytes
        .get(start..start.checked_add(len).ok_or(DistributionError::Integrity)?)
        .ok_or(DistributionError::Integrity)
}
fn u16_at(bytes: &[u8], start: usize) -> Result<u16, DistributionError> {
    Ok(u16::from_le_bytes(
        slice(bytes, start, 2)?
            .try_into()
            .map_err(|_| DistributionError::Integrity)?,
    ))
}
fn u32_at(bytes: &[u8], start: usize) -> Result<u32, DistributionError> {
    Ok(u32::from_le_bytes(
        slice(bytes, start, 4)?
            .try_into()
            .map_err(|_| DistributionError::Integrity)?,
    ))
}
struct Entry {
    name: Vec<u8>,
    flags: u16,
    method: u16,
    crc: u32,
    compressed: u32,
    size: u32,
    local: usize,
}
// Check central entry count before ZipArchive can allocate an index. Checking
// archive.len() afterwards is too late for a hostile central-directory count.
fn preflight(bytes: &[u8], binary: &str) -> Result<(), DistributionError> {
    let end = bytes
        .len()
        .checked_sub(22)
        .ok_or(DistributionError::Integrity)?;
    if slice(bytes, end, 4)? != b"PK\x05\x06"
        || u16_at(bytes, end + 4)? != 0
        || u16_at(bytes, end + 6)? != 0
        || u16_at(bytes, end + 8)? != 2
        || u16_at(bytes, end + 10)? != 2
        || u16_at(bytes, end + 20)? != 0
    {
        return invalid();
    }
    let size = u32_at(bytes, end + 12)? as usize;
    let start = u32_at(bytes, end + 16)? as usize;
    if size > 8192 || start.checked_add(size) != Some(end) {
        return invalid();
    }
    let mut position = start;
    let mut entries = Vec::new();
    for _ in 0..2 {
        if slice(bytes, position, 4)? != b"PK\x01\x02" {
            return invalid();
        }
        let name_len = u16_at(bytes, position + 28)? as usize;
        let extra_len = u16_at(bytes, position + 30)? as usize;
        let comment_len = u16_at(bytes, position + 32)? as usize;
        // Extra fields can override the sizes/offsets just checked (notably ZIP64).
        // This fixed package profile rejects every extension rather than letting
        // a second parser reinterpret its central/local metadata.
        if extra_len != 0 {
            return invalid();
        }
        let name = slice(bytes, position + 46, name_len)?.to_vec();
        if (name != binary.as_bytes() && name != b"LICENSE")
            || entries.iter().any(|entry: &Entry| entry.name == name)
        {
            return invalid();
        }
        let flags = u16_at(bytes, position + 8)?;
        let method = u16_at(bytes, position + 10)?;
        // Only deflate level hints, data descriptors and UTF-8 are accepted.
        if flags & !0x080e != 0 || !matches!(method, 0 | 8) || u16_at(bytes, position + 34)? != 0 {
            return invalid();
        }
        let mode = u32_at(bytes, position + 38)? >> 16;
        if mode & 0o7000 != 0 || !matches!(mode & 0o170000, 0 | 0o100000) {
            return invalid();
        }
        let entry = Entry {
            name,
            flags,
            method,
            crc: u32_at(bytes, position + 16)?,
            compressed: u32_at(bytes, position + 20)?,
            size: u32_at(bytes, position + 24)?,
            local: u32_at(bytes, position + 42)? as usize,
        };
        let limit = if entry.name == b"LICENSE" {
            MAX_LICENSE
        } else {
            MAX_BINARY
        };
        if entry.size as u64 > limit || entry.compressed as u64 > crate::catalog::MAX_ARCHIVE_BYTES
        {
            return invalid();
        }
        entries.push(entry);
        position = position
            .checked_add(46 + name_len + extra_len + comment_len)
            .ok_or(DistributionError::Integrity)?;
        if position > end {
            return invalid();
        }
    }
    if position != end {
        return invalid();
    }
    entries.sort_by_key(|entry| entry.local);
    let mut expected = 0;
    for entry in entries {
        let at = entry.local;
        if at != expected
            || slice(bytes, at, 4)? != b"PK\x03\x04"
            || u16_at(bytes, at + 6)? != entry.flags
            || u16_at(bytes, at + 8)? != entry.method
        {
            return invalid();
        }
        let name_len = u16_at(bytes, at + 26)? as usize;
        let extra_len = u16_at(bytes, at + 28)? as usize;
        if extra_len != 0 || slice(bytes, at + 30, name_len)? != entry.name {
            return invalid();
        }
        let data = at
            .checked_add(30 + name_len + extra_len)
            .ok_or(DistributionError::Integrity)?;
        expected = data
            .checked_add(entry.compressed as usize)
            .ok_or(DistributionError::Integrity)?;
        if expected > start {
            return invalid();
        }
        if entry.flags & 8 == 0 {
            if u32_at(bytes, at + 14)? != entry.crc
                || u32_at(bytes, at + 18)? != entry.compressed
                || u32_at(bytes, at + 22)? != entry.size
            {
                return invalid();
            }
        } else {
            if slice(bytes, expected, 4)? == b"PK\x07\x08" {
                expected += 4;
            }
            if u32_at(bytes, expected)? != entry.crc
                || u32_at(bytes, expected + 4)? != entry.compressed
                || u32_at(bytes, expected + 8)? != entry.size
            {
                return invalid();
            }
            expected += 12;
        }
    }
    if expected != start {
        return invalid();
    }
    Ok(())
}
pub(super) fn decode(release: &Release, bytes: &[u8]) -> Result<Contents, DistributionError> {
    if bytes.len() as u64 > crate::catalog::MAX_ARCHIVE_BYTES
        || bytes.len() as u64 != release.archive_size
        || hash(bytes) != release.archive_sha256
    {
        return invalid();
    }
    let binary = filename(&release.target);
    preflight(bytes, binary)?;
    let mut zip =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| DistributionError::Integrity)?;
    if zip.len() != 2 {
        return invalid();
    }
    let mut executable = None;
    let mut license = None;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|_| DistributionError::Integrity)?;
        let is_binary = entry.name_raw() == binary.as_bytes();
        if (!is_binary && entry.name_raw() != b"LICENSE")
            || !entry.is_file()
            || entry.is_symlink()
            || entry.encrypted()
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o7000 != 0 || !matches!(mode & 0o170000, 0 | 0o100000))
        {
            return invalid();
        }
        let limit = if is_binary { MAX_BINARY } else { MAX_LICENSE };
        let expected = entry.size();
        if expected > limit {
            return invalid();
        }
        let mut output = Vec::new();
        entry
            .by_ref()
            .take(limit + 1)
            .read_to_end(&mut output)
            .map_err(|_| DistributionError::Integrity)?;
        if output.len() as u64 != expected || output.len() as u64 > limit {
            return invalid();
        }
        if is_binary {
            if executable.replace(output).is_some() {
                return invalid();
            }
        } else if license.replace(output).is_some() {
            return invalid();
        }
    }
    let executable = executable.ok_or(DistributionError::Integrity)?;
    if hash(&executable) != release.executable_sha256
        || !matches_target(&executable, &release.target)
    {
        return invalid();
    }
    Ok(Contents {
        executable,
        license: license.ok_or(DistributionError::Integrity)?,
    })
}
// Basic object format and CPU checks, not proof the OS loader will accept or
// safely execute the program. Fat Mach-O bundles are outside this ZIP profile.
pub(super) fn matches_target(bytes: &[u8], target: &str) -> bool {
    if target.ends_with("unknown-linux-gnu") {
        let machine = if target.starts_with("x86_64-") {
            62
        } else {
            183
        };
        bytes.len() >= 64
            && bytes.starts_with(b"\x7fELF\x02\x01\x01")
            && matches!(u16_at(bytes, 16), Ok(2 | 3))
            && u16_at(bytes, 18).is_ok_and(|value| value == machine)
            && u32_at(bytes, 20).is_ok_and(|value| value == 1)
            && u16_at(bytes, 52).is_ok_and(|value| value == 64)
    } else if target.ends_with("apple-darwin") {
        let machine = if target.starts_with("x86_64-") {
            0x01000007
        } else {
            0x0100000c
        };
        bytes.len() >= 32
            && bytes.starts_with(&[0xcf, 0xfa, 0xed, 0xfe])
            && u32_at(bytes, 4).is_ok_and(|value| value == machine)
            && u32_at(bytes, 12).is_ok_and(|value| value == 2)
    } else if target == "x86_64-pc-windows-msvc" {
        let Ok(offset) = u32_at(bytes, 60) else {
            return false;
        };
        let offset = offset as usize;
        bytes.starts_with(b"MZ")
            && slice(bytes, offset, 4).is_ok_and(|v| v == b"PE\0\0")
            && u16_at(bytes, offset + 4).is_ok_and(|value| value == 0x8664)
            && u16_at(bytes, offset + 20).is_ok_and(|n| n >= 112)
            && u16_at(bytes, offset + 22).is_ok_and(|flags| flags & 2 != 0 && flags & 0x2000 == 0)
            && u16_at(bytes, offset + 24).is_ok_and(|value| value == 0x20b)
    } else {
        false
    }
}
