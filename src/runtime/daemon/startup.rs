use anyhow::{Context as _, Result};
use ipc_channel::ipc::IpcSender;
use serde::{Deserialize, Serialize};
pub(crate) const READY_ENDPOINT_ENV: &str = "FUNCTERM_DAEMON_READY_ENDPOINT";
#[derive(Deserialize, Serialize)]
pub(crate) enum StartupReply {
    Ready,
    AlreadyRunning { service_name: String },
    Failed { message: String },
}
pub(super) struct StartupReporter {
    endpoint_name: Option<String>,
}
impl StartupReporter {
    pub(super) fn from_env() -> Result<Self> {
        let endpoint_name = match std::env::var_os(READY_ENDPOINT_ENV) {
            Some(env_value) => Some(env_value.into_string().map_err(|invalid_value| {
                anyhow::anyhow!(
                    "{READY_ENDPOINT_ENV} is not valid Unicode: {}",
                    invalid_value.to_string_lossy()
                )
            })?),
            None => None,
        };
        Ok(Self { endpoint_name })
    }
    pub(super) fn ready(&mut self) -> Result<()> {
        self.send(StartupReply::Ready)
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
        if let Err(send_error) = self.send(reply) {
            eprintln!("failed to send daemon startup error: {send_error:#}");
        }
    }
    fn send(&mut self, reply: StartupReply) -> Result<()> {
        let Some(endpoint_name) = self.endpoint_name.take() else {
            return Ok(());
        };
        IpcSender::<StartupReply>::connect(endpoint_name)
            .context("failed to connect daemon startup reporter")?
            .send(reply)
            .context("failed to send daemon startup status")
    }
}
