// SPDX-License-Identifier: MIT OR Apache-2.0
//! The only unsafe module in this crate. Win32 does not expose secure-at-create
//! ACLs through std. Every pointer below is borrowed from a live owned allocation
//! or handle; the public-to-crate boundary uses paths, files, and static errors.
//! Administrators and SYSTEM are privileged OS principals, not adversaries.
use crate::ExternalError;
use std::{
    ffi::c_void,
    fs::File,
    mem::{size_of, zeroed},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::Path,
    ptr,
};
use windows_sys::Win32::{
    Foundation::{GENERIC_ALL, GENERIC_WRITE, INVALID_HANDLE_VALUE, LocalFree},
    Security::{
        ACE_HEADER, ACL,
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            GetSecurityInfo, SE_FILE_OBJECT,
        },
        DACL_SECURITY_INFORMATION, GetAce, GetSecurityDescriptorControl, GetTokenInformation,
        IsValidAcl, IsValidSid, OWNER_SECURITY_INFORMATION, SE_DACL_PROTECTED, SECURITY_ATTRIBUTES,
        TOKEN_QUERY, TOKEN_USER, TokenUser,
    },
    Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, CREATE_NEW, CreateDirectoryW, CreateFileW, DELETE,
        FILE_ALL_ACCESS, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_DELETE_CHILD, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, GetFileInformationByHandle, GetVolumeInformationByHandleW, OPEN_EXISTING,
        READ_CONTROL, WRITE_DAC, WRITE_OWNER,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

const SYSTEM: &str = "S-1-5-18";
const ADMINISTRATORS: &str = "S-1-5-32-544";
const TRUSTED_INSTALLER: &str = "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464";
const MAX_SECURITY_BYTES: u32 = 1024 * 1024;
const FILE_PERSISTENT_ACLS: u32 = 0x8;
const ALLOW_ACE: u8 = 0;
const DENY_ACE: u8 = 1;
const INHERIT_ONLY: u8 = 0x8;

struct LocalAllocation(*mut c_void);
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        // SAFETY: only successful LocalAlloc-owning Win32 outputs construct this
        // wrapper. LocalFree is its documented deallocator; ownership is unique.
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn wide_path(path: &Path) -> Result<Vec<u16>, ExternalError> {
    if !path.is_absolute() {
        return Err(ExternalError::Input);
    }
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.contains(&0) {
        return Err(ExternalError::Input);
    }
    wide.push(0);
    Ok(wide)
}
// Caller supplies a SID within a live OS-owned security descriptor/token, or a
// length-checked ACE. Conversion copies the string before that owner is dropped.
fn sid_string(sid: *mut c_void) -> Result<String, ExternalError> {
    if sid.is_null() {
        return Err(ExternalError::Trust);
    }
    // SAFETY: the callers guarantee the SID storage described above remains live.
    if unsafe { IsValidSid(sid) } == 0 {
        return Err(ExternalError::Trust);
    }
    let mut value = ptr::null_mut();
    // SAFETY: sid is validated and value is a writable pointer-sized output.
    if unsafe { ConvertSidToStringSidW(sid, &mut value) } == 0 || value.is_null() {
        return Err(ExternalError::Trust);
    }
    let _allocation = LocalAllocation(value.cast());
    // A string SID has at most 184 characters; allow room for future formatting.
    for len in 0..256 {
        // SAFETY: successful conversion returns a NUL-terminated UTF-16 string.
        if unsafe { *value.add(len) } == 0 {
            // SAFETY: the scan establishes the initialized string prefix.
            return String::from_utf16(unsafe { std::slice::from_raw_parts(value, len) })
                .map_err(|_| ExternalError::Trust);
        }
    }
    Err(ExternalError::Trust)
}
fn current_user() -> Result<String, ExternalError> {
    let mut raw = ptr::null_mut();
    // SAFETY: current process pseudo-handle needs no close, raw is valid output.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } == 0 {
        return Err(ExternalError::Storage);
    }
    // SAFETY: successful OpenProcessToken returned unique owned handle.
    let token = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut needed = 0;
    // SAFETY: documented sizing query, with a valid output size and no buffer.
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            ptr::null_mut(),
            0,
            &mut needed,
        );
    }
    if needed < size_of::<TOKEN_USER>() as u32 || needed > MAX_SECURITY_BYTES {
        return Err(ExternalError::Trust);
    }
    let mut words = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
    // SAFETY: usize storage is pointer-aligned and covers the requested bytes.
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            words.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } == 0
    {
        return Err(ExternalError::Storage);
    }
    // SAFETY: TokenUser initialized this aligned TOKEN_USER and its SID tail.
    let user = unsafe { &*words.as_ptr().cast::<TOKEN_USER>() };
    sid_string(user.User.Sid)
}
fn descriptor(sddl: &str) -> Result<LocalAllocation, ExternalError> {
    let wide: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
    let mut raw = ptr::null_mut();
    // SAFETY: wide is NUL-terminated and outputs refer to valid local variables.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide.as_ptr(),
            1,
            &mut raw,
            ptr::null_mut(),
        )
    } == 0
        || raw.is_null()
    {
        return Err(ExternalError::Storage);
    }
    Ok(LocalAllocation(raw))
}
fn private_descriptor() -> Result<LocalAllocation, ExternalError> {
    let user = current_user()?;
    // Explicit owner prevents elevated tokens assigning the Administrators group.
    descriptor(&format!("O:{user}D:P(A;OICI;FA;;;{user})(A;OICI;FA;;;SY)"))
}
fn attributes(sd: &LocalAllocation) -> SECURITY_ATTRIBUTES {
    SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd.0,
        bInheritHandle: 0,
    }
}
pub(super) fn make_dir(path: &Path) -> Result<(), ExternalError> {
    let path_wide = wide_path(path)?;
    let sd = private_descriptor()?;
    let attrs = attributes(&sd);
    // SAFETY: both terminated path and the complete SD remain live for the call.
    // CreateDirectory is exclusive; an existing directory is never adopted.
    if unsafe { CreateDirectoryW(path_wide.as_ptr(), &attrs) } == 0 {
        return Err(ExternalError::Storage);
    }
    validate(path, true, true)
}
pub(super) fn create_file(path: &Path) -> Result<File, ExternalError> {
    let path_wide = wide_path(path)?;
    let sd = private_descriptor()?;
    let attrs = attributes(&sd);
    // SAFETY: all input allocations live across call; CREATE_NEW never replaces.
    let raw = unsafe {
        CreateFileW(
            path_wide.as_ptr(),
            GENERIC_WRITE | READ_CONTROL | FILE_READ_ATTRIBUTES,
            0,
            &attrs,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(ExternalError::Storage);
    }
    // SAFETY: successful CreateFile returned a unique owned handle.
    let file = unsafe { File::from_raw_handle(raw) };
    validate_handle(&file, false, true)?;
    Ok(file)
}
pub(super) fn validate(path: &Path, directory: bool, private: bool) -> Result<(), ExternalError> {
    let path_wide = wide_path(path)?;
    // SAFETY: terminated path is live. OPEN_REPARSE_POINT avoids following the
    // final component; caller checks every preceding path component separately.
    let raw = unsafe {
        CreateFileW(
            path_wide.as_ptr(),
            READ_CONTROL | FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(ExternalError::Trust);
    }
    // SAFETY: successful CreateFile returned a unique owned handle.
    let file = unsafe { File::from_raw_handle(raw) };
    validate_handle(&file, directory, private)
}
fn trusted(sid: &str, user: &str, private: bool) -> bool {
    sid == user || sid == SYSTEM || (!private && matches!(sid, ADMINISTRATORS | TRUSTED_INSTALLER))
}
fn validate_handle(file: &File, directory: bool, private: bool) -> Result<(), ExternalError> {
    let handle = file.as_raw_handle();
    // SAFETY: these Win32 output structs consist entirely of zeroable integers.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { zeroed() };
    // SAFETY: owned live file and correctly sized output structure.
    if unsafe { GetFileInformationByHandle(handle, &mut info) } == 0
        || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0) != directory
    {
        return Err(ExternalError::Trust);
    }
    let mut flags = 0;
    // SAFETY: optional unused outputs are null; flags points to a live u32.
    if unsafe {
        GetVolumeInformationByHandleW(
            handle,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut flags,
            ptr::null_mut(),
            0,
        )
    } == 0
        || flags & FILE_PERSISTENT_ACLS == 0
    {
        return Err(ExternalError::Trust);
    }
    let (mut owner, mut acl, mut sd) = (ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
    // SAFETY: OS returns one LocalAlloc SD; owner and ACL borrow that allocation.
    if unsafe {
        GetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut sd,
        )
    } != 0
        || sd.is_null()
    {
        return Err(ExternalError::Trust);
    }
    let allocation = LocalAllocation(sd);
    let user = current_user()?;
    let owner = sid_string(owner)?;
    if (private && owner != user) || !trusted(&owner, &user, private) {
        return Err(ExternalError::Trust);
    }
    let (mut control, mut revision) = (0, 0);
    // SAFETY: SD is live and output pointers are correctly typed local variables.
    if unsafe { GetSecurityDescriptorControl(allocation.0, &mut control, &mut revision) } == 0
        || (private && control & SE_DACL_PROTECTED == 0)
    {
        return Err(ExternalError::Trust);
    }
    check_acl(acl, &user, private)
}
fn check_acl(acl: *mut ACL, user: &str, private: bool) -> Result<(), ExternalError> {
    if acl.is_null() {
        return Err(ExternalError::Trust);
    } // Null DACL grants everyone access.
    // SAFETY: ACL borrows the live descriptor obtained from GetSecurityInfo.
    if unsafe { IsValidAcl(acl) } == 0 {
        return Err(ExternalError::Trust);
    }
    // SAFETY: validated ACL header is initialized inside that allocation.
    let header = unsafe { &*acl };
    let base = acl as usize;
    let end = base
        .checked_add(usize::from(header.AclSize))
        .ok_or(ExternalError::Trust)?;
    let mut user_full_access = false;
    for index in 0..u32::from(header.AceCount) {
        let mut raw = ptr::null_mut();
        // SAFETY: index ranges over valid ACL entries and raw is writable output.
        if unsafe { GetAce(acl, index, &mut raw) } == 0 || raw.is_null() {
            return Err(ExternalError::Trust);
        }
        let address = raw as usize;
        if address < base
            || address
                .checked_add(size_of::<ACE_HEADER>())
                .is_none_or(|last| last > end)
        {
            return Err(ExternalError::Trust);
        }
        // SAFETY: preceding bounds check places the entire header in the ACL.
        let ace = unsafe { ptr::read_unaligned(raw.cast::<ACE_HEADER>()) };
        let size = usize::from(ace.AceSize);
        if size < 16 || address.checked_add(size).is_none_or(|last| last > end) {
            return Err(ExternalError::Trust);
        }
        // Fail closed for object-specific, callback, and unknown ACE forms.
        if !matches!(ace.AceType, ALLOW_ACE | DENY_ACE) {
            return Err(ExternalError::Trust);
        }
        if ace.AceFlags & INHERIT_ONLY != 0 || ace.AceType == DENY_ACE {
            continue;
        }
        // SAFETY: size and complete bounds were checked against the live ACL.
        let bytes = unsafe { std::slice::from_raw_parts(raw.cast::<u8>(), size) };
        let mask = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let sid_length = 8 + usize::from(bytes[9]) * 4;
        if bytes[9] > 15 || sid_length > size - 8 {
            return Err(ExternalError::Trust);
        }
        // SAFETY: fixed SID header and all subauthorities fit inside the ACE.
        let sid = sid_string(unsafe { raw.cast::<u8>().add(8).cast() })?;
        if sid == user && (mask & FILE_ALL_ACCESS == FILE_ALL_ACCESS || mask & GENERIC_ALL != 0) {
            user_full_access = true;
        }
        // Creating a sibling does not permit replacing an existing child on NTFS.
        // Ancestor owners and DELETE/DELETE_CHILD/ACL-owner mutation do permit it.
        let replacement_rights = DELETE | FILE_DELETE_CHILD | WRITE_DAC | WRITE_OWNER | GENERIC_ALL;
        if !trusted(&sid, user, private) && (private || mask & replacement_rights != 0) {
            return Err(ExternalError::Trust);
        }
    }
    if private && !user_full_access {
        return Err(ExternalError::Trust);
    }
    Ok(())
}

pub(crate) struct WorkingDirectory {
    path: std::path::PathBuf,
}
impl WorkingDirectory {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn close(mut self) -> Result<(), ExternalError> {
        // Job termination can precede the final descendant handle closing.
        // Bound this synchronous filesystem cleanup to one additional second.
        for attempt in 0..=20 {
            match std::fs::remove_dir_all(&self.path) {
                Ok(()) => {
                    self.path.clear();
                    return Ok(());
                }
                Err(error)
                    if attempt < 20 && matches!(error.raw_os_error(), Some(5 | 32 | 145)) =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(_) => return Err(ExternalError::Cleanup),
            }
        }
        Err(ExternalError::Cleanup)
    }
}
impl Drop for WorkingDirectory {
    fn drop(&mut self) {
        if !self.path.as_os_str().is_empty() {
            // Last-resort cleanup on early error/drop. The normal host path calls
            // close() and reports failures; Drop cannot propagate an error.
            match std::fs::remove_dir_all(&self.path) {
                Ok(()) | Err(_) => {}
            }
        }
    }
}
pub(super) fn working_directory() -> Result<WorkingDirectory, ExternalError> {
    let parent = std::env::temp_dir()
        .canonicalize()
        .map_err(|_| ExternalError::Storage)?;
    super::checked_path(&parent, false, true)?;
    let reservation = tempfile::Builder::new()
        .prefix("permesh-provider-")
        .tempfile_in(&parent)
        .map_err(|_| ExternalError::Storage)?;
    let path = reservation.path().to_owned();
    reservation.close().map_err(|_| ExternalError::Storage)?;
    // This exclusive directory creation either applies the private SD atomically
    // or fails if another process occupied the random name after close().
    make_dir(&path)?;
    Ok(WorkingDirectory { path })
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Security::Authorization::SetNamedSecurityInfoW;
    use windows_sys::Win32::Security::{
        GetSecurityDescriptorDacl, PROTECTED_DACL_SECURITY_INFORMATION,
    };
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    fn apply_dacl(path: &Path, sddl: &str) -> Result<(), ExternalError> {
        let sd = descriptor(sddl)?;
        let (mut present, mut defaulted, mut acl) = (0, 0, ptr::null_mut());
        // SAFETY: parsed SD remains alive, all output variables are valid.
        if unsafe { GetSecurityDescriptorDacl(sd.0, &mut present, &mut acl, &mut defaulted) } == 0
            || present == 0
        {
            return Err(ExternalError::Storage);
        }
        let wide = wide_path(path)?;
        // SAFETY: test-only ACL mutation, all pointers borrow live allocations.
        if unsafe {
            SetNamedSecurityInfoW(
                wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                acl,
                ptr::null(),
            )
        } != 0
        {
            return Err(ExternalError::Storage);
        }
        Ok(())
    }
    #[test]
    fn creation_is_private_even_under_permissive_inherited_acl() -> TestResult {
        let root = tempfile::tempdir()?;
        let user = current_user()?;
        apply_dacl(
            root.path(),
            &format!("D:P(A;OICI;FA;;;{user})(A;OICI;FA;;;WD)"),
        )?;
        let dir = root.path().join("private");
        make_dir(&dir)?;
        validate(&dir, true, true)?;
        let path = dir.join("file");
        drop(create_file(&path)?);
        validate(&path, false, true)?;
        assert!(make_dir(&dir).is_err());
        assert!(create_file(&path).is_err());
        Ok(())
    }
    #[test]
    fn foreign_file_write_and_ancestor_delete_child_are_rejected() -> TestResult {
        let root = tempfile::tempdir()?;
        let user = current_user()?;
        let dir = root.path().join("private");
        make_dir(&dir)?;
        let path = dir.join("file");
        drop(create_file(&path)?);
        apply_dacl(&path, &format!("D:P(A;;FA;;;{user})(A;;GW;;;WD)"))?;
        assert!(validate(&path, false, true).is_err());
        apply_dacl(&dir, &format!("D:P(A;;FA;;;{user})(A;;0x40;;;WD)"))?;
        assert!(validate(&dir, true, false).is_err());
        Ok(())
    }
    #[test]
    fn broad_read_registry_dacl_is_rejected() -> TestResult {
        let root = tempfile::tempdir()?;
        let dir = root.path().join("private");
        make_dir(&dir)?;
        let user = current_user()?;
        apply_dacl(&dir, &format!("D:P(A;;FA;;;{user})(A;;GR;;;WD)"))?;
        assert!(validate(&dir, true, true).is_err());
        Ok(())
    }
    #[test]
    fn working_directory_is_private_and_cleanup_is_observable() -> TestResult {
        let directory = working_directory()?;
        let path = directory.path().to_owned();
        validate(&path, true, true)?;
        directory.close()?;
        assert!(!path.exists());
        Ok(())
    }
    #[test]
    fn foreign_write_dac_and_null_dacls_are_rejected() -> TestResult {
        let root = tempfile::tempdir()?;
        let dir = root.path().join("private");
        make_dir(&dir)?;
        let user = current_user()?;
        apply_dacl(&dir, &format!("D:P(A;;FA;;;{user})(A;;0x40000;;;WD)"))?;
        assert!(validate(&dir, true, false).is_err());
        apply_dacl(&dir, "D:NO_ACCESS_CONTROL")?;
        assert!(validate(&dir, true, false).is_err());
        Ok(())
    }
    #[test]
    fn inherit_only_foreign_ace_does_not_grant_ancestor_replacement() -> TestResult {
        let root = tempfile::tempdir()?;
        let dir = root.path().join("private");
        make_dir(&dir)?;
        let user = current_user()?;
        apply_dacl(&dir, &format!("D:P(A;;FA;;;{user})(A;OICIIO;FA;;;WD)"))?;
        validate(&dir, true, false)?;
        // A child created by our API blocks these permissive inherited entries.
        let child = dir.join("child");
        make_dir(&child)?;
        validate(&child, true, true)?;
        Ok(())
    }
    #[test]
    fn manifest_acl_tampering_invalidates_trust() -> TestResult {
        let root = tempfile::tempdir()?;
        let root = root.path().canonicalize()?;
        let source = std::env::current_exe()?;
        let registry = super::super::Registry::new(root.join("registry"))?;
        let digest = super::super::inspect(&source)?.sha256;
        let registration = registry.trust(&source, "example", &digest, &[])?;
        let user = current_user()?;
        apply_dacl(
            &root.join("registry/example/manifest.json"),
            &format!("D:P(A;;FA;;;{user})(A;;GW;;;WD)"),
        )?;
        assert!(registry.verify(&registration).is_err());
        Ok(())
    }
    #[test]
    fn working_directory_cleanup_waits_for_transient_open_file() -> TestResult {
        let directory = working_directory()?;
        let path = directory.path().to_owned();
        let file = create_file(&path.join("held"))?;
        let release = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(100));
            drop(file);
        });
        directory.close()?;
        assert!(release.join().is_ok());
        assert!(!path.exists());
        Ok(())
    }
}
