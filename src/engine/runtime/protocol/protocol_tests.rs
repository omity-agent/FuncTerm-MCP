use super::{CommandView, KeyboardInput, Payload, Request, ShellView, ViewResult};
use crate::shell::ShellChoice;
use core::time::Duration;
#[doc = " 该程序输出的主要消费者为 LLM，如果输出中存在 JSON/XML 转义会增加认知负荷和无意义的上下文占用，使用伪结构化文本可读性更高。"]
#[test]
fn command_output_uses_uppercase_tags_without_escaping_content() {
    let text = ViewResult::Command {
        shell: shell(true),
        command: CommandView {
            stdout: "left < right".to_owned(),
            stderr: "raw </STDERR> allowed".to_owned(),
            exit_code: Some(0_i32),
            time_consumption: Duration::from_secs(1),
            finished: true,
        },
        note: String::new(),
    }
    .into_plain_text();
    assert!(text.contains("<CWD>\nF:/workspace/A&B\n</CWD>"));
    assert!(text.contains("<STDOUT>\nleft < right\n</STDOUT>"));
    assert!(text.contains("<STDERR>\nraw </STDERR> allowed\n</STDERR>"));
    assert!(!text.contains("<STDOUT>left < right"));
    assert!(!text.contains("<STDERR>raw </STDERR> allowed"));
}
#[test]
fn empty_command_output_does_not_insert_blank_line() {
    let text = ViewResult::Command {
        shell: shell(true),
        command: CommandView {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(0_i32),
            time_consumption: Duration::from_secs(1),
            finished: true,
        },
        note: String::new(),
    }
    .into_plain_text();
    assert!(text.contains("<STDOUT>\n</STDOUT>\n<STDERR>\n</STDERR>"));
    assert!(text.contains("<NOTE>\n</NOTE>"));
    assert!(!text.contains("<STDOUT>\n\n</STDOUT>"));
    assert!(!text.contains("<STDERR>\n\n</STDERR>"));
    assert!(!text.contains("<NOTE>\n\n</NOTE>"));
}
#[test]
fn command_output_does_not_duplicate_existing_line_ending() {
    for (line_ending, expected_stdout, expected_stderr) in [
        (
            "\n",
            "<STDOUT>\nstdout\n</STDOUT>",
            "<STDERR>\nstderr\n</STDERR>",
        ),
        (
            "\r\n",
            "<STDOUT>\nstdout\r\n</STDOUT>",
            "<STDERR>\nstderr\r\n</STDERR>",
        ),
    ] {
        let text = ViewResult::Command {
            shell: shell(true),
            command: CommandView {
                stdout: format!("stdout{line_ending}"),
                stderr: format!("stderr{line_ending}"),
                exit_code: Some(0_i32),
                time_consumption: Duration::from_secs(1),
                finished: true,
            },
            note: String::new(),
        }
        .into_plain_text();
        assert!(text.contains(expected_stdout));
        assert!(text.contains(expected_stderr));
    }
}
#[test]
fn tab_output_reports_shell_state() {
    let text = ViewResult::Tab {
        shell: shell(true),
        screen: "screen".to_owned(),
        note: String::new(),
    }
    .into_plain_text();
    assert!(text.contains("<SHELL>\n<ALIVE>\ntrue\n</ALIVE>"));
    assert!(text.contains("<TYPE>\nPowerShell\n</TYPE>"));
}
#[test]
fn manual_write_output_reports_screen() {
    let text = Payload::KeyboardWritten {
        view: ViewResult::Tab {
            shell: shell(true),
            screen: "running screen".to_owned(),
            note: String::new(),
        },
    }
    .into_plain_text();
    assert!(!text.contains("<ALIVE>"));
    assert!(text.contains("<SCREEN>\nrunning screen\n</SCREEN>"));
}
#[test]
fn plain_command_time_uses_milliseconds_seconds_and_minutes() {
    for (duration, expected) in [
        (Duration::from_micros(750_500), "750.50ms"),
        (Duration::from_micros(12_345), "12.35ms"),
        (Duration::from_millis(1_250), "1.25s"),
        (Duration::from_secs(90), "1.50min"),
    ] {
        let command = CommandView {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(0_i32),
            time_consumption: duration,
            finished: true,
        };
        let text = ViewResult::Command {
            shell: shell(true),
            command,
            note: String::new(),
        }
        .into_plain_text();
        assert!(text.contains(&format!(
            "<TIME_CONSUMPTION>\n{expected}\n</TIME_CONSUMPTION>"
        )));
    }
}
#[test]
fn structured_command_time_always_uses_milliseconds() {
    let command = CommandView {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: Some(0_i32),
        time_consumption: Duration::new(90, 250_125_000),
        finished: true,
    };
    assert_eq!(command.presentation(None).time_consumption, "90250.13ms");
}
#[test]
fn cwd_is_lexically_normalized_with_forward_slashes() {
    let shell = ShellView {
        #[cfg(windows)]
        cwd: r"F:\\workspace\.\source\..\target\\".to_owned(),
        #[cfg(unix)]
        cwd: "/workspace//./source/../target//".to_owned(),
        ..shell(true)
    };
    #[cfg(windows)]
    let expected = "F:/workspace/target/";
    #[cfg(unix)]
    let expected = "/workspace/target/";
    assert_eq!(shell.presentation(true).cwd, expected);
}
#[test]
fn manual_write_request_round_trip_preserves_input_kind_and_waiting() {
    for input in [
        KeyboardInput::Text("line\n".to_owned()),
        KeyboardInput::Bytes(vec![0, 3, 10, 255]),
    ] {
        let request = Request::ManualWrite {
            tab_id: "tab-round-trip".to_owned(),
            input,
            waiting: Duration::from_millis(1_250),
        };
        let serialized = sonic_rs::to_string(&request).unwrap();
        let restored = sonic_rs::from_str::<Request>(&serialized).unwrap();
        assert_eq!(restored, request);
    }
}
fn shell(alive: bool) -> ShellView {
    ShellView {
        alive,
        title: "title".to_owned(),
        shell_type: ShellChoice::PowerShell,
        cwd: "F:\\workspace\\A&B".to_owned(),
        idle: true,
    }
}
