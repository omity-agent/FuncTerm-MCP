use crate::shell::ShellChoice;
use anyhow::{Context as _, Result, ensure};
use serde::Serialize;
#[derive(Serialize)]
pub(super) struct CommandReceipt {
    pub(super) command_id: String,
    pub(super) directory: String,
    pub(super) working_directory: String,
    pub(super) separate_output: bool,
}
impl CommandReceipt {
    pub(super) fn encode(&self, shell: ShellChoice) -> Result<String> {
        match shell {
            ShellChoice::Bash | ShellChoice::Zsh => Ok(format!(
                "{}\0{}\0{}\0{}\0",
                self.command_id,
                self.directory,
                self.working_directory,
                u8::from(self.separate_output)
            )),
            ShellChoice::Cmd => {
                ensure!(
                    !self.directory.contains(['\r', '\n'])
                        && !self.working_directory.contains(['\r', '\n']),
                    "CMD command paths contain line breaks"
                );
                Ok(format!(
                    "{}\n{}\n{}\n{}",
                    self.command_id,
                    self.directory,
                    self.working_directory,
                    u8::from(self.separate_output)
                ))
            }
            ShellChoice::PowerShell
            | ShellChoice::NuShell
            | ShellChoice::Python
            | ShellChoice::Bun => {
                sonic_rs::to_string(self).context("failed to serialize command receipt")
            }
        }
    }
}
