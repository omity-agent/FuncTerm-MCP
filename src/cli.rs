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
    NewShell {
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long, default_value = "powershell")]
        shell: String,
    },
    WriteKeyboard {
        shell_id: String,
        #[arg(long)]
        base64: String,
    },
    SendCommand {
        shell_id: String,
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
        CliCommand::NewShell { cwd, shell } => print_result(crate::commands::with_daemon(
            &settings.daemon_service_name,
            |call| crate::commands::new_shell(call, cwd.as_deref(), &shell),
        )),
        CliCommand::WriteKeyboard { shell_id, base64 } => {
            let bytes = STANDARD
                .decode(&base64)
                .context("invalid base64 keyboard input")?;
            print_result(crate::commands::with_daemon(
                &settings.daemon_service_name,
                |call| crate::commands::write_keyboard(call, shell_id, bytes),
            ))
        }
        CliCommand::SendCommand {
            shell_id,
            command,
            waiting: waiting_seconds,
        } => print_result(crate::commands::with_daemon(
            &settings.daemon_service_name,
            |call| crate::commands::send_command(call, shell_id, command, waiting_seconds),
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
