use crate::runtime::protocol::{
    CommandPresentation, Payload, ShellPresentation, ViewResult,
    format::{command_plain_text, tab_created_plain_text, tab_plain_text},
};
use rmcp::model::{CallToolResult, ContentBlock};
use serde::Serialize;
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct NewTabOutput<'payload> {
    pub(super) tab_id: &'payload str,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct ManualWriteOutput<'payload> {
    pub(super) shell: ShellPresentation<'payload>,
    pub(super) screen: &'payload str,
    pub(super) note: &'payload str,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct SendCommandOutput<'payload> {
    pub(super) shell: ShellPresentation<'payload>,
    pub(super) command: CommandPresentation<'payload>,
    pub(super) note: &'payload str,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct ViewOutput<'payload> {
    pub(super) shell: ShellPresentation<'payload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) screen: Option<&'payload str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) command: Option<CommandPresentation<'payload>>,
    pub(super) note: &'payload str,
}
pub(super) fn new_tab(payload: Payload) -> Result<CallToolResult, String> {
    let Payload::TabCreated { tab_id } = payload else {
        return Err(unexpected_response());
    };
    tool_result(
        tab_created_plain_text(&tab_id),
        &NewTabOutput { tab_id: &tab_id },
    )
}
pub(super) fn manual_write(payload: Payload) -> Result<CallToolResult, String> {
    let Payload::KeyboardWritten {
        view: ViewResult::Tab {
            shell,
            screen,
            note,
        },
    } = payload
    else {
        return Err(unexpected_response());
    };
    tool_result(
        tab_plain_text(&shell, &screen, &note, false),
        &ManualWriteOutput {
            shell: shell.presentation(false),
            screen: &screen,
            note: &note,
        },
    )
}
pub(super) fn send_command(payload: Payload) -> Result<CallToolResult, String> {
    let Payload::CommandAccepted {
        command_id,
        view:
            ViewResult::Command {
                shell,
                command,
                note,
            },
        end_reason: _,
    } = payload
    else {
        return Err(unexpected_response());
    };
    tool_result(
        command_plain_text(&shell, &command, &note, false, Some(&command_id)),
        &SendCommandOutput {
            shell: shell.presentation(false),
            command: command.presentation(Some(&command_id)),
            note: &note,
        },
    )
}
pub(super) fn view(payload: Payload) -> Result<CallToolResult, String> {
    let Payload::View(view) = payload else {
        return Err(unexpected_response());
    };
    match view {
        ViewResult::Tab {
            shell,
            screen,
            note,
        } => tool_result(
            tab_plain_text(&shell, &screen, &note, true),
            &ViewOutput {
                shell: shell.presentation(true),
                screen: Some(&screen),
                command: None,
                note: &note,
            },
        ),
        ViewResult::Command {
            shell,
            command,
            note,
        } => tool_result(
            command_plain_text(&shell, &command, &note, true, None),
            &ViewOutput {
                shell: shell.presentation(true),
                screen: None,
                command: Some(command.presentation(None)),
                note: &note,
            },
        ),
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
