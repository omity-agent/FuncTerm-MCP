use crate::runtime::config;
use anyhow::{Context as _, Result};
use base64_turbo::STANDARD;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
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
        #[arg(long, default_value = "powershell")]
        starting_shell: String,
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
    Query {
        id: String,
    },
}
pub(crate) async fn run() -> Result<()> {
    let args = Args::parse();
    let settings = config::load()?;
    match args.command.unwrap_or(CliCommand::Mcp) {
        CliCommand::Mcp => crate::mcp::run(settings).await,
        CliCommand::Daemon => crate::runtime::daemon::run(settings),
        CliCommand::NewTab {
            starting_directory,
            starting_shell,
        } => print_result(crate::commands::with_daemon(
            &settings.daemon_service_name,
            |call| crate::commands::new_tab(call, starting_directory.as_deref(), &starting_shell),
        )),
        CliCommand::ManualWrite { tab_id, base64 } => {
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
            command,
            waiting: waiting_seconds,
        } => print_result(crate::commands::with_daemon(
            &settings.daemon_service_name,
            |call| crate::commands::send_command(call, tab_id, command, waiting_seconds),
        )),
        CliCommand::Query { id } => print_result(crate::commands::with_daemon(
            &settings.daemon_service_name,
            |call| crate::commands::query(call, id),
        )),
    }
}
fn print_result(result: Result<String>) -> Result<()> {
    let text = result?;
    println!("{text}");
    Ok(())
}
