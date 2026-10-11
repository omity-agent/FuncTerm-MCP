use super::workspace_acl;
use crate::runtime::config::Settings;
use anyhow::{Context as _, Result, ensure};
use nix::unistd::{Uid, User};
use std::ffi::OsString;
use std::os::unix::process::CommandExt as _;
use std::path::Path;
pub(crate) struct ShellIdentity {
    account: Option<User>,
}
impl ShellIdentity {
    pub(crate) fn resolve(elevated: bool, settings: &Settings) -> Result<Self> {
        let privileged = Uid::effective().is_root();
        ensure!(
            !elevated || privileged,
            "elevated Tab requires the Daemon to already run as Root"
        );
        let account = if elevated {
            Some(account_by_uid(Uid::from_raw(0))?)
        } else if privileged {
            let normal = normal_account(settings)?;
            ensure!(!normal.uid.is_root(), "normal Tab cannot use the Root user");
            Some(normal)
        } else {
            None
        };
        Ok(Self { account })
    }
    pub(super) fn helper_arguments(&self) -> Option<Vec<OsString>> {
        self.account
            .as_ref()
            .map(|account| vec!["--uid".into(), account.uid.as_raw().to_string().into()])
    }
    pub(crate) fn prepare_workspace(
        &self,
        runtime_root: &Path,
        generation_root: &Path,
        tab_root: &Path,
        shim_dir: &Path,
    ) -> Result<()> {
        workspace_acl::secure_tab(tab_root)?;
        if let Some(account) = &self.account
            && !account.uid.is_root()
        {
            workspace_acl::delegate(runtime_root, generation_root, tab_root, shim_dir, account)?;
        }
        Ok(())
    }
}
fn account_by_uid(uid: Uid) -> Result<User> {
    User::from_uid(uid)?.with_context(|| format!("Unix user {} does not exist", uid.as_raw()))
}
fn normal_account(settings: &Settings) -> Result<User> {
    if !settings.unix_normal_user.is_empty() {
        return User::from_name(&settings.unix_normal_user)?
            .with_context(|| format!("Unix user {} does not exist", settings.unix_normal_user));
    }
    let real_uid = Uid::current();
    if !real_uid.is_root() {
        return account_by_uid(real_uid);
    }
    let sudo_uid = std :: env :: var ("SUDO_UID") . context ("cannot identify a normal Unix user; configure unix_normal_user or start the Daemon via sudo") ? ;
    let parsed = sudo_uid
        .parse()
        .context("Daemon SUDO_UID is not a valid Unix UID")?;
    account_by_uid(Uid::from_raw(parsed))
}
pub(super) fn run(uid: u32, arguments: &[OsString]) -> Result<()> {
    ensure!(
        Uid::effective().is_root(),
        "Unix identity switch requires Root permissions"
    );
    let account = account_by_uid(Uid::from_raw(uid))?;
    privdrop::PrivDrop::default()
        .user(&account.name)
        .include_default_supplementary_groups()
        .apply()
        .context("failed to switch shell user and groups")?;
    ensure!(
        Uid::current() == account.uid && Uid::effective() == account.uid,
        "shell user switch did not set the expected Unix UID"
    );
    let (executable, parameters) = arguments.split_first().context("shell command is empty")?;
    let error = std::process::Command::new(executable)
        .args(parameters)
        .env("HOME", &account.dir)
        .env("USER", &account.name)
        .env("LOGNAME", &account.name)
        .exec();
    Err(error).context("failed to execute shell after switching Unix identity")
}
