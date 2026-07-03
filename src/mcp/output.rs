use crate::runtime::protocol::{CommandView, Payload, ShellView, ViewResult};
use alloc::sync::Arc;
use rmcp::model::{CallToolResult, ContentBlock, JsonObject};
use serde::Serialize;
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct NewTabOutput {
    pub(super) tab_id: String,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct ManualWriteOutput {
    pub(super) shell: ShellData,
    pub(super) screen: String,
    pub(super) note: String,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct SendCommandOutput {
    pub(super) shell: ShellData,
    pub(super) command: CommandData,
    pub(super) note: String,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
#[serde(untagged)]
pub(super) enum ViewOutput {
    Tab {
        shell: ShellData,
        screen: String,
        note: String,
    },
    Command {
        shell: ShellData,
        command: CommandData,
        note: String,
    },
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct ShellData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) alive: Option<bool>,
    pub(super) title: String,
    #[serde(rename = "type")]
    pub(super) shell_type: String,
    pub(super) cwd: String,
    pub(super) idle: bool,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct CommandData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) command_id: Option<String>,
    pub(super) stdout: String,
    pub(super) stderr: String,
    pub(super) exit_code: Option<i32>,
    pub(super) time_consumption: String,
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
    let text = payload.clone().into_plain_text();
    let Payload::TabCreated { tab_id } = payload else {
        return Err(unexpected_response());
    };
    result(text, NewTabOutput { tab_id })
}
pub(super) fn manual_write(payload: &Payload) -> Result<CallToolResult, String> {
    let owned_payload = payload.clone();
    let text = owned_payload.clone().into_plain_text();
    let Payload::KeyboardWritten { view } = owned_payload else {
        return Err(unexpected_response());
    };
    let ViewResult::Tab {
        shell,
        screen,
        note,
    } = view
    else {
        return Err(unexpected_response());
    };
    result(
        text,
        ManualWriteOutput {
            shell: ShellData::from_shell(shell, false),
            screen,
            note,
        },
    )
}
pub(super) fn send_command(payload: Payload) -> Result<CallToolResult, String> {
    let text = payload.clone().into_plain_text();
    let Payload::CommandAccepted {
        command_id, view, ..
    } = payload
    else {
        return Err(unexpected_response());
    };
    let ViewResult::Command {
        shell,
        command,
        note,
    } = view
    else {
        return Err(unexpected_response());
    };
    result(
        text,
        SendCommandOutput {
            shell: ShellData::from_shell(shell, false),
            command: CommandData::from_command(command, Some(command_id)),
            note,
        },
    )
}
pub(super) fn view(payload: Payload) -> Result<CallToolResult, String> {
    let text = payload.clone().into_plain_text();
    let Payload::View(view) = payload else {
        return Err(unexpected_response());
    };
    result(text, ViewOutput::from(view))
}
fn result<T>(content: String, structured_content: T) -> Result<CallToolResult, String>
where
    T: Serialize,
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
impl From<ViewResult> for ViewOutput {
    fn from(value: ViewResult) -> Self {
        match value {
            ViewResult::Tab {
                shell,
                screen,
                note,
            } => Self::Tab {
                shell: ShellData::from_shell(shell, true),
                screen,
                note,
            },
            ViewResult::Command {
                shell,
                command,
                note,
            } => Self::Command {
                shell: ShellData::from_shell(shell, true),
                command: CommandData::from_command(command, None),
                note,
            },
        }
    }
}
impl ShellData {
    fn from_shell(shell: ShellView, include_alive: bool) -> Self {
        Self {
            alive: include_alive.then_some(shell.alive),
            title: shell.title,
            shell_type: shell.shell_type.display_name().to_owned(),
            cwd: shell.cwd,
            idle: shell.idle,
        }
    }
}
impl CommandData {
    fn from_command(command: CommandView, command_id: Option<String>) -> Self {
        Self {
            command_id,
            stdout: command.stdout,
            stderr: command.stderr,
            exit_code: command.exit_code,
            time_consumption: command.time_consumption,
            finished: command.finished,
        }
    }
}
