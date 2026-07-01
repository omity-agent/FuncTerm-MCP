use serde::Deserialize;
use std::path::Path;
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct NewShellRequest {
    #[schemars(description = "初始工作目录。工作目录可以被后续命令改变。")]
    pub(super) cwd: Option<String>,
    #[schemars(description = "选择一种 Shell")]
    pub(super) shell: String,
}
impl NewShellRequest {
    pub(super) fn cwd_path(&self) -> Option<&Path> {
        self.cwd.as_deref().map(Path::new)
    }
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct WriteKeyboardRequest {
    pub(super) shell_id: String,
    pub(super) bytes: Vec<u8>,
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct SendCommandRequest {
    pub(super) shell_id: String,
    pub(super) command: String,
    #[schemars(
        description = "等待时长，单位为秒。输入 0 代表不等待命令执行。等待结束后命令不会被终止，仍可通过 Query 查看进展。"
    )]
    pub(super) waiting: f64,
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
pub(super) struct QueryRequest {
    #[schemars(
        description = "如果你输入 Shell 的 ID，你将查看到 Shell 视口范围内显示的内容；如果你输入命令的 ID，你将查看到命令目前已有的输出。"
    )]
    pub(super) id: String,
}
