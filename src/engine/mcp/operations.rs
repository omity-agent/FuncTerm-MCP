use super::{
    arguments::{ManualWriteExec, NewTabExec, SendCommandExec},
    error_text, output,
};
use crate::runtime::client::DaemonClient;
use core::time::Duration;
use rmcp::model::CallToolResult;
pub(super) struct LookupId(pub(super) String);
pub(super) trait Operation: Send + 'static {
    const LIST_PARAMETER: &'static str = "exec";
    fn tab_id(&self) -> Option<&str> {
        None
    }
    fn execute(
        self,
        client: &mut DaemonClient,
        wait_timeout: Duration,
    ) -> Result<CallToolResult, String>;
}
impl Operation for NewTabExec {
    fn execute(
        self,
        client: &mut DaemonClient,
        _wait_timeout: Duration,
    ) -> Result<CallToolResult, String> {
        let starting_directory =
            crate::runtime::working_dir::resolve(self.starting_directory_path())
                .map_err(error_text)?;
        let request = crate::runtime::protocol::Request::NewTab {
            starting_directory,
            starting_shell: self.starting_shell,
            environment: crate::runtime::protocol::EnvironmentSnapshot::for_new_tab_request(),
        };
        output::new_tab(call(client, &request)?)
    }
}
impl Operation for ManualWriteExec {
    fn tab_id(&self) -> Option<&str> {
        Some(&self.tab_id)
    }
    fn execute(
        self,
        client: &mut DaemonClient,
        wait_timeout: Duration,
    ) -> Result<CallToolResult, String> {
        let (tab_id, input) = self.into_parts().map_err(error_text)?;
        let request = crate::runtime::protocol::Request::ManualWrite {
            tab_id,
            input,
            wait_timeout,
        };
        output::manual_write(call(client, &request)?)
    }
}
impl Operation for SendCommandExec {
    fn tab_id(&self) -> Option<&str> {
        Some(&self.tab_id)
    }
    fn execute(
        self,
        client: &mut DaemonClient,
        wait_timeout: Duration,
    ) -> Result<CallToolResult, String> {
        let request = crate::runtime::protocol::Request::SendCommand {
            tab_id: self.tab_id,
            command: self.command,
            wait_timeout,
        };
        output::send_command(call(client, &request)?)
    }
}
impl Operation for LookupId {
    const LIST_PARAMETER: &'static str = "ids";
    fn execute(
        self,
        client: &mut DaemonClient,
        wait_timeout: Duration,
    ) -> Result<CallToolResult, String> {
        let request = crate::runtime::protocol::Request::View {
            id: self.0,
            wait_timeout,
        };
        output::view(call(client, &request)?)
    }
}
fn call(
    client: &mut DaemonClient,
    request: &crate::runtime::protocol::Request,
) -> Result<crate::runtime::protocol::Payload, String> {
    let payload = client.call(request).map_err(error_text)?;
    payload.ensure_matches(request).map_err(error_text)
}
