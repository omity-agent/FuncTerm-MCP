use alloc::sync::Arc;
use anyhow::Result;
use std::sync::{Mutex, MutexGuard};
use tastty_core::{HostProfile, Parser, host_reply::auto_reply_bytes};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
pub(super) type TerminalParser = Parser;
pub(super) type TerminalWriter = Arc<tokio::sync::Mutex<Box<dyn AsyncWrite + Send + Unpin>>>;
pub(super) fn start_reader<R>(
    screen: Arc<Mutex<TerminalParser>>,
    writer: TerminalWriter,
    runtime: &tokio::runtime::Runtime,
    mut reader: R,
) where
    R: AsyncRead + Send + Unpin + 'static,
{
    runtime.spawn(async move {
        let host = HostProfile::default();
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(read_len) => {
                    let Some(chunk) = buffer.get(..read_len) else {
                        break;
                    };
                    let replies = match screen.lock() {
                        Ok(mut parser) => {
                            parser.process(chunk);
                            parser
                                .screen_mut()
                                .drain_events()
                                .into_iter()
                                .filter_map(|event| auto_reply_bytes(&event, &host))
                                .collect::<Vec<_>>()
                        }
                        Err(_) => break,
                    };
                    write_replies(&writer, &replies).await;
                }
            }
        }
    });
}
pub(super) fn screen_title(parser: &TerminalParser) -> String {
    parser.screen().title().to_owned()
}
async fn write_replies(writer: &TerminalWriter, replies: &[Vec<u8>]) {
    if replies.is_empty() {
        return;
    }
    let mut locked_writer = writer.lock().await;
    for reply in replies {
        if let Err(error) = locked_writer.as_mut().write_all(reply).await {
            eprintln!("failed to answer terminal host query: {error}");
            return;
        }
    }
    if let Err(error) = locked_writer.as_mut().flush().await {
        eprintln!("failed to flush terminal host query replies: {error}");
    }
}
pub(super) fn lock_mutex<'guard, T>(
    mutex: &'guard Mutex<T>,
    name: &str,
) -> Result<MutexGuard<'guard, T>> {
    mutex
        .lock()
        .map_err(|error| anyhow::anyhow!("{name} mutex poisoned: {error}"))
}
