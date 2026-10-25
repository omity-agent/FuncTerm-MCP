use super::{
    execution::create_tabs,
    wire::{McpSession, assert_field, entries, text},
};
use rmcp::serde_json::json;
#[test]
fn mcp_manual_write_delivers_text_and_bytes_to_distinct_tabs() {
    let mut session = McpSession::new();
    let [first, second] = create_tabs(&mut session);
    let idle = session.call(
        "manual_write",
        json!({
            "exec": [{"tab_id": first, "text": "UNEXPECTED"}], "wait_timeout": 0_u64
        }),
    );
    assert!(
        text(entries(&idle).first().unwrap(), "/error").contains("prompt is idle"),
        "{idle}"
    );
    #[cfg(windows)]
    let source = "$line = [Console]::In.ReadLine()\nWrite-Output \"RECEIVED_$line\"";
    #[cfg(not(windows))]
    let source = "read -r line\nprintf 'RECEIVED_%s\\n' \"$line\"";
    let pending = session.call(
        "send_command",
        json!({
            "exec": [
                {"tab_id": first, "command": source, "separate_output": true},
                {"tab_id": second, "command": source, "separate_output": true}
            ], "wait_timeout": 0_u64
        }),
    );
    let busy = session.execute(&first, "echo UNEXPECTED", true, 0.0);
    assert!(
        entries(&busy).first().unwrap().get("error").is_some(),
        "{busy}"
    );
    let ids = entries(&pending)
        .iter()
        .map(|entry| text(entry, "/result/command/command_id"))
        .collect::<Vec<_>>();
    let written = session.call("manual_write", json!({
        "exec": [{"tab_id": first, "text": "FIRST\r\n", "bytes": null}, {"tab_id": second, "text": null, "bytes": b"SECOND\r\n".to_vec()}], "wait_timeout": 2_u64
    }));
    assert_eq!(entries(&written).len(), 2);
    super::wire::text_documents(&written);
    assert!(
        entries(&written)
            .iter()
            .all(|entry| entry.pointer("/result/screen").is_some()),
        "{written}"
    );
    let completed = session.view(&ids, 1, 5.0);
    for (entry, expected) in entries(&completed)
        .iter()
        .zip(["RECEIVED_FIRST", "RECEIVED_SECOND"])
    {
        assert_field(entry, "/result/command/finished", true);
        assert_field(entry, "/result/command/exit_code", 0_i32);
        assert!(
            text(entry, "/result/command/stdout").contains(expected),
            "{completed}"
        );
    }
}
