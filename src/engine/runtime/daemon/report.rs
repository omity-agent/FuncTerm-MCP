use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::io::Write as _;
use std::path::PathBuf;
pub(crate) const READY_STDOUT_ENV: &str = "FUNCTERM_DAEMON_READY_STDOUT";
pub(crate) const READY_FILE_ENV: &str = "FUNCTERM_DAEMON_READY_FILE";
#[derive(Deserialize, Serialize)]
pub(crate) enum StartupReply {
    Ready,
    AlreadyRunning { service_name: String },
    Failed { message: String },
}
pub(crate) struct StartupReporter {
    stdout_enabled: bool,
    file: Option<PathBuf>,
}
impl StartupReporter {
    pub(crate) fn from_env() -> Self {
        Self {
            stdout_enabled: std::env::var_os(READY_STDOUT_ENV).is_some(),
            file: std::env::var_os(READY_FILE_ENV).map(PathBuf::from),
        }
    }
    pub(crate) fn ready(&mut self) -> Result<()> {
        self.send(&StartupReply::Ready)
    }
    pub(crate) fn failed(&mut self, error: &anyhow::Error) {
        let reply = crate::runtime::daemon_lock::already_running_service_name(error).map_or_else(
            || StartupReply::Failed {
                message: format!("{error:#}"),
            },
            |service_name| StartupReply::AlreadyRunning {
                service_name: service_name.to_owned(),
            },
        );
        if let Err(send_error) = self.send(&reply) {
            eprintln!("failed to send daemon startup error: {send_error:#}");
        }
    }
    fn send(&mut self, reply: &StartupReply) -> Result<()> {
        if !self.stdout_enabled && self.file.is_none() {
            return Ok(());
        }
        let mut text =
            sonic_rs::to_string(&reply).context("failed to serialize daemon startup status")?;
        text.push('\n');
        if self.stdout_enabled {
            self.stdout_enabled = false;
            std::io::stdout()
                .write_all(text.as_bytes())
                .context("failed to write daemon startup status")?;
            std::io::stdout()
                .flush()
                .context("failed to flush daemon startup status")?;
        }
        if let Some(path) = self.file.take() {
            write_startup_file(&path, &text)?;
        }
        Ok(())
    }
}
fn write_startup_file(path: &std::path::Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create startup report directory {}",
                parent.display()
            )
        })?;
    }
    let temp_path = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&temp_path, text)
        .with_context(|| format!("failed to write startup report {}", temp_path.display()))?;
    std::fs::rename(&temp_path, path)
        .with_context(|| format!("failed to publish startup report {}", path.display()))
}
