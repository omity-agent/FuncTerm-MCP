use super::wire::McpSession;
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
        let name = tool.get("name").and_then(Value::as_str).unwrap();
        let properties = tool.pointer("/inputSchema/properties").unwrap();
        let root = properties.as_object().unwrap();
        let parameter_count = match name {
            "new_tab" => 1,
            "view" => 3,
            _ => 2,
        };
        assert_eq!(root.len(), parameter_count);
        assert_eq!(root.contains_key("wait_timeout"), name != "new_tab");
        assert_eq!(
            tool.pointer("/inputSchema/additionalProperties"),
            Some(&json!(false))
        );
        let configuration = settings.get("mcp").unwrap().get(name).unwrap();
        assert_eq!(
            tool.get("description").and_then(Value::as_str),
            configuration
                .get("description")
                .and_then(toml::Value::as_str)
        );
        assert_descriptions(root, configuration.get("parameters").unwrap());
        if name == "view" {
            assert!(!root.contains_key("exec"));
            assert_eq!(properties.pointer("/ids/type"), Some(&json!("array")));
            assert_eq!(properties.pointer("/ids/minItems"), Some(&json!(1_u64)));
            assert_eq!(
                properties.pointer("/ids/items/type"),
                Some(&json!("string"))
            );
            assert_eq!(
                properties
                    .pointer("/wait_timeout/minimum")
                    .and_then(Value::as_f64),
                Some(0.0_f64)
            );
            assert_eq!(
                tool.pointer("/inputSchema/required"),
                Some(&json!(["ids", "wait_timeout"]))
            );
            assert_eq!(
                properties.pointer("/start_token/type"),
                Some(&json!("integer"))
            );
            assert_eq!(
                properties.pointer("/start_token/default"),
                Some(&json!(1_u64))
            );
            assert_eq!(
                properties.pointer("/start_token/minimum"),
                Some(&json!(1_u64))
            );
        } else {
            assert_eq!(properties.pointer("/exec/type"), Some(&json!("array")));
            assert_eq!(properties.pointer("/exec/minItems"), Some(&json!(1_u64)));
            let item = properties.pointer("/exec/items").unwrap();
            assert_eq!(item.get("additionalProperties"), Some(&json!(false)));
            let parameters = item.get("properties").and_then(Value::as_object).unwrap();
            assert!(!parameters.contains_key("wait_timeout"));
            assert_descriptions(parameters, configuration.get("exec_parameters").unwrap());
        }
        assert!(tool.pointer("/outputSchema/properties/results").is_some());
    }
    drop(session);
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
