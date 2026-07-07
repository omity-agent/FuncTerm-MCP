use crate::shell::ShellChoice;
use anyhow::{Context as _, Result};
use core::time::Duration;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
pub(crate) mod format;
mod kind;
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum Request {
    Ping,
    NewTab {
        starting_directory: PathBuf,
        starting_shell: ShellChoice,
    },
    ManualWrite {
        tab_id: String,
        bytes: Vec<u8>,
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
pub(crate) enum Response {
    Ok { payload: Payload },
    Err { message: String },
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
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
    pub(crate) time_consumption: String,
    pub(crate) finished: bool,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct CommandSnapshot {
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
mod tests {
    use super::{CommandView, Payload, ShellView, ViewResult};
    use crate::shell::ShellChoice;
    #[doc = " 该程序输出的主要消费者为 LLM，如果输出中存在 JSON/XML 转义会增加认知负荷和无意义的上下文占用，使用伪结构化文本可读性更高。"]
    #[test]
    fn command_output_uses_uppercase_tags_without_escaping_content() {
        let text = ViewResult::Command {
            shell: shell(true),
            command: CommandView {
                stdout: "left < right".to_owned(),
                stderr: "raw </STDERR> allowed".to_owned(),
                exit_code: Some(0_i32),
                time_consumption: "1s".to_owned(),
                finished: true,
            },
            note: String::new(),
        }
        .into_plain_text();
        assert!(text.contains("<CWD>\nF:\\workspace\\A&B\n</CWD>"));
        assert!(text.contains("<STDOUT>\nleft < right\n</STDOUT>"));
        assert!(text.contains("<STDERR>\nraw </STDERR> allowed\n</STDERR>"));
        assert!(!text.contains("<STDOUT>left < right"));
        assert!(!text.contains("<STDERR>raw </STDERR> allowed"));
    }
    #[test]
    fn empty_command_output_does_not_insert_blank_line() {
        let text = ViewResult::Command {
            shell: shell(true),
            command: CommandView {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: Some(0_i32),
                time_consumption: "1s".to_owned(),
                finished: true,
            },
            note: String::new(),
        }
        .into_plain_text();
        assert!(text.contains("<STDOUT>\n</STDOUT>\n<STDERR>\n</STDERR>"));
        assert!(!text.contains("<STDOUT>\n\n</STDOUT>"));
        assert!(!text.contains("<STDERR>\n\n</STDERR>"));
    }
    #[test]
    fn tab_output_reports_shell_state() {
        let text = ViewResult::Tab {
            shell: shell(true),
            screen: "screen".to_owned(),
            note: String::new(),
        }
        .into_plain_text();
        assert!(text.contains("<SHELL>\n<ALIVE>\ntrue\n</ALIVE>"));
        assert!(text.contains("<TYPE>\nPowerShell\n</TYPE>"));
    }
    #[test]
    fn manual_write_output_reports_screen() {
        let text = Payload::KeyboardWritten {
            view: ViewResult::Tab {
                shell: shell(true),
                screen: "running screen".to_owned(),
                note: String::new(),
            },
        }
        .into_plain_text();
        assert!(!text.contains("<ALIVE>"));
        assert!(text.contains("<SCREEN>\nrunning screen\n</SCREEN>"));
    }
    fn shell(alive: bool) -> ShellView {
        ShellView {
            alive,
            title: "title".to_owned(),
            shell_type: ShellChoice::PowerShell,
            cwd: "F:\\workspace\\A&B".to_owned(),
            idle: true,
        }
    }
}
