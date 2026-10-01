use super::McpServer;
use crate::runtime::config::{McpSettings, ToolDescription};
use alloc::{borrow::Cow, collections::BTreeMap, sync::Arc};
use anyhow::{Context as _, Result, bail};
use rmcp::{handler::server::router::tool::ToolRouter, model::Tool, serde_json::Value};
pub(super) fn apply(router: &mut ToolRouter<McpServer>, descriptions: &McpSettings) -> Result<()> {
    let tools = [
        ("new_tab", &descriptions.new_tab),
        ("manual_write", &descriptions.manual_write),
        ("send_command", &descriptions.send_command),
        ("view", &descriptions.view),
    ];
    for (name, description) in tools {
        let route = router
            .map
            .get_mut(name)
            .with_context(|| format!("MCP tool {name} is missing from the tool router"))?;
        apply_to_route(name, &mut route.attr, description)?;
    }
    Ok(())
}
fn apply_to_route(name: &str, tool: &mut Tool, description: &ToolDescription) -> Result<()> {
    tool.description = optional_description(&description.description);
    let mut input_schema = tool.input_schema.as_ref().clone();
    let properties = input_schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .with_context(|| format!("MCP tool {name} input schema has no properties object"))?;
    apply_parameters(name, properties, &description.parameters)?;
    if let Some(exec_schema) = properties.get_mut("exec") {
        let exec_properties = exec_schema
            .pointer_mut("/items/properties")
            .and_then(Value::as_object_mut)
            .with_context(|| {
                format!("MCP tool {name} exec item schema has no properties object")
            })?;
        apply_parameters(
            &format!("{name}.exec"),
            exec_properties,
            &description.exec_parameters,
        )?;
    } else if !description.exec_parameters.is_empty() {
        bail!("MCP tool {name} has exec parameter descriptions but no exec parameter");
    }
    tool.input_schema = Arc::new(input_schema);
    Ok(())
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
