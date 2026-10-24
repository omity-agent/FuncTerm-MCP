extern crate alloc;
use super::{
    execution::create_tabs,
    wire::{McpSession, assert_field, command_result, text},
};
use crate::support::locked_with_env;
use alloc::rc::Rc;
#[test]
fn mcp_no_profile_applies_to_all_created_tabs() {
    let daemon = Rc::new(locked_with_env(&[]));
    #[cfg(windows)]
    let command = "Write-Output $env:FUNCTERM_LOAD_PROFILE";
    #[cfg(unix)]
    let command = "printf '%s\\n' \"$FUNCTERM_LOAD_PROFILE\"";
    for (arguments, expected) in [(&[][..], "1"), (&["--no-profile"][..], "0")] {
        let mut session = McpSession::on(&daemon, arguments);
        for tab in create_tabs::<2>(&mut session) {
            let response = session.execute(&tab, command, true, 10.0);
            let result = command_result(&response);
            assert_field(result, "/exit_code", 0_i32);
            assert_eq!(text(result, "/stdout").trim(), expected, "{response}");
        }
    }
}
