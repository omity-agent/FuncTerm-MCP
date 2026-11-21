use crate::runtime::protocol::EnvironmentSnapshot;
use std::ffi::OsString;
use std::path::PathBuf;
use windows_registry::{Key, LOCAL_MACHINE};
const MSI_KEY: &str = r"Software\Microsoft\PowerShellCore\InstalledVersions";
pub(super) fn discover(configured: &[String], environment: &EnvironmentSnapshot) -> Vec<OsString> {
    let stable = includes(configured, "pwsh", "pwsh.exe");
    let preview = includes(configured, "pwsh-preview", "pwsh-preview.exe");
    let mut paths = Vec::new();
    if let Some(local) = environment.value("LOCALAPPDATA").map(PathBuf::from)
        && local.is_absolute()
    {
        let aliases = local.join(r"Microsoft\WindowsApps");
        if stable {
            paths.push(
                aliases
                    .join(r"Microsoft.PowerShell_8wekyb3d8bbwe\pwsh.exe")
                    .into_os_string(),
            );
        }
        if preview {
            paths.push(
                aliases
                    .join(r"Microsoft.PowerShellPreview_8wekyb3d8bbwe\pwsh-preview.exe")
                    .into_os_string(),
            );
        }
    }
    if stable {
        paths.extend(registered_installations());
    }
    paths
}
fn includes(configured: &[String], stem: &str, filename: &str) -> bool {
    configured
        .iter()
        .any(|name| name.eq_ignore_ascii_case(stem) || name.eq_ignore_ascii_case(filename))
}
fn registered_installations() -> Vec<OsString> {
    let views = [
        LOCAL_MACHINE.options().read().wow64_64().open(MSI_KEY),
        LOCAL_MACHINE.options().read().wow64_32().open(MSI_KEY),
    ];
    let mut paths = Vec::new();
    for view in views {
        let Ok(root) = view.inspect_err(|error| {
            tracing::debug!(%error, "PowerShell MSI registry key is unavailable");
        }) else {
            continue;
        };
        let Ok(names) = root.keys().inspect_err(|error| {
            tracing::debug!(%error, "failed to list registered PowerShell installations");
        }) else {
            continue;
        };
        for name in names {
            let location = install_location(&root, &name).inspect_err(|error| {
                tracing::debug!(%error, %name, "failed to read registered PowerShell location");
            });
            if let Ok(directory) = location
                && directory.is_absolute()
            {
                paths.push(directory.join("pwsh.exe").into_os_string());
            }
        }
    }
    paths
}
fn install_location(key: &Key, name: &str) -> windows_registry::Result<PathBuf> {
    Ok(PathBuf::from(
        key.open(name)?.get_string("InstallLocation")?,
    ))
}
