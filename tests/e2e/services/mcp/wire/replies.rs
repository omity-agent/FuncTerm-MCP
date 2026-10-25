use rmcp::serde_json::Value;
#[track_caller]
pub(crate) fn field<'value>(value: &'value Value, path: &str) -> &'value Value {
    value
        .pointer(path)
        .unwrap_or_else(|| panic!("missing field at {path}: {value}"))
}
#[track_caller]
pub(crate) fn text<'value>(value: &'value Value, path: &str) -> &'value str {
    field(value, path)
        .as_str()
        .unwrap_or_else(|| panic!("expected string at {path}: {value}"))
}
pub(crate) fn entries(response: &Value) -> &Vec<Value> {
    field(response, "/result/structuredContent/results")
        .as_array()
        .unwrap()
}
#[track_caller]
pub(crate) fn result(response: &Value) -> &Value {
    field(entries(response).first().unwrap(), "/result")
}
#[track_caller]
pub(crate) fn command_result(response: &Value) -> &Value {
    field(result(response), "/command")
}
#[track_caller]
pub(crate) fn assert_field(value: &Value, path: &str, expected: impl Into<Value>) {
    assert_eq!(field(value, path), &expected.into(), "{path}: {value}");
}
pub(crate) fn assert_rejected(response: &Value) {
    assert!(
        response.get("error").is_some()
            || response.pointer("/result/isError") == Some(&Value::Bool(true)),
        "{response}"
    );
}
pub(crate) fn assert_saved(sent: &Value, viewed: &Value) {
    let mut expected = command_result(sent).clone();
    expected
        .as_object_mut()
        .unwrap()
        .remove("command_id")
        .unwrap();
    assert_eq!(&expected, command_result(viewed), "{viewed}");
}
pub(crate) fn text_documents(response: &Value) -> Vec<&str> {
    let blocks = field(response, "/result/content").as_array().unwrap();
    assert_eq!(blocks.len(), entries(response).len());
    blocks
        .iter()
        .zip(entries(response))
        .enumerate()
        .map(|(index, (block, entry))| {
            let document = text(block, "/text");
            assert_eq!(
                document.ends_with('\n'),
                index + 1 < blocks.len(),
                "{document:?}"
            );
            assert!(!document.starts_with('\n'), "{document:?}");
            if let Some(error) = entry.get("error").and_then(Value::as_str) {
                assert_eq!(document.trim_end_matches('\n'), error);
            }
            document
        })
        .collect()
}
