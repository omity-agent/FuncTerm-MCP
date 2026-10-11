use crate::runtime::config::IpcSettings;
use anyhow::{Context as _, Result, ensure};
use interprocess::local_socket::ListenerOptions;
use interprocess::os::windows::local_socket::ListenerOptionsExt as _;
use interprocess::os::windows::security_descriptor::{
    AsSecurityDescriptorExt as _, SecurityDescriptor,
};
use widestring::U16CString;
pub(super) fn configure<'name>(
    options: ListenerOptions<'name>,
    settings: &IpcSettings,
) -> Result<ListenerOptions<'name>> {
    let label = U16CString::from_str(&settings.windows_security_label)
        .context("Windows IPC security label contains a NUL character")?;
    let descriptor =
        SecurityDescriptor::deserialize(&label).context("invalid Windows IPC security label")?;
    ensure!(
        descriptor.dacl()?.is_none()
            && descriptor.owner()?.0.is_null()
            && descriptor.group()?.0.is_null(),
        "windows_security_label can only configure the pipe SACL, not its access principals"
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
    Ok(options.security_descriptor(secured))
}
