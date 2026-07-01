use super::{RequestFrame, ResponseFrame};
use crate::runtime::protocol::{EndReason, Payload, QueryResult, Request, Response};
use crate::shell::ShellChoice;
use std::path::PathBuf;
#[test]
fn request_frames_round_trip_in_both_directions() {
    let requests = [
        Request::Ping,
        Request::NewShell {
            cwd: PathBuf::from("F:\\workspace\\shell-mcp"),
            shell: ShellChoice::PowerShell,
        },
        Request::NewShell {
            cwd: PathBuf::from("/tmp/shell-mcp"),
            shell: ShellChoice::Bash,
        },
        Request::WriteKeyboard {
            shell_id: "shell-keyboard".to_owned(),
            bytes: vec![0, b'a', b'\r', b'\n', 255],
        },
        Request::SendCommand {
            shell_id: "shell-command".to_owned(),
            command: "Write-Output 'hello'; Set-Location F:\\".to_owned(),
            wait_ms: 1234,
        },
        Request::Query {
            id: "command-query".to_owned(),
        },
    ];
    for request in &requests {
        assert_request_round_trip(request);
    }
}
#[test]
fn response_frames_round_trip_in_both_directions() {
    let responses = [
        Response::Ok {
            payload: Payload::Pong,
        },
        Response::Ok {
            payload: Payload::ShellCreated {
                shell_id: "shell-created".to_owned(),
            },
        },
        Response::Ok {
            payload: Payload::KeyboardWritten,
        },
        Response::Ok {
            payload: Payload::CommandAccepted {
                command_id: "command-ended".to_owned(),
                end_reason: EndReason::CommandEnded,
                query: QueryResult::Command {
                    cwd: "F:\\workspace\\shell-mcp".to_owned(),
                    finished: true,
                    stdout: "out\n".to_owned(),
                    stderr: "err\n".to_owned(),
                    exit_code: Some(7_i32),
                },
            },
        },
        Response::Ok {
            payload: Payload::CommandAccepted {
                command_id: "command-timeout".to_owned(),
                end_reason: EndReason::WaitTimeout,
                query: QueryResult::Shell {
                    alive: true,
                    cwd: "/tmp".to_owned(),
                    screen: "prompt> ".to_owned(),
                },
            },
        },
        Response::Ok {
            payload: Payload::Query(QueryResult::Command {
                cwd: "C:\\".to_owned(),
                finished: false,
                stdout: String::new(),
                stderr: String::new(),
                exit_code: None,
            }),
        },
        Response::Err {
            message: "failed to parse request: unknown request kind 99".to_owned(),
        },
    ];
    for response in &responses {
        assert_response_round_trip(response);
    }
}
fn assert_request_round_trip(request: &Request) {
    let frame = RequestFrame::from_request(request).unwrap();
    let decoded = RequestFrame {
        header: frame.header,
        payload: frame.payload.clone(),
    }
    .into_request()
    .unwrap();
    assert_eq!(&decoded, request);
    let encoded_again = RequestFrame::from_request(&decoded).unwrap();
    assert_eq!(encoded_again.header, frame.header);
    assert_eq!(encoded_again.payload, frame.payload);
}
fn assert_response_round_trip(response: &Response) {
    let frame = ResponseFrame::from_response(response).unwrap();
    let decoded = ResponseFrame {
        header: frame.header,
        payload: frame.payload.clone(),
    }
    .into_response()
    .unwrap();
    assert_eq!(&decoded, response);
    let encoded_again = ResponseFrame::from_response(&decoded).unwrap();
    assert_eq!(encoded_again.header, frame.header);
    assert_eq!(encoded_again.payload, frame.payload);
}
