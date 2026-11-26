mod arguments;
mod batch;
mod collection;
mod invocation;
mod operations;
mod output;
mod streamable;
use crate::engine::privileges::PrivilegeMode;
use crate::runtime::config::Settings;
use crate::runtime::protocol::OutputMode;
use crate::runtime::protocol::time_consumption;
use anyhow::Result;
pub(crate) use batch::{WaitTimeoutMultiplier, WaitTimeoutSettings};
use invocation::Backend;
use output::internal_error;
use rmcp::{
    ErrorData, ServerHandler, ServiceExt as _,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext},
    model::{
        CacheScope, CallToolRequestParams, CallToolResponse, ListToolsResult,
        PaginatedRequestParams, ProtocolVersion, ResultType,
    },
    service::RequestContext,
    tool_handler,
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
impl McpServer {
    fn new(
        settings: Settings,
        privilege_mode: Option<PrivilegeMode>,
        output_mode: Option<OutputMode>,
        backend: Backend,
    ) -> Result<Self> {
        let tool_router = operations::router(&settings.mcp, privilege_mode, output_mode)?;
        Ok(Self {
            settings,
            backend,
            privilege_mode,
            output_mode,
            tool_router,
        })
    }
}
pub(crate) async fn run(
    settings: Settings,
    privilege_mode: Option<PrivilegeMode>,
    output_mode: Option<OutputMode>,
) -> Result<()> {
    super::init_diagnostics(&settings.diagnostics_filter)?;
    let service = McpServer::new(settings, privilege_mode, output_mode, Backend::Ipc)?
        .serve(rmcp::transport::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
