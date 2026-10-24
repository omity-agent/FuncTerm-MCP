use super::{McpServer, invocation::Backend};
use crate::runtime::{config::Settings, session::Manager};
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use axum::Router;
use core::net::SocketAddr;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_util::sync::CancellationToken;
pub(crate) struct HttpServer {
    address: SocketAddr,
    cancellation: CancellationToken,
    task: Option<JoinHandle<Result<()>>>,
}
impl HttpServer {
    pub(crate) async fn bind(settings: Settings, manager: Arc<Manager>) -> Result<Self> {
        let http = &settings.mcp.http;
        let listener = TcpListener::bind(http.bind_address)
            .await
            .with_context(|| {
                format!("failed to bind MCP HTTP listener at {}", http.bind_address)
            })?;
        let address = listener
            .local_addr()
            .context("failed to read MCP HTTP address")?;
        let cancellation = CancellationToken::new();
        let config = StreamableHttpServerConfig::default()
            .with_allowed_hosts(http.allowed_hosts.clone())
            .with_allowed_origins(http.allowed_origins.clone())
            .enforce_origin_validation()
            .with_max_request_body_bytes(http.max_request_body_bytes)
            .with_sse_retry(None)
            .with_cancellation_token(cancellation.clone());
        let mut sessions = LocalSessionManager::default();
        sessions.session_config.sse_retry = None;
        sessions.session_config.init_timeout = Some(settings.ipc.setup_timeout()?);
        let path = http.path.clone();
        let privilege_mode = http.privilege_mode;
        let output_mode = http.output_mode;
        let handler = McpServer::new(
            settings,
            privilege_mode,
            output_mode,
            Backend::direct(manager),
        )?;
        let service =
            StreamableHttpService::new(move || Ok(handler.clone()), Arc::new(sessions), config);
        let router = Router::new().route_service(&path, service);
        let shutdown = cancellation.clone();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await
                .context("MCP HTTP listener failed")
        });
        tracing::info!(%address, %path, "MCP HTTP listener ready without authentication");
        Ok(Self {
            address,
            cancellation,
            task: Some(task),
        })
    }
    pub(crate) const fn address(&self) -> SocketAddr {
        self.address
    }
    pub(crate) async fn failure(&mut self) -> anyhow::Error {
        let Some(task) = self.task.as_mut() else {
            return anyhow::anyhow!("MCP HTTP task has already completed");
        };
        let result = task.await;
        self.task.take();
        match result {
            Ok(Ok(())) => anyhow::anyhow!("MCP HTTP listener stopped unexpectedly"),
            Ok(Err(error)) => error,
            Err(error) => anyhow::Error::new(error).context("MCP HTTP task failed"),
        }
    }
    pub(crate) async fn stop(mut self, timeout: core::time::Duration) -> Result<()> {
        self.cancellation.cancel();
        if let Some(task) = self.task.as_mut() {
            tokio::time::timeout(timeout, task)
                .await
                .context("MCP HTTP shutdown timed out")?
                .context("MCP HTTP task failed during shutdown")??;
        }
        self.task.take();
        Ok(())
    }
}
impl Drop for HttpServer {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Some(task) = self.task.as_ref() {
            task.abort();
        }
    }
}
