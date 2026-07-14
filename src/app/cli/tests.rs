use serde::Deserialize;
#[derive(Deserialize)]
struct WrittenDone {
    command_id: String,
    exit_code: i32,
    time_consumption: String,
    cwd: String,
}
#[test]
fn internal_done_writer_serializes_json_strings() {
    let directory = crate::test_fs::temp_dir("internal-done-writer");
    let done = crate::app::command_state::DoneOutput {
        command_id: "command\"id",
        exit_code: 7,
        time_consumption: "123.456ms",
        cwd: "cwd\nwith\\chars",
    };
    let mut terminal_output = Vec::new();
    crate::app::command_state::write_done_to(&done, &directory, &mut terminal_output).unwrap();
    assert_eq!(terminal_output, b"\x1b]9999;FuncTerm;end;command\"id\x1b\\");
    let text = std::fs::read_to_string(
        directory
            .join(crate::contract::COMMAND_STATE_DIRECTORY)
            .join(crate::contract::DONE_FILE),
    )
    .unwrap();
    let written = sonic_rs::from_str::<WrittenDone>(&text).unwrap();
    assert_eq!(written.command_id, "command\"id");
    assert_eq!(written.exit_code, 7_i32);
    assert_eq!(written.time_consumption, "123.456ms");
    assert_eq!(written.cwd, "cwd\nwith\\chars");
    std::fs::remove_dir_all(directory).unwrap();
}
