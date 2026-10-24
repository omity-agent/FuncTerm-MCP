#[cfg(windows)]
use super::matrix::{case_dir, required_executable, shell_cases};
use super::matrix::{plain_title_command, set_title_command};
#[cfg(windows)]
use crate::support::{create_tab, locked_with_env};
use crate::support::{
    parse_command_id, parse_command_result, parse_tab_view, run_cli, send_command,
};
#[cfg(windows)]
#[test]
fn cli_distinguishes_native_host_title_from_command_title() {
    let case = shell_cases()
        .iter()
        .find(|case| case.name == "powershell")
        .unwrap();
    let executable = required_executable(case);
    let _guard = locked_with_env(&[(case.env_var, &executable)]);
    let tab = create_tab(&case_dir(case.name, "native command title"), case.name);
    let model_title = parse_tab_view(&run_cli(&["view", &tab.tab_id])).title;
    let native = "Write-Output ('HOST_TITLE_BEFORE=<' + [Console]::Title + '>'); where.exe where.exe; Write-Output ('HOST_TITLE_AFTER=<' + [Console]::Title + '>')";
    let output = send_command(&tab.tab_id, native, 10.0);
    let first_id = parse_command_id(&output);
    assert_native_title(&parse_command_result(&output), &model_title);
    let intended = "MCP_PTY_自定义标题_😀";
    let command = format!(
        "{}; where.exe where.exe",
        set_title_command(case.name, intended)
    );
    let titled = parse_command_result(&send_command(&tab.tab_id, &command, 10.0));
    assert_finished(&titled, "MCP_PTY_TITLE_SET");
    assert_eq!(titled.title, intended);
    let reset = parse_command_result(&send_command(&tab.tab_id, native, 10.0));
    assert_native_title(&reset, &model_title);
    let historical = parse_command_result(&run_cli(&["view", &first_id]));
    assert_eq!(historical.title, model_title);
    assert_eq!(
        parse_tab_view(&run_cli(&["view", &tab.tab_id])).title,
        model_title
    );
}
#[cfg(windows)]
fn assert_native_title(result: &crate::support::CommandResult, model: &str) {
    assert_finished(result, "where.exe");
    assert_eq!(result.title, model);
    for phase in ["BEFORE", "AFTER"] {
        assert!(
            result
                .stdout
                .contains(&format!("HOST_TITLE_{phase}=<{model}>")),
            "{result:?}"
        );
    }
}
pub(super) fn assert_command_titles(tab: &str, shell: &str) {
    let model_title = parse_tab_view(&run_cli(&["view", tab])).title;
    let output = send_command(tab, plain_title_command(shell), 10.0);
    let id = parse_command_id(&output);
    let first = parse_command_result(&output);
    assert_finished(&first, "MCP_PTY_PLAIN_TITLE");
    assert_eq!(first.title, model_title);
    let expected = format!("MCP_PTY_{}_COMMAND_TITLE", shell.to_ascii_uppercase());
    let titled = parse_command_result(&send_command(
        tab,
        &set_title_command(shell, &expected),
        10.0,
    ));
    assert_finished(&titled, "MCP_PTY_TITLE_SET");
    if shell == "cmd" {
        assert!(titled.title.contains(&expected), "{titled:?}");
    } else {
        assert_eq!(titled.title, expected);
    }
    assert_eq!(parse_tab_view(&run_cli(&["view", tab])).title, model_title);
    assert_eq!(
        parse_command_result(&run_cli(&["view", &id])).title,
        model_title
    );
    let last = parse_command_result(&send_command(tab, plain_title_command(shell), 10.0));
    assert_finished(&last, "MCP_PTY_PLAIN_TITLE");
    assert_eq!(last.title, model_title);
}
fn assert_finished(result: &crate::support::CommandResult, marker: &str) {
    assert!(result.finished, "{result:?}");
    assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
    assert!(result.stdout.contains(marker), "{result:?}");
}
