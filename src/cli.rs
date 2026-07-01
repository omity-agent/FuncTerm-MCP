use crate::runtime::client;
use crate::runtime::config;
use crate::runtime::protocol::{Payload, Request, waiting_from_seconds};
use crate::runtime::working_dir;
use crate::shell::ShellChoice;
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
        CliCommand::NewShell { cwd, shell } => {
            client::ensure_daemon(&settings.daemon_service_name)?;
            let shell_choice = ShellChoice::parse(&shell)?;
            let resolved_cwd = working_dir::resolve(cwd.as_deref())?;
            let payload = client::call(
                &settings.daemon_service_name,
                &Request::NewShell {
                    cwd: resolved_cwd,
                    shell: shell_choice,
                },
            )?;
            print_payload(&payload);
            Ok(())
        }
        CliCommand::WriteKeyboard { shell_id, base64 } => {
            client::ensure_daemon(&settings.daemon_service_name)?;
            let bytes = STANDARD
                .decode(&base64)
                .context("invalid base64 keyboard input")?;
            let payload = client::call(
                &settings.daemon_service_name,
                &Request::WriteKeyboard { shell_id, bytes },
            )?;
            print_payload(&payload);
            Ok(())
        }
        CliCommand::SendCommand {
            shell_id,
            command,
            waiting: waiting_seconds,
        } => {
            client::ensure_daemon(&settings.daemon_service_name)?;
            let waiting = waiting_from_seconds(waiting_seconds)?;
            let payload = client::call(
                &settings.daemon_service_name,
                &Request::SendCommand {
                    shell_id,
                    command,
                    waiting,
                },
            )?;
            print_payload(&payload);
            Ok(())
        }
        CliCommand::Query { id } => {
            client::ensure_daemon(&settings.daemon_service_name)?;
            let payload = client::call(&settings.daemon_service_name, &Request::Query { id })?;
            print_payload(&payload);
            Ok(())
        }
    }
}
fn print_payload(payload: &Payload) {
    println!("{}", payload.to_plain_text());
}
