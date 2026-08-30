mod descriptions;
mod output;
mod types;
use crate::runtime::client;
use crate::runtime::config::Settings;
use anyhow::Result;
use rmcp::{
    ServerHandler, ServiceExt as _,
    handler::server::{router::tool::ToolRouter, tool::schema_for_output, wrapper::Parameters},
    model::CallToolResult,
    tool, tool_handler, tool_router,
};
use types::{ManualWriteRequest, NewTabRequest, SendCommandRequest, ViewRequest};
#[derive(Clone, Debug)]
struct McpServer {
    daemon_service_name: String,
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
    fn new(settings: Settings) -> Result<Self> {
        let mut tool_router = Self::tool_router();
        descriptions::apply(&mut tool_router, &settings.mcp)?;
        Ok(Self {
            daemon_service_name: settings.daemon_service_name,
            tool_router,
        })
    }
    fn call(
        &self,
        request: &crate::runtime::protocol::Request,
    ) -> Result<crate::runtime::protocol::Payload> {
        client::ensure_daemon(&self.daemon_service_name)?;
        client::DaemonClient::connect(&self.daemon_service_name)?.call(request)
    }
    # [tool (name = "new_tab" , output_schema = schema_for_output ::< output :: NewTabOutput < 'static > > ())]
    async fn new_tab(
        &self,
        Parameters(request): Parameters<NewTabRequest>,
    ) -> Result<CallToolResult, String> {
        let payload = crate::commands::new_tab_payload(
            |command| self.call(command),
            request.starting_directory_path(),
            request.starting_shell,
        )
        .map_err(error_text)?;
        output::new_tab(payload)
    }
    # [tool (name = "manual_write" , output_schema = schema_for_output ::< output :: ManualWriteOutput < 'static > > ())]
    async fn manual_write(
        &self,
        Parameters(request): Parameters<ManualWriteRequest>,
    ) -> Result<CallToolResult, String> {
        let (tab_id, input, waiting) = request.into_parts().map_err(error_text)?;
        let payload = crate::commands::manual_write_payload(
            |command| self.call(command),
            tab_id,
            input,
            waiting,
        )
        .map_err(error_text)?;
        output::manual_write(payload)
    }
    # [tool (name = "send_command" , output_schema = schema_for_output ::< output :: SendCommandOutput < 'static > > ())]
    async fn send_command(
        &self,
        Parameters(request): Parameters<SendCommandRequest>,
    ) -> Result<CallToolResult, String> {
        let payload = crate::commands::send_command_payload(
            |command| self.call(command),
            request.tab_id,
            request.command,
            request.waiting,
        )
        .map_err(error_text)?;
        output::send_command(payload)
    }
    # [tool (name = "view" , output_schema = schema_for_output ::< output :: ViewOutput < 'static > > ())]
    async fn view(
        &self,
        Parameters(request): Parameters<ViewRequest>,
    ) -> Result<CallToolResult, String> {
        let payload = crate::commands::view_payload(
            |command| self.call(command),
            request.id,
            request.waiting,
        )
        .map_err(error_text)?;
        output::view(payload)
    }
}
pub(crate) async fn run(settings: Settings) -> Result<()> {
    let service = McpServer::new(settings)?
        .serve(rmcp::transport::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
fn error_text(error: impl core::fmt::Display) -> String {
    error.to_string()
}
