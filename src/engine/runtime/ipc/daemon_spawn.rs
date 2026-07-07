use crate::runtime::daemon::report::{
    READY_FILE_ENV, READY_STDOUT_ENV, StartupReply, StartupReporter,
};
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
use std::process::{Command, Stdio};
mod process_flags;
const DAEMON_SUBCOMMAND: &str = "daemon";
const LAUNCHER_SUBCOMMAND: &str = "internal-launch-daemon";
#[derive(Clone, Copy)]
pub(super) enum StartupProcess {
    Launcher,
    Daemon,
}
pub(crate) fn spawn_daemon(service_name: &str, timeout: Duration) -> Result<()> {
    let startup_file = startup_file();
    let mut launcher = spawn_launcher_process(service_name, &startup_file)
        .context("failed to spawn daemon launcher")?;
    wait_for_startup_file(&startup_file, &mut launcher, timeout)
}
pub(crate) fn run_launcher(service_name: &str) -> Result<()> {
    let mut startup_reporter = StartupReporter::from_env();
    match spawn_daemon_child(service_name) {
        Ok(()) => Ok(()),
        Err(error) => {
            startup_reporter.failed(&error);
            Err(error)
        }
    }
}
fn spawn_daemon_child(service_name: &str) -> Result<()> {
    if process_flags::needs_shell_parent_daemon_spawn() {
        return spawn_daemon_child_through_shell_parent(service_name);
    }
    let _daemon = spawn_daemon_process(service_name).context("failed to spawn detached daemon")?;
    Ok(())
}
fn spawn_daemon_child_through_shell_parent(service_name: &str) -> Result<()> {
    let current_exe = std::env::current_exe().context("failed to locate current executable")?;
    let mut command = Command::new(current_exe);
    command.envs(std::env::vars_os());
    command
        .arg(DAEMON_SUBCOMMAND)
        .env("FUNCTERM_DAEMON_SERVICE_NAME", service_name)
        .env_remove(READY_STDOUT_ENV)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let _daemon = process_flags::spawn_with_shell_parent(command)
        .context("failed to spawn detached daemon")?;
    Ok(())
}
fn startup_file() -> std::path::PathBuf {
    std::env::temp_dir()
        .join("functerm")
        .join("daemon-startup")
        .join(format!("{}-{}.json", std::process::id(), nanoid::nanoid!()))
}
fn spawn_launcher_process(
    service_name: &str,
    startup_file: &std::path::Path,
) -> Result<std::process::Child> {
    let mut command = startup_command(LAUNCHER_SUBCOMMAND)?;
    command
        .env("FUNCTERM_DAEMON_SERVICE_NAME", service_name)
        .env(READY_FILE_ENV, startup_file)
        .stdout(Stdio::null());
    process_flags::spawn_detached(command, StartupProcess::Launcher)
        .with_context(|| format!("failed to spawn {LAUNCHER_SUBCOMMAND}"))
}
fn spawn_daemon_process(service_name: &str) -> Result<std::process::Child> {
    let mut command = startup_command(DAEMON_SUBCOMMAND)?;
    command
        .env("FUNCTERM_DAEMON_SERVICE_NAME", service_name)
        .stdout(Stdio::null());
    process_flags::spawn_detached(command, StartupProcess::Daemon)
        .with_context(|| format!("failed to spawn {DAEMON_SUBCOMMAND}"))
}
fn startup_command(subcommand: &str) -> Result<Command> {
    let current_exe = std::env::current_exe().context("failed to locate current executable")?;
    let mut command = Command::new(current_exe);
    command
        .arg(subcommand)
        .env_remove(READY_STDOUT_ENV)
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    Ok(command)
}
fn wait_for_startup_file(
    path: &std::path::Path,
    launcher: &mut std::process::Child,
    timeout: Duration,
) -> Result<()> {
    if crate::runtime::session::records::wait_for_path(path, timeout)? {
        return read_startup_file(path);
    }
    if let Some(status) = launcher
        .try_wait()
        .context("failed to poll daemon launcher startup status")?
    {
        bail!("daemon launcher exited before startup report with status {status}");
    }
    bail!("daemon startup report was not published within {timeout:?}");
}
fn read_startup_file(path: &std::path::Path) -> Result<()> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read daemon startup report {}", path.display()))?;
    match sonic_rs::from_str::<StartupReply>(text.trim_end())
        .context("failed to parse daemon startup report")?
    {
        StartupReply::Ready => Ok(()),
        StartupReply::AlreadyRunning { service_name } => {
            Err(crate::runtime::daemon_lock::DaemonAlreadyRunning::new(service_name).into())
        }
        StartupReply::Failed { message } => bail!(message),
    }
}
#[cfg(test)]
mod tests {
    use core::time::Duration;
    use std::process::Stdio;
    use std::thread;
    #[test]
    fn startup_wait_uses_daemon_report_after_launcher_exits() {
        let path = crate::test_fs::temp_case("startup-wait").join("ready.json");
        let mut launcher = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--help")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let report_path = path.clone();
        let report_worker = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            std::fs::create_dir_all(report_path.parent().unwrap()).unwrap();
            let text =
                sonic_rs::to_string(&crate::runtime::daemon::report::StartupReply::Ready).unwrap();
            std::fs::write(report_path, text).unwrap();
        });
        super::wait_for_startup_file(&path, &mut launcher, Duration::from_secs(2)).unwrap();
        report_worker.join().unwrap();
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
