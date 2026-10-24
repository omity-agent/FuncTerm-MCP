extern crate alloc;
use super::wire::{McpSession, assert_field, command_result, entries, text};
use crate::{
    permission_policy::{QUERY, SHELL, daemon_privileged, privileged_output},
    support::locked_with_env,
};
use alloc::rc::Rc;
use rmcp::serde_json::{Value, json};
#[test]
fn mcp_fixed_permissions_are_used_for_new_tabs() {
    let daemon = Rc::new(locked_with_env(&[]));
    for (mode, elevated) in [("normal", false), ("elevated", true)] {
        let mut session = McpSession::on(&daemon, &["--no-profile", "--privilege-mode", mode]);
        let creation = session.call("new_tab", json!({"exec": [{"starting_shell": SHELL}]}));
        assert_created_permission(&mut session, entries(&creation).first().unwrap(), elevated);
    }
}
#[test]
fn mcp_new_tab_selects_permissions_per_entry() {
    let mut session = McpSession::new();
    let creation = session.call(
        "new_tab",
        json!({"exec": [
            {"starting_shell": SHELL},
            {"starting_shell": SHELL, "elevated": false},
            {"starting_shell": SHELL, "elevated": true}
        ]}),
    );
    assert_eq!(entries(&creation).len(), 3);
    for (index, entry) in entries(&creation).iter().enumerate() {
        assert_created_permission(&mut session, entry, index == 2);
    }
}
fn assert_created_permission(session: &mut McpSession, entry: &Value, elevated: bool) {
    if elevated && !daemon_privileged() {
        assert!(
            text(entry, "/error").contains("requires the Daemon"),
            "{entry}"
        );
        return;
    }
    let tab = text(entry, "/result/tab_id");
    let response = session.execute(tab, QUERY, true, 10.0);
    let command = command_result(&response);
    assert_field(command, "/exit_code", 0_i32);
    assert_eq!(
        privileged_output(text(command, "/stdout")),
        elevated,
        "{response}"
    );
}
