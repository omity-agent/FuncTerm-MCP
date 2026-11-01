use crate::support::{complete_command, create_tab, locked_with_env, temp_dir};
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
        let launched = complete_command(&tab.tab_id, launch);
        assert_eq!(launched.exit_code, Some(0_i32), "{launched:?}");
        let nested = complete_command(&tab.tab_id, query);
        assert_eq!(nested.exit_code, Some(0_i32), "{nested:?}");
        assert!(nested.stdout.contains("NESTED_REPL_READY"), "{nested:?}");
        let exited = complete_command(&tab.tab_id, exit);
        assert_eq!(exited.exit_code, Some(code), "{exited:?}");
        let restored = complete_command(&tab.tab_id, "echo PARENT_RESTORED");
        assert_eq!(
            restored.exit_code,
            Some(if parent == "cmd" { code } else { 0_i32 }),
            "{restored:?}"
        );
        assert!(restored.stdout.contains("PARENT_RESTORED"), "{restored:?}");
    }
}
