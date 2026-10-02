use super::{
    execution::create_tabs,
    wire::{McpSession, entries},
};
use rmcp::serde_json::{Value, json};
#[test]
fn mcp_manual_write_delivers_text_and_bytes_to_distinct_tabs() {
    let mut session = McpSession::new();
    let [first, second] = create_tabs(&mut session);
    let idle = session . call ("manual_write" , json ! ({ "exec" : [{ "tab_id" : first , "text" : "UNEXPECTED" }] , "wait_timeout" : 0_u64 }) ,) ;
    assert!(
        entries(&idle)
            .first()
            .unwrap()
            .get("error")
            .and_then(Value::as_str)
            .unwrap()
            .contains("prompt is idle"),
        "{idle}"
    );
    #[cfg(windows)]
    let command = "$line = [Console]::In.ReadLine()\nWrite-Output \"RECEIVED_$line\"";
    #[cfg(not(windows))]
    let command = "read -r line\nprintf 'RECEIVED_%s\\n' \"$line\"";
    let pending = session . call ("send_command" , json ! ({ "exec" : [{ "tab_id" : first , "command" : command } , { "tab_id" : second , "command" : command }] , "wait_timeout" : 0_u64 })) ;
    let busy = session . call ("send_command" , json ! ({ "exec" : [{ "tab_id" : first , "command" : "echo UNEXPECTED" }] , "wait_timeout" : 0_u64 }) ,) ;
    assert!(
        entries(&busy).first().unwrap().get("error").is_some(),
        "{busy}"
    );
    let ids: Vec<_> = entries(&pending)
        .iter()
        .map(|entry| {
            entry
                .pointer("/result/command/command_id")
                .and_then(Value::as_str)
                .unwrap()
        })
        .collect();
    let written = session . call ("manual_write" , json ! ({ "exec" : [{ "tab_id" : first , "text" : "FIRST\r\n" } , { "tab_id" : second , "bytes" : b"SECOND\r\n" . to_vec () }] , "wait_timeout" : 2_u64 })) ;
    assert_eq!(entries(&written).len(), 2);
    assert!(
        entries(&written)
            .iter()
            .all(|entry| entry.pointer("/result/screen").is_some()),
        "{written}"
    );
    let completed = session.call("view", json ! ({ "ids" : ids , "wait_timeout" : 5_u64 }));
    for (entry, expected) in entries(&completed)
        .iter()
        .zip(["RECEIVED_FIRST", "RECEIVED_SECOND"])
    {
        assert_eq!(
            entry.pointer("/result/command/finished"),
            Some(&json!(true))
        );
        assert_eq!(
            entry.pointer("/result/command/exit_code"),
            Some(&json!(0_i32))
        );
        assert!(
            entry
                .pointer("/result/command/stdout")
                .and_then(Value::as_str)
                .unwrap()
                .contains(expected),
            "{completed}"
        );
    }
    drop(session);
}
