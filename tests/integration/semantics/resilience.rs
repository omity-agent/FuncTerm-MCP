use crate::support::{
    complete_command, create_tab, locked_with_env, manual_write, parse_tab_view, run_cli,
    tagged_content, temp_dir, wait_for_completion,
};
#[test]
fn bash_noclobber_preserves_dispatch_and_user_redirections() {
    assert_environment_change("set -o noclobber");
}
#[test]
fn bash_empty_path_preserves_completion_and_dispatch() {
    assert_environment_change("PATH=");
}
fn assert_environment_change(change: &str) {
    let _guard = locked_with_env(&[]);
    for separated in ["true", "false"] {
        let tab = create_tab(&temp_dir("environment-changes"), "bash").tab_id;
        for source in [
            format!("{change}; printf 'CHANGED\\n'"),
            "printf 'NEXT_COMMAND\\n'; printf 'ERROR_STREAM\\n' >&2".to_owned(),
        ] {
            let result = wait_for_completion(&submit(&tab, &source, separated));
            let screen = parse_tab_view(&run_cli(&["view", &tab])).screen;
            assert_eq!(result.exit_code, Some(0_i32), "{result:?}\n{screen}");
            assert!(!screen.contains("command not found"), "{screen}");
        }
        if change.contains("noclobber") {
            let result =
                complete_command(&tab, "printf first > user-file; printf second > user-file");
            assert_ne!(result.exit_code, Some(0_i32), "{result:?}");
        }
    }
}
#[test]
fn window_title_persists_across_commands_and_tab_queries() {
    let _guard = locked_with_env(&[]);
    for (shell, source, next) in [
        (
            "bash",
            "PROMPT_COMMAND='printf \"\\033]2;SHELL_DEFAULT\\033\\\\\"'; printf '\\033]2;持久标题\\033\\\\'",
            "printf NEXT",
        ),
        (
            "python",
            "print('\\x1b]2;持久标题\\x1b\\\\', end='')",
            "print('NEXT')",
        ),
        (
            "bun",
            "console.log('\\x1b]2;持久标题\\x1b\\\\')",
            "console.log('NEXT')",
        ),
    ] {
        let tab = create_tab(&temp_dir("persistent-title"), shell).tab_id;
        complete_command(&tab, source);
        let accepted = crate::support::send_command(&tab, next, 0.0);
        let id = crate::support::parse_command_id(&accepted);
        wait_for_completion(&accepted);
        for response in [run_cli(&["view", &tab]), run_cli(&["view", &id])] {
            let document = String::from_utf8(response.stdout).unwrap();
            assert_eq!(
                tagged_content(&document, "TITLE").unwrap().trim(),
                "持久标题",
                "{document}"
            );
        }
    }
}
#[test]
fn bash_exit_replays_separated_streams_to_the_screen_once() {
    let _guard = locked_with_env(&[]);
    for separated in ["true", "false"] {
        let tab = create_tab(&temp_dir("exit-streams"), "bash").tab_id;
        let result = wait_for_completion(&submit(
            &tab,
            "printf 'LAST_STDOUT\\n'; printf 'LAST_STDERR\\n' >&2; exit 28",
            separated,
        ));
        assert_eq!(result.exit_code, Some(28_i32), "{result:?}");
        assert!(result.stdout.contains("LAST_STDOUT"), "{result:?}");
        assert!(
            format!("{}{}", result.stdout, result.stderr).contains("LAST_STDERR"),
            "{result:?}"
        );
        let deadline = std::time::Instant::now() + core::time::Duration::from_secs(20);
        let view = loop {
            let current = parse_tab_view(&run_cli(&["view", &tab, "--wait-timeout", "0.1"]));
            if !current.alive {
                break current;
            }
            assert!(std::time::Instant::now() < deadline, "{}", current.screen);
        };
        for marker in ["LAST_STDOUT", "LAST_STDERR"] {
            assert_eq!(view.screen.matches(marker).count(), 1, "{}", view.screen);
        }
    }
}
#[test]
fn bash_keyboard_echo_preserves_supplementary_unicode() {
    let _guard = locked_with_env(&[]);
    let tab = create_tab(&temp_dir("unicode-echo"), "bash").tab_id;
    let source = "IFS= read -r answer; printf '\\nVALUE=<%s>\\n' \"$answer\"";
    let accepted = submit(&tab, source, "false");
    assert!(manual_write(&tab, "🙂\n".as_bytes(), 0.0).status.success());
    let result = wait_for_completion(&accepted);
    assert!(result.stdout.contains("VALUE=<🙂>"), "{result:?}");
    let screen = parse_tab_view(&run_cli(&["view", &tab])).screen;
    assert_eq!(screen.matches('🙂').count(), 2, "{screen}");
    assert!(!screen.contains('\u{fffd}'), "{screen}");
}
fn submit(tab: &str, source: &str, separated: &str) -> std::process::Output {
    run_cli(&[
        "send-command",
        tab,
        "--command",
        source,
        "--separate-output",
        separated,
    ])
}
