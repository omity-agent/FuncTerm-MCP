use super::fixtures::assert_parent;
use crate::support::{
    create_tab, locked_with_env, parse_command_result, parse_tab_view, run_cli, send_command,
    temp_dir,
};
#[test]
fn batch_handoffs_keep_redirection_inside_the_command() {
    let _guard = locked_with_env(&[]);
    let cwd = temp_dir("batch handoff (quoted path)");
    fs_err::create_dir(cwd.join("next directory")).unwrap();
    fs_err::write(cwd.join("supplied.txt"), "BATCH_INPUT\r\n").unwrap();
    fs_err :: write (cwd . join ("leaf.bat") , "@echo off\r\necho BATCH_STDOUT\r\necho BATCH_STDERR 1>&2\r\nset \"BATCH_PERSISTED=kept\"\r\ncd /d \"%~dp0next directory\"\r\nexit /b 7\r\n" ,) . unwrap () ;
    fs_err::write(cwd.join("bridge.cmd"), "@echo off\r\n\"%~dp0leaf.bat\"\r\n").unwrap();
    for (target, input) in [("leaf.bat", " < NUL"), ("bridge.cmd", " < supplied.txt")] {
        let created = create_tab(&cwd, "cmd");
        let command = format!("\"{target}\"{input}\r\necho UNREACHABLE_TAIL");
        let result = parse_command_result(&send_command(&created.tab_id, &command, 10.0));
        assert!(result.finished, "{command} did not finish");
        assert_eq!(result.exit_code, Some(7_i32), "stderr: {}", result.stderr);
        assert_eq!(result.stdout.trim(), "BATCH_STDOUT");
        assert_eq!(result.stderr.trim(), "BATCH_STDERR");
        assert!(
            result.cwd.replace('\\', "/").ends_with("/next directory"),
            "cwd: {}",
            result.cwd
        );
        assert_parent(&created.tab_id, "cmd");
        let persisted = parse_command_result(&send_command(
            &created.tab_id,
            "echo %BATCH_PERSISTED%",
            10.0,
        ));
        assert_eq!(persisted.stdout.trim(), "kept");
        let tab = parse_tab_view(&run_cli(&["view", &created.tab_id]));
        assert!(tab.screen.contains("BATCH_STDOUT"));
        assert!(tab.screen.contains("BATCH_STDERR"));
    }
}
#[test]
fn batch_output_redirection_does_not_capture_completion_markers() {
    let _guard = locked_with_env(&[]);
    let cwd = temp_dir("batch output capture");
    fs_err :: write (cwd . join ("redirected.cmd") , "@echo off\r\necho REDIRECTED_BATCH_OUTPUT\r\necho REDIRECTED_BATCH_ERROR 1>&2\r\nexit /b 7\r\n" ,) . unwrap () ;
    let created = create_tab(&cwd, "cmd");
    let result = parse_command_result(&send_command(
        &created.tab_id,
        "\"redirected.cmd\" < NUL > captured.txt 2> errors.txt",
        10.0,
    ));
    assert!(result.finished);
    assert_eq!(result.exit_code, Some(7_i32));
    assert!(result.stdout.trim().is_empty(), "stdout: {}", result.stdout);
    assert!(result.stderr.is_empty(), "stderr: {}", result.stderr);
    assert_eq!(
        fs_err::read_to_string(cwd.join("captured.txt"))
            .unwrap()
            .trim(),
        "REDIRECTED_BATCH_OUTPUT"
    );
    assert_eq!(
        fs_err::read_to_string(cwd.join("errors.txt"))
            .unwrap()
            .trim(),
        "REDIRECTED_BATCH_ERROR"
    );
    assert_parent(&created.tab_id, "cmd");
}
