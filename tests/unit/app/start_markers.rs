#[test]
fn command_start_publishes_capture_marker_and_started_file() {
    let directory = crate::test_fs::temp_dir("command-start-marker");
    let mut output = Vec::new();
    super::write_start_to("command-a", &directory, &mut output).unwrap();
    assert_eq!(output, b"\x1b]9999;FuncTerm;start;command-a\x1b\\");
    assert!(
        directory
            .join(crate::contract::COMMAND_STATE_DIRECTORY)
            .join(crate::contract::STARTED_FILE)
            .is_file()
    );
    std::fs::remove_dir_all(directory).unwrap();
}
#[cfg(unix)]
#[test]
fn unix_restores_model_title_before_capture_marker() {
    let directory = crate::test_fs::temp_dir("unix-title-restore");
    for (title, sequence) in [("", "\x1b]2;\x1b\\"), ("模型😀", "\x1b]2;模型😀\x1b\\")] {
        let mut output = Vec::new();
        super::restore_model_title(&mut output, title).unwrap();
        super::write_start_to("command-a", &directory, &mut output).unwrap();
        assert_eq!(
            output,
            format!("{sequence}\x1b]9999;FuncTerm;start;command-a\x1b\\").as_bytes()
        );
        std::fs::remove_file(
            directory
                .join(crate::contract::COMMAND_STATE_DIRECTORY)
                .join(crate::contract::STARTED_FILE),
        )
        .unwrap();
    }
    let mut output = Vec::new();
    assert!(super::restore_model_title(&mut output, "unsafe\x1btitle").is_err());
    assert_eq!(
        output, b"",
        "invalid titles should be rejected before writing output"
    );
    std::fs::remove_dir_all(directory).unwrap();
}
