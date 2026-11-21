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
use crate::runtime::protocol::time_consumption;
use anyhow::Result;
use arguments::{
    FixedCommandInput, FixedNewTabOptions, ManualWriteRequest, NewTabOptions, NewTabRequest,
    RunInput, RunRequest, ViewRequest,
};
pub(crate) use batch::{WaitTimeoutMultiplier, WaitTimeoutSettings};
use collection::BatchOutput;
use invocation::Backend;
use output::internal_error;
use rmcp::{
    ErrorData, ServerHandler, ServiceExt as _,
    handler::server::{
        router::tool::ToolRouter,
        tool::{ToolCallContext, schema_for_output},
        wrapper::Parameters,
    },
    model::{
        CacheScope, CallToolRequestParams, CallToolResponse, CallToolResult, ListToolsResult,
        PaginatedRequestParams, ProtocolVersion, ResultType,
    },
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
# [tool_handler (router = self . tool_router)]
impl ServerHandler for McpServer {
    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<rmcp::RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        let cache_hints = context
            .protocol_version()
            .is_some_and(|version| version >= ProtocolVersion::V_2026_07_28);
        core::future::ready(Ok(ListToolsResult {
            result_type: Some(ResultType::COMPLETE),
            tools: self.tool_router.list_all(),
            meta: None,
            next_cursor: None,
            ttl_ms: cache_hints.then_some(0),
            cache_scope: cache_hints.then_some(CacheScope::Public),
        }))
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<rmcp::RoleServer>,
    ) -> Result<CallToolResponse, rmcp::ErrorData> {
        let (response, processing) =
            time_consumption::capture(self.settings.benchmark.processing_timing, async {
                let june_revision =
                    context.protocol_version() == Some(ProtocolVersion::V_2025_06_18);
                let call = ToolCallContext::new(self, request, context);
                if june_revision {
                    let route =
                        self.tool_router.map.get(call.name()).ok_or_else(|| {
                            rmcp::ErrorData::invalid_params("tool not found", None)
                        })?;
                    return (route.call)(call).await;
                }
                self.tool_router.call(call).await
            })
            .await;
        output::processing(response?, processing)
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
        Parameters(request): Parameters<NewTabRequest<NewTabOptions>>,
    ) -> Result<CallToolResult, ErrorData> {
        batch::run(&self.settings, &self.backend, request.tabs, None).await
    }
    # [tool (name = "fixed_new_tab" , output_schema = schema_for_output ::< BatchOutput < output :: NewTabOutput <'static >>> ())]
    async fn fixed_new_tab(
        &self,
        Parameters(request): Parameters<NewTabRequest<FixedNewTabOptions>>,
    ) -> Result<CallToolResult, ErrorData> {
        let mode = self
            .privilege_mode
            .ok_or_else(|| internal_error("MCP privilege mode is not fixed"))?;
        let entries = request
            .tabs
            .into_iter()
            .map(|entry| entry.with_privilege(mode.elevated()))
            .collect();
        batch::run(&self.settings, &self.backend, entries, None).await
    }
    # [tool (name = "manual_write" , output_schema = schema_for_output ::< BatchOutput < output :: ManualWriteOutput <'static >>> ())]
    async fn manual_write(
        &self,
        Parameters(request): Parameters<ManualWriteRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        batch::run(
            &self.settings,
            &self.backend,
            request.inputs,
            Some(request.wait_timeout),
        )
        .await
    }
    # [tool (name = "run" , output_schema = schema_for_output ::< BatchOutput < output :: RunOutput <'static >>> ())]
    async fn run(
        &self,
        Parameters(request): Parameters<RunRequest<RunInput>>,
    ) -> Result<CallToolResult, ErrorData> {
        batch::run(
            &self.settings,
            &self.backend,
            request.commands,
            Some(request.wait_timeout),
        )
        .await
    }
    #[tool(name = "fixed_run", output_schema = schema_for_output::<BatchOutput<output::RunOutput<'static>>>())]
    async fn fixed_run(
        &self,
        Parameters(request): Parameters<RunRequest<FixedCommandInput>>,
    ) -> Result<CallToolResult, ErrorData> {
        let mode = self
            .output_mode
            .ok_or_else(|| internal_error("MCP output mode is not fixed"))?;
        let entries = request
            .commands
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
    ) -> Result<CallToolResult, ErrorData> {
        batch::run(
            &self.settings,
            &self.backend,
            request.queries,
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
    super::init_diagnostics(&settings.diagnostics_filter)?;
    let service = McpServer::new(settings, privilege_mode, output_mode, Backend::ipc())?
        .serve(rmcp::transport::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
