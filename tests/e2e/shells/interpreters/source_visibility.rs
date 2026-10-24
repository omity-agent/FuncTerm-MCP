use crate::support::{
    create_tab, locked_with_env, manual_write, parse_command_id, parse_command_result,
    parse_tab_view, run_cli, temp_dir,
};
use core::time::Duration;
use std::{fs, process::Output, thread, time::Instant};
#[test]
fn submitted_repl_source_is_hidden_without_losing_results() {
    let _guard = locked_with_env(&[]);
    for (shell, source) in [
        (
            "python",
            "source_only = 'REPL_SOURCE_ONLY'\nprint('REPL_' + 'RESULT')\nimport sys; print('REPL_' + 'ERROR', file=sys.stderr)\n40 + 2",
        ),
        (
            "bun",
            "var sourceOnly = 'REPL_SOURCE_ONLY';\nconsole.log('REPL_' + 'RESULT');\nconsole.error('REPL_' + 'ERROR');\n40 + 2",
        ),
    ] {
        let tab = create_tab(&temp_dir("source-visibility"), shell);
        for separated in [false, true] {
            let result = submit(&tab.tab_id, source, separated, "10");
            let text = result_text(&result, separated);
            for expected in ["REPL_RESULT", "REPL_ERROR", "42"] {
                assert!(text.contains(expected), "{shell}: {text}");
            }
            assert!(!text.contains("REPL_SOURCE_ONLY"), "{shell}: {text}");
            let saved = run_cli(&["view", &parse_command_id(&result)]);
            assert_eq!(result_text(&saved, separated), text);
            let view = parse_tab_view(&run_cli(&["view", &tab.tab_id]));
            assert!(
                !view.screen.contains("REPL_SOURCE_ONLY"),
                "{shell}: {}",
                view.screen
            );
            if shell == "bun" {
                let awaited = submit(&tab.tab_id, "await Promise.resolve(6 * 7)", separated, "10");
                assert_eq!(result_text(&awaited, separated).trim(), "42");
            }
        }
    }
}
#[test]
fn manual_repl_input_keeps_its_echo() {
    let _guard = locked_with_env(&[]);
    for (shell, source) in [
        (
            "python",
            "response = input('INPUT_READY:'); print('INPUT_ACCEPTED')",
        ),
        (
            "bun",
            r"console.log('INPUT_READY:'); await new Promise(resolve => { function accept(data) { if (!/[\r\n]/.test(data.toString())) return; process.stdin.off('data', accept); console.log('INPUT_ACCEPTED'); resolve(); } process.stdin.on('data', accept); })",
        ),
    ] {
        let tab = create_tab(&temp_dir("interactive-echo"), shell);
        for separated in [false, true] {
            let ready = if separated {
                "INPUT_READY_SEPARATED:"
            } else {
                "INPUT_READY_TERMINAL:"
            };
            let command_source = source.replace("INPUT_READY:", ready);
            let pending = submit(&tab.tab_id, &command_source, separated, "1");
            let command_id = parse_command_id(&pending);
            wait_for_prompt(&tab.tab_id, &command_id, ready, separated);
            let input = if cfg!(windows) { "\r" } else { "\n" };
            let typed = if shell == "bun" {
                "// REPL_TYPED_INPUT"
            } else {
                "REPL_TYPED_INPUT"
            };
            let written = manual_write(&tab.tab_id, format!("{typed}{input}").as_bytes(), 1.0);
            assert!(written.status.success(), "{written:?}");
            let completed = run_cli(&["view", &command_id, "--wait-timeout", "10"]);
            assert!(
                String::from_utf8_lossy(&completed.stdout).contains("<FINISHED>\ntrue\n"),
                "{completed:?}"
            );
            let text = result_text(&completed, separated);
            assert!(text.contains("INPUT_ACCEPTED"), "{shell}: {text}");
            let view = parse_tab_view(&run_cli(&["view", &tab.tab_id]));
            assert!(
                view.screen.contains("REPL_TYPED_INPUT"),
                "{shell}: {}",
                view.screen
            );
        }
    }
}
#[test]
fn bun_native_commands_keep_working_without_terminal_source_echo() {
    let _guard = locked_with_env(&[]);
    let cwd = temp_dir("native-command-visibility");
    fs::write(
        cwd.join("loaded.js"),
        "var loadedSource = 'REPL_LOADED_SOURCE';\n40 + 2\n",
    )
    .unwrap();
    let tab = create_tab(&cwd, "bun");
    let definition = "var savedSource = 'REPL_SAVED_SOURCE'";
    let defined = submit(&tab.tab_id, definition, false, "10");
    assert!(!result_text(&defined, false).contains("REPL_SAVED_SOURCE"));
    let help = submit(&tab.tab_id, ".help", false, "10");
    let help_text = result_text(&help, false);
    for command in [".break", ".clear", ".exit", ".help", ".load", ".save"] {
        assert!(help_text.contains(command), "{help_text}");
    }
    let saved = submit(&tab.tab_id, ".save saved.js", false, "10");
    assert!(result_text(&saved, false).contains("Session saved"));
    assert!(
        fs::read_to_string(cwd.join("saved.js"))
            .unwrap()
            .contains(definition)
    );
    let loaded = submit(&tab.tab_id, ".load loaded.js", false, "10");
    let loaded_text = result_text(&loaded, false);
    assert!(loaded_text.contains("42"), "{loaded_text}");
    assert!(!loaded_text.contains("REPL_LOADED_SOURCE"), "{loaded_text}");
    let multiline = submit(&tab.tab_id, "({\nvalue: 42\n})", false, "10");
    assert!(
        result_text(&multiline, false).contains("{ value: 42 }"),
        "{multiline:?}"
    );
}
fn submit(tab_id: &str, source: &str, separated: bool, timeout: &str) -> Output {
    run_cli(&[
        "send-command",
        tab_id,
        "--command",
        source,
        "--separate-output",
        if separated { "true" } else { "false" },
        "--wait-timeout",
        timeout,
    ])
}
fn wait_for_prompt(tab_id: &str, command_id: &str, marker: &str, separated: bool) {
    let started = Instant::now();
    loop {
        let view = parse_tab_view(&run_cli(&["view", tab_id]));
        let command = run_cli(&["view", command_id]);
        let output = result_text(&command, separated);
        if view.screen.contains(marker) || output.contains(marker) {
            return;
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "{output}\n{}",
            view.screen
        );
        thread::sleep(Duration::from_millis(50));
    }
}
fn result_text(result: &Output, separated: bool) -> String {
    assert!(result.status.success(), "{result:?}");
    if separated {
        let parsed = parse_command_result(result);
        return format!("{}{}", parsed.stdout, parsed.stderr);
    }
    let text = String::from_utf8(result.stdout.clone()).unwrap();
    text.split_once("<OUTPUT>\n")
        .unwrap()
        .1
        .split_once("</OUTPUT>")
        .unwrap()
        .0
        .to_owned()
}
