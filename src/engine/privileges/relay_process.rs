use super::PtyProcess;
use super::access_token::normal_parent;
use super::tree_guard::HostedChild;
use crate::shell::ShellStartup;
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use portable_pty::{ChildKiller as _, PtySize};
use std::path::Path;
use windows_spawn::{Command, CreationFlags, Job, ParentProcess, SpawnOptions, Stdio};
pub(crate) fn spawn(
    executable: &str,
    startup: ShellStartup,
    starting_directory: &Path,
    size: PtySize,
) -> Result<PtyProcess> {
    let parent = ParentProcess::from_handle(normal_parent()?)?;
    let helper = std::env::current_exe().context("failed to resolve ordinary PTY host")?;
    let mut command = Command::new(helper);
    command
        .args([
            "internal-pty-host",
            "--rows",
            &size.rows.to_string(),
            "--cols",
            &size.cols.to_string(),
            "--",
            executable,
        ])
        .args(startup.args);
    command
        .env_clear()
        .envs(startup.env)
        .current_dir(starting_directory);
    let (reader, output) = os_pipe::pipe().context("failed to create ordinary PTY output pipe")?;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::from_borrowed(&output)?)
        .stderr(Stdio::from_borrowed(&output)?);
    let job = Arc::new(Job::create().context("failed to create ordinary PTY host Job")?);
    job.set_kill_on_close(true)
        .context("failed to guard ordinary PTY descendants")?;
    let options = SpawnOptions::new()
        .parent_process(&parent)
        .job(&job)
        .creation_flags(CreationFlags::DETACHED_PROCESS);
    let mut child = command
        .spawn_with(options)
        .context("failed to launch ordinary PTY host")?;
    let writer = child
        .stdin
        .take()
        .context("ordinary PTY host has no input pipe")?;
    let hosted = HostedChild::new(child, job);
    let tree_killer = hosted.clone_killer();
    Ok(PtyProcess {
        child: Box::new(hosted),
        reader: Box::new(reader),
        writer: Box::new(writer),
        slave: None,
        master: None,
        tree_killer: Some(tree_killer),
    })
}
