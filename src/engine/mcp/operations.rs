use super::{
    arguments::{ManualWriteExec, NewTabExec, SendCommandExec},
    error_text, output,
};
use crate::runtime::config::Settings;
use crate::runtime::protocol::{EnvironmentSnapshot, Payload, Request};
use core::time::Duration;
use rmcp::model::CallToolResult;
pub(super) struct LookupId(pub(super) String);
pub(super) trait Operation {
    const LIST_PARAMETER: &'static str = "exec";
    fn tab_id(&self) -> Option<&str> {
        None
    }
    fn request(self, settings: &Settings, wait_timeout: Duration) -> Result<Request, String>;
    fn output(payload: Payload) -> Result<CallToolResult, String>;
}
impl Operation for NewTabExec {
    fn request(self, settings: &Settings, _wait_timeout: Duration) -> Result<Request, String> {
        let starting_directory =
            crate::runtime::working_dir::resolve(self.starting_directory_path())
                .map_err(error_text)?;
        Ok(Request::NewTab {
            starting_directory,
            starting_shell: self.starting_shell,
            load_profile: settings.shell_load_profile,
            environment: EnvironmentSnapshot::for_new_tab_request(),
        })
    }
    fn output(payload: Payload) -> Result<CallToolResult, String> {
        output::new_tab(payload)
    }
}
impl Operation for ManualWriteExec {
    fn tab_id(&self) -> Option<&str> {
        Some(&self.tab_id)
    }
    fn request(self, _settings: &Settings, wait_timeout: Duration) -> Result<Request, String> {
        let (tab_id, input) = self.into_parts().map_err(error_text)?;
        Ok(Request::ManualWrite {
            tab_id,
            input,
            wait_timeout,
        })
    }
    fn output(payload: Payload) -> Result<CallToolResult, String> {
        output::manual_write(payload)
    }
}
impl Operation for SendCommandExec {
    fn tab_id(&self) -> Option<&str> {
        Some(&self.tab_id)
    }
    fn request(self, _settings: &Settings, wait_timeout: Duration) -> Result<Request, String> {
        Ok(Request::SendCommand {
            tab_id: self.tab_id,
            command: self.command,
            wait_timeout,
        })
    }
    fn output(payload: Payload) -> Result<CallToolResult, String> {
        output::send_command(payload)
    }
}
impl Operation for LookupId {
    const LIST_PARAMETER: &'static str = "ids";
    fn request(self, _settings: &Settings, wait_timeout: Duration) -> Result<Request, String> {
        Ok(Request::View {
            id: self.0,
            wait_timeout,
        })
    }
    fn output(payload: Payload) -> Result<CallToolResult, String> {
        output::view(payload)
    }
}
