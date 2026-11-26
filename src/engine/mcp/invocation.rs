use crate::runtime::{
    client::DaemonClient,
    config::Settings,
    daemon::dispatch::Dispatcher,
    protocol::{Payload, Request},
};
use alloc::sync::Arc;
use anyhow::Result;
use std::time::Instant;
#[derive(Clone)]
pub(super) enum Backend {
    Ipc,
    Direct(Arc<Dispatcher>),
}
pub(super) enum PreparedBackend<'dispatcher> {
    Ipc(Option<DaemonClient>),
    Direct(&'dispatcher Dispatcher),
}
impl Backend {
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "reuse the dispatcher without cloning its Arc"
    )]
    pub(super) async fn prepare(&self, settings: &Settings) -> Result<PreparedBackend<'_>> {
        match self {
            Self::Ipc => Ok(PreparedBackend::Ipc(Some(
                DaemonClient::connect_or_start(settings).await?,
            ))),
            Self::Direct(dispatcher) => Ok(PreparedBackend::Direct(dispatcher)),
        }
    }
}
impl<'dispatcher> PreparedBackend<'dispatcher> {
    pub(super) fn dispatch(
        &mut self,
        settings: &'dispatcher Settings,
        mut request: Request,
        batch_started: Instant,
    ) -> impl Future<Output = Result<Payload>> + use<'dispatcher> {
        #[expect(
            clippy::pattern_type_mismatch,
            reason = "move the initial connection without taking ownership of the batch"
        )]
        let transport = match self {
            Self::Ipc(connection) => Self::Ipc(connection.take()),
            Self::Direct(dispatcher) => Self::Direct(dispatcher),
        };
        async move {
            #[expect(
                clippy::pattern_type_mismatch,
                reason = "adjust the budget without rebuilding the request or moving its input"
            )]
            match &mut request {
                Request::ManualWrite {
                    wait_timeout,
                    tab_id: _,
                    input: _,
                }
                | Request::Run {
                    wait_timeout,
                    tab_id: _,
                    command: _,
                    separate_output: _,
                }
                | Request::View {
                    wait_timeout,
                    id: _,
                    start_token: _,
                } => {
                    *wait_timeout = wait_timeout.saturating_sub(batch_started.elapsed());
                }
                Request::Ping
                | Request::Shutdown
                | Request::NewTab {
                    starting_directory: _,
                    starting_shell: _,
                    load_profile: _,
                    elevated: _,
                    environment: _,
                }
                | Request::Close { tab_id: _ } => {}
            }
            match transport {
                Self::Direct(dispatcher) => dispatcher.execute(request).await,
                Self::Ipc(initial) => {
                    let mut connection = match initial {
                        Some(client) => client,
                        None => DaemonClient::connect(settings).await?,
                    };
                    connection.call(request).await
                }
            }
        }
    }
}
