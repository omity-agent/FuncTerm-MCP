use super::{schema, script};
use crate::engine::privileges::PrivilegeMode;
use crate::mcp::{
    McpServer,
    arguments::{
        FixedCommandInput, FixedNewTabOptions, ManualWriteRequest, NewTabOptions, NewTabRequest,
        RunInput, RunRequest, ScriptRequest, ViewRequest,
    },
    batch,
    collection::BatchOutput,
    output::{self, internal_error},
};
use crate::runtime::{config::McpSettings, protocol::OutputMode};
use anyhow::{Context as _, Result};
use core::time::Duration;
use rmcp::{
    ErrorData,
    handler::server::{router::tool::ToolRouter, tool::schema_for_output, wrapper::Parameters},
    model::CallToolResult,
    tool, tool_router,
};
pub(in crate::engine::mcp) fn router(
    settings: &McpSettings,
    privilege_mode: Option<PrivilegeMode>,
    output_mode: Option<OutputMode>,
) -> Result<ToolRouter<McpServer>> {
    let mut router = McpServer::tool_router();
    for (internal, public, selected) in [
        ("fixed_new_tab", "new_tab", privilege_mode.is_some()),
        (
            "fixed_run",
            "run",
            output_mode.is_some() && !settings.single_parameter_run,
        ),
        ("script_run", "run", settings.single_parameter_run),
    ] {
        let mut route = router
            .map
            .remove(internal)
            .with_context(|| format!("MCP route {internal} is missing"))?;
        if selected {
            route.attr.name = public.to_owned().into();
            router.map.insert(public.to_owned().into(), route);
        }
    }
    schema::apply(
        &mut router,
        settings,
        privilege_mode.is_some(),
        output_mode.is_some(),
    )?;
    Ok(router)
}
#[tool_router(router = tool_router)]
impl McpServer {
    #[tool(name = "new_tab", output_schema = schema_for_output::<BatchOutput<output::NewTabOutput<'static>>>())]
    async fn new_tab(
        &self,
        Parameters(request): Parameters<NewTabRequest<NewTabOptions>>,
    ) -> Result<CallToolResult, ErrorData> {
        batch::run(&self.settings, &self.backend, request.tabs, Duration::ZERO).await
    }
    #[tool(name = "fixed_new_tab", output_schema = schema_for_output::<BatchOutput<output::NewTabOutput<'static>>>())]
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
        batch::run(&self.settings, &self.backend, entries, Duration::ZERO).await
    }
    #[tool(name = "manual_write", output_schema = schema_for_output::<BatchOutput<output::ManualWriteOutput<'static>>>())]
    async fn manual_write(
        &self,
        Parameters(request): Parameters<ManualWriteRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        batch::run_timed(
            &self.settings,
            &self.backend,
            request.inputs,
            request.wait_timeout,
        )
        .await
    }
    #[tool(name = "run", output_schema = schema_for_output::<BatchOutput<output::RunOutput<'static>>>())]
    async fn run(
        &self,
        Parameters(request): Parameters<RunRequest<RunInput>>,
    ) -> Result<CallToolResult, ErrorData> {
        batch::run_timed(
            &self.settings,
            &self.backend,
            request.commands,
            request.wait_timeout,
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
        batch::run_timed(&self.settings, &self.backend, entries, request.wait_timeout).await
    }
    #[tool(name = "script_run", output_schema = schema_for_output::<BatchOutput<output::RunOutput<'static>>>())]
    async fn script_run(
        &self,
        Parameters(request): Parameters<ScriptRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        let separate_output = self.output_mode.map(OutputMode::separate_output);
        match script::parse(&request.script, separate_output) {
            Ok(parsed) => {
                batch::run_timed(
                    &self.settings,
                    &self.backend,
                    parsed.commands,
                    parsed.timeouts,
                )
                .await
            }
            Err(error) => Ok(batch::input_error(format!("{error:#}"))),
        }
    }
    #[tool(name = "view", output_schema = schema_for_output::<BatchOutput<output::ViewOutput<'static>>>())]
    async fn view(
        &self,
        Parameters(request): Parameters<ViewRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        batch::run_timed(
            &self.settings,
            &self.backend,
            request.queries,
            request.wait_timeout,
        )
        .await
    }
}
