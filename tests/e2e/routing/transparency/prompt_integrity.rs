use super::{checked_text, run_terminal, terminal_output};
use crate::support::{
    assert_powershell_primary_prompt, create_tab, locked_with_env, parse_tab_view, run_cli,
    temp_dir,
};
#[test]
fn powershell_host_reports_do_not_become_prompt_input() {
    verify_clean_prompt(
        "powershell",
        "1..45 | ForEach-Object { Write-Output \"CURSOR_ROW_$_\" }",
        "Write-Output 'CURSOR_FOLLOW_UP'",
    );
}
#[test]
fn cmd_host_reports_do_not_become_prompt_input() {
    verify_clean_prompt(
        "cmd",
        &"echo CURSOR_ROW\r\n".repeat(45),
        "echo CURSOR_FOLLOW_UP",
    );
}
fn verify_clean_prompt(shell: &str, initial: &str, follow_up: &str) {
    let _guard = locked_with_env(&[]);
    let directory = temp_dir("host-query-roundtrip");
    let tab = create_tab(&directory, shell);
    let first = checked_text(&run_terminal(&tab.tab_id, initial, "10"));
    assert!(first.contains("<EXIT_CODE>\n0\n"), "{shell}: {first}");
    assert!(terminal_output(&first).contains("CURSOR_ROW"), "{first}");
    for _ in 0_u8..3 {
        let result = checked_text(&run_terminal(&tab.tab_id, follow_up, "10"));
        assert!(result.contains("<EXIT_CODE>\n0\n"), "{shell}: {result}");
        assert_eq!(
            terminal_output(&result).trim(),
            "CURSOR_FOLLOW_UP",
            "{shell}: {result}"
        );
    }
    let response = run_cli(&["view", &tab.tab_id]);
    let view = parse_tab_view(&response);
    assert!(view.alive, "{shell}: {}", view.screen);
    assert!(checked_text(&response).contains("<IDLE>\ntrue\n"));
    if shell == "powershell" {
        assert_powershell_primary_prompt(&view);
    } else {
        assert_eq!(
            view.screen.lines().next_back(),
            Some(">"),
            "{}",
            view.screen
        );
    }
}
