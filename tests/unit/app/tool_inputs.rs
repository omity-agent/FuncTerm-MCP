use super::ManualWriteExec;
use crate::runtime::protocol::KeyboardInput;
fn parse_manual_write_request(json: &str) -> ManualWriteExec {
    match sonic_rs::from_str(json) {
        Ok(request) => request,
        Err(error) => panic!("request should be valid json: {error}"),
    }
}
fn accepted_parts(request: ManualWriteExec) -> (String, KeyboardInput) {
    match request.into_parts() {
        Ok(parts) => parts,
        Err(error) => panic!("request should be accepted: {error}"),
    }
}
fn rejected_error(request: ManualWriteExec) -> String {
    match request.into_parts() {
        Ok(_) => panic!("request should be rejected"),
        Err(error) => error.to_string(),
    }
}
#[test]
fn manual_write_accepts_text() {
    let request = parse_manual_write_request(r#"{"tab_id":"tab","text":"echo 你好\n"}"#);
    let (tab_id, input) = accepted_parts(request);
    assert_eq!(tab_id, "tab");
    assert_eq!(input, KeyboardInput::Text("echo 你好\n".to_owned()));
}
#[test]
fn manual_write_accepts_bytes() {
    let request = parse_manual_write_request(r#"{"tab_id":"tab","bytes":[3,10]}"#);
    let (tab_id, input) = accepted_parts(request);
    assert_eq!(tab_id, "tab");
    assert_eq!(input, KeyboardInput::Bytes(vec![3, 10]));
}
#[test]
fn manual_write_rejects_text_and_bytes_together() {
    let request = parse_manual_write_request(r#"{"tab_id":"tab","text":"x","bytes":[120]}"#);
    let error = rejected_error(request);
    assert_eq!(error, "text and bytes cannot be provided together");
}
#[test]
fn manual_write_rejects_missing_input() {
    let request = parse_manual_write_request(r#"{"tab_id":"tab"}"#);
    let error = rejected_error(request);
    assert_eq!(error, "either text or bytes must be provided");
}
