use anyhow::{Context as _, Result};
use exacl::{AclEntry, Perm};
use nix::unistd::User;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
pub(super) fn secure_tab(tab_root: &Path) -> Result<()> {
    fs_err::set_permissions(tab_root, std::fs::Permissions::from_mode(0o700))
        .context("failed to restrict Tab runtime access")
}
pub(super) fn delegate(
    runtime_root: &Path,
    generation_root: &Path,
    tab_root: &Path,
    shim_dir: &Path,
    account: &User,
) -> Result<()> {
    for directory in tab_root
        .ancestors()
        .skip(1)
        .chain(shim_dir.ancestors().skip(1))
        .filter(|path| path.starts_with(runtime_root))
    {
        grant(directory, &account.name, Perm::EXECUTE, false)?;
    }
    anyhow::ensure!(
        generation_root.starts_with(runtime_root),
        "Tab generation is outside Daemon runtime"
    );
    let permissions = delegated_permissions();
    grant(tab_root, &account.name, permissions, true)?;
    grant(shim_dir, &account.name, permissions, true)?;
    Ok(())
}
fn grant(path: &Path, user: &str, permissions: Perm, inherit: bool) -> Result<()> {
    let mut acl = exacl::getfacl(path, None)
        .with_context(|| format!("failed to read runtime ACL {}", path.display()))?;
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    acl.retain(|entry| entry.kind != exacl::AclEntryKind::Mask);
    merge_entry(&mut acl, AclEntry::allow_user(user, permissions, None));
    if inherit {
        add_inheritance(&mut acl, user, permissions);
    }
    exacl::setfacl(&[path], &acl, None)
        .with_context(|| format!("failed to grant shell access to runtime {}", path.display()))
}
fn merge_entry(acl: &mut Vec<AclEntry>, entry: AclEntry) {
    if let Some(existing) = acl.iter_mut().find(|existing| {
        existing.kind == entry.kind && existing.name == entry.name && existing.flags == entry.flags
    }) {
        existing.perms |= entry.perms;
    } else {
        acl.push(entry);
    }
}
#[cfg(not(target_os = "macos"))]
const fn delegated_permissions() -> Perm {
    Perm::READ.union(Perm::WRITE).union(Perm::EXECUTE)
}
#[cfg(target_os = "macos")]
const fn delegated_permissions() -> Perm {
    Perm::READ
        .union(Perm::WRITE)
        .union(Perm::EXECUTE)
        .union(Perm::APPEND)
        .union(Perm::DELETE)
        .union(Perm::DELETE_CHILD)
        .union(Perm::READATTR)
        .union(Perm::WRITEATTR)
        .union(Perm::READEXTATTR)
        .union(Perm::WRITEEXTATTR)
}
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(crate) fn inherit_publication_acl(path: &Path) -> Result<()> {
    let parent = path.parent().context("publication file has no parent")?;
    let defaults = exacl::getfacl(parent, None)?
        .into_iter()
        .filter(|entry| {
            entry.flags.contains(exacl::Flag::DEFAULT) && entry.kind != exacl::AclEntryKind::Mask
        })
        .map(|mut entry| {
            entry.flags = exacl::Flag::empty();
            entry.perms &= Perm::READ | Perm::WRITE;
            entry
        })
        .collect::<Vec<_>>();
    if !defaults.is_empty() {
        exacl::setfacl(&[path], &defaults, None)
            .context("failed to preserve inherited publication file permissions")?;
    }
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn add_inheritance(acl: &mut Vec<AclEntry>, user: &str, permissions: Perm) {
    let mut defaults = exacl::from_mode(0o700);
    defaults.push(AclEntry::allow_user(user, permissions, None));
    for entry in &mut defaults {
        entry.flags = exacl::Flag::DEFAULT;
    }
    acl.retain(|entry| !entry.flags.contains(exacl::Flag::DEFAULT));
    acl.extend(defaults);
}
#[cfg(target_os = "macos")]
fn add_inheritance(acl: &mut Vec<AclEntry>, user: &str, permissions: Perm) {
    merge_entry(
        acl,
        AclEntry::allow_user(
            user,
            permissions,
            exacl::Flag::FILE_INHERIT | exacl::Flag::DIRECTORY_INHERIT,
        ),
    );
}
