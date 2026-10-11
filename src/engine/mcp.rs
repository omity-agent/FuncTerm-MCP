mod arguments;
mod batch;
mod collection;
mod descriptions;
mod operations;
mod output;
use crate::engine::privileges::PrivilegeMode;
use crate::runtime::config::Settings;
use anyhow::{Context as _, Result};
use arguments::{FixedNewTabExec, ManualWriteExec, NewTabExec, SendCommandExec, ViewRequest};
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
    settings: Settings,
    privilege_mode: Option<PrivilegeMode>,
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
    fn new(settings: Settings, privilege_mode: Option<PrivilegeMode>) -> Result<Self> {
        let mut tool_router = Self::tool_router();
        let mut fixed_route = tool_router
            .map
            .remove("fixed_new_tab")
            .context("fixed New Tab route is missing")?;
        if privilege_mode.is_some() {
            fixed_route.attr.name = "new_tab".into();
            tool_router.map.insert("new_tab".into(), fixed_route);
        }
        descriptions::apply(&mut tool_router, &settings.mcp, privilege_mode.is_some())?;
        Ok(Self {
            settings,
            privilege_mode,
            tool_router,
        })
    }
    # [tool (name = "new_tab" , output_schema = schema_for_output ::< BatchOutput < output :: NewTabOutput <'static >>> ())]
    async fn new_tab(
        &self,
        Parameters(request): Parameters<ExecBatch<NewTabExec>>,
    ) -> Result<CallToolResult, String> {
        batch::run(&self.settings, request.exec, None).await
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
        batch::run(&self.settings, entries, None).await
    }
    # [tool (name = "manual_write" , output_schema = schema_for_output ::< BatchOutput < output :: ManualWriteOutput <'static >>> ())]
    async fn manual_write(
        &self,
        Parameters(request): Parameters<TimedBatch<ManualWriteExec>>,
    ) -> Result<CallToolResult, String> {
        batch::run(&self.settings, request.exec, Some(request.wait_timeout)).await
    }
    # [tool (name = "send_command" , output_schema = schema_for_output ::< BatchOutput < output :: SendCommandOutput <'static >>> ())]
    async fn send_command(
        &self,
        Parameters(request): Parameters<TimedBatch<SendCommandExec>>,
    ) -> Result<CallToolResult, String> {
        batch::run(&self.settings, request.exec, Some(request.wait_timeout)).await
    }
    # [tool (name = "view" , output_schema = schema_for_output ::< BatchOutput < output :: ViewOutput <'static >>> ())]
    async fn view(
        &self,
        Parameters(request): Parameters<ViewRequest>,
    ) -> Result<CallToolResult, String> {
        batch::run(
            &self.settings,
            request.ids.into_iter().map(operations::LookupId).collect(),
            Some(request.wait_timeout),
        )
        .await
    }
}
pub(crate) async fn run(settings: Settings, privilege_mode: Option<PrivilegeMode>) -> Result<()> {
    let service = McpServer::new(settings, privilege_mode)?
        .serve(rmcp::transport::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
fn error_text(error: impl core::fmt::Display) -> String {
    error.to_string()
}
