use super::fixtures::{assert_parent, pipe_command, start_runtime};
use crate::support::{create_tab, parse_command_result, send_command};
#[test]
fn file_and_null_input_never_activate_a_nested_repl() {
    let (_guard, cwd, _parent) = start_runtime("redirected devices");
    #[cfg(windows)]
    let (parent, null, native) = ("cmd", "NUL", "\"%FUNCTERM_REAL_PYTHON%\"");
    #[cfg(not(windows))]
    let (parent, null, native) = ("bash", "/dev/null", "\"$FUNCTERM_REAL_PYTHON\"");
    fs_err::write(
        cwd.join("supplied_code.py"),
        "import sys\nprint('FILE_INPUT_RAN')\nsys.exit(7)\n",
    )
    .unwrap();
    let created = create_tab(&cwd, parent);
    for target in ["python", "python -i"] {
        let result = parse_command_result(&send_command(
            &created.tab_id,
            &format!("{target} < supplied_code.py"),
            10.0,
        ));
        assert!(result.finished);
        assert_eq!(result.exit_code, Some(7_i32), "stderr: {}", result.stderr);
        assert!(
            result.stdout.contains("FILE_INPUT_RAN"),
            "stdout: {}",
            result.stdout
        );
        assert_parent(&created.tab_id, parent);
    }
    let expected = parse_command_result(&send_command(
        &created.tab_id,
        &format!("{native} < {null}"),
        10.0,
    ));
    let empty = parse_command_result(&send_command(
        &created.tab_id,
        &format!("python < {null}"),
        10.0,
    ));
    assert!(empty.finished);
    assert_eq!(
        empty.exit_code, expected.exit_code,
        "stdout: {}\nstderr: {}",
        empty.stdout, empty.stderr
    );
    assert_eq!(empty.stdout, expected.stdout);
    assert_eq!(empty.stderr, expected.stderr);
    assert_parent(&created.tab_id, parent);
}
#[test]
fn piped_python_respects_output_redirection() {
    let (_guard, cwd, parent) = start_runtime("redirected output");
    let created = create_tab(&cwd, parent);
    let command = format!(
        "{} > captured-output.txt",
        pipe_command(parent, "print('REDIRECTED_OUTPUT')\n", "python")
    );
    let result = parse_command_result(&send_command(&created.tab_id, &command, 10.0));
    assert!(result.finished);
    assert_eq!(result.exit_code, Some(0_i32));
    assert_eq!(result.stdout.trim(), "");
    assert_eq!(result.stderr, "");
    let captured = fs_err::read_to_string(cwd.join("captured-output.txt")).unwrap();
    assert!(
        captured.contains("REDIRECTED_OUTPUT"),
        "captured: {captured}"
    );
    assert_parent(&created.tab_id, parent);
}
