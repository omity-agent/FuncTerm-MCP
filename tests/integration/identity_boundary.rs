use crate::permission_policy::{QUERY, privileged_output};
use crate::support::{create_tab, locked_with_env, parse_command_result, send_command, temp_root};
use functerm::shell::quote;
use named_lock::NamedLock;
#[test]
fn ordinary_client_controls_a_daemon_and_mutex_created_by_the_test_process() {
    let target = locked_with_env(&[]);
    let environment = target.env();
    let service = &environment
        .iter()
        .find(|pair| pair.0 == "FUNCTERM_DAEMON_SERVICE_NAME")
        .unwrap()
        .1;
    let name = format!(
        "functerm-instance-{}",
        blake3::hash(service.as_bytes()).to_hex()
    );
    // Keep the original mutex alive after shutdown so its security policy is exercised.
    let instance = NamedLock::create(&name).unwrap();
    let _controller = locked_with_env(&[]);
    let tab = create_tab(&temp_root(), "powershell");
    let identity = parse_command_result(&send_command(&tab.tab_id, QUERY, 10.0));
    assert_eq!(identity.exit_code, Some(0_i32), "{identity:?}");
    assert!(!privileged_output(&identity.stdout), "{identity:?}");
    let executable =
        quote::powershell_path(std::path::Path::new(env!("CARGO_BIN_EXE_functerm"))).unwrap();
    for (operation, message) in [
        ("start", "daemon is already running"),
        ("stop", "daemon stopped"),
        ("start", "daemon started"),
        ("restart", "daemon restarted"),
        ("stop", "daemon stopped"),
    ] {
        let command = format!(
            "$env:FUNCTERM_DAEMON_SERVICE_NAME = {}\n& {executable} daemon {operation}",
            quote::powershell_string(service)
        );
        let result = parse_command_result(&send_command(&tab.tab_id, &command, 10.0));
        assert_eq!(result.exit_code, Some(0_i32), "{operation}: {result:?}");
        assert!(result.stdout.contains(message), "{operation}: {result:?}");
    }
    drop(instance);
}
