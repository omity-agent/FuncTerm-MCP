mod types;
use crate::runtime::client;
use crate::runtime::config::Settings;
use alloc::sync::Arc;
use anyhow::{Result, anyhow};
use rmcp::{
    ServerHandler, ServiceExt as _,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router,
};
use std::sync::Mutex;
use types::{ManualWriteRequest, NewTabRequest, SendCommandRequest, ViewRequest};
#[derive(Clone, Debug)]
struct McpServer {
    daemon_service_name: String,
    daemon: Arc<Mutex<Option<client::DaemonClient>>>,
    tool_router: ToolRouter<Self>,
}
#[expect(
    clippy::unused_async_trait_impl,
    reason = "rmcp's tool_handler macro generates the async ServerHandler implementation"
)]
# [tool_handler (router = self . tool_router)]
impl ServerHandler for McpServer {}
# [tool_router (router = tool_router)]
impl McpServer {
    fn new(daemon_service_name: String) -> Self {
        Self {
            daemon_service_name,
            daemon: Arc::new(Mutex::new(None)),
            tool_router: Self::tool_router(),
        }
    }
    fn call(
        &self,
        request: &crate::runtime::protocol::Request,
    ) -> Result<crate::runtime::protocol::Payload> {
        let mut daemon = self.daemon.lock().map_err(|error| anyhow!("{error}"))?;
        if daemon.is_none() {
            client::ensure_daemon(&self.daemon_service_name)?;
            *daemon = Some(client::DaemonClient::connect(&self.daemon_service_name)?);
        }
        daemon
            .as_ref()
            .ok_or_else(|| anyhow!("daemon returned an unexpected response"))?
            .call(request)
    }
    #[tool(name = "new_tab", description = "打开一个新的终端标签页。")]
    async fn new_tab(
        &self,
        Parameters(request): Parameters<NewTabRequest>,
    ) -> Result<String, String> {
        crate::commands::new_tab(
            |command| self.call(command),
            request.starting_directory_path(),
            &request.starting_shell,
        )
        .map_err(error_text)
    }
    #[tool(
        name = "manual_write",
        description = "手动写入键盘输入。该工具用于使用 TUI 程序、发送快捷键等 send_command 无法覆盖的场景。使用时需在 text 和 bytes 中选择一个传入。"
    )]
    async fn manual_write(
        &self,
        Parameters(request): Parameters<ManualWriteRequest>,
    ) -> Result<String, String> {
        let (tab_id, bytes) = request.into_parts().map_err(error_text)?;
        crate::commands::manual_write(|command| self.call(command), tab_id, bytes)
            .map_err(error_text)
    }
    #[tool(
        name = "send_command",
        description = "执行命令。获得等待时间结束前该命令产生的所有输出。"
    )]
    async fn send_command(
        &self,
        Parameters(request): Parameters<SendCommandRequest>,
    ) -> Result<String, String> {
        crate::commands::send_command(
            |command| self.call(command),
            request.tab_id,
            request.command,
            request.waiting,
        )
        .map_err(error_text)
    }
    #[tool(name = "view", description = "工具会在等待结束或命令结束时输出。")]
    async fn view(&self, Parameters(request): Parameters<ViewRequest>) -> Result<String, String> {
        crate::commands::view(|command| self.call(command), request.id, request.waiting)
            .map_err(error_text)
    }
}
pub(crate) async fn run(settings: Settings) -> Result<()> {
    let service = McpServer::new(settings.daemon_service_name)
        .serve(rmcp::transport::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
fn error_text(error: impl core::fmt::Display) -> String {
    error.to_string()
}
