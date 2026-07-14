use crate::runtime::config;
use crate::runtime::protocol::KeyboardInput;
use crate::shell::ShellChoice;
use anyhow::{Context as _, Result};
use base64_turbo::STANDARD;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
#[derive(Parser)]
#[command(version, about)]
struct Args {
    #[command(subcommand)]
    command: Option<CliCommand>,
}
#[derive(Subcommand)]
enum CliCommand {
    Mcp,
    Daemon,
    NewTab {
        #[arg(long)]
        starting_directory: Option<PathBuf>,
        # [arg (long , default_value = "powershell" , value_parser = ShellChoice :: from_canonical_name)]
        starting_shell: ShellChoice,
    },
    ManualWrite {
        tab_id: String,
        #[arg(long, required_unless_present = "base64", conflicts_with = "base64")]
        text: Option<String>,
        #[arg(long, required_unless_present = "text", conflicts_with = "text")]
        base64: Option<String>,
        #[arg(long, default_value_t = 0.0)]
        waiting: f64,
    },
    SendCommand {
        tab_id: String,
        #[arg(long)]
        command: String,
        #[arg(long, default_value_t = 0.0)]
        waiting: f64,
    },
    View {
        id: String,
        #[arg(long, default_value_t = 0.0)]
        waiting: f64,
    },
    #[command(hide = true)]
    InternalLaunchDaemon,
    #[command(hide = true)]
    InternalWriteDone {
        #[arg(long)]
        command_id: String,
        #[arg(long)]
        exit_code: i32,
        #[arg(long)]
        time_consumption: String,
        #[arg(long)]
        cwd: String,
        #[arg(long)]
        directory: PathBuf,
    },
    #[command(hide = true)]
    InternalWriteStart {
        #[arg(long)]
        command_id: String,
        #[arg(long)]
        directory: PathBuf,
    },
    #[command(hide = true)]
    InternalEnsureShims {
        #[arg(long)]
        directory: PathBuf,
    },
}
pub(crate) async fn run() -> Result<()> {
    let args = Args::parse();
    match args.command.unwrap_or(CliCommand::Mcp) {
        CliCommand::InternalEnsureShims { directory } => {
            crate::shell::shims::ensure_directory(&directory)
        }
        CliCommand::InternalLaunchDaemon => crate::runtime::client::run_daemon_launcher(),
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
        CliCommand::Daemon => crate::runtime::daemon::run(config::load()?),
        CliCommand::NewTab {
            starting_directory,
            starting_shell,
        } => {
            let settings = config::load()?;
            print_result(crate::commands::with_daemon(
                &settings.daemon_service_name,
                |call| {
                    crate::commands::new_tab(call, starting_directory.as_deref(), starting_shell)
                },
            ))
        }
        CliCommand::ManualWrite {
            tab_id,
            text,
            base64,
            waiting,
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
            print_result(crate::commands::with_daemon(
                &settings.daemon_service_name,
                |call| crate::commands::manual_write(call, tab_id, input, waiting),
            ))
        }
        CliCommand::SendCommand {
            tab_id,
            command: shell_command,
            waiting: waiting_seconds,
        } => {
            let settings = config::load()?;
            print_result(crate::commands::with_daemon(
                &settings.daemon_service_name,
                |call| crate::commands::send_command(call, tab_id, shell_command, waiting_seconds),
            ))
        }
        CliCommand::View {
            id,
            waiting: waiting_seconds,
        } => {
            let settings = config::load()?;
            print_result(crate::commands::with_daemon(
                &settings.daemon_service_name,
                |call| crate::commands::view(call, id, waiting_seconds),
            ))
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
#[cfg(test)]
#[path = "cli/tests.rs"]
mod tests;
