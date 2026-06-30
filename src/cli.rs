use crate::client;
use crate::config;
use crate::ipc::{Payload, Request};
use crate::shell::ShellChoice;
use anyhow::{Context as _, Result};
use base64_turbo::STANDARD;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use uuid::Uuid;
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
        cwd: PathBuf,
        #[arg(long, default_value = "powershell")]
        shell: String,
    },
    WriteKeyboard {
        shell_id: Uuid,
        #[arg(long)]
        base64: String,
    },
    SendCommand {
        shell_id: Uuid,
        #[arg(long)]
        command: String,
        #[arg(long, default_value_t = 0)]
        wait_ms: u64,
    },
    Query {
        id: Uuid,
    },
}
pub(crate) async fn run() -> Result<()> {
    let args = Args::parse();
    let settings = config::load()?;
    match args.command.unwrap_or(CliCommand::Mcp) {
        CliCommand::Mcp => crate::mcp::run(settings).await,
        CliCommand::Daemon => crate::daemon::run(settings),
        CliCommand::NewShell { cwd, shell } => {
            client::ensure_daemon(&settings.daemon_address)?;
            let shell_choice = ShellChoice::parse(&shell)?;
            let payload = client::call(
                &settings.daemon_address,
                &Request::NewShell {
                    cwd,
                    shell: shell_choice,
                },
            )?;
            print_payload(&payload)
        }
        CliCommand::WriteKeyboard { shell_id, base64 } => {
            client::ensure_daemon(&settings.daemon_address)?;
            let bytes = STANDARD
                .decode(&base64)
                .context("invalid base64 keyboard input")?;
            let bytes_base64 = STANDARD.encode(&bytes);
            let payload = client::call(
                &settings.daemon_address,
                &Request::WriteKeyboard {
                    shell_id,
                    bytes_base64,
                },
            )?;
            print_payload(&payload)
        }
        CliCommand::SendCommand {
            shell_id,
            command,
            wait_ms,
        } => {
            client::ensure_daemon(&settings.daemon_address)?;
            let payload = client::call(
                &settings.daemon_address,
                &Request::SendCommand {
                    shell_id,
                    command,
                    wait_ms,
                },
            )?;
            print_payload(&payload)
        }
        CliCommand::Query { id } => {
            client::ensure_daemon(&settings.daemon_address)?;
            let payload = client::call(&settings.daemon_address, &Request::Query { id })?;
            print_payload(&payload)
        }
    }
}
fn print_payload(payload: &Payload) -> Result<()> {
    let text = sonic_rs::to_string_pretty(payload).context("failed to serialize payload")?;
    println!("{text}");
    Ok(())
}
