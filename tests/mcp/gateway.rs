mod execution;
mod input_delivery;
mod inspection;
mod wire;
use rmcp::serde_json::{Value, json};
use wire::{McpSession, assert_rejected};
#[test]
fn mcp_schema_exposes_tool_inputs_and_preserves_descriptions() {
    let mut session = McpSession::new();
    let response = session.request("tools/list", json!({}));
    let tools = response
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(tools.len(), 4);
    let settings: toml::Value = toml::from_str(include_str!("../../settings.toml")).unwrap();
    for tool in tools {
        let name = tool.get("name").and_then(Value::as_str).unwrap();
        let properties = tool.pointer("/inputSchema/properties").unwrap();
        let root = properties.as_object().unwrap();
        assert_eq!(root.len(), if name == "new_tab" { 1 } else { 2 });
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
}
#[test]
fn mcp_rejects_invalid_batches_before_dispatch() {
    let mut session = McpSession::new();
    for (name, arguments) in [
        ("new_tab", json ! ({ "starting_shell" : "powershell" })),
        ("new_tab", json ! ({ "exec" : [] , "wait_timeout" : 0_u64 })),
        ("new_tab", json ! ({ "exec" : [] })),
        (
            "send_command",
            json ! ({ "exec" : [] , "wait_timeout" : 0_u64 }),
        ),
        (
            "manual_write",
            json ! ({ "exec" : [] , "wait_timeout" : 0_u64 }),
        ),
        (
            "send_command",
            json ! ({ "exec" : [{ "tab_id" : "same" , "command" : "echo one" } , { "tab_id" : "same" , "command" : "echo two" }] , "wait_timeout" : 0_u64 }),
        ),
        (
            "manual_write",
            json ! ({ "exec" : [{ "tab_id" : "same" , "text" : "one" } , { "tab_id" : "same" , "text" : "two" }] , "wait_timeout" : 0_u64 }),
        ),
    ] {
        let response = session.call(name, arguments);
        assert_rejected(&response);
        assert!(
            response
                .pointer("/result/structuredContent/results")
                .is_none(),
            "{response}"
        );
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
