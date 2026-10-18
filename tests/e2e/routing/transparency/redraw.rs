use super::{checked_text, parent_shell, run_terminal, terminal_output};
use crate::support::{
    create_tab, locked_with_env, manual_write, parse_command_id, parse_command_result, run_cli,
    send_command, temp_dir,
};
use core::time::Duration;
use std::time::Instant;

#[test]
fn terminal_state_replaces_progress_instead_of_appending_frames() {
    let _guard = locked_with_env(&[]);
    let directory = temp_dir("progress-redraw");
    let tab = create_tab(&directory, parent_shell());
    let cases = [
        (
            "FIRST_10%\\r\\x1b[2KSECOND_50%",
            "\\r\\x1b[2KFINAL_100%\\n",
            "FINAL_100%",
        ),
        (
            "FIRST_10%\\nSECOND_50%\\n",
            "\\x1b[2F\\x1b[2KFINAL_A\\n\\x1b[2KFINAL_B\\n",
            "FINAL_A\nFINAL_B",
        ),
        (
            "FIRST_10%\\nSECOND_50%\\n",
            "\\x1b[2J\\x1b[HFINAL_CLEARED\\n",
            "FINAL_CLEARED",
        ),
        ("FIRST_10%", "\\r\\x1b[2K记录_完成\\n", "记录_完成"),
        ("FIRST_10%", "\\rDONE\\n", "DONET_10%"),
        ("VALUE_123", "\\b\\b\\b999\\n", "VALUE_999"),
        ("REPEATED\\n", "REPEATED\\n", "REPEATED\nREPEATED"),
    ];
    for (initial, replacement, expected) in cases {
        let command = format!(
            "python -c \"import sys,time; sys.stdout.write('{initial}'); sys.stdout.flush(); time.sleep(0.2); sys.stdout.write('{replacement}'); sys.stdout.flush()\""
        );
        let response = run_terminal(&tab.tab_id, &command, "10");
        let text = checked_text(&response);
        let output = terminal_output(&text).trim_end_matches('\n');
        assert_eq!(output, expected, "{text}");
        assert!(!output.contains('\r'), "{text}");
        assert!(!output.contains('\u{1b}'), "{text}");
        let saved = checked_text(&run_cli(&["view", &parse_command_id(&response)]));
        assert_eq!(terminal_output(&saved), terminal_output(&text));
    }
    let empty = checked_text(&run_terminal(&tab.tab_id, "python -c \"pass\"", "10"));
    assert_eq!(terminal_output(&empty), "");
    let separated = parse_command_result(&send_command(
        &tab.tab_id,
        "python -c \"import sys; print('SEPARATE_OUT'); print('SEPARATE_ERR', file=sys.stderr)\"",
        10.0,
    ));
    assert_eq!(separated.exit_code, Some(0_i32));
    assert_eq!(separated.stdout.trim(), "SEPARATE_OUT");
    assert_eq!(separated.stderr.trim(), "SEPARATE_ERR");
}

#[test]
fn terminal_state_queries_refresh_and_finished_results_stay_frozen() {
    let _guard = locked_with_env(&[]);
    let directory = temp_dir("live-rendering");
    let tab = create_tab(&directory, parent_shell());
    let command = "python -c \"print('PENDING_10%', end='', flush=True); input(); print('\\x1b[1F\\r\\x1b[2KCOMPLETE_100%', flush=True)\"";
    let response = run_terminal(&tab.tab_id, command, "0.1");
    let id = parse_command_id(&response);
    let mut pending = checked_text(&response);
    let started = Instant::now();
    while !terminal_output(&pending).contains("PENDING_10%") {
        assert!(started.elapsed() < Duration::from_secs(10), "{pending}");
        pending = checked_text(&run_cli(&["view", &id, "--wait-timeout", "0.05"]));
    }
    assert!(pending.contains("<FINISHED>\nfalse\n"), "{pending}");
    assert_eq!(terminal_output(&pending).trim(), "PENDING_10%");
    let input = manual_write(&tab.tab_id, b"\r", 0.1);
    assert!(input.status.success(), "{input:?}");
    let finished = checked_text(&run_cli(&["view", &id, "--wait-timeout", "10"]));
    assert_eq!(terminal_output(&finished).trim(), "COMPLETE_100%");
    assert!(finished.contains("<FINISHED>\ntrue\n"), "{finished}");
    let next = checked_text(&run_terminal(&tab.tab_id, "echo NEXT_COMMAND", "10"));
    assert_eq!(terminal_output(&next).trim(), "NEXT_COMMAND");
    let saved = checked_text(&run_cli(&["view", &id]));
    assert_eq!(terminal_output(&saved), terminal_output(&finished));
}

#[test]
fn terminal_state_preserves_wrapped_unicode_across_scrollback() {
    let _guard = locked_with_env(&[]);
    let directory = temp_dir("wrapped-history");
    let tab = create_tab(&directory, parent_shell());
    let command =
        "python -c \"[print('记录_%03d_' % i + '宽' * 130, flush=True) for i in range(180)]\"";
    let response = run_terminal(&tab.tab_id, command, "10");
    let text = checked_text(&response);
    let expected = (0_usize..180)
        .map(|index| format!("记录_{index:03}_{}", "宽".repeat(130)))
        .collect::<Vec<_>>()
        .join("\n");
    let output = terminal_output(&text).trim_end_matches('\n');
    assert!(
        output.replace('\n', "") == expected.replace('\n', ""),
        "wrapped terminal output changed Unicode content"
    );
    #[cfg(unix)]
    assert!(output == expected, "soft-wrapped logical lines changed");
}

#[test]
fn terminal_state_exports_the_active_alternate_screen() {
    let _guard = locked_with_env(&[]);
    let directory = temp_dir("alternate-buffer");
    let tab = create_tab(&directory, parent_shell());
    let previous = run_terminal(&tab.tab_id, "echo BEFORE_ALTERNATE", "10");
    assert!(previous.status.success(), "{previous:?}");
    let command = "python -c \"import sys; sys.stdout.write('\\x1b[?1049h\\x1b[H\\x1b[2KALT_PROGRESS'); sys.stdout.flush(); input(); sys.stdout.write('\\x1b[?1049l'); print('MAIN_FINISHED', flush=True)\"";
    let response = run_terminal(&tab.tab_id, command, "0.1");
    let id = parse_command_id(&response);
    let mut pending = checked_text(&response);
    let started = Instant::now();
    while !terminal_output(&pending).contains("ALT_PROGRESS") {
        assert!(started.elapsed() < Duration::from_secs(10), "{pending}");
        pending = checked_text(&run_cli(&["view", &id, "--wait-timeout", "0.05"]));
    }
    assert_eq!(terminal_output(&pending).trim(), "ALT_PROGRESS");
    let input = manual_write(&tab.tab_id, b"\r", 0.1);
    assert!(input.status.success(), "{input:?}");
    let finished = checked_text(&run_cli(&["view", &id, "--wait-timeout", "10"]));
    assert_eq!(terminal_output(&finished).trim(), "MAIN_FINISHED");
}
