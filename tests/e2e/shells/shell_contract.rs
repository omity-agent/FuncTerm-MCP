use super::matrix::{assert_cwd, assert_shell_query, case_command, case_dir, shell_cases};
use crate::support::{complete_command, create_tab, parse_tab_view, run_cli};
use std::fs;
#[test]
fn cli_runs_commands_for_every_supported_shell() {
    let _guard = super::matrix::daemon();
    for case in shell_cases() {
        let start = case_dir(case.name, "start dir");
        let next = case_dir(case.name, "next dir");
        let created = create_tab(&start, case.name);
        let shell_before = parse_tab_view(&run_cli(&["view", &created.tab_id]));
        assert_shell_query(&shell_before, &start, case.name);
        let command = case_command(case.name, &next);
        let result = complete_command(&created.tab_id, &command);
        assert!(result.stdout.contains("MCP_PTY_STDOUT"), "{result:?}");
        assert!(
            if case.name == "nu" {
                result.stdout.contains("MCP_PTY_STDERR")
            } else {
                result.stderr.contains("MCP_PTY_STDERR")
            },
            "{result:?}"
        );
        assert_eq!(
            result.exit_code,
            Some(case.expected_exit_code),
            "{result:?}"
        );
        assert_ne!(result.time_consumption, "0ns");
        assert_cwd(&result.cwd, &next, case.name);
        let shell_after = parse_tab_view(&run_cli(&["view", &created.tab_id]));
        assert_shell_query(&shell_after, &next, case.name);
        super::state::assert_persistent_state(&created.tab_id, case.name);
        if case.name == "nu" {
            let marker = "MCP_PTY_NUSHELL_IMPLICIT.txt";
            fs::write(next.join(marker), "implicit output marker").unwrap();
            let implicit =
                complete_command(&created.tab_id, &format!("ls | where name == {marker:?}"));
            assert_eq!(implicit.exit_code, Some(0_i32), "{implicit:?}");
            assert!(implicit.stdout.contains(marker), "{implicit:?}");
        }
    }
}
