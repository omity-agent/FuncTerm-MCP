use crate::support::{
    command_directory, command_view, complete_command, create_tab, locked_with_env, manual_write,
    parse_command_id, run, run_with_output, tab_view, temp_dir, wait_for_completion,
    wait_for_removal,
};
use core::time::Duration;
use std::time::Instant;
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
#[test]
fn nested_exit_keeps_the_tab_busy_until_the_parent_continuation_returns() {
    let _guard = locked_with_env(&[]);
    let cwd = temp_dir("nested-continuation");
    fs_err::create_dir(cwd.join("resumed")).unwrap();
    let scenarios: [(&str, &[u8], &str, bool); 2] = [
        (
            "IFS= read -r answer; cd resumed",
            b"CONTINUED\n",
            "ANSWER=CONTINUED",
            true,
        ),
        ("sleep 30", &[3], "ANSWER=", false),
    ];
    for (continuation, input, expected, moved) in scenarios {
        let tab = create_tab(&cwd, "bash").tab_id;
        let source = format!("python; printf 'PARENT_WAITING\\n'; {continuation}");
        let launched = run_with_output(&tab, &source, false, 0.0);
        assert_eq!(wait_for_completion(&launched).exit_code, Some(0_i32));
        let exiting = run(&tab, "raise SystemExit(7)", 0.0);
        let id = parse_command_id(&exiting);
        let until = Instant::now() + Duration::from_secs(15);
        loop {
            let view = tab_view(&tab);
            if view.screen.contains("PARENT_WAITING") {
                break;
            }
            assert!(
                Instant::now() < until,
                "parent did not resume: {}",
                view.screen
            );
        }
        assert!(!command_view(&id).finished);
        assert!(manual_write(&tab, input, 0.0).status.success());
        assert_eq!(wait_for_completion(&exiting).exit_code, Some(7_i32));
        let next = complete_command(&tab, "printf 'ANSWER=%s\\n' \"$answer\"");
        assert_eq!(next.cwd.ends_with("resumed"), moved, "{next:?}");
        assert_eq!(next.stdout.trim(), expected, "{next:?}");
        let launch_id = parse_command_id(&launched);
        assert_eq!(command_view(&launch_id).exit_code, Some(0_i32));
        wait_for_removal(&command_directory(&tab, &launch_id));
    }
}
