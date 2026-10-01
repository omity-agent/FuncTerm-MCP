mod arguments;
mod batch;
mod collection;
mod descriptions;
mod operations;
mod output;
use crate::runtime::config::Settings;
use anyhow::Result;
use arguments::{ManualWriteExec, NewTabExec, SendCommandExec, ViewRequest};
use batch::{ExecBatch, TimedBatch};
use collection::BatchOutput;
use rmcp::{
    ServerHandler, ServiceExt as _,
    handler::server::{router::tool::ToolRouter, tool::schema_for_output, wrapper::Parameters},
    model::CallToolResult,
    tool, tool_handler, tool_router,
};
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
    # [tool (name = "new_tab" , output_schema = schema_for_output ::< BatchOutput < output :: NewTabOutput <'static >>> ())]
    async fn new_tab(
        &self,
        Parameters(request): Parameters<ExecBatch<NewTabExec>>,
    ) -> Result<CallToolResult, String> {
        batch::run(&self.daemon_service_name, request.exec, None).await
    }
    # [tool (name = "manual_write" , output_schema = schema_for_output ::< BatchOutput < output :: ManualWriteOutput <'static >>> ())]
    async fn manual_write(
        &self,
        Parameters(request): Parameters<TimedBatch<ManualWriteExec>>,
    ) -> Result<CallToolResult, String> {
        batch::run(
            &self.daemon_service_name,
            request.exec,
            Some(request.wait_timeout),
        )
        .await
    }
    # [tool (name = "send_command" , output_schema = schema_for_output ::< BatchOutput < output :: SendCommandOutput <'static >>> ())]
    async fn send_command(
        &self,
        Parameters(request): Parameters<TimedBatch<SendCommandExec>>,
    ) -> Result<CallToolResult, String> {
        batch::run(
            &self.daemon_service_name,
            request.exec,
            Some(request.wait_timeout),
        )
        .await
    }
    # [tool (name = "view" , output_schema = schema_for_output ::< BatchOutput < output :: ViewOutput <'static >>> ())]
    async fn view(
        &self,
        Parameters(request): Parameters<ViewRequest>,
    ) -> Result<CallToolResult, String> {
        batch::run(
            &self.daemon_service_name,
            request.ids.into_iter().map(operations::LookupId).collect(),
            Some(request.wait_timeout),
        )
        .await
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
