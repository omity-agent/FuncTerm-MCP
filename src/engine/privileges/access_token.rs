use super::desktop_host;
use crate::runtime::config::Settings;
use anyhow::{Context as _, Result, ensure};
use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle};
use windows::Win32::Foundation::{HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{
    GetTokenInformation, SECURITY_MAX_SID_SIZE, TOKEN_ELEVATION, TOKEN_INFORMATION_CLASS,
    TOKEN_QUERY, TOKEN_USER, TokenElevation, TokenUser,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcessToken, PROCESS_CREATE_PROCESS, PROCESS_DUP_HANDLE,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::core::PWSTR;
pub(crate) struct ShellIdentity {
    reduced: bool,
}
impl ShellIdentity {
    pub(crate) fn resolve(elevated: bool, _settings: &Settings) -> Result<Self> {
        let token = current_token()?;
        let privileged = is_elevated(&token)?;
        ensure!(
            !elevated || privileged,
            "elevated Tab requires the Daemon to already have administrator permissions"
        );
        let reduced = privileged && !elevated;
        if reduced {
            let _normal_parent = normal_parent()?;
        }
        Ok(Self { reduced })
    }
    pub(crate) const fn needs_normal_host(&self) -> bool {
        self.reduced
    }
}
pub(super) fn current_token() -> Result<OwnedHandle> {
    let mut handle = HANDLE::default();
    let process = unsafe { GetCurrentProcess() };
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &raw mut handle) }
        .context("failed to open Daemon process token")?;
    Ok(unsafe { OwnedHandle::from_raw_handle(handle.0) })
}
fn token_information<T>(token: &OwnedHandle, class: TOKEN_INFORMATION_CLASS) -> Result<T>
where
    T: Default,
{
    let mut value = T::default();
    let mut returned = 0;
    let size = u32::try_from(core::mem::size_of::<T>())?;
    unsafe {
        GetTokenInformation(
            HANDLE(token.as_raw_handle()),
            class,
            Some(core::ptr::from_mut(&mut value).cast()),
            size,
            &raw mut returned,
        )
    }
    .context("failed to query process token")?;
    ensure!(
        returned == size,
        "unexpected process token information size"
    );
    Ok(value)
}
pub(super) fn is_elevated(token: &OwnedHandle) -> Result<bool> {
    Ok(token_information::<TOKEN_ELEVATION>(token, TokenElevation)?.TokenIsElevated != 0)
}
pub(super) fn normal_parent() -> Result<OwnedHandle> {
    let parent = desktop_host::open(
        PROCESS_CREATE_PROCESS | PROCESS_DUP_HANDLE | PROCESS_QUERY_LIMITED_INFORMATION,
    )?;
    let mut handle = HANDLE::default();
    unsafe { OpenProcessToken(HANDLE(parent.as_raw_handle()), TOKEN_QUERY, &raw mut handle) }
        .context("failed to open ordinary desktop user token")?;
    let token = unsafe { OwnedHandle::from_raw_handle(handle.0) };
    ensure!(
        !is_elevated(&token)?,
        "cannot create a normal Tab: the Windows desktop is running as administrator"
    );
    Ok(parent)
}
pub(crate) fn user_sid() -> Result<String> {
    let token = current_token()?;
    let capacity = core::mem::size_of::<TOKEN_USER>() + usize::try_from(SECURITY_MAX_SID_SIZE)?;
    let mut storage = vec![0_usize; capacity.div_ceil(core::mem::size_of::<usize>())];
    let size = u32::try_from(core::mem::size_of_val(storage.as_slice()))?;
    let mut returned = 0;
    unsafe {
        GetTokenInformation(
            HANDLE(token.as_raw_handle()),
            TokenUser,
            Some(storage.as_mut_ptr().cast()),
            size,
            &raw mut returned,
        )
    }
    .context("failed to query Daemon user SID")?;
    ensure!(
        usize::try_from(returned)? >= core::mem::size_of::<TOKEN_USER>() && returned <= size,
        "invalid Daemon user token information size"
    );
    let user = unsafe { storage.as_ptr().cast::<TOKEN_USER>().read() };
    let mut text = PWSTR::null();
    unsafe { ConvertSidToStringSidW(user.User.Sid, &raw mut text) }
        .context("failed to format Daemon user SID")?;
    let decoded = unsafe { text.to_string() };
    let freed = unsafe { LocalFree(Some(HLOCAL(text.0.cast()))) };
    ensure!(
        freed.0.is_null(),
        "failed to release Daemon SID string allocation"
    );
    decoded.context("Daemon user SID is not valid UTF-16")
}
