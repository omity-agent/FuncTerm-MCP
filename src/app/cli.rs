use crate::runtime::config;
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
        #[arg(long)]
        base64: String,
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
        CliCommand::ManualWrite { tab_id, base64 } => {
            let settings = config::load()?;
            let bytes = STANDARD
                .decode(&base64)
                .context("invalid base64 keyboard input")?;
            print_result(crate::commands::with_daemon(
                &settings.daemon_service_name,
                |call| crate::commands::manual_write(call, tab_id, bytes),
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
    crate::app::done::write(
        &crate::app::done::DoneOutput {
            command_id,
            exit_code,
            time_consumption,
            cwd,
        },
        directory,
    )
}
#[cfg(test)]
mod tests {
    use serde::Deserialize;
    #[derive(Deserialize)]
    struct WrittenDone {
        command_id: String,
        exit_code: i32,
        time_consumption: String,
        cwd: String,
    }
    #[test]
    fn internal_done_writer_serializes_json_strings() {
        let directory = crate::test_fs::temp_case("internal-done-writer");
        super::write_done(
            "command\"id",
            7,
            "123.456ms",
            "cwd\nwith\\chars",
            &directory,
        )
        .unwrap();
        let text = std::fs::read_to_string(
            directory
                .join(crate::contract::COMMAND_STATE_DIRECTORY)
                .join(crate::contract::DONE_FILE),
        )
        .unwrap();
        let done = sonic_rs::from_str::<WrittenDone>(&text).unwrap();
        assert_eq!(done.command_id, "command\"id");
        assert_eq!(done.exit_code, 7_i32);
        assert_eq!(done.time_consumption, "123.456ms");
        assert_eq!(done.cwd, "cwd\nwith\\chars");
        std::fs::remove_dir_all(directory).unwrap();
    }
}
