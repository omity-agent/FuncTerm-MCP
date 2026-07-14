use crate::runtime::protocol::KeyboardInput;
use crate::shell::ShellChoice;
use anyhow::{Result, bail};
use serde::Deserialize;
use std::path::Path;
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct NewTabRequest {
    pub(super) starting_directory: Option<String>,
    #[schemars(description = "启动时使用的 Shell")]
    pub(super) starting_shell: ShellChoice,
}
impl NewTabRequest {
    pub(super) fn starting_directory_path(&self) -> Option<&Path> {
        self.starting_directory.as_deref().map(Path::new)
    }
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct ManualWriteRequest {
    pub(super) tab_id: String,
    #[serde(default)]
    #[schemars(description = "要写入的 UTF-8 文本")]
    pub(super) text: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "要写入的原始字节"
    )]
    pub(super) bytes: Option<Vec<u8>>,
    #[schemars(
        description = "等待终端产生新输出的最长时长，单位为秒。输入 0 代表写入后立即返回，此时可能尚未更新。"
    )]
    pub(super) waiting: f64,
}
impl ManualWriteRequest {
    pub(super) fn into_parts(self) -> Result<(String, KeyboardInput, f64)> {
        let Self {
            tab_id,
            text,
            bytes,
            waiting,
        } = self;
        let input = match (text, bytes) {
            (Some(input_text), None) => KeyboardInput::Text(input_text),
            (None, Some(input_bytes)) => KeyboardInput::Bytes(input_bytes),
            (Some(_), Some(_)) => bail!("text and bytes cannot be provided together"),
            (None, None) => bail!("either text or bytes must be provided"),
        };
        Ok((tab_id, input, waiting))
    }
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct SendCommandRequest {
    pub(super) tab_id: String,
    pub(super) command: String,
    #[schemars(
        description = "等待时长，单位为秒。一般建议设置略大于预期时长。时间结束后命令不会被终止，仍可通过 view 查看其状态。输入 0 代表不等待，适用于不会自然停止的程序（如 `vim`）。"
    )]
    pub(super) waiting: f64,
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct ViewRequest {
    #[schemars(
        description = "如果输入命令 ID，将返回该命令目前的状态；如果输入标签页 ID，将返回终端屏幕范围内显示的内容。一般输入命令 ID。"
    )]
    pub(super) id: String,
    #[schemars(description = "等待时长，单位为秒。")]
    pub(super) waiting: f64,
}
#[cfg(test)]
mod tests {
    use super::ManualWriteRequest;
    use crate::runtime::protocol::KeyboardInput;
    fn parse_manual_write_request(json: &str) -> ManualWriteRequest {
        match sonic_rs::from_str(json) {
            Ok(request) => request,
            Err(error) => panic!("request should be valid json: {error}"),
        }
    }
    fn accepted_parts(request: ManualWriteRequest) -> (String, KeyboardInput, f64) {
        match request.into_parts() {
            Ok(parts) => parts,
            Err(error) => panic!("request should be accepted: {error}"),
        }
    }
    fn rejected_error(request: ManualWriteRequest) -> String {
        match request.into_parts() {
            Ok(_) => panic!("request should be rejected"),
            Err(error) => error.to_string(),
        }
    }
    #[test]
    fn manual_write_accepts_text() {
        let request =
            parse_manual_write_request(r#"{"tab_id":"tab","text":"echo 你好\n","waiting":1.5}"#);
        let (tab_id, input, waiting) = accepted_parts(request);
        assert_eq!(tab_id, "tab");
        assert_eq!(input, KeyboardInput::Text("echo 你好\n".to_owned()));
        assert!((waiting - 1.5_f64).abs() < f64::EPSILON);
    }
    #[test]
    fn manual_write_accepts_bytes() {
        let request = parse_manual_write_request(r#"{"tab_id":"tab","bytes":[3,10],"waiting":0}"#);
        let (tab_id, input, waiting) = accepted_parts(request);
        assert_eq!(tab_id, "tab");
        assert_eq!(input, KeyboardInput::Bytes(vec![3, 10]));
        assert!(waiting.abs() < f64::EPSILON);
    }
    #[test]
    fn manual_write_rejects_text_and_bytes_together() {
        let request =
            parse_manual_write_request(r#"{"tab_id":"tab","text":"x","bytes":[120],"waiting":0}"#);
        let error = rejected_error(request);
        assert_eq!(error, "text and bytes cannot be provided together");
    }
    #[test]
    fn manual_write_rejects_missing_input() {
        let request = parse_manual_write_request(r#"{"tab_id":"tab","waiting":0}"#);
        let error = rejected_error(request);
        assert_eq!(error, "either text or bytes must be provided");
    }
}
