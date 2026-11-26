use super::Operation as _;
use crate::mcp::{
    McpServer, WaitTimeoutSettings,
    arguments::{ManualWriteInput, NewTabOptions, RunInput, ViewQuery},
};
use crate::runtime::config::{McpSettings, ToolDescription};
use alloc::{borrow::Cow, collections::BTreeMap, sync::Arc};
use anyhow::{Context as _, Result, bail};
use rmcp::{
    handler::server::router::tool::ToolRouter,
    model::{JsonObject, Tool},
    serde_json::Value,
};
use schemars::{
    Schema,
    transform::{RecursiveTransform, RestrictFormats, Transform as _},
};
pub(super) fn apply(
    router: &mut ToolRouter<McpServer>,
    descriptions: &McpSettings,
    fixed_privilege: bool,
    fixed_output: bool,
) -> Result<()> {
    let mut new_tab = descriptions.new_tab.clone();
    if fixed_privilege {
        new_tab.item_parameters.remove("elevated");
    }
    let mut run = descriptions.run.clone();
    if fixed_output {
        run.item_parameters.remove("separate_output");
    }
    let (run_parameter, run_description) = if descriptions.single_parameter_run {
        (None, &descriptions.script_run)
    } else {
        (Some(RunInput::LIST_PARAMETER), &run)
    };
    let tools = [
        ("new_tab", Some(NewTabOptions::LIST_PARAMETER), &new_tab),
        (
            "manual_write",
            Some(ManualWriteInput::LIST_PARAMETER),
            &descriptions.manual_write,
        ),
        ("run", run_parameter, run_description),
        ("view", Some(ViewQuery::LIST_PARAMETER), &descriptions.view),
    ];
    for (name, list_parameter, description) in tools {
        let route = router
            .map
            .get_mut(name)
            .with_context(|| format!("MCP tool {name} is missing from the tool router"))?;
        apply_to_route(
            name,
            list_parameter,
            &mut route.attr,
            description,
            descriptions.wait_timeout,
        )?;
    }
    Ok(())
}
fn apply_to_route(
    name: &str,
    list_parameter: Option<&str>,
    tool: &mut Tool,
    description: &ToolDescription,
    timing: WaitTimeoutSettings,
) -> Result<()> {
    tool.description = optional_description(&description.description);
    let mut input_schema = tool.input_schema.as_ref().clone();
    let properties = input_schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .with_context(|| format!("MCP tool {name} input schema has no properties object"))?;
    apply_parameters(name, properties, &description.parameters)?;
    if let Some(timeout) = properties.get_mut("wait_timeout") {
        let schema = timeout
            .as_object_mut()
            .with_context(|| format!("MCP tool {name} wait_timeout schema is not an object"))?;
        timing.apply_schema(schema);
    }
    if let Some(parameter) = list_parameter {
        let item_properties = properties
            .get_mut(parameter)
            .and_then(|schema| schema.pointer_mut("/items/properties"))
            .and_then(Value::as_object_mut)
            .with_context(|| {
                format!("MCP tool {name} {parameter} item schema has no properties object")
            })?;
        apply_parameters(
            &format!("{name}.{parameter}"),
            item_properties,
            &description.item_parameters,
        )?;
    }
    tool.input_schema = normalize_schema(input_schema, true)?;
    tool.output_schema = tool
        .output_schema
        .as_deref()
        .cloned()
        .map(|schema| normalize_schema(schema, false))
        .transpose()?;
    Ok(())
}
fn normalize_schema(schema: JsonObject, input: bool) -> Result<Arc<JsonObject>> {
    let mut normalized = Schema::from(schema);
    RestrictFormats::default().transform(&mut normalized);
    if input {
        RecursiveTransform(require_properties).transform(&mut normalized);
    }
    let Value::Object(object) = normalized.to_value() else {
        bail!("MCP tool schema is not an object");
    };
    Ok(Arc::new(object))
}
fn require_properties(schema: &mut Schema) {
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        let required = properties
            .keys()
            .filter(|name| name.as_str() != "wait_timeout")
            .cloned()
            .map(Value::String)
            .collect();
        schema.insert("required".to_owned(), Value::Array(required));
    }
}
fn apply_parameters(
    name: &str,
    properties: &mut rmcp::serde_json::Map<String, Value>,
    descriptions: &BTreeMap<String, String>,
) -> Result<()> {
    ensure_parameter_names(name, properties, descriptions)?;
    for (parameter_name, parameter_description) in descriptions {
        let schema_value = properties
            .get_mut(parameter_name)
            .with_context(|| format!("MCP tool {name} parameter {parameter_name} is missing"))?;
        let parameter_schema = schema_value.as_object_mut().with_context(|| {
            format!("MCP tool {name} parameter {parameter_name} has an invalid schema")
        })?;
        set_description(parameter_schema, parameter_description);
    }
    Ok(())
}
fn ensure_parameter_names(
    tool_name: &str,
    properties: &rmcp::serde_json::Map<String, Value>,
    descriptions: &BTreeMap<String, String>,
) -> Result<()> {
    for parameter_name in descriptions.keys() {
        if !properties.contains_key(parameter_name) {
            bail!("MCP tool {tool_name} has an unknown parameter {parameter_name}");
        }
    }
    for parameter_name in properties.keys() {
        if !descriptions.contains_key(parameter_name) {
            bail!(
                "MCP tool {tool_name} has no description configuration for parameter {parameter_name}"
            );
        }
    }
    Ok(())
}
fn optional_description(description: &str) -> Option<Cow<'static, str>> {
    (!description.is_empty()).then(|| Cow::Owned(description.to_owned()))
}
fn set_description(parameter: &mut rmcp::serde_json::Map<String, Value>, description: &str) {
    if description.is_empty() {
        parameter.remove("description");
    } else {
        parameter.insert(
            "description".to_owned(),
            Value::String(description.to_owned()),
        );
    }
}
