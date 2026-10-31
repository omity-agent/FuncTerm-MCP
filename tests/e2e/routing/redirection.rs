use super::fixtures::{assert_parent, start_runtime};
use crate::support::{complete_command, create_tab};
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
        let result = complete_command(&created.tab_id, &format!("{target} < supplied_code.py"));
        assert!(result.finished, "{target}: {result:?}");
        assert_eq!(result.exit_code, Some(7_i32), "stderr: {}", result.stderr);
        assert!(
            result.stdout.contains("FILE_INPUT_RAN"),
            "stdout: {}",
            result.stdout
        );
        assert_parent(&created.tab_id, parent);
    }
    let expected = complete_command(&created.tab_id, &format!("{native} < {null}"));
    let empty = complete_command(&created.tab_id, &format!("python < {null}"));
    assert!(empty.finished, "{empty:?}");
    assert_eq!(
        empty.exit_code, expected.exit_code,
        "stdout: {}\nstderr: {}",
        empty.stdout, empty.stderr
    );
    assert_eq!(empty.stdout, expected.stdout);
    assert_eq!(empty.stderr, expected.stderr);
    assert_parent(&created.tab_id, parent);
}
