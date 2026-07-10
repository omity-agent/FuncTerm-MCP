use super::{CommandView, ShellView};
use serde::Serialize;
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(crate) struct ShellPresentation<'view> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) alive: Option<bool>,
    pub(crate) title: &'view str,
    #[serde(rename = "type")]
    pub(crate) shell_type: &'static str,
    pub(crate) cwd: &'view str,
    pub(crate) idle: bool,
}
#[derive(Debug, Serialize, rmcp :: schemars :: JsonSchema)]
pub(crate) struct CommandPresentation<'view> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) command_id: Option<&'view str>,
    pub(crate) stdout: &'view str,
    pub(crate) stderr: &'view str,
    pub(crate) exit_code: Option<i32>,
    pub(crate) time_consumption: &'view str,
    pub(crate) finished: bool,
}
impl ShellView {
    pub(crate) fn presentation(&self, include_alive: bool) -> ShellPresentation<'_> {
        ShellPresentation {
            alive: include_alive.then_some(self.alive),
            title: &self.title,
            shell_type: self.shell_type.display_name(),
            cwd: &self.cwd,
            idle: self.idle,
        }
    }
}
impl CommandView {
    pub(crate) fn presentation<'view>(
        &'view self,
        command_id: Option<&'view str>,
    ) -> CommandPresentation<'view> {
        CommandPresentation {
            command_id,
            stdout: &self.stdout,
            stderr: &self.stderr,
            exit_code: self.exit_code,
            time_consumption: &self.time_consumption,
            finished: self.finished,
        }
    }
}
