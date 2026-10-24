mod arguments;
mod batch;
mod collection;
mod descriptions;
mod invocation;
mod operations;
mod output;
mod streamable;
use crate::engine::privileges::PrivilegeMode;
use crate::runtime::config::Settings;
use crate::runtime::protocol::OutputMode;
use anyhow::Result;
use arguments::{
    FixedCommandExec, FixedNewTabExec, ManualWriteExec, NewTabExec, SendCommandExec, ViewRequest,
};
use batch::{ExecBatch, TimedBatch};
use collection::BatchOutput;
use invocation::Backend;
use rmcp::{
    ServerHandler, ServiceExt as _,
    handler::server::{
        router::tool::ToolRouter,
        tool::{ToolCallContext, schema_for_output},
        wrapper::Parameters,
    },
    model::{CallToolRequestParams, CallToolResponse, CallToolResult, ProtocolVersion},
    service::RequestContext,
    tool, tool_handler, tool_router,
};
pub(crate) use streamable::HttpServer;
#[derive(Clone)]
struct McpServer {
    settings: Settings,
    backend: Backend,
    privilege_mode: Option<PrivilegeMode>,
    output_mode: Option<OutputMode>,
    tool_router: ToolRouter<Self>,
}
#[expect(
    clippy::unused_async_trait_impl,
    reason = "rmcp's tool_handler macro generates the async ServerHandler implementation"
)]
# [tool_handler (router = self . tool_router)]
impl ServerHandler for McpServer {
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<rmcp::RoleServer>,
    ) -> Result<CallToolResponse, rmcp::ErrorData> {
        let june_revision = context.protocol_version() == Some(ProtocolVersion::V_2025_06_18);
        let call = ToolCallContext::new(self, request, context);
        if june_revision {
            let route = self
                .tool_router
                .map
                .get(call.name())
                .ok_or_else(|| rmcp::ErrorData::invalid_params("tool not found", None))?;
            return (route.call)(call).await;
        }
        self.tool_router.call(call).await
    }
}
# [tool_router (router = tool_router)]
impl McpServer {
    fn new(
        settings: Settings,
        privilege_mode: Option<PrivilegeMode>,
        output_mode: Option<OutputMode>,
        backend: Backend,
    ) -> Result<Self> {
        let tool_router = descriptions::router(&settings.mcp, privilege_mode, output_mode)?;
        Ok(Self {
            settings,
            backend,
            privilege_mode,
            output_mode,
            tool_router,
        })
    }
    # [tool (name = "new_tab" , output_schema = schema_for_output ::< BatchOutput < output :: NewTabOutput <'static >>> ())]
    async fn new_tab(
        &self,
        Parameters(request): Parameters<ExecBatch<NewTabExec>>,
    ) -> Result<CallToolResult, String> {
        batch::run(&self.settings, &self.backend, request.exec, None).await
    }
    # [tool (name = "fixed_new_tab" , output_schema = schema_for_output ::< BatchOutput < output :: NewTabOutput <'static >>> ())]
    async fn fixed_new_tab(
        &self,
        Parameters(request): Parameters<ExecBatch<FixedNewTabExec>>,
    ) -> Result<CallToolResult, String> {
        let mode = self
            .privilege_mode
            .ok_or("MCP privilege mode is not fixed")?;
        let entries = request
            .exec
            .into_iter()
            .map(|entry| entry.with_privilege(mode.elevated()))
            .collect();
        batch::run(&self.settings, &self.backend, entries, None).await
    }
    # [tool (name = "manual_write" , output_schema = schema_for_output ::< BatchOutput < output :: ManualWriteOutput <'static >>> ())]
    async fn manual_write(
        &self,
        Parameters(request): Parameters<TimedBatch<ManualWriteExec>>,
    ) -> Result<CallToolResult, String> {
        batch::run(
            &self.settings,
            &self.backend,
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
            &self.settings,
            &self.backend,
            request.exec,
            Some(request.wait_timeout),
        )
        .await
    }
    #[tool(name = "fixed_send_command", output_schema = schema_for_output::<BatchOutput<output::SendCommandOutput<'static>>>())]
    async fn fixed_send_command(
        &self,
        Parameters(request): Parameters<TimedBatch<FixedCommandExec>>,
    ) -> Result<CallToolResult, String> {
        let mode = self.output_mode.ok_or("MCP output mode is not fixed")?;
        let entries = request
            .exec
            .into_iter()
            .map(|entry| entry.with_output(mode.separate_output()))
            .collect();
        batch::run(
            &self.settings,
            &self.backend,
            entries,
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
            &self.settings,
            &self.backend,
            request
                .ids
                .into_iter()
                .map(|id| operations::LookupId {
                    id,
                    start_token: request.start_token,
                })
                .collect(),
            Some(request.wait_timeout),
        )
        .await
    }
}
pub(crate) async fn run(
    settings: Settings,
    privilege_mode: Option<PrivilegeMode>,
    output_mode: Option<OutputMode>,
) -> Result<()> {
    let service = McpServer::new(settings, privilege_mode, output_mode, Backend::ipc())?
        .serve(rmcp::transport::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
fn error_text(error: impl core::fmt::Display) -> String {
    error.to_string()
}
