use crate::runtime::protocol::{Payload, Request};
use crate::runtime::session::Manager;
use alloc::sync::Arc;
use anyhow::Result;
pub(super) async fn execute(manager: &Arc<Manager>, request: Request) -> Result<Payload> {
    match request {
        Request::Ping => Ok(Payload::Pong),
        Request::Close { tab_id } => {
            manager.close(&tab_id).await?;
            Ok(Payload::TabClosed { tab_id })
        }
        Request::NewTab {
            starting_directory,
            starting_shell,
            environment,
        } => {
            let tab_id = manager
                .new_tab(starting_directory, starting_shell, environment)
                .await?;
            Ok(Payload::TabCreated { tab_id })
        }
        Request::ManualWrite {
            tab_id,
            input,
            wait_timeout,
        } => {
            let view = manager.manual_write(&tab_id, input, wait_timeout).await?;
            Ok(Payload::KeyboardWritten { view })
        }
        Request::SendCommand {
            tab_id,
            command,
            wait_timeout,
        } => {
            let (command_id, end_reason, view) = manager
                .send_command(&tab_id, &command, wait_timeout)
                .await?;
            Ok(Payload::CommandAccepted {
                command_id,
                end_reason,
                view,
            })
        }
        Request::View { id, wait_timeout } => {
            Ok(Payload::View(manager.view(&id, wait_timeout).await?))
        }
    }
}
