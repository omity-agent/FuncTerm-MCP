use super::EnvironmentSnapshot;
use anyhow::{Context as _, Result, bail};
use core::ffi::c_void;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt as _;
use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Security::{TOKEN_DUPLICATE, TOKEN_QUERY};
use windows::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
pub(super) fn capture_user_environment() -> Result<EnvironmentSnapshot> {
    let token = current_process_token()?;
    let mut block = core::ptr::null_mut();
    let token_handle = HANDLE(token.as_raw_handle());
    unsafe { CreateEnvironmentBlock(&raw mut block, Some(token_handle), false) }
        .context("CreateEnvironmentBlock failed without inheritance")?;
    if block.is_null() {
        bail!("CreateEnvironmentBlock returned a null environment block");
    }
    let decoded = decode_environment_block(block.cast_const());
    unsafe { DestroyEnvironmentBlock(block.cast_const()) }
        .context("DestroyEnvironmentBlock failed")?;
    Ok(EnvironmentSnapshot::from_variables(decoded?))
}
fn current_process_token() -> Result<OwnedHandle> {
    let mut token = HANDLE::default();
    let process = unsafe { GetCurrentProcess() };
    unsafe { OpenProcessToken(process, TOKEN_QUERY | TOKEN_DUPLICATE, &raw mut token) }
        .context("OpenProcessToken failed")?;
    let owned = unsafe { OwnedHandle::from_raw_handle(token.0) };
    Ok(owned)
}
fn decode_environment_block(block: *const c_void) -> Result<Vec<(OsString, OsString)>> {
    let mut variables = Vec::new();
    let mut cursor = block.cast::<u16>();
    loop {
        let start = cursor;
        while unsafe { cursor.read() } != 0 {
            cursor = unsafe { cursor.add(1) };
        }
        if cursor == start {
            return Ok(variables);
        }
        let length = usize::try_from(unsafe { cursor.offset_from(start) })
            .context("environment entry length overflowed")?;
        let entry = unsafe { core::slice::from_raw_parts(start, length) };
        variables.push(decode_entry(entry)?);
        cursor = unsafe { cursor.add(1) };
    }
}
fn decode_entry(entry: &[u16]) -> Result<(OsString, OsString)> {
    let Some(separator) = entry
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, unit)| (*unit == u16::from(b'=')).then_some(index))
    else {
        bail!("CreateEnvironmentBlock returned an entry without a name separator");
    };
    let name = entry
        .get(..separator)
        .context("environment entry name is out of bounds")?;
    let value = entry
        .get(separator + 1..)
        .context("environment entry value is out of bounds")?;
    Ok((OsString::from_wide(name), OsString::from_wide(value)))
}
#[cfg(test)]
mod tests {
    use super::decode_entry;
    use std::ffi::OsString;
    #[test]
    fn ordinary_environment_entry_is_decoded() {
        let entry = "PATH=C:\\Windows".encode_utf16().collect::<Vec<_>>();
        assert_eq!(
            decode_entry(&entry).unwrap(),
            (OsString::from("PATH"), OsString::from("C:\\Windows"))
        );
    }
    #[test]
    fn hidden_drive_environment_entry_is_decoded() {
        let entry = "=C:=C:\\work".encode_utf16().collect::<Vec<_>>();
        assert_eq!(
            decode_entry(&entry).unwrap(),
            (OsString::from("=C:"), OsString::from("C:\\work"))
        );
    }
}
