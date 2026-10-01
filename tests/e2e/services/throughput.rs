use crate::support::{
    locked, parse_command_result, parse_tab_created, parse_tab_view, run_cli, run_cli_with_pipes,
    send_command, temp_root,
};
#[test]
fn cli_streams_large_command_output_without_closing_the_tab() {
    const OUTPUT_SIZE: usize = 2 * 1024 * 1024;
    let _guard = locked();
    let created = parse_tab_created(&run_cli_with_pipes(&[
        "new-tab",
        "--starting-directory",
        temp_root().to_str().unwrap(),
        "--starting-shell",
        "powershell",
    ]));
    let command = parse_command_result(&send_command(
        &created.tab_id,
        &format!("Write-Output ('x' * {OUTPUT_SIZE})"),
        10.0,
    ));
    let output_bytes = command.stdout.bytes().filter(|byte| *byte == b'x').count();
    assert_eq!(output_bytes, OUTPUT_SIZE, "large stdout was truncated");
    let view = parse_tab_view(&run_cli(&["view", &created.tab_id]));
    assert!(
        view.alive,
        "large output should not close the PowerShell tab"
    );
}
