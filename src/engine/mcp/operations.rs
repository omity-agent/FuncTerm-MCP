mod routes;
mod schema;
mod script;
use super::{
    arguments::{ManualWriteInput, NewTabOptions, RunInput, ViewQuery},
    output::{self, EntryOutput},
};
use crate::runtime::config::Settings;
use crate::runtime::protocol::{EnvironmentSnapshot, Payload, Request};
use anyhow::Result;
use core::time::Duration;
pub(super) use routes::router;
pub(super) trait Operation {
    const LIST_PARAMETER: &'static str;
    fn tab_id(&self) -> Option<&str> {
        None
    }
    fn request(self, settings: &Settings, wait_timeout: Duration) -> Result<Request>;
    fn output(payload: Payload) -> Result<EntryOutput>;
}
impl Operation for NewTabOptions {
    const LIST_PARAMETER: &'static str = "tabs";
    fn request(self, settings: &Settings, _wait_timeout: Duration) -> Result<Request> {
        let starting_directory =
            crate::runtime::working_dir::resolve(self.starting_directory_path())?;
        Ok(Request::NewTab {
            starting_directory,
            starting_shell: self.starting_shell,
            load_profile: settings.shell_load_profile,
            elevated: self.elevated,
            environment: EnvironmentSnapshot::capture(),
        })
    }
    fn output(payload: Payload) -> Result<EntryOutput> {
        output::new_tab(payload)
    }
}
impl Operation for ManualWriteInput {
    const LIST_PARAMETER: &'static str = "inputs";
    fn tab_id(&self) -> Option<&str> {
        Some(&self.tab_id)
    }
    fn request(self, _settings: &Settings, wait_timeout: Duration) -> Result<Request> {
        let (tab_id, input) = self.into_parts()?;
        Ok(Request::ManualWrite {
            tab_id,
            input,
            wait_timeout,
        })
    }
    fn output(payload: Payload) -> Result<EntryOutput> {
        output::manual_write(payload)
    }
}
impl Operation for RunInput {
    const LIST_PARAMETER: &'static str = "commands";
    fn tab_id(&self) -> Option<&str> {
        Some(&self.tab_id)
    }
    fn request(self, settings: &Settings, wait_timeout: Duration) -> Result<Request> {
        Ok(Request::Run {
            tab_id: self.tab_id,
            command: self.command,
            separate_output: self
                .separate_output
                .unwrap_or(settings.command_separate_output),
            wait_timeout,
        })
    }
    fn output(payload: Payload) -> Result<EntryOutput> {
        output::run(payload)
    }
}
impl Operation for ViewQuery {
    const LIST_PARAMETER: &'static str = "queries";
    fn request(self, _settings: &Settings, wait_timeout: Duration) -> Result<Request> {
        Ok(Request::View {
            id: self.id,
            wait_timeout,
            start_token: self.start_token,
        })
    }
    fn output(payload: Payload) -> Result<EntryOutput> {
        output::view(payload)
    }
}
