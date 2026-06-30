use crate::runtime::config::Settings;
use crate::runtime::ipc::{Payload, Request, Response};
use crate::runtime::session::Manager;
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use base64_turbo::STANDARD;
use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{TcpListener, TcpStream};
use std::thread;
pub(crate) fn run(settings: Settings) -> Result<()> {
    let listener = TcpListener::bind(&settings.daemon_address)
        .with_context(|| format!("failed to bind {}", settings.daemon_address))?;
    let manager = Arc::new(Manager::new(settings)?);
    for incoming_stream in listener.incoming() {
        let accepted_stream = incoming_stream.context("failed to accept daemon connection")?;
        let shared_manager = Arc::clone(&manager);
        thread::spawn(move || {
            if let Err(error) = handle_connection(accepted_stream, &shared_manager) {
                eprintln!("{error:#}");
            }
        });
    }
    Ok(())
}
fn handle_connection(stream: TcpStream, manager: &Arc<Manager>) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone().context("failed to clone stream")?);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .context("failed to read request line")?;
    let request =
        sonic_rs::from_str::<Request>(&request_line).context("failed to parse request")?;
    let response = match dispatch(manager, request) {
        Ok(payload) => Response::Ok { payload },
        Err(error) => Response::Err {
            message: format!("{error:#}"),
        },
    };
    let mut writer = stream;
    let line = sonic_rs::to_string(&response).context("failed to serialize response")?;
    writer
        .write_all(line.as_bytes())
        .context("failed to write response")?;
    writer.write_all(b"\n").context("failed to finish response")
}
fn dispatch(manager: &Arc<Manager>, request: Request) -> Result<Payload> {
    match request {
        Request::Ping => Ok(Payload::Pong),
        Request::NewShell { cwd, shell } => {
            let shell_id = manager.new_shell(&cwd, shell)?;
            Ok(Payload::ShellCreated { shell_id })
        }
        Request::WriteKeyboard {
            shell_id,
            bytes_base64,
        } => {
            let bytes = STANDARD
                .decode(bytes_base64)
                .context("failed to decode keyboard bytes")?;
            manager.write_keyboard(&shell_id, &bytes)?;
            Ok(Payload::KeyboardWritten)
        }
        Request::SendCommand {
            shell_id,
            command,
            wait_ms,
        } => {
            let (command_id, end_reason, query) =
                manager.send_command(&shell_id, &command, wait_ms)?;
            Ok(Payload::CommandAccepted {
                command_id,
                end_reason,
                query,
            })
        }
        Request::Query { id } => Ok(Payload::Query(manager.query(&id)?)),
    }
}
