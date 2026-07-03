use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::io::Write as _;
pub(crate) const READY_STDOUT_ENV: &str = "FUNCTERM_DAEMON_READY_STDOUT";
#[derive(Deserialize, Serialize)]
pub(crate) enum StartupReply {
    Ready,
    AlreadyRunning { service_name: String },
    Failed { message: String },
}
pub(super) struct StartupReporter {
    enabled: bool,
}
impl StartupReporter {
    pub(super) fn from_env() -> Self {
        Self {
            enabled: std::env::var_os(READY_STDOUT_ENV).is_some(),
        }
    }
    pub(super) fn ready(&mut self) -> Result<()> {
        self.send(&StartupReply::Ready)
    }
    pub(super) fn failed(&mut self, error: &anyhow::Error) {
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
        if !self.enabled {
            return Ok(());
        }
        self.enabled = false;
        let mut text =
            sonic_rs::to_string(&reply).context("failed to serialize daemon startup status")?;
        text.push('\n');
        std::io::stdout()
            .write_all(text.as_bytes())
            .context("failed to write daemon startup status")?;
        std::io::stdout()
            .flush()
            .context("failed to flush daemon startup status")
    }
}
