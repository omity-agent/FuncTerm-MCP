use crate::support::{
    complete_command, create_tab, locked_with_env, manual_write, parse_tab_view, run_cli,
    run_with_output, tagged_content, temp_dir, wait_for_completion,
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
    for separated in [true, false] {
        let tab = create_tab(&temp_dir("environment-changes"), "bash").tab_id;
        for source in [
            format!("{change}; printf 'CHANGED\\n'"),
            "printf 'NEXT_COMMAND\\n'; printf 'ERROR_STREAM\\n' >&2".to_owned(),
        ] {
            let result = wait_for_completion(&run_with_output(&tab, &source, separated, 0.0));
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
        ("bash", r"printf '\033]2;持久标题\033\\'", "printf NEXT"),
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
        if shell == "bash" {
            complete_command(
                &tab,
                r#"PROMPT_COMMAND='printf "\033]1;PROMPT_ICON\007\033]2;PROMPT_WINDOW\007"'"#,
            );
        }
        for command in [source, next] {
            let accepted = crate::support::run(&tab, command, 0.0);
            let id = crate::support::parse_command_id(&accepted);
            assert_eq!(wait_for_completion(&accepted).exit_code, Some(0_i32));
            for response in [run_cli(&["view", &tab]), run_cli(&["view", &id])] {
                let document = String::from_utf8(response.stdout).unwrap();
                assert_eq!(
                    tagged_content(&document, "TITLE"),
                    Some("\n持久标题\n"),
                    "{document}"
                );
            }
        }
    }
}
#[test]
fn bash_exit_replays_separated_streams_to_the_screen_once() {
    let _guard = locked_with_env(&[]);
    for separated in [true, false] {
        let tab = create_tab(&temp_dir("exit-streams"), "bash").tab_id;
        let result = wait_for_completion(&run_with_output(
            &tab,
            "printf 'LAST_STDOUT\\n'; printf 'LAST_STDERR\\n' >&2; exit 28",
            separated,
            0.0,
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
fn keyboard_roundtrip_preserves_supplementary_unicode() {
    let _guard = locked_with_env(&[]);
    for (shell, source, input, echoes) in [
        (
            "bash",
            if cfg!(windows) {
                "IFS= read -rs answer; printf '\\nVALUE=<%s>\\n' \"$answer\""
            } else {
                "IFS= read -r answer; printf '\\nVALUE=<%s>\\n' \"$answer\""
            },
            "🙂\n",
            if cfg!(windows) { 1 } else { 2 },
        ),
        #[cfg(windows)]
        (
            "powershell",
            "$answer = [Console]::ReadLine(); [Console]::WriteLine(\"VALUE=<{0}>\", $answer)",
            "🙂\r",
            2,
        ),
    ] {
        let tab = create_tab(&temp_dir("unicode-input"), shell).tab_id;
        let accepted = run_with_output(&tab, source, false, 0.0);
        assert!(manual_write(&tab, input.as_bytes(), 0.0).status.success());
        let result = wait_for_completion(&accepted);
        assert!(result.stdout.contains("VALUE=<🙂>"), "{shell}: {result:?}");
        let screen = parse_tab_view(&run_cli(&["view", &tab])).screen;
        assert_eq!(screen.matches('🙂').count(), echoes, "{shell}: {screen}");
        assert!(!screen.contains('\u{fffd}'), "{shell}: {screen}");
    }
}
