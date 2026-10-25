use crate::runtime::config::IpcSettings;
use anyhow::{Context as _, Result, ensure};
use interprocess::os::windows::security_descriptor::{
    AsSecurityDescriptor as _, AsSecurityDescriptorExt as _, SecurityDescriptor,
};
use std::os::windows::io::{FromRawHandle as _, OwnedHandle};
use widestring::U16CString;
use windows::Win32::{CreateMutexW, SECURITY_ATTRIBUTES};
use windows::core::PCWSTR;
pub(super) fn descriptor(settings: &IpcSettings) -> Result<SecurityDescriptor> {
    let label = U16CString::from_str(&settings.windows_security_label)
        .context("Windows IPC security label contains a NUL character")?;
    let descriptor =
        SecurityDescriptor::deserialize(&label).context("invalid Windows IPC security label")?;
    ensure!(
        descriptor.dacl()?.is_none()
            && descriptor.owner()?.0.is_null()
            && descriptor.group()?.0.is_null(),
        "windows_security_label can only configure the IPC object SACL, not its access principals"
    );
    ensure!(
        descriptor.sacl()?.is_some(),
        "Windows IPC security label has no SACL"
    );
    let user = crate::engine::privileges::user_sid()?;
    ensure!(
        settings.windows_access_control.contains("{user_sid}"),
        "windows_access_control has no Daemon user SID placeholder"
    );
    let policy = settings.windows_access_control.replace("{user_sid}", &user);
    let complete = U16CString::from_str(format!("{policy}{}", settings.windows_security_label))?;
    let secured = SecurityDescriptor::deserialize(&complete)
        .context("invalid Windows IPC access control policy")?;
    ensure!(
        secured.dacl()?.is_some_and(|entry| !entry.0.is_null()),
        "Windows IPC access control policy has no DACL"
    );
    Ok(secured)
}
pub(super) fn create_mutex(name: &str, settings: &IpcSettings) -> Result<OwnedHandle> {
    let security = descriptor(settings)?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(core::mem::size_of::<SECURITY_ATTRIBUTES>())?,
        lpSecurityDescriptor: security.as_sd().cast_mut(),
        bInheritHandle: false.into(),
    };
    let global_name = U16CString::from_str(format!("Global\\{name}"))?;
    let handle = unsafe {
        CreateMutexW(
            Some(core::ptr::from_ref(&attributes).cast_mut()),
            false,
            PCWSTR(global_name.as_ptr()),
        )
    };
    if handle.is_null() {
        return Err(windows::core::Error::from_thread())
            .context("failed to create secured Windows daemon mutex");
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
}
