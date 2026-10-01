use super::interface::{self, Args, CliCommand};
use crate::runtime::config;
use crate::runtime::protocol::{
    EnvironmentSnapshot, KeyboardInput, Request, wait_timeout_from_seconds,
};
use anyhow::{Context as _, Result};
use base64_turbo::STANDARD;
use clap::Parser as _;
use std::path::Path;
pub(crate) async fn run() -> Result<()> {
    let args = Args::parse();
    match args.command.unwrap_or(CliCommand::Mcp) {
        CliCommand::InternalEnsureShims { directory } => {
            crate::shell::shims::ensure_directory(&directory)
        }
        CliCommand::InternalLaunchDaemon => crate::runtime::client::run_daemon_launcher(),
        CliCommand::InternalTimeMillis => print_result(crate::app::command_state::time_millis()),
        CliCommand::InternalWriteDone {
            command_id,
            exit_code,
            time_consumption,
            cwd,
            directory,
        } => write_done(&command_id, exit_code, &time_consumption, &cwd, &directory),
        CliCommand::InternalWriteStart {
            command_id,
            directory,
        } => {
            let settings = config::load()?;
            crate::app::command_state::write_start(
                &command_id,
                &directory,
                &settings.terminal_model_title,
            )
        }
        CliCommand::Mcp => crate::mcp::run(config::load()?).await,
        CliCommand::Daemon => crate::runtime::daemon::run(config::load()?).await,
        CliCommand::Close { tab_id, current: _ } => {
            let target = interface::close_target(tab_id)?;
            let settings = config::load()?;
            print_result(interface::execute(&settings, Request::Close { tab_id: target }).await)
        }
        CliCommand::NewTab {
            starting_directory,
            starting_shell,
        } => {
            let settings = config::load()?;
            let request = Request::NewTab {
                starting_directory: crate::runtime::working_dir::resolve(
                    starting_directory.as_deref(),
                )?,
                starting_shell,
                environment: EnvironmentSnapshot::for_new_tab_request(),
            };
            print_result(interface::execute(&settings, request).await)
        }
        CliCommand::ManualWrite {
            tab_id,
            text,
            base64,
            wait_timeout,
        } => {
            let settings = config::load()?;
            let input = match (text, base64) {
                (Some(input_text), None) => KeyboardInput::Text(input_text),
                (None, Some(encoded)) => KeyboardInput::Bytes(
                    STANDARD
                        .decode(&encoded)
                        .context("invalid base64 keyboard input")?,
                ),
                _ => anyhow::bail!("manual-write requires exactly one of --text or --base64"),
            };
            let request = Request::ManualWrite {
                tab_id,
                input,
                wait_timeout: wait_timeout_from_seconds(wait_timeout)?,
            };
            print_result(interface::execute(&settings, request).await)
        }
        CliCommand::SendCommand {
            tab_id,
            command: shell_command,
            wait_timeout: wait_timeout_seconds,
        } => {
            let settings = config::load()?;
            let request = Request::SendCommand {
                tab_id,
                command: shell_command,
                wait_timeout: wait_timeout_from_seconds(wait_timeout_seconds)?,
            };
            print_result(interface::execute(&settings, request).await)
        }
        CliCommand::View {
            id,
            wait_timeout: wait_timeout_seconds,
        } => {
            let settings = config::load()?;
            let request = Request::View {
                id,
                wait_timeout: wait_timeout_from_seconds(wait_timeout_seconds)?,
            };
            print_result(interface::execute(&settings, request).await)
        }
    }
}
fn print_result(result: Result<String>) -> Result<()> {
    let text = result?;
    println!("{text}");
    Ok(())
}
fn write_done(
    command_id: &str,
    exit_code: i32,
    time_consumption: &str,
    cwd: &str,
    directory: &Path,
) -> Result<()> {
    crate::app::command_state::write_done(
        &crate::app::command_state::DoneOutput {
            command_id,
            exit_code,
            time_consumption,
            cwd,
        },
        directory,
    )
}
