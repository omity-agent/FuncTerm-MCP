use super::interface::{self, Args, CliCommand, DaemonCommand, print_result};
use crate::runtime::protocol::{
    EnvironmentSnapshot, KeyboardInput, Request, wait_timeout_from_seconds,
};
use crate::runtime::{config, service_control};
use anyhow::{Context as _, Result};
use base64_turbo::STANDARD;
use clap::Parser as _;
pub(crate) async fn run() -> Result<std::process::ExitCode> {
    let args = Args::parse();
    match args
        .command
        .unwrap_or_else(|| CliCommand::Mcp(super::stdio::Options::default()))
    {
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
        CliCommand::InternalQueryShell {
            require_ordinary,
            command,
        } => {
            return crate::engine::privileges::query_process::execute(&command, require_ordinary)
                .map(crate::shim::process_exit);
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
        #[cfg(windows)]
        CliCommand::InternalCmdPath { startup_directory } => print_result(
            crate::app::command_state::cmd_search_path(&startup_directory),
        ),
        CliCommand::InternalClaimCommand {
            session_root,
            shell,
            output_file,
        } => {
            let receipt = crate::app::command_state::claim_command(
                &session_root,
                shell,
                output_file.as_deref(),
            )?;
            if output_file.is_some() {
                Ok(())
            } else {
                print_result(Ok(receipt))
            }
        }
        CliCommand::InternalWriteDone {
            command_id,
            exit_code,
            time_consumption,
            cwd,
            directory,
            options,
        } => crate::app::command_state::complete(
            &crate::contract::CommandReport {
                command_id: &command_id,
                elapsed: time_consumption,
                cwd: &cwd,
                outcome: crate::contract::CommandOutcome::Returned {
                    exit_code,
                    shell_exit: options.session_exit,
                },
            },
            &directory,
            options,
        ),
        CliCommand::InternalWriteStart {
            command_id,
            directory,
            stdio,
        } => crate::app::command_state::write_start(&command_id, &directory, stdio),
        CliCommand::Mcp(options) => options.run().await,
        CliCommand::Daemon { command } => {
            let mut settings = config::load()?;
            let message = match command {
                DaemonCommand::Start(options) => {
                    options.apply(&mut settings);
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
                DaemonCommand::Restart(options) => {
                    options.apply(&mut settings);
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
            bytes,
            wait_timeout,
        } => {
            let settings = config::load()?;
            let input = match (text, bytes) {
                (Some(input_text), None) => KeyboardInput::Text(input_text),
                (None, Some(encoded)) => KeyboardInput::Bytes(
                    STANDARD
                        .decode(&encoded)
                        .context("invalid bytes keyboard input")?,
                ),
                _ => anyhow::bail!("requires exactly one of --text or --bytes"),
            };
            let request = Request::ManualWrite {
                tab_id,
                input,
                wait_timeout: wait_timeout_from_seconds(wait_timeout)?,
            };
            print_result(interface::execute(&settings, request).await)
        }
        CliCommand::Run {
            tab_id,
            command: shell_command,
            separate_output,
            wait_timeout: wait_timeout_seconds,
        } => {
            let settings = config::load()?;
            let request = Request::Run {
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
