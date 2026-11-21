use super::{
    execution::SHELL,
    http_peer::HttpSession,
    wire::{result, text, text_documents},
};
use crate::support::{create_tab, locked_with_env, run_cli, tagged_content};
use core::time::Duration;
use rmcp::serde_json::json;
use std::time::Instant;
#[test]
fn resource_alerts_are_optional_and_preserve_original_notes() {
    #[cfg(windows)]
    let silent_command = "$null";
    #[cfg(not(windows))]
    let silent_command = ":";
    for enabled in [false, true] {
        let daemon = locked_with_env(&[
            (
                "FUNCTERM_RESOURCE_ALERTS",
                if enabled { "true" } else { "false" },
            ),
            ("FUNCTERM_RESOURCE_CPU_PERCENT", "0"),
            ("FUNCTERM_RESOURCE_RAM_PERCENT", "0"),
        ]);
        let tab = create_tab(&std::env::current_dir().unwrap(), SHELL).tab_id;
        let started = Instant::now();
        loop {
            let viewed = run_cli(&[
                "view",
                &tab,
                "--wait-timeout",
                if enabled { "0.2" } else { "2" },
            ]);
            assert!(viewed.status.success(), "{viewed:?}");
            let output = String::from_utf8(viewed.stdout).unwrap();
            let note = tagged_content(&output, "NOTE").unwrap();
            if !enabled {
                assert!(note.trim().is_empty(), "{note}");
                break;
            }
            if note.contains("Host resource warning: CPU usage")
                && note.contains("Host resource warning: RAM usage")
            {
                break;
            }
            assert!(started.elapsed() < Duration::from_secs(10), "{output}");
        }
        let mut http = HttpSession::new(&daemon, "2025-11-25");
        let response = http.call(
            "run",
            json!({
                "commands": [{"tab_id": tab, "command": silent_command, "separate_output": true}],
                "wait_timeout": 10_u64,
            }),
        );
        let note = text(result(&response), "/note");
        assert_eq!(
            note.lines().next(),
            Some("No stdout or stderr content was captured.")
        );
        for resource in ["CPU", "RAM"] {
            let warning = format!("Host resource warning: {resource} usage");
            let alert = note.lines().find(|line| line.starts_with(&warning));
            assert_eq!(alert.is_some(), enabled, "{response}");
            if let Some(line) = alert {
                assert!(line.ends_with("(threshold 0%)."), "{line}");
            }
        }
        assert!(text_documents(&response).first().unwrap().contains(note));
        let closed = run_cli(&["close", "--tab-id", &tab]);
        assert!(closed.status.success(), "{closed:?}");
    }
}
