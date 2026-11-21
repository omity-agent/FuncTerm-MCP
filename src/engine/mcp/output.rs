use crate::runtime::protocol::{
    CommandPresentation, EndReason, Payload, ShellPresentation, ViewResult,
    format::{command_plain_text, tab_created_plain_text, tab_plain_text},
};
use anyhow::{Result, bail, ensure};
use rmcp::serde_json::Value;
use serde::Serialize;
pub(super) fn processing(
    response: rmcp::model::CallToolResponse,
    measurement: Option<crate::runtime::protocol::time_consumption::ProcessingTime>,
) -> Result<rmcp::model::CallToolResponse, rmcp::ErrorData> {
    let rmcp::model::CallToolResponse::Complete(mut result) = response else {
        return Ok(response);
    };
    if let Some(data) = measurement {
        let value = rmcp::serde_json::to_value(data).map_err(internal_error)?;
        result
            .meta
            .get_or_insert_default()
            .0
            .insert("functerm/processing".to_owned(), value);
    }
    Ok(result.into())
}
pub(super) fn internal_error(error: impl core::fmt::Display) -> rmcp::ErrorData {
    tracing::error!("MCP runtime failed to complete the tool call");
    tracing::debug!(error = format!("{error:#}"), "MCP runtime failure details");
    rmcp::ErrorData::internal_error("FuncTerm could not complete the tool call", None)
}
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
pub(super) struct RunOutput<'payload> {
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
pub(super) struct EntryOutput {
    pub(super) text: String,
    pub(super) value: Value,
}
pub(super) fn new_tab(payload: Payload) -> Result<EntryOutput> {
    let Payload::TabCreated { tab_id } = payload else {
        bail!("daemon returned a response other than TabCreated");
    };
    tool_result(
        tab_created_plain_text(&tab_id),
        &NewTabOutput { tab_id: &tab_id },
    )
}
pub(super) fn manual_write(payload: Payload) -> Result<EntryOutput> {
    let Payload::KeyboardWritten {
        view: ViewResult::Tab {
            shell,
            screen,
            note,
        },
    } = payload
    else {
        bail!("daemon returned a response other than a keyboard write with a Tab view");
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
pub(super) fn run(payload: Payload) -> Result<EntryOutput> {
    let Payload::CommandAccepted {
        command_id,
        view:
            ViewResult::Command {
                shell,
                command,
                note,
                end_reason,
            },
    } = payload
    else {
        bail!("daemon returned a response other than a command result");
    };
    ensure!(
        end_reason != EndReason::CommandFailed,
        "command was stopped after a runtime failure"
    );
    tool_result(
        command_plain_text(&shell, &command, &note, false, Some(&command_id)),
        &RunOutput {
            shell: shell.presentation(false),
            command: command.presentation(Some(&command_id)),
            note: &note,
        },
    )
}
pub(super) fn view(payload: Payload) -> Result<EntryOutput> {
    let Payload::View(view) = payload else {
        bail!("daemon returned a response other than a view");
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
            end_reason,
        } => {
            ensure!(
                end_reason != EndReason::CommandFailed,
                "command was stopped after a runtime failure"
            );
            tool_result(
                command_plain_text(&shell, &command, &note, true, None),
                &ViewOutput {
                    shell: shell.presentation(true),
                    screen: None,
                    command: Some(command.presentation(None)),
                    note: &note,
                },
            )
        }
    }
}
fn tool_result<T>(text: String, structured_content: &T) -> Result<EntryOutput>
where
    T: Serialize + ?Sized,
{
    let value = rmcp::serde_json::to_value(structured_content)?;
    Ok(EntryOutput { text, value })
}
