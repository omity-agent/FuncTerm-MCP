use super::{checked_text, run_terminal, terminal_output};
use crate::support::{create_tab, locked_with_env, temp_dir};
#[test]
fn terminal_powershell_keeps_delayed_object_formatting() {
    let _guard = locked_with_env(&[]);
    let directory = temp_dir("terminal-formatting");
    let tab = create_tab(&directory, "powershell");
    let result = run_terminal(
        &tab.tab_id,
        "[pscustomobject]@{Name='STRUCTURED_TERMINAL_OBJECT'; Value=42}",
        "10",
    );
    let text = checked_text(&result);
    let output = terminal_output(&text);
    assert!(output.contains("STRUCTURED_TERMINAL_OBJECT"), "{text}");
    assert!(output.contains("42"), "{text}");
    assert!(text.contains("<EXIT_CODE>\n0\n"), "{text}");
}
