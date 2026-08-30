use crate::shell::ShellChoice;
use anyhow::{Context as _, Result};
use core::time::Duration;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
mod environment;
pub(crate) mod format;
mod kind;
mod presentation;
mod time_consumption;
pub(crate) use environment::EnvironmentSnapshot;
pub(crate) use presentation::{CommandPresentation, ShellPresentation};
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum Request {
    Ping,
    NewTab {
        starting_directory: PathBuf,
        starting_shell: ShellChoice,
        environment: EnvironmentSnapshot,
    },
    ManualWrite {
        tab_id: String,
        input: KeyboardInput,
        waiting: Duration,
    },
    SendCommand {
        tab_id: String,
        command: String,
        waiting: Duration,
    },
    View {
        id: String,
        waiting: Duration,
    },
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum KeyboardInput {
    Text(String),
    Bytes(Vec<u8>),
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum Response {
    Ok { payload: Payload },
    Err { message: String },
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize, strum :: EnumDiscriminants)]
#[strum_discriminants(name(PayloadKind))]
pub(crate) enum Payload {
    Pong,
    TabCreated {
        tab_id: String,
    },
    KeyboardWritten {
        view: ViewResult,
    },
    CommandAccepted {
        command_id: String,
        end_reason: EndReason,
        view: ViewResult,
    },
    View(ViewResult),
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum EndReason {
    CommandEnded,
    WaitTimeout,
    CommandFailed,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct ShellView {
    pub(crate) alive: bool,
    pub(crate) title: String,
    pub(crate) shell_type: ShellChoice,
    pub(crate) cwd: String,
    pub(crate) idle: bool,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct CommandView {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) exit_code: Option<i32>,
    pub(crate) time_consumption: Duration,
    pub(crate) finished: bool,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct CommandSnapshot {
    pub(crate) title: String,
    pub(crate) command: CommandView,
    pub(crate) note: String,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum ViewResult {
    Tab {
        shell: ShellView,
        screen: String,
        note: String,
    },
    Command {
        shell: ShellView,
        command: CommandView,
        note: String,
    },
}
pub(crate) fn waiting_from_seconds(seconds: f64) -> Result<Duration> {
    Duration::try_from_secs_f64(seconds)
        .context("waiting must be a finite non-negative number of seconds")
}
#[cfg(test)]
#[path = "protocol/protocol_tests.rs"]
mod tests;
