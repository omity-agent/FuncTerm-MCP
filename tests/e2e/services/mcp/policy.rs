use super::wire::{McpSession, entries};
use rmcp::serde_json::{Value, json};
#[test]
fn mcp_no_profile_applies_to_all_created_tabs() {
    #[cfg(windows)]
    let (shell, command) = ("powershell", "Write-Output $env:FUNCTERM_LOAD_PROFILE");
    #[cfg(unix)]
    let (shell, command) = ("bash", "printf '%s\\n' \"$FUNCTERM_LOAD_PROFILE\"");
    for (arguments, expected) in [(&[][..], "1"), (&["--no-profile"][..], "0")] {
        let mut session = McpSession::with_arguments(arguments);
        let creation = session.call(
            "new_tab",
            json ! ({ "exec" : [{ "starting_shell" : shell } , { "starting_shell" : shell }] }),
        );
        for entry in entries(&creation) {
            let tab_id = entry
                .pointer("/result/tab_id")
                .and_then(Value::as_str)
                .unwrap();
            let response = session . call ("send_command" , json ! ({ "exec" : [{ "tab_id" : tab_id , "command" : command }] , "wait_timeout" : 10_u64 }) ,) ;
            let result = entries(&response).first().unwrap();
            assert_eq!(
                result.pointer("/result/command/exit_code"),
                Some(&json!(0_i32)),
                "{response}"
            );
            assert_eq!(
                result
                    .pointer("/result/command/stdout")
                    .and_then(Value::as_str)
                    .unwrap()
                    .trim(),
                expected,
                "{response}"
            );
        }
        drop(session);
    }
}
