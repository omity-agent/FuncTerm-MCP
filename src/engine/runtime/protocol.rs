use crate::shell::ShellChoice;
use anyhow::{Context as _, Result};
use bytestring::ByteString;
use core::num::NonZeroUsize;
use core::time::Duration;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
mod emission;
mod environment;
pub(crate) mod format;
mod kind;
mod presentation;
mod time_consumption;
mod tokenization;
pub(crate) use emission::{CommandOutput, OutputMode, OutputPresentation};
pub(crate) use environment::{EnvironmentSnapshot, environment_name_equals};
pub(crate) use presentation::{CommandPresentation, ShellPresentation};
pub(crate) use tokenization::OutputTokenizer;
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize, strum :: EnumDiscriminants)]
#[strum_discriminants(name(RequestKind), derive(strum::Display))]
#[strum_discriminants(strum(serialize_all = "kebab-case"))]
pub(crate) enum Request {
    Ping,
    Shutdown,
    NewTab {
        starting_directory: PathBuf,
        starting_shell: ShellChoice,
        load_profile: bool,
        elevated: bool,
        environment: EnvironmentSnapshot,
    },
    Close {
        tab_id: String,
    },
    ManualWrite {
        tab_id: String,
        input: KeyboardInput,
        wait_timeout: Duration,
    },
    SendCommand {
        tab_id: String,
        command: String,
        separate_output: bool,
        wait_timeout: Duration,
    },
    View {
        id: String,
        wait_timeout: Duration,
        start_token: NonZeroUsize,
    },
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum KeyboardInput {
    Text(String),
    Bytes(#[serde(with = "serde_bytes")] Vec<u8>),
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum Response {
    Ok { payload: Payload },
    Err { message: String },
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize, strum :: EnumDiscriminants)]
#[strum_discriminants(name(PayloadKind), derive(strum::Display))]
#[strum_discriminants(strum(serialize_all = "kebab-case"))]
pub(crate) enum Payload {
    Pong,
    ShutdownComplete,
    TabCreated {
        tab_id: String,
    },
    TabClosed {
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
    pub(crate) output: CommandOutput,
    pub(crate) tokens: usize,
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
        screen: ByteString,
        note: String,
    },
    Command {
        shell: ShellView,
        command: CommandView,
        note: String,
    },
}
pub(crate) fn wait_timeout_from_seconds(seconds: f64) -> Result<Duration> {
    Duration::try_from_secs_f64(seconds)
        .context("wait timeout must be a finite non-negative number of seconds")
}
