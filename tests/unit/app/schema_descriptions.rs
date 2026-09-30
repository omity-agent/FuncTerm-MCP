use super::{apply, optional_description};
use crate::runtime::config::{McpSettings, ToolDescription};
#[test]
fn empty_description_is_omitted() {
    assert!(optional_description("").is_none());
}
#[test]
fn configured_descriptions_are_applied_to_tools_and_parameters() {
    let mut router = crate::mcp::McpServer::tool_router();
    let descriptions = McpSettings {
        new_tab: ToolDescription {
            description: "tool".to_owned(),
            parameters: [
                ("starting_directory".to_owned(), "directory".to_owned()),
                ("starting_shell".to_owned(), String::new()),
            ]
            .into_iter()
            .collect(),
        },
        manual_write: ToolDescription {
            description: String::new(),
            parameters: [
                ("tab_id".to_owned(), String::new()),
                ("text".to_owned(), String::new()),
                ("bytes".to_owned(), String::new()),
                ("wait_timeout".to_owned(), String::new()),
            ]
            .into_iter()
            .collect(),
        },
        send_command: ToolDescription {
            description: String::new(),
            parameters: [
                ("tab_id".to_owned(), String::new()),
                ("command".to_owned(), String::new()),
                ("wait_timeout".to_owned(), String::new()),
            ]
            .into_iter()
            .collect(),
        },
        view: ToolDescription {
            description: String::new(),
            parameters: [
                ("id".to_owned(), String::new()),
                ("wait_timeout".to_owned(), String::new()),
            ]
            .into_iter()
            .collect(),
        },
    };
    if let Err(error) = apply(&mut router, &descriptions) {
        panic!("tool descriptions should be valid: {error:#}");
    }
    let new_tab = router.map.get("new_tab").map_or_else(
        || panic!("new_tab tool should be registered"),
        |route| &route.attr,
    );
    assert_eq!(new_tab.description.as_deref(), Some("tool"));
    let properties = new_tab
        .input_schema
        .get("properties")
        .and_then(rmcp::serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("new_tab input schema should have properties"));
    let starting_directory = properties
        .get("starting_directory")
        .and_then(rmcp::serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("starting_directory schema should be an object"));
    assert_eq!(
        starting_directory.get("description"),
        Some(&rmcp::serde_json::Value::String("directory".to_owned()))
    );
    assert!(
        properties
            .get("starting_shell")
            .and_then(rmcp::serde_json::Value::as_object)
            .and_then(|schema| schema.get("description"))
            .is_none()
    );
}
