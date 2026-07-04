use crate::runtime::protocol::{
    CommandView, Payload, ShellView, ViewResult,
    format::{command_plain_text, tab_created_plain_text, tab_plain_text},
};
use alloc::sync::Arc;
use rmcp::model::{CallToolResult, ContentBlock, JsonObject};
use serde::Serialize;
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct NewTabOutput<'payload> {
    pub(super) tab_id: &'payload str,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct ManualWriteOutput<'payload> {
    pub(super) shell: ShellData<'payload>,
    pub(super) screen: &'payload str,
    pub(super) note: &'payload str,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct SendCommandOutput<'payload> {
    pub(super) shell: ShellData<'payload>,
    pub(super) command: CommandData<'payload>,
    pub(super) note: &'payload str,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct ViewOutput<'payload> {
    pub(super) shell: ShellData<'payload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) screen: Option<&'payload str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) command: Option<CommandData<'payload>>,
    pub(super) note: &'payload str,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct ShellData<'payload> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) alive: Option<bool>,
    pub(super) title: &'payload str,
    #[serde(rename = "type")]
    pub(super) shell_type: &'static str,
    pub(super) cwd: &'payload str,
    pub(super) idle: bool,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct CommandData<'payload> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) command_id: Option<&'payload str>,
    pub(super) stdout: &'payload str,
    pub(super) stderr: &'payload str,
    pub(super) exit_code: Option<i32>,
    pub(super) time_consumption: &'payload str,
    pub(super) finished: bool,
}
pub(super) fn schema<T>() -> Arc<JsonObject>
where
    T: rmcp::schemars::JsonSchema + 'static,
{
    rmcp::handler::server::tool::schema_for_output::<T>()
        .unwrap_or_else(|error| panic!("invalid MCP output schema: {error}"))
}
pub(super) fn new_tab(payload: Payload) -> Result<CallToolResult, String> {
    match payload {
        Payload::TabCreated { tab_id } => tool_result(
            tab_created_plain_text(&tab_id),
            &NewTabOutput { tab_id: &tab_id },
        ),
        Payload::Pong
        | Payload::KeyboardWritten { .. }
        | Payload::CommandAccepted { .. }
        | Payload::View(_) => Err(unexpected_response()),
    }
}
pub(super) fn manual_write(payload: Payload) -> Result<CallToolResult, String> {
    match payload {
        Payload::KeyboardWritten {
            view:
                ViewResult::Tab {
                    shell,
                    screen,
                    note,
                },
        } => tool_result(
            tab_plain_text(&shell, &screen, &note, false),
            &ManualWriteOutput {
                shell: ShellData::from_shell(&shell, false),
                screen: &screen,
                note: &note,
            },
        ),
        Payload::KeyboardWritten {
            view: ViewResult::Command { .. },
        }
        | Payload::Pong
        | Payload::TabCreated { .. }
        | Payload::CommandAccepted { .. }
        | Payload::View(_) => Err(unexpected_response()),
    }
}
pub(super) fn send_command(payload: Payload) -> Result<CallToolResult, String> {
    match payload {
        Payload::CommandAccepted {
            command_id,
            view:
                ViewResult::Command {
                    shell,
                    command,
                    note,
                },
            ..
        } => tool_result(
            command_plain_text(&shell, &command, &note, false, Some(&command_id)),
            &SendCommandOutput {
                shell: ShellData::from_shell(&shell, false),
                command: CommandData::from_command(&command, Some(&command_id)),
                note: &note,
            },
        ),
        Payload::CommandAccepted {
            view: ViewResult::Tab { .. },
            ..
        }
        | Payload::Pong
        | Payload::TabCreated { .. }
        | Payload::KeyboardWritten { .. }
        | Payload::View(_) => Err(unexpected_response()),
    }
}
pub(super) fn view(payload: Payload) -> Result<CallToolResult, String> {
    match payload {
        Payload::View(ViewResult::Tab {
            shell,
            screen,
            note,
        }) => tool_result(
            tab_plain_text(&shell, &screen, &note, true),
            &ViewOutput {
                shell: ShellData::from_shell(&shell, true),
                screen: Some(&screen),
                command: None,
                note: &note,
            },
        ),
        Payload::View(ViewResult::Command {
            shell,
            command,
            note,
        }) => tool_result(
            command_plain_text(&shell, &command, &note, true, None),
            &ViewOutput {
                shell: ShellData::from_shell(&shell, true),
                screen: None,
                command: Some(CommandData::from_command(&command, None)),
                note: &note,
            },
        ),
        Payload::Pong
        | Payload::TabCreated { .. }
        | Payload::KeyboardWritten { .. }
        | Payload::CommandAccepted { .. } => Err(unexpected_response()),
    }
}
fn tool_result<T>(content: String, structured_content: &T) -> Result<CallToolResult, String>
where
    T: Serialize + ?Sized,
{
    let value = rmcp::serde_json::to_value(structured_content)
        .map_err(|error| format!("failed to serialize structured content: {error}"))?;
    let mut result = CallToolResult::success(vec![ContentBlock::text(content)]);
    result.structured_content = Some(value);
    Ok(result)
}
fn unexpected_response() -> String {
    "daemon returned an unexpected response".to_owned()
}
impl<'payload> ShellData<'payload> {
    fn from_shell(shell: &'payload ShellView, include_alive: bool) -> Self {
        Self {
            alive: include_alive.then_some(shell.alive),
            title: &shell.title,
            shell_type: shell.shell_type.display_name(),
            cwd: &shell.cwd,
            idle: shell.idle,
        }
    }
}
impl<'payload> CommandData<'payload> {
    fn from_command(command: &'payload CommandView, command_id: Option<&'payload str>) -> Self {
        Self {
            command_id,
            stdout: &command.stdout,
            stderr: &command.stderr,
            exit_code: command.exit_code,
            time_consumption: &command.time_consumption,
            finished: command.finished,
        }
    }
}
