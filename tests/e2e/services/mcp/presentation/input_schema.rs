use super::wire::{McpSession, assert_field, field, text};
use rmcp::serde_json::{Value, json};
#[test]
fn mcp_schema_exposes_tool_inputs_and_preserves_descriptions() {
    let mut session = McpSession::new();
    let response = session.request("tools/list", json!({}));
    let tools = response
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(tools.len(), 4);
    let settings: toml::Value = toml::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/settings.toml"
    )))
    .unwrap();
    for tool in tools {
        let name = text(tool, "/name");
        let properties = field(tool, "/inputSchema/properties");
        let root = properties.as_object().unwrap();
        assert_eq!(root.contains_key("wait_timeout"), name != "new_tab");
        assert_field(tool, "/inputSchema/additionalProperties", false);
        let configuration = settings.get("mcp").unwrap().get(name).unwrap();
        assert_eq!(
            text(tool, "/description"),
            configuration
                .get("description")
                .and_then(toml::Value::as_str)
                .unwrap()
        );
        assert_descriptions(root, configuration.get("parameters").unwrap());
        if name != "view" {
            let parameters = field(properties, "/exec/items/properties")
                .as_object()
                .unwrap();
            assert_descriptions(parameters, configuration.get("exec_parameters").unwrap());
        }
        assert!(tool.pointer("/outputSchema/properties/results").is_some());
    }
}
fn assert_descriptions(
    parameters: &rmcp::serde_json::Map<String, Value>,
    configuration: &toml::Value,
) {
    for (parameter, schema) in parameters {
        let description = configuration
            .get(parameter)
            .and_then(toml::Value::as_str)
            .unwrap();
        assert_eq!(
            schema.get("description").and_then(Value::as_str),
            (!description.is_empty()).then_some(description)
        );
    }
}
