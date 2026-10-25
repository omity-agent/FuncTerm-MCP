use crate::support::{create_tab, locked_with_env, parse_command_result, send_command, temp_dir};

#[test]
fn interpreter_shims_restore_parent_after_python_exit_and_bun_native_exit() {
    let _guard = locked_with_env(&[]);
    let parent = if cfg!(windows) { "cmd" } else { "bash" };
    for (launch, query, exit, code) in [
        (
            "python",
            "print('NESTED_REPL_READY')",
            "import sys; sys.exit(7)",
            7_i32,
        ),
        (
            "bun repl",
            "console.log('NESTED_REPL_READY')",
            ".exit",
            0_i32,
        ),
    ] {
        let tab = create_tab(&temp_dir("nested-interpreter"), parent);
        let launched = parse_command_result(&send_command(&tab.tab_id, launch, 15.0));
        assert!(launched.finished, "{launched:?}");
        assert_eq!(launched.exit_code, Some(0_i32), "{launched:?}");
        let nested = parse_command_result(&send_command(&tab.tab_id, query, 5.0));
        assert_eq!(nested.exit_code, Some(0_i32), "{nested:?}");
        assert!(nested.stdout.contains("NESTED_REPL_READY"), "{nested:?}");
        let exited = parse_command_result(&send_command(&tab.tab_id, exit, 5.0));
        assert!(exited.finished, "{exited:?}");
        assert_eq!(exited.exit_code, Some(code), "{exited:?}");
        let restored =
            parse_command_result(&send_command(&tab.tab_id, "echo PARENT_RESTORED", 5.0));
        assert_eq!(restored.exit_code, Some(0_i32), "{restored:?}");
        assert!(restored.stdout.contains("PARENT_RESTORED"), "{restored:?}");
    }
}
