use super::interface::{self, Args, CliCommand, DaemonCommand};
use crate::runtime::protocol::{
    EnvironmentSnapshot, KeyboardInput, Request, format::line_terminated, wait_timeout_from_seconds,
};
use crate::runtime::{config, service_control};
use anyhow::{Context as _, Result};
use base64_turbo::STANDARD;
use clap::Parser as _;
use std::path::Path;
pub(crate) async fn run() -> Result<std::process::ExitCode> {
    let args = Args::parse();
    match args.command.unwrap_or(CliCommand::Mcp {
        no_profile: false,
        privilege_mode: None,
        output_mode: None,
    }) {
        CliCommand::InternalEnsureShims { directory } => {
            crate::shell::shims::ensure_directory(&directory)
        }
        CliCommand::InternalLaunchDaemon => crate::runtime::client::run_daemon_launcher(),
        CliCommand::InternalRunDaemon => crate::runtime::daemon::run(config::load()?).await,
        #[cfg(unix)]
        CliCommand::InternalRunShell { uid, command } => {
            crate::engine::privileges::run_shell(uid, &command)
        }
        #[cfg(windows)]
        CliCommand::InternalPtyHost {
            rows,
            cols,
            starting_directory,
            command,
        } => {
            return crate::engine::privileges::stream_host::run(
                rows,
                cols,
                &starting_directory,
                command,
            )
            .map(crate::shim::process_exit);
        }
        CliCommand::InternalTimeMillis => print_result(crate::app::command_state::time_millis()),
        #[cfg(windows)]
        CliCommand::InternalCmdPath { startup_directory } => print_result(
            crate::app::command_state::cmd_search_path(&startup_directory),
        ),
        CliCommand::InternalClaimCommand {
            session_root,
            shell,
        } => print_result(crate::app::command_state::claim_command(
            &session_root,
            shell,
        )),
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
        CliCommand::Mcp {
            no_profile,
            privilege_mode,
            output_mode,
        } => {
            let mut settings = config::load()?;
            settings.shell_load_profile &= !no_profile;
            crate::mcp::run(settings, privilege_mode, output_mode).await
        }
        CliCommand::Daemon { command } => {
            let settings = config::load()?;
            let message = match command {
                DaemonCommand::Start => {
                    if service_control::start(&settings).await? {
                        "daemon started"
                    } else {
                        "daemon is already running"
                    }
                }
                DaemonCommand::Stop => {
                    if service_control::stop(&settings).await? {
                        "daemon stopped"
                    } else {
                        "daemon is not running"
                    }
                }
                DaemonCommand::Restart => {
                    service_control::restart(&settings).await?;
                    "daemon restarted"
                }
            };
            print_result(Ok(message.to_owned()))
        }
        CliCommand::Close { tab_id, current: _ } => {
            let target = interface::close_target(tab_id)?;
            let settings = config::load()?;
            print_result(interface::execute(&settings, Request::Close { tab_id: target }).await)
        }
        CliCommand::NewTab {
            starting_directory,
            starting_shell,
            no_profile,
            elevated,
        } => {
            let settings = config::load()?;
            let request = Request::NewTab {
                starting_directory: crate::runtime::working_dir::resolve(
                    starting_directory.as_deref(),
                )?,
                starting_shell,
                load_profile: settings.shell_load_profile && !no_profile,
                elevated,
                environment: EnvironmentSnapshot::capture(),
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
            separate_output,
            wait_timeout: wait_timeout_seconds,
        } => {
            let settings = config::load()?;
            let request = Request::SendCommand {
                tab_id,
                command: shell_command,
                separate_output: separate_output.unwrap_or(settings.command_separate_output),
                wait_timeout: wait_timeout_from_seconds(wait_timeout_seconds)?,
            };
            print_result(interface::execute(&settings, request).await)
        }
        CliCommand::View {
            id,
            wait_timeout: wait_timeout_seconds,
            start_token,
        } => {
            let settings = config::load()?;
            let request = Request::View {
                id,
                wait_timeout: wait_timeout_from_seconds(wait_timeout_seconds)?,
                start_token,
            };
            print_result(interface::execute(&settings, request).await)
        }
    }?;
    Ok(std::process::ExitCode::SUCCESS)
}
fn print_result(result: Result<String>) -> Result<()> {
    let text = line_terminated(result?);
    print!("{text}");
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
